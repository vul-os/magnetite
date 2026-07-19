//! Live HTTP tests against the **real axum router** (DECENTRALIZATION.md §3.4).
//!
//! Why this file exists
//! ────────────────────
//! The offline e2e proof (`magnetite-e2e/tests/decentralized_loop.rs`) drives
//! the same envelope and the same parser in-process, which proves the *protocol*
//! but says nothing about the axum wiring: whether the routes are actually
//! mounted at the paths the node client posts to, whether `#[serde(flatten)]`
//! survives a real request body, whether a rejected announce becomes a 4xx on
//! the wire. Those are exactly the seams where a "beautiful but not wired"
//! surface breaks. So these tests bind a **real TCP socket on an ephemeral
//! port** and speak HTTP to it with `reqwest`.
//!
//! What is and is not covered — stated plainly
//! ───────────────────────────────────────────
//! The discovery router needs a `PgPool`. We build one with `connect_lazy`,
//! which constructs the pool **without** connecting, so no Postgres is required
//! to run these tests (the guardrail is: everything runs offline). That splits
//! the handlers cleanly in two:
//!
//! * **Fully covered — every rejection path.** Signature verification, lease
//!   validation and content validation all run *before* any query is issued, so
//!   a forged/unsigned/expired/garbage announce is refused end-to-end over real
//!   HTTP with the database never touched. This is the security-relevant half,
//!   and it is the half a fail-open bug would live in.
//! * **Reachability only — the success path.** An honest announce gets past
//!   verification and then fails at the database. We assert it reaches the DB
//!   layer (i.e. it was accepted by every check) rather than pretending it
//!   returns 200. Asserting a stored-and-served round trip genuinely requires
//!   Postgres; faking it with an in-memory store would test a fake, not the
//!   router, so we do not.
//!
//! The distinction matters: "an unsigned announce is refused over HTTP" is
//! proven here. "A signed announce is persisted and later served" is not, and is
//! left to an integration environment with a database.

use std::net::SocketAddr;

use magnetite_backend::api::discovery::router;
use magnetite_seams::blobstore::Hash;
use magnetite_seams::comms::RoomAddr;
use magnetite_seams::discovery::{
    Capacity, NodeAddr, Price, SessionAd, SignedAd, SignedWithdraw, MAX_AD_TTL_SECS,
};
use magnetite_seams::identity::{Identity, RawKeypairAuth};
use sqlx::postgres::PgPoolOptions;

/// Boot the real discovery router on an ephemeral port. Returns its base URL.
///
/// The pool is lazy: constructing it performs no I/O and contacts no server, so
/// this needs no Postgres and no network beyond loopback.
async fn spawn_router() -> String {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        // A short timeout so the "reaches the DB layer" assertions do not hang.
        .acquire_timeout(std::time::Duration::from_millis(700))
        .connect_lazy("postgres://discovery-tests-never-connect@127.0.0.1:1/none")
        .expect("a lazy pool is just a config object");

    let app = router(pool);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind an ephemeral port");
    let addr: SocketAddr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    format!("http://{addr}")
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sample_ad(node: &str) -> SessionAd {
    SessionAd {
        game: Hash::of(b"a content-addressed game"),
        node: NodeAddr(node.into()),
        operator: Some("nordfjord".into()),
        region: Some("eu-north".into()),
        capacity: Capacity {
            cpu_cores: 8,
            ram_mb: 32768,
            bandwidth_mbps: 1000,
            free_slots: 6,
            max_shards: 8,
        },
        ping_hint: 21,
        price: Some(Price {
            amount: 500,
            currency: "USDC".into(),
            unit: "per_seat".into(),
        }),
        chat_room: Some(RoomAddr("builtin://room/lobby".into())),
        voice_room: None,
    }
}

/// POST an announce body and return the HTTP status.
async fn post_announce(base: &str, body: &serde_json::Value) -> reqwest::StatusCode {
    reqwest::Client::new()
        .post(format!("{base}/announce"))
        .json(body)
        .send()
        .await
        .expect("the router answered")
        .status()
}

/// A status that means "verification passed, we got as far as the database".
/// With no Postgres behind the lazy pool that surfaces as a 5xx.
fn reached_the_database(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
}

// ---------------------------------------------------------------------------
// Negative paths — fully proven over real HTTP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_unsigned_announce_is_refused_over_real_http() {
    let base = spawn_router().await;
    let honest = RawKeypairAuth::from_seed([3u8; 32]);

    // A well-formed envelope whose signature bytes are zeroed — the "I did not
    // sign this at all" case. A phonebook that accepts it lets anyone list a box
    // they do not run.
    let mut unsigned = SignedAd::sign(&honest, sample_ad("box:9000"), now_unix(), 120);
    unsigned.sig = magnetite_seams::identity::Sig([0u8; 64]);

    let status = post_announce(&base, &serde_json::to_value(&unsigned).unwrap()).await;
    assert!(
        status.is_client_error(),
        "an unsigned announce must be refused by the router, got {status}"
    );
    assert!(
        !reached_the_database(status),
        "it must be rejected BEFORE any database work — got {status}"
    );
}

#[tokio::test]
async fn a_forged_announce_is_refused_over_real_http() {
    let base = spawn_router().await;
    let honest = RawKeypairAuth::from_seed([3u8; 32]);
    let attacker = RawKeypairAuth::from_seed([4u8; 32]);

    // (a) Body edited after signing: undercutting the advertised price.
    let mut tampered = SignedAd::sign(&honest, sample_ad("box:9000"), now_unix(), 120);
    tampered.ad.price = None;
    let status = post_announce(&base, &serde_json::to_value(&tampered).unwrap()).await;
    assert!(status.is_client_error(), "price edit accepted?! got {status}");

    // (b) The attacker's own ad, relabelled with the honest node's key.
    let mut spoofed = SignedAd::sign(&attacker, sample_ad("box:9000"), now_unix(), 120);
    spoofed.node_key = honest.pubkey();
    let status = post_announce(&base, &serde_json::to_value(&spoofed).unwrap()).await;
    assert!(status.is_client_error(), "key relabel accepted?! got {status}");

    // (c) Node-declared labels rewritten in flight by a relay.
    let mut relabelled = SignedAd::sign(&honest, sample_ad("box:9000"), now_unix(), 120);
    relabelled.ad.operator = Some("someone-else".into());
    let status = post_announce(&base, &serde_json::to_value(&relabelled).unwrap()).await;
    assert!(status.is_client_error(), "operator edit accepted?! got {status}");
}

#[tokio::test]
async fn an_expired_or_overlong_lease_is_refused_over_real_http() {
    let base = spawn_router().await;
    let node = RawKeypairAuth::from_seed([3u8; 32]);

    // A lease that lapsed an hour ago.
    let stale = SignedAd::sign(&node, sample_ad("box:9000"), now_unix() - 7_200, 120);
    let status = post_announce(&base, &serde_json::to_value(&stale).unwrap()).await;
    assert!(status.is_client_error(), "lapsed lease accepted?! got {status}");

    // A lease longer than the protocol maximum — squatting the phonebook.
    let squat = SignedAd::sign(
        &node,
        sample_ad("box:9000"),
        now_unix(),
        MAX_AD_TTL_SECS + 60,
    );
    let status = post_announce(&base, &serde_json::to_value(&squat).unwrap()).await;
    assert!(status.is_client_error(), "over-long lease accepted?! got {status}");
}

#[tokio::test]
async fn garbage_bodies_are_refused_rather_than_guessed_at() {
    let base = spawn_router().await;

    for body in [
        serde_json::json!({}),
        serde_json::json!({ "ad": "not an object" }),
        serde_json::json!({ "hello": "world" }),
    ] {
        let status = post_announce(&base, &body).await;
        assert!(
            status.is_client_error(),
            "a malformed announce must 4xx, got {status} for {body}"
        );
    }
}

#[tokio::test]
async fn a_bad_game_hash_in_the_query_is_refused_before_the_database() {
    let base = spawn_router().await;

    let status = reqwest::get(format!("{base}/sessions?game=not-a-blake3-hash"))
        .await
        .expect("the router answered")
        .status();
    assert!(
        status.is_client_error(),
        "a non-hex game filter is a bad request, got {status}"
    );
    assert!(
        !reached_the_database(status),
        "validated before any query is issued, got {status}"
    );
}

#[tokio::test]
async fn a_forged_withdrawal_cannot_retract_another_nodes_ad() {
    let base = spawn_router().await;
    let honest = RawKeypairAuth::from_seed([3u8; 32]);
    let attacker = RawKeypairAuth::from_seed([4u8; 32]);
    let game = Hash::of(b"a content-addressed game");

    // The attacker signs a withdrawal, then relabels it as the honest node's.
    let mut forged =
        SignedWithdraw::sign(&attacker, game, NodeAddr("box:9000".into()), now_unix());
    forged.node_key = honest.pubkey();

    let status = reqwest::Client::new()
        .delete(format!("{base}/announce"))
        .json(&forged)
        .send()
        .await
        .expect("the router answered")
        .status();
    assert!(
        status.is_client_error(),
        "nobody may retract another node's listing, got {status}"
    );
    assert!(!reached_the_database(status), "refused before the delete runs");
}

// ---------------------------------------------------------------------------
// Reachability — the routes exist and honest traffic gets through the checks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_routes_are_mounted_where_a_node_actually_posts() {
    let base = spawn_router().await;
    let node = RawKeypairAuth::from_seed([3u8; 32]);
    let signed = SignedAd::sign(&node, sample_ad("box:9000"), now_unix(), 120);

    // The exact wire form `HttpTrackerClient` sends: the flattened SignedAd plus
    // the unsigned display counters.
    let mut body = serde_json::to_value(&signed).unwrap();
    body["players"] = serde_json::json!(3);
    body["max_players"] = serde_json::json!(16);

    let status = post_announce(&base, &body).await;
    assert_ne!(
        status,
        reqwest::StatusCode::NOT_FOUND,
        "POST /announce must be mounted — a 404 here means the node client and \
         the router disagree about the path"
    );
    assert!(
        reached_the_database(status),
        "an honest, fully-signed announce must survive every check and reach the \
         storage layer; it only fails here because no Postgres is attached. Got {status}"
    );
}

#[tokio::test]
async fn the_sessions_route_is_mounted_and_accepts_the_browser_query() {
    let base = spawn_router().await;

    // Exactly the query string the server browser builds.
    let url = format!(
        "{base}/sessions?game={}&max_ping=80&free_slots_only=true&free_only=false&limit=50",
        Hash::of(b"a content-addressed game").to_hex()
    );
    let status = reqwest::get(&url).await.expect("the router answered").status();

    assert_ne!(status, reqwest::StatusCode::NOT_FOUND, "GET /sessions must be mounted");
    assert_ne!(
        status,
        reqwest::StatusCode::BAD_REQUEST,
        "the browser's own filter query must parse — a 400 here is a frontend/backend \
         contract break"
    );
    assert!(
        reached_the_database(status),
        "a well-formed query reaches the storage layer, got {status}"
    );
}
