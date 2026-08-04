/**
 * magnetite-web-client/src/connection.ts
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
import { redirectUrl, verifyRedirect, followRedirect, type FollowRoute } from './follow.js';
import type { ServerMessage } from './types';

const DEFAULT_RECONNECT_INITIAL_MS = 500;
const DEFAULT_RECONNECT_MAX_MS = 16000;
const DEFAULT_RECONNECT_FACTOR = 2;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ConnectionManagerOptions {
  /** WebSocket URL (ws:// or wss://) */
  url: string;
  /** Optional auth token (appended as ?token=) */
  token?: string;
  reconnectInitialMs?: number;
  reconnectMaxMs?: number;
  /** default true */
  autoReconnect?: boolean;
}

/** What `onFollowed` receives once a session-follow completes. */
export type FollowedInfo = FollowRoute & { nodeKey: string };

export interface EnableSessionFollowOptions {
  /** hex node key of the current server */
  nodeKey: string;
  /** our current player id */
  getPlayerId: () => number;
  onFollowed?: (info: FollowedInfo) => void;
  /** called when a redirect is refused */
  onRefused?: (err: Error) => void;
  /** injectable for tests */
  openSocket?: (url: string) => WebSocket;
}

interface FollowState {
  nodeKey: string;
  getPlayerId: () => number;
  onFollowed: ((info: FollowedInfo) => void) | null;
  onRefused: ((err: Error) => void) | null;
  openSocket: ((url: string) => WebSocket) | null;
}

function errMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

// ---------------------------------------------------------------------------
// ConnectionManager
// ---------------------------------------------------------------------------

export class ConnectionManager {
  private _baseUrl: string;
  private _token: string | null;
  private _autoReconnect: boolean;
  private _reconnectInitialMs: number;
  private _reconnectMaxMs: number;

  private _ws: WebSocket | null;
  private _reconnectDelay: number;
  private _reconnectTimer: ReturnType<typeof setTimeout> | null;
  private _closed: boolean;

  // Message handler registry: type → fn(msg)
  private _handlers: Map<string, (msg: ServerMessage) => void>;

  private _onOpen: (() => void) | null;
  private _onClose: ((event?: CloseEvent) => void) | null;
  private _onError: ((event: Event) => void) | null;

  // Session follow (see follow.js). Off unless `enableSessionFollow` is
  // called: without a pinned node key there is nothing to verify a redirect
  // against, and an unverifiable redirect must never be followed.
  private _follow: FollowState | null;

  constructor(opts: ConnectionManagerOptions) {
    this._baseUrl = opts.url;
    this._token = opts.token || null;
    this._autoReconnect = opts.autoReconnect !== false;
    this._reconnectInitialMs = opts.reconnectInitialMs || DEFAULT_RECONNECT_INITIAL_MS;
    this._reconnectMaxMs = opts.reconnectMaxMs || DEFAULT_RECONNECT_MAX_MS;

    this._ws = null;
    this._reconnectDelay = this._reconnectInitialMs;
    this._reconnectTimer = null;
    this._closed = false;

    this._handlers = new Map();

    this._onOpen = null;
    this._onClose = null;
    this._onError = null;

    this._follow = null;
  }

  /**
   * Follow migrated shards to their new node.
   *
   * Requires `nodeKey` — the hex node key of the server this session is
   * connected to, learned out of band (a signed discovery ad, or the
   * `target_key` this connection was itself followed to). The redirect's issuer
   * signature is checked against it; an address alone is never an identity.
   */
  enableSessionFollow(opts: EnableSessionFollowOptions): this {
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
   * @returns whether the follow succeeded
   */
  private async _handleRedirect(msg: ServerMessage): Promise<boolean> {
    const f = this._follow;
    if (!f) {
      // No pinned key ⇒ nothing could verify this. Ignore it rather than
      // reconnect somewhere on an unauthenticated instruction.
      console.warn('[magnetite] ignoring redirect: session follow is not enabled');
      return false;
    }
    try {
      const route = await verifyRedirect(msg.redirect as object, {
        issuerKey: f.nodeKey,
        playerId: f.getPlayerId(),
      });
      const url = redirectUrl(route.addr, this._buildUrl());
      const socket = await followRedirect({
        url,
        targetKey: route.targetKey,
        redirect: msg.redirect as object,
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
      console.warn('[magnetite] refusing session redirect:', errMessage(e));
      if (f.onRefused) f.onRefused(e instanceof Error ? e : new Error(errMessage(e)));
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
  connect(): void {
    if (this._ws && this._ws.readyState < WebSocket.CLOSING) return;
    this._closed = false;
    this._openSocket();
  }

  /**
   * Close the connection permanently (no reconnect).
   */
  disconnect(): void {
    this._closed = true;
    this._cancelReconnect();
    if (this._ws) {
      this._ws.close(1000, 'client disconnect');
      this._ws = null;
    }
    if (this._onClose) this._onClose();
  }

  /**
   * @returns true if the socket is open and ready
   */
  get isConnected(): boolean {
    return this._ws !== null && this._ws.readyState === WebSocket.OPEN;
  }

  // --------------------------------------------------------------------------
  // Sending
  // --------------------------------------------------------------------------

  /**
   * Send a JSON string to the server.
   * No-ops if not connected.
   */
  send(message: string): void {
    if (this.isConnected) {
      this._ws!.send(message);
    }
  }

  // --------------------------------------------------------------------------
  // Event registration
  // --------------------------------------------------------------------------

  /**
   * Register a handler for a ServerNet message type.
   *
   * @param type - snake_case type tag (e.g. 'welcome', 'ack')
   */
  on(type: string, handler: (msg: ServerMessage) => void): this {
    this._handlers.set(type, handler);
    return this;
  }

  /** Called when the socket opens (before Welcome) */
  set onOpen(fn: (() => void) | null) { this._onOpen = fn; }
  /** Called when the socket closes (after all retries or explicit disconnect) */
  set onClose(fn: ((event?: CloseEvent) => void) | null) { this._onClose = fn; }
  /** Called on a socket error event */
  set onError(fn: ((event: Event) => void) | null) { this._onError = fn; }

  // --------------------------------------------------------------------------
  // Internal
  // --------------------------------------------------------------------------

  private _buildUrl(): string {
    const url = this._token
      ? `${this._baseUrl}${this._baseUrl.includes('?') ? '&' : '?'}token=${encodeURIComponent(this._token)}`
      : this._baseUrl;
    return url;
  }

  private _openSocket(): void {
    this._adopt(new WebSocket(this._buildUrl()));
  }

  /**
   * Attach this manager's handlers to a socket — either one we just opened, or
   * one handed back by a completed session follow.
   */
  private _adopt(ws: WebSocket): void {
    this._ws = ws;

    ws.addEventListener('open', () => {
      this._reconnectDelay = this._reconnectInitialMs;
      if (this._onOpen) this._onOpen();
    });

    ws.addEventListener('message', (event: MessageEvent<string>) => {
      const msg = parseServerMessage(event.data);
      if (!msg) return;
      if (msg.type === 'redirect') {
        // Handled here rather than by a user handler: following a redirect is a
        // security decision, not application logic.
        void this._handleRedirect(msg);
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

    ws.addEventListener('close', (event: CloseEvent) => {
      this._ws = null;
      if (!this._closed && this._autoReconnect) {
        this._scheduleReconnect();
      } else {
        if (this._onClose) this._onClose(event);
      }
    });

    ws.addEventListener('error', (event: Event) => {
      if (this._onError) this._onError(event);
    });

    // A socket adopted from a completed follow is already open, so its 'open'
    // event fired before we were listening. Run the same bookkeeping.
    if (ws.readyState === 1) {
      this._reconnectDelay = this._reconnectInitialMs;
      if (this._onOpen) this._onOpen();
    }
  }

  private _scheduleReconnect(): void {
    const delay = this._reconnectDelay;
    this._reconnectDelay = Math.min(
      this._reconnectDelay * DEFAULT_RECONNECT_FACTOR,
      this._reconnectMaxMs
    );
    this._reconnectTimer = setTimeout(() => {
      if (!this._closed) this._openSocket();
    }, delay);
  }

  private _cancelReconnect(): void {
    if (this._reconnectTimer !== null) {
      clearTimeout(this._reconnectTimer);
      this._reconnectTimer = null;
    }
  }
}
