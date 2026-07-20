//! Attested-input wire ingress, **over a real socket**.
//!
//! The unit tests in `attested.rs` prove the route in isolation. This file
//! proves the wiring, because that is exactly what was missing: seam §3.7 has
//! had `AttestedEventInput` for a while, but `ClientNet` had no variant and the
//! server had no arm, so a browser client emitting a correctly signed event was
//! talking to an open socket that silently dropped every frame. "Silently" is
//! the operative word — there was no way for the client to tell the difference
//! between ingestion and a black hole.
//!
//! So each test here sends real JSON bytes down a real WebSocket to a real
//! `GameServer` and asserts on the frame that comes back:
//!
//! | Test | What it pins |
//! |---|---|
//! | `a_signed_event_sent_over_a_real_socket_is_acked` | ingestion actually happens |
//! | `the_clients_golden_frame_is_answered_not_ignored` | the client's exact bytes are understood |
//! | `a_bad_signature_is_rejected_over_the_socket` | forgery refused, and *said so* |
//! | `an_unsigned_attested_frame_is_refused_explicitly` | the unsigned shape fails closed |
//! | `an_attested_flood_is_rate_limited_over_the_socket` | flooding is not a DoS |
//! | `an_attested_frame_does_not_disturb_the_deterministic_path` | the class boundary holds on the wire |
//!
//! # What this does not show
//!
//! That any of the admitted events are *true*. They are client-attested sensor
//! claims; a signature proves authorship and the gate proves only that a claim
//! was not physically impossible. Nothing here is anti-cheat.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use magnetite_runtime::{GameServer, GameServerConfig};
use magnetite_sdk::authority::{
    AuthoritativeGame, MatchConfig, NativeExecutor, RejectReason, StepCtx, Tick,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::protocol::{ClientNet, ServerNet};
use magnetite_sdk::state::PlayerId;
use magnetite_seams::identity::{Identity, RawKeypairAuth};
use magnetite_seams::input::{AttestedEvent, SignedAttestedEvent};

// ---------------------------------------------------------------------------
// A do-nothing game — these tests are about the wire, not the simulation.
// ---------------------------------------------------------------------------

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
// Harness
// ---------------------------------------------------------------------------

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The client's golden frame, verbatim. Identical to the fixture in wibbly's
/// `packages/wibbly-magnetite/test/wire.test.ts`; the `sig` was produced by
/// `RawKeypairAuth::from_seed([7u8; 32])`.
const GOLDEN_FRAME: &str = r#"{"type":"attested_event","signed":{"event":{"player":"ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c","kind":"swing","confidence":0.725,"vector":[0.125,-0.0625,0.0],"speed_mps":6.5,"t_capture_ms":1763000000123,"seq":42},"player_key":"ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c","sig":"77bb88c4c43f147b5ff8749d9a22c6e275ae34564ed7bc1c4dc8bd5d28b05ef57f5c1ed0af1d2088c6e713bf01ab36c7a5112855e054a0c2bae11ae92f685e00"}}"#;

struct Node {
    ws_url: String,
    shutdown: watch::Sender<bool>,
}

impl Node {
    async fn start() -> Self {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        drop(l);

        let cfg = MatchConfig::auto(4);
        let executor = NativeExecutor::<NopGame>::new(cfg.clone());
        let server_cfg = GameServerConfig {
            bind_addr: addr.clone(),
            match_config: cfg,
            anticheat: None,
            fleet: None,
        };
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let tx = shutdown_tx.clone();
        tokio::spawn(async move {
            let _ = GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx, tx).await;
        });
        tokio::time::sleep(Duration::from_millis(80)).await;
        Node {
            ws_url: format!("ws://{addr}"),
            shutdown: shutdown_tx,
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

async fn open(url: &str) -> Ws {
    let (ws, _) = connect_async(url).await.expect("connect");
    ws
}

async fn recv_until<F>(ws: &mut Ws, budget: Duration, mut pred: F) -> Option<ServerNet>
where
    F: FnMut(&ServerNet) -> bool,
{
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                if let Ok(msg) = serde_json::from_str::<ServerNet>(&t) {
                    if pred(&msg) {
                        return Some(msg);
                    }
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return None,
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(_))) | Err(_) => return None,
        }
    }
}

/// Wait for the `Welcome` so the connection is fully established first.
async fn welcomed(url: &str) -> Ws {
    let mut ws = open(url).await;
    recv_until(&mut ws, Duration::from_secs(2), |m| {
        matches!(m, ServerNet::Welcome { .. })
    })
    .await
    .expect("Welcome");
    ws
}

async fn send_raw(ws: &mut Ws, text: &str) {
    ws.send(Message::Text(text.into())).await.unwrap();
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A freshly signed event with a current capture time. The golden fixture's
/// timestamp is fixed so its signature stays reproducible, but a live server
/// screens capture time against the real clock, so the happy path needs "now".
fn fresh(seed: u8, kind: &str, seq: u64) -> ClientNet {
    let k = RawKeypairAuth::from_seed([seed; 32]);
    let signed = SignedAttestedEvent::sign(
        &k,
        AttestedEvent {
            player: k.pubkey(),
            kind: kind.into(),
            confidence: 0.9,
            vector: Some([1.0, 0.0, 0.0]),
            speed_mps: Some(6.4),
            t_capture_ms: now_ms(),
            seq,
        },
    );
    ClientNet::AttestedEvent {
        signed: Box::new(signed),
    }
}

async fn send_frame(ws: &mut Ws, frame: &ClientNet) {
    send_raw(ws, &serde_json::to_string(frame).unwrap()).await;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The acceptance test. Before this change the identical byte sequence produced
/// no response at all.
#[tokio::test]
async fn a_signed_event_sent_over_a_real_socket_is_acked() {
    let node = Node::start().await;
    let mut ws = welcomed(&node.ws_url).await;

    send_frame(&mut ws, &fresh(21, "swing", 1)).await;

    let reply = recv_until(&mut ws, Duration::from_secs(2), |m| {
        matches!(
            m,
            ServerNet::AttestedAck { .. } | ServerNet::AttestedReject { .. }
        )
    })
    .await
    .expect("an attested frame must be answered, not silently dropped");

    match reply {
        ServerNet::AttestedAck { seq } => assert_eq!(seq, 1),
        other => panic!("expected AttestedAck, got {other:?}"),
    }
}

/// The client's exact bytes reach a route that understands them.
///
/// The golden fixture's `t_capture_ms` is a fixed date, so a live server screens
/// it as stale — and that is the *proof of ingestion*: a stale-timestamp refusal
/// can only be produced by code that parsed the frame, verified its signature,
/// and ran it through the plausibility gate. Silence was the old behaviour.
#[tokio::test]
async fn the_clients_golden_frame_is_answered_not_ignored() {
    let node = Node::start().await;
    let mut ws = welcomed(&node.ws_url).await;

    send_raw(&mut ws, GOLDEN_FRAME).await;

    let reply = recv_until(&mut ws, Duration::from_secs(2), |m| {
        matches!(
            m,
            ServerNet::AttestedAck { .. } | ServerNet::AttestedReject { .. }
        )
    })
    .await
    .expect("the client's golden frame must be understood, not ignored");

    match reply {
        ServerNet::AttestedReject { seq, reason } => {
            assert_eq!(seq, 42, "the server recovered the event's own sequence");
            assert!(
                reason.contains("stale"),
                "the fixture is dated, so the gate should refuse it as stale, not as \
                 malformed or badly signed — got {reason:?}"
            );
        }
        other => panic!("expected AttestedReject, got {other:?}"),
    }
}

#[tokio::test]
async fn a_bad_signature_is_rejected_over_the_socket() {
    let node = Node::start().await;
    let mut ws = welcomed(&node.ws_url).await;

    // Edit the claim, keep the signature — the edit a relay in the middle makes.
    let ClientNet::AttestedEvent { mut signed } = fresh(22, "swing", 1) else {
        unreachable!()
    };
    signed.event.speed_mps = Some(12.0);
    send_frame(&mut ws, &ClientNet::AttestedEvent { signed }).await;

    let reply = recv_until(&mut ws, Duration::from_secs(2), |m| {
        matches!(
            m,
            ServerNet::AttestedAck { .. } | ServerNet::AttestedReject { .. }
        )
    })
    .await
    .expect("answered");

    match reply {
        ServerNet::AttestedReject { reason, .. } => assert!(
            reason.contains("signature"),
            "expected a signature refusal, got {reason:?}"
        ),
        other => panic!("a tampered event must not be acked, got {other:?}"),
    }
}

/// wibbly's `AttestedFrameUnsigned` shape. It carries no authorship binding, so
/// it must fail closed — and be *told* it failed, not left guessing.
#[tokio::test]
async fn an_unsigned_attested_frame_is_refused_explicitly() {
    let node = Node::start().await;
    let mut ws = welcomed(&node.ws_url).await;

    send_raw(
        &mut ws,
        r#"{"type":"attested_event","event":{"player":"ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c","kind":"swing","confidence":0.9,"vector":null,"speed_mps":null,"t_capture_ms":1763000000123,"seq":1}}"#,
    )
    .await;

    let reply = recv_until(&mut ws, Duration::from_secs(2), |m| {
        matches!(
            m,
            ServerNet::AttestedAck { .. } | ServerNet::AttestedReject { .. }
        )
    })
    .await
    .expect("an unsigned attested frame must be refused out loud");

    match reply {
        ServerNet::AttestedReject { seq, reason } => {
            assert_eq!(seq, 0, "nothing recoverable from a frame that did not parse");
            assert!(
                reason.contains("malformed"),
                "expected a malformed refusal, got {reason:?}"
            );
        }
        other => panic!("an unsigned event must never be acked, got {other:?}"),
    }
}

#[tokio::test]
async fn an_attested_flood_is_rate_limited_over_the_socket() {
    let node = Node::start().await;
    let mut ws = welcomed(&node.ws_url).await;

    // Every frame is correctly signed; the connection ceiling is what stops it.
    for seq in 1..400u64 {
        send_frame(&mut ws, &fresh(23, "swing", seq)).await;
    }

    let limited = recv_until(&mut ws, Duration::from_secs(5), |m| {
        matches!(m, ServerNet::AttestedReject { reason, .. } if reason.contains("exceed"))
    })
    .await;

    assert!(
        limited.is_some(),
        "a flood of attested frames must hit the connection rate limit"
    );
}

/// The class boundary, on the wire. An attested frame and a deterministic input
/// frame on the same socket must be answered on their own channels: an attested
/// event must never produce a `ServerNet::Ack`/`Reject`, because those drive the
/// client's `PredictionBuffer` and belong to the replay-verifiable path alone.
#[tokio::test]
async fn an_attested_frame_does_not_disturb_the_deterministic_path() {
    let node = Node::start().await;
    let mut ws = welcomed(&node.ws_url).await;

    send_frame(&mut ws, &fresh(24, "swing", 1)).await;

    let reply = recv_until(&mut ws, Duration::from_secs(2), |m| {
        matches!(
            m,
            ServerNet::AttestedAck { .. }
                | ServerNet::AttestedReject { .. }
                | ServerNet::Ack { .. }
                | ServerNet::Reject { .. }
        )
    })
    .await
    .expect("answered");

    assert!(
        matches!(reply, ServerNet::AttestedAck { .. }),
        "an attested event answered on the deterministic Ack/Reject channel would \
         corrupt prediction state and blur the class boundary — got {reply:?}"
    );
}
