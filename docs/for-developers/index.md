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
| [Developer Marketplace](./marketplace.md) | In-game stores, items, non-custodial purchase, entitlements |
| [FPS Starter Template](./fps-starter.md) | `game-template-fps` — Bevy + rapier3d FPS, hitscan, gamepad |
| [Motorsport Starter Template](./motorsport-starter.md) | `game-template-motorsport` — vehicle physics, lap → points |
| [API Reference](../api-reference/index.md) | All REST endpoints |
| [Hosting a server](../hosting-a-server.md) | `magnetite node`, capacity-elastic hosting, discovery |
| [Payments](../payments.md) | Non-custodial checkout and signed receipts |

---

## One-minute summary

1. **Clone** a starter template:
   - `game-templates/arcade/` — 2D arcade, any genre
   - `game-templates/fps/` — advanced 3D FPS
   - `game-templates/motorsport/` — vehicle physics + racing
2. **Implement** the `GameLogic` trait: `new()`, `handle_input()`, `tick()`, `state()`.
3. **Build** to WASM: `bash build.sh` (wraps `cargo build --target wasm32-unknown-unknown`
   + `wasm-bindgen`).
4. **Register** your GitHub repo with the Magnetite GitHub App so the CI pipeline receives
   push events and tracks build status.
5. **Publish** by submitting for review via the Developer Portal; after approval your game
   appears in the marketplace.

The platform handles the game runtime (server-authoritative sim, WASM sandbox,
deterministic replay), hosting on capacity-elastic nodes, discovery,
matchmaking, netcode, leaderboards, the points economy, and non-custodial
checkout. Comms is a pluggable provider seam, not something Magnetite operates.
You only write game logic.

You can also skip the platform entirely: `magnetite dev` runs your game with no
backend, and `magnetite node` hosts it on your own hardware.

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
| Chat + voice | `platform::comms` | Rooms via the `CommsProvider` seam — `builtin` by default, or Matrix / Jitsi / LiveKit if the operator configures one |
| Streaming | `platform::streaming` | Go live and manage stream sessions. Media hosting is per-node and optional (Owncast provider, or an opt-in MediaMTX profile) |
| Points / XP | `platform::points` | Award, spend, and query the platform-wide points ledger |
| Marketplace | `platform::marketplace` | Check entitlements, initiate server-side purchases |
| Cloud saves | `platform::cloud_save` | Per-user, per-game save slots with conflict resolution |

---

## Developer earnings

**You keep the whole subtotal and there is nothing to withdraw.**

- A store purchase is an atomic wallet→wallet checkout: the buyer's wallet pays
  yours in the same transaction that mints the signed receipt. Magnetite is not
  in the path.
- Protocol fee: `PROTOCOL_FEE_BPS`, **default 0**, charged on top of the
  subtotal — never deducted from your share.
- Link a wallet with `POST /api/v1/wallet/link` so checkout has a payee.
- Track settlement: `GET /api/v1/developer/earnings` (sums non-voided
  receipts; `pending_payout` is always `0` because nothing is held) and
  `GET /api/v1/developer/dashboard`.

The 15/85 subscription split, the 70/30 store split, and
`POST /api/v1/developer/payouts` were **deleted**. See
[Payments](../payments.md).
