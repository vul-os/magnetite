# Motorsport Starter Template

The `game-templates/motorsport/` crate (`magnetite-game-motorsport`) is a ready-made
racing / motorsport game built on **Bevy + rapier3d** vehicle physics, with analog
gamepad input and automatic lap → points integration.

---

## What's included

| Module | File | Description |
|--------|------|-------------|
| Game entry point | `src/lib.rs` | `MotorsportGame` struct implementing `GameLogic` |
| Native binary | `src/main.rs` | Desktop runner |
| Vehicle physics | Inline in `lib.rs` | rapier3d rigid body + wheel joints, suspension, friction |
| Analog input | Inline in `lib.rs` | `GamepadAxis::RightTrigger` → throttle, `LeftTrigger` → brake, `LeftStickX` → steer |
| Lap timing | Inline in `lib.rs` | Checkpoint detection → lap time → points award |

---

## Requirements

- Rust 1.75+
- No extra tooling for `cargo check --no-default-features`
- Bevy render dependencies for native builds (see Bevy docs)
- `wasm-bindgen-cli` for WASM builds

---

## Getting started

```bash
git clone https://github.com/<org>/magnetite.git
cd magnetite/game-templates/motorsport
```

### Fast CI check

```bash
cargo check --no-default-features
cargo test --no-default-features
```

26 tests cover the game logic (lap timing, points calculation, vehicle state) without
compiling Bevy or rapier3d.

### Native run

```bash
cargo run --features native
```

### WASM build

```bash
cargo build --target wasm32-unknown-unknown --no-default-features --features wasm
wasm-bindgen target/wasm32-unknown-unknown/debug/magnetite_game_motorsport.wasm \
  --out-dir pkg --web
```

---

## Vehicle physics

The template uses rapier3d rigid bodies for the car chassis and wheel joints for suspension.
Key parameters you can customise:

```rust
// src/lib.rs
const VEHICLE_MASS:       f32 = 1_200.0;   // kg
const MAX_THROTTLE_FORCE: f32 = 8_000.0;   // N
const MAX_BRAKE_FORCE:    f32 = 12_000.0;  // N
const MAX_STEER_ANGLE:    f32 = 0.45;      // radians (~25°)
const SUSPENSION_STIFFNESS: f32 = 15_000.0;
const SUSPENSION_DAMPING:   f32 = 1_500.0;
```

---

## Analog input

The vehicle reads analog axes directly from `GamepadAxis`:

```rust
use magnetite_sdk::input::gamepad::GamepadAxis;

fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action {
    let throttle = input.gamepad_axis(GamepadAxis::RightTrigger);  // 0.0 … 1.0
    let brake    = input.gamepad_axis(GamepadAxis::LeftTrigger);   // 0.0 … 1.0
    let steer    = input.gamepad_axis(GamepadAxis::LeftStickX);    // -1.0 … 1.0

    self.vehicle.apply_throttle(player_id, throttle);
    self.vehicle.apply_brake(player_id, brake);
    self.vehicle.apply_steer(player_id, steer);

    Action::None
}
```

Keyboard fallback: W → full throttle, S → full brake, A/D → full steer.

---

## Lap timing and points

When a player crosses the finish line, the game calculates the lap time and awards points:

```rust
fn on_lap_complete(&self, player_id: PlayerId, lap_time_ms: u64) {
    let pts = match lap_time_ms {
        t if t < GOLD_MS   => 500,
        t if t < SILVER_MS => 250,
        _                  => 100,
    };
    // Points are awarded via platform::points SDK.
    let client = self.points_client.clone();
    tokio::spawn(async move {
        let _ = client.award_points(AwardPointsRequest {
            user_id: player_id.into(),
            points: pts,
            reason: "lap_complete".to_string(),
            game_id: Some(GAME_ID),
            metadata: Some(json!({ "lap_time_ms": lap_time_ms })),
        }).await;
    });
}
```

---

## Track layout

The current template uses a simple oval circuit defined by waypoints. Extend it by:

1. Replacing the waypoint list in `lib.rs` with your own checkpoint sequence.
2. Adding rapier3d `ColliderBuilder::trimesh` for track barriers.
3. Optionally loading a Blender-exported GLTF mesh for the visual track surface.

---

## Graphics tier

The motorsport template declares `GraphicsTier::Advanced3D`:

```rust
fn metadata(&self) -> GameMetadata {
    GameMetadata {
        name: "Rustcraft Rally".to_string(),
        max_players: 12,
        tick_rate: 60,
        graphics_tier: Some(GraphicsTier::Advanced3D),
        ..Default::default()
    }
}
```

This enables HDR, physics substeps (for stable vehicle suspension at high speed), and
shadow maps — appropriate for a motorsport game.

---

## Multiplayer considerations

- Use `interest_manager` from `magnetite_sdk::networking` to limit state sync to nearby
  vehicles (relevant in large lobbies).
- Vehicle state serialises cleanly — position, velocity, steer angle, and lap number are
  all `f32`/`u32` primitives that compress well in the SDK's wire protocol.
- The server runs `tick()` at 60 Hz; the client uses the `PredictionBuffer` for smooth
  local input response while waiting for server acknowledgement.

---

## Publishing

1. Implement your track, vehicles, and lap logic.
2. Push to GitHub, register in the developer dashboard.
3. Platform CI runs `cargo check --no-default-features`.
4. WASM build for browser delivery.
5. Submit for review.

---

## See also

- [Quickstart](./quickstart.md) — full publish workflow
- [Controllers & Gamepad Input](./controllers.md) — analog axis details
- [Graphics Tiers](./graphics-tiers.md) — Advanced3D + physics substeps
- [Points Economy](./points-economy.md) — lap → points integration
- [FPS Starter Template](./fps-starter.md)
