//! Single-node multi-shard topology integration tests.
//!
//! ## What this proves
//!
//! 1. **Shard separation** — players placed in different spatial cells are
//!    routed to *different* [`ShardId`]s by the [`ShardedRuntime`].
//! 2. **Cross-boundary HANDOFF** — when a player's accumulated position
//!    crosses a cell boundary the runtime migrates their logical shard
//!    assignment without losing state.  After handoff the player's new shard
//!    executor holds a consistent world snapshot (hash equality on a
//!    deterministic replay).
//! 3. **State consistency** — two independent [`ShardedRuntime`] instances
//!    driven with identical inputs produce the same per-shard state hashes
//!    tick-by-tick.
//!
//! ## Design
//!
//! All tests are fully in-process (no WebSocket server) so they are fast,
//! deterministic, and free of port-binding races.  The [`ShardedRuntime`]
//! provides direct access to per-shard executors and the [`ShardManager`]
//! routing table, making it straightforward to assert invariants.
//!
//! The test game is [`ArenaShooter`] from `game-template-authoritative`.  We
//! use a small `cell_size` (100 units) and drive mouse-delta inputs large
//! enough to cross cell boundaries quickly.

use magnetite_runtime::{ShardId, ShardedRuntime};
use magnetite_sdk::authority::{GameExecutor, MatchConfig, NativeExecutor, Topology};
use magnetite_sdk::input::{Input, MouseState};
use magnetite_sdk::state::PlayerId;

use game_template_authoritative::ArenaShooter;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `Sharded` [`MatchConfig`] with a small cell size for fast boundary
/// crossings in tests.
fn sharded_cfg(cell_size: f32) -> MatchConfig {
    MatchConfig {
        topology: Topology::Sharded {
            tick_hz: 20,
            cell_size,
            max_per_shard: 64,
        },
        max_players: 256,
        tick_hz: 20,
        seed: 0x5A4D_CA4E,
        snapshot_every: 10,
    }
}

/// Build a fresh [`ShardedRuntime`] backed by [`NativeExecutor<ArenaShooter>`].
fn make_runtime(cfg: MatchConfig) -> ShardedRuntime {
    ShardedRuntime::new(cfg.clone(), Box::new(move |_shard_id, config| {
        Box::new(NativeExecutor::<ArenaShooter>::new(config.clone()))
    }))
}

/// An input that moves the player `dx` units on the x-axis (and 0 on y).
fn move_x(dx: f64) -> Input {
    Input {
        mouse: MouseState {
            delta_x: dx,
            delta_y: 0.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// An input that moves the player `dy` units on the y-axis (and 0 on x).
fn move_y(dy: f64) -> Input {
    Input {
        mouse: MouseState {
            delta_x: 0.0,
            delta_y: dy,
            ..Default::default()
        },
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Test 1 — players in different cells land on different shards
// ---------------------------------------------------------------------------

/// Players whose accumulated proxy positions put them in *different* cells must
/// be assigned to *different* [`ShardId`]s.
///
/// Setup:
/// - cell_size = 100 units.
/// - Player A: starts at origin (cell 0,0 = ShardId::LOCAL).
/// - Player B: given a single input that moves them 150 units on x → cell (1,0).
/// - Player C: given an input that moves them 110 units on y → cell (0,1).
///
/// After one tick the routing table must show A, B, C in three distinct shards.
#[test]
fn players_in_different_cells_use_different_shards() {
    let cfg = sharded_cfg(100.0);
    let mut runtime = make_runtime(cfg);

    let pa = PlayerId::new(1);
    let pb = PlayerId::new(2);
    let pc = PlayerId::new(3);

    runtime.join(pa);
    runtime.join(pb);
    runtime.join(pc);

    // All three start in the origin shard.
    assert_eq!(runtime.shard_of(pa), Some(ShardId::LOCAL));
    assert_eq!(runtime.shard_of(pb), Some(ShardId::LOCAL));
    assert_eq!(runtime.shard_of(pc), Some(ShardId::LOCAL));

    // Inputs: A stays put, B crosses into cell (1,0), C crosses into cell (0,1).
    let inputs = vec![
        (pa, Input::default()),       // stays in cell (0,0)
        (pb, move_x(150.0)),          // crosses into cell (1,0)
        (pc, move_y(110.0)),          // crosses into cell (0,1)
    ];

    let out = runtime.step(1, &inputs);

    // B and C should have triggered handoff events.
    let handoff_players: Vec<PlayerId> = out.handoffs.iter().map(|h| h.player).collect();
    assert!(
        handoff_players.contains(&pb),
        "player B should have been handed off (moved to cell 1,0); handoffs={handoff_players:?}"
    );
    assert!(
        handoff_players.contains(&pc),
        "player C should have been handed off (moved to cell 0,1); handoffs={handoff_players:?}"
    );

    // After tick: three distinct shards.
    let shard_a = runtime.shard_of(pa).expect("A has a shard");
    let shard_b = runtime.shard_of(pb).expect("B has a shard");
    let shard_c = runtime.shard_of(pc).expect("C has a shard");

    println!(
        "[shards] A={shard_a:?}, B={shard_b:?}, C={shard_c:?}"
    );

    assert_eq!(shard_a, ShardId::LOCAL, "A stays in origin cell (0,0)");
    assert_eq!(
        shard_b,
        ShardId::from_cell(1, 0),
        "B is in cell (1,0) after moving 150 units on x"
    );
    assert_eq!(
        shard_c,
        ShardId::from_cell(0, 1),
        "C is in cell (0,1) after moving 110 units on y"
    );

    // All three shards must be distinct.
    assert_ne!(shard_a, shard_b, "A and B must be on different shards");
    assert_ne!(shard_a, shard_c, "A and C must be on different shards");
    assert_ne!(shard_b, shard_c, "B and C must be on different shards");

    println!("[shards] PASS — players_in_different_cells_use_different_shards");
}

// ---------------------------------------------------------------------------
// Test 2 — handoff preserves state (state hash consistency after migration)
// ---------------------------------------------------------------------------

/// When a player crosses a cell boundary, the target shard executor receives
/// the full world snapshot from the source shard.  A re-simulation of the
/// exact same inputs on an independent executor must produce the same state
/// hash, proving no state was lost during the handoff.
///
/// Proof strategy:
/// - Run `ShardedRuntime` for 5 ticks; on tick 2 player crosses boundary.
/// - Capture the snapshot from the target shard after the handoff tick.
/// - Independently run `NativeExecutor<ArenaShooter>` for the same 5 ticks
///   (single shard, same inputs).
/// - The state hash from the independent run must equal the target shard's
///   snapshot hash after tick 2 (both represent the same world state at the
///   same tick).
#[test]
fn handoff_preserves_state_hash() {
    let cell_size = 100.0f32;
    let cfg = sharded_cfg(cell_size);

    // Build the sharded runtime.
    let mut runtime = make_runtime(cfg.clone());

    let pa = PlayerId::new(10);
    let pb = PlayerId::new(11);

    runtime.join(pa);
    runtime.join(pb);

    // Tick 1: both players move within cell (0,0).
    let inputs_t1 = vec![
        (pa, move_x(10.0)),  // within cell 0
        (pb, move_x(20.0)),  // within cell 0
    ];
    let out1 = runtime.step(1, &inputs_t1);
    assert!(
        out1.handoffs.is_empty(),
        "tick 1: no handoffs expected — both players still in cell (0,0)"
    );

    // Tick 2: pb crosses from cell (0,0) into cell (1,0).
    // Accumulated x for pb = 20 + 150 = 170 → cell 1 (>= 100).
    let inputs_t2 = vec![
        (pa, Input::default()),  // stays put
        (pb, move_x(150.0)),     // crosses boundary
    ];
    let out2 = runtime.step(2, &inputs_t2);
    let pb_handoff = out2
        .handoffs
        .iter()
        .find(|h| h.player == pb)
        .expect("tick 2: pb must be handed off");
    assert_eq!(pb_handoff.from_shard, ShardId::LOCAL, "pb leaves origin shard");
    assert_eq!(pb_handoff.to_shard, ShardId::from_cell(1, 0), "pb arrives at cell (1,0)");
    assert!(pb_handoff.target_addr.is_none(), "single-process: no remote addr");

    // The target shard executor must now exist.
    let target_shard = ShardId::from_cell(1, 0);
    let snap_bytes = runtime
        .snapshot_shard(target_shard)
        .expect("target shard executor must exist after handoff");

    // Independent reference executor (same config, same inputs).
    let mut ref_exec = NativeExecutor::<ArenaShooter>::new(cfg.clone());
    ref_exec.step(1, &inputs_t1);
    ref_exec.step(2, &inputs_t2);

    let ref_snap = ref_exec.snapshot();

    // Both should be non-empty; we compare length as a sanity check, then
    // compare the FNV hash that NativeExecutor computes internally.
    // Since both run the same inputs, their snapshots must be identical bytes.
    assert!(
        !snap_bytes.is_empty(),
        "target shard snapshot must not be empty after handoff"
    );
    assert!(
        !ref_snap.is_empty(),
        "reference executor snapshot must not be empty"
    );

    // The snapshot from the target shard (received via handoff) must equal the
    // independent reference run.  Both ran the same deterministic game over the
    // same inputs, so their serialised snapshots must be byte-identical.
    assert_eq!(
        snap_bytes, ref_snap,
        "target shard snapshot after handoff must match independent reference run \
         — state was lost or corrupted during handoff"
    );

    println!(
        "[handoff] PASS — handoff_preserves_state_hash \
         (snap_len={}, target={target_shard:?})",
        snap_bytes.len()
    );
}

// ---------------------------------------------------------------------------
// Test 3 — overall state consistency (two runtimes agree tick-by-tick)
// ---------------------------------------------------------------------------

/// Two independent [`ShardedRuntime`] instances driven with the same inputs
/// must produce the same per-shard state hashes on every tick.
///
/// This proves that [`ShardedRuntime`] is deterministic: given the same
/// (config, player set, ordered inputs) the system always reaches the same
/// authoritative state.  It is the sharded analogue of the convergence test
/// in `convergence.rs`.
#[test]
fn two_sharded_runtimes_converge() {
    let cfg = sharded_cfg(100.0);

    let mut runtime_a = make_runtime(cfg.clone());
    let mut runtime_b = make_runtime(cfg.clone());

    let p1 = PlayerId::new(20);
    let p2 = PlayerId::new(21);
    let p3 = PlayerId::new(22);

    for rt in [&mut runtime_a, &mut runtime_b] {
        rt.join(p1);
        rt.join(p2);
        rt.join(p3);
    }

    // Drive 10 ticks; on tick 4 p2 crosses into cell (1,0);
    // on tick 7 p3 crosses into cell (0,1).
    let tick_inputs: Vec<Vec<(PlayerId, Input)>> = (1u64..=10)
        .map(|tick| match tick {
            4 => vec![
                (p1, Input::default()),
                (p2, move_x(400.0)), // jumps to cell (4,0)
                (p3, Input::default()),
            ],
            7 => vec![
                (p1, Input::default()),
                (p2, Input::default()),
                (p3, move_y(400.0)), // jumps to cell (0,4)
            ],
            _ => vec![
                (p1, Input::default()),
                (p2, Input::default()),
                (p3, Input::default()),
            ],
        })
        .collect();

    for (tick_idx, inputs) in tick_inputs.iter().enumerate() {
        let tick = tick_idx as u64 + 1;
        let out_a = runtime_a.step(tick, inputs);
        let out_b = runtime_b.step(tick, inputs);

        // Collect shard→hash maps for both runtimes.
        let map_a: std::collections::HashMap<ShardId, u64> = out_a
            .shard_outputs
            .iter()
            .map(|(sid, so)| (*sid, so.state_hash))
            .collect();
        let map_b: std::collections::HashMap<ShardId, u64> = out_b
            .shard_outputs
            .iter()
            .map(|(sid, so)| (*sid, so.state_hash))
            .collect();

        // Every shard present in A must match B.
        for (sid, hash_a) in &map_a {
            if let Some(hash_b) = map_b.get(sid) {
                assert_eq!(
                    hash_a, hash_b,
                    "[convergence] FAIL at tick {tick}, shard {sid:?}: \
                     runtime_a={hash_a} runtime_b={hash_b}"
                );
            }
        }

        println!(
            "[convergence] tick {tick} OK — {} shards active, {} handoffs",
            out_a.shard_outputs.len(),
            out_a.handoffs.len()
        );
    }

    // After 10 ticks: p2 should be in cell (4,0), p3 in cell (0,4).
    // Both runtimes must agree on routing.
    let shard_p2_a = runtime_a.shard_of(p2).expect("p2 in A");
    let shard_p2_b = runtime_b.shard_of(p2).expect("p2 in B");
    assert_eq!(shard_p2_a, shard_p2_b, "p2 routing must agree across runtimes");
    assert_eq!(shard_p2_a, ShardId::from_cell(4, 0), "p2 in cell (4,0)");

    let shard_p3_a = runtime_a.shard_of(p3).expect("p3 in A");
    let shard_p3_b = runtime_b.shard_of(p3).expect("p3 in B");
    assert_eq!(shard_p3_a, shard_p3_b, "p3 routing must agree across runtimes");
    assert_eq!(shard_p3_a, ShardId::from_cell(0, 4), "p3 in cell (0,4)");

    println!(
        "[convergence] PASS — two_sharded_runtimes_converge \
         (p2={shard_p2_a:?}, p3={shard_p3_a:?})"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — player moving across multiple boundaries in sequence
// ---------------------------------------------------------------------------

/// A player crossing multiple cell boundaries across consecutive ticks
/// is handed off each time without losing shard routing continuity.
///
/// After each handoff the routing table is immediately correct, and the next
/// tick's inputs are routed to the *new* shard.
#[test]
fn sequential_handoffs_track_correctly() {
    let cfg = sharded_cfg(100.0);
    let mut runtime = make_runtime(cfg);

    let p = PlayerId::new(30);
    runtime.join(p);

    // Sequence: cross into (1,0) → (2,0) → (2,1).
    let moves: &[(f64, f64, ShardId)] = &[
        (150.0, 0.0,   ShardId::from_cell(1, 0)),  // tick 1: x→150 → cell (1,0)
        (110.0, 0.0,   ShardId::from_cell(2, 0)),  // tick 2: x→260 → cell (2,0)
        (0.0,   110.0, ShardId::from_cell(2, 1)),  // tick 3: y→110 → cell (2,1)
        (0.0,   0.0,   ShardId::from_cell(2, 1)),  // tick 4: stays
    ];

    for (tick_idx, &(dx, dy, expected_shard)) in moves.iter().enumerate() {
        let tick = tick_idx as u64 + 1;
        let input = Input {
            mouse: MouseState {
                delta_x: dx,
                delta_y: dy,
                ..Default::default()
            },
            ..Default::default()
        };
        let out = runtime.step(tick, &[(p, input)]);

        let actual_shard = runtime.shard_of(p).expect("player has a shard");
        assert_eq!(
            actual_shard, expected_shard,
            "tick {tick}: expected shard {expected_shard:?}, got {actual_shard:?}"
        );

        // A handoff must have occurred on the ticks where we crossed a boundary.
        let crossed = dx != 0.0 || dy != 0.0;
        // Note: a handoff only fires if the cell actually changed.  On tick 1
        // the player starts in (0,0) so any large dx triggers a handoff.  On
        // tick 4 (no movement) no handoff is expected.
        if tick < 4 {
            assert!(
                !out.handoffs.is_empty(),
                "tick {tick}: expected a handoff event but got none"
            );
            assert_eq!(
                out.handoffs[0].to_shard, expected_shard,
                "tick {tick}: handoff target mismatch"
            );
        } else {
            assert!(
                out.handoffs.is_empty(),
                "tick {tick}: no handoff expected when player stays put"
            );
        }
        let _ = crossed; // suppress unused warning

        println!("[sequential] tick {tick} shard={actual_shard:?} handoffs={}", out.handoffs.len());
    }

    println!("[sequential] PASS — sequential_handoffs_track_correctly");
}

// ---------------------------------------------------------------------------
// Test 5 — executor_count grows lazily
// ---------------------------------------------------------------------------

/// The runtime provisions a new shard executor only when a player actually
/// enters an unprovisioned shard.  Initially only the origin shard exists.
#[test]
fn executor_count_grows_on_demand() {
    let cfg = sharded_cfg(100.0);
    let mut runtime = make_runtime(cfg);

    // Only the origin shard exists at startup.
    assert_eq!(
        runtime.executor_count(),
        1,
        "runtime starts with exactly 1 executor (origin shard)"
    );

    let p = PlayerId::new(40);
    runtime.join(p);

    // No new shard after joining (player starts at origin).
    assert_eq!(runtime.executor_count(), 1);

    // Move into cell (1,0) → new executor should be provisioned.
    let out = runtime.step(1, &[(p, move_x(150.0))]);
    assert_eq!(
        out.handoffs.len(),
        1,
        "one handoff expected when crossing into cell (1,0)"
    );
    assert_eq!(
        runtime.executor_count(),
        2,
        "runtime should now have 2 executors after entering a new cell"
    );

    // Move into cell (2,0) → third executor.
    let out2 = runtime.step(2, &[(p, move_x(110.0))]);
    assert_eq!(out2.handoffs.len(), 1);
    assert_eq!(
        runtime.executor_count(),
        3,
        "runtime should now have 3 executors after entering cell (2,0)"
    );

    println!(
        "[lazy] PASS — executor_count_grows_on_demand (executors={})",
        runtime.executor_count()
    );
}
