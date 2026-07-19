//! End-to-end session follow, **over real sockets**.
//!
//! The unit tests in `cluster.rs` and `fleet.rs` prove the redirect mechanism in
//! isolation. These tests prove the wiring: a player connected to node A on a
//! real WebSocket, a shard that really migrates to node B over a real TCP
//! handoff, and the same player ending up attached on B — with the player id the
//! session started with.
//!
//! The negatives are the point of the file. A protocol that only works when
//! everyone is honest is not a security protocol, so each one drives an attack
//! through the same socket path the happy case uses:
//!
//! | Test | Attack |
//! |---|---|
//! | `a_forged_redirect_is_refused_by_the_client` | inject a redirect signed by an attacker |
//! | `an_expired_redirect_is_refused_end_to_end` | replay a lapsed redirect |
//! | `a_redirect_retargeted_at_another_player_is_refused` | steal a token, present it as someone else |
//! | `a_non_member_cannot_send_us_a_player` | a node outside the cluster mints a follow |
//! | `a_redirect_cannot_be_redeemed_twice` | replay a good redirect |
//! | `a_failed_migration_delivers_no_redirect_on_the_socket` | rolled-back migration must be silent |

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use magnetite_runtime::cluster::{ClusterMembership, Redirector, SignedRedirect};
use magnetite_runtime::fleet::{FleetNode, NetworkHandoffTransport, ShardAuthority};
use magnetite_runtime::follow::FleetSession;
use magnetite_runtime::shard::ShardId;
use magnetite_runtime::{GameServer, GameServerConfig};
use magnetite_sdk::authority::{
    AuthoritativeGame, MatchConfig, NativeExecutor, RejectReason, StepCtx, Tick,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::protocol::{ClientNet, ServerNet};
use magnetite_sdk::state::PlayerId;
use magnetite_seams::identity::RawKeypairAuth;

// ---------------------------------------------------------------------------
// A do-nothing game — these tests are about the session, not the simulation.
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

/// The shard every player lands on under `SingleRoom` / `Dedicated`.
const SHARD: ShardId = ShardId::LOCAL;

fn ident(seed: u8) -> Arc<RawKeypairAuth> {
    Arc::new(RawKeypairAuth::from_seed([seed; 32]))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Reserve an OS-assigned port and release it, so the server can bind it.
async fn free_addr() -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let a = l.local_addr().unwrap();
    drop(l);
    a.to_string()
}

/// A running game server with fleet wiring attached.
struct Node {
    ws_url: String,
    shutdown: watch::Sender<bool>,
}

impl Node {
    async fn start(fleet: FleetSession) -> Self {
        let addr = free_addr().await;
        let cfg = MatchConfig::auto(4);
        let executor = NativeExecutor::<NopGame>::new(cfg.clone());
        let server_cfg = GameServerConfig {
            bind_addr: addr.clone(),
            match_config: cfg,
            anticheat: None,
            fleet: Some(fleet),
        };
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let tx = shutdown_tx.clone();
        tokio::spawn(async move {
            let _ = GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx, tx).await;
        });
        // Give the listener a moment to bind.
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

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn open(url: &str) -> Ws {
    let (ws, _) = connect_async(url).await.expect("connect");
    ws
}

/// Read frames until one satisfies `pred`, or `budget` elapses.
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

async fn welcome_id(ws: &mut Ws) -> u64 {
    match recv_until(ws, Duration::from_secs(2), |m| {
        matches!(m, ServerNet::Welcome { .. })
    })
    .await
    {
        Some(ServerNet::Welcome { player_id, .. }) => player_id.as_u64(),
        other => panic!("expected Welcome, got {other:?}"),
    }
}

async fn send(ws: &mut Ws, frame: &ClientNet) {
    ws.send(Message::Text(serde_json::to_string(frame).unwrap()))
        .await
        .unwrap();
}

/// A two-node cluster: A is the redirect source, B the follow target.
struct Cluster {
    a_ident: Arc<RawKeypairAuth>,
    b_handoff: FleetNode,
    transport: Arc<Mutex<NetworkHandoffTransport>>,
    fleet_a: FleetSession,
    fleet_b: FleetSession,
    membership: ClusterMembership,
}

/// Build a cluster whose shard route points at `route_override`, or at B's real
/// handoff listener when `None`.
fn cluster(route_override: Option<(String, magnetite_seams::identity::PubKey)>) -> Cluster {
    let a = ident(200);
    let b = ident(201);
    let b_handoff = FleetNode::bind("127.0.0.1:0", Arc::clone(&b), None).unwrap();
    let membership = ClusterMembership::from_keys([a.node_pubkey(), b_handoff.pubkey()]);

    let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
        .with_timeout(Duration::from_millis(500))
        .with_membership(membership.clone())
        .with_redirects(Redirector::new());
    match route_override {
        Some((addr, key)) => {
            t.add_route(SHARD, magnetite_runtime::fleet::PeerRoute::new(addr, key));
        }
        None => {
            t.add_route(SHARD, b_handoff.route());
        }
    }
    t.authority().claim(SHARD, b"the world".to_vec());
    let authority_a = t.authority();
    let transport = Arc::new(Mutex::new(t));

    let fleet_a = FleetSession::new(Arc::clone(&a), authority_a, membership.clone())
        .with_transport(Arc::clone(&transport));
    // B's follow door reads the authority table the handoff listener writes
    // into — so `admit` is fenced on the epoch B *actually* owns.
    let fleet_b = FleetSession::new(Arc::clone(&b), b_handoff.authority(), membership.clone());

    Cluster {
        a_ident: a,
        b_handoff,
        transport,
        fleet_a,
        fleet_b,
        membership,
    }
}

impl Cluster {
    /// Run the real two-phase migration off the async runtime (it uses blocking
    /// sockets).
    async fn migrate(&self) -> Result<u64, String> {
        let t = Arc::clone(&self.transport);
        tokio::task::spawn_blocking(move || {
            t.lock()
                .unwrap()
                .migrate_shard(SHARD)
                .map_err(|e| e.to_string())
        })
        .await
        .unwrap()
    }
}

/// Wait for a redirect frame on a live session.
async fn await_redirect(ws: &mut Ws) -> Option<SignedRedirect> {
    match recv_until(ws, Duration::from_secs(3), |m| {
        matches!(m, ServerNet::Redirect { .. })
    })
    .await
    {
        Some(ServerNet::Redirect { redirect }) => {
            Some(serde_json::from_value(redirect).expect("redirect body parses"))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn player_session_follows_a_migrated_shard_over_real_sockets() {
    let c = cluster(None);
    let node_a = Node::start(c.fleet_a.clone()).await;
    let node_b = Node::start(c.fleet_b.clone()).await;

    // Two players join A, so the followed id is distinguishable from the id a
    // fresh join on B would have been handed.
    let mut p1 = open(&node_a.ws_url).await;
    let _ = welcome_id(&mut p1).await;
    let mut p2 = open(&node_a.ws_url).await;
    let p2_id = welcome_id(&mut p2).await;
    assert_eq!(p2_id, 2, "second player on A");

    // A tracks both of them against the shard, without anyone calling
    // `track_player` by hand: that is the wiring under test.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        c.fleet_a.tracked_players(SHARD),
        vec![1, 2],
        "the listener tracks connected players per shard"
    );

    // The shard really moves, over a real TCP handoff.
    let epoch = c.migrate().await.expect("migration commits");
    assert!(c.b_handoff.authority().owns(SHARD), "B owns the shard now");

    // The redirect arrives on the socket the player is already using.
    let redirect = await_redirect(&mut p2)
        .await
        .expect("player 2 receives a signed redirect");
    assert_eq!(redirect.player, p2_id);
    assert_eq!(redirect.epoch, epoch);

    // Client side: verify against the node key we are already talking to, and
    // pin the target key. An address is a hint; the key is the identity.
    let route = redirect
        .verify_for(&c.a_ident.node_pubkey(), p2_id, now_secs())
        .expect("redirect verifies");
    assert_eq!(route.pubkey, c.b_handoff.pubkey());

    // A closes the redirected session — nothing here is authoritative for that
    // shard any more.
    assert!(
        recv_until(&mut p2, Duration::from_millis(500), |_| true)
            .await
            .is_none(),
        "the source closes the redirected connection"
    );

    // Follow to B, proving B's key first, then presenting the redirect.
    let mut f = open(&node_b.ws_url).await;
    let _provisional = welcome_id(&mut f).await;
    send(
        &mut f,
        &ClientNet::Hello {
            nonce: "n-1".into(),
        },
    )
    .await;
    let ident_frame = recv_until(&mut f, Duration::from_secs(2), |m| {
        matches!(m, ServerNet::NodeIdentity { .. })
    })
    .await
    .expect("B proves its node key");
    match ident_frame {
        ServerNet::NodeIdentity { node_key, nonce, .. } => {
            assert_eq!(
                node_key,
                route.pubkey.to_hex(),
                "the far side presents the pinned target key"
            );
            assert_eq!(nonce, "n-1");
        }
        _ => unreachable!(),
    }

    send(
        &mut f,
        &ClientNet::Follow {
            redirect: serde_json::to_value(&redirect).unwrap(),
        },
    )
    .await;

    // Continuity: B welcomes the player back under their ORIGINAL id.
    let followed = welcome_id(&mut f).await;
    assert_eq!(
        followed, p2_id,
        "the session is continuous — same player id on the new owner"
    );
    assert_eq!(c.fleet_b.redeemed_count(), 1);

    // And the session is live: inputs are accepted on B.
    send(
        &mut f,
        &ClientNet::InputFrame {
            seq: 1,
            tick: 0,
            input: Input::default(),
        },
    )
    .await;
    assert!(
        recv_until(&mut f, Duration::from_secs(2), |m| matches!(
            m,
            ServerNet::Ack { .. } | ServerNet::Snapshot { .. } | ServerNet::Delta { .. }
        ))
        .await
        .is_some(),
        "the followed session is ticking on B"
    );
}

// ---------------------------------------------------------------------------
// Negatives
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_forged_redirect_is_refused_by_the_client() {
    // The attacker can inject a frame but cannot sign as the node the client
    // authenticated. This is the whole reason the redirect is signed.
    let c = cluster(None);
    let attacker = ident(66);
    let route = magnetite_runtime::fleet::PeerRoute::new(
        "attacker.example:9999",
        attacker.node_pubkey(),
    );
    let forged = SignedRedirect::mint(&attacker, 1, SHARD, 1, &route, now_secs(), 30);

    let err = forged
        .verify_for(&c.a_ident.node_pubkey(), 1, now_secs())
        .expect_err("a redirect from anyone but our node must be discarded");
    assert!(
        matches!(
            err,
            magnetite_runtime::cluster::RedirectError::WrongIssuer { .. }
        ),
        "got {err:?}"
    );

    // And even if a client did follow it, B refuses the token: the issuer is
    // not a cluster member.
    let node_b = Node::start(c.fleet_b.clone()).await;
    let mut f = open(&node_b.ws_url).await;
    let _ = welcome_id(&mut f).await;
    send(
        &mut f,
        &ClientNet::Follow {
            redirect: serde_json::to_value(&forged).unwrap(),
        },
    )
    .await;
    assert!(
        closed(&mut f).await,
        "a refused follow closes the connection — no degraded fallback session"
    );
}

#[tokio::test]
async fn an_expired_redirect_is_refused_end_to_end() {
    let c = cluster(None);
    let node_b = Node::start(c.fleet_b.clone()).await;
    let epoch = c.migrate().await.expect("migration commits");

    // Mint with a TTL that has already lapsed.
    let stale = SignedRedirect::mint(
        &c.a_ident,
        1,
        SHARD,
        epoch,
        &c.b_handoff.route(),
        now_secs() - 600,
        30,
    );

    // The client refuses it…
    assert!(matches!(
        stale.verify_for(&c.a_ident.node_pubkey(), 1, now_secs()),
        Err(magnetite_runtime::cluster::RedirectError::Expired)
    ));

    // …and so does the target, independently. Neither side is trusted to be the
    // only one checking.
    let mut f = open(&node_b.ws_url).await;
    let _ = welcome_id(&mut f).await;
    send(
        &mut f,
        &ClientNet::Follow {
            redirect: serde_json::to_value(&stale).unwrap(),
        },
    )
    .await;
    assert!(closed(&mut f).await, "an expired follow is refused at B");
}

#[tokio::test]
async fn a_redirect_retargeted_at_another_player_is_refused() {
    // Steal player 1's redirect and try to ride it in as player 9 by rewriting
    // the envelope. The token underneath still names player 1, and the envelope
    // is signed, so both the signature and the player binding fail.
    let c = cluster(None);
    let node_b = Node::start(c.fleet_b.clone()).await;
    let epoch = c.migrate().await.expect("migration commits");

    let good = SignedRedirect::mint(
        &c.a_ident,
        1,
        SHARD,
        epoch,
        &c.b_handoff.route(),
        now_secs(),
        30,
    );
    let mut body = serde_json::to_value(&good).unwrap();
    body["player"] = serde_json::json!(9);

    let mut f = open(&node_b.ws_url).await;
    let _ = welcome_id(&mut f).await;
    send(&mut f, &ClientNet::Follow { redirect: body }).await;
    assert!(
        closed(&mut f).await,
        "a token minted for another player admits nobody"
    );
    assert_eq!(
        c.fleet_b.redeemed_count(),
        0,
        "a refused token is never consumed"
    );
}

#[tokio::test]
async fn a_non_member_cannot_send_us_a_player() {
    let c = cluster(None);
    let node_b = Node::start(c.fleet_b.clone()).await;
    let epoch = c.migrate().await.expect("migration commits");
    assert!(c.membership.contains(&c.a_ident.node_pubkey()));

    // A perfectly well-formed redirect — signed by a node the operator never
    // authorized. Announcing is not joining.
    let outsider = ident(77);
    let r = SignedRedirect::mint(
        &outsider,
        1,
        SHARD,
        epoch,
        &c.b_handoff.route(),
        now_secs(),
        30,
    );

    let mut f = open(&node_b.ws_url).await;
    let _ = welcome_id(&mut f).await;
    send(
        &mut f,
        &ClientNet::Follow {
            redirect: serde_json::to_value(&r).unwrap(),
        },
    )
    .await;
    assert!(closed(&mut f).await, "membership is deny-by-default");
}

#[tokio::test]
async fn a_redirect_cannot_be_redeemed_twice() {
    let c = cluster(None);
    let node_b = Node::start(c.fleet_b.clone()).await;
    let epoch = c.migrate().await.expect("migration commits");
    let r = SignedRedirect::mint(
        &c.a_ident,
        1,
        SHARD,
        epoch,
        &c.b_handoff.route(),
        now_secs(),
        30,
    );
    let body = serde_json::to_value(&r).unwrap();

    let mut first = open(&node_b.ws_url).await;
    let _ = welcome_id(&mut first).await;
    send(
        &mut first,
        &ClientNet::Follow {
            redirect: body.clone(),
        },
    )
    .await;
    assert_eq!(welcome_id(&mut first).await, 1, "first follow is admitted");

    let mut second = open(&node_b.ws_url).await;
    let _ = welcome_id(&mut second).await;
    send(&mut second, &ClientNet::Follow { redirect: body }).await;
    assert!(
        closed(&mut second).await,
        "the nonce is one-shot — a replayed redirect admits nobody"
    );
}

#[tokio::test]
async fn a_failed_migration_delivers_no_redirect_on_the_socket() {
    // Route the shard at a port nobody is listening on: the handoff cannot
    // commit, so the source keeps authority — and the player must hear nothing.
    let b_key = ident(201).node_pubkey();
    let dead = free_addr().await;
    let c = cluster(Some((dead, b_key)));
    let node_a = Node::start(c.fleet_a.clone()).await;

    let mut p = open(&node_a.ws_url).await;
    let id = welcome_id(&mut p).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(c.fleet_a.tracked_players(SHARD), vec![id]);

    assert!(c.migrate().await.is_err(), "the migration must fail");
    assert!(
        c.transport.lock().unwrap().authority().owns(SHARD),
        "a failed migration leaves authority with the source"
    );
    assert!(
        await_redirect(&mut p).await.is_none(),
        "a rolled-back migration must never redirect a live session"
    );
    // The session is untouched: still connected, still on this node.
    assert_eq!(c.fleet_a.tracked_players(SHARD), vec![id]);
}

/// True when the server closed the connection (or stopped talking entirely).
async fn closed(ws: &mut Ws) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return false;
        }
        match tokio::time::timeout(left, ws.next()).await {
            Ok(None) | Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) => return true,
            Ok(Some(Ok(Message::Text(t)))) => {
                // A second Welcome would mean it was admitted — that is a
                // failure, not a close.
                if let Ok(ServerNet::Welcome { .. }) = serde_json::from_str::<ServerNet>(&t) {
                    return false;
                }
            }
            Ok(Some(Ok(_))) => {}
            Err(_) => return false,
        }
    }
}
