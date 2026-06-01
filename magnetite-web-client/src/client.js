/**
 * magnetite-web-client/src/client.js
 *
 * Public API entry point for the Magnetite web client.
 *
 * Usage:
 *   import { createClient } from './magnetite-web-client/src/client.js';
 *
 *   const client = createClient({
 *     url:    'ws://localhost:9001',  // magnetite dev server
 *     token:  'optional-jwt',
 *     canvas: document.getElementById('game'),  // HTMLCanvasElement (optional)
 *     render: (ctx, state, playerId) => { ... }, // optional custom renderer
 *   });
 *
 *   client.connect();
 *   client.onState(state => { ... });  // subscribe to predicted state updates
 *   client.disconnect();
 *
 * Protocol mapping (authority.rs / protocol.rs):
 *
 *   ServerNet::Welcome     → sets player_id + config; starts tick loop
 *   ServerNet::Snapshot    → full authoritative reset; prediction.applySnapshot
 *   ServerNet::Delta       → per-tick diff; applyDeltaToSnapshot + prediction update
 *   ServerNet::Ack         → prediction.ack(seq, tick)
 *   ServerNet::Reject      → prediction.reject(seq)
 *   ClientNet::InputFrame  → sent each tick as { type:'input_frame', seq, tick, input }
 */

import { ConnectionManager } from './connection.js';
import { PredictionBuffer, arenaApplyInput } from './prediction.js';
import { InputCapture } from './input-capture.js';
import { renderArenaView } from './renderer.js';
import { encodeInputFrame, decodeBytes } from './protocol.js';
import { applyDeltaToSnapshot, snapshotToView } from './delta.js';

// ---------------------------------------------------------------------------
// createClient
// ---------------------------------------------------------------------------

/**
 * @typedef {Object} ClientOptions
 * @property {string}  url                   - WebSocket URL
 * @property {string}  [token]               - Optional auth token
 * @property {HTMLCanvasElement} [canvas]    - Canvas to render into
 * @property {RenderFn} [render]             - Custom render function
 * @property {ApplyInputFn} [applyInput]     - Custom prediction function
 * @property {boolean} [autoReconnect=true]  - Whether to reconnect on drop
 */

/**
 * @callback RenderFn
 * @param {CanvasRenderingContext2D} ctx
 * @param {unknown} state
 * @param {string | null} localPlayerId
 */

/**
 * @callback ApplyInputFn
 * @param {unknown} state
 * @param {import('./types.js').Input} input
 * @param {number} tick
 * @returns {unknown}
 */

/**
 * Create a Magnetite game client.
 *
 * @param {ClientOptions} opts
 * @returns {MagnetiteClient}
 */
export function createClient(opts) {
  return new MagnetiteClient(opts);
}

// ---------------------------------------------------------------------------
// MagnetiteClient
// ---------------------------------------------------------------------------

class MagnetiteClient {
  /**
   * @param {ClientOptions} opts
   */
  constructor(opts) {
    this._opts = opts;

    /** @type {string | null} The local player's id, assigned by Welcome */
    this._playerId = null;

    /** @type {object | null} MatchConfig from Welcome */
    this._config = null;

    /** @type {number} Current client-predicted tick */
    this._tick = 0;

    /** @type {number} Monotonically increasing sequence number for InputFrames */
    this._seq = 0;

    /** @type {import('./prediction.js').PredictionBuffer} */
    this._prediction = new PredictionBuffer(
      opts.applyInput || arenaApplyInput
    );

    /** @type {import('./input-capture.js').InputCapture | null} */
    this._input = opts.canvas
      ? new InputCapture(opts.canvas)
      : (typeof window !== 'undefined' ? new InputCapture(window) : null);

    /** @type {CanvasRenderingContext2D | null} */
    this._ctx = opts.canvas ? opts.canvas.getContext('2d') : null;

    /** @type {RenderFn} */
    this._render = opts.render || renderArenaView;

    /** @type {Set<(state: unknown) => void>} */
    this._stateListeners = new Set();

    /** @type {number | null} requestAnimationFrame handle */
    this._rafHandle = null;

    /** @type {number | null} setInterval tick handle */
    this._tickHandle = null;

    /** @type {number} server tick rate in Hz (from Welcome or default 60) */
    this._tickHz = 60;

    /** @type {import('./connection.js').ConnectionManager} */
    this._conn = new ConnectionManager({
      url: opts.url,
      token: opts.token,
      autoReconnect: opts.autoReconnect !== false,
    });

    this._wireHandlers();
  }

  // --------------------------------------------------------------------------
  // Public API
  // --------------------------------------------------------------------------

  /**
   * Open the WebSocket connection.
   * @returns {this}
   */
  connect() {
    if (this._input) this._input.attach();
    this._conn.connect();
    return this;
  }

  /**
   * Close the connection and stop the tick loop.
   */
  disconnect() {
    this._stopLoop();
    this._conn.disconnect();
    if (this._input) this._input.detach();
  }

  /**
   * Send a pre-built input directly (advanced use).
   * Increments seq and predicts locally.
   *
   * @param {import('./types.js').Input} input
   */
  sendInput(input) {
    const seq = ++this._seq;
    const tick = this._tick;
    const predicted = this._prediction.predict(seq, tick, input);
    this._conn.send(encodeInputFrame(seq, tick, input));
    this._emitState(predicted);
  }

  /**
   * Subscribe to state updates.
   * Callback fires on every local prediction step AND on authoritative updates.
   *
   * @param {(state: unknown) => void} fn
   * @returns {() => void} unsubscribe function
   */
  onState(fn) {
    this._stateListeners.add(fn);
    // Immediately call with current state if available
    const current = this._prediction.state;
    if (current !== null) fn(current);
    return () => this._stateListeners.delete(fn);
  }

  /**
   * @returns {string | null} the local player id (set after Welcome)
   */
  get playerId() {
    return this._playerId;
  }

  /**
   * @returns {object | null} the MatchConfig (set after Welcome)
   */
  get matchConfig() {
    return this._config;
  }

  /**
   * @returns {unknown} the current predicted state
   */
  get state() {
    return this._prediction.state;
  }

  // --------------------------------------------------------------------------
  // Server message handlers
  // --------------------------------------------------------------------------

  _wireHandlers() {
    this._conn.on('welcome', (msg) => this._handleWelcome(msg));
    this._conn.on('snapshot', (msg) => this._handleSnapshot(msg));
    this._conn.on('delta', (msg) => this._handleDelta(msg));
    this._conn.on('ack', (msg) => this._handleAck(msg));
    this._conn.on('reject', (msg) => this._handleReject(msg));
  }

  /**
   * ServerNet::Welcome { player_id, config }
   */
  _handleWelcome(msg) {
    this._playerId = String(msg.player_id);
    this._config = msg.config || null;
    if (this._config && this._config.tick_hz) {
      this._tickHz = Number(this._config.tick_hz);
    }
    this._tick = 0;
    this._startLoop();
  }

  /**
   * ServerNet::Snapshot { tick, full }
   *
   * `full` is a Vec<u8> serialised snapshot (JSON of ArenaSnapshot).
   * serde_json serialises Vec<u8> as a base64 string.
   */
  _handleSnapshot(msg) {
    const tick = Number(msg.tick);
    const snapshot = decodeBytes(msg.full);
    if (!snapshot) return;

    // advance local tick to server tick
    if (tick > this._tick) this._tick = tick;

    // Build an ArenaView from the snapshot for this player
    const view = snapshotToView(snapshot, this._playerId);
    this._prediction.applySnapshot(view, tick);
    this._emitState(view);
    this._renderFrame();
  }

  /**
   * ServerNet::Delta { tick, since_tick, diff }
   *
   * `diff` is a Vec<u8> serialised ArenaDelta.
   */
  _handleDelta(msg) {
    const tick = Number(msg.tick);
    const delta = decodeBytes(msg.diff);
    if (!delta) return;

    // advance local tick
    if (tick > this._tick) this._tick = tick;

    // Get current base from authoritative state
    const authSnap = this._prediction.authoritativeState;
    if (!authSnap) return;

    // Convert view → snapshot for delta application
    const snapForDelta = _viewToSnapshot(authSnap);
    const newSnap = applyDeltaToSnapshot(snapForDelta, delta, tick);
    const newView = snapshotToView(newSnap, this._playerId);

    this._prediction.applySnapshot(newView, tick);
    this._emitState(newView);
    this._renderFrame();
  }

  /**
   * ServerNet::Ack { seq, tick }
   */
  _handleAck(msg) {
    this._prediction.ack(Number(msg.seq), Number(msg.tick));
  }

  /**
   * ServerNet::Reject { seq, reason }
   */
  _handleReject(msg) {
    console.warn('[magnetite] input rejected:', msg.reason, '(seq', msg.seq, ')');
    this._prediction.reject(Number(msg.seq));
    const state = this._prediction.state;
    if (state) this._emitState(state);
  }

  // --------------------------------------------------------------------------
  // Tick loop
  // --------------------------------------------------------------------------

  _startLoop() {
    this._stopLoop();
    const intervalMs = Math.round(1000 / this._tickHz);
    this._tickHandle = setInterval(() => this._tick_step(), intervalMs);
    if (this._ctx) this._scheduleRaf();
  }

  _stopLoop() {
    if (this._tickHandle !== null) {
      clearInterval(this._tickHandle);
      this._tickHandle = null;
    }
    if (this._rafHandle !== null) {
      cancelAnimationFrame(this._rafHandle);
      this._rafHandle = null;
    }
  }

  _tick_step() {
    if (!this._conn.isConnected) return;
    this._tick++;

    // Capture input snapshot
    const input = this._input
      ? this._input.snapshot(this._seq + 1, Date.now())
      : _emptyInput(this._seq + 1);

    this.sendInput(input);
  }

  _scheduleRaf() {
    this._rafHandle = requestAnimationFrame(() => {
      this._renderFrame();
      this._rafHandle = null;
      if (this._ctx) this._scheduleRaf();
    });
  }

  _renderFrame() {
    if (!this._ctx) return;
    const state = this._prediction.state;
    if (!state) return;
    try {
      this._render(this._ctx, state, this._playerId);
    } catch (e) {
      console.error('[magnetite] render error:', e);
    }
  }

  // --------------------------------------------------------------------------
  // Internal helpers
  // --------------------------------------------------------------------------

  _emitState(state) {
    for (const fn of this._stateListeners) {
      try {
        fn(state);
      } catch (e) {
        console.error('[magnetite] onState listener error:', e);
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Convert an ArenaView back to a minimal ArenaSnapshot for delta application.
 * (Views contain self_state + other_players; snapshots contain players[])
 *
 * @param {import('./types.js').ArenaView} view
 * @returns {import('./types.js').ArenaSnapshot}
 */
function _viewToSnapshot(view) {
  if (!view) return { players: [], projectiles: [], tick: 0 };

  const players = [];
  if (view.self_state) players.push(view.self_state);
  if (view.other_players) players.push(...view.other_players);

  return {
    players,
    projectiles: view.projectiles || [],
    tick: view.tick || 0,
  };
}

/**
 * Return a no-op input for ticks when input capture is unavailable.
 *
 * @param {number} seq
 * @returns {import('./types.js').Input}
 */
function _emptyInput(seq) {
  return {
    keys: {
      forward: false, backward: false, left: false, right: false,
      jump: false, crouch: false, attack: false, secondary_attack: false,
      interact: false, sprint: false,
    },
    mouse: {
      x: 0, y: 0, delta_x: 0, delta_y: 0,
      left_button: false, right_button: false, middle_button: false,
      scroll: 0,
    },
    sequence: seq,
    timestamp_ms: Date.now(),
  };
}

// ---------------------------------------------------------------------------
// Re-export lower-level primitives for advanced usage
// ---------------------------------------------------------------------------

export { PredictionBuffer, arenaApplyInput } from './prediction.js';
export { renderArenaView } from './renderer.js';
export { InputCapture } from './input-capture.js';
export { ConnectionManager } from './connection.js';
export { applyDeltaToSnapshot, snapshotToView } from './delta.js';
export { encodeInputFrame, parseServerMessage, decodeBytes,
  defaultInput, defaultKeyState, defaultMouseState } from './protocol.js';
