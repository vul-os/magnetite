# Build & run your first authoritative Rust game in 5 minutes

This guide walks through the complete Magnetite pipeline:
`magnetite new` → implement `AuthoritativeGame` → `magnetite build`
→ `magnetite dev` (live sandboxed match) → connect the Bevy client
→ `magnetite deploy`.

---

## Prerequisites

- Rust stable toolchain (1.75+)
- `wasm32-wasip1` target: `rustup target add wasm32-wasip1`
- `magnetite` CLI installed (from the workspace): `cargo install --path magnetite-cli`

---

## Step 1 — Scaffold a new game crate

```bash
magnetite new my-arena
cd my-arena
```

`magnetite new` creates a ready-to-build crate:

```
my-arena/
  Cargo.toml    # cdylib + rlib; [features] wasm = []
  src/
    lib.rs      # AuthoritativeGame stub + wasm ABI comment
```

The generated `Cargo.toml` enables both `cdylib` (for the Wasm sandbox
artifact) and `rlib` (for unit tests / native linking) and declares a
`wasm` feature used to gate the `mag_*` ABI exports.

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = []
wasm = []
```

---

## Step 2 — Implement `AuthoritativeGame`

Open `src/lib.rs`. The scaffold already imports everything you need.
Replace the stub types with your real game data, then implement the five
required methods.

### Trait signature (from `magnetite_sdk::authority`)

```rust
pub trait AuthoritativeGame: Send + 'static {
    type Snapshot: serde::Serialize + serde::de::DeserializeOwned + Clone;
    type Delta:    serde::Serialize + serde::de::DeserializeOwned;
    type View:     serde::Serialize;          // per-player, interest-filtered
    type Command:  serde::Serialize + serde::de::DeserializeOwned;

    fn init(cfg: &MatchConfig) -> Self;
    fn validate(&self, player: PlayerId, input: &Input, tick: Tick)
        -> Result<Vec<Self::Command>, RejectReason>;
    fn step(&mut self, ctx: &mut StepCtx, commands: &[(PlayerId, Self::Command)]);
    fn snapshot(&self) -> Self::Snapshot;
    fn restore(snap: &Self::Snapshot, cfg: &MatchConfig) -> Self;
    fn delta(&self, since: &Self::Snapshot) -> Self::Delta;
    fn view_for(&self, player: PlayerId) -> Self::View;

    // Optional lifecycle hooks:
    fn on_join(&mut self, _p: PlayerId) {}
    fn on_leave(&mut self, _p: PlayerId) {}
}
```

### What each method does

| Method | Role |
|---|---|
| `init` | Create a fresh match state from `MatchConfig` |
| `on_join` / `on_leave` | React to player join / disconnect (spawn, cleanup) |
| `validate` | Translate untrusted raw `Input` → 0+ authoritative `Command`s, or return `Err(RejectReason)` |
| `step` | Advance state by one tick given the ordered (player, command) list; this is your game loop |
| `snapshot` / `restore` | Full state serialisation (for replay, shard handoff, periodic broadcast) |
| `delta` | Compact diff since a prior snapshot (broadcast every tick) |
| `view_for` | Per-player interest-filtered view — **only this is sent to that player** |

### Minimal working example

```rust
use magnetite_sdk::authority::{
    AuthoritativeGame, MatchConfig, RejectReason, StepCtx, Tick,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct MySnapshot { pub tick: u64 }

#[derive(Serialize, Deserialize)]
pub struct MyDelta { pub tick: u64 }

#[derive(Serialize)]
pub struct MyView { pub tick: u64 }

#[derive(Serialize, Deserialize)]
pub enum MyCommand { Move { dx: f32, dy: f32 } }

pub struct MyGame { tick: u64 }

impl AuthoritativeGame for MyGame {
    type Snapshot = MySnapshot;
    type Delta    = MyDelta;
    type View     = MyView;
    type Command  = MyCommand;

    fn init(_cfg: &MatchConfig) -> Self { Self { tick: 0 } }

    fn validate(&self, _player: PlayerId, input: &Input, _tick: Tick)
        -> Result<Vec<MyCommand>, RejectReason>
    {
        // Translate raw input into clean commands; reject suspicious inputs here.
        let dx = input.mouse.delta_x as f32;
        let dy = input.mouse.delta_y as f32;
        Ok(vec![MyCommand::Move { dx, dy }])
    }

    fn step(&mut self, ctx: &mut StepCtx, _commands: &[(PlayerId, MyCommand)]) {
        // ONLY use ctx.rng for randomness — never std::random or thread_rng.
        self.tick = ctx.tick;
    }

    fn snapshot(&self) -> MySnapshot { MySnapshot { tick: self.tick } }
    fn restore(s: &MySnapshot, _cfg: &MatchConfig) -> Self { Self { tick: s.tick } }
    fn delta(&self, since: &MySnapshot) -> MyDelta {
        MyDelta { tick: self.tick.saturating_sub(since.tick) }
    }
    fn view_for(&self, _player: PlayerId) -> MyView { MyView { tick: self.tick } }
}
```

---

## Determinism rules (enforced and checked at runtime)

The runtime records every input and the state hash after each tick in a
`ReplayLog`. The anti-cheat service re-simulates that log with
`verify_replay` and flags any divergence. Your game must not violate these
rules or the verifier will fire `ReplayVerdict::Divergence`.

**You must:**

1. **Use only `StepCtx::rng` for randomness.** Never call `rand::thread_rng`,
   `std::random`, or any OS RNG inside `step` or `validate`.
   `DeterministicRng` implements xoshiro256** seeded from `MatchConfig::seed`.
   ```rust
   let random_value = ctx.rng.next_f32(); // always deterministic
   ```

2. **Never read wall-clock time in `step` or `validate`.** Use `ctx.tick`
   and `ctx.dt_ms` for time. Wall-clock reads are allowed only in
   out-of-simulation code (e.g. `RateLimit` validator operates on message
   arrival, not game time).

3. **Prefer fixed-point arithmetic for values accumulated across ticks.**
   Cross-platform `f64` accumulation can diverge. Use `f32` with bounded
   per-tick increments (as the reference `ArenaShooter` does for positions),
   or fixed-point integers for anything that must be bit-exact.

4. **No I/O, no threads, no blocking in `step` / `validate`.** The Wasmtime
   sandbox strips these capabilities; code that relies on them will not build
   for `wasm32-wasip1`.

### How `validate` → `step` works

```text
Client sends InputFrame{seq, tick, input}
    │
    ▼
ValidatorChain  (RateLimit + InputSchema + your validators)
    │ Ok → passes through
    │ Err → ServerNet::Reject{seq, reason} sent to client; input dropped
    ▼
AuthoritativeGame::validate(player, input, tick)
    │ Ok(commands) → collected
    │ Err(reason)  → ServerNet::Reject sent; input dropped
    ▼
commands sorted deterministically by PlayerId
    ▼
AuthoritativeGame::step(ctx, &commands)
    ▼
snapshot/delta computed → ServerNet::Delta sent to all players (interest-filtered)
state_hash recorded in ReplayLog
```

---

## Step 3 — Build

```bash
magnetite build
```

Runs `cargo build --release --target wasm32-wasip1 --features wasm` and
prints the path to the produced `.wasm` artifact:

```
Building `/path/to/my-arena` for wasm32-wasip1…
Build succeeded.
Artifact: target/wasm32-wasip1/release/my_arena.wasm
```

The `--features wasm` flag enables the `mag_*` ABI exports that the
Wasmtime sandbox calls into:

```
mag_alloc(len: u32) -> u32        // bump allocator
mag_free(ptr: u32, len: u32)      // no-op (bump allocator)
mag_init(cfg_ptr: u32, cfg_len: u32)
mag_step(inputs_ptr, inputs_len) -> u32   // packed StepOutput ptr
mag_snapshot() -> u32
mag_restore(ptr: u32, len: u32)
mag_view(player_id: u64) -> u32
```

Each `-> u32` return is a pointer into linear memory where a
4-byte little-endian length prefix is followed by a JSON payload.
See `game-templates/authoritative/src/wasm_abi.rs` for the reference
implementation.

---

## Step 4 — Run locally with `magnetite dev`

```bash
magnetite dev
```

This does the following in one command:

1. Builds the Wasm artifact (`magnetite build`).
2. Loads it into `magnetite-sandbox` (`WasmExecutor`) — the game logic
   runs in Wasmtime with fuel limits, memory cap, and epoch interruption.
3. Starts `magnetite-runtime` in `SingleRoom` topology (one process,
   broadcast-all, up to 16 players).
4. Binds a WebSocket listener on a free port.
5. Prints the connect URL.

```
Loading `target/wasm32-wasip1/release/my_arena.wasm`…

  Connect URL : ws://127.0.0.1:54321
  Topology    : SingleRoom (max 4 players)
  Tick rate   : 20 Hz

Press Ctrl-C to stop.
```

### Dev flags

| Flag | Default | Description |
|---|---|---|
| `--port <PORT>` | `0` (OS-assigned) | WebSocket listen port |
| `--max-players <N>` | `4` | Player cap; also drives topology selection |
| `--path <PATH>` | `.` | Path to the game crate |

```bash
# Custom port and player cap:
magnetite dev --port 9000 --max-players 8
```

### Sandbox limits (default `LimitsConfig`)

| Limit | Default |
|---|---|
| Fuel per step | 10,000,000 Wasm instructions |
| Max memory | 64 MiB |
| Max wall time per step | 10 ms (2 epochs × 5 ms) |

---

## Step 5 — Connect the Bevy client

The `game-client-bevy` crate provides a ready-made Bevy app with
WebSocket transport, client-side prediction, and server reconciliation.

```bash
# In a separate terminal while `magnetite dev` is running:
MAGNETITE_SERVER=ws://127.0.0.1:54321/ws cargo run -p game-client-bevy
```

Or from code:

```rust
use magnetite_sdk::state::PlayerId;
use game_client_bevy::app::{build_app, NetConfig};

let player_id = PlayerId::new(1);
let net_config = NetConfig {
    url: "ws://127.0.0.1:54321/ws".to_string(),
    token: "dev-token".to_string(),
    player_id,
};
build_app(player_id, net_config).run();
```

### Netcode frame types

The client and server speak `magnetite_sdk::protocol`:

```
// client → server
ClientNet::InputFrame { seq: u32, tick: Tick, input: Input }

// server → client
ServerNet::Welcome { player_id, config }
ServerNet::Snapshot { tick, full: Vec<u8> }   // every MatchConfig.snapshot_every ticks (default: 300)
ServerNet::Delta    { tick, since_tick, diff: Vec<u8> }  // per tick, interest-filtered
ServerNet::Ack      { seq, tick }              // for prediction reconciliation
ServerNet::Reject   { seq, reason }
```

The client uses `magnetite_sdk::networking::PredictionBuffer` to apply
inputs immediately (client-side prediction) and reconcile against
`Ack` / `Snapshot` frames from the server.

---

## Step 6 — Deploy

```bash
export MAGNETITE_API_URL=https://api.magnetite.dev
export MAGNETITE_GAME_ID=01234567-89ab-cdef-0123-456789abcdef
export MAGNETITE_API_TOKEN=my-bearer-token   # if required

magnetite deploy
```

This:

1. Runs `magnetite build` to produce a fresh `.wasm` artifact.
2. POSTs the artifact metadata to
   `POST /api/v1/distribution/<game_id>/versions`.
3. Prints the registered version ID and instructions to promote it:

```
Deploy registered successfully.

  Version ID  : abc123
  Version     : 0.1.0
  Commit      : a1b2c3d

The artifact has been registered. To promote it to live, use the
platform dashboard or call:

  PUT https://api.magnetite.dev/api/v1/distribution/<game_id>/versions/abc123/promote
```

### Deploy environment variables

| Variable | Required | Description |
|---|---|---|
| `MAGNETITE_API_URL` | Yes | Backend base URL |
| `MAGNETITE_GAME_ID` | Yes | UUID of the registered game |
| `MAGNETITE_API_TOKEN` | No | Bearer token for authenticated endpoints |
| `MAGNETITE_VERSION` | No | Semantic version string (default: `0.1.0`) |
| `MAGNETITE_COMMIT` | No | Git commit SHA (default: `local`) |

---

## Anti-cheat by construction

Magnetite's anti-cheat is not an optional add-on — it is an architectural
consequence of server-authoritative design. Here is why cheating is
structurally impossible:

**Clients cannot send state, only inputs.** The server runs `validate`
to translate raw `Input` frames into authoritative `Command`s. Any attempt
to inject fake positions, health values, or events is rejected before it
reaches `step`.

**Validators run before game logic.** The SDK ships composable `Validator`
built-ins (`RateLimit`, `MovementVelocity`, `ActionCooldown`, `InputSchema`)
and the `magnetite-anticheat` crate adds `AimbotSnap`, `PositionTeleport`,
`FireRateCooldown`, and `InputFlood`. They chain together into a
`ValidatorChain` that runs on every input frame before `AuthoritativeGame::validate`.

**Deterministic replay verification.** Every input and state hash is
recorded in a `ReplayLog`. The anti-cheat service re-simulates the log
offline with `verify_replay<G>(&log)`. If any `state_hash` differs between
the original run and the re-simulation the result is
`ReplayVerdict::Divergence { tick, expected, got }` and the match is
flagged. This catches both cheated inputs (which produce different state)
and nondeterminism bugs.

**The sandbox isolates game code.** In the Wasm executor the game logic
has no access to the network, filesystem, system clock, or OS RNG. It
cannot phone home or read player data outside its linear memory. Fuel
metering prevents infinite loops from hanging the server.

**Only `view_for` bytes leave the server.** Each player receives only what
`view_for(player)` returns — enemies behind walls, inventory, or fog-of-war
regions are never serialised for that player, eliminating wallhack.

---

## Scale story — same code, different topologies

You write one `AuthoritativeGame` implementation. The runtime picks the
appropriate topology from `MatchConfig`, which you never touch:

| Players | Topology | Notes |
|---|---|---|
| 1–16 | `SingleRoom` | One process, broadcast-all. Optimal for game jams. |
| 17–256 | `Dedicated { tick_hz: 60 }` | Authoritative + per-player interest-filtered deltas. |
| 257+ | `Sharded { tick_hz: 20, cell_size: 500.0, max_per_shard: 64 }` | Spatial shards + handoff when players cross cell boundaries. AAA scale. |

`MatchConfig::auto(max_players)` selects the topology automatically. The
server selects it; your game code never changes.

```rust
// 4 players → SingleRoom automatically.
let cfg = MatchConfig::auto(4);
assert!(matches!(cfg.topology, Topology::SingleRoom));

// 100 players → Dedicated automatically.
let cfg = MatchConfig::auto(100);
assert!(matches!(cfg.topology, Topology::Dedicated { .. }));

// 1000 players → Sharded automatically.
let cfg = MatchConfig::auto(1000);
assert!(matches!(cfg.topology, Topology::Sharded { .. }));
```

For `Sharded`, the `ShardManager` in `magnetite-runtime` routes each player
to the shard that owns their spatial cell, and hands off state when a player
crosses a cell boundary. Because the game state is serialisable via
`snapshot` / `restore`, handoff is transparent to the game code.

---

## What to read next

- **Architecture details** — [architecture-overview.md](architecture-overview.md)
- **Full interface reference** — [../../docs/MOAT-ARCHITECTURE.md](../MOAT-ARCHITECTURE.md)
- **Reference game implementation** — `game-templates/authoritative/src/game.rs`
- **Wasm ABI reference** — `game-templates/authoritative/src/wasm_abi.rs`
- **Anti-cheat validators** — `magnetite-anticheat/src/lib.rs`
