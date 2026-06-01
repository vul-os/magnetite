//! Scale and throughput bench harness.
//!
//! This module escalates player count across topologies (SingleRoom → Dedicated)
//! and measures ticks/sec and per-tick latency for each configuration.
//!
//! Mark as `#[ignore]` so the CI suite doesn't block on it; run explicitly with:
//!
//! ```sh
//! cargo test -p magnetite-e2e -- scale_bench --ignored --nocapture
//! ```
//!
//! Results are written to stdout (and, when invoked from the task, to
//! /tmp/e2e.txt via shell redirect).

use std::time::{Duration, Instant};

use magnetite_sdk::authority::{GameExecutor, MatchConfig, NativeExecutor};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

use game_template_authoritative::ArenaShooter;

/// One benchmark scenario.
struct Scenario {
    label: &'static str,
    max_players: u32,
    ticks: u64,
}

/// Run one scenario and return `(total_ticks, elapsed_secs, per_tick_us)`.
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
/// Run with: `cargo test -p magnetite-e2e -- scale_bench --ignored --nocapture`
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

    println!("{}", "─".repeat(74));
    println!();
    println!("Note: Sharded topology (AAA) requires the full ShardManager");
    println!("      integration which is completed in N3.");
    println!();

    // Performance smoke-check: SingleRoom at 4 players must sustain ≥ 1 000 ticks/sec.
    let (_, tps_4p, _) = run_scenario(&scenarios[0]);
    assert!(
        tps_4p >= 1_000.0,
        "SingleRoom 4-player throughput must be ≥ 1 000 ticks/sec (got {tps_4p:.1})"
    );
}

/// Async WS-level bench: measures round-trip latency from input send to Ack.
///
/// Also `#[ignore]` — run manually.
#[tokio::test]
#[ignore]
async fn ws_round_trip_latency_bench() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    use magnetite_runtime::{GameServer, GameServerConfig};
    use magnetite_sdk::protocol::{ClientNet, ServerNet};
    use tokio::net::TcpListener;
    use tokio::sync::watch;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener);

    let cfg = MatchConfig {
        seed: 0,
        snapshot_every: 300,
        ..MatchConfig::auto(1)
    };
    let executor = NativeExecutor::<ArenaShooter>::new(cfg.clone());
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

    tokio::time::sleep(Duration::from_millis(80)).await;

    let url = format!("ws://{addr}");
    let (mut ws, _) = connect_async(&url).await.unwrap();

    // Consume Welcome.
    let _ = ws.next().await;

    let n_samples = 50usize;
    let mut latencies_us = Vec::with_capacity(n_samples);

    for seq in 1u32..=(n_samples as u32) {
        let frame = ClientNet::InputFrame {
            seq,
            tick: seq as u64,
            input: Input::default(),
        };
        let text = serde_json::to_string(&frame).unwrap();

        let t0 = Instant::now();
        ws.send(Message::Text(text.into())).await.unwrap();

        // Wait for Ack(seq).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, ws.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    if let Ok(ServerNet::Ack { seq: ack_seq, .. }) =
                        serde_json::from_str::<ServerNet>(&text)
                    {
                        if ack_seq == seq {
                            latencies_us.push(t0.elapsed().as_micros() as f64);
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(true);

    if latencies_us.is_empty() {
        println!("WS round-trip bench: no samples collected");
        return;
    }

    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = latencies_us.iter().sum::<f64>() / latencies_us.len() as f64;
    let p50 = latencies_us[latencies_us.len() / 2];
    let p99 = latencies_us[(latencies_us.len() * 99) / 100];

    println!();
    println!("WS Round-Trip Latency (single client, {n_samples} samples):");
    println!("  mean  = {mean:.1} μs");
    println!("  p50   = {p50:.1} μs");
    println!("  p99   = {p99:.1} μs");
    println!();
}
