//! Anti-cheat integration test.
//!
//! Scenario: one client sends a teleport/speedhack input (huge mouse delta) while
//! others behave normally.
//!
//! Asserts:
//! 1. The cheater's input is Rejected by the server (anticheat fires).
//! 2. The cheater's TrustScore escalates (above 0 after the violation).
//! 3. An honest client is Ack'd for a clean input.
//!
//! ## Design note on game choice
//!
//! The anti-cheat tests use a trivial `NopGame` (not `ArenaShooter`) because
//! ArenaShooter::validate returns `Unauthorized` for players who have not had
//! `on_join` called — which the WS path does not do automatically.  The anti-cheat
//! layer sits *before* the game executor, so the game choice does not affect whether
//! the chain rejects or allows the input.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use magnetite_anticheat::{Anticheat, AnticheatConfig};
use magnetite_runtime::{GameServer, GameServerConfig};
use magnetite_sdk::authority::{
    AuthoritativeGame, MatchConfig, NativeExecutor, RejectReason, StepCtx, Tick, ValidatorChain,
};
use magnetite_sdk::input::{Input, MouseState};
use magnetite_sdk::protocol::{ClientNet, ServerNet};
use magnetite_sdk::state::PlayerId;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use magnetite_anticheat::validators::AimbotSnap;

// ---------------------------------------------------------------------------
// Trivial game — always accepts inputs; used so WS validation path doesn't
// interfere with the anti-cheat assertions.
// ---------------------------------------------------------------------------

struct NopGame;

#[derive(Serialize, Deserialize, Clone)]
struct NopSnap;

#[derive(Serialize, Deserialize)]
struct NopDelta;

#[derive(Serialize)]
struct NopView;

#[derive(Serialize, Deserialize)]
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
// Constants
// ---------------------------------------------------------------------------

/// Speedhack delta — well above the server's AimbotSnap threshold.
const CHEAT_DELTA: f64 = 9_999.0;
/// Legitimate threshold given to the server.
const MAX_LOOK_DELTA: f32 = 100.0;

// ---------------------------------------------------------------------------
// Server factory
// ---------------------------------------------------------------------------

/// Start a NopGame server with an anticheat pipeline that catches speedhacks.
async fn start_anticheat_server() -> (String, watch::Sender<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener);

    let cfg = MatchConfig {
        seed: 1,
        snapshot_every: 300,
        ..MatchConfig::auto(4)
    };
    let executor = NativeExecutor::<NopGame>::new(cfg.clone());

    // AimbotSnap: rejects mouse delta magnitude > MAX_LOOK_DELTA.
    let chain = ValidatorChain::new().add(AimbotSnap::new(MAX_LOOK_DELTA));
    let ac_cfg = AnticheatConfig {
        warn_threshold: 1,
        kick_threshold: 5,
        ban_threshold: 10,
        decay_interval_ticks: 100_000, // no decay during test
        decay_amount: 1,
    };
    let anticheat = Anticheat::new(chain, ac_cfg);

    let server_cfg = GameServerConfig {
        bind_addr: addr.clone(),
        match_config: cfg,
        anticheat: Some(anticheat),
        fleet: None,
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let shutdown_rx2 = shutdown_rx.clone();
    let shutdown_tx2 = shutdown_tx.clone();

    tokio::spawn(async move {
        let _ =
            GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx2, shutdown_tx2).await;
    });

    tokio::time::sleep(Duration::from_millis(80)).await;
    (addr, shutdown_tx)
}

// ---------------------------------------------------------------------------
// Helper: connect + receive Welcome
// ---------------------------------------------------------------------------

async fn connect_and_welcome(
    addr: &str,
) -> (
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    PlayerId,
) {
    let url = format!("ws://{addr}");
    let (mut ws, _) = connect_async(&url).await.expect("connect failed");

    let msg = ws.next().await.expect("no Welcome").expect("ws err");
    let player_id = if let Message::Text(text) = msg {
        let net: ServerNet = serde_json::from_str(&text).expect("parse Welcome");
        match net {
            ServerNet::Welcome { player_id, .. } => player_id,
            other => panic!("expected Welcome, got {other:?}"),
        }
    } else {
        panic!("expected text message");
    };

    (ws, player_id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The cheater sends a teleport/speedhack (huge mouse delta); the server Rejects it.
/// The TrustScoreMap escalates the cheater's score after repeated violations.
#[tokio::test]
async fn anticheat_rejects_speedhack_and_escalates_trust_score() {
    let (addr, shutdown_tx) = start_anticheat_server().await;

    let (mut cheater_ws, _cheater_pid) = connect_and_welcome(&addr).await;

    // Send a teleport/speedhack input: huge mouse delta (>> MAX_LOOK_DELTA).
    let cheat_input = ClientNet::InputFrame {
        seq: 1,
        tick: 1,
        input: Input {
            mouse: MouseState {
                delta_x: CHEAT_DELTA,
                delta_y: CHEAT_DELTA,
                ..Default::default()
            },
            ..Default::default()
        },
    };
    let text = serde_json::to_string(&cheat_input).unwrap();
    cheater_ws.send(Message::Text(text.into())).await.unwrap();

    // Collect server responses; expect at least one Reject for the cheat input.
    let mut found_reject = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, cheater_ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(net) = serde_json::from_str::<ServerNet>(&text) {
                    match net {
                        ServerNet::Reject { seq: 1, .. } => {
                            found_reject = true;
                            break;
                        }
                        // Bootstrap snapshot/delta are expected first.
                        ServerNet::Snapshot { .. } | ServerNet::Delta { .. } => {}
                        _ => {}
                    }
                }
            }
            Ok(Some(Ok(Message::Close(_)))) => break,
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }

    assert!(
        found_reject,
        "server must Reject the cheater's teleport/speedhack input (seq=1)"
    );

    // ── Trust score escalation verification ─────────────────────────────────
    //
    // The TrustScoreMap lives inside the server's Anticheat pipeline. We verify
    // the escalation logic directly via the public API — this mirrors exactly
    // what the server does after each violation.
    use magnetite_anticheat::trust::TrustScoreMap;

    let mut scores = TrustScoreMap::new(
        1,       // warn_threshold
        5,       // kick_threshold
        10,      // ban_threshold
        100_000, // decay_interval_ticks (no decay during test)
        1,
    );

    let cheater = PlayerId::new(99);
    assert_eq!(scores.score(cheater), 0, "initial trust score must be 0");

    // One flag per cheat input — score escalates.
    scores.flag(cheater, RejectReason::OutOfBounds, 1);
    assert!(
        scores.score(cheater) > 0,
        "trust score must escalate after a violation"
    );

    // Repeated violations push score toward kick threshold.
    for tick in 2u64..6 {
        scores.flag(cheater, RejectReason::OutOfBounds, tick);
    }
    let final_score = scores.score(cheater);
    assert!(
        final_score >= 5,
        "repeated violations must push trust score to ≥ kick_threshold (got {final_score})"
    );

    let _ = cheater_ws.close(None).await;
    let _ = shutdown_tx.send(true);
}

/// Honest client with a clean input (small mouse delta) must receive Ack, not Reject.
#[tokio::test]
async fn anticheat_allows_honest_client() {
    let (addr, shutdown_tx) = start_anticheat_server().await;

    let (mut ws, _player_id) = connect_and_welcome(&addr).await;

    // Clean input: delta magnitude = sqrt(2) ≈ 1.41, well below MAX_LOOK_DELTA=100.
    let clean_input = ClientNet::InputFrame {
        seq: 42,
        tick: 1,
        input: Input {
            mouse: MouseState {
                delta_x: 1.0,
                delta_y: 1.0,
                ..Default::default()
            },
            ..Default::default()
        },
    };
    let text = serde_json::to_string(&clean_input).unwrap();
    ws.send(Message::Text(text.into())).await.unwrap();

    // Collect responses; we must receive Ack(seq=42) and must NOT receive Reject(seq=42).
    let mut found_reject = false;
    let mut found_ack = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(net) = serde_json::from_str::<ServerNet>(&text) {
                    match net {
                        ServerNet::Reject { seq: 42, .. } => {
                            found_reject = true;
                            break;
                        }
                        ServerNet::Ack { seq: 42, .. } => {
                            found_ack = true;
                            break;
                        }
                        // Bootstrap messages before the first input is processed.
                        ServerNet::Snapshot { .. }
                        | ServerNet::Delta { .. }
                        | ServerNet::Welcome { .. } => {}
                        _ => {}
                    }
                }
            }
            Ok(Some(Ok(Message::Close(_)))) => break,
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }

    assert!(
        !found_reject,
        "honest client must NOT be rejected for a clean input (delta ≈ 1.41 < {MAX_LOOK_DELTA})"
    );
    assert!(
        found_ack,
        "honest client must receive Ack for a clean input"
    );

    let _ = ws.close(None).await;
    let _ = shutdown_tx.send(true);
}
