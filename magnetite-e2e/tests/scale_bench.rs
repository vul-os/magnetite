//! Scale and throughput bench harness.
//!
//! This module escalates player count across topologies (SingleRoom → Dedicated)
//! and measures ticks/sec and per-tick latency for each configuration.
//!
//! Mark as `#[ignore]` so the CI suite doesn't block on it; run explicitly with:
//!
//! ```sh
//! cargo test -p magnetite-e2e --test scale_bench -- --ignored --nocapture
//! ```
//!
//! Results are written to stdout (and, when invoked from the task, to
//! /tmp/e2e.txt via shell redirect).
//!
//! ## WS round-trip latency bench
//!
//! Uses a `NopGame` (not `ArenaShooter`) so that every `InputFrame` produces an
//! `Ack` without requiring `on_join`. `ArenaShooter::validate` returns
//! `Unauthorized` for players who have not been joined, which means the server
//! sends `Reject` instead of `Ack` — causing zero latency samples.  `NopGame`
//! accepts any input from any player and always returns `Ok(vec![])`, so the
//! round-trip is: client sends `InputFrame` → server emits `Ack{seq}` → client
//! receives and records elapsed time.

use std::time::{Duration, Instant};

use magnetite_sdk::authority::{
    AuthoritativeGame, GameExecutor, MatchConfig, NativeExecutor, RejectReason, StepCtx, Tick,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

use game_template_authoritative::ArenaShooter;

// ---------------------------------------------------------------------------
// NopGame — accepts all inputs, requires no on_join
// ---------------------------------------------------------------------------

/// Minimal game that always accepts any input from any player without requiring
/// `on_join`.  Used by the WS latency bench to ensure inputs produce `Ack` (not
/// `Reject`) so we can measure real round-trip latency.
struct NopGame;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct NopSnap;

#[derive(serde::Serialize, serde::Deserialize)]
struct NopDelta;

#[derive(serde::Serialize)]
struct NopView;

#[derive(serde::Serialize, serde::Deserialize)]
struct NopCmd;

impl AuthoritativeGame for NopGame {
    type Snapshot = NopSnap;
    type Delta = NopDelta;
    type View = NopView;
    type Command = NopCmd;

    fn init(_cfg: &MatchConfig) -> Self {
        NopGame
    }
    fn validate(&self, _p: PlayerId, _i: &Input, _t: Tick) -> Result<Vec<NopCmd>, RejectReason> {
        Ok(vec![])
    }
    fn step(&mut self, _ctx: &mut StepCtx, _cmds: &[(PlayerId, NopCmd)]) {}
    fn snapshot(&self) -> NopSnap {
        NopSnap
    }
    fn restore(_s: &NopSnap, _cfg: &MatchConfig) -> Self {
        NopGame
    }
    fn delta(&self, _s: &NopSnap) -> NopDelta {
        NopDelta
    }
    fn view_for(&self, _p: PlayerId) -> NopView {
        NopView
    }
}

// ---------------------------------------------------------------------------
// Throughput bench (scale_bench)
// ---------------------------------------------------------------------------

/// One benchmark scenario.
struct Scenario {
    label: &'static str,
    max_players: u32,
    ticks: u64,
}

/// Run one scenario and return `(total_ticks, ticks_per_sec, per_tick_us)`.
fn run_scenario(s: &Scenario) -> (u64, f64, f64) {
    let cfg = MatchConfig {
        seed: 0xDEAD_BEEF,
        snapshot_every: 300,
        ..MatchConfig::auto(s.max_players)
    };
    let mut exec = NativeExecutor::<ArenaShooter>::new(cfg);

    let players: Vec<PlayerId> = (1..=(s.max_players as u64)).map(PlayerId::new).collect();
    let inputs: Vec<(PlayerId, Input)> = players.iter().map(|&p| (p, Input::default())).collect();

    let start = Instant::now();
    for tick in 1..=s.ticks {
        exec.step(tick, &inputs);
    }
    let elapsed = start.elapsed();

    let elapsed_secs = elapsed.as_secs_f64();
    let ticks_per_sec = s.ticks as f64 / elapsed_secs;
    let per_tick_us = (elapsed.as_micros() as f64) / s.ticks as f64;

    (s.ticks, ticks_per_sec, per_tick_us)
}

/// Scale bench — escalates player count SingleRoom → Dedicated.
///
/// Marked `#[ignore]` so it does not run in normal `cargo test`.
///
/// Run with:
/// ```sh
/// cargo test -p magnetite-e2e --test scale_bench -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn scale_bench() {
    let scenarios = vec![
        Scenario {
            label: "SingleRoom  (4 players)",
            max_players: 4,
            ticks: 1_000,
        },
        Scenario {
            label: "SingleRoom  (16 players)",
            max_players: 16,
            ticks: 1_000,
        },
        Scenario {
            label: "Dedicated   (32 players)",
            max_players: 32,
            ticks: 1_000,
        },
        Scenario {
            label: "Dedicated   (64 players)",
            max_players: 64,
            ticks: 1_000,
        },
        Scenario {
            label: "Dedicated   (128 players)",
            max_players: 128,
            ticks: 500,
        },
        Scenario {
            label: "Dedicated   (256 players)",
            max_players: 256,
            ticks: 200,
        },
    ];

    println!();
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║        Magnetite MOAT — Scale / Throughput Report         ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "{:<30}  {:>10}  {:>14}  {:>14}",
        "Scenario", "Ticks", "ticks/sec", "μs/tick"
    );
    println!("{}", "─".repeat(74));

    for s in &scenarios {
        let (ticks, tps, us_per_tick) = run_scenario(s);
        println!(
            "{:<30}  {:>10}  {:>14.1}  {:>14.2}",
            s.label, ticks, tps, us_per_tick
        );
    }

    // Sharded topology row: pending agent-1's ShardManager integration.
    // When available, add:
    //   Scenario { label: "Sharded     (1024 players)", max_players: 1024, ticks: 100 }
    // and run it via NativeExecutor with Topology::Sharded config.
    println!(
        "{:<30}  {:>10}  {:>14}  {:>14}",
        "Sharded     (pending N3)", "—", "—", "—"
    );

    println!("{}", "─".repeat(74));
    println!();
    println!("Note: Sharded topology (AAA) — single-node multi-shard ShardManager exists");
    println!("      in magnetite-runtime; full perf row pending agent-1 sharded.rs integration.");
    println!();

    // Performance smoke-check: SingleRoom at 4 players must sustain >= 1 000 ticks/sec.
    // Re-run to avoid borrow issues with the first scenarios vector element.
    let smoke = Scenario {
        label: "SingleRoom  (4 players) [smoke]",
        max_players: 4,
        ticks: 1_000,
    };
    let (_, tps_4p, _) = run_scenario(&smoke);
    assert!(
        tps_4p >= 1_000.0,
        "SingleRoom 4-player throughput must be >= 1 000 ticks/sec (got {tps_4p:.1})"
    );
}

// ---------------------------------------------------------------------------
// WS round-trip latency bench
// ---------------------------------------------------------------------------

/// Async WS-level bench: measures round-trip latency from `InputFrame` send to
/// `Ack{seq}` receipt.
///
/// **Uses `NopGame`** — not `ArenaShooter`.  `ArenaShooter::validate` returns
/// `Unauthorized` for players that have not been registered via `on_join`,
/// causing the server to send `Reject` instead of `Ack`.  With `NopGame` every
/// input is accepted unconditionally, so real Ack latency samples are collected.
///
/// Also `#[ignore]` — run with:
/// ```sh
/// cargo test -p magnetite-e2e --test scale_bench -- ws_round_trip_latency_bench --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn ws_round_trip_latency_bench() {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio::sync::watch;
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    use magnetite_runtime::{GameServer, GameServerConfig};
    use magnetite_sdk::protocol::{ClientNet, ServerNet};

    // Bind an ephemeral port then release it so GameServer can bind.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener);

    // Use NopGame — accepts any input, no on_join required.
    let cfg = MatchConfig {
        seed: 0,
        snapshot_every: 300,
        ..MatchConfig::auto(1)
    };
    let executor = NativeExecutor::<NopGame>::new(cfg.clone());
    let server_cfg = GameServerConfig {
        bind_addr: addr.clone(),
        match_config: cfg,
        anticheat: None,
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let shutdown_rx2 = shutdown_rx.clone();
    let shutdown_tx2 = shutdown_tx.clone();

    tokio::spawn(async move {
        let _ =
            GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx2, shutdown_tx2).await;
    });

    // Give the server time to bind and start the accept loop.
    tokio::time::sleep(Duration::from_millis(80)).await;

    let url = format!("ws://{addr}");
    let (mut ws, _) = connect_async(&url).await.unwrap();

    // Consume the Welcome frame.
    let _ = ws.next().await;

    let n_samples = 50usize;
    let mut latencies_us: Vec<f64> = Vec::with_capacity(n_samples);

    for seq in 1u32..=(n_samples as u32) {
        let frame = ClientNet::InputFrame {
            seq,
            tick: seq as u64,
            input: Input::default(),
        };
        let text = serde_json::to_string(&frame).unwrap();

        let t0 = Instant::now();
        ws.send(Message::Text(text.into())).await.unwrap();

        // Drain until we receive Ack{seq} or timeout (2 s per sample).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        'drain: loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break 'drain;
            }
            match tokio::time::timeout(remaining, ws.next()).await {
                Ok(Some(Ok(Message::Text(txt)))) => {
                    if let Ok(net) = serde_json::from_str::<ServerNet>(&txt) {
                        match net {
                            ServerNet::Ack { seq: ack_seq, .. } if ack_seq == seq => {
                                latencies_us.push(t0.elapsed().as_micros() as f64);
                                break 'drain;
                            }
                            // Snapshot / Delta frames come interleaved — skip them.
                            ServerNet::Snapshot { .. } | ServerNet::Delta { .. } => continue,
                            _ => {}
                        }
                    }
                }
                Ok(Some(Ok(Message::Ping(_)))) | Ok(Some(Ok(Message::Pong(_)))) => continue,
                _ => break 'drain,
            }
        }
    }

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(true);

    // Assert at least some samples were collected.
    assert!(
        !latencies_us.is_empty(),
        "ws_round_trip_latency_bench collected 0 samples — \
         no Ack frames received for {} InputFrames. \
         Check that NopGame is used (not ArenaShooter) and the server is running.",
        n_samples
    );

    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = latencies_us.len();
    let mean = latencies_us.iter().sum::<f64>() / n as f64;
    let p50 = latencies_us[n / 2];
    let p99 = latencies_us[(n * 99) / 100];
    let min = latencies_us[0];
    let max = latencies_us[n - 1];

    println!();
    println!("WS Round-Trip Latency — NopGame, single client, {n_samples} samples");
    println!("  collected = {n} samples");
    println!("  min   = {min:.1} \u{03bc}s");
    println!("  mean  = {mean:.1} \u{03bc}s");
    println!("  p50   = {p50:.1} \u{03bc}s");
    println!("  p99   = {p99:.1} \u{03bc}s");
    println!("  max   = {max:.1} \u{03bc}s");
    println!();
}
