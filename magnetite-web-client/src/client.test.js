/**
 * magnetite-web-client/src/client.test.js
 *
 * Unit tests for the reconcile / delta-apply logic.
 * Does NOT require a live server — all tests run in Node/vitest with jsdom.
 *
 * Coverage:
 *  - PredictionBuffer: predict, ack, snapshot reconciliation, reject
 *  - arenaApplyInput: movement, clamping, aiming
 *  - applyDeltaToSnapshot: changed_players, removed/new projectiles
 *  - snapshotToView: self_state vs other_players split
 *  - encodeInputFrame / parseServerMessage: wire format
 *  - decodeBytes: base64 round-trip
 */

import { describe, it, expect } from 'vitest';

import { PredictionBuffer, arenaApplyInput } from './prediction.js';
import { applyDeltaToSnapshot, snapshotToView } from './delta.js';
import {
  encodeInputFrame,
  parseServerMessage,
  decodeBytes,
  defaultInput,
  defaultKeyState,
  defaultMouseState,
} from './protocol.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makePlayer(id, x = 0, y = 0, hp = 100, alive = true) {
  return {
    id,
    x,
    y,
    angle: 0,
    hp,
    alive,
    last_shot_tick: 0,
    score: 0,
  };
}

function makeSnapshot(players = [], projectiles = [], tick = 0) {
  return { players, projectiles, tick };
}

function makeView(selfState, others = [], projectiles = [], tick = 0) {
  return {
    self_state: selfState,
    other_players: others,
    projectiles,
    tick,
  };
}

function makeInput(keys = {}, mouse = {}) {
  return {
    keys: { ...defaultKeyState(), ...keys },
    mouse: { ...defaultMouseState(), ...mouse },
    sequence: 0,
    timestamp_ms: 0,
  };
}

// ---------------------------------------------------------------------------
// PredictionBuffer
// ---------------------------------------------------------------------------

describe('PredictionBuffer', () => {
  it('starts with null state', () => {
    const buf = new PredictionBuffer(arenaApplyInput);
    expect(buf.state).toBeNull();
    expect(buf.pendingCount).toBe(0);
  });

  it('applies snapshot and sets authoritative state', () => {
    const buf = new PredictionBuffer(arenaApplyInput);
    const p = makePlayer('1', 10, 20);
    const view = makeView(p);
    buf.applySnapshot(view, 5);

    expect(buf.authoritativeState).toEqual(view);
    expect(buf.state).toEqual(view);
    expect(buf.pendingCount).toBe(0);
  });

  it('predict: advances state and buffers frame', () => {
    const buf = new PredictionBuffer(arenaApplyInput);
    const p = makePlayer('1', 0, 0);
    const view = makeView(p);
    buf.applySnapshot(view, 1);

    const input = makeInput({ right: true });
    const predicted = buf.predict(1, 2, input);

    expect(buf.pendingCount).toBe(1);
    // Moving right (KeyD) should increase x
    expect(predicted.self_state.x).toBeGreaterThan(0);
  });

  it('ack: discards acknowledged frames', () => {
    const buf = new PredictionBuffer(arenaApplyInput);
    const p = makePlayer('1', 0, 0);
    buf.applySnapshot(makeView(p), 0);

    const input = makeInput({ forward: true });
    buf.predict(1, 1, input);
    buf.predict(2, 2, input);
    buf.predict(3, 3, input);
    expect(buf.pendingCount).toBe(3);

    buf.ack(2, 2);
    expect(buf.pendingCount).toBe(1); // only seq=3 remains
  });

  it('reconcile on snapshot: discards frames at or before snapshot tick', () => {
    const buf = new PredictionBuffer(arenaApplyInput);
    const p = makePlayer('1', 0, 0);
    buf.applySnapshot(makeView(p), 0);

    const input = makeInput({ right: true });
    buf.predict(1, 1, input);
    buf.predict(2, 2, input);
    buf.predict(3, 3, input);
    expect(buf.pendingCount).toBe(3);

    // Server sends a full snapshot at tick 2 (frames 1+2 are now confirmed)
    const authView = makeView(makePlayer('1', 8, 0)); // server-corrected position
    buf.applySnapshot(authView, 2);

    // Only seq=3 (tick=3 > 2) should remain
    expect(buf.pendingCount).toBe(1);
  });

  it('reject: removes the rejected frame and reconciles from authority', () => {
    const buf = new PredictionBuffer(arenaApplyInput);
    const p = makePlayer('1', 0, 0);
    const initialView = makeView(p);
    buf.applySnapshot(initialView, 0);

    const input = makeInput({ right: true });
    buf.predict(1, 1, input);
    buf.predict(2, 2, input);
    buf.predict(3, 3, input);

    buf.reject(2);
    expect(buf.pendingCount).toBe(2); // seq 1 and 3 remain
    // State replayed from authority + remaining frames
    expect(buf.state).not.toBeNull();
  });

  it('does not grow past maxBufferSize', () => {
    const buf = new PredictionBuffer(arenaApplyInput, 3);
    const p = makePlayer('1', 0, 0);
    buf.applySnapshot(makeView(p), 0);

    const input = makeInput();
    buf.predict(1, 1, input);
    buf.predict(2, 2, input);
    buf.predict(3, 3, input);
    buf.predict(4, 4, input);

    expect(buf.pendingCount).toBe(3);
  });
});

// ---------------------------------------------------------------------------
// arenaApplyInput — local prediction
// ---------------------------------------------------------------------------

describe('arenaApplyInput', () => {
  it('returns null state unchanged', () => {
    expect(arenaApplyInput(null, makeInput(), 1)).toBeNull();
  });

  it('returns unchanged state when self_state is missing', () => {
    const view = { self_state: null, other_players: [], projectiles: [], tick: 0 };
    const result = arenaApplyInput(view, makeInput(), 1);
    expect(result.self_state).toBeNull();
  });

  it('moves right on right key (KeyD)', () => {
    const p = makePlayer('1', 0, 0);
    const view = makeView(p);
    const input = makeInput({ right: true });
    const result = arenaApplyInput(view, input, 1);
    expect(result.self_state.x).toBeCloseTo(4.0, 5); // MAX_SPEED = 4
    expect(result.self_state.y).toBe(0);
  });

  it('moves left on left key', () => {
    const p = makePlayer('1', 0, 0);
    const view = makeView(p);
    const input = makeInput({ left: true });
    const result = arenaApplyInput(view, input, 1);
    expect(result.self_state.x).toBeCloseTo(-4.0, 5);
  });

  it('moves forward on W key (increases y)', () => {
    const p = makePlayer('1', 0, 0);
    const view = makeView(p);
    const input = makeInput({ forward: true });
    const result = arenaApplyInput(view, input, 1);
    expect(result.self_state.y).toBeCloseTo(4.0, 5);
  });

  it('diagonal movement is normalised to MAX_SPEED', () => {
    const p = makePlayer('1', 0, 0);
    const view = makeView(p);
    const input = makeInput({ right: true, forward: true });
    const result = arenaApplyInput(view, input, 1);
    const dx = result.self_state.x;
    const dy = result.self_state.y;
    const dist = Math.sqrt(dx * dx + dy * dy);
    expect(dist).toBeCloseTo(4.0, 4); // normalised to MAX_SPEED
  });

  it('clamps to arena boundary', () => {
    const p = makePlayer('1', 99, 0); // near right edge (half-width = 100)
    const view = makeView(p);
    const input = makeInput({ right: true });
    const result = arenaApplyInput(view, input, 1);
    expect(result.self_state.x).toBeLessThanOrEqual(100);
  });

  it('updates angle from mouse delta', () => {
    const p = makePlayer('1', 0, 0);
    const view = makeView(p);
    const input = makeInput({}, { delta_x: 1.0, delta_y: 0.0 });
    const result = arenaApplyInput(view, input, 1);
    expect(result.self_state.angle).toBeCloseTo(0, 5); // atan2(0, 1) = 0
  });

  it('does not move dead players', () => {
    const p = makePlayer('1', 0, 0, 0, false); // dead
    const view = makeView(p);
    const input = makeInput({ right: true });
    const result = arenaApplyInput(view, input, 1);
    expect(result.self_state.x).toBe(0);
  });

  it('advances tick on the returned view', () => {
    const p = makePlayer('1', 0, 0);
    const view = makeView(p, [], [], 5);
    const input = makeInput();
    const result = arenaApplyInput(view, input, 7);
    expect(result.tick).toBe(7);
  });
});

// ---------------------------------------------------------------------------
// applyDeltaToSnapshot
// ---------------------------------------------------------------------------

describe('applyDeltaToSnapshot', () => {
  it('returns unchanged state for empty delta', () => {
    const snap = makeSnapshot([makePlayer('1', 10, 20)], [], 5);
    const delta = { changed_players: [], removed_projectile_ids: [], new_projectiles: [] };
    const result = applyDeltaToSnapshot(snap, delta, 6);
    expect(result.players).toHaveLength(1);
    expect(result.players[0].x).toBe(10);
    expect(result.tick).toBe(6);
  });

  it('updates a changed player', () => {
    const snap = makeSnapshot([makePlayer('1', 0, 0, 100)], [], 1);
    const delta = {
      changed_players: [makePlayer('1', 50, 30, 75)],
      removed_projectile_ids: [],
      new_projectiles: [],
    };
    const result = applyDeltaToSnapshot(snap, delta, 2);
    expect(result.players[0].x).toBe(50);
    expect(result.players[0].hp).toBe(75);
  });

  it('adds a new player from delta (upsert)', () => {
    const snap = makeSnapshot([makePlayer('1', 0, 0)], [], 1);
    const delta = {
      changed_players: [makePlayer('2', 10, 10)],
      removed_projectile_ids: [],
      new_projectiles: [],
    };
    const result = applyDeltaToSnapshot(snap, delta, 2);
    expect(result.players).toHaveLength(2);
  });

  it('adds new projectiles', () => {
    const snap = makeSnapshot([], [], 1);
    const proj = { id: 42n, owner: '1', x: 5, y: 5, vx: 1, vy: 0, ticks_left: 40 };
    const delta = {
      changed_players: [],
      removed_projectile_ids: [],
      new_projectiles: [proj],
    };
    const result = applyDeltaToSnapshot(snap, delta, 2);
    expect(result.projectiles).toHaveLength(1);
  });

  it('removes expired projectiles', () => {
    const proj = { id: 99, owner: '1', x: 0, y: 0, vx: 1, vy: 0, ticks_left: 1 };
    const snap = makeSnapshot([], [proj], 1);
    const delta = {
      changed_players: [],
      removed_projectile_ids: [99],
      new_projectiles: [],
    };
    const result = applyDeltaToSnapshot(snap, delta, 2);
    expect(result.projectiles).toHaveLength(0);
  });

  it('handles null snapshot gracefully', () => {
    const delta = { changed_players: [], removed_projectile_ids: [], new_projectiles: [] };
    const result = applyDeltaToSnapshot(null, delta, 1);
    expect(result).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// snapshotToView
// ---------------------------------------------------------------------------

describe('snapshotToView', () => {
  it('splits self and others correctly', () => {
    const p1 = makePlayer('1', 10, 0);
    const p2 = makePlayer('2', 20, 0);
    const snap = makeSnapshot([p1, p2], [], 3);

    const view = snapshotToView(snap, '1');
    expect(view.self_state).not.toBeNull();
    expect(String(view.self_state.id)).toBe('1');
    expect(view.other_players).toHaveLength(1);
    expect(String(view.other_players[0].id)).toBe('2');
    expect(view.tick).toBe(3);
  });

  it('self_state is null when player not found', () => {
    const snap = makeSnapshot([makePlayer('2')], [], 1);
    const view = snapshotToView(snap, '99');
    expect(view.self_state).toBeNull();
    expect(view.other_players).toHaveLength(1);
  });

  it('returns all players as other_players when playerId is null', () => {
    const snap = makeSnapshot([makePlayer('1'), makePlayer('2')], [], 1);
    const view = snapshotToView(snap, null);
    expect(view.self_state).toBeNull();
    expect(view.other_players).toHaveLength(2);
  });

  it('handles empty snapshot', () => {
    const snap = makeSnapshot([], [], 0);
    const view = snapshotToView(snap, '1');
    expect(view.self_state).toBeNull();
    expect(view.other_players).toHaveLength(0);
    expect(view.projectiles).toHaveLength(0);
  });

  it('handles null snapshot', () => {
    const view = snapshotToView(null, '1');
    expect(view.self_state).toBeNull();
    expect(view.other_players).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// Protocol: encodeInputFrame / parseServerMessage
// ---------------------------------------------------------------------------

describe('encodeInputFrame', () => {
  it('produces valid JSON with correct type tag', () => {
    const input = defaultInput();
    const raw = encodeInputFrame(1, 42, input);
    const parsed = JSON.parse(raw);
    expect(parsed.type).toBe('input_frame');
    expect(parsed.seq).toBe(1);
    expect(parsed.tick).toBe(42);
    expect(parsed.input).toBeDefined();
  });

  it('includes all KeyState and MouseState fields', () => {
    const input = makeInput({ forward: true, attack: true }, { delta_x: 1.5 });
    const raw = encodeInputFrame(5, 10, input);
    const parsed = JSON.parse(raw);
    expect(parsed.input.keys.forward).toBe(true);
    expect(parsed.input.keys.attack).toBe(true);
    expect(parsed.input.mouse.delta_x).toBe(1.5);
  });
});

describe('parseServerMessage', () => {
  it('parses a welcome message', () => {
    const raw = JSON.stringify({ type: 'welcome', player_id: '1', config: {} });
    const msg = parseServerMessage(raw);
    expect(msg).not.toBeNull();
    expect(msg.type).toBe('welcome');
    expect(msg.player_id).toBe('1');
  });

  it('parses an ack message', () => {
    const raw = JSON.stringify({ type: 'ack', seq: 7, tick: 100 });
    const msg = parseServerMessage(raw);
    expect(msg.type).toBe('ack');
    expect(msg.seq).toBe(7);
  });

  it('parses a reject message', () => {
    const raw = JSON.stringify({ type: 'reject', seq: 3, reason: 'RateLimited' });
    const msg = parseServerMessage(raw);
    expect(msg.type).toBe('reject');
    expect(msg.reason).toBe('RateLimited');
  });

  it('returns null for invalid JSON', () => {
    const msg = parseServerMessage('not json {');
    expect(msg).toBeNull();
  });

  it('returns null for JSON without type', () => {
    const msg = parseServerMessage(JSON.stringify({ foo: 'bar' }));
    expect(msg).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// decodeBytes: base64 → JSON object
// ---------------------------------------------------------------------------

describe('decodeBytes', () => {
  it('decodes base64-encoded JSON', () => {
    const obj = { players: [makePlayer('1', 5, 10)], projectiles: [], tick: 3 };
    const json = JSON.stringify(obj);
    // Encode to base64 (browser-style)
    const b64 = btoa(json);
    const decoded = decodeBytes(b64);
    expect(decoded).not.toBeNull();
    expect(decoded.tick).toBe(3);
    expect(decoded.players).toHaveLength(1);
  });

  it('returns null for empty/null input', () => {
    expect(decodeBytes(null)).toBeNull();
    expect(decodeBytes('')).toBeNull();
    expect(decodeBytes(undefined)).toBeNull();
  });

  it('decodes a plain byte array (number[])', () => {
    const obj = { tick: 7 };
    const json = JSON.stringify(obj);
    const bytes = Array.from(new TextEncoder().encode(json));
    const decoded = decodeBytes(bytes);
    expect(decoded).not.toBeNull();
    expect(decoded.tick).toBe(7);
  });
});
