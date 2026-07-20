# Play in the browser — web client guide

> Last updated: 2026-06-03 (INFRA-E2E wave)

This guide covers the full path from a registered WASM game artifact to a
player clicking **Play** in the browser and seeing a live canvas. It covers
the `magnetite-web-client` JS module, the `ClientNet`/`ServerNet` protocol,
and how to extend the renderer for your own game.

---

## Overview

```
Browser player
  │  click Play
  ▼
GET /api/v1/distribution/<game-id>/play        (play manifest)
  │  → { ws_url, wasm_url, game_id, version_id }
  ▼
magnetite-web-client (ES module, zero deps)
  │  new MagnetiteClient(canvas, { wsUrl, token })
  ▼
WebSocket → ws://localhost:9000  (magnetite-runtime)
  │  ClientNet::InputFrame { seq, tick, input }
  ◄──────────────────────────────────────────────
  │  ServerNet::Welcome { player_id, config }
  │  ServerNet::Snapshot { tick, full }         (every N ticks)
  │  ServerNet::Delta    { tick, since_tick, diff }  (every tick)
  │  ServerNet::Ack      { seq, tick }
  │  ServerNet::Reject   { seq, reason }
  ▼
<canvas> renders the per-player interest-filtered view
```

The browser never runs game logic. The authoritative `step` function runs
server-side inside `WasmExecutor` (Wasmtime, fuel-metered). The client
receives only the `view_for(player)` subset of state — enemies behind walls
are never sent.

---

## Prerequisites

- The full stack is running (see
  [run-it-all.md](../self-hosting/run-it-all.md) or `magnetite dev`).
- A game is registered and has a successful WASM artifact (the play manifest
  `wasm_url` must not be `null`).
- A modern browser with WebSocket support (Chrome, Firefox, Safari, Edge).

---

## The play manifest

When a player clicks **Play**, `Playground.jsx` calls:

```
GET /api/v1/distribution/<game-id>/play
```

The backend responds with:

```json
{
  "ws_url":     "ws://127.0.0.1:9000",
  "wasm_url":   "file:///tmp/magnetite-wasip1-builds/.../game.wasm",
  "game_id":    "01234567-89ab-cdef-0123-456789abcdef",
  "version_id": "abc123"
}
```

`ws_url` is where the runtime is listening. In production this comes from
`GAME_SERVER_WS_BASE` set in the backend environment. In local dev it is
`ws://127.0.0.1:9000` (the default for `run-runtime.sh`).

---

## The `magnetite-web-client` module

`magnetite-web-client` is a zero-dependency ES module (plain JavaScript)
that lives in `magnetite-web-client/src/index.js`. Import it directly from
the module path — no bundler required.

### Quick start

```js
import { MagnetiteClient, arenaApplyInput } from '/magnetite-web-client/src/index.js';

const canvas = document.getElementById('game-canvas');

const client = new MagnetiteClient(canvas, {
  wsUrl:        'ws://127.0.0.1:9000',
  token:        'dev-token',       // bearer token from your auth session
  applyInput:   arenaApplyInput,   // client-side prediction function
  onStateUpdate: (view) => { /* optional: custom renderer hook */ },
});

client.connect();
// To stop: client.disconnect();
```

`MagnetiteClient` manages:

- WebSocket lifecycle, reconnection, and authentication handshake.
- Sequence number generation for `ClientNet::InputFrame`.
- Client-side prediction via `PredictionBuffer`-equivalent: inputs applied
  immediately then reconciled on `Ack` or `Snapshot` from the server.
- Default 2D canvas renderer for the `ArenaView` reference game.
- Keyboard, mouse, and Gamepad API input capture.

### Constructor options

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `wsUrl` | `string` | Yes | WebSocket URL returned by the play manifest |
| `token` | `string` | Yes | Bearer token for the `?token=` query parameter |
| `applyInput` | `function` | No | Client-side prediction function — receives `(state, input)` and returns updated `state`. Defaults to `arenaApplyInput`. |
| `onStateUpdate` | `function` | No | Called every tick with `(view, localPlayerId)` after the server delta is applied. Override to implement a custom renderer. |
| `onConnect` | `function` | No | Called when the `ServerNet::Welcome` frame is received |
| `onDisconnect` | `function` | No | Called when the WebSocket closes |

---

## The protocol

All frames are JSON-encoded binary WebSocket messages.

### Client → server

```
ClientNet::InputFrame {
  seq:   u32,    // monotonically increasing sequence number
  tick:  u64,    // the tick the client intends this input for
  input: {
    sequence: u64,
    keys: {
      forward, backward, left, right,
      jump, crouch, attack, secondary_attack, interact: bool
    },
    mouse: {
      x, y, delta_x, delta_y: f64,
      left_button, right_button, middle_button: bool
    }
  }
}
```

The web client sends an `InputFrame` on every animation frame (~60/s), even
when no inputs are active. This keeps the server's per-player input queue
populated so ticks proceed without stalls.

### Server → client

| Frame | When | Contains |
|-------|------|---------|
| `ServerNet::Welcome` | On first connect | `player_id: u64`, `config: MatchConfig` |
| `ServerNet::Snapshot` | Every `MatchConfig.snapshot_every` ticks (default 300) | `tick: u64`, `full: base64<Snapshot>` |
| `ServerNet::Delta` | Every tick | `tick: u64`, `since_tick: u64`, `diff: base64<Delta>` |
| `ServerNet::Ack` | After each accepted `InputFrame` | `seq: u32`, `tick: u64` |
| `ServerNet::Reject` | After a rejected `InputFrame` | `seq: u32`, `reason: RejectReason` |

`full` and `diff` are base64-encoded JSON (serde_json serialises `Vec<u8>`
as base64). The `decodeBytes` helper in the web client handles both base64
strings and raw `number[]` arrays for forward compatibility.

### Prediction and reconciliation

The web client maintains a ring buffer of recent inputs and their predicted
state. On `Ack { seq }` the client pops all inputs up to `seq` from the
buffer — they were accepted. On `Reject { seq, reason }` the input is popped
and the rejection is surfaced (e.g. as a UI flash for rate-limited actions).
On `Snapshot { tick, full }` the client resets local state to the authoritative
snapshot and replays any buffered inputs that are newer than `tick`.

---

## Extending the renderer for your game

Replace the default `ArenaView` renderer with your own by passing
`onStateUpdate`:

```js
const client = new MagnetiteClient(canvas, {
  wsUrl:  manifest.ws_url,
  token:  authToken,
  onStateUpdate: (view, localPlayerId) => {
    // `view` is the JSON-decoded value of AuthoritativeGame::view_for(player)
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    renderMyGame(ctx, view, localPlayerId);
  },
});
```

The shape of `view` matches whatever `type View` you defined in your
`AuthoritativeGame` implementation. The server serialises it with
`serde_json` so any JSON-representable Rust struct maps directly to a
JavaScript object.

### Overriding client-side prediction

Client-side prediction makes input feel instant by applying inputs locally
before the server acknowledges them. If your game's movement rules differ
from the default arena shooter, supply a custom `applyInput`:

```js
function myApplyInput(state, input) {
  // Replicate the logic of AuthoritativeGame::validate + the relevant part
  // of step for this player. Must be deterministic and match the server.
  const newState = { ...state };
  if (input.keys.forward) newState.y -= 5;
  if (input.keys.backward) newState.y += 5;
  if (input.keys.left) newState.x -= 5;
  if (input.keys.right) newState.x += 5;
  return newState;
}

const client = new MagnetiteClient(canvas, {
  wsUrl:      manifest.ws_url,
  token:      authToken,
  applyInput: myApplyInput,
  onStateUpdate: (view) => { renderMyGame(ctx, view); },
});
```

Prediction accuracy determines how often the client must snap back to the
server's authoritative state. Divergence is expected and handled
transparently — the canvas will appear smooth as long as the prediction
function is a reasonable approximation of the server's `step`.

---

## `magnetite dev` play URL

When running locally with `magnetite dev`, the runtime prints a play URL you
can open directly:

```
  Connect URL : ws://127.0.0.1:54321
  Play URL    : http://localhost:54321/play?token=dev-token
  Topology    : SingleRoom (max 4 players)
  Tick rate   : 20 Hz

Press Ctrl-C to stop.
```

Opening the Play URL starts `magnetite-web-client` pre-connected to that
runtime instance. Share it on your LAN to play with others (replace
`127.0.0.1` with your machine's LAN IP).

---

## Sandbox limits enforced during play

The same `LimitsConfig` applies in production and local dev:

| Limit | Default |
|-------|---------|
| Fuel per tick | 10,000,000 Wasm instructions |
| Max memory | 64 MiB |
| Max wall time per tick | 10 ms (2 epochs × 5 ms) |

If your game's `step` function exceeds fuel per tick, the runtime logs
`"fuel exhausted"` and the tick is skipped (no crash, no ban). Reduce per-tick
work or increase `LimitsConfig.fuel_per_step` in `magnetite-runtime`'s
config.

---

## Anti-cheat guarantees visible to the browser

| Guarantee | How it works |
|-----------|-------------|
| No client-sent state | The browser sends only `InputFrame`. The server runs `validate` → `step`. Fake position values sent from the browser are rejected before `step`. |
| Interest filtering | `view_for(player)` is the only data sent to the browser. Enemies outside the field of view or behind walls are never serialised. No wallhack possible. |
| Replay logging | Every input and state hash is recorded in `ReplayLog`. The anti-cheat service calls `verify_replay` offline; divergence triggers a flag. |
| Fuel metering | Wasmtime fuel prevents a compromised game module from hanging the server with an infinite loop. |

---

## Troubleshooting

### The canvas is blank after clicking Play

Check the browser DevTools console for WebSocket errors. Confirm:

1. `magnetite-runtime` is running: `curl http://localhost:9000` should return
   a 101 upgrade (or an error message, but not connection refused).
2. The `ws_url` in the play manifest matches where the runtime is listening.
3. The `token` query parameter is appended to the WebSocket URL.

### "fuel exhausted" in the runtime logs

Your `step` function is doing too much work per tick. Common causes:

- Unbounded loops or O(n²) collision detection.
- Large allocations on every tick.
- Complex spatial queries without a spatial index.

Profile with `wasm-opt --print-size` or reduce tick work. As a workaround,
increase `fuel_per_step` in `LimitsConfig`.

### `ReplayVerdict::Divergence` in e2e tests

Your game has a determinism bug. Run `verify_replay` in a test:

```rust
assert_eq!(verify_replay::<MyGame>(&log), ReplayVerdict::Clean);
```

Common causes:

- `HashMap` fields in `Snapshot` (non-deterministic iteration order — use
  `BTreeMap` instead).
- `f64` accumulation across ticks (use `f32` with bounded increments or
  fixed-point integers).
- Any call to `std::random`, `rand::thread_rng`, or `SystemTime` inside
  `step` or `validate`.

### Play manifest returns `ws_url: null`

The backend's `GAME_SERVER_WS_BASE` is not set, or is set to a URL where
the runtime is not listening. Check:

```bash
# If running the backend via docker compose:
docker compose exec backend env | grep GAME_SERVER
# → GAME_SERVER_WS_BASE=ws://magnetite-runtime:9000  (or ws://localhost:9000)

# If running the backend natively:
echo $GAME_SERVER_WS_BASE
```

---

## Further reading

- [run-it-all.md](../self-hosting/run-it-all.md) — full stack runbook
- [quickstart.md](quickstart.md) — CLI developer walkthrough (native + WASM)
- [develop-in-browser.md](develop-in-browser.md) — Studio-based workflow
- [architecture-overview.md](architecture-overview.md) — per-tick pipeline
  and crate map
- `magnetite-web-client/src/` — web client source
- `game-templates/authoritative/src/game.rs` — reference `View` / `Delta` shapes
