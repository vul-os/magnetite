/**
 * magnetite-web-client/src/replay-player.test.js
 *
 * Unit tests for ReplayPlayer.
 *
 * Coverage:
 *  - Construction from a ReplayLog JSON
 *  - totalTicks / currentTick reporting
 *  - seek(tick) → deterministic view
 *  - seek to non-existent tick → nearest preceding tick
 *  - setSpeed() updates speed
 *  - play() / pause() lifecycle
 *  - onFrame callback fires on seek
 *  - Multiple seeks to the same tick yield identical views (determinism)
 *  - Empty replay log handled gracefully
 *  - dispose() releases resources
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { ReplayPlayer } from './replay-player.js';
import { defaultKeyState, defaultMouseState } from './protocol.js';

// ---------------------------------------------------------------------------
// Helpers — build sample ReplayLog fixtures
// ---------------------------------------------------------------------------

/**
 * Build a minimal Input for a player, optionally overriding keys/mouse.
 */
function makeInput(keysOverride = {}, mouseOverride = {}, seq = 0) {
  return {
    keys: { ...defaultKeyState(), ...keysOverride },
    mouse: { ...defaultMouseState(), ...mouseOverride },
    sequence: seq,
    timestamp_ms: 0,
  };
}

/**
 * Build a ReplayLog with two players and N ticks.
 * Player "p1" moves right every tick. Player "p2" is idle.
 *
 * @param {number} numTicks
 * @param {number} tickHz
 * @returns {import('./replay-player.js').ReplayLog}
 */
function makeReplayLog(numTicks = 10, tickHz = 60) {
  const frames = [];
  for (let t = 1; t <= numTicks; t++) {
    frames.push([
      t,
      [
        ['p1', makeInput({ right: true }, {}, t)],
        ['p2', makeInput({}, {}, t)],
      ],
    ]);
  }
  return {
    config: {
      tick_hz: tickHz,
      max_players: 4,
      seed: 42,
      snapshot_every: 300,
      topology: 'SingleRoom',
    },
    frames,
    state_hashes: frames.map(([t]) => [t, BigInt(t)]),
  };
}

/** Single-player, single-tick log */
function makeSingleTickLog() {
  return {
    config: { tick_hz: 60, max_players: 2, seed: 1, snapshot_every: 300, topology: 'SingleRoom' },
    frames: [
      [5, [['alice', makeInput({ forward: true })]]],
    ],
    state_hashes: [[5, 0n]],
  };
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

describe('ReplayPlayer construction', () => {
  it('constructs from a valid ReplayLog', () => {
    const log = makeReplayLog(5);
    const player = new ReplayPlayer(log);
    expect(player).toBeDefined();
    expect(player.totalTicks).toBe(5);
    expect(player.currentTick).toBe(1); // first tick in log
    expect(player.isPlaying).toBe(false);
    player.dispose();
  });

  it('throws on missing frames array', () => {
    expect(() => new ReplayPlayer({})).toThrow('[ReplayPlayer]');
    expect(() => new ReplayPlayer(null)).toThrow('[ReplayPlayer]');
  });

  it('handles an empty frames array', () => {
    const log = { config: { tick_hz: 60 }, frames: [], state_hashes: [] };
    const player = new ReplayPlayer(log);
    expect(player.totalTicks).toBe(0);
    expect(player.currentTick).toBe(0);
    player.dispose();
  });

  it('accepts custom speed option', () => {
    const log = makeReplayLog(3);
    const player = new ReplayPlayer(log, { speed: 2.0 });
    expect(player._speed).toBe(2.0);
    player.dispose();
  });
});

// ---------------------------------------------------------------------------
// totalTicks and currentTick
// ---------------------------------------------------------------------------

describe('ReplayPlayer.totalTicks / currentTick', () => {
  it('reports correct totalTicks for a 10-tick log', () => {
    const player = new ReplayPlayer(makeReplayLog(10));
    expect(player.totalTicks).toBe(10);
    player.dispose();
  });

  it('currentTick starts at the first recorded tick', () => {
    const log = makeSingleTickLog(); // only tick 5
    const player = new ReplayPlayer(log);
    expect(player.currentTick).toBe(5);
    player.dispose();
  });

  it('currentTick updates after seek', () => {
    const player = new ReplayPlayer(makeReplayLog(10));
    player.seek(7);
    expect(player.currentTick).toBe(7);
    player.dispose();
  });
});

// ---------------------------------------------------------------------------
// seek() — deterministic view reconstruction
// ---------------------------------------------------------------------------

describe('ReplayPlayer.seek()', () => {
  it('seeking to tick N yields a deterministic view', () => {
    const log = makeReplayLog(10);
    const player = new ReplayPlayer(log);

    const frames = [];
    player.onFrame = (tick, view) => frames.push({ tick, view });

    // Seek twice to the same tick — must produce identical views
    player.seek(5);
    player.seek(5);

    expect(frames).toHaveLength(2);
    expect(frames[0].tick).toBe(5);
    expect(frames[1].tick).toBe(5);

    // Views must be deeply equal (same positions, same players)
    expect(frames[0].view).toEqual(frames[1].view);
    player.dispose();
  });

  it('seek to a non-existent tick lands on the nearest preceding tick', () => {
    // Log has ticks 1..5 only
    const log = makeReplayLog(5);
    const player = new ReplayPlayer(log);

    const frames = [];
    player.onFrame = (tick, view) => frames.push({ tick, view });

    // Tick 100 doesn't exist — should go to tick 5 (last)
    player.seek(100);
    expect(frames[0].tick).toBe(5);

    frames.length = 0;

    // Tick 0 doesn't exist — should go to tick 1 (first)
    player.seek(0);
    expect(frames[0].tick).toBe(1);

    player.dispose();
  });

  it('seeking to tick 1 yields position 0+4=4 for a right-moving player', () => {
    // p1 moves right once (at tick 1) so x should be 4.0 (MAX_SPEED=4)
    const log = makeReplayLog(1);
    const player = new ReplayPlayer(log);

    let capturedView = null;
    player.onFrame = (_tick, view) => { capturedView = view; };
    player.seek(1);

    expect(capturedView).not.toBeNull();
    const p1 = capturedView.other_players.find(p => String(p.id) === 'p1');
    expect(p1).toBeDefined();
    // After 1 right-key step from x=0, x = 4.0
    expect(p1.x).toBeCloseTo(4.0, 5);

    player.dispose();
  });

  it('p1 position at tick N is cumulative (deterministic accumulation)', () => {
    // After N right-key steps from x=0, x = min(N * 4, 100)
    const n = 5;
    const log = makeReplayLog(n);
    const player = new ReplayPlayer(log);

    let capturedView = null;
    player.onFrame = (_tick, view) => { capturedView = view; };
    player.seek(n);

    const p1 = capturedView.other_players.find(p => String(p.id) === 'p1');
    // 5 * 4.0 = 20, well within arena bounds
    expect(p1.x).toBeCloseTo(n * 4.0, 4);

    player.dispose();
  });

  it('view at tick N has tick property set to N', () => {
    const log = makeReplayLog(10);
    const player = new ReplayPlayer(log);

    let lastTick = null;
    let lastView = null;
    player.onFrame = (t, v) => { lastTick = t; lastView = v; };

    player.seek(7);

    expect(lastTick).toBe(7);
    expect(lastView.tick).toBe(7);

    player.dispose();
  });

  it('both p1 and p2 appear in other_players', () => {
    const log = makeReplayLog(3);
    const player = new ReplayPlayer(log);

    let view = null;
    player.onFrame = (_t, v) => { view = v; };
    player.seek(3);

    expect(view.other_players).toHaveLength(2);
    const ids = view.other_players.map(p => String(p.id)).sort();
    expect(ids).toEqual(['p1', 'p2']);

    player.dispose();
  });

  it('self_state is null in replay views (no single designated self)', () => {
    const log = makeReplayLog(5);
    const player = new ReplayPlayer(log);

    let view = null;
    player.onFrame = (_t, v) => { view = v; };
    player.seek(3);

    expect(view.self_state).toBeNull();
    player.dispose();
  });
});

// ---------------------------------------------------------------------------
// setSpeed()
// ---------------------------------------------------------------------------

describe('ReplayPlayer.setSpeed()', () => {
  it('updates _speed', () => {
    const player = new ReplayPlayer(makeReplayLog(5));
    player.setSpeed(3.0);
    expect(player._speed).toBe(3.0);
    player.dispose();
  });

  it('throws on non-positive speed', () => {
    const player = new ReplayPlayer(makeReplayLog(5));
    expect(() => player.setSpeed(0)).toThrow('[ReplayPlayer]');
    expect(() => player.setSpeed(-1)).toThrow('[ReplayPlayer]');
    expect(() => player.setSpeed('fast')).toThrow('[ReplayPlayer]');
    player.dispose();
  });
});

// ---------------------------------------------------------------------------
// play() / pause() / isPlaying
// ---------------------------------------------------------------------------

describe('ReplayPlayer play / pause lifecycle', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('isPlaying becomes true after play()', () => {
    const player = new ReplayPlayer(makeReplayLog(10));
    expect(player.isPlaying).toBe(false);
    player.play();
    expect(player.isPlaying).toBe(true);
    player.dispose();
  });

  it('isPlaying becomes false after pause()', () => {
    const player = new ReplayPlayer(makeReplayLog(10));
    player.play();
    player.pause();
    expect(player.isPlaying).toBe(false);
    player.dispose();
  });

  it('play() is idempotent (calling twice does not double-schedule)', () => {
    const player = new ReplayPlayer(makeReplayLog(10));
    player.play();
    const handle = player._playHandle;
    player.play(); // second call — no new handle
    expect(player._playHandle).toBe(handle);
    player.dispose();
  });

  it('advances currentTick on timer tick', () => {
    const log = makeReplayLog(10, 60); // 60 Hz → ~16.67 ms per tick
    const player = new ReplayPlayer(log);
    player.seek(1);
    expect(player.currentTick).toBe(1);

    player.play();
    // Advance time by one tick period (16.67 ms → use 17)
    vi.advanceTimersByTime(17);
    expect(player.currentTick).toBe(2);

    player.dispose();
  });

  it('fires onFrame callback during play', () => {
    const log = makeReplayLog(10, 60);
    const player = new ReplayPlayer(log);
    const events = [];
    player.onFrame = (tick, view) => events.push({ tick, view });

    player.play();
    vi.advanceTimersByTime(50); // ~3 ticks at 60 Hz
    expect(events.length).toBeGreaterThanOrEqual(1);
    for (const { tick, view } of events) {
      expect(view.tick).toBe(tick);
    }

    player.dispose();
  });

  it('stops at end of replay and sets isPlaying to false', () => {
    const log = makeReplayLog(3, 60);
    const player = new ReplayPlayer(log);
    player.play();

    // Advance past all 3 ticks (3 * 17 ms)
    vi.advanceTimersByTime(3 * 17 + 50);
    expect(player.isPlaying).toBe(false);

    player.dispose();
  });

  it('play() from end restarts from beginning', () => {
    const log = makeReplayLog(3, 60);
    const player = new ReplayPlayer(log);

    // Seek to last tick
    player.seek(3);
    expect(player.currentTick).toBe(3);

    player.play();
    expect(player.currentTick).toBe(1); // rewound to start

    player.dispose();
  });
});

// ---------------------------------------------------------------------------
// onFrame setter
// ---------------------------------------------------------------------------

describe('ReplayPlayer.onFrame setter', () => {
  it('can be replaced with a new callback', () => {
    const log = makeReplayLog(5);
    const player = new ReplayPlayer(log);
    const calls = [];
    player.onFrame = (t, _v) => calls.push(t);

    player.seek(3);
    expect(calls).toEqual([3]);

    // Replace callback
    const calls2 = [];
    player.onFrame = (t, _v) => calls2.push(t);
    player.seek(4);
    expect(calls2).toEqual([4]);
    expect(calls).toHaveLength(1); // original callback not called again

    player.dispose();
  });

  it('accepts null to remove callback', () => {
    const log = makeReplayLog(5);
    const player = new ReplayPlayer(log);
    const calls = [];
    player.onFrame = (t, _v) => calls.push(t);
    player.onFrame = null;
    player.seek(3);
    expect(calls).toHaveLength(0);
    player.dispose();
  });
});

// ---------------------------------------------------------------------------
// dispose()
// ---------------------------------------------------------------------------

describe('ReplayPlayer.dispose()', () => {
  it('stops playback', () => {
    vi.useFakeTimers();
    const player = new ReplayPlayer(makeReplayLog(10));
    player.play();
    expect(player.isPlaying).toBe(true);
    player.dispose();
    expect(player.isPlaying).toBe(false);
    vi.useRealTimers();
  });

  it('clears the onFrame callback', () => {
    const player = new ReplayPlayer(makeReplayLog(5));
    player.onFrame = () => {};
    player.dispose();
    // After dispose, the callback is cleared
    expect(player._onFrameCb).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

describe('ReplayPlayer edge cases', () => {
  it('single-tick replay works correctly', () => {
    const log = makeSingleTickLog();
    const player = new ReplayPlayer(log);
    expect(player.totalTicks).toBe(1);

    let view = null;
    player.onFrame = (_t, v) => { view = v; };
    player.seek(5);

    expect(view).not.toBeNull();
    expect(view.tick).toBe(5);
    const alice = view.other_players.find(p => String(p.id) === 'alice');
    expect(alice).toBeDefined();
    // forward key → y increased by MAX_SPEED=4
    expect(alice.y).toBeCloseTo(4.0, 5);

    player.dispose();
  });

  it('duplicate ticks in frames are deduplicated in tickList', () => {
    const log = {
      config: { tick_hz: 60, max_players: 2, seed: 1, snapshot_every: 300 },
      frames: [
        [1, [['p1', makeInput({ right: true })]]],
        [1, [['p2', makeInput({ forward: true })]]],
        [2, [['p1', makeInput({ right: true })]]],
      ],
      state_hashes: [],
    };
    const player = new ReplayPlayer(log);
    // Tick 1 appears twice but should only count once in totalTicks
    expect(player.totalTicks).toBe(2);
    player.dispose();
  });

  it('seeking beyond last tick lands on last tick', () => {
    const log = makeReplayLog(5);
    const player = new ReplayPlayer(log);
    let tick = null;
    player.onFrame = (t) => { tick = t; };
    player.seek(9999);
    expect(tick).toBe(5);
    player.dispose();
  });

  it('seeking to tick 0 on a log starting at tick 1 lands on tick 1', () => {
    const log = makeReplayLog(5);
    const player = new ReplayPlayer(log);
    let tick = null;
    player.onFrame = (t) => { tick = t; };
    player.seek(0);
    expect(tick).toBe(1);
    player.dispose();
  });
});
