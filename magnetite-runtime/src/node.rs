//! Generic capacity-elastic **node host** (DECENTRALIZATION.md §4).
//!
//! A node is *generic compute that fills its own hardware*. This module ties the
//! seams together so a bare `magnetite` binary can:
//!
//! 1. **Measure its own hardware** → a [`Capacity`] (see [`crate::capacity`]).
//! 2. **Load a content-addressed game** by BLAKE3 [`Hash`] from a [`BlobStore`],
//!    **verifying the hash before running it** — the game id *is* the hash of
//!    `(wasm + manifest)`, so a dishonest blob source cannot swap the code.
//! 3. **Self-advertise** a [`SessionAd`] via a [`Discovery`] provider instead of
//!    polling a central `runtime_instances` table.
//! 4. **Serve** the game with the sandboxed Wasm executor.
//!
//! Everything here programs against the seam *traits* only — no provider-specific
//! type appears — so a node works fully offline with the defaults
//! ([`LocalBlobStore`](magnetite_seams::blobstore::LocalBlobStore),
//! [`LanDiscovery`](magnetite_seams::discovery::LanDiscovery)) and gains a real
//! tracker/DHT/BitTorrent backend purely by swapping the provider in.

use std::sync::Arc;
use std::time::Duration;

use magnetite_seams::blobstore::{BlobStore, Hash};
use magnetite_seams::discovery::{
    Capacity, Discovery, NodeAddr, Price, SessionAd, MAX_AD_TTL_SECS,
};

use magnetite_sandbox::{LimitsConfig, WasmExecutor};
use magnetite_sdk::authority::MatchConfig;

use crate::capacity::measure_capacity;
use crate::server::{GameServer, GameServerConfig, ServerError};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Something went wrong bringing a node up.
#[derive(Debug)]
pub enum NodeError {
    /// The requested game hash is not present in the blob store.
    BlobMissing(Hash),
    /// The fetched bytes did not hash to the requested content address.
    /// (A dishonest or corrupt blob source — refused, fail-closed.)
    HashMismatch {
        /// What was asked for.
        want: Hash,
        /// What the bytes actually hashed to.
        got: Hash,
    },
    /// The discovery provider rejected the announcement.
    Announce(String),
    /// The game server failed to start or crashed.
    Server(ServerError),
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeError::BlobMissing(h) => write!(f, "game blob {} not found in store", h.to_hex()),
            NodeError::HashMismatch { want, got } => write!(
                f,
                "content-address mismatch: wanted {}, bytes hashed to {} — refusing to run",
                want.to_hex(),
                got.to_hex()
            ),
            NodeError::Announce(e) => write!(f, "discovery announce failed: {e}"),
            NodeError::Server(e) => write!(f, "game server error: {e}"),
        }
    }
}

impl std::error::Error for NodeError {}

// ---------------------------------------------------------------------------
// Content addressing
// ---------------------------------------------------------------------------

/// The content address (game id) of a module's bytes.
///
/// The game id is the BLAKE3 hash of the `(wasm + manifest)` bytes — no central
/// registry row is needed to name a game.
pub fn content_address(bytes: &[u8]) -> Hash {
    Hash::of(bytes)
}

/// Fetch a game module by content address and **verify the hash before use**.
///
/// This is the trust boundary for content-addressed distribution: bytes from any
/// source (local store, HTTP mirror, peer) are only accepted if they hash to the
/// requested [`Hash`]. Any mismatch is refused ([`NodeError::HashMismatch`]) —
/// fail-closed, never run unverified code.
pub async fn load_verified_game<B: BlobStore>(
    blobs: &B,
    game: &Hash,
) -> Result<Vec<u8>, NodeError> {
    let bytes = blobs.get(game).await.ok_or(NodeError::BlobMissing(*game))?;
    let got = Hash::of(&bytes);
    if got != *game {
        return Err(NodeError::HashMismatch {
            want: *game,
            got,
        });
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Self-advertisement
// ---------------------------------------------------------------------------

/// Build the [`SessionAd`] this node will publish for a hosted game.
///
/// `operator` and `region` are whatever this node chooses to call itself. They
/// are carried inside the signed ad body so nobody can relabel this box, but no
/// tracker verifies them — see [`SessionAd::operator`].
pub fn build_session_ad(
    game: Hash,
    node_addr: impl Into<String>,
    capacity: Capacity,
    price: Option<Price>,
    operator: Option<String>,
    region: Option<String>,
) -> SessionAd {
    SessionAd {
        game,
        node: NodeAddr(node_addr.into()),
        operator,
        region,
        capacity,
        ping_hint: 0,
        price,
        chat_room: None,
        voice_room: None,
    }
}

/// Announce a session to the phonebook. Replaces the central provisioning poll:
/// the node tells discovery "I host game X with this capacity", clients query it.
pub async fn announce<D: Discovery + ?Sized>(discovery: &D, ad: SessionAd) -> Result<(), NodeError> {
    discovery
        .announce(ad)
        .await
        .map_err(|e| NodeError::Announce(e.to_string()))
}

// ---------------------------------------------------------------------------
// Lease heartbeat
// ---------------------------------------------------------------------------

/// Default lease length a node asks for, in seconds.
///
/// Matches `magnetite_runtime::tracker::DEFAULT_TTL_SECS`: the node renews at
/// roughly half of it, so one missed heartbeat is survivable while a node that
/// actually died vanishes from the phonebook within the lease.
pub const DEFAULT_LEASE_SECS: u64 = 120;

/// A running background task that keeps this node's ad alive in the phonebook.
///
/// A tracker ad is a **lease**, not a registration (§3.4): it lapses within
/// [`MAX_AD_TTL_SECS`] unless renewed. Without this, a node that has been up for
/// an hour is still serving happily but is invisible to everyone — the phonebook
/// only ever lists nodes that keep saying "still here", which is precisely what
/// makes a stale/restored-from-backup tracker converge to the truth.
///
/// Dropping the handle does **not** stop the task; call [`Heartbeat::stop`] (or
/// [`Heartbeat::stop_and_withdraw`] on a graceful shutdown).
pub struct Heartbeat {
    stop: Arc<tokio::sync::Notify>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
    discovery: Arc<dyn Discovery + Send + Sync>,
    ad: SessionAd,
}

impl Heartbeat {
    /// Signal the renew loop to stop and wait for it to wind down.
    ///
    /// The ad is left to lapse on its own — correct for a crash-like exit, and
    /// bounded by the lease.
    pub async fn stop(self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // `notify_one` stores a permit if the task is not parked on `notified()`
        // yet, so the stop signal cannot be lost to a race — `notify_waiters`
        // would silently drop it and leave us blocked for a whole interval.
        self.stop.notify_one();
        let _ = self.handle.await;
    }

    /// Stop renewing **and** retract the ad — the graceful-shutdown path.
    ///
    /// Best-effort: if the phonebook is unreachable the lease expiry still
    /// removes us, just a few minutes later.
    pub async fn stop_and_withdraw(self) {
        let discovery = Arc::clone(&self.discovery);
        let ad = self.ad.clone();
        self.stop().await;
        let _ = discovery.withdraw(&ad).await;
    }
}

/// Cheap, dependency-free jitter in `[0, span)` derived from the wall clock.
///
/// Jitter matters here because many nodes booted by the same orchestrator would
/// otherwise renew in lockstep and hit a tracker as a thundering herd.
fn jitter(span: Duration) -> Duration {
    if span.is_zero() {
        return Duration::ZERO;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    Duration::from_millis(nanos % (span.as_millis().max(1) as u64))
}

/// Start renewing `ad` in the background until stopped.
///
/// * Renews every `lease / 2` (so one lost renew is survivable), plus up to 10%
///   jitter.
/// * On failure, retries with exponential backoff starting at one second and
///   capped at the normal interval — a transient blip recovers fast, a tracker
///   that is truly down is not hammered, and we never back off *past* the
///   renew interval because the lease is ticking the whole time.
pub fn spawn_heartbeat(
    discovery: Arc<dyn Discovery + Send + Sync>,
    ad: SessionAd,
    lease: Duration,
) -> Heartbeat {
    // Clamp to something sane: never renew in a tight loop, never ask for a
    // lease longer than the protocol honours (a node must not be able to squat
    // the phonebook by routing around the tracker client's own clamp).
    let lease = lease.clamp(
        Duration::from_millis(100),
        Duration::from_secs(MAX_AD_TTL_SECS),
    );
    let interval = (lease / 2).max(Duration::from_millis(50));

    let stop = Arc::new(tokio::sync::Notify::new());
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let task_stop = Arc::clone(&stop);
    let task_stopped = Arc::clone(&stopped);
    let task_discovery = Arc::clone(&discovery);
    let task_ad = ad.clone();

    let handle = tokio::spawn(async move {
        let mut backoff: Option<Duration> = None;
        loop {
            let base = backoff.unwrap_or(interval);
            let wait = base + jitter(base / 10);

            tokio::select! {
                _ = task_stop.notified() => break,
                _ = tokio::time::sleep(wait) => {}
            }
            if task_stopped.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            match task_discovery.announce(task_ad.clone()).await {
                Ok(()) => backoff = None,
                Err(e) => {
                    tracing::warn!("discovery heartbeat failed, will retry: {e}");
                    let next = backoff
                        .map(|b| b * 2)
                        .unwrap_or(Duration::from_secs(1))
                        .min(interval);
                    backoff = Some(next);
                }
            }
        }
    });

    Heartbeat {
        stop,
        stopped,
        handle,
        discovery,
        ad,
    }
}

// ---------------------------------------------------------------------------
// NodeConfig + run
// ---------------------------------------------------------------------------

/// Configuration for booting a capacity-elastic node.
pub struct NodeConfig {
    /// WebSocket bind address, e.g. `127.0.0.1:9000`.
    pub bind_addr: String,
    /// Shard cell size (world units) used when capacity implies a sharded world.
    pub cell_size: f32,
    /// Deterministic RNG seed for the match.
    pub seed: u64,
    /// Wasm sandbox limits.
    pub limits: LimitsConfig,
    /// Override the measured hardware capacity (e.g. for tests / containers).
    /// `None` → measure this box.
    pub capacity: Option<Capacity>,
    /// Optional per-seat/per-hour price hint carried in the advertisement.
    pub price: Option<Price>,
    /// What this node calls its operator. Self-declared; signed but unverified.
    pub operator: Option<String>,
    /// What this node calls its region. Self-declared; signed but unverified.
    pub region: Option<String>,
    /// Lease length to request, and therefore how often to renew (`lease / 2`).
    pub lease: Duration,
    /// Optional cluster wiring: when `Some`, this node is a fleet member and
    /// player sessions follow migrated shards (see [`crate::follow`]).
    ///
    /// **Deny-by-default.** `None` — the default — means this box hands shards
    /// to nobody and admits follows from nobody; it is an ordinary standalone
    /// node. A caller must construct a [`FleetSession`] over an explicit
    /// [`ClusterMembership`](crate::cluster::ClusterMembership) to opt in, and
    /// an empty membership still admits nobody.
    pub fleet: Option<crate::follow::FleetSession>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9000".to_string(),
            cell_size: 500.0,
            seed: 0xDEAD_BEEF_CAFE_1234,
            limits: LimitsConfig::default(),
            capacity: None,
            price: None,
            operator: None,
            region: None,
            lease: Duration::from_secs(DEFAULT_LEASE_SECS),
            fleet: None,
        }
    }
}

/// What a node prepared to host, returned before it starts serving so callers
/// (CLI, tests) can log/inspect it.
pub struct PreparedGame {
    /// The verified content address of the game.
    pub game: Hash,
    /// The verified module bytes.
    pub bytes: Vec<u8>,
    /// The capacity-elastic match configuration derived from this box.
    pub match_config: MatchConfig,
    /// The advertisement published to discovery.
    pub ad: SessionAd,
}

/// Prepare a content-addressed game for hosting: measure capacity, load+verify
/// the module, derive an **emergent** [`MatchConfig`], and publish a [`SessionAd`].
///
/// This is the whole node bring-up *except* binding the socket, so it is fully
/// testable offline (no port, no real network) with the default providers.
pub async fn prepare_game<B: BlobStore, D: Discovery + ?Sized>(
    blobs: &B,
    discovery: &D,
    game: &Hash,
    cfg: &NodeConfig,
) -> Result<PreparedGame, NodeError> {
    // 1. Measure hardware (player cap is emergent from this — never a constant).
    //    If a caller supplied a raw capacity without its emergent shard budget,
    //    derive it so the advertised ad is always honest.
    let mut capacity = cfg.capacity.clone().unwrap_or_else(measure_capacity);
    if capacity.max_shards == 0 {
        capacity.max_shards = magnetite_sdk::scaling::shards_for_capacity(&capacity);
    }

    // 2. Load the module by hash and verify before we ever run it.
    let bytes = load_verified_game(blobs, game).await?;

    // 3. Derive the match configuration from measured capacity.
    let match_config = MatchConfig::elastic(cfg.seed, &capacity, cfg.cell_size);

    // 4. Self-advertise instead of registering in a central table.
    let ad = build_session_ad(
        *game,
        cfg.bind_addr.clone(),
        capacity,
        cfg.price.clone(),
        cfg.operator.clone(),
        cfg.region.clone(),
    );
    announce(discovery, ad.clone()).await?;

    Ok(PreparedGame {
        game: *game,
        bytes,
        match_config,
        ad,
    })
}

/// Boot a full capacity-elastic node: [`prepare_game`] + serve the verified game
/// over WebSocket with the sandboxed Wasm executor. Blocks until the server
/// exits.
///
/// While it serves, a background [`Heartbeat`] keeps renewing the ad, because a
/// phonebook entry is a lease and a node that stops saying "still here" is
/// correctly forgotten. When the serve loop returns — for any reason — the
/// heartbeat is stopped and the ad is retracted, so a clean shutdown removes us
/// from the phonebook immediately instead of leaving a ghost until the lease
/// lapses.
pub async fn run_node<B: BlobStore, D: Discovery + Send + Sync + 'static>(
    blobs: &B,
    discovery: Arc<D>,
    game: &Hash,
    cfg: NodeConfig,
) -> Result<(), NodeError> {
    let prepared = prepare_game(blobs, discovery.as_ref(), game, &cfg).await?;

    let executor = WasmExecutor::from_bytes(
        &prepared.bytes,
        prepared.match_config.clone(),
        cfg.limits,
    )
    .map_err(|e| NodeError::Server(ServerError(format!("wasm load error: {e}"))))?;

    let heartbeat = spawn_heartbeat(
        Arc::clone(&discovery) as Arc<dyn Discovery + Send + Sync>,
        prepared.ad.clone(),
        cfg.lease,
    );

    let server_cfg = GameServerConfig {
        bind_addr: cfg.bind_addr,
        match_config: prepared.match_config,
        anticheat: None,
        // Cluster wiring is opt-in and deny-by-default: a standalone hosted node
        // leaves this `None` (no membership, no migration transport, so no
        // session follow). See `crate::follow`.
        fleet: cfg.fleet,
    };

    let result = GameServer::with_executor(Box::new(executor), server_cfg)
        .await
        .map_err(NodeError::Server);

    // Stop announcing ourselves before returning, whether we are exiting
    // cleanly or on error — we are no longer hosting either way.
    heartbeat.stop_and_withdraw().await;
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::blobstore::LocalBlobStore;
    use magnetite_seams::discovery::{Discovery, Filter, LanDiscovery};

    fn test_capacity() -> Capacity {
        Capacity {
            cpu_cores: 8,
            ram_mb: 32768,
            bandwidth_mbps: 1000,
            free_slots: 0,
            max_shards: 0,
        }
    }

    #[tokio::test]
    async fn load_verified_game_accepts_matching_bytes() {
        let blobs = LocalBlobStore::new();
        let payload = b"\x00asm content-addressed game module";
        let h = blobs.put(payload).await;
        let got = load_verified_game(&blobs, &h).await.unwrap();
        assert_eq!(got, payload);
    }

    #[tokio::test]
    async fn load_verified_game_rejects_missing() {
        let blobs = LocalBlobStore::new();
        let h = Hash::of(b"never stored");
        assert!(matches!(
            load_verified_game(&blobs, &h).await,
            Err(NodeError::BlobMissing(_))
        ));
    }

    #[tokio::test]
    async fn load_verified_game_rejects_hash_mismatch() {
        // A dishonest store: it hands back bytes that don't match the key.
        struct LyingStore;
        #[async_trait::async_trait]
        impl BlobStore for LyingStore {
            async fn put(&self, bytes: &[u8]) -> Hash {
                Hash::of(bytes)
            }
            async fn get(&self, _hash: &Hash) -> Option<Vec<u8>> {
                Some(b"tampered payload".to_vec())
            }
            async fn has(&self, _hash: &Hash) -> bool {
                true
            }
        }
        let wanted = Hash::of(b"the honest game");
        let err = load_verified_game(&LyingStore, &wanted).await.unwrap_err();
        assert!(matches!(err, NodeError::HashMismatch { .. }));
    }

    #[tokio::test]
    async fn prepare_game_advertises_emergent_capacity() {
        let blobs = LocalBlobStore::new();
        let discovery = LanDiscovery::new();
        let module = b"\x00asm fake module for prepare test";
        let game = blobs.put(module).await;

        let cfg = NodeConfig {
            bind_addr: "127.0.0.1:0".into(),
            capacity: Some(test_capacity()),
            ..Default::default()
        };
        let prepared = prepare_game(&blobs, &discovery, &game, &cfg).await.unwrap();

        // Game id is the content address.
        assert_eq!(prepared.game, game);
        // Player cap is emergent (8 cores ⇒ multi-shard, cap well above a room).
        assert!(prepared.match_config.max_players > 16);
        // The ad is now discoverable by game hash.
        let found = discovery.find(game, Filter::default()).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].capacity.cpu_cores, 8);
        assert!(found[0].capacity.max_shards >= 1);
    }

    // ── Lease heartbeat ──────────────────────────────────────────────────────

    use magnetite_seams::Result as SeamResult;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::Instant;

    /// A phonebook that behaves like a real tracker: every ad is a **lease**
    /// that lapses unless it is re-announced. `LanDiscovery` deliberately has no
    /// lease concept, so testing the heartbeat against it would prove nothing —
    /// this double is what makes "still discoverable past the original lease" a
    /// meaningful assertion.
    struct LeasedDiscovery {
        lease: Duration,
        /// (ad, instant the current lease expires)
        ads: Mutex<Vec<(SessionAd, Instant)>>,
        announces: AtomicUsize,
        /// When true, every announce is rejected (a tracker that is down).
        failing: std::sync::atomic::AtomicBool,
    }

    impl LeasedDiscovery {
        fn new(lease: Duration) -> Self {
            Self {
                lease,
                ads: Mutex::new(Vec::new()),
                announces: AtomicUsize::new(0),
                failing: std::sync::atomic::AtomicBool::new(false),
            }
        }
        fn live_count(&self) -> usize {
            let now = Instant::now();
            self.ads
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, exp)| *exp > now)
                .count()
        }
        fn announces(&self) -> usize {
            self.announces.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl Discovery for LeasedDiscovery {
        async fn announce(&self, session: SessionAd) -> SeamResult<()> {
            self.announces.fetch_add(1, Ordering::SeqCst);
            if self.failing.load(Ordering::SeqCst) {
                return Err(magnetite_seams::SeamError::Transport("tracker down".into()));
            }
            let expiry = Instant::now() + self.lease;
            let mut ads = self.ads.lock().unwrap();
            ads.retain(|(a, _)| !(a.game == session.game && a.node == session.node));
            ads.push((session, expiry));
            Ok(())
        }
        async fn find(&self, game: Hash, filter: Filter) -> Vec<SessionAd> {
            let now = Instant::now();
            self.ads
                .lock()
                .unwrap()
                .iter()
                .filter(|(a, exp)| *exp > now && a.game == game && filter.accepts(a))
                .map(|(a, _)| a.clone())
                .collect()
        }
        async fn withdraw(&self, ad: &SessionAd) -> SeamResult<()> {
            let mut ads = self.ads.lock().unwrap();
            ads.retain(|(a, _)| !(a.game == ad.game && a.node == ad.node));
            Ok(())
        }
    }

    fn heartbeat_ad() -> SessionAd {
        build_session_ad(
            Hash::of(b"heartbeat game"),
            "box.example:9000",
            test_capacity(),
            None,
            Some("pareto".into()),
            Some("lan".into()),
        )
    }

    #[tokio::test]
    async fn a_renewed_node_outlives_its_original_lease() {
        // 200ms leases, renewed every ~100ms.
        let lease = Duration::from_millis(200);
        let d = Arc::new(LeasedDiscovery::new(lease));
        let ad = heartbeat_ad();
        let game = ad.game;

        d.announce(ad.clone()).await.unwrap();
        assert_eq!(d.live_count(), 1, "listed on the initial announce");

        let hb = spawn_heartbeat(Arc::clone(&d) as Arc<dyn Discovery + Send + Sync>, ad, lease);

        // Wait well past the ORIGINAL lease. Without renewal this ad is gone.
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(
            d.announces() >= 2,
            "the heartbeat should have renewed at least twice by now, saw {}",
            d.announces()
        );
        assert_eq!(
            d.find(game, Filter::default()).await.len(),
            1,
            "a node that keeps saying 'still here' stays in the phonebook past its first lease"
        );

        // Stop renewing WITHOUT withdrawing: this is the crash-like path, where
        // the lease alone must evict us.
        hb.stop().await;
        let after = d.announces();
        tokio::time::sleep(lease + Duration::from_millis(150)).await;
        assert_eq!(
            d.announces(),
            after,
            "a stopped heartbeat must not keep announcing"
        );
        assert_eq!(
            d.find(game, Filter::default()).await.len(),
            0,
            "once the renews stop, the lease lapses and the node disappears"
        );
    }

    #[tokio::test]
    async fn graceful_shutdown_deregisters_immediately() {
        // A long lease: if the ad vanishes, it is because we retracted it, not
        // because it timed out.
        let lease = Duration::from_secs(300);
        let d = Arc::new(LeasedDiscovery::new(lease));
        let ad = heartbeat_ad();
        let game = ad.game;

        d.announce(ad.clone()).await.unwrap();
        let hb = spawn_heartbeat(
            Arc::clone(&d) as Arc<dyn Discovery + Send + Sync>,
            ad,
            lease,
        );
        assert_eq!(d.find(game, Filter::default()).await.len(), 1);

        hb.stop_and_withdraw().await;
        assert_eq!(
            d.find(game, Filter::default()).await.len(),
            0,
            "a clean shutdown removes the entry now, not in five minutes"
        );
    }

    #[tokio::test]
    async fn heartbeat_survives_a_failing_tracker_and_recovers() {
        let lease = Duration::from_millis(200);
        let d = Arc::new(LeasedDiscovery::new(lease));
        let ad = heartbeat_ad();
        let game = ad.game;

        // Tracker is down from the start.
        d.failing.store(true, Ordering::SeqCst);
        let hb = spawn_heartbeat(
            Arc::clone(&d) as Arc<dyn Discovery + Send + Sync>,
            ad,
            lease,
        );

        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(d.announces() >= 1, "it kept trying rather than giving up");
        assert_eq!(
            d.find(game, Filter::default()).await.len(),
            0,
            "a failed announce lists nothing — no optimistic local state"
        );

        // Tracker comes back; the loop must re-list us without a restart.
        d.failing.store(false, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            d.find(game, Filter::default()).await.len(),
            1,
            "the node re-appears on its own once the phonebook recovers"
        );
        hb.stop().await;
    }

    #[tokio::test]
    async fn heartbeat_lease_is_clamped_to_the_protocol_maximum() {
        // A node must not be able to ask for a lease longer than the protocol
        // allows by routing around the tracker client.
        let d = Arc::new(LeasedDiscovery::new(Duration::from_secs(1)));
        let hb = spawn_heartbeat(
            Arc::clone(&d) as Arc<dyn Discovery + Send + Sync>,
            heartbeat_ad(),
            Duration::from_secs(u64::MAX / 2),
        );
        // Nothing to assert beyond "it did not panic on the absurd duration and
        // it is not renewing at an absurd interval"; stop cleanly.
        hb.stop().await;
    }

    #[test]
    fn content_address_is_blake3_of_bytes() {
        assert_eq!(content_address(b"abc"), Hash::of(b"abc"));
        assert_ne!(content_address(b"abc"), content_address(b"abd"));
    }
}
