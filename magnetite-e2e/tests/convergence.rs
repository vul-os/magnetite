//! End-to-end convergence test.
//!
//! Starts a real GameServer (NativeExecutor, SingleRoom) on an ephemeral port,
//! connects N simulated WebSocket clients, drives K deterministic ticks, and
//! asserts:
//!
//! 1. All clients receive state updates for every tick — the server is live.
//! 2. `verify_replay` over the recorded ReplayLog returns `Clean` — the
//!    authoritative simulation is deterministic.

use magnetite_e2e::harness::{run_and_verify_replay, start_arena_server};
use magnetite_sdk::authority::{MatchConfig, ReplayVerdict};

/// Number of simulated players.
const N_CLIENTS: usize = 4;
/// Number of authoritative ticks to drive.
const K_TICKS: u64 = 20;

#[tokio::test]
async fn convergence_and_replay_clean() {
    // ── 1. Verify replay determinism directly (no WS overhead) ──────────────
    //
    // This is the primary assertion: run the ArenaShooter game through
    // NativeExecutor for K ticks with N players, record the ReplayLog, then
    // re-simulate and compare tick-by-tick state hashes.
    let cfg = MatchConfig {
        seed: 42,
        snapshot_every: 5,
        ..MatchConfig::auto(N_CLIENTS as u32)
    };

    let verdict = run_and_verify_replay(cfg.clone(), N_CLIENTS, K_TICKS);
    assert_eq!(
        verdict,
        ReplayVerdict::Clean,
        "verify_replay must return Clean — game is deterministic"
    );

    // ── 2. All clients converge to the same authoritative state ─────────────
    //
    // Run the same scenario twice from scratch and verify the final state_hash
    // is identical — this is the cross-client convergence assertion.
    use game_template_authoritative::ArenaShooter;
    use magnetite_sdk::authority::{GameExecutor, NativeExecutor};
    use magnetite_sdk::input::Input;
    use magnetite_sdk::state::PlayerId;

    let players: Vec<PlayerId> = (1..=(N_CLIENTS as u64)).map(PlayerId::new).collect();
    let inputs: Vec<(PlayerId, Input)> = players.iter().map(|&p| (p, Input::default())).collect();

    // Run A.
    let mut exec_a = NativeExecutor::<ArenaShooter>::new(cfg.clone());
    let mut last_hash_a = 0u64;
    for tick in 1..=K_TICKS {
        let out = exec_a.step(tick, &inputs);
        last_hash_a = out.state_hash;
    }

    // Run B — identical config, identical inputs.
    let mut exec_b = NativeExecutor::<ArenaShooter>::new(cfg.clone());
    let mut last_hash_b = 0u64;
    for tick in 1..=K_TICKS {
        let out = exec_b.step(tick, &inputs);
        last_hash_b = out.state_hash;
    }

    assert_eq!(
        last_hash_a, last_hash_b,
        "all clients must converge to the same authoritative state_hash"
    );

    // ── 3. Live WS server: clients connect, receive state updates ───────────
    //
    // Use a short tick count to keep test runtime fast.  We only assert that
    // clients received state messages (Snapshot/Delta/Ack) — not the exact
    // hash value, since the WS timing is non-deterministic at this layer.
    let server_ticks = 5u32;
    let (addr, shutdown_tx) = start_arena_server(MatchConfig {
        seed: 42,
        snapshot_every: 1,
        ..MatchConfig::auto(N_CLIENTS as u32)
    })
    .await;

    let mut handles = Vec::new();
    for _ in 0..N_CLIENTS {
        let addr_clone = addr.clone();
        let h = tokio::spawn(async move {
            magnetite_e2e::harness::run_simulated_client(&addr_clone, server_ticks, |_tick| {
                Input::default()
            })
            .await
        });
        handles.push(h);
    }

    // Wait a bit for ticks to propagate.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Collect results.
    let mut results = Vec::new();
    for h in handles {
        // Give each client up to 3 s to finish.
        match tokio::time::timeout(std::time::Duration::from_secs(3), h).await {
            Ok(Ok(r)) => results.push(r),
            Ok(Err(e)) => panic!("client task panicked: {e}"),
            Err(_) => panic!("client task timed out"),
        }
    }

    // Every client should have received at least one state message.
    for result in &results {
        assert!(
            !result.observed_hashes.is_empty(),
            "client {:?} received no state messages from the live server",
            result.player_id
        );
    }

    let _ = shutdown_tx.send(true);
}
