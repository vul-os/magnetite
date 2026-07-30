# Magnetite MOAT — Scale Bench and Test Suite Report

> Last updated: 2026-06-03 (SCALE+POLISH wave — bench fix + real numbers)

This document describes the `magnetite-e2e` test harness: what it exercises, what each
test proves, and how to run the scale bench. All tests referenced here are in the
`magnetite-e2e` crate and were verified passing at the N3 close.

---

## Test suite overview

The `magnetite-e2e` crate (`magnetite-e2e/`) contains four integration test files. Three
run automatically under `cargo test`; two bench tests are `#[ignore]`-gated and must be
invoked explicitly.

| File | Tests | Run by default |
|---|---|---|
| `tests/convergence.rs` | `convergence_and_replay_clean` | yes |
| `tests/anticheat.rs` | `anticheat_rejects_speedhack_and_escalates_trust_score`, `anticheat_allows_honest_client` | yes |
| `tests/wasm_end_to_end.rs` | `wasm_sandbox_parity_with_native`, `wasm_state_hash_is_reproducible_across_instances`, `wasm_snapshot_restore_resumes_the_same_trajectory`, `native_verify_replay_clean_baseline` | yes (requires wasm artifact) |
| `tests/scale_bench.rs` | `scale_bench`, `ws_round_trip_latency_bench` | no (`#[ignore]`) |

---

## What each test proves

### `convergence_and_replay_clean`

**File:** `tests/convergence.rs`

Three-part proof:

1. **Determinism:** runs `ArenaShooter` through `NativeExecutor` for 20 ticks with 4 players,
   records the `ReplayLog`, and calls `verify_replay::<ArenaShooter>`. Asserts
   `ReplayVerdict::Clean` — tick-by-tick `state_hash` equality confirms the game simulation
   is deterministic (same seed + same ordered inputs → same result, every tick).

2. **Cross-run convergence:** runs two independent `NativeExecutor` instances with identical
   `MatchConfig` (same seed) and identical inputs. Asserts the final `state_hash` is equal.
   This is the cross-client convergence guarantee: any two clients simulating from the same
   authoritative state will agree.

3. **Live WS server:** starts a real `GameServer` (tokio, ephemeral port), connects 4
   simulated WebSocket clients, drives 5 authoritative ticks, and asserts that every client
   received at least one `Snapshot` or `Delta` message. Proves the server is live and
   broadcasting state.

### `anticheat_rejects_speedhack_and_escalates_trust_score`

**File:** `tests/anticheat.rs`

Starts a `GameServer` with `NativeExecutor<NopGame>` and an `Anticheat` pipeline containing
an `AimbotSnap` validator (threshold: 100 units). Connects a "cheater" client and sends a
`ClientNet::InputFrame` with `mouse.delta_x = mouse.delta_y = 9999.0` (magnitude ≈ 14 141 —
far above threshold). Asserts:

- The server sends `ServerNet::Reject { seq: 1 }` back to the cheater.
- A direct `TrustScoreMap` simulation (mirroring what the server's `Anticheat` does) confirms
  that 5 violations push the cheater's score to ≥ 5 (the configured kick threshold).

### `anticheat_allows_honest_client`

**File:** `tests/anticheat.rs`

Same server setup. Connects an honest client and sends a clean input with
`mouse.delta_x = delta_y = 1.0` (magnitude ≈ 1.41, well below the 100-unit threshold).
Asserts:

- No `Reject` is received for this input.
- `Ack { seq: 42 }` is received — the server accepted the input.

### `wasm_sandbox_parity_with_native` (headline proof)

**File:** `tests/wasm_end_to_end.rs`

Requires the WASM artifact (see "How to run the wasm tests" below).

Loads `game_template_authoritative.wasm` into a `WasmExecutor` (Wasmtime, fuel-metered,
memory-capped, epoch-interrupt timeout, no wall clock, no OS random). Simultaneously runs the
same game via `NativeExecutor<ArenaShooter>`. Drives 30 ticks with **non-empty, varying inputs
from four players** — movement, aim and shooting, so validation, rejection and the RNG draw that
mints a projectile id are all exercised on both sides. The sandbox ABI has no `on_join`, so first
sight of a player id in a `mag_step` payload is that player's join.

> **This used to be an empty input list**, on the reasoning that identical empty state makes for a
> clean hash-equality assertion. It does — and it is worthless. Two executors simulating nothing
> agree perfectly, which is how a guest that discarded every input frame passed this test for
> months. See [`site/docs/sandbox-abi.md`](../../site/docs/sandbox-abi.md) §8.

Asserts:

- `native_out.state_hash == wasm_out.state_hash` on **every tick** — the sandboxed execution
  produces bit-for-bit identical state hashes to the native path.
- `native_out.rejects.len() == wasm_out.rejects.len()` on every tick.
- More than half the ticks produce a *distinct* state hash — proof the inputs actually reached
  the simulation rather than the two sides agreeing on an idle physics loop.
- `verify_replay::<ArenaShooter>` over the native log returns `ReplayVerdict::Clean`.

This is the headline MOAT proof: the sandbox is deterministic, parity with native is exact under
real inputs, and the replay verifier passes.

Observed: **30 distinct state hashes over 30 ticks** (seed = `0xDEADCAFE1337BABE`). Specific hash
values are deliberately not pinned here — `StepCtx::rng` is now derived per tick from
`(seed, tick)`, and a table of literals in a report is a thing that goes stale silently.

### `wasm_snapshot_restore_resumes_the_same_trajectory`

**File:** `tests/wasm_end_to_end.rs`

Snapshots a sandboxed match at tick 15, installs that snapshot in a **fresh** `WasmExecutor`, and
runs both forward to tick 30 under identical inputs. Every tick's `state_hash` must match.

This is the property shard handoff and replay-from-checkpoint actually rest on, and it is strictly
stronger than "`restore(snapshot())` is idempotent" — which also passes on a module that ignores
`mag_restore` altogether, the exact shape of the bug it was written to catch.

### `wasm_state_hash_is_reproducible_across_instances`

**File:** `tests/wasm_end_to_end.rs`

Creates two independent `WasmExecutor` instances (A and B) from the same `.wasm` file,
same `MatchConfig`, same seed. Drives both for 30 ticks and asserts that every tick's
`state_hash` agrees between A and B. Also asserts the snapshot from A is non-empty.

This proves the sandbox is self-consistent across instances — any number of Wasmtime processes
running the same game with the same seed will agree on state at every tick.

Note: snapshot/restore across instances is not tested here because the N1/N2 ABI's static
`CURRENT_TICK` counter in the WASM guest is not reset by `mag_restore` (see `docs/project/DECISIONS.md`
crossroads M15 and N3-3). Two fresh instances are used instead. The production fix is to pass
tick as an explicit ABI parameter.

### `native_verify_replay_clean_baseline`

**File:** `tests/wasm_end_to_end.rs`

Regression guard that does not require the wasm artifact. Runs `NativeExecutor<ArenaShooter>`
for 30 ticks with the same four-player input sequence as the parity test, records the
`ReplayLog`, and asserts `ReplayVerdict::Clean`.

Worth stating plainly, because the restore defects in
[`site/docs/sandbox-abi.md`](../../site/docs/sandbox-abi.md) §8 raise the question: replay
verification never depended on any of them. `verify_replay` re-simulates from tick 0 and never
calls `restore`. What it *did* depend on was `on_join` being reachable — re-simulation has to
spawn the same players the recorded run had — and that now holds in both paths, so this baseline
covers a populated world rather than an empty one.

---

## The scale bench

### What it measures

`tests/scale_bench.rs` contains two `#[ignore]`-gated tests:

**`scale_bench`** — escalates player count from `SingleRoom` (4, 16 players) through
`Dedicated` (32, 64, 128, 256 players), running the authoritative `ArenaShooter` tick loop
via `NativeExecutor` and measuring:

- `ticks/sec` (throughput)
- `μs/tick` (per-tick latency)

The bench confirms the moat scale promise: identical game code, same `NativeExecutor`
interface, topology auto-selected by `MatchConfig::auto(max_players)`.

It includes a smoke-check assertion: SingleRoom at 4 players must sustain ≥ 1 000 ticks/sec
on the test machine. Failure fails the test.

**`ws_round_trip_latency_bench`** — starts a real `GameServer` on an ephemeral port, connects
one WebSocket client, sends 50 `ClientNet::InputFrame` messages, and collects per-message
round-trip latency (send → `ServerNet::Ack` received). Reports min, mean, p50, p99, and max
in microseconds.

> **Bug fix (SCALE+POLISH wave):** earlier versions used `ArenaShooter` which rejected inputs
> from players that had not been registered via `on_join`, causing the server to send
> `Reject` instead of `Ack` — resulting in zero latency samples. The bench uses `NopGame`
> (accepts any input unconditionally), so real round-trip Ack latency is measured. The test
> asserts `samples > 0` so the regression cannot silently re-appear.
>
> That original reason no longer applies: `NativeExecutor::step` now joins a player on first
> sight of their id, so `ArenaShooter` would `Ack`. `NopGame` stays anyway — a transport
> latency bench should measure the transport, not a simulation.

### How to run the scale bench

```sh
# Run both ignored bench tests (throughput + WS latency):
cargo test -p magnetite-e2e --test scale_bench -- --ignored --nocapture

# Run only the throughput bench:
cargo test -p magnetite-e2e --test scale_bench -- scale_bench --ignored --nocapture

# Run only the WS round-trip latency bench:
cargo test -p magnetite-e2e --test scale_bench -- ws_round_trip_latency_bench --ignored --nocapture
```

#### Verified output (debug build, Apple M-series, 2026-06-03)

Throughput bench (`scale_bench`):

```
╔═══════════════════════════════════════════════════════════╗
║        Magnetite MOAT — Scale / Throughput Report         ║
╚═══════════════════════════════════════════════════════════╝

Scenario                             Ticks       ticks/sec         μs/tick
──────────────────────────────────────────────────────────────────────────
SingleRoom  (4 players)               1000         10518.8           95.07
SingleRoom  (16 players)              1000         29546.0           33.84
Dedicated   (32 players)              1000         53082.7           18.84
Dedicated   (64 players)              1000          9793.4          102.11
Dedicated   (128 players)              500         13401.2           74.62
Dedicated   (256 players)              200         17509.4           57.11
Sharded     (pending N3)                 —               —               —
──────────────────────────────────────────────────────────────────────────
```

WS round-trip latency bench (`ws_round_trip_latency_bench`):

```
WS Round-Trip Latency — NopGame, single client, 50 samples
  collected = 50 samples
  min   = 13037.0 μs
  mean  = 16898.9 μs
  p50   = 16796.0 μs
  p99   = 24557.0 μs
  max   = 24557.0 μs
```

> Numbers are from an unoptimised debug build on loopback. A `--release` build typically
> cuts per-tick latency by 5–20x. The WS latency includes tokio async scheduler overhead
> and the 60 Hz tick boundary (≈ 16 ms per tick) — not pure network RTT.

### How to run the wasm end-to-end tests

The wasm parity tests require the WASM artifact to be built first:

```sh
# 1. Ensure the wasm32-wasip1 target is installed:
rustup target add wasm32-wasip1

# 2. Build the reference game as a WASM module:
cd game-templates/authoritative
cargo build --release --target wasm32-wasip1 --features wasm

# 3. Run the end-to-end parity tests:
cd ../magnetite-e2e
cargo test --test wasm_end_to_end -- --nocapture
```

Or use the one-command demo script that does all of the above plus convergence and anti-cheat
tests:

```sh
bash scripts/moat-demo.sh
```

### How to run the full non-bench suite

```sh
# Runs convergence + anticheat + wasm_end_to_end (if wasm artifact present):
cargo test -p magnetite-e2e
```

---

## What the test results prove about the MOAT guarantees

| MOAT guarantee | Test evidence |
|---|---|
| Identical game code scales from 4 to 256 players (SingleRoom → Dedicated) | `scale_bench` (throughput table; same `NativeExecutor` interface, topology auto-selected) |
| Authoritative simulation is deterministic | `convergence_and_replay_clean` (`verify_replay` returns `Clean`; two independent runs produce identical final hash) |
| All clients receive the same authoritative state | `convergence_and_replay_clean` (4 WS clients each receive ≥1 state message from the live server) |
| Sandbox (Wasmtime) is deterministically identical to native | `wasm_sandbox_parity_with_native` (state_hash matches on every tick for 30 ticks, under four players' non-empty varying inputs) |
| Sandbox is self-consistent across instances | `wasm_state_hash_is_reproducible_across_instances` (two independent WasmExecutor instances agree) |
| A sandboxed match can be resumed from a snapshot | `wasm_snapshot_restore_resumes_the_same_trajectory` (snapshot at tick 15 → fresh executor → identical hashes to tick 30) |
| Any wasm module can be checked against the ABI, in any language | `magnetite-sandbox` `conformance` + `mag-conformance`; `conformance/reference.wat` (hand-written WebAssembly, zero imports) reports 34 pass / 0 fail |
| Replay log enables tamper detection | `wasm_sandbox_parity_with_native` + `convergence_and_replay_clean` (`verify_replay` → `Clean`) |
| Anti-cheat rejects cheating inputs server-side | `anticheat_rejects_speedhack_and_escalates_trust_score` (`Reject` for huge mouse delta) |
| Anti-cheat allows honest inputs | `anticheat_allows_honest_client` (`Ack` for clean input, no `Reject`) |
| Trust escalation works | `anticheat_rejects_speedhack_and_escalates_trust_score` (score 0 → ≥ 5 after 5 violations) |
| One-command pipeline compiles and runs | `scripts/moat-demo.sh` (build → sandbox parity → convergence → fmt → check; all exit 0) |

---

## Remaining scale infrastructure (Bucket D)

The following are the genuine scale-path items that require external infrastructure and are
not faked. They are documented as the roadmap, not gaps in the current MOAT implementation.

- **Multi-node sharding / distributed shard coordination** — `ShardManager` assigns
  `ShardId::LOCAL` in N1; the handoff hook and the shard routing table exist but multi-process
  coordination is not implemented. Single-node multi-shard and distributed shard handoff are
  the next scale step.
- **Cloud auto-scaled runner fleet** — `magnetite deploy` provisions a runtime instance on the
  self-hosted backend; no auto-scaled cloud fleet exists. Kubernetes/Nomad manifests and a
  cloud runner API are the production scale path.
- **Production container orchestration** — no Kubernetes/Nomad manifests for the
  `magnetite-runtime` process.
- **MediaMTX media server** — required for HLS watch and RTMP egress. Not part of the MOAT
  primitive; documented as a deploy dependency in `docs/self-hosting/external-dependencies.md`.
- **GitHub CI wasm runner for the store** — the platform storefront's `trigger_wasm_build`
  writes a DB row; no CI subprocess is invoked. A GitHub Actions runner with the
  `wasm32-wasip1` target installed is the production path.
