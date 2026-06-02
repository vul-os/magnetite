/**
 * magnetite-web-client/src/smoke.test.js
 *
 * Node/vitest smoke test — drives the full MagnetiteClient protocol pipeline
 * against a MOCK WebSocket server emitting:
 *   Welcome → Snapshot → Delta → Ack
 *
 * No real backend or network needed.  A MockWebSocket intercepts all
 * ConnectionManager activity; outbound InputFrames are recorded and
 * verified.  The test asserts the client:
 *
 *  1. Sets player_id and config on Welcome.
 *  2. Applies Snapshot: authoritative state matches server payload.
 *  3. Applies Delta: state updated with changed player position.
 *  4. Processes Ack: pending frame count drops.
 *  5. Processes Reject: falls back to authoritative state.
 *  6. Fires onState listener on every authoritative update.
 *
 * Environment: vitest + jsdom (WebSocket + btoa/atob available from jsdom).
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

import { createClient } from './client.js';
import { parseServerMessage } from './protocol.js';

// ---------------------------------------------------------------------------
// MockWebSocket — intercepts new WebSocket(...) calls in the jsdom environment
// ---------------------------------------------------------------------------

/**
 * A minimal synchronous mock WebSocket.
 *
 * The test holds a reference via MockWebSocket.lastInstance and can
 * call .serverSend(raw) to push a message to the client, exactly as
 * the real server would.
 */
class MockWebSocket extends EventTarget {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  constructor(url) {
    super();
    this.url = url;
    this.readyState = MockWebSocket.CONNECTING;
    this.sent = []; // records raw strings sent by the client
    MockWebSocket.lastInstance = this;

    // Simulate async open on the next tick
    Promise.resolve().then(() => {
      this.readyState = MockWebSocket.OPEN;
      this.dispatchEvent(new Event('open'));
    });
  }

  /** Called by the client code (connection.js) */
  send(data) {
    this.sent.push(data);
  }

  /** Called by the test to push a server message to the client */
  serverSend(raw) {
    const evt = new MessageEvent('message', { data: raw });
    this.dispatchEvent(evt);
  }

  close(code, reason) {
    this.readyState = MockWebSocket.CLOSED;
    this.dispatchEvent(new CloseEvent('close', { code: code ?? 1000, reason: reason ?? '' }));
  }
}
MockWebSocket.lastInstance = null;

// ---------------------------------------------------------------------------
// Encode a server payload as base64 (mirrors serde_json Vec<u8> encoding)
// ---------------------------------------------------------------------------

function encodePayload(obj) {
  const json = JSON.stringify(obj);
  return btoa(json); // base64 string, matches decodeBytes in protocol.js
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makePlayer(id, x = 0, y = 0, hp = 100, alive = true) {
  return { id, x, y, angle: 0, hp, alive, last_shot_tick: 0, score: 0 };
}

function makeArenaSnapshot(players = [], projectiles = [], tick = 0) {
  return { players, projectiles, tick };
}

/** Build a Welcome message (ServerNet::Welcome) */
function welcome(playerId = 'p1', config = { tick_hz: 20, max_players: 4, seed: 1 }) {
  return JSON.stringify({ type: 'welcome', player_id: playerId, config });
}

/** Build a Snapshot message (ServerNet::Snapshot) */
function snapshot(tick, snapshotObj) {
  return JSON.stringify({ type: 'snapshot', tick, full: encodePayload(snapshotObj) });
}

/** Build a Delta message (ServerNet::Delta) */
function delta(tick, sinceTick, deltaObj) {
  return JSON.stringify({ type: 'delta', tick, since_tick: sinceTick, diff: encodePayload(deltaObj) });
}

/** Build an Ack message (ServerNet::Ack) */
function ack(seq, tick) {
  return JSON.stringify({ type: 'ack', seq, tick });
}

/** Build a Reject message (ServerNet::Reject) */
function reject(seq, reason = 'RateLimited') {
  return JSON.stringify({ type: 'reject', seq, reason });
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe('MagnetiteClient mock-WS smoke test', () => {
  let originalWebSocket;
  let client;
  let stateEvents;

  beforeEach(() => {
    // Replace the global WebSocket with our mock BEFORE createClient is called
    originalWebSocket = globalThis.WebSocket;
    globalThis.WebSocket = MockWebSocket;
    MockWebSocket.lastInstance = null;
    stateEvents = [];
  });

  afterEach(() => {
    // Restore and clean up
    if (client) {
      client.disconnect();
      client = null;
    }
    globalThis.WebSocket = originalWebSocket;
    vi.restoreAllMocks();
  });

  // --------------------------------------------------------------------------
  // Helper: connect and wait for the mock WS to open
  // --------------------------------------------------------------------------

  async function connectAndOpen(_playerId = 'p1') {
    client = createClient({
      url: 'ws://localhost:9001',
      autoReconnect: false,
    });

    // Subscribe to state events BEFORE connecting
    client.onState((s) => stateEvents.push(s));

    client.connect();

    // Wait for the mock WS to open (happens on next micro-task)
    await new Promise((r) => setTimeout(r, 0));

    const ws = MockWebSocket.lastInstance;
    expect(ws).not.toBeNull();
    expect(ws.readyState).toBe(MockWebSocket.OPEN);

    return ws;
  }

  // --------------------------------------------------------------------------
  // 1. Welcome — player_id + config applied
  // --------------------------------------------------------------------------

  it('applies Welcome: sets player_id and config', async () => {
    const ws = await connectAndOpen('player-42');

    ws.serverSend(welcome('player-42', { tick_hz: 30, max_players: 8, seed: 99 }));

    expect(client.playerId).toBe('player-42');
    expect(client.matchConfig).toMatchObject({ tick_hz: 30, max_players: 8 });
  });

  // --------------------------------------------------------------------------
  // 2. Snapshot — client applies full authoritative state
  // --------------------------------------------------------------------------

  it('applies Snapshot: authoritative state reflects server payload', async () => {
    const ws = await connectAndOpen('p1');

    const snap = makeArenaSnapshot([makePlayer('p1', 10, 20), makePlayer('p2', 50, 50)], [], 5);

    ws.serverSend(welcome('p1'));
    ws.serverSend(snapshot(5, snap));

    // state should now be the ArenaView of p1
    const state = client.state;
    expect(state).not.toBeNull();
    expect(state.self_state).not.toBeNull();
    expect(state.self_state.id).toBe('p1');
    expect(state.self_state.x).toBe(10);
    expect(state.self_state.y).toBe(20);
    expect(state.other_players).toHaveLength(1);
    expect(state.other_players[0].id).toBe('p2');
    expect(state.tick).toBe(5);
  });

  // --------------------------------------------------------------------------
  // 3. Delta — client updates state from a diff
  // --------------------------------------------------------------------------

  it('applies Delta: state updated with changed player position', async () => {
    const ws = await connectAndOpen('p1');

    const initialSnap = makeArenaSnapshot(
      [makePlayer('p1', 0, 0), makePlayer('p2', 10, 10)],
      [],
      1
    );

    ws.serverSend(welcome('p1'));
    ws.serverSend(snapshot(1, initialSnap));

    // Server sends a delta — p1 moved to (30, 40)
    const d = {
      changed_players: [makePlayer('p1', 30, 40, 90)],
      removed_projectile_ids: [],
      new_projectiles: [],
    };
    ws.serverSend(delta(2, 1, d));

    const state = client.state;
    expect(state).not.toBeNull();
    expect(state.self_state.x).toBe(30);
    expect(state.self_state.y).toBe(40);
    expect(state.self_state.hp).toBe(90);
    expect(state.tick).toBe(2);
  });

  // --------------------------------------------------------------------------
  // 4. Ack — pending frame count decreases
  // --------------------------------------------------------------------------

  it('processes Ack: pending frame count decreases', async () => {
    const ws = await connectAndOpen('p1');

    const snap = makeArenaSnapshot([makePlayer('p1', 0, 0)], [], 0);
    ws.serverSend(welcome('p1'));
    ws.serverSend(snapshot(0, snap));

    // Manually push some inputs so the prediction buffer has pending frames
    const input = {
      keys: { forward: true, backward: false, left: false, right: false, jump: false,
               crouch: false, attack: false, secondary_attack: false, interact: false, sprint: false },
      mouse: { x: 0, y: 0, delta_x: 0, delta_y: 0, left_button: false, right_button: false,
               middle_button: false, scroll: 0 },
      sequence: 1,
      timestamp_ms: 1000,
    };

    client.sendInput({ ...input, sequence: 1 });
    client.sendInput({ ...input, sequence: 2 });
    client.sendInput({ ...input, sequence: 3 });

    // Access internals to verify pending count
    const pendingBefore = client._prediction.pendingCount;
    expect(pendingBefore).toBeGreaterThanOrEqual(1);

    // Server acks seq 2 (acknowledges frames 1 and 2)
    ws.serverSend(ack(2, 2));

    const pendingAfter = client._prediction.pendingCount;
    expect(pendingAfter).toBeLessThan(pendingBefore);
    // Only seq=3 should remain
    expect(pendingAfter).toBe(1);
  });

  // --------------------------------------------------------------------------
  // 5. Reject — falls back to authoritative state, remaining frames replayed
  // --------------------------------------------------------------------------

  it('processes Reject: state rolls back and replays remaining frames', async () => {
    const ws = await connectAndOpen('p1');

    const snap = makeArenaSnapshot([makePlayer('p1', 5, 5)], [], 0);
    ws.serverSend(welcome('p1'));
    ws.serverSend(snapshot(0, snap));

    const input = {
      keys: { forward: false, backward: false, left: false, right: true, jump: false,
               crouch: false, attack: false, secondary_attack: false, interact: false, sprint: false },
      mouse: { x: 0, y: 0, delta_x: 0, delta_y: 0, left_button: false, right_button: false,
               middle_button: false, scroll: 0 },
      sequence: 1,
      timestamp_ms: 0,
    };

    client.sendInput({ ...input, sequence: 1 });
    client.sendInput({ ...input, sequence: 2 });

    const stateBeforeReject = client.state;
    expect(stateBeforeReject).not.toBeNull();

    // Server rejects seq 1
    ws.serverSend(reject(1, 'RateLimited'));

    // State should still be non-null (replayed from authority + seq=2)
    expect(client.state).not.toBeNull();
    // Pending count should be 1 (only seq=2 remains)
    expect(client._prediction.pendingCount).toBe(1);
  });

  // --------------------------------------------------------------------------
  // 6. onState listener fires on authoritative updates
  // --------------------------------------------------------------------------

  it('onState listener fires on Welcome Snapshot and Delta', async () => {
    const ws = await connectAndOpen('p1');

    const snap = makeArenaSnapshot([makePlayer('p1', 0, 0)], [], 1);
    ws.serverSend(welcome('p1'));
    ws.serverSend(snapshot(1, snap));

    const d = {
      changed_players: [makePlayer('p1', 10, 10)],
      removed_projectile_ids: [],
      new_projectiles: [],
    };
    ws.serverSend(delta(2, 1, d));

    // stateEvents collected by the onState listener registered in beforeEach
    // should have at least: one from snapshot + one from delta
    expect(stateEvents.length).toBeGreaterThanOrEqual(2);
    const last = stateEvents[stateEvents.length - 1];
    expect(last.self_state.x).toBe(10);
    expect(last.self_state.y).toBe(10);
  });

  // --------------------------------------------------------------------------
  // 7. Multiple Snapshots reconcile correctly
  // --------------------------------------------------------------------------

  it('reconciles on a second Snapshot (server correction)', async () => {
    const ws = await connectAndOpen('p1');

    const snap1 = makeArenaSnapshot([makePlayer('p1', 0, 0)], [], 1);
    ws.serverSend(welcome('p1'));
    ws.serverSend(snapshot(1, snap1));

    // Client sends some inputs (locally predicted rightward movement)
    const rightInput = {
      keys: { forward: false, backward: false, left: false, right: true, jump: false,
               crouch: false, attack: false, secondary_attack: false, interact: false, sprint: false },
      mouse: { x: 0, y: 0, delta_x: 0, delta_y: 0, left_button: false, right_button: false,
               middle_button: false, scroll: 0 },
      sequence: 1,
      timestamp_ms: 0,
    };
    client.sendInput({ ...rightInput, sequence: 1 });
    client.sendInput({ ...rightInput, sequence: 2 });

    // Server sends a corrective snapshot at tick 2 placing p1 at (8, 0)
    const snap2 = makeArenaSnapshot([makePlayer('p1', 8, 0)], [], 2);
    ws.serverSend(snapshot(2, snap2));

    // After reconciliation the base is (8, 0); only frames with tick > 2 remain.
    // Both our frames had implicit tick = 0 (sendInput uses client._tick which
    // started at 0 before Welcome and was not incremented in this test), so
    // they are all discarded and the authoritative state is (8, 0).
    expect(client._prediction.authoritativeState).not.toBeNull();
    const auth = client._prediction.authoritativeState;
    expect(auth.self_state.x).toBe(8);
  });

  // --------------------------------------------------------------------------
  // 8. InputFrame wire format is correct (protocol round-trip)
  // --------------------------------------------------------------------------

  it('sends well-formed InputFrame JSON when sendInput is called', async () => {
    const ws = await connectAndOpen('p1');
    ws.serverSend(welcome('p1'));

    const snap = makeArenaSnapshot([makePlayer('p1', 0, 0)], [], 0);
    ws.serverSend(snapshot(0, snap));

    ws.sent = []; // clear any sent before our input

    const input = {
      keys: { forward: true, backward: false, left: false, right: false, jump: false,
               crouch: false, attack: false, secondary_attack: false, interact: false, sprint: false },
      mouse: { x: 0, y: 0, delta_x: 0, delta_y: 0, left_button: false, right_button: false,
               middle_button: false, scroll: 0 },
      sequence: 1,
      timestamp_ms: 42,
    };

    client.sendInput(input);

    expect(ws.sent.length).toBeGreaterThanOrEqual(1);
    const raw = ws.sent[ws.sent.length - 1];
    const parsed = parseServerMessage(raw); // reuse our parser — works for client frames too
    expect(parsed.type).toBe('input_frame');
    expect(parsed.seq).toBeGreaterThan(0);
    expect(parsed.input.keys.forward).toBe(true);
  });
});
