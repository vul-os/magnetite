/**
 * magnetite-web-client/src/replay-player.js
 *
 * ReplayPlayer — pure local playback of a ReplayLog JSON.
 *
 * No live server / WebSocket — driven entirely from the recorded frames.
 *
 * ReplayLog JSON shape (mirrors authority.rs::ReplayLog):
 *   {
 *     config: {
 *       tick_hz: number,
 *       max_players: number,
 *       seed: number,
 *       topology: object,
 *       snapshot_every: number,
 *     },
 *     // frames[i] = [tick, [[playerId, Input], ...]]
 *     frames: Array<[number, Array<[string, Input]>]>,
 *     // state_hashes[i] = [tick, hash]
 *     state_hashes: Array<[number, number]>,
 *   }
 *
 * The player re-renders the match tick-by-tick by applying recorded inputs
 * through the same arena logic the live client uses (arenaApplyInput).
 * For each tick, the view is reconstructed as an ArenaView where every
 * player in that frame is treated as a "player" entity.
 *
 * Public API:
 *   new ReplayPlayer(replayLog, options?)
 *   player.play()
 *   player.pause()
 *   player.seek(tick)
 *   player.setSpeed(x)
 *   player.onFrame(tick, view)       — set callback
 *   player.totalTicks                — number
 *   player.currentTick               — number
 *   player.isPlaying                 — boolean
 *   player.dispose()
 */

import { arenaApplyInput } from './prediction.js';
import { renderArenaView } from './renderer.js';

// ---------------------------------------------------------------------------
// ReplayPlayer
// ---------------------------------------------------------------------------

/**
 * @typedef {Object} ReplayLog
 * @property {object} config
 * @property {number} config.tick_hz
 * @property {number} config.max_players
 * @property {number} config.seed
 * @property {Array<[number, Array<[string, import('./types.js').Input]>]>} frames
 * @property {Array<[number, number]>} state_hashes
 */

/**
 * @typedef {Object} ReplayPlayerOptions
 * @property {CanvasRenderingContext2D | null} [ctx]    - Canvas context for rendering
 * @property {Function | null} [render]                 - Custom render fn (ctx, view, playerId) => void
 * @property {(tick: number, view: import('./types.js').ArenaView) => void} [onFrame] - Frame callback
 * @property {number} [speed]                            - Playback speed multiplier (default 1.0)
 */

export class ReplayPlayer {
  /**
   * @param {ReplayLog} replayLog  - Parsed ReplayLog JSON
   * @param {ReplayPlayerOptions} [opts]
   */
  constructor(replayLog, opts = {}) {
    if (!replayLog || !Array.isArray(replayLog.frames)) {
      throw new Error('[ReplayPlayer] replayLog must have a frames array');
    }

    /** @type {ReplayLog} */
    this._log = replayLog;

    /** @type {number} playback speed multiplier */
    this._speed = typeof opts.speed === 'number' && opts.speed > 0 ? opts.speed : 1.0;

    /** @type {(tick: number, view: import('./types.js').ArenaView) => void | null} */
    this._onFrameCb = typeof opts.onFrame === 'function' ? opts.onFrame : null;

    /** @type {CanvasRenderingContext2D | null} */
    this._ctx = opts.ctx || null;

    /** @type {Function} */
    this._renderFn = typeof opts.render === 'function' ? opts.render : renderArenaView;

    /** @type {boolean} */
    this._playing = false;

    /** @type {number} current playback tick index (index into _tickList) */
    this._tickIndex = 0;

    /** @type {number | null} setInterval/setTimeout handle */
    this._playHandle = null;

    // Build a sorted list of unique ticks from the frames
    /** @type {number[]} sorted ascending */
    this._tickList = _buildTickList(replayLog.frames);

    // Build a per-player state map indexed by tick for O(1) seeks:
    // We eagerly simulate the full replay once and cache every tick's ArenaView.
    /** @type {Map<number, import('./types.js').ArenaView>} */
    this._viewCache = _buildViewCache(replayLog.frames, this._tickList);
  }

  // --------------------------------------------------------------------------
  // Public API
  // --------------------------------------------------------------------------

  /**
   * Total number of distinct ticks in the replay.
   * @type {number}
   */
  get totalTicks() {
    return this._tickList.length;
  }

  /**
   * The tick value at the current playback position.
   * @type {number}
   */
  get currentTick() {
    return this._tickList[this._tickIndex] ?? 0;
  }

  /**
   * Whether the player is currently playing.
   * @type {boolean}
   */
  get isPlaying() {
    return this._playing;
  }

  /**
   * Set or replace the onFrame callback.
   * Called each time the player advances to a new tick.
   *
   * @param {(tick: number, view: import('./types.js').ArenaView) => void} fn
   */
  set onFrame(fn) {
    this._onFrameCb = typeof fn === 'function' ? fn : null;
  }

  /**
   * Start or resume playback from the current tick.
   */
  play() {
    if (this._playing) return;
    if (this._tickIndex >= this._tickList.length - 1) {
      // At end — restart from beginning
      this._tickIndex = 0;
    }
    this._playing = true;
    this._scheduleNext();
  }

  /**
   * Pause playback. The current tick position is retained.
   */
  pause() {
    this._playing = false;
    this._clearHandle();
  }

  /**
   * Seek to a specific tick value.
   * If the exact tick is not in the log, seeks to the nearest preceding tick.
   *
   * @param {number} tick
   */
  seek(tick) {
    const idx = _findTickIndex(this._tickList, tick);
    this._tickIndex = idx;
    this._emitCurrentFrame();
  }

  /**
   * Set the playback speed multiplier.
   * 1.0 = real-time, 2.0 = 2x, 0.5 = half speed.
   *
   * @param {number} x - must be > 0
   */
  setSpeed(x) {
    if (typeof x !== 'number' || x <= 0) {
      throw new Error('[ReplayPlayer] speed must be a positive number');
    }
    this._speed = x;
    // Reschedule the next step at the new rate if playing
    if (this._playing) {
      this._clearHandle();
      this._scheduleNext();
    }
  }

  /**
   * Stop playback and release resources.
   */
  dispose() {
    this.pause();
    this._onFrameCb = null;
    this._ctx = null;
  }

  // --------------------------------------------------------------------------
  // Internal
  // --------------------------------------------------------------------------

  _scheduleNext() {
    if (!this._playing) return;
    if (this._tickIndex >= this._tickList.length - 1) {
      // Reached end of replay
      this._playing = false;
      return;
    }

    const tickHz = (this._log.config && this._log.config.tick_hz) ? this._log.config.tick_hz : 60;
    const msPerTick = (1000 / tickHz) / this._speed;

    this._playHandle = setTimeout(() => {
      if (!this._playing) return;
      this._tickIndex++;
      this._emitCurrentFrame();
      this._scheduleNext();
    }, msPerTick);
  }

  _clearHandle() {
    if (this._playHandle !== null) {
      clearTimeout(this._playHandle);
      this._playHandle = null;
    }
  }

  _emitCurrentFrame() {
    const tick = this._tickList[this._tickIndex];
    const view = this._viewCache.get(tick);
    if (!view) return;

    // Render to canvas if context provided
    if (this._ctx) {
      try {
        this._renderFn(this._ctx, view, null);
      } catch (e) {
        console.error('[ReplayPlayer] render error:', e);
      }
    }

    // Fire onFrame callback
    if (this._onFrameCb) {
      try {
        this._onFrameCb(tick, view);
      } catch (e) {
        console.error('[ReplayPlayer] onFrame callback error:', e);
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Build a sorted list of unique tick values from the frames array.
 *
 * @param {Array<[number, Array<[string, import('./types.js').Input]>]>} frames
 * @returns {number[]}
 */
function _buildTickList(frames) {
  const ticks = frames.map(f => Number(f[0]));
  const unique = Array.from(new Set(ticks));
  unique.sort((a, b) => a - b);
  return unique;
}

/**
 * Simulate the entire replay and cache the ArenaView for each tick.
 *
 * Strategy: treat each distinct player as a separate entity whose state is
 * evolved tick-by-tick using arenaApplyInput (the same local predictor used
 * by the live client). This is deterministic: same inputs → same view.
 *
 * Each tick's view is stored as:
 *   { self_state: null, other_players: ShooterPlayer[], projectiles: [], tick }
 *
 * (No "self" in replay — the UI may pick one for highlight via the scrubber.)
 *
 * @param {Array<[number, Array<[string, import('./types.js').Input]>]>} frames
 * @param {number[]} tickList
 * @returns {Map<number, import('./types.js').ArenaView>}
 */
function _buildViewCache(frames, tickList) {
  // Indexed frames by tick for fast lookup
  /** @type {Map<number, Array<[string, import('./types.js').Input]>>} */
  const framesByTick = new Map();
  for (const [tick, inputs] of frames) {
    framesByTick.set(Number(tick), inputs);
  }

  /**
   * Running per-player state, keyed by player id.
   * Each entry is an ArenaView where self_state holds that player's data.
   * @type {Map<string, import('./types.js').ArenaView>}
   */
  const playerViews = new Map();

  /** @type {Map<number, import('./types.js').ArenaView>} */
  const cache = new Map();

  for (const tick of tickList) {
    const inputs = framesByTick.get(tick) || [];

    // Apply each player's input to their individual state
    for (const [playerId, input] of inputs) {
      const pid = String(playerId);
      let pv = playerViews.get(pid);

      if (!pv) {
        // First appearance of this player — initialise a default ArenaView
        pv = _defaultPlayerView(pid, tick);
        playerViews.set(pid, pv);
      }

      // Advance this player's state via the arena logic
      const updated = arenaApplyInput(pv, input, tick);
      playerViews.set(pid, updated || pv);
    }

    // Collect all players as other_players (no designated self in replay)
    const allPlayers = [];
    for (const [, pv] of playerViews) {
      if (pv.self_state) {
        allPlayers.push({ ...pv.self_state });
      }
    }

    cache.set(tick, {
      self_state: null,
      other_players: allPlayers,
      projectiles: [],
      tick,
    });
  }

  return cache;
}

/**
 * Create a default per-player ArenaView for a newly seen player.
 *
 * @param {string} playerId
 * @param {number} tick
 * @returns {import('./types.js').ArenaView}
 */
function _defaultPlayerView(playerId, tick) {
  return {
    self_state: {
      id: playerId,
      x: 0,
      y: 0,
      angle: 0,
      hp: 100,
      alive: true,
      last_shot_tick: 0,
      score: 0,
    },
    other_players: [],
    projectiles: [],
    tick,
  };
}

/**
 * Binary search for the index of the nearest tick at or before `target`.
 * Returns 0 if target is before all ticks.
 *
 * @param {number[]} tickList - sorted ascending
 * @param {number} target
 * @returns {number} index into tickList
 */
function _findTickIndex(tickList, target) {
  if (tickList.length === 0) return 0;
  if (target <= tickList[0]) return 0;
  if (target >= tickList[tickList.length - 1]) return tickList.length - 1;

  let lo = 0;
  let hi = tickList.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (tickList[mid] <= target) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }
  return lo;
}
