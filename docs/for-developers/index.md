# For Developers

Build Rust games on Magnetite — from a weekend jam to a COD-scale AAA title.

---

## Guides

| Guide | Description |
|-------|-------------|
| [Quickstart](./quickstart.md) | Clone template → implement `GameLogic` → build WASM → register → publish |
| [SDK Reference](./sdk.md) | `magnetite-sdk` trait and type reference |
| [Build & Distribution Pipeline](./build-pipeline.md) | CI hooks, artifact management, versioning, play manifest |
| [Controllers & Gamepad Input](./controllers.md) | `InputMap`, `GameAction`, `GamepadAxis`, binding editor |
| [Graphics Tiers](./graphics-tiers.md) | `Lite2D` / `Standard3D` / `Advanced3D` — declare your engine tier |
| [Points & Score Economy](./points-economy.md) | Ledger, seasons, award/spend, SDK integration |
| [Developer Marketplace](./marketplace.md) | In-game stores, items, purchase, entitlements, revenue split |
| [FPS Starter Template](./fps-starter.md) | `game-template-fps` — Bevy + rapier3d FPS, hitscan, gamepad |
| [Motorsport Starter Template](./motorsport-starter.md) | `game-template-motorsport` — vehicle physics, lap → points |
| [API Reference](../api-reference/index.md) | All REST endpoints |

---

## One-minute summary

1. **Clone** a starter template:
   - `game-template/` — 2D arcade, any genre
   - `game-template-fps/` — advanced 3D FPS
   - `game-template-motorsport/` — vehicle physics + racing
2. **Implement** the `GameLogic` trait: `new()`, `handle_input()`, `tick()`, `state()`.
3. **Build** to WASM: `bash build.sh` (wraps `cargo build --target wasm32-unknown-unknown`
   + `wasm-bindgen`).
4. **Register** your GitHub repo with the Magnetite GitHub App so the CI pipeline receives
   push events and tracks build status.
5. **Publish** by submitting for review via the Developer Portal; after approval your game
   appears in the marketplace.

The platform handles hosting, matchmaking, netcode, leaderboards, comms (chat + voice),
points economy, marketplace, payments, and payouts. You only write game logic.

---

## SDK at a glance

```rust
use magnetite_sdk::{export_game, game::{GameLogic, GameMetadata}, input::{Action, Input}, state::{GameState, PlayerId, Snapshot}};

struct MyGame { state: GameState }

impl GameLogic for MyGame {
    fn new() -> Self { MyGame { state: GameState::default() } }
    fn handle_input(&mut self, _pid: PlayerId, _input: Input) -> Action { Action::None }
    fn tick(&mut self) { self.state.tick += 1; }
    fn state(&self) -> &GameState { &self.state }
    fn players(&self) -> Vec<PlayerId> { vec![] }
    fn metadata(&self) -> GameMetadata { GameMetadata::default() }
    fn snapshot(&self) -> Snapshot { Snapshot::new(self.state.tick, self.state.clone()) }
    fn restore(&mut self, snap: Snapshot) { self.state = snap.state; }
}

export_game!(MyGame);
```

See the [SDK Reference](./sdk.md) for all types.

---

## Platform services (callable from in-game code)

| Service | SDK module | Description |
|---------|-----------|-------------|
| Chat + voice | `platform::comms` | Real-time text chat and WebRTC voice in any lobby or community |
| Streaming | `platform::streaming` | Go live, manage stream sessions, RTMP egress config |
| Points / XP | `platform::points` | Award, spend, and query the platform-wide points ledger |
| Marketplace | `platform::marketplace` | Check entitlements, initiate server-side purchases |
| Cloud saves | `platform::cloud_save` | Per-user, per-game save slots with conflict resolution |

---

## Developer earnings

- Platform fee: **15%** on subscription revenue.
- Developer earnings: **85%**, paid out on request via `POST /api/v1/developer/payouts`.
- In-game store USDC purchases: **70 % developer / 30 % platform**.
- Track earnings in your developer dashboard: `GET /api/v1/developer/dashboard`.
