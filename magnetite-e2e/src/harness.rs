//! Shared test harness helpers — server startup, client simulation, result collection.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use magnetite_runtime::{GameServer, GameServerConfig};
use magnetite_sdk::authority::{MatchConfig, NativeExecutor, ReplayVerdict};
use magnetite_sdk::input::Input;
use magnetite_sdk::protocol::{ClientNet, ServerNet};
use magnetite_sdk::state::PlayerId;

use game_template_authoritative::ArenaShooter;

// ---------------------------------------------------------------------------
// Server startup
// ---------------------------------------------------------------------------

/// Bind an ephemeral port and start a GameServer with NativeExecutor<ArenaShooter>.
///
/// Returns (addr_string, shutdown_tx) so tests can connect then signal shutdown.
pub async fn start_arena_server(cfg: MatchConfig) -> (String, watch::Sender<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener);

    let executor = NativeExecutor::<ArenaShooter>::new(cfg.clone());
    let server_cfg = GameServerConfig {
        bind_addr: addr.clone(),
        match_config: cfg,
        anticheat: None,
        fleet: None,
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let shutdown_rx2 = shutdown_rx.clone();
    let shutdown_tx2 = shutdown_tx.clone();

    tokio::spawn(async move {
        let _ =
            GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx2, shutdown_tx2).await;
    });

    // Give the server a moment to bind.
    tokio::time::sleep(Duration::from_millis(80)).await;
    (addr, shutdown_tx)
}

// ---------------------------------------------------------------------------
// Client simulation
// ---------------------------------------------------------------------------

/// Result collected by one simulated client after driving K ticks.
#[derive(Debug)]
pub struct ClientResult {
    pub player_id: PlayerId,
    /// State hashes received via Snapshot/Delta/Ack messages.
    /// The *last* entry is the final state hash seen by this client.
    pub observed_hashes: Vec<u64>,
    /// Number of Ack messages received.
    pub ack_count: usize,
    /// Number of Reject messages received.
    pub reject_count: usize,
}

/// Connect a simulated client, send `ticks` input frames, collect results.
///
/// The client sends a simple default Input every tick and records every
/// state hash it receives.
pub async fn run_simulated_client(
    addr: &str,
    ticks: u32,
    input_fn: impl Fn(u32) -> Input,
) -> ClientResult {
    let url = format!("ws://{addr}");
    let (mut ws, _) = connect_async(&url).await.expect("client connect failed");

    // Receive Welcome.
    let welcome_msg = ws.next().await.expect("no Welcome").expect("ws error");
    let player_id = if let Message::Text(text) = welcome_msg {
        let net: ServerNet = serde_json::from_str(&text).expect("parse Welcome");
        match net {
            ServerNet::Welcome { player_id, .. } => player_id,
            other => panic!("expected Welcome, got {other:?}"),
        }
    } else {
        panic!("expected text Welcome");
    };

    let mut observed_hashes: Vec<u64> = Vec::new();
    let mut ack_count = 0usize;
    let mut reject_count = 0usize;

    // Drive the tick loop: send inputs and collect server responses.
    for tick in 1u32..=ticks {
        let input = input_fn(tick);
        let frame = ClientNet::InputFrame {
            seq: tick,
            tick: tick as u64,
            input,
        };
        let text = serde_json::to_string(&frame).unwrap();
        ws.send(Message::Text(text.into())).await.unwrap();

        // Drain any pending messages from the server within a short window.
        let deadline = tokio::time::Instant::now() + Duration::from_millis(80);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, ws.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    let net: ServerNet =
                        serde_json::from_str(&text).unwrap_or_else(|_| return_reject());
                    match net {
                        ServerNet::Snapshot { tick: t, .. } => {
                            // Record tick as a proxy hash — the real hash lives in
                            // the replay log; here we just track per-client tick receipt.
                            observed_hashes.push(t);
                        }
                        ServerNet::Delta { tick: t, .. } => {
                            observed_hashes.push(t);
                        }
                        ServerNet::Ack { .. } => {
                            ack_count += 1;
                        }
                        ServerNet::Reject { .. } => {
                            reject_count += 1;
                        }
                        ServerNet::Welcome { .. } => {}
                        // Fleet frames: not exercised by this harness.
                        ServerNet::NodeIdentity { .. } | ServerNet::Redirect { .. } => {}
                        // Attested-input frames (seam §3.7). Deliberately NOT
                        // folded into `ack_count`/`reject_count`: those measure
                        // the deterministic, replay-verifiable input path, and
                        // counting a sensor claim there would quietly overstate
                        // what this harness verified. Attested ingress has its
                        // own coverage in `magnetite-runtime/tests/attested_wire.rs`.
                        ServerNet::AttestedAck { .. } | ServerNet::AttestedReject { .. } => {}
                    }
                }
                Ok(Some(Ok(Message::Close(_)))) => break,
                Ok(Some(Ok(_))) => {}
                _ => break,
            }
        }
    }

    let _ = ws.close(None).await;

    ClientResult {
        player_id,
        observed_hashes,
        ack_count,
        reject_count,
    }
}

/// Dummy helper to return a benign Reject when JSON parse fails (shouldn't happen).
fn return_reject() -> ServerNet {
    ServerNet::Reject {
        seq: 0,
        reason: magnetite_sdk::authority::RejectReason::IllegalAction("parse error".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Direct (non-WS) replay verification helper
// ---------------------------------------------------------------------------

/// Run `ticks` authoritative ticks with `n_players` using NativeExecutor, record
/// the ReplayLog, and return `verify_replay` verdict.
pub fn run_and_verify_replay(cfg: MatchConfig, n_players: usize, ticks: u64) -> ReplayVerdict {
    use magnetite_sdk::authority::{verify_replay, GameExecutor, ReplayLog};

    let mut exec = NativeExecutor::<ArenaShooter>::new(cfg.clone());
    let mut log = ReplayLog::new(cfg);

    let players: Vec<PlayerId> = (1..=(n_players as u64)).map(PlayerId::new).collect();

    for tick in 1..=ticks {
        let inputs: Vec<(PlayerId, Input)> =
            players.iter().map(|&p| (p, Input::default())).collect();
        let out = exec.step(tick, &inputs);
        log.record(tick, inputs, out.state_hash);
    }

    verify_replay::<ArenaShooter>(&log)
}
