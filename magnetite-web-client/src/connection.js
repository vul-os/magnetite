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
    const ws = new WebSocket(this._buildUrl());
    this._ws = ws;

    ws.addEventListener('open', () => {
      this._reconnectDelay = this._reconnectInitialMs;
      if (this._onOpen) this._onOpen();
    });

    ws.addEventListener('message', (event) => {
      const msg = parseServerMessage(event.data);
      if (!msg) return;
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
