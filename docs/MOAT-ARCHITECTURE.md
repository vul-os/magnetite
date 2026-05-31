# Magnetite Moat — The Scale / Sandbox / Anti-Cheat Primitive

> This is the genuinely novel core: **write one Rust game; the platform runs it from a 2-player room to
> COD-scale, server-authoritative, sandboxed, with anti-cheat as a first-class guarantee, deployable from
> GitHub in minutes.** Every agent building this MUST conform to the frozen interfaces below — they are the
> contract between the SDK, the runtime, the sandbox, the anti-cheat layer, and the CLI. Build on the existing
> `magnetite-sdk` (which already has `GameLogic`, `state` types, `Snapshot`, `protocol`, `InterestManager`,
> `TickLoop`, `PredictionBuffer`, `graphics` tiers, `platform::*` clients). Do NOT break those.

## Why this is the moat
Nakama/PlayFab give you services; Roblox gives you a closed platform. **Nobody gives an open, Rust-native
"same code, jam-to-AAA, authoritative + sandboxed + anti-cheat + one-command-deploy" primitive.** That's what
we build here. The three differentiators are ONE system:
1. **Scale primitive** — identical game code, topology auto-selected (SingleRoom → Dedicated → Sharded).
2. **Sandbox** — untrusted game logic runs in Wasmtime with fuel/memory/time limits; deterministic.
3. **Anti-cheat** — server-authoritative by construction + deterministic replay re-simulation verification.
Plus the **pipeline**: `magnetite new|build|dev|deploy` (GitHub → live multiplayer).

## Crate layout (new crates are standalone dirs; depend on `magnetite-sdk` via `path`)
- `backend/magnetite-sdk` (EXISTING) — gains a new module **`authority`** holding the frozen traits/types below
  + `NativeExecutor` + `DeterministicRng` + `ReplayLog`/`verify_replay` + `Topology`/`MatchConfig`; extend
  `protocol` with the netcode frames. **Single owner.**
- `magnetite-runtime/` (NEW) — the authoritative game-server **host**: tick loop, connection mgmt (tokio +
  WebSocket), per-tick input collection → executor step → interest-filtered delta broadcast + periodic
  snapshot; topology impls. Depends on sdk only (uses `GameExecutor`).
- `magnetite-sandbox/` (NEW) — **Wasmtime** host implementing `GameExecutor` as `WasmExecutor` (load a game
  compiled to `wasm32-wasip1`, run with limits, deterministic). Depends on sdk + `wasmtime`.
- `magnetite-anticheat/` (NEW) — composable `Validator`s + the deterministic replay verifier + flag/trust
  aggregation. Depends on sdk only.
- `magnetite-cli/` (NEW) — the `magnetite` binary: `new|build|dev|deploy`. (N1: `new` + `build` only — they
  just scaffold + shell `cargo`; `dev`/`deploy` come in N2 once runtime+sandbox exist.)
- `game-template-authoritative/` (NEW) — reference game implementing `AuthoritativeGame` (a small top-down
  arena shooter) used by everything as the canonical example + integration target.

## FROZEN INTERFACES (`magnetite-sdk::authority`)
Exact signatures — implement to these names so the crates compose. Adjust ONLY with a recorded reason.

```rust
pub type Tick = u64;

/// Deterministic per-match RNG (seeded). The ONLY source of randomness a game may use.
pub struct DeterministicRng { /* xoshiro/pcg seeded from MatchConfig.seed */ }
impl DeterministicRng { pub fn next_u64(&mut self) -> u64; pub fn next_f32(&mut self) -> f32; }

pub struct StepCtx<'a> { pub tick: Tick, pub dt_ms: u32, pub rng: &'a mut DeterministicRng }

#[derive(Clone)] pub enum RejectReason { RateLimited, OutOfBounds, IllegalAction(String), StaleInput, Unauthorized }

/// The dev writes ONE of these. MUST be deterministic: same (state, ordered commands, tick, seed) => same
/// result. No wall clock, no std rng, no I/O inside step/validate — use StepCtx only. This determinism is what
/// makes replay verification + sharding + sandboxing possible.
pub trait AuthoritativeGame: Send + 'static {
    type Snapshot: serde::Serialize + serde::de::DeserializeOwned + Clone;
    type Delta:    serde::Serialize + serde::de::DeserializeOwned;
    type View:     serde::Serialize;                 // per-player, interest-filtered (anti-wallhack)
    type Command:  serde::Serialize + serde::de::DeserializeOwned;

    fn init(cfg: &MatchConfig) -> Self;
    /// Turn an untrusted client Input into 0+ authoritative Commands. NEVER trust client-sent state.
    fn validate(&self, player: crate::PlayerId, input: &crate::Input, tick: Tick)
        -> Result<Vec<Self::Command>, RejectReason>;
    /// Deterministic state transition over this tick's ordered (player, command) list.
    fn step(&mut self, ctx: &mut StepCtx, commands: &[(crate::PlayerId, Self::Command)]);
    fn snapshot(&self) -> Self::Snapshot;
    fn restore(snap: &Self::Snapshot, cfg: &MatchConfig) -> Self;
    fn delta(&self, since: &Self::Snapshot) -> Self::Delta;
    fn view_for(&self, player: crate::PlayerId) -> Self::View;   // drives interest mgmt + bandwidth
    fn on_join(&mut self, _p: crate::PlayerId) {}
    fn on_leave(&mut self, _p: crate::PlayerId) {}
}

/// Scale primitive — chosen by config/load; game code is IDENTICAL across all three.
pub enum Topology {
    SingleRoom,                                            // 1 process, broadcast-all (≲16)
    Dedicated  { tick_hz: u16 },                           // authoritative + interest snapshots (≲256)
    Sharded    { tick_hz: u16, cell_size: f32, max_per_shard: u32 },  // spatial shards + handoff (AAA)
}
pub struct MatchConfig {
    pub topology: Topology, pub max_players: u32, pub tick_hz: u16, pub seed: u64,
    pub snapshot_every: u16,                               // full snapshot cadence in ticks
}
impl MatchConfig { pub fn auto(max_players: u32) -> Self; } // escalates topology by player count

/// Runtime-facing execution abstraction. Same game runs native (in-proc) or sandboxed (wasm).
pub trait GameExecutor: Send {
    fn step(&mut self, tick: Tick, inputs: &[(crate::PlayerId, crate::Input)]) -> StepOutput;
    fn snapshot(&self) -> Vec<u8>;                         // serialized Snapshot
    fn restore(&mut self, bytes: &[u8]);
    fn view_for(&self, player: crate::PlayerId) -> Vec<u8>;
    fn delta_since(&self, snapshot_bytes: &[u8]) -> Vec<u8>;
}
pub struct StepOutput { pub rejects: Vec<(crate::PlayerId, RejectReason)>, pub state_hash: u64 }

/// In-proc executor (sdk provides). Sandbox provides WasmExecutor implementing the same trait.
pub struct NativeExecutor<G: AuthoritativeGame> { /* game + config + rng */ }
impl<G: AuthoritativeGame> NativeExecutor<G> { pub fn new(cfg: MatchConfig) -> Self; }
// impl<G: AuthoritativeGame> GameExecutor for NativeExecutor<G> { .. }

/// First-class anti-cheat — server-side, composable.
pub trait Validator: Send {
    fn check(&mut self, player: crate::PlayerId, input: &crate::Input, tick: Tick) -> Result<(), RejectReason>;
}
// sdk built-ins: RateLimit{max_per_sec}, MovementVelocity{max}, ActionCooldown{..}, InputSchema.

/// Deterministic replay = tamper detection + match replays. Runtime records inputs; verifier re-simulates.
pub struct ReplayLog { pub config: MatchConfig, pub frames: Vec<(Tick, Vec<(crate::PlayerId, crate::Input)>)>,
                       pub state_hashes: Vec<(Tick, u64)> }
pub enum ReplayVerdict { Clean, Divergence { tick: Tick, expected: u64, got: u64 } }
pub fn verify_replay<G: AuthoritativeGame>(log: &ReplayLog) -> ReplayVerdict;
```

### Netcode frames (extend `magnetite-sdk::protocol`)
```rust
// client → server
ClientNet::InputFrame { seq: u32, tick: Tick, input: Input }
// server → client
ServerNet::Welcome { player_id, config }
ServerNet::Snapshot { tick, full: bytes }            // every MatchConfig.snapshot_every ticks
ServerNet::Delta    { tick, since_tick, diff: bytes }// per tick, interest-filtered per player
ServerNet::Ack      { seq, tick }                    // for client prediction reconciliation
ServerNet::Reject   { seq, reason }
```
Client uses the existing `PredictionBuffer` to predict locally and reconcile on `Ack`/`Snapshot`.

### Sandbox ABI (`magnetite-sandbox`, target `wasm32-wasip1`)
Guest game module exports (host calls these); host provides a bump allocator import + deterministic env
(NO wall clock, NO real rng, fuel-metered, memory-capped, epoch-interrupt timeout per step):
```
mag_alloc(len)->ptr ; mag_free(ptr,len)
mag_init(cfg_ptr, cfg_len)
mag_step(inputs_ptr, inputs_len) -> packed StepOutput ptr      // returns rejects + state_hash
mag_snapshot() -> ptr ; mag_restore(ptr, len)
mag_view(player_id) -> ptr
```
`WasmExecutor` wraps a `wasmtime::Store` with `Config::consume_fuel(true)` + `epoch_interruption(true)` +
`StoreLimits` (memory). Determinism: fixed tick, seeded rng passed in, no `wasi` clock/random.

### Pipeline / CLI
- `magnetite new <name>` → scaffold a crate implementing `AuthoritativeGame` (+ optional Bevy client).
- `magnetite build` → `cargo build --release --target wasm32-wasip1` (server logic) [+ `wasm32-unknown-unknown`
  for the Bevy client]. Produces `game.wasm`.
- `magnetite dev` → build → load `game.wasm` into `magnetite-sandbox` → run `magnetite-runtime` (SingleRoom) →
  serve WS → print a connect URL. **One command, local, real.**
- `magnetite deploy` → build → register the artifact via the backend distribution API → request a runtime
  instance. (Local/self-hosted runner now; cloud auto-scaled fleet = Bucket D.)

## Determinism rules (enforced + documented)
Games MUST: use only `StepCtx.rng` for randomness; avoid `f64` accumulation across ticks where cross-platform
determinism matters (prefer fixed-point or documented tolerance); never read wall clock in `step`/`validate`.
The replay verifier asserts `state_hash` equality tick-by-tick; divergence = tamper or nondeterminism bug.

## Wave plan
- **N1 (foundation, 5 agents, disjoint crates, build against sdk only):** (1) sdk `authority` module + Native
  Executor + Replay + protocol frames + tests; (2) `magnetite-runtime` host (SingleRoom + Dedicated, Sharded
  seam); (3) `magnetite-sandbox` Wasmtime `WasmExecutor`; (4) `magnetite-anticheat` validators + replay
  verifier; (5) `magnetite-cli` (`new`+`build`) + this doc's example wiring + `game-template-authoritative`.
- **N2 (integration):** runtime uses WasmExecutor + validators; CLI `dev`/`deploy`; Bevy client example with
  prediction/reconciliation; backend distribution ↔ runtime provisioning; end-to-end demo + load test;
  Sharded topology fleshed out (single-node multi-shard + handoff).
- Verify each crate: `cargo check` 0 warnings + `cargo fmt --check` + `cargo test`. New crates use
  `cargo check --no-default-features` if they pull heavy/native deps. Wasmtime build may be slow — allow it.
