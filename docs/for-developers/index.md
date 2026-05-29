# For Developers

Build Rust games on Magnetite — from a weekend jam to a COD-scale AAA title.

## Guides

| Guide | Description |
|-------|-------------|
| [Quickstart](./quickstart.md) | Clone template → implement `GameLogic` → build WASM → register repo → publish |
| [SDK Reference](./sdk.md) | `magnetite-sdk` trait and type reference |
| [Build & Distribution Pipeline](./build-pipeline.md) | CI hooks, artifact management, versioning, play manifest |
| [API Reference](../api-reference/index.md) | All REST endpoints |

## One-minute summary

1. **Clone** `game-template/` — a Bevy + `magnetite-sdk` crate that compiles to WASM.
2. **Implement** the `GameLogic` trait: `new()`, `handle_input()`, `tick()`, `state()`.
3. **Build** to WASM: `bash build.sh` (wraps `cargo build --target wasm32-unknown-unknown`
   + `wasm-bindgen`).
4. **Register** your GitHub repo with the Magnetite GitHub App so the CI pipeline receives
   push events and tracks build status.
5. **Publish** by submitting for review via the Developer Portal; after approval your game
   appears in the marketplace.

The platform handles hosting, matchmaking, netcode, leaderboards, payments, and payouts.
You only write game logic.

## SDK at a glance

```rust
use magnetite_sdk::{GameLogic, GameMetadata, Input, GameState, PlayerId};

impl GameLogic for MyGame {
    fn new() -> Self { /* initialise */ }
    fn handle_input(&mut self, player: PlayerId, input: Input) -> Action { /* …*/ }
    fn tick(&mut self) { /* advance simulation */ }
    fn state(&self) -> &GameState { /* return reference to authoritative state */ }
    fn players(&self) -> Vec<PlayerId> { /* active players */ }
    fn metadata(&self) -> GameMetadata {
        GameMetadata { name: "my-game".into(), max_players: 4, tick_rate: 60 }
    }
}
```

See the [SDK Reference](./sdk.md) for all types.

## Developer earnings

- Games set their own pricing (entry fees, subscription shares).
- Platform fee: **15%**.
- Developer earnings: **85%**, paid out on request via `POST /api/v1/developer/payouts`.
- Track earnings in your developer dashboard: `GET /api/v1/developer/dashboard`.
