<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>

# For developers

Get from zero to a running game in four steps, using the CLI path that
actually ships in this repository today. `docs/for-developers/quickstart.md`
in the repo also documents a "Web Studio" / "Developer Portal" / GitHub-App
publishing flow against a hosted `magnetite.gg` storefront — that describes a
central platform this project is moving *away* from (see
[Decentralization](#decentralization)) and is not reproduced here; treat it
as aspirational, not current, until it is reconciled with the no-central-cloud
redesign.

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.82+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `wasm32-wasip1` target | — | `rustup target add wasm32-wasip1` |
| `magnetite` CLI | — | `cargo install --path magnetite-cli` |

## Step 1 — Scaffold a game crate

```bash
magnetite new my-game
cd my-game
```

`magnetite new` takes a name only — there is no `--template` flag; it emits
one canonical scaffold. To start from a richer starter, copy
`game-templates/arcade/`, `game-templates/authoritative/`,
`game-templates/fps/`, or `game-templates/motorsport/` from this repository
instead.

## Step 2 — Implement `AuthoritativeGame`

```rust
use magnetite_sdk::authority::{
    AuthoritativeGame, MatchConfig, RejectReason, StepCtx, Tick,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

pub struct MyGame { /* your state here */ }

impl AuthoritativeGame for MyGame {
    type Snapshot = MySnapshot;   // full state — Serialize + DeserializeOwned + Clone
    type Delta    = MyDelta;      // compact diff — Serialize + DeserializeOwned
    type View     = MyView;       // per-player filtered view — Serialize only
    type Command  = MyCommand;    // validated command — Serialize + DeserializeOwned

    fn init(cfg: &MatchConfig) -> Self { /* create fresh match state */ }
    fn validate(&self, player: PlayerId, input: &Input, tick: Tick)
        -> Result<Vec<MyCommand>, RejectReason> { /* translate raw input into commands */ }
    fn step(&mut self, ctx: &mut StepCtx, commands: &[(PlayerId, MyCommand)]) { /* tick */ }
    fn snapshot(&self) -> MySnapshot { /* full state */ }
    fn restore(snap: &MySnapshot, cfg: &MatchConfig) -> Self { /* reconstruct */ }
    fn delta(&self, since: &MySnapshot) -> MyDelta { /* compact diff */ }
    fn view_for(&self, player: PlayerId) -> MyView { /* interest-filtered */ }
}
```

**Determinism contract (enforced by the replay verifier):**

- Use **only** `ctx.rng` (`DeterministicRng` / xoshiro256\*\*) for randomness.
- Never read wall-clock time in `step` or `validate`.
- Prefer `f32` with bounded per-tick deltas over `f64` accumulation.
- No I/O, threads, or blocking in `step` / `validate` — the Wasmtime sandbox
  strips those capabilities outright.

## Step 3 — Build to wasm

```bash
magnetite build
```

Runs `cargo build --release --target wasm32-wasip1 --features wasm` and
prints the artifact path.

## Step 4 — Run locally and play in the browser

```bash
magnetite dev
```

One command: builds the artifact, loads it into `magnetite-sandbox`
(Wasmtime, fuel-metered, memory-capped), starts `magnetite-runtime` in
`SingleRoom` topology, and prints a WebSocket URL and a play URL. Open the
play URL in any modern browser — `magnetite-web-client`, the lightweight
in-browser canvas client, connects over WebSocket and begins the tick loop.
Keyboard, mouse, and gamepad input is captured automatically.

## When you're ready for more players

`magnetite serve` / `magnetite node` takes an arbitrary box you own, measures
its own hardware, and advertises what it can hold — see
[Hosting a server](#hosting-a-server).

## Reference material

The following live under `docs/for-developers/` in the repository checkout
and go deeper than this page: `sdk.md` (full `magnetite-sdk` type reference),
`build-pipeline.md` (build/distribution pipeline), `points-economy.md` and
`marketplace.md` (the legacy backend's economic APIs — see
[Economy & Marketplace](#economy-marketplace)). The frozen interface
contract — `AuthoritativeGame`, `Topology`, `ReplayLog` — is
[The moat](#moat), the authoritative source for exact signatures.
