# Magnetite Game Template — Dot Collector

> A minimal but real starter game built on the **Magnetite SDK** and
> [Bevy](https://bevyengine.org).  Use it as the scaffold for your own
> Rust game that runs natively on the desktop and in the browser via WASM.

---

## What's here

```
game-templates/arcade/
├── Cargo.toml        — dependencies + feature flags (native / wasm)
├── src/
│   └── lib.rs        — game logic + Bevy plugin + wasm-bindgen entry points
├── index.html        — browser host page (drives the WASM game loop)
└── build.sh          — local build helper (Rust → WASM → dist/)
```

The game itself (`DotCollector`) is a top-down arena where players move a dot,
collect coins for points, and regenerate health over time.  Simple rules, but
it exercises every part of the `magnetite-sdk`:

| SDK type | Used for |
|----------|----------|
| `GameLogic` trait | implemented on `DotCollector` |
| `Input` / `KeyState` / `MouseState` | input framing per tick |
| `Action` / `Direction` | return value of `handle_input` |
| `GameState` / `PlayerState` | state snapshots sent to clients |
| `PlayerId` | player identity |
| `GameMetadata` | name, tick-rate, max-players |

---

## Feature flags

| Flag | What it enables |
|------|-----------------|
| *(none / default)* | Core game logic + WASM-bindgen API. Fast `cargo check`. |
| `wasm` | `console_error_panic_hook` + `web-sys` canvas bindings. Use for WASM builds. |
| `native` | Full Bevy render stack (window, renderer, audio). Use for desktop builds. |

---

## Running natively

```bash
# From the repo root
cd game-templates/arcade

# Build and open a desktop window (requires a GPU / display)
cargo run --features native
```

A top-down window opens.  Use `WASD` / arrow keys to move; collect the amber
coins.

---

## Building for the browser (WASM)

### Prerequisites

```bash
# 1. Add the WASM target
rustup target add wasm32-unknown-unknown

# 2. Install wasm-bindgen CLI (must match Cargo.toml version)
WBG_VER=$(grep 'wasm-bindgen' Cargo.toml | grep -oE '"[0-9.]+"' | tr -d '"')
cargo install wasm-bindgen-cli --version "$WBG_VER" --locked

# Optional: install wasm-opt for smaller binaries
cargo install wasm-opt --locked  # or use the system binaryen package
```

### Build

```bash
# From game-templates/arcade/
./build.sh               # release build → dist/
./build.sh --dev         # debug build (no optimisations)
./build.sh --check       # cargo check only (fastest; no WASM output)
./build.sh --serve       # build + launch local HTTP server on :8080
```

Or call the central script from the repo root:

```bash
GAME_DIR=./game-templates/arcade ./scripts/build-game.sh
```

### Output

```
dist/
  game.js           — ES module glue (import this in index.html)
  game_bg.wasm      — Wasm binary (~200 KB release, ~1 MB debug)
  game.d.ts         — TypeScript declarations
  game_bg.wasm.d.ts
```

### Serve

The `index.html` imports `./dist/game.js`.  Because WASM requires
`Content-Type: application/wasm` from an HTTP server, you **cannot** open
`index.html` directly as a `file://` URL.

```bash
# Option A — build.sh (after building)
./build.sh --serve

# Option B — Python
python3 -m http.server 8080

# Option C — basic-http-server (cargo install basic-http-server)
basic-http-server . --addr 127.0.0.1:8080
```

Then open [http://localhost:8080](http://localhost:8080).

---

## JavaScript API

After loading, the module exports a `GameHandle` class:

```js
import init, { GameHandle } from './dist/game.js';
await init();

const game = new GameHandle();

// Add a second player; returns their numeric ID
const p2 = game.add_player();

// Send input (JSON-serialised magnetite_sdk::Input)
game.handle_input(0, JSON.stringify({
  keys: {
    forward: true, backward: false, left: false, right: false,
    jump: false, crouch: false, attack: false,
    secondary_attack: false, interact: false, sprint: false,
  },
  mouse: {
    x: 0, y: 0, delta_x: 0.5, delta_y: 0,
    left_button: false, right_button: false, middle_button: false, scroll: 0,
  },
  sequence: 1,
  timestamp_ms: Date.now(),
}));

// Advance the simulation by one tick (call at ~60 Hz)
game.tick();

// Read state
const state  = JSON.parse(game.get_state());   // { players: [...] }
const scores = JSON.parse(game.get_scores());  // { "0": 3, "1": 1 }
const tick   = game.tick_count();              // number
```

---

## Implementing your own game

1. Copy this directory and rename `magnetite-game-template` in `Cargo.toml`.
2. Replace `DotCollector` with your own struct.
3. Implement `magnetite_sdk::GameLogic` — the six methods are all you need.
4. Optionally add a Bevy [`Plugin`] in `bevy_client` for local rendering.
5. Run `./build.sh --serve` to test in the browser.
6. Push to GitHub — the `game-ci.yml` workflow builds and optionally deploys.

---

## CI/CD

The workflow at `.github/workflows/game-ci.yml` runs on every push that touches
`game-templates/arcade/` or the SDK:

| Job | What it does |
|-----|-------------|
| `check` | `cargo check --no-default-features` + Clippy |
| `test` | `cargo test --no-default-features` (host, fast) |
| `build-wasm` | Full WASM build → `wasm-bindgen` → `wasm-opt` → upload artifact |
| `audit` | `cargo audit` for known vulnerabilities |
| `deploy` | Upload to Magnetite platform (needs `MAGNETITE_API_KEY` secret) |

To deploy manually: **Actions → Game CI → Run workflow → deploy: true**.

---

## License

MIT — see [LICENSE](../../LICENSE).
