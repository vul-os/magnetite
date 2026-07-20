# Circuit Rush — Magnetite Motorsport Starter

A racing / motorsport game starter template built on the
[Magnetite platform SDK](../../backend/magnetite-sdk).  Demonstrates:

- **Bevy + rapier3d** vehicle physics on the Magnetite SDK
- **Gamepad / controller input** — analog throttle (right trigger), brake
  (left trigger), steering (left stick) via the SDK `InputMap` convention
- **Sector-based lap timing** with platform leaderboard submission
- **Server-authoritative physics** (`RacingGame` implements `GameLogic`)
- **Native + WASM** compilation (same crate, feature-gated)

---

## Quick start

### Fast CI check (no Bevy build)

```bash
cargo check --no-default-features
# Or via the build script:
./build.sh --check
```

### Native desktop window

```bash
cargo run --features native
# Or:
./build.sh --native
```

Controls: `W`/`S` throttle/brake, `A`/`D` steer, `Space` handbrake.

### WASM browser build

```bash
# Prerequisites:
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli

./build.sh            # release → dist/
./build.sh --serve    # build + serve on http://localhost:8081
```

---

## Architecture

```
src/
  lib.rs        — game logic + all types (no feature flags needed)
  main.rs       — native binary entry point
```

### Key types

| Type | Purpose |
|---|---|
| `RacingGame` | Implements `GameLogic`; the server instantiates one per session |
| `Vehicle` | Per-car physics (discrete raycast-suspension model) |
| `Track` | Oval with 6 sector gates + finish line |
| `LapTimer` | Sector progression + lap time recording |
| `VehicleControls` | Normalised throttle/brake/steer/handbrake |
| `InputMap` (= `VehicleControls::from_input`) | SDK `Input` → `VehicleControls` |
| `RaceWorld` | Full world snapshot (baked into `GameState::world`) |

### Gamepad input convention

Until `GamepadState` is added to the SDK, analog controller axes are
carried on the existing `MouseState` fields:

| `MouseState` field | Gamepad axis |
|--------------------|-------------|
| `scroll` | Right trigger (throttle, 0 → +1) |
| `delta_y` | Left trigger (brake, negative: −1 → 0) |
| `delta_x` | Left stick X (steer, −1 → +1) |

The HTML host page (`index.html`) polls the Gamepad API and maps axes
to this convention automatically.

### Platform score / leaderboard

`PlayerState::score = -(best_lap_ms)`.  More negative = faster lap = higher
on the platform's descending leaderboard.  Configure the leaderboard for
ascending sort in the game metadata for a conventional lower-is-better display.

---

## Track layout (starter oval)

```
       Turn 1 apex (100, -35)
   ┌───────────────────────────┐
   │                           │
Turn 4 exit (-90, 0)  ←  ←  ← Start/Finish (0, 0)
   │                           │
   └───────────────────────────┘
       Turn 3 apex (-100, -35)
```

Six gates (sectors 0–5) define the valid lap path.  The car must pass
every gate in order; skipping a gate invalidates the lap.

---

## Extending this template

- **Track mesh:** replace the flat ground plane with a Bevy `Mesh` loaded
  from GLTF; update gate positions to match.
- **Multiple cars:** the server already supports up to 8 players; call
  `on_player_join` and drive each with a separate `handle_input` call.
- **Collision:** add `bevy_rapier3d` `Collider` components to car + track
  wall entities in `bevy_client::setup_scene`.
- **HUD / minimap:** extend `index.html` with lap delta, sector times,
  position standings from `get_lap_times()`.
- **Streaming telemetry:** call the Magnetite comms SDK to broadcast lap
  times to spectators in real time.

---

## Cargo features

| Feature | Description |
|---------|-------------|
| *(none)* | SDK + game logic only — no Bevy/rapier. Fast `cargo check` path. |
| `native` | Full Bevy + rapier3d debug render for desktop. |
| `wasm` | Bevy + rapier3d for browser via WebGL2. |

---

## License

MIT — same as the Magnetite platform.
