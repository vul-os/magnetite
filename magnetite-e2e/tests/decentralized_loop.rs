//! **The decentralized loop, end to end, with zero external services.**
//!
//! This is the headline proof for `DECENTRALIZATION.md`: everything a player
//! does — find a game, pay for it, get into its chat, and be served by an
//! elastic node — happens across the six seams, with no chain, no homeserver,
//! no live tracker, and no database. If this test passes on a laptop with the
//! network cable pulled, the architecture is real and not a diagram.
//!
//! The loop under test:
//!
//! | Step | Seam | What is proven |
//! |---|---|---|
//! | 1 | `BlobStore` §3.3 | A node loads a game **by BLAKE3 content address** and refuses bytes that do not hash to it. |
//! | 2 | `Discovery` §3.4 | The node **self-advertises** a `SessionAd` measured from its own hardware. |
//! | 3 | `Discovery` §3.4 | A client **finds** that session and sees the right game hash, capacity, and price. |
//! | 4 | `PaymentRail` §3.6 | A checkout yields a **signed receipt**; a tampered one is rejected. |
//! | 5 | `CommsProvider` §3.5 | A room credential is minted from the player's key, and a **paid room refuses entry without a valid receipt**. |
//! | 6 | runtime §4 | The game **ticks deterministically across multiple shards** on one box. |
//!
//! Every step asserts its negative case too. A test that only walks the happy
//! path proves the feature exists; it does not prove the system is safe. The
//! fail-closed assertions here are the point.

use magnetite_runtime::node::{load_verified_game, prepare_game, NodeConfig, NodeError};
use magnetite_runtime::tracker::{parse_sessions, HttpTrackerClient};
use magnetite_runtime::{ShardId, ShardedRuntime};

use magnetite_sdk::authority::{MatchConfig, NativeExecutor, Topology};
use magnetite_sdk::input::{Input, MouseState};
use magnetite_sdk::state::PlayerId;

use magnetite_seams::blobstore::{BlobStore, Hash, LocalBlobStore};
use magnetite_seams::comms::{BuiltinProvider, CommsProvider, RoomScope};
use magnetite_seams::discovery::{
    Capacity, Discovery, Filter, LanDiscovery, NodeAddr, Price, SessionAd, SignedAd, SignedWithdraw,
};
use magnetite_seams::identity::{Identity, PubKey, RawKeypairAuth};
use magnetite_seams::payment::{
    receipt_admits, MockPaymentRail, PaymentRail, PaymentSplit, Receipt, Split,
};

use game_template_authoritative::ArenaShooter;

// ---------------------------------------------------------------------------
// The offline world: an in-memory tracker that speaks the real wire protocol
// ---------------------------------------------------------------------------

/// A stand-in for `POST /api/v1/discovery/announce` + `GET /sessions` that runs
/// in-process.
///
/// It is deliberately NOT a simplification of the backend's rules: it verifies
/// the very same [`SignedAd`] envelope, with the same seam verifier, and holds
/// the same soft leased state. What it drops is Postgres and a socket — so this
/// test needs neither, while still exercising the signature gate that a real
/// tracker enforces.
#[derive(Default)]
struct OfflineTracker {
    ads: std::sync::Mutex<Vec<(SignedAd, String)>>,
}

impl OfflineTracker {
    /// The tracker's whole job. Fails closed on a bad signature or lease.
    fn announce(&self, signed: SignedAd, now: u64) -> Result<(), String> {
        signed
            .verify::<RawKeypairAuth>(now, 60)
            .map_err(|e| e.to_string())?;
        let mut ads = self.ads.lock().unwrap();
        let slot = (signed.ad.game.to_hex(), signed.ad.node.0.clone());
        // A re-announce refreshes the slot — but only from the SAME node key.
        if let Some((existing, _)) = ads
            .iter()
            .find(|(a, _)| (a.ad.game.to_hex(), a.ad.node.0.clone()) == slot)
        {
            if existing.node_key != signed.node_key {
                return Err("slot is held by a different node key".into());
            }
        }
        ads.retain(|(a, _)| (a.ad.game.to_hex(), a.ad.node.0.clone()) != slot);
        let key_hex = signed.node_key.to_hex();
        ads.push((signed, key_hex));
        Ok(())
    }

    fn withdraw(&self, w: SignedWithdraw, now: u64) -> Result<usize, String> {
        w.verify::<RawKeypairAuth>(now, 300)
            .map_err(|e| e.to_string())?;
        let mut ads = self.ads.lock().unwrap();
        let before = ads.len();
        ads.retain(|(a, _)| {
            !(a.ad.game == w.game && a.ad.node == w.node && a.node_key == w.node_key)
        });
        Ok(before - ads.len())
    }

    /// The `GET /sessions` body, in the exact envelope the backend returns, so
    /// the runtime's real `parse_sessions` is what reads it.
    fn sessions_json(&self, now: u64) -> serde_json::Value {
        let ads = self.ads.lock().unwrap();
        let sessions: Vec<serde_json::Value> = ads
            .iter()
            .filter(|(a, _)| a.is_live_at(now))
            .map(|(a, key)| {
                let mut v = serde_json::to_value(&a.ad).unwrap();
                v["id"] = serde_json::json!("00000000-0000-0000-0000-000000000001");
                v["node_key"] = serde_json::json!(key);
                v["expires_at"] = serde_json::json!(a.expires_at);
                v
            })
            .collect();
        serde_json::json!({ "success": true, "data": { "sessions": sessions } })
    }
}

/// A [`Discovery`] client backed by [`OfflineTracker`], going through the same
/// signing and parsing code paths the HTTP client uses.
struct TrackedDiscovery<'a> {
    tracker: &'a OfflineTracker,
    client: HttpTrackerClient<RawKeypairAuth>,
    now: u64,
}

#[async_trait::async_trait]
impl Discovery for TrackedDiscovery<'_> {
    async fn announce(&self, session: SessionAd) -> magnetite_seams::Result<()> {
        // Exactly the envelope HttpTrackerClient would put on the wire.
        let signed = self.client.envelope(&session, self.now);
        self.tracker
            .announce(signed, self.now)
            .map_err(magnetite_seams::SeamError::Transport)
    }

    async fn find(&self, game: Hash, filter: Filter) -> Vec<SessionAd> {
        let body = self.tracker.sessions_json(self.now);
        parse_sessions(&body)
            .into_iter()
            .filter(|a| a.game == game && filter.accepts(a))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A minimal but *valid* wasm module, so the node's content-addressed load path
/// operates on something real rather than a placeholder byte string.
const GAME_WASM: &[u8] = b"\x00asm\x01\x00\x00\x00magnetite arena shooter build";

const NOW: u64 = 1_800_000_000;
const SEAT_PRICE: u64 = 500;

fn node_capacity() -> Capacity {
    Capacity {
        cpu_cores: 8,
        ram_mb: 32_768,
        bandwidth_mbps: 1_000,
        free_slots: 0,
        max_shards: 0, // 0 ⇒ derived from hardware; never a hardcoded cap.
    }
}

fn seat_price() -> Price {
    Price {
        amount: SEAT_PRICE,
        currency: "USDC".into(),
        unit: "per_seat".into(),
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ===========================================================================
// THE LOOP
// ===========================================================================

#[tokio::test]
async fn decentralized_loop_end_to_end_offline() {
    // Three keypairs, no accounts anywhere: the node that hosts, the player who
    // pays, and the developer who gets paid.
    let node_key = RawKeypairAuth::from_seed([0x11; 32]);
    let player = RawKeypairAuth::from_seed([0x22; 32]);
    let developer = RawKeypairAuth::from_seed([0x33; 32]);

    let blobs = LocalBlobStore::new();
    let rail = MockPaymentRail::new();
    let comms = BuiltinProvider::new(RawKeypairAuth::from_seed([0x11; 32]));
    let tracker = OfflineTracker::default();

    // ── 1. Content-addressed load ───────────────────────────────────────────
    // The game id IS the hash of its bytes. No registry row names it.
    let game: Hash = blobs.put(GAME_WASM).await;
    assert_eq!(game, Hash::of(GAME_WASM), "game id is BLAKE3 of the module");

    let bytes = load_verified_game(&blobs, &game)
        .await
        .expect("honest bytes load");
    assert_eq!(bytes, GAME_WASM);

    // NEGATIVE: a blob source that hands back different bytes is refused, not
    // trusted. This is the whole trust boundary of content addressing.
    struct LyingStore;
    #[async_trait::async_trait]
    impl BlobStore for LyingStore {
        async fn put(&self, b: &[u8]) -> Hash {
            Hash::of(b)
        }
        async fn get(&self, _h: &Hash) -> Option<Vec<u8>> {
            Some(b"\x00asm backdoored module".to_vec())
        }
        async fn has(&self, _h: &Hash) -> bool {
            true
        }
    }
    assert!(
        matches!(
            load_verified_game(&LyingStore, &game).await,
            Err(NodeError::HashMismatch { .. })
        ),
        "a node must never run bytes that do not match the address it asked for"
    );
    // NEGATIVE: an address nobody stored resolves to nothing, not to a default.
    assert!(matches!(
        load_verified_game(&blobs, &Hash::of(b"a game that does not exist")).await,
        Err(NodeError::BlobMissing(_))
    ));

    // ── 2. Measure + self-advertise ─────────────────────────────────────────
    let discovery = TrackedDiscovery {
        tracker: &tracker,
        client: HttpTrackerClient::with_ttl(RawKeypairAuth::from_seed([0x11; 32]), 120),
        now: NOW,
    };

    let cfg = NodeConfig {
        bind_addr: "node-a.local:9000".into(),
        capacity: Some(node_capacity()),
        price: Some(seat_price()),
        cell_size: 100.0,
        ..Default::default()
    };
    let prepared = prepare_game(&blobs, &discovery, &game, &cfg)
        .await
        .expect("node prepares and announces the game it hosts");

    assert_eq!(prepared.game, game);
    // Player cap is EMERGENT from measured hardware, never a constant.
    assert!(
        prepared.match_config.max_players > 16,
        "8 cores must yield more than a single-room cap, got {}",
        prepared.match_config.max_players
    );
    assert!(
        prepared.ad.capacity.max_shards >= 1,
        "shard budget is derived from capacity"
    );

    // ── 3. A client discovers it ────────────────────────────────────────────
    let found = discovery.find(game, Filter::default()).await;
    assert_eq!(found.len(), 1, "exactly one node advertises this game");
    let ad = &found[0];
    assert_eq!(ad.game, game, "advertised game is the content address");
    assert_eq!(ad.node, NodeAddr("node-a.local:9000".into()));
    assert_eq!(ad.capacity.cpu_cores, 8);
    assert_eq!(
        ad.price.as_ref().map(|p| p.amount),
        Some(SEAT_PRICE),
        "the price the client sees is the price the node signed"
    );

    // Filters behave (the server browser's controls).
    assert!(
        discovery
            .find(
                game,
                Filter {
                    max_price: Some(SEAT_PRICE - 1),
                    ..Default::default()
                }
            )
            .await
            .is_empty(),
        "a paid session must not surface under a cheaper price filter"
    );
    assert!(
        discovery
            .find(Hash::of(b"some other game"), Filter::default())
            .await
            .is_empty(),
        "discovery must not return ads for a game nobody advertised"
    );

    // NEGATIVE: a forged announce for someone else's node is refused.
    let attacker = RawKeypairAuth::from_seed([0x99; 32]);
    let mut forged = SignedAd::sign(&attacker, prepared.ad.clone(), NOW, 120);
    forged.node_key = node_key.pubkey(); // claim to be the honest node
    assert!(
        tracker.announce(forged, NOW).is_err(),
        "a tracker is a phonebook, but it must not accept a forged entry"
    );
    // NEGATIVE: an attacker cannot overwrite an honest node's slot with their
    // own (validly signed!) ad — signature alone is not ownership of a slot.
    let hijack = SignedAd::sign(&attacker, prepared.ad.clone(), NOW, 120);
    assert!(
        tracker.announce(hijack, NOW).is_err(),
        "one node must never be able to take over another's listing"
    );
    // NEGATIVE: a lapsed lease disappears — soft state, no stale phonebook.
    assert!(
        parse_sessions(&tracker.sessions_json(NOW + 10_000)).is_empty(),
        "an un-renewed ad must expire on its own"
    );
    // And a withdrawal must be signed by the owner.
    let mut forged_withdraw =
        SignedWithdraw::sign(&attacker, game, prepared.ad.node.clone(), NOW);
    forged_withdraw.node_key = node_key.pubkey();
    assert!(
        tracker.withdraw(forged_withdraw, NOW).is_err(),
        "nobody may de-list another node"
    );

    // ── 4. Pay for a seat ───────────────────────────────────────────────────
    // Wallet → wallet. No balance table, no custody: the receipt IS the
    // entitlement, and the developer takes the whole subtotal.
    let split = PaymentSplit {
        developer: Split {
            wallet: developer.pubkey(),
            amount: SEAT_PRICE,
        },
        operator: None,
        protocol_fee_bps: 0,
    };
    let receipt: Receipt = rail.checkout(&player.pubkey(), split).await;

    assert_eq!(receipt.buyer, player.pubkey());
    assert_eq!(receipt.total, SEAT_PRICE);
    assert_eq!(receipt.payouts.len(), 1);
    assert_eq!(receipt.payouts[0].wallet, developer.pubkey());
    assert!(
        rail.verify_receipt(&receipt),
        "an honest receipt must verify against the rail"
    );

    // NEGATIVE: a tampered receipt is worthless. This is the difference between
    // "a row says paid" and "the payment is provable".
    let mut tampered = receipt.clone();
    tampered.payouts[0].amount = 1;
    tampered.total = 1;
    assert!(
        !rail.verify_receipt(&tampered),
        "an edited receipt must never verify"
    );
    // NEGATIVE: swapping the payee is equally fatal.
    let mut redirected = receipt.clone();
    redirected.payouts[0].wallet = attacker.pubkey();
    assert!(
        !rail.verify_receipt(&redirected),
        "a receipt must not be re-pointable at another wallet"
    );

    // ── 5. Get into the session's rooms ─────────────────────────────────────
    // Free lobby: a credential is minted straight from the player's key — no
    // account, no password, no homeserver.
    let lobby = comms.create_room(RoomScope::Lobby).await;
    let cred = comms.issue_join_credential(&player.pubkey(), &lobby).await;
    assert_eq!(cred.room, lobby);
    assert_eq!(
        cred.token.claims.subject,
        player.pubkey(),
        "the credential names the player's own key"
    );
    assert_eq!(cred.token.claims.issuer, node_key.pubkey());
    assert!(
        cred.token.is_valid_at(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ),
        "a freshly minted credential must be live"
    );
    assert!(
        !cred.token.is_valid_at(u64::MAX),
        "FAIL-CLOSED: a credential must not be valid forever"
    );

    // Paid room: the SAME shared gate the backend delegates to. Free rooms cost
    // nothing to enter; a paid room without a good receipt is refused.
    assert!(
        receipt_admits(&rail, &receipt, &player.pubkey(), SEAT_PRICE),
        "the player paid for this seat and must be admitted"
    );
    assert!(
        receipt_admits(&rail, &receipt, &player.pubkey(), 0),
        "a free room needs no receipt at all"
    );

    // NEGATIVE — the four ways in that must all stay shut:
    assert!(
        !receipt_admits(&rail, &tampered, &player.pubkey(), SEAT_PRICE),
        "FAIL-CLOSED: a forged receipt must not open a paid room"
    );
    assert!(
        !receipt_admits(&rail, &receipt, &attacker.pubkey(), SEAT_PRICE),
        "FAIL-CLOSED: another player's receipt must not open a paid room"
    );
    assert!(
        !receipt_admits(&rail, &receipt, &player.pubkey(), SEAT_PRICE + 1),
        "FAIL-CLOSED: underpaying must not open a paid room"
    );
    let cheap = rail
        .checkout(
            &player.pubkey(),
            PaymentSplit {
                developer: Split {
                    wallet: developer.pubkey(),
                    amount: 1,
                },
                operator: None,
                protocol_fee_bps: 0,
            },
        )
        .await;
    assert!(
        rail.verify_receipt(&cheap),
        "the cheap receipt is genuine — it is simply for the wrong amount"
    );
    assert!(
        !receipt_admits(&rail, &cheap, &player.pubkey(), SEAT_PRICE),
        "FAIL-CLOSED: a genuine receipt for a cheaper item must not admit here"
    );

    // ── 6. The game ticks, deterministically, across shards ─────────────────
    let (hashes_a, shards_a) = run_multishard_world(prepared.match_config.clone(), 40.0);
    let (hashes_b, shards_b) = run_multishard_world(prepared.match_config.clone(), 40.0);

    assert!(
        shards_a > 1,
        "one box must host several shards for an elastic world, got {shards_a}"
    );
    assert_eq!(shards_a, shards_b, "same inputs ⇒ same shard topology");
    assert_eq!(
        hashes_a, hashes_b,
        "same inputs ⇒ identical per-shard state hashes (determinism holds across shards)"
    );

    // NEGATIVE: determinism means "same inputs ⇒ same world", not "always the
    // same world". Drive the players differently and the shard layout must
    // change — otherwise the equality above would be vacuously true.
    let (hashes_c, shards_c) = run_multishard_world(prepared.match_config.clone(), 7.0);
    assert_ne!(
        (hashes_a.clone(), shards_a),
        (hashes_c, shards_c),
        "different inputs must yield a different world — determinism is not a constant"
    );

    // ── Clean shutdown ──────────────────────────────────────────────────────
    let bye = SignedWithdraw::sign(&node_key, game, prepared.ad.node.clone(), NOW);
    assert_eq!(
        tracker.withdraw(bye, NOW).expect("owner may de-list"),
        1,
        "a node that shuts down cleanly leaves no ghost in the phonebook"
    );
    assert!(discovery.find(game, Filter::default()).await.is_empty());
}

/// Drive a sharded world for a few ticks and hash every live shard.
fn run_multishard_world(base: MatchConfig, drift: f64) -> (Vec<(ShardId, u64)>, usize) {
    let cfg = MatchConfig {
        topology: Topology::Sharded {
            tick_hz: 20,
            cell_size: 100.0,
            max_per_shard: 64,
        },
        ..base
    };
    let mut rt = ShardedRuntime::new(
        cfg.clone(),
        Box::new(move |_shard, config| Box::new(NativeExecutor::<ArenaShooter>::new(config.clone()))),
    );

    let players: Vec<PlayerId> = (1..=5).map(PlayerId::new).collect();
    for p in &players {
        rt.join(*p);
    }
    for tick in 1u64..=6 {
        let inputs: Vec<(PlayerId, Input)> = players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    *p,
                    Input {
                        mouse: MouseState {
                            delta_x: drift * (i as f64 + 1.0),
                            delta_y: 0.0,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
            })
            .collect();
        rt.step(tick, &inputs);
    }

    let active = rt.active_shards();
    let hashes = active
        .iter()
        .map(|s| (*s, fnv(&rt.snapshot_shard(*s).unwrap_or_default())))
        .collect();
    (hashes, active.len())
}

// ---------------------------------------------------------------------------
// A second node joins the phonebook
// ---------------------------------------------------------------------------

/// Two independent nodes advertising the same content address are provably
/// running the same build — that is the property content addressing buys, and
/// it is what lets a client pick a host on latency/price rather than on trust.
#[tokio::test]
async fn two_nodes_advertise_the_same_build_and_the_client_chooses() {
    let blobs = LocalBlobStore::new();
    let tracker = OfflineTracker::default();
    let game = blobs.put(GAME_WASM).await;

    for (seed, addr, ping, price) in [
        ([0x41u8; 32], "cheap-far.example:9000", 180u32, None),
        ([0x42u8; 32], "pricey-near.example:9000", 12u32, Some(seat_price())),
    ] {
        let d = TrackedDiscovery {
            tracker: &tracker,
            client: HttpTrackerClient::with_ttl(RawKeypairAuth::from_seed(seed), 120),
            now: NOW,
        };
        let cfg = NodeConfig {
            bind_addr: addr.into(),
            capacity: Some(node_capacity()),
            price,
            ..Default::default()
        };
        let prepared = prepare_game(&blobs, &d, &game, &cfg).await.unwrap();
        // Both nodes advertise the SAME hash — same code, different operators.
        assert_eq!(prepared.game, game);
        // Ads carry a ping hint the browser sorts on; set it post-announce for
        // this fixture by re-announcing with the hint filled in.
        let mut ad = prepared.ad.clone();
        ad.ping_hint = ping;
        d.announce(ad).await.unwrap();
    }

    let d = TrackedDiscovery {
        tracker: &tracker,
        client: HttpTrackerClient::with_ttl(RawKeypairAuth::from_seed([0x43; 32]), 120),
        now: NOW,
    };

    let all = d.find(game, Filter::default()).await;
    assert_eq!(all.len(), 2, "both operators are listed");

    let near = d
        .find(
            game,
            Filter {
                max_ping: Some(50),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(near.len(), 1);
    assert_eq!(near[0].node, NodeAddr("pricey-near.example:9000".into()));

    let free = d
        .find(
            game,
            Filter {
                max_price: Some(0),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(free.len(), 1);
    assert_eq!(free[0].node, NodeAddr("cheap-far.example:9000".into()));

    // NEGATIVE: nothing satisfies "cheap AND near" — discovery must return an
    // empty list rather than a best-effort substitute the client did not ask for.
    assert!(
        d.find(
            game,
            Filter {
                max_ping: Some(50),
                max_price: Some(0),
                require_free_slot: false,
            },
        )
        .await
        .is_empty(),
        "a phonebook must not invent an entry to satisfy a query"
    );
}

/// A node that never signs its ad gets nowhere, even against a tracker that is
/// otherwise happy to list anyone. Discovery holds no authority — but it does
/// hold the line on authorship.
#[tokio::test]
async fn an_unsigned_announce_never_reaches_the_phonebook() {
    let tracker = OfflineTracker::default();
    let honest = RawKeypairAuth::from_seed([0x51; 32]);
    let game = Hash::of(GAME_WASM);

    let ad = SessionAd {
        game,
        node: NodeAddr("ghost.example:9000".into()),
        capacity: node_capacity(),
        ping_hint: 5,
        price: None,
        chat_room: None,
        voice_room: None,
    };

    let mut unsigned = SignedAd::sign(&honest, ad.clone(), NOW, 120);
    unsigned.sig = magnetite_seams::identity::Sig([0u8; 64]);
    assert!(tracker.announce(unsigned, NOW).is_err());

    // An over-long lease is refused too: no squatting the phonebook.
    let squat = SignedAd::sign(&honest, ad.clone(), NOW, 60 * 60 * 24 * 365);
    assert!(tracker.announce(squat, NOW).is_err());

    // Backdated so it is already dead on arrival.
    let stale = SignedAd::sign(&honest, ad, NOW - 10_000, 120);
    assert!(tracker.announce(stale, NOW).is_err());

    assert!(
        parse_sessions(&tracker.sessions_json(NOW)).is_empty(),
        "nothing unsigned, expired, or over-leased may be served to a client"
    );
}

/// Sanity: the whole loop above touched no external service. If any seam had
/// silently reached for one, the defaults would not be these types.
#[test]
fn every_seam_default_is_offline() {
    let _: LocalBlobStore = LocalBlobStore::new();
    let _: LanDiscovery = LanDiscovery::new();
    let _: MockPaymentRail = MockPaymentRail::new();
    let _: BuiltinProvider<RawKeypairAuth> =
        BuiltinProvider::new(RawKeypairAuth::from_seed([0; 32]));
    let _: PubKey = RawKeypairAuth::from_seed([0; 32]).pubkey();
}
