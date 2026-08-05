/**
 * magnetite-web-client/src/prediction.ts
 *
 * Client-side prediction + reconciliation.
 *
 * Mirrors the Rust PredictionBuffer logic in magnetite-sdk:
 *  - Keep a ring-buffer of unacked InputFrames.
 *  - Apply each input locally to produce a predicted state.
 *  - On Ack: discard frames with seq ≤ acked seq.
 *  - On Snapshot: reset to the authoritative state, then replay
 *    all buffered (unacked) inputs from newest snapshot forward.
 *
 * The game-specific "apply input" function is injected so this module
 * stays game-agnostic.
 */

import type { ArenaView, Input } from './types';

// ---------------------------------------------------------------------------
// PredictionBuffer
// ---------------------------------------------------------------------------

interface PendingFrame {
  /** client-local sequence number */
  seq: number;
  /** authoritative tick targeted */
  tick: number;
  input: Input;
}

/**
 * Pure function: (state, input, tick) → newState.
 * Must not mutate the original state — return a new object.
 */
export type ApplyInputFn<S> = (state: S, input: Input, tick: number) => S;

export class PredictionBuffer<S = unknown> {
  private _applyInput: ApplyInputFn<S>;
  /** sorted by seq ascending */
  private _pending: PendingFrame[];
  /** Maximum number of unacked frames to retain */
  private _maxBufferSize: number;
  /**
   * Last authoritative snapshot received from the server.
   * Reset to this on reconciliation.
   */
  private _authoritativeState: S | null;
  /** The tick of the last authoritative snapshot applied. */
  private _authoritativeTick: number;
  /**
   * Current predicted state (ahead of server, includes locally applied
   * frames not yet acked).
   */
  private _predictedState: S | null;

  /**
   * @param applyInput
   *   Pure function: (state, input, tick) → newState.
   *   Must not mutate the original state — return a new object.
   * @param maxBufferSize
   *   Hard cap on buffered unacked frames (oldest are dropped when exceeded).
   */
  constructor(applyInput: ApplyInputFn<S>, maxBufferSize = 128) {
    this._applyInput = applyInput;
    this._pending = [];
    this._maxBufferSize = maxBufferSize;
    this._authoritativeState = null;
    this._authoritativeTick = 0;
    this._predictedState = null;
  }

  // --------------------------------------------------------------------------
  // Public API
  // --------------------------------------------------------------------------

  /**
   * Apply a fresh authoritative snapshot from the server and reconcile.
   *
   * Called on ServerNet::Snapshot or after a Welcome.
   *
   * @param state   - parsed ArenaSnapshot (or game-specific snapshot)
   * @param tick     - the tick this snapshot corresponds to
   */
  applySnapshot(state: S, tick: number): void {
    this._authoritativeState = state;
    this._authoritativeTick = tick;

    // Discard all pending frames at or before this tick
    this._pending = this._pending.filter(f => f.tick > tick);

    // Replay remaining pending frames on top of authoritative state
    this._predictedState = this._replayPending(state);
  }

  /**
   * Acknowledge frames up to `seq` (inclusive).
   *
   * Called on ServerNet::Ack.
   *
   * @param seq  - sequence number acknowledged by the server
   * @param tick - authoritative tick produced by that input
   */
  ack(seq: number, tick: number): void {
    // Discard frames with seq ≤ acknowledged seq
    this._pending = this._pending.filter(f => f.seq > seq);

    // Advance authoritative tick to at least this ack tick
    if (tick > this._authoritativeTick) {
      this._authoritativeTick = tick;
    }

    // Do NOT re-simulate here — we keep the predicted state as-is
    // (it was already the result of applying those frames). The next
    // Snapshot will fully reconcile if divergence occurred.
  }

  /**
   * Record a rejected frame and force reconciliation from last snapshot.
   *
   * Called on ServerNet::Reject.
   *
   * @param seq - rejected sequence number
   */
  reject(seq: number): void {
    // Remove the rejected frame
    this._pending = this._pending.filter(f => f.seq !== seq);

    // Re-derive predicted state from authority
    if (this._authoritativeState !== null) {
      this._predictedState = this._replayPending(this._authoritativeState);
    }
  }

  /**
   * Predict the effect of a new input locally and buffer it for reconciliation.
   *
   * Called each time the client sends an InputFrame to the server.
   *
   * @returns the new predicted state after applying this input
   */
  predict(seq: number, tick: number, input: Input): S {
    // Drop oldest if buffer is full
    if (this._pending.length >= this._maxBufferSize) {
      this._pending.shift();
    }

    this._pending.push({ seq, tick, input });
    this._pending.sort((a, b) => a.seq - b.seq);

    const base = (this._predictedState ?? this._authoritativeState) as S;
    this._predictedState = this._applyInput(base, input, tick);
    return this._predictedState;
  }

  /** the current predicted state */
  get state(): S | null {
    return this._predictedState;
  }

  /** the last confirmed authoritative state */
  get authoritativeState(): S | null {
    return this._authoritativeState;
  }

  /** number of unacked frames in the buffer */
  get pendingCount(): number {
    return this._pending.length;
  }

  // --------------------------------------------------------------------------
  // Internal helpers
  // --------------------------------------------------------------------------

  /**
   * Replay all pending (unacked) frames on top of `base`.
   */
  private _replayPending(base: S): S {
    let state = base;
    for (const frame of this._pending) {
      state = this._applyInput(state, frame.input, frame.tick);
    }
    return state;
  }
}

// ---------------------------------------------------------------------------
// Arena-specific prediction helper (applies ArenaView/Snapshot locally)
// ---------------------------------------------------------------------------

/**
 * A simple local predictor for the arena-shooter game type.
 *
 * This mirrors the server-side ArenaShooter::validate + step for the *local*
 * player only (we can't predict other players). Used as the default
 * `applyInput` function when createClient is used with the arena-shooter game.
 *
 * Constants match game-templates/authoritative/src/types.rs:
 *   MAX_SPEED = 4.0, ARENA_WIDTH = 200.0, ARENA_HEIGHT = 200.0
 */
const ARENA_MAX_SPEED = 4.0;
const ARENA_HALF_W = 100.0;
const ARENA_HALF_H = 100.0;

/**
 * Apply one input frame to an ArenaView state, predicting local player movement.
 *
 * Returns a new state object (shallow clone with mutated self_state).
 */
export function arenaApplyInput(state: ArenaView | null, input: Input, tick: number): ArenaView | null {
  if (!state || !state.self_state) return state;

  const self = { ...state.self_state };

  if (!self.alive) {
    return { ...state, self_state: self, tick };
  }

  // Movement: W/A/S/D → dx/dy (mirrors ArenaShooter::validate)
  const rawDx =
    (input.keys.right ? 1.0 : 0.0) - (input.keys.left ? 1.0 : 0.0);
  const rawDy =
    (input.keys.forward ? 1.0 : 0.0) - (input.keys.backward ? 1.0 : 0.0);

  if (rawDx !== 0.0 || rawDy !== 0.0) {
    const mag = Math.sqrt(rawDx * rawDx + rawDy * rawDy);
    const dx = (rawDx / mag) * ARENA_MAX_SPEED;
    const dy = (rawDy / mag) * ARENA_MAX_SPEED;
    self.x = Math.max(-ARENA_HALF_W, Math.min(ARENA_HALF_W, self.x + dx));
    self.y = Math.max(-ARENA_HALF_H, Math.min(ARENA_HALF_H, self.y + dy));
  }

  // Aim: derive angle from mouse delta (mirrors server validate)
  if (Math.abs(input.mouse.delta_x) > 0.001 || Math.abs(input.mouse.delta_y) > 0.001) {
    self.angle = Math.atan2(input.mouse.delta_y, input.mouse.delta_x);
  }

  return { ...state, self_state: self, tick };
}
