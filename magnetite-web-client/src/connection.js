/**
 * magnetite-web-client/src/connection.js
 *
 * WebSocket connection manager for the Magnetite authoritative server.
 *
 * Protocol:
 *  - Connects to ws[s]://<host>?token=<optional>
 *  - On open: waits for ServerNet::Welcome { player_id, config }
 *  - Sends ClientNet::InputFrame { seq, tick, input } each tick
 *  - Handles ServerNet::Snapshot, Delta, Ack, Reject
 *
 * Reconnection: exponential backoff up to maxReconnectDelay ms.
 */

import { parseServerMessage } from './protocol.js';
import { redirectUrl, verifyRedirect, followRedirect } from './follow.js';

const DEFAULT_RECONNECT_INITIAL_MS = 500;
const DEFAULT_RECONNECT_MAX_MS = 16000;
const DEFAULT_RECONNECT_FACTOR = 2;

// ---------------------------------------------------------------------------
// ConnectionManager
// ---------------------------------------------------------------------------

export class ConnectionManager {
  /**
   * @param {object} opts
   * @param {string}  opts.url   - WebSocket URL (ws:// or wss://)
   * @param {string}  [opts.token] - Optional auth token (appended as ?token=)
   * @param {number}  [opts.reconnectInitialMs]
   * @param {number}  [opts.reconnectMaxMs]
   * @param {boolean} [opts.autoReconnect=true]
   */
  constructor(opts) {
    this._baseUrl = opts.url;
    this._token = opts.token || null;
    this._autoReconnect = opts.autoReconnect !== false;
    this._reconnectInitialMs = opts.reconnectInitialMs || DEFAULT_RECONNECT_INITIAL_MS;
    this._reconnectMaxMs = opts.reconnectMaxMs || DEFAULT_RECONNECT_MAX_MS;

    /** @type {WebSocket | null} */
    this._ws = null;
    this._reconnectDelay = this._reconnectInitialMs;
    this._reconnectTimer = null;
    this._closed = false;

    // Message handler registry: type → fn(msg)
    /** @type {Map<string, (msg: object) => void>} */
    this._handlers = new Map();

    this._onOpen = null;
    this._onClose = null;
    this._onError = null;

    // Session follow (see follow.js). Off unless `enableSessionFollow` is
    // called: without a pinned node key there is nothing to verify a redirect
    // against, and an unverifiable redirect must never be followed.
    this._follow = null;
  }

  /**
   * Follow migrated shards to their new node.
   *
   * Requires `nodeKey` — the hex node key of the server this session is
   * connected to, learned out of band (a signed discovery ad, or the
   * `target_key` this connection was itself followed to). The redirect's issuer
   * signature is checked against it; an address alone is never an identity.
   *
   * @param {object} opts
   * @param {string} opts.nodeKey - hex node key of the current server
   * @param {() => number} opts.getPlayerId - our current player id
   * @param {(info: {nodeKey: string, addr: string, shard: number, epoch: number}) => void} [opts.onFollowed]
   * @param {(err: Error) => void} [opts.onRefused] - called when a redirect is refused
   * @param {(url: string) => WebSocket} [opts.openSocket] - injectable for tests
   */
  enableSessionFollow(opts) {
    this._follow = {
      nodeKey: opts.nodeKey,
      getPlayerId: opts.getPlayerId,
      onFollowed: opts.onFollowed || null,
      onRefused: opts.onRefused || null,
      openSocket: opts.openSocket || null,
    };
    return this;
  }

  /**
   * Handle a `ServerNet::Redirect`: verify it, follow it, and continue the
   * session on the new node.
   *
   * Every failure path leaves this connection exactly as it was and reports via
   * `onRefused` — a refused redirect is not a reason to go anywhere.
   *
   * @param {object} msg - the redirect frame
   * @returns {Promise<boolean>} whether the follow succeeded
   */
  async _handleRedirect(msg) {
    const f = this._follow;
    if (!f) {
      // No pinned key ⇒ nothing could verify this. Ignore it rather than
      // reconnect somewhere on an unauthenticated instruction.
      console.warn('[magnetite] ignoring redirect: session follow is not enabled');
      return false;
    }
    try {
      const route = await verifyRedirect(msg.redirect, {
        issuerKey: f.nodeKey,
        playerId: f.getPlayerId(),
      });
      const url = redirectUrl(route.addr, this._buildUrl());
      const socket = await followRedirect({
        url,
        targetKey: route.targetKey,
        redirect: msg.redirect,
        ...(f.openSocket ? { openSocket: f.openSocket } : {}),
      });
      // Adopt the proven connection. The node key we now trust is the one we
      // pinned and the far side proved — not whatever answered at the address.
      this._cancelReconnect();
      if (this._ws) {
        try {
          this._ws.close(1000, 'followed to new node');
        } catch {
          /* already gone */
        }
      }
      f.nodeKey = route.targetKey;
      this._baseUrl = url;
      this._adopt(socket);
      if (f.onFollowed) f.onFollowed({ nodeKey: route.targetKey, ...route });
      return true;
    } catch (e) {
      console.warn('[magnetite] refusing session redirect:', e.message);
      if (f.onRefused) f.onRefused(e);
      return false;
    }
  }

  // --------------------------------------------------------------------------
  // Lifecycle
  // --------------------------------------------------------------------------

  /**
   * Open the WebSocket connection.
   * Safe to call multiple times — no-ops if already connected.
   */
  connect() {
    if (this._ws && this._ws.readyState < WebSocket.CLOSING) return;
    this._closed = false;
    this._openSocket();
  }

  /**
   * Close the connection permanently (no reconnect).
   */
  disconnect() {
    this._closed = true;
    this._cancelReconnect();
    if (this._ws) {
      this._ws.close(1000, 'client disconnect');
      this._ws = null;
    }
    if (this._onClose) this._onClose();
  }

  /**
   * @returns {boolean} true if the socket is open and ready
   */
  get isConnected() {
    return this._ws !== null && this._ws.readyState === WebSocket.OPEN;
  }

  // --------------------------------------------------------------------------
  // Sending
  // --------------------------------------------------------------------------

  /**
   * Send a JSON string to the server.
   * No-ops if not connected.
   *
   * @param {string} message
   */
  send(message) {
    if (this.isConnected) {
      this._ws.send(message);
    }
  }

  // --------------------------------------------------------------------------
  // Event registration
  // --------------------------------------------------------------------------

  /**
   * Register a handler for a ServerNet message type.
   *
   * @param {string} type  - snake_case type tag (e.g. 'welcome', 'ack')
   * @param {(msg: object) => void} handler
   */
  on(type, handler) {
    this._handlers.set(type, handler);
    return this;
  }

  /** Called when the socket opens (before Welcome) */
  set onOpen(fn) { this._onOpen = fn; }
  /** Called when the socket closes (after all retries or explicit disconnect) */
  set onClose(fn) { this._onClose = fn; }
  /** Called on a socket error event */
  set onError(fn) { this._onError = fn; }

  // --------------------------------------------------------------------------
  // Internal
  // --------------------------------------------------------------------------

  _buildUrl() {
    const url = this._token
      ? `${this._baseUrl}${this._baseUrl.includes('?') ? '&' : '?'}token=${encodeURIComponent(this._token)}`
      : this._baseUrl;
    return url;
  }

  _openSocket() {
    this._adopt(new WebSocket(this._buildUrl()));
  }

  /**
   * Attach this manager's handlers to a socket — either one we just opened, or
   * one handed back by a completed session follow.
   *
   * @param {WebSocket} ws
   */
  _adopt(ws) {
    this._ws = ws;

    ws.addEventListener('open', () => {
      this._reconnectDelay = this._reconnectInitialMs;
      if (this._onOpen) this._onOpen();
    });

    ws.addEventListener('message', (event) => {
      const msg = parseServerMessage(event.data);
      if (!msg) return;
      if (msg.type === 'redirect') {
        // Handled here rather than by a user handler: following a redirect is a
        // security decision, not application logic.
        this._handleRedirect(msg);
        return;
      }
      const handler = this._handlers.get(msg.type);
      if (handler) {
        try {
          handler(msg);
        } catch (e) {
          console.error('[magnetite] handler error for', msg.type, e);
        }
      }
    });

    ws.addEventListener('close', (event) => {
      this._ws = null;
      if (!this._closed && this._autoReconnect) {
        this._scheduleReconnect();
      } else {
        if (this._onClose) this._onClose(event);
      }
    });

    ws.addEventListener('error', (event) => {
      if (this._onError) this._onError(event);
    });

    // A socket adopted from a completed follow is already open, so its 'open'
    // event fired before we were listening. Run the same bookkeeping.
    if (ws.readyState === 1) {
      this._reconnectDelay = this._reconnectInitialMs;
      if (this._onOpen) this._onOpen();
    }
  }

  _scheduleReconnect() {
    const delay = this._reconnectDelay;
    this._reconnectDelay = Math.min(
      this._reconnectDelay * DEFAULT_RECONNECT_FACTOR,
      this._reconnectMaxMs
    );
    this._reconnectTimer = setTimeout(() => {
      if (!this._closed) this._openSocket();
    }, delay);
  }

  _cancelReconnect() {
    if (this._reconnectTimer !== null) {
      clearTimeout(this._reconnectTimer);
      this._reconnectTimer = null;
    }
  }
}
