# Magnetite Moat — Architecture Overview

> This document is a readable guide to how the crates compose.
> For the canonical frozen interface definitions and wave plan, see
> [docs/MOAT-ARCHITECTURE.md](../MOAT-ARCHITECTURE.md).
> For the one-command developer walkthrough, see [quickstart.md](quickstart.md).

---

## The moat in one sentence

Write one Rust game implementing `AuthoritativeGame`; the platform runs it
from a 2-player room to COD-scale — server-authoritative, sandboxed,
anti-cheat by construction, deployable with `magnetite deploy`.

---

## Crate map

```
magnetite-sdk           (existing, extended)
    └─ authority.rs     ← frozen trait + executor + replay API
    └─ protocol.rs      ← netcode frames (ClientNet / ServerNet)
    └─ input.rs / state.rs / networking.rs / graphics.rs / platform/

magnetite-runtime/      (new)  — game-server host (tokio + WebSocket)
magnetite-sandbox/      (new)  — Wasmtime WasmExecutor
magnetite-anticheat/    (new)  — composable validators + replay verifier
magnetite-cli/          (new)  — `magnetite` binary

game-templates/authoritative/   — reference arena shooter
game-client-bevy/              — reference Bevy client
magnetite-e2e/                 — integration test suite
```

---

## Layer diagram

```
┌─────────────────────────────────────────────────────────────────┐
│  Developer game crate  (implements AuthoritativeGame)           │
│  my-game/src/lib.rs   ← only file you write                     │
└────────────────────────────┬────────────────────────────────────┘
                             │  AuthoritativeGame trait
                             ▼
┌────────────────────────────────────────────────────────────────┐
│  magnetite-sdk::authority                                      │
│  ┌──────────────────┐  ┌──────────────────┐  ┌─────────────┐  │
│  │ NativeExecutor   │  │  DeterministicRng │  │ ReplayLog / │  │
│  │ (in-proc)        │  │  (xoshiro256**)   │  │ verify_replay│  │
│  └──────────────────┘  └──────────────────┘  └─────────────┘  │
│  GameExecutor trait  ← runtime-facing abstraction              │
└────────┬───────────────────────────────────────────────────────┘
         │ GameExecutor
         ├──────────────────────────────────────┐
         │                                      │
         ▼                                      ▼
┌─────────────────────┐              ┌─────────────────────────┐
│ magnetite-sandbox   │              │ magnetite-runtime        │
│ WasmExecutor        │              │ GameServer               │
│ • Wasmtime          │◄─── loads ───│ • tokio + WebSocket      │
│ • fuel metered      │   game.wasm  │ • TickScheduler          │
│ • memory capped     │              │ • ConnectionManager      │
│ • epoch timeout     │              │ • ShardManager           │
│ • no clock/rng/io   │              │ • anticheat pipeline     │
└─────────────────────┘              └────────────┬────────────┘
                                                  │ WS frames
                                                  ▼
                                     ┌────────────────────────┐
                                     │ magnetite-anticheat     │
                                     │ ValidatorChain          │
                                     │ TrustScoreMap           │
                                     │ replay verifier         │
                                     └────────────────────────┘
                                                  │
                                                  ▼
                                     ┌────────────────────────┐
                                     │ game-client-bevy        │
                                     │ NetPlugin (WS)          │
                                     │ PredictionPlugin        │
                                     │ ArenaRenderPlugin       │
                                     └────────────────────────┘
```

---

## `magnetite-sdk::authority` — the frozen contract

Everything else depends on these types. They are defined in
`backend/magnetite-sdk/src/authority.rs` and must not change without a
recorded decision in `docs/project/DECISIONS.md`.

### `AuthoritativeGame`

The trait every game implements. The runtime calls `validate → step →
snapshot/delta/view_for` every tick.

```rust
pub trait AuthoritativeGame: Send + 'static {
    type Snapshot: Serialize + DeserializeOwned + Clone;
    type Delta:    Serialize + DeserializeOwned;
    type View:     Serialize;
    type Command:  Serialize + DeserializeOwned;

    fn init(cfg: &MatchConfig) -> Self;
    fn validate(&self, player: PlayerId, input: &Input, tick: Tick)
        -> Result<Vec<Self::Command>, RejectReason>;
    fn step(&mut self, ctx: &mut StepCtx, commands: &[(PlayerId, Self::Command)]);
    fn snapshot(&self) -> Self::Snapshot;
    fn restore(snap: &Self::Snapshot, cfg: &MatchConfig) -> Self;
    fn delta(&self, since: &Self::Snapshot) -> Self::Delta;
    fn view_for(&self, player: PlayerId) -> Self::View;
    fn on_join(&mut self, _p: PlayerId) {}
    fn on_leave(&mut self, _p: PlayerId) {}
}
```

### `GameExecutor` — runtime-facing abstraction

`NativeExecutor<G>` and `WasmExecutor` both implement this trait so the
runtime is executor-agnostic.

```rust
pub trait GameExecutor: Send {
    fn step(&mut self, tick: Tick, inputs: &[(PlayerId, Input)]) -> StepOutput;
    fn snapshot(&self) -> Vec<u8>;
    fn restore(&mut self, bytes: &[u8]);
    fn view_for(&self, player: PlayerId) -> Vec<u8>;
    fn delta_since(&self, snapshot_bytes: &[u8]) -> Vec<u8>;
}
```

`StepOutput { rejects: Vec<(PlayerId, RejectReason)>, state_hash: u64 }`
is returned every tick. The runtime broadcasts rejects as `ServerNet::Reject`
and records `state_hash` in the `ReplayLog`.

### `DeterministicRng`

xoshiro256** seeded from `MatchConfig::seed` (set by the matchmaker from a
CSPRNG). The only source of randomness games may use. Same seed → identical
sequence on every platform and every Rust version.

### `ReplayLog` and `verify_replay`

```rust
pub struct ReplayLog {
    pub config: MatchConfig,
    pub frames: Vec<(Tick, Vec<(PlayerId, Input)>)>,
    pub state_hashes: Vec<(Tick, u64)>,
}

pub fn verify_replay<G: AuthoritativeGame>(log: &ReplayLog) -> ReplayVerdict;
```

`verify_replay` re-creates the game from `log.config`, replays all frames,
and compares `state_hash` tick-by-tick. The result is either
`ReplayVerdict::Clean` or `ReplayVerdict::Divergence { tick, expected, got }`.

State hashes are computed via FNV-1a 64-bit over the canonical JSON
serialisation of the snapshot (deterministic across platforms because
`serde_json` serialises struct fields in declaration order).

### `Topology` and `MatchConfig`

```rust
pub enum Topology {
    SingleRoom,
    Dedicated  { tick_hz: u16 },
    Sharded    { tick_hz: u16, cell_size: f32, max_per_shard: u32 },
}

impl MatchConfig {
    // Picks topology by player count:
    //  1–16  → SingleRoom
    // 17–256 → Dedicated { tick_hz: 60 }
    // 257+   → Sharded { tick_hz: 20, cell_size: 500.0, max_per_shard: 64 }
    pub fn auto(max_players: u32) -> Self;
}
```

---

## `magnetite-runtime` — the authoritative game-server host

Source: `magnetite-runtime/src/`

| Module | Role |
|---|---|
| `server.rs` | `GameServer` — entry points, WebSocket accept loop |
| `tick.rs` | `TickScheduler` — per-tick input collection → executor step → broadcast |
| `connection.rs` | `ConnectionManager` — per-player WS state |
| `shard.rs` | `ShardManager` — topology dispatch and spatial routing |

### Entry points

```rust
// Native executor (trusted game compiled in):
GameServer::serve(executor, config).await

// Wasm executor (sandboxed game.wasm):
GameServer::serve_wasm(wasm_path, limits, config).await

// Generic (bring your own GameExecutor):
GameServer::with_executor(Box<dyn GameExecutor>, config).await
```

`GameServerConfig` fields:

```rust
pub struct GameServerConfig {
    pub bind_addr: String,       // e.g. "127.0.0.1:9000"
    pub match_config: MatchConfig,
    pub anticheat: Option<Anticheat>, // None → SDK defaults (RateLimit + InputSchema)
}
```

### Per-tick pipeline

```
TickScheduler::tick()
  1. Collect InputFrames from all connections
  2. anticheat.inspect(player, input, tick) per frame
      → Decision::Allow / Reject / Kick / Ban
  3. GameExecutor::step(tick, &accepted_inputs) → StepOutput
  4. ReplayLog::record(tick, inputs, state_hash)
  5. Every snapshot_every ticks: GameExecutor::snapshot() → ServerNet::Snapshot
  6. Every tick: GameExecutor::delta_since(last_snap) → ServerNet::Delta
  7. GameExecutor::view_for(player) → per-player ServerNet::Delta payload
  8. Broadcast Ack/Reject/Delta/Snapshot to each connection
```

### Default anticheat pipeline

When `GameServerConfig::anticheat` is `None` the runtime builds:

```rust
ValidatorChain::new()
    .add(RateLimit::new(120))
    .add(InputSchema::default())
```

Pass `Some(anticheat)` to extend with `AimbotSnap`, `PositionTeleport`,
`FireRateCooldown`, `InputFlood` from `magnetite-anticheat`, or your own
`Validator` implementations.

---

## `magnetite-sandbox` — the Wasmtime sandbox

Source: `magnetite-sandbox/src/`

`WasmExecutor` implements `GameExecutor` by loading a `wasm32-wasip1`
module and calling into its `mag_*` exports.

### Security model

- **No WASI clock or RNG** — the host does not expose `clock_time_get` or
  `random_get`. Game code cannot read wall time or get OS-seeded random
  numbers.
- **Fuel metering** — Wasmtime `Config::consume_fuel(true)`. Each
  `mag_step` call is given `LimitsConfig::fuel_per_step` units (default:
  10 million). Running out of fuel traps and returns an error.
- **Memory cap** — `StoreLimits` implements `ResourceLimiter`. Guest
  `memory.grow` is denied if it would exceed `max_memory_bytes` (default:
  64 MiB).
- **Epoch interruption** — a background thread increments the engine epoch
  every `epoch_tick_ms` (default: 5 ms). The store kills a step that exceeds
  `max_epochs_per_step` epochs (default: 2, giving a 10 ms wall-clock budget
  per step).
- **No filesystem or network** — WASI is not linked; the only host function
  the guest sees is the bump allocator (`mag_alloc` / `mag_free`).

### `LimitsConfig` defaults

| Field | Default | Meaning |
|---|---|---|
| `fuel_per_step` | 10,000,000 | ~10M Wasm instructions per step |
| `max_memory_bytes` | 67,108,864 (64 MiB) | Guest linear memory cap |
| `max_epochs_per_step` | 2 | Epoch count before step is killed |
| `epoch_tick_ms` | 5 | Epoch tick period → 10 ms wall budget |

### Wasm ABI

The sandbox calls these exports on the guest module:

```
mag_alloc(len: u32) -> u32         // bump alloc; host uses for write buffers
mag_free(ptr: u32, len: u32)       // no-op in the reference implementation
mag_init(cfg_ptr: u32, cfg_len: u32)
mag_step(inputs_ptr: u32, inputs_len: u32) -> u32
mag_snapshot() -> u32
mag_restore(ptr: u32, len: u32)
mag_view(player_id: u64) -> u32
```

All `-> u32` returns are pointers to a 4-byte LE length prefix followed by
a JSON payload in the guest's linear memory. The sandbox reads the length,
copies the bytes out, and deserialises.

See `game-templates/authoritative/src/wasm_abi.rs` for the reference
implementation.

---

## `magnetite-anticheat` — composable validators

Source: `magnetite-anticheat/src/`

### Pipeline

```
Client Input
    │
    ▼
ValidatorChain (sdk built-ins + anticheat built-ins)
    │ Ok / Err(RejectReason)
    ▼
Anticheat::inspect(player, input, tick) -> Decision
    │ Allow / Reject / Kick / Ban
    ▼
TrustScoreMap — per-player decay + escalation
    │ emits AntiCheatEvent
    ▼
GameExecutor::step (receives only allowed inputs)
```

### Built-in validators

**SDK (`magnetite_sdk::authority`)**

| Validator | Checks |
|---|---|
| `RateLimit::new(max_per_sec)` | Input frames per second per player |
| `MovementVelocity::new(max)` | Mouse delta magnitude |
| `ActionCooldown::new(action, ticks)` | Minimum tick gap for `"attack"`, `"jump"`, etc. |
| `InputSchema::default()` | Non-finite mouse deltas/positions |

**`magnetite-anticheat`**

| Validator | Checks |
|---|---|
| `AimbotSnap::new(max_deg)` | Angular velocity jump (aimbot detection) |
| `PositionTeleport::new(max)` | Claimed position jump |
| `FireRateCooldown::new(ticks)` | Shot cadence |
| `InputFlood::new(max)` | Input volume spike |

### Trust escalation

`TrustScoreMap` tracks a score per player. Each rejection degrades the score
by a configurable amount; score recovers over time. Thresholds trigger:
`Decision::Warn` → `Decision::Kick` → `Decision::Ban`.

---

## `magnetite-cli` — the `magnetite` binary

Source: `magnetite-cli/src/main.rs`

| Command | What it does |
|---|---|
| `magnetite new <name>` | Scaffold a crate with `Cargo.toml` + `src/lib.rs` stub |
| `magnetite build` | `cargo build --release --target wasm32-wasip1 --features wasm` |
| `magnetite dev` | Build → `WasmExecutor` → `GameServer::serve_wasm` → print WS URL |
| `magnetite deploy` | Build → `POST /api/v1/distribution/<game_id>/versions` |

`magnetite dev` wires all the crates together without any manual plumbing:

```rust
// Inside cmd_dev:
let match_cfg = MatchConfig::auto(max_players);
let server_cfg = GameServerConfig { bind_addr, match_config: match_cfg, anticheat: None };
let limits = LimitsConfig::default();
GameServer::serve_wasm(wasm_path, limits, server_cfg).await
```

---

## Netcode protocol

Defined in `magnetite_sdk::protocol`.

```
// client → server
ClientNet::InputFrame { seq: u32, tick: Tick, input: Input }

// server → client
ServerNet::Welcome { player_id, config }
ServerNet::Snapshot { tick, full: Vec<u8> }     // every MatchConfig.snapshot_every ticks
ServerNet::Delta    { tick, since_tick, diff: Vec<u8> }  // per tick, interest-filtered
ServerNet::Ack      { seq, tick }               // for client-side prediction reconciliation
ServerNet::Reject   { seq, reason }
```

The client uses `magnetite_sdk::networking::PredictionBuffer` to:
1. Buffer outgoing `InputFrame`s with their sequence numbers.
2. Predict locally and render immediately.
3. Acknowledge confirmed frames on `Ack { seq }`.
4. Re-simulate unacked frames against the authoritative state on
   `Snapshot` or after a state correction.

---

## Reference implementations

| Purpose | Location |
|---|---|
| Arena shooter (full `AuthoritativeGame`) | `game-templates/authoritative/src/game.rs` |
| Wasm ABI exports | `game-templates/authoritative/src/wasm_abi.rs` |
| Bevy client app | `game-client-bevy/src/app.rs` |
| Client prediction/reconciliation | `game-client-bevy/src/prediction.rs` |
| WebSocket transport | `game-client-bevy/src/net.rs` |
| End-to-end test suite | `magnetite-e2e/` |

---

## Further reading

- [quickstart.md](quickstart.md) — five-minute developer guide
- [docs/MOAT-ARCHITECTURE.md](../MOAT-ARCHITECTURE.md) — frozen interface
  definitions and wave plan (source of truth for all crate contracts)
