/**
 * magnetite-web-client/src/prediction.js
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

// ---------------------------------------------------------------------------
// PredictionBuffer
// ---------------------------------------------------------------------------

/**
 * @typedef {Object} PendingFrame
 * @property {number} seq   - client-local sequence number
 * @property {number} tick  - authoritative tick targeted
 * @property {import('./types.js').Input} input
 */

/**
 * @callback ApplyInputFn
 * @param {unknown} state   - current predicted state (mutable copy)
 * @param {import('./types.js').Input} input
 * @param {number} tick
 * @returns {unknown}       - new predicted state (may be same reference or new)
 */

export class PredictionBuffer {
  /**
   * @param {ApplyInputFn} applyInput
   *   Pure function: (state, input, tick) → newState.
   *   Must not mutate the original state — return a new object.
   * @param {number} [maxBufferSize=128]
   *   Hard cap on buffered unacked frames (oldest are dropped when exceeded).
   */
  constructor(applyInput, maxBufferSize = 128) {
    /** @type {ApplyInputFn} */
    this._applyInput = applyInput;

    /** @type {PendingFrame[]} sorted by seq ascending */
    this._pending = [];

    /** Maximum number of unacked frames to retain */
    this._maxBufferSize = maxBufferSize;

    /**
     * Last authoritative snapshot received from the server.
     * Reset to this on reconciliation.
     * @type {unknown}
     */
    this._authoritativeState = null;

    /**
     * The tick of the last authoritative snapshot applied.
     * @type {number}
     */
    this._authoritativeTick = 0;

    /**
     * Current predicted state (ahead of server, includes locally applied
     * frames not yet acked).
     * @type {unknown}
     */
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
   * @param {unknown} state   - parsed ArenaSnapshot (or game-specific snapshot)
   * @param {number} tick     - the tick this snapshot corresponds to
   */
  applySnapshot(state, tick) {
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
   * @param {number} seq  - sequence number acknowledged by the server
   * @param {number} tick - authoritative tick produced by that input
   */
  ack(seq, tick) {
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
   * @param {number} seq - rejected sequence number
   */
  reject(seq) {
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
   * @param {number} seq
   * @param {number} tick
   * @param {import('./types.js').Input} input
   * @returns {unknown} the new predicted state after applying this input
   */
  predict(seq, tick, input) {
    // Drop oldest if buffer is full
    if (this._pending.length >= this._maxBufferSize) {
      this._pending.shift();
    }

    this._pending.push({ seq, tick, input });
    this._pending.sort((a, b) => a.seq - b.seq);

    const base = this._predictedState ?? this._authoritativeState;
    this._predictedState = this._applyInput(base, input, tick);
    return this._predictedState;
  }

  /**
   * @returns {unknown} the current predicted state
   */
  get state() {
    return this._predictedState;
  }

  /**
   * @returns {unknown} the last confirmed authoritative state
   */
  get authoritativeState() {
    return this._authoritativeState;
  }

  /**
   * @returns {number} number of unacked frames in the buffer
   */
  get pendingCount() {
    return this._pending.length;
  }

  // --------------------------------------------------------------------------
  // Internal helpers
  // --------------------------------------------------------------------------

  /**
   * Replay all pending (unacked) frames on top of `base`.
   *
   * @param {unknown} base
   * @returns {unknown}
   */
  _replayPending(base) {
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
 * Constants match game-template-authoritative/src/types.rs:
 *   MAX_SPEED = 4.0, ARENA_WIDTH = 200.0, ARENA_HEIGHT = 200.0
 */
const ARENA_MAX_SPEED = 4.0;
const ARENA_HALF_W = 100.0;
const ARENA_HALF_H = 100.0;

/**
 * Apply one input frame to an ArenaView state, predicting local player movement.
 *
 * Returns a new state object (shallow clone with mutated self_state).
 *
 * @param {import('./types.js').ArenaView | null} state
 * @param {import('./types.js').Input} input
 * @param {number} tick
 * @returns {import('./types.js').ArenaView | null}
 */
export function arenaApplyInput(state, input, tick) {
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
