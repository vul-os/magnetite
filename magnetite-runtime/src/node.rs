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

use magnetite_seams::blobstore::{BlobStore, Hash};
use magnetite_seams::discovery::{Capacity, Discovery, NodeAddr, Price, SessionAd};

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
pub fn build_session_ad(
    game: Hash,
    node_addr: impl Into<String>,
    capacity: Capacity,
    price: Option<Price>,
) -> SessionAd {
    SessionAd {
        game,
        node: NodeAddr(node_addr.into()),
        capacity,
        ping_hint: 0,
        price,
        chat_room: None,
        voice_room: None,
    }
}

/// Announce a session to the phonebook. Replaces the central provisioning poll:
/// the node tells discovery "I host game X with this capacity", clients query it.
pub async fn announce<D: Discovery>(discovery: &D, ad: SessionAd) -> Result<(), NodeError> {
    discovery
        .announce(ad)
        .await
        .map_err(|e| NodeError::Announce(e.to_string()))
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
pub async fn prepare_game<B: BlobStore, D: Discovery>(
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
    let ad = build_session_ad(*game, cfg.bind_addr.clone(), capacity, cfg.price.clone());
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
pub async fn run_node<B: BlobStore, D: Discovery>(
    blobs: &B,
    discovery: &D,
    game: &Hash,
    cfg: NodeConfig,
) -> Result<(), NodeError> {
    let prepared = prepare_game(blobs, discovery, game, &cfg).await?;

    let executor = WasmExecutor::from_bytes(
        &prepared.bytes,
        prepared.match_config.clone(),
        cfg.limits,
    )
    .map_err(|e| NodeError::Server(ServerError(format!("wasm load error: {e}"))))?;

    let server_cfg = GameServerConfig {
        bind_addr: cfg.bind_addr,
        match_config: prepared.match_config,
        anticheat: None,
    };

    GameServer::with_executor(Box::new(executor), server_cfg)
        .await
        .map_err(NodeError::Server)
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

    #[test]
    fn content_address_is_blake3_of_bytes() {
        assert_eq!(content_address(b"abc"), Hash::of(b"abc"));
        assert_ne!(content_address(b"abc"), content_address(b"abd"));
    }
}
