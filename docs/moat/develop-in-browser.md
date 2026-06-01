# Develop and Play Magnetite Games in the Browser

Magnetite's Web Studio lets you create, build, and test authoritative
multiplayer games without leaving the browser. This guide covers the
complete in-browser path from project creation to live playable preview.

---

## Overview of the browser-based workflow

```
Web Studio (browser)
  │  Pick a template tier → scaffold
  ▼
Repository / local checkout
  │  Implement AuthoritativeGame  (the ONLY Rust you write)
  ▼
magnetite build  (wasm32-wasip1)
  │  produces game.wasm
  ▼
magnetite dev  (or CI push)
  │  WasmExecutor + GameServer → WebSocket URL
  ▼
magnetite-web-client (browser canvas)
  │  Connect → play → share
  ▼
magnetite deploy → publish to marketplace
```

The web path is parallel to the CLI path. Both speak the same
`ServerNet` / `ClientNet` protocol over WebSocket; the runtime is
identical (`magnetite-runtime`). The only difference is that instead of
a native Bevy app you connect with the lightweight `magnetite-web-client`
JS module running in a `<canvas>` tab.

---

## Step 1 — Create a project in the Web Studio

Sign in to Magnetite and open **Developer Portal → Studio**.

1. Click **New Game**.
2. Choose a **template tier**:

   | Tier | What you get | Typical use case |
   |------|-------------|-----------------|
   | Minimal | Bare `AuthoritativeGame` stub | Starting from scratch |
   | Arena Shooter | Full top-down arena reference game | Action / jam games |
   | FPS Starter | First-person movement + physics skeleton | Shooter prototypes |
   | Motorsport Starter | Track, lap counter, collision hull | Racing games |

3. Enter a project name. The Studio calls
   `POST /api/v1/games` and returns a `game_id` UUID plus a scaffold
   archive (a `magnetite new`-equivalent zip you can clone directly or
   open in a GitHub Codespace).

4. Connect your GitHub repo when prompted. The Magnetite GitHub App
   monitors push events and triggers builds automatically.

The Studio also shows you the `magnetite` CLI command you can use
offline:

```bash
magnetite new my-game --template arena-shooter
```

---

## Step 2 — Implement `AuthoritativeGame`

Open `src/lib.rs` in your editor (or the Codespace). The scaffold has
already wired up the Wasm ABI exports (`mag_init`, `mag_step`,
`mag_snapshot`, `mag_restore`, `mag_view`) so you only fill in the game
trait.

The determinism contract is non-negotiable — the replay verifier
enforces it tick-by-tick:

- **Use only `StepCtx::rng`** (`DeterministicRng` / xoshiro256\*\*) for
  randomness. Never call `rand::thread_rng` or any OS RNG inside
  `step` or `validate`.
- **Never read wall-clock time** in `step` or `validate`. Use
  `ctx.tick` and `ctx.dt_ms` for time.
- **No `f64` accumulation across ticks** where cross-platform
  bit-exactness matters. Use `f32` with bounded per-tick increments,
  or fixed-point integers.
- **No I/O, no threads, no blocking** — the Wasmtime sandbox strips
  these capabilities.

For a full walkthrough of the trait and its methods see
[quickstart.md](quickstart.md).

---

## Step 3 — Build

### Option A — CLI (local)

```bash
# Inside your game crate:
magnetite build
```

Runs `cargo build --release --target wasm32-wasip1 --features wasm` and
writes `target/wasm32-wasip1/release/<name>.wasm`.

### Option B — CI (Studio push)

Push to the connected repo. The Magnetite GitHub App triggers a build
and streams logs to **Studio → Builds**. When the build is green, the
artifact is registered automatically.

### Option C — Codespace / in-browser terminal

The Codespace template has the `wasm32-wasip1` target and the
`magnetite` CLI pre-installed. Open the integrated terminal and run
`magnetite build` exactly as in Option A.

---

## Step 4 — Preview in the browser with `magnetite dev`

```bash
magnetite dev
```

This single command:

1. Builds the Wasm artifact (equivalent to `magnetite build`).
2. Loads `game.wasm` into `magnetite-sandbox` (`WasmExecutor`) — the
   game logic runs inside Wasmtime with fuel limits, memory cap, and
   epoch interruption.
3. Starts `magnetite-runtime` in `SingleRoom` topology (up to 16
   players by default).
4. Binds a WebSocket listener on a free port.
5. Prints a connect URL and a **play URL**:

```
Loading `target/wasm32-wasip1/release/my_game.wasm`…

  Connect URL : ws://127.0.0.1:54321
  Play URL    : http://localhost:54321/play?token=dev-token
  Topology    : SingleRoom (max 4 players)
  Tick rate   : 60 Hz

Press Ctrl-C to stop.
```

Open the Play URL in any modern browser. `magnetite-web-client`
connects over WebSocket, receives `ServerNet::Welcome`, and begins
the tick loop:

```
ServerNet::Welcome   → player_id + MatchConfig recorded
ServerNet::Snapshot  → full game state applied to canvas
ServerNet::Delta     → per-tick interest-filtered diff applied
ServerNet::Ack       → client-side prediction reconciled
ServerNet::Reject    → rejected input logged to console
```

The client sends `ClientNet::InputFrame { seq, tick, input }` on every
animation frame. Keyboard and mouse inputs are captured automatically;
gamepad inputs are forwarded via the Gamepad API.

### Sandbox limits during `magnetite dev`

| Limit | Default |
|-------|---------|
| Fuel per step | 10,000,000 Wasm instructions |
| Max memory | 64 MiB |
| Max wall time per step | 10 ms (2 epochs × 5 ms) |

These are the same limits used in production so local testing is
representative.

---

## Step 5 — Preview from the Studio (provisioned instance)

After a successful CI build the Studio **Preview** button is enabled.
Clicking it:

1. Provisions a `magnetite-runtime` instance for your game version via
   the Magnetite platform backend.
2. Returns a **play manifest** — a signed WebSocket URL and a short-
   lived token:

   ```json
   {
     "ws_url": "wss://runtime-eu1.magnetite.dev/ws",
     "token":  "eyJ…",
     "game_id": "01234567-89ab-cdef-0123-456789abcdef",
     "version_id": "abc123"
   }
   ```

3. Opens the in-browser play view using `magnetite-web-client` pointed
   at the provisioned instance.

You can share the play URL with anyone — no installation required. The
runtime instance is torn down after an idle timeout.

---

## The `magnetite-web-client` module

`magnetite-web-client` is a zero-dependency ES module (plain
JavaScript) that speaks the authoritative `ServerNet` / `ClientNet`
protocol over a browser WebSocket. It is the browser equivalent of
`game-client-bevy`.

### Public API

```js
import { MagnetiteClient, arenaApplyInput } from '/magnetite-web-client/src/index.js';

const client = new MagnetiteClient(canvas, {
  wsUrl:        'ws://127.0.0.1:54321',
  token:        'dev-token',
  applyInput:   arenaApplyInput,   // swap for your own game's prediction function
  onStateUpdate: (view) => { /* custom renderer hook */ },
});

client.connect();   // sends ClientNet::InputFrame every animation frame
client.disconnect();
```

`MagnetiteClient` manages:

- WebSocket lifecycle and reconnection.
- `PredictionBuffer`-equivalent: sequence numbers, local prediction,
  reconciliation on `Ack` and `Snapshot`.
- Default 2D canvas renderer for `ArenaView` (the reference shooter's
  view type); injecting `onStateUpdate` lets you override rendering
  for any game.

### Extending the renderer for your game

```js
const client = new MagnetiteClient(canvas, {
  wsUrl:  'ws://127.0.0.1:54321',
  token:  'dev-token',
  onStateUpdate: (view, localPlayerId) => {
    // `view` is the JSON-decoded value of AuthoritativeGame::view_for(player)
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    renderMyGame(ctx, view, localPlayerId);
  },
});
```

The view shape matches whatever `type View` you defined in
`AuthoritativeGame`. Deserialise fields and draw to the `<canvas>` — no
framework required.

---

## Protocol reference (browser → server and back)

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

### Server → client

```
ServerNet::Welcome   { player_id: u64, config: MatchConfig }
ServerNet::Snapshot  { tick: u64, full: Vec<u8> }    // every MatchConfig.snapshot_every ticks
ServerNet::Delta     { tick: u64, since_tick: u64, diff: Vec<u8> }
ServerNet::Ack       { seq: u32, tick: u64 }
ServerNet::Reject    { seq: u32, reason: RejectReason }
```

`full` and `diff` are base64-encoded JSON (serde_json serialises
`Vec<u8>` as base64). `decodeBytes` in the web client handles both
base64 strings and raw `number[]` arrays.

---

## Publish

Once you are happy with the preview:

```bash
magnetite deploy
```

or click **Publish** in the Studio. The platform promotes the artifact
to the marketplace where players can discover and play it in-browser
with no download.

---

## Troubleshooting

### The canvas is blank

Check the browser console for WebSocket errors. Confirm `magnetite dev`
is running and the URL matches. If the token is expired, restart
`magnetite dev` to get a fresh token.

### "fuel exhausted" error in `magnetite dev` output

Your `step` function is doing too much work per tick. Common causes:
unbounded loops, O(n²) collision checks, or large allocations. Profile
with `wasm-opt` or reduce tick work.

### State hash divergence (`ReplayVerdict::Divergence`)

Your game has a determinism bug. Run `verify_replay` in a test:

```rust
assert_eq!(verify_replay::<MyGame>(&log), ReplayVerdict::Clean);
```

Look for `HashMap` fields in your `Snapshot` (non-deterministic
iteration order), `f64` accumulation, or any external random source.

### The web client predicts wrong positions

Implement a custom `applyInput` function that matches your game's
`validate` + `step` logic, and pass it via the `applyInput` option to
`MagnetiteClient`.

---

## Further reading

- [quickstart.md](quickstart.md) — full CLI-based developer walkthrough
- [architecture-overview.md](architecture-overview.md) — crate map and
  per-tick pipeline
- [../../docs/MOAT-ARCHITECTURE.md](../MOAT-ARCHITECTURE.md) — frozen
  interface definitions (source of truth)
- `game-template-authoritative/src/` — reference arena shooter
  implementation (Snapshot / Delta / View / Command shapes)
- `src/magnetite-web-client/src/` — web client source and tests
