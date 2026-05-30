# FPS Starter Template

The `game-template-fps/` crate (`magnetite-fps-starter`) is a ready-made advanced
first-person shooter built on **Bevy + rapier3d**, controller-ready, and wired to the
Magnetite SDK.

---

## What's included

| Module | File | Description |
|--------|------|-------------|
| Game entry point | `src/lib.rs` | `FpsGame` struct implementing `GameLogic` |
| Bevy ECS plugin | `src/bevy_client.rs` | Bevy plugin, systems, schedules (native + WASM feature flags) |
| Hitscan | `src/hitscan.rs` | Raycast hit registration against rapier3d colliders |
| Level | `src/level.rs` | Procedural level layout with walls, spawn points, pick-ups |
| Input map | `src/input_map.rs` | `InputMap` bindings: right stick → look, left stick → move, RT → shoot, X → reload |
| Binary entry | `src/main.rs` | Native desktop runner |

---

## Requirements

- Rust 1.75+
- `cargo` (no other tooling needed for `cargo check`)
- For a full native build: Bevy render dependencies (SDL2 / Vulkan drivers — see Bevy docs)
- For WASM: `wasm-pack` or `wasm-bindgen-cli`

---

## Getting started

```bash
git clone https://github.com/<org>/magnetite.git
cd magnetite/game-template-fps
```

### Fast CI check (no Bevy compile)

```bash
cargo check --no-default-features
cargo test --no-default-features
```

This is what the platform CI runs. It compiles the game logic and SDK integration
without the full Bevy/rapier render stack, making it fast and dependency-light.

### Native desktop run

```bash
cargo run --features native
```

### WASM build

```bash
cargo build --target wasm32-unknown-unknown --no-default-features --features wasm
wasm-bindgen target/wasm32-unknown-unknown/debug/magnetite_fps_starter.wasm \
  --out-dir pkg --web
```

---

## Implementing your FPS

The `FpsGame` struct implements `GameLogic`. Extend it by modifying:

- **`tick()`** — advance physics (rapier3d step), process queued inputs, check win conditions
- **`handle_input()`** — accept an `Input` frame and queue it for the next tick
- **`hitscan.rs`** — customize damage, headshot multipliers, weapon types
- **`level.rs`** — replace the procedural level with your own geometry

### Adding a weapon

```rust
// src/lib.rs
fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action {
    if input.actions.contains(&Action::Fire) {
        self.queue_shoot(player_id);
    }
    Action::None
}

fn tick(&mut self) {
    for (shooter_id, shot) in self.pending_shots.drain(..) {
        if let Some(hit) = self.hitscan.cast(&shot) {
            self.apply_damage(hit.target_id, shot.weapon.damage);
            // Award points via platform::points
        }
    }
    // … rapier3d physics step …
}
```

---

## Gamepad input

The template uses `InputMap::default()`:

| `GameAction` | Gamepad binding |
|--------|----------------|
| `MoveForward/MoveBackward/MoveLeft/MoveRight` | Left stick |
| `Look { dx, dy }` | Right stick (camera pitch/yaw) |
| `Fire` | Right trigger (RT / R2) |
| `Reload` | West button (X / Square) |
| `Jump` | South button (A / Cross) |

See [Controllers & Gamepad Input](./controllers.md) for custom binding examples.

---

## Graphics tier

The FPS template declares `GraphicsTier::Advanced3D`:

```rust
fn metadata(&self) -> GameMetadata {
    GameMetadata {
        name: "My FPS".to_string(),
        max_players: 16,
        tick_rate: 64,
        graphics_tier: Some(GraphicsTier::Advanced3D),
        ..Default::default()
    }
}
```

This tells the platform to provision a WebGPU-capable runtime, enable HDR, and
allocate adequate server resources.

---

## Platform integration

### Points on kill

```rust
use magnetite_sdk::platform::points::{AwardPointsRequest, PointsClient};

fn on_kill(&self, killer_id: PlayerId) {
    let client = self.points_client.clone();
    tokio::spawn(async move {
        let _ = client.award_points(AwardPointsRequest {
            user_id: killer_id.into(),
            points: 100,
            reason: "kill".to_string(),
            game_id: Some(GAME_ID),
            metadata: None,
        }).await;
    });
}
```

### In-game chat/voice overlay

The `InGameStore` and `GameOverlay` components auto-attach when the platform
detects the game is running via Magnetite. No additional SDK calls needed.

---

## Publishing

1. Implement your game logic extending `FpsGame`.
2. Push to GitHub and register your repo in the Magnetite developer dashboard.
3. The platform CI runs `cargo check --no-default-features` and the test suite.
4. For the browser build: `cargo build --target wasm32-unknown-unknown --features wasm`.
5. Submit for review; after approval your game appears in the marketplace.

---

## See also

- [Quickstart](./quickstart.md) — full publish workflow
- [Controllers & Gamepad Input](./controllers.md)
- [Graphics Tiers](./graphics-tiers.md) — Advanced3D tier details
- [Points Economy](./points-economy.md) — kill → points integration
- [Motorsport Starter Template](./motorsport-starter.md)
