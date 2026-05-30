# Magnetite FPS Starter

A credible, minimal **first-person shooter** starter built on the
[Magnetite SDK](../backend/magnetite-sdk), [Bevy 0.14](https://bevyengine.org),
and [bevy_rapier3d](https://rapier.rs).  It demonstrates the **advanced 3-D
path** on the Magnetite platform — the same SDK surface that powers the simple
arcade game template, scaled up to a full FPS.

---

## What you get

| Feature | Details |
|---|---|
| Player controller | First-person WASD + mouse look; jump + crouch; sprint |
| Gamepad support | Left stick move, right stick look, face buttons, triggers — unified through `InputMap` |
| Hitscan shooting | Instant-hit raycast; headshot detection; configurable damage |
| Simple level | 80×80m arena, cover boxes, outer walls; all described as rapier3d collider descriptors |
| Multiplayer-ready | Full `GameLogic` impl: snapshot/restore, `on_player_join/leave`, authoritative tick loop |
| Points economy ready | `kills` + `deaths` in `FpsPlayerCustom::custom`, `score` in the platform `PlayerState` |
| Respawn system | Configurable respawn timer, spawn-point selection (furthest from enemies) |
| WASM + native | One codebase; `--features native` runs a Bevy desktop window; WASM compiles for the browser |

---

## Quick start

### Fast check (no Bevy build, CI-friendly)

```bash
cd game-template-fps
cargo check --no-default-features
```

### Native desktop (Bevy window + rapier3d)

```bash
cargo run --features native
```

Controls: **WASD** move, **mouse** look, **Space** jump, **Ctrl/C** crouch,
**Shift** sprint, **Z/LMB** fire, **X/RMB** aim.

With a controller plugged in: **left stick** move, **right stick** look,
**A** jump, **B** crouch, **RT** fire, **LT** aim, **LB** sprint.

### WASM (browser)

```bash
# Install prerequisites once:
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2  # match Cargo.toml version

# Build:
./build.sh            # release → dist/
./build.sh --dev      # debug   → dist/
./build.sh --serve    # build + open a local HTTP server at :8080
```

The WASM module exposes [`FpsGameHandle`] via wasm-bindgen:

```js
import init, { FpsGameHandle } from './dist/fps.js';

await init();
const game = new FpsGameHandle();
const pid  = game.add_player();

// Game loop (60 Hz):
function loop() {
  const inputJson = JSON.stringify({ keys: { forward: true, ... }, mouse: { ... }, sequence: n++ });
  game.handle_input(pid, inputJson);
  game.tick();
  const state = JSON.parse(game.get_state());
  render(state);
  requestAnimationFrame(loop);
}
```

---

## Project layout

```
game-template-fps/
├── Cargo.toml          — features: native / wasm / (neither = SDK-only)
├── build.sh            — WASM build + native run helper
├── README.md
└── src/
    ├── lib.rs           — FpsGame: GameLogic impl + wasm-bindgen API
    ├── main.rs          — native binary entry (requires native feature)
    ├── input_map.rs     — InputMap: keyboard/mouse/gamepad → FpsAction
    ├── hitscan.rs       — Ray, hitscan cast, Projectile
    ├── level.rs         — LevelDescriptor, spawn points, floor/wall helpers
    └── bevy_client.rs   — Bevy + rapier3d plugin (feature-gated)
```

---

## Architecture

```
        Client side                     Server side (Magnetite platform)
 ┌───────────────────────────┐         ┌────────────────────────────────┐
 │  Bevy App                 │         │  FpsGame (GameLogic impl)      │
 │  ├─ collect_keyboard_input│         │  ├─ handle_input (deterministic)│
 │  ├─ collect_gamepad_input │  Input  │  ├─ tick (gravity, hitscan,    │
 │  ├─ run_local_game_tick   │──────►  │  │   respawn, world payload)   │
 │  └─ sync_player_entities  │◄──────  │  ├─ snapshot / restore         │
 │      (rapier3d capsules)  │ State   │  └─ on_player_join/leave       │
 └───────────────────────────┘         └────────────────────────────────┘
```

### Netcode hooks

- `snapshot()` / `restore()` — GGPO-style rollback-and-reconcile.
- `handle_input()` is **pure**: same input + same state → same output.
- `on_player_join` / `on_player_leave` — platform-driven lifecycle.
- `InterestManager` (from `magnetite_sdk::networking`) can cull distant players
  for bandwidth efficiency at scale (use `RadiusInterest` or a custom impl).

### Gamepad protocol

The `InputMap` module defines a set of `KeyCode::Custom(u8)` codes used to
carry gamepad axis values and button presses through the standard `Input` frame.
The Bevy `collect_gamepad_input` system (native) writes into `PendingInput`
using Bevy's `Gamepads` + `Axis<GamepadAxis>` resources.  On WASM the JS host
uses the Gamepad API and maps to the same custom codes before calling
`FpsGameHandle::handle_input`.

---

## Scaling up

| Concern | Path |
|---|---|
| More weapons | Add variants to `FpsPlayerCustom`; extend `FpsAction` and `InputMap` |
| Projectile weapons | Use `hitscan::Projectile` per tick in `FpsGame::tick` |
| Realistic physics | Swap the simple gravity integration for rapier3d `KinematicCharacterController` |
| Server authority | Remove `run_local_game_tick` from the client; consume server snapshots instead |
| Lag compensation | Use `PredictionBuffer` + `restore` to re-wind and re-simulate on ack |
| Anti-cheat | Run `handle_input` server-side only; clients only predict locally |
| Voice + chat | Call `magnetite_sdk::platform::comms::CommsClient` from `on_player_join` |
| Points / leaderboard | Post `kills`/`score` to the platform Points API after each kill |

---

## License

MIT — see the repository root.
