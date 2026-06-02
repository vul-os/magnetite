# Magnetite MOAT — Scaling Architecture and Bench Guide

> Scale primitive: identical game code, topology auto-selected from `SingleRoom` through
> `Dedicated` to `Sharded`. Same `AuthoritativeGame` trait, same `NativeExecutor` interface,
> no game-code changes across topologies.

---

## Topologies

### SingleRoom

Optimal for game jams and small matches (≤ 16 players).

- One process, one executor, broadcast-all tick loop.
- `MatchConfig::auto(n)` selects this for `n ≤ 16`.
- `GameServer` binds one TCP/WS listener; the `TickScheduler` fans out to all connected players.
- No shard routing — `ShardManager` assigns every player to `ShardId::LOCAL`.

### Dedicated

Standard authoritative server with per-player interest-filtered deltas (≤ 256 players).

- Same single-process architecture as SingleRoom; topology is a `MatchConfig` field.
- `MatchConfig::auto(n)` selects `Dedicated { tick_hz: 60 }` for `17 ≤ n ≤ 256`.
- Each tick: drain inputs → anticheat → executor step → Ack/Reject/Delta/Snapshot fan-out.
- Interest filtering: `view_for(player)` limits each player to their visible slice of the world.

### Sharded (N2 — single-process multi-shard; N3+ — multi-node)

Spatial sharding for AAA-scale player counts. `MatchConfig::auto(n)` selects
`Sharded { tick_hz: 20, cell_size: 500.0, max_per_shard: 64 }` for `n > 256`.

The world is partitioned into a 2-D grid of square cells. Each cell maps to a `ShardId`.
`ShardManager` routes each player to the shard covering their current position (estimated
from input mouse-delta accumulation as a proxy until the game exposes an explicit position
signal).

When a player crosses a cell boundary, `ShardManager::update_position` emits a `HandoffEvent`:

1. The runtime serialises the player's state (`GameExecutor::snapshot`).
2. `ShardManager::apply_handoff` updates the in-process routing table (N2) or forwards the
   state blob to the target shard's runtime over the network (N3+).
3. The player's WS connection stays on the same tokio task; only the logical shard routing
   changes.

**Multi-node seam:** `HandoffEvent::target_addr` is `None` in N2 (single-process). In N3+
replace `apply_handoff`'s body with an HTTP `POST /restore` call to the target node.

---

## Throughput bench results

Run on Apple M-series, debug build, `ArenaShooter` (reference game), loopback, 2026-06-03.

| Scenario | Ticks | ticks/sec | μs/tick |
|---|---|---|---|
| SingleRoom (4 players) | 1 000 | 10 518.8 | 95.07 |
| SingleRoom (16 players) | 1 000 | 29 546.0 | 33.84 |
| Dedicated (32 players) | 1 000 | 53 082.7 | 18.84 |
| Dedicated (64 players) | 1 000 | 9 793.4 | 102.11 |
| Dedicated (128 players) | 500 | 13 401.2 | 74.62 |
| Dedicated (256 players) | 200 | 17 509.4 | 57.11 |
| Sharded (pending N3) | — | — | — |

> Debug builds include bounds-checks and no LLVM optimisation. Release builds (`--release`)
> typically reduce `μs/tick` by 5–20x for the `ArenaShooter` reference game.
>
> The non-monotonic shape (64p slower than 32p, then improving at 128/256) is expected in
> debug builds: collision detection is O(players²) and debug-build bounds-checks dominate
> at 64+ players; release builds see a different (monotonically increasing) profile.

### Smoke-check assertion

The `scale_bench` test asserts:

```
SingleRoom 4-player throughput >= 1 000 ticks/sec
```

This is a regression guard. Failure fails `cargo test -p magnetite-e2e --test scale_bench -- --ignored`.

---

## WS round-trip latency results

Measured with `ws_round_trip_latency_bench` — `NopGame`, 50 samples, single client on
loopback, debug build, Apple M-series, 2026-06-03.

| Metric | Value |
|---|---|
| samples | 50 |
| min | 13 037 μs |
| mean | 16 898.9 μs |
| p50 | 16 796.0 μs |
| p99 | 24 557.0 μs |
| max | 24 557.0 μs |

> The WS latency is dominated by the tokio async scheduler and the server's tick boundary
> (default `tick_hz = 30` for SingleRoom → 33 ms per tick). The round-trip includes:
> client send → kernel TCP → server `ws.next()` poll → tick boundary → Ack serialise →
> kernel TCP → client `ws.next()` poll. Pure network RTT on loopback is < 0.1 ms.
>
> **Why NopGame?** `ArenaShooter::validate` returns `Unauthorized` for players that have not
> been registered via `on_join`. The WS connection path does not call `on_join` (that is the
> runtime host's responsibility, a Bucket-N2 integration point). Using `ArenaShooter`
> therefore produces only `Reject` frames, not `Ack`, yielding zero latency samples. `NopGame`
> accepts any input from any player without `on_join`, so real Ack round-trip latency is
> measured. The test asserts `samples > 0` so the regression cannot silently re-appear.

---

## How to run the bench

```sh
# Both bench tests (throughput table + WS latency):
cargo test -p magnetite-e2e --test scale_bench -- --ignored --nocapture

# Throughput bench only:
cargo test -p magnetite-e2e --test scale_bench -- scale_bench --ignored --nocapture

# WS round-trip latency bench only:
cargo test -p magnetite-e2e --test scale_bench -- ws_round_trip_latency_bench --ignored --nocapture
```

To capture output to a file (as the CI task does):

```sh
cargo test -p magnetite-e2e --test scale_bench -- --ignored --nocapture > /tmp/e2e.txt 2>&1
cat /tmp/e2e.txt
```

---

## Sharded topology — pending rows

The `scale_bench` table includes a `Sharded (pending N3)` row with `—` placeholders.
The `ShardManager` spatial routing implementation exists in `magnetite-runtime/src/shard.rs`
and is fully tested, but a dedicated end-to-end Sharded perf scenario is pending agent-1's
`tests/sharded.rs` integration. Once that lands, add to `scale_bench`:

```rust
// In run_scenario, use Topology::Sharded explicitly:
let cfg = MatchConfig {
    topology: Topology::Sharded { tick_hz: 20, cell_size: 500.0, max_per_shard: 64 },
    max_players: 1024,
    tick_hz: 20,
    seed: 0xDEAD_BEEF,
    snapshot_every: 300,
};
```

And add a `Scenario` entry:

```rust
Scenario {
    label: "Sharded     (1024 players)",
    max_players: 1024,
    ticks: 100,
},
```

---

## Scale architecture — next steps (Bucket D)

| Item | Status |
|---|---|
| Single-node multi-shard (`ShardManager` + handoff) | Implemented (N2) |
| Sharded perf bench row | Pending agent-1 sharded.rs |
| Multi-node shard coordination (etcd / distributed KV) | N3+ |
| Cloud auto-scaled runner fleet | Bucket D |
| Kubernetes/Nomad manifests for `magnetite-runtime` | Bucket D |
