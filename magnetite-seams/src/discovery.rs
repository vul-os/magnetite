//! Seam §3.4 — `Discovery` (the phonebook — never an authority).
//!
//! Nodes self-advertise the sessions they host; clients query for sessions of a
//! game. Discovery holds no authority — it is a swappable, redundant hint layer
//! that replaces the old central `runtime_instances` poll.
//!
//! Defaults:
//! - [`LanDiscovery`] — in-process registry stub standing in for mDNS/LAN.
//! - [`TrackerDiscovery`] — a dumb BitTorrent-style HTTP tracker. Its HTTP calls
//!   sit behind the [`TrackerClient`] trait, so it unit-tests with no network.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::blobstore::Hash;
use crate::comms::RoomAddr;
use crate::error::Result;

/// How to reach a node hosting a session (opaque address; e.g. `ip:port`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAddr(pub String);

/// A node's self-measured capacity (§4 — player cap is emergent from hardware).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    /// Logical CPU cores.
    pub cpu_cores: u32,
    /// RAM in megabytes.
    pub ram_mb: u64,
    /// Advertised bandwidth in Mbps.
    pub bandwidth_mbps: u32,
    /// Free player/seat slots right now.
    pub free_slots: u32,
    /// Max shards this box can host.
    pub max_shards: u32,
}

/// A price hint for a paid session (settlement is the [`crate::payment`] seam).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    /// Amount in the smallest unit of `currency`.
    pub amount: u64,
    /// e.g. `"USDC"`.
    pub currency: String,
    /// `"per_seat"`, `"per_hour"`, ...
    pub unit: String,
}

/// A node's advertisement of a hostable/live session (§3.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAd {
    /// Content address of the game (wasm + manifest).
    pub game: Hash,
    /// Where to reach the hosting node.
    pub node: NodeAddr,
    /// Self-measured capacity.
    pub capacity: Capacity,
    /// Rough latency hint in ms.
    pub ping_hint: u32,
    /// Optional price for joining.
    pub price: Option<Price>,
    /// Optional pre-provisioned chat room.
    pub chat_room: Option<RoomAddr>,
    /// Optional pre-provisioned voice room.
    pub voice_room: Option<RoomAddr>,
}

/// Client-side filter applied over discovered ads.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    /// Only ads with `ping_hint <= max_ping`.
    pub max_ping: Option<u32>,
    /// Only ads with at least one free slot.
    pub require_free_slot: bool,
    /// Only ads whose price amount is `<= max_price` (free counts as 0).
    pub max_price: Option<u64>,
}

impl Filter {
    fn accepts(&self, ad: &SessionAd) -> bool {
        if let Some(mp) = self.max_ping {
            if ad.ping_hint > mp {
                return false;
            }
        }
        if self.require_free_slot && ad.capacity.free_slots == 0 {
            return false;
        }
        if let Some(cap) = self.max_price {
            let amount = ad.price.as_ref().map(|p| p.amount).unwrap_or(0);
            if amount > cap {
                return false;
            }
        }
        true
    }
}

/// The phonebook seam (§3.4).
#[async_trait::async_trait]
pub trait Discovery {
    /// Advertise a session this node hosts.
    async fn announce(&self, session: SessionAd) -> Result<()>;
    /// Find sessions for a game, honoring `filter`.
    async fn find(&self, game: Hash, filter: Filter) -> Vec<SessionAd>;
}

/// In-process registry stub standing in for mDNS/LAN discovery. Offline.
#[derive(Default)]
pub struct LanDiscovery {
    ads: Mutex<Vec<SessionAd>>,
}

impl LanDiscovery {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl Discovery for LanDiscovery {
    async fn announce(&self, session: SessionAd) -> Result<()> {
        let mut ads = self.ads.lock().unwrap();
        // De-dup on (game, node): a re-announce refreshes the ad.
        ads.retain(|a| !(a.game == session.game && a.node == session.node));
        ads.push(session);
        Ok(())
    }
    async fn find(&self, game: Hash, filter: Filter) -> Vec<SessionAd> {
        self.ads
            .lock()
            .unwrap()
            .iter()
            .filter(|a| a.game == game && filter.accepts(a))
            .cloned()
            .collect()
    }
}

/// Pluggable HTTP transport for [`TrackerDiscovery`]. A real impl talks to a
/// tracker over HTTP; tests inject an in-memory fake. Keeps the crate free of an
/// HTTP dependency and lets discovery unit-test offline.
#[async_trait::async_trait]
pub trait TrackerClient: Send + Sync {
    /// POST an announce to `{base}/announce`.
    async fn announce(&self, base_url: &str, ad: &SessionAd) -> Result<()>;
    /// GET all ads for a game from `{base}/find/{game_hex}`.
    async fn find(&self, base_url: &str, game: &Hash) -> Result<Vec<SessionAd>>;
}

/// Dumb, swappable HTTP tracker (BitTorrent-style). Anyone runs one; they are
/// redundant and hold no authority. Filtering happens client-side.
pub struct TrackerDiscovery<C: TrackerClient> {
    base_url: String,
    client: C,
}

impl<C: TrackerClient> TrackerDiscovery<C> {
    /// Build over a tracker base URL and a transport client.
    pub fn new(base_url: impl Into<String>, client: C) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
        }
    }
}

#[async_trait::async_trait]
impl<C: TrackerClient> Discovery for TrackerDiscovery<C> {
    async fn announce(&self, session: SessionAd) -> Result<()> {
        self.client.announce(&self.base_url, &session).await
    }
    async fn find(&self, game: Hash, filter: Filter) -> Vec<SessionAd> {
        let ads = self
            .client
            .find(&self.base_url, &game)
            .await
            .unwrap_or_default();
        ads.into_iter().filter(|a| filter.accepts(a)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ad(game: &[u8], node: &str, free: u32, ping: u32, price: Option<u64>) -> SessionAd {
        SessionAd {
            game: Hash::of(game),
            node: NodeAddr(node.into()),
            capacity: Capacity {
                cpu_cores: 8,
                ram_mb: 16384,
                bandwidth_mbps: 1000,
                free_slots: free,
                max_shards: 32,
            },
            ping_hint: ping,
            price: price.map(|amount| Price {
                amount,
                currency: "USDC".into(),
                unit: "per_seat".into(),
            }),
            chat_room: None,
            voice_room: None,
        }
    }

    #[tokio::test]
    async fn lan_announce_find_and_filter() {
        let d = LanDiscovery::new();
        let g = Hash::of(b"snake");
        d.announce(ad(b"snake", "n1", 4, 20, None)).await.unwrap();
        d.announce(ad(b"snake", "n2", 0, 200, Some(500)))
            .await
            .unwrap();
        d.announce(ad(b"other", "n3", 4, 10, None)).await.unwrap();

        // Unfiltered: two snake ads.
        assert_eq!(d.find(g, Filter::default()).await.len(), 2);
        // Require a free slot -> drops n2.
        let free = d
            .find(
                g,
                Filter {
                    require_free_slot: true,
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(free.len(), 1);
        assert_eq!(free[0].node, NodeAddr("n1".into()));
        // Max ping 50 -> drops n2.
        assert_eq!(
            d.find(
                g,
                Filter {
                    max_ping: Some(50),
                    ..Default::default()
                }
            )
            .await
            .len(),
            1
        );
    }

    #[tokio::test]
    async fn re_announce_refreshes_same_node() {
        let d = LanDiscovery::new();
        let g = Hash::of(b"pong");
        d.announce(ad(b"pong", "n1", 4, 20, None)).await.unwrap();
        d.announce(ad(b"pong", "n1", 1, 20, None)).await.unwrap();
        let found = d.find(g, Filter::default()).await;
        assert_eq!(found.len(), 1, "same (game,node) de-duped");
        assert_eq!(found[0].capacity.free_slots, 1, "latest wins");
    }

    /// In-memory tracker backing store, keyed by game hex.
    #[derive(Default)]
    struct FakeTracker {
        store: Mutex<HashMap<String, Vec<SessionAd>>>,
    }
    #[async_trait::async_trait]
    impl TrackerClient for FakeTracker {
        async fn announce(&self, _base: &str, ad: &SessionAd) -> Result<()> {
            self.store
                .lock()
                .unwrap()
                .entry(ad.game.to_hex())
                .or_default()
                .push(ad.clone());
            Ok(())
        }
        async fn find(&self, _base: &str, game: &Hash) -> Result<Vec<SessionAd>> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .get(&game.to_hex())
                .cloned()
                .unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn tracker_announce_find_via_client_trait() {
        let tracker = TrackerDiscovery::new("https://tracker.example/", FakeTracker::default());
        let g = Hash::of(b"tetris");
        tracker
            .announce(ad(b"tetris", "n1", 4, 30, Some(1000)))
            .await
            .unwrap();
        tracker
            .announce(ad(b"tetris", "n2", 4, 30, Some(50)))
            .await
            .unwrap();

        assert_eq!(tracker.find(g, Filter::default()).await.len(), 2);
        // Price filter is applied client-side.
        let cheap = tracker
            .find(
                g,
                Filter {
                    max_price: Some(100),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(cheap.len(), 1);
        assert_eq!(cheap[0].node, NodeAddr("n2".into()));
    }
}
