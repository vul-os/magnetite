/**
 * magnetite-web-client/src/client.ts
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
import { PredictionBuffer, arenaApplyInput, type ApplyInputFn } from './prediction';
import { InputCapture } from './input-capture';
import { renderArenaView } from './renderer';
import { encodeInputFrame, decodeBytes } from './protocol';
import { applyDeltaToSnapshot, snapshotToView } from './delta';
import type { ArenaSnapshot, ArenaView, Input, MatchConfig } from './types';

// ---------------------------------------------------------------------------
// createClient
// ---------------------------------------------------------------------------

/** Custom render function — draws `state` (always the arena-shooter's
 * ArenaView shape; see the module doc for why the wire protocol is not
 * pluggable even though prediction/rendering are) onto `ctx`. */
export type RenderFn = (
  ctx: CanvasRenderingContext2D,
  state: ArenaView,
  localPlayerId: string | null,
) => void;

export interface ClientOptions {
  /** WebSocket URL */
  url: string;
  /** Optional auth token */
  token?: string;
  /** Canvas to render into */
  canvas?: HTMLCanvasElement;
  /** Custom render function */
  render?: RenderFn;
  /** Custom local-prediction function */
  applyInput?: ApplyInputFn<ArenaView | null>;
  /** Whether to reconnect on drop (default true) */
  autoReconnect?: boolean;
}

/**
 * Create a Magnetite game client.
 */
export function createClient(opts: ClientOptions): MagnetiteClient {
  return new MagnetiteClient(opts);
}

// ---------------------------------------------------------------------------
// Server message shapes (the raw JSON each ServerNet handler receives)
// ---------------------------------------------------------------------------

interface WelcomeMessage {
  player_id: string | number;
  config?: MatchConfig;
}

interface SnapshotMessage {
  tick: number | string;
  full: string | number[] | null | undefined;
}

interface DeltaMessage {
  tick: number | string;
  since_tick?: number | string;
  diff: string | number[] | null | undefined;
}

interface AckMessage {
  seq: number | string;
  tick: number | string;
}

interface RejectMessage {
  seq: number | string;
  reason?: string;
}

// ---------------------------------------------------------------------------
// MagnetiteClient
// ---------------------------------------------------------------------------

class MagnetiteClient {
  private _opts: ClientOptions;
  /** The local player's id, assigned by Welcome */
  private _playerId: string | null;
  /** MatchConfig from Welcome */
  private _config: MatchConfig | null;
  /** Current client-predicted tick */
  private _tick: number;
  /** Monotonically increasing sequence number for InputFrames */
  private _seq: number;
  private _prediction: PredictionBuffer<ArenaView | null>;
  private _input: InputCapture | null;
  private _ctx: CanvasRenderingContext2D | null;
  private _render: RenderFn;
  private _stateListeners: Set<(state: ArenaView | null) => void>;
  /** requestAnimationFrame handle */
  private _rafHandle: number | null;
  /** setInterval tick handle */
  private _tickHandle: ReturnType<typeof setInterval> | null;
  /** server tick rate in Hz (from Welcome or default 60) */
  private _tickHz: number;
  private _conn: ConnectionManager;

  constructor(opts: ClientOptions) {
    this._opts = opts;
    this._playerId = null;
    this._config = null;
    this._tick = 0;
    this._seq = 0;

    this._prediction = new PredictionBuffer<ArenaView | null>(
      opts.applyInput || arenaApplyInput
    );

    this._input = opts.canvas
      ? new InputCapture(opts.canvas)
      : (typeof window !== 'undefined' ? new InputCapture(window) : null);

    this._ctx = opts.canvas ? opts.canvas.getContext('2d') : null;

    this._render = opts.render || renderArenaView;

    this._stateListeners = new Set();

    this._rafHandle = null;
    this._tickHandle = null;
    this._tickHz = 60;

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
   */
  connect(): this {
    if (this._input) this._input.attach();
    this._conn.connect();
    return this;
  }

  /**
   * Close the connection and stop the tick loop.
   */
  disconnect(): void {
    this._stopLoop();
    this._conn.disconnect();
    if (this._input) this._input.detach();
  }

  /**
   * Send a pre-built input directly (advanced use).
   * Increments seq and predicts locally.
   */
  sendInput(input: Input): void {
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
   * @returns unsubscribe function
   */
  onState(fn: (state: ArenaView | null) => void): () => void {
    this._stateListeners.add(fn);
    // Immediately call with current state if available
    const current = this._prediction.state;
    if (current !== null) fn(current);
    return () => this._stateListeners.delete(fn);
  }

  /** the local player id (set after Welcome) */
  get playerId(): string | null {
    return this._playerId;
  }

  /** the MatchConfig (set after Welcome) */
  get matchConfig(): MatchConfig | null {
    return this._config;
  }

  /** the current predicted state */
  get state(): ArenaView | null {
    return this._prediction.state;
  }

  // --------------------------------------------------------------------------
  // Server message handlers
  // --------------------------------------------------------------------------

  private _wireHandlers(): void {
    this._conn.on('welcome', (msg) => this._handleWelcome(msg));
    this._conn.on('snapshot', (msg) => this._handleSnapshot(msg as unknown as SnapshotMessage));
    this._conn.on('delta', (msg) => this._handleDelta(msg as unknown as DeltaMessage));
    this._conn.on('ack', (msg) => this._handleAck(msg as unknown as AckMessage));
    this._conn.on('reject', (msg) => this._handleReject(msg as unknown as RejectMessage));
  }

  /**
   * ServerNet::Welcome { player_id, config }
   *
   * Takes `unknown` (not `WelcomeMessage`) so this stays the exact shape a
   * caller replacing it (e.g. GamePreview.tsx, which wraps this to also flip
   * connection status to "connected") needs to match.
   */
  private _handleWelcome(msg: unknown): void {
    const m = msg as WelcomeMessage;
    this._playerId = String(m.player_id);
    this._config = m.config || null;
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
  private _handleSnapshot(msg: SnapshotMessage): void {
    const tick = Number(msg.tick);
    const snapshot = decodeBytes(msg.full) as ArenaSnapshot | null;
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
  private _handleDelta(msg: DeltaMessage): void {
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
    const newSnap = applyDeltaToSnapshot(snapForDelta, delta as Parameters<typeof applyDeltaToSnapshot>[1], tick);
    const newView = snapshotToView(newSnap, this._playerId);

    this._prediction.applySnapshot(newView, tick);
    this._emitState(newView);
    this._renderFrame();
  }

  /**
   * ServerNet::Ack { seq, tick }
   */
  private _handleAck(msg: AckMessage): void {
    this._prediction.ack(Number(msg.seq), Number(msg.tick));
  }

  /**
   * ServerNet::Reject { seq, reason }
   */
  private _handleReject(msg: RejectMessage): void {
    console.warn('[magnetite] input rejected:', msg.reason, '(seq', msg.seq, ')');
    this._prediction.reject(Number(msg.seq));
    const state = this._prediction.state;
    if (state) this._emitState(state);
  }

  // --------------------------------------------------------------------------
  // Tick loop
  // --------------------------------------------------------------------------

  private _startLoop(): void {
    this._stopLoop();
    const intervalMs = Math.round(1000 / this._tickHz);
    this._tickHandle = setInterval(() => this._tick_step(), intervalMs);
    if (this._ctx) this._scheduleRaf();
  }

  private _stopLoop(): void {
    if (this._tickHandle !== null) {
      clearInterval(this._tickHandle);
      this._tickHandle = null;
    }
    if (this._rafHandle !== null) {
      cancelAnimationFrame(this._rafHandle);
      this._rafHandle = null;
    }
  }

  private _tick_step(): void {
    if (!this._conn.isConnected) return;
    this._tick++;

    // Capture input snapshot
    const input = this._input
      ? this._input.snapshot(this._seq + 1, Date.now())
      : _emptyInput(this._seq + 1);

    this.sendInput(input);
  }

  private _scheduleRaf(): void {
    this._rafHandle = requestAnimationFrame(() => {
      this._renderFrame();
      this._rafHandle = null;
      if (this._ctx) this._scheduleRaf();
    });
  }

  private _renderFrame(): void {
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

  private _emitState(state: ArenaView | null): void {
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
 */
function _viewToSnapshot(view: ArenaView | null): ArenaSnapshot {
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
 */
function _emptyInput(seq: number): Input {
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

export { PredictionBuffer, arenaApplyInput } from './prediction';
export { renderArenaView } from './renderer';
export { InputCapture } from './input-capture';
export { ConnectionManager } from './connection.js';
export { applyDeltaToSnapshot, snapshotToView } from './delta';
export { encodeInputFrame, parseServerMessage, decodeBytes,
  defaultInput, defaultKeyState, defaultMouseState } from './protocol';
export { ReplayPlayer } from './replay-player.js';
