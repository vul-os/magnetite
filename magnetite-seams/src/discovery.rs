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
use crate::error::{Result, SeamError};
use crate::identity::{Identity, PubKey, Sig};

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
    /// **Node-declared** operator name. A tracker cannot verify this — it is a
    /// label the node chose for itself. It is inside the signed body only so it
    /// is bound to the node key: "the box holding this key calls itself X".
    /// Never treat it as vouched-for identity. `None` ⇒ the node declared none.
    #[serde(default)]
    pub operator: Option<String>,
    /// **Node-declared** region hint (e.g. `"eu-north"`, `"lan"`). Same caveat
    /// as [`SessionAd::operator`]: signature-bound, but not certified by anyone.
    #[serde(default)]
    pub region: Option<String>,
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

// ---------------------------------------------------------------------------
// Signed announcements
// ---------------------------------------------------------------------------

/// Domain-separation tag for announce signatures (`v1`).
pub const AD_DOMAIN: &[u8] = b"magnetite/discovery/announce/v1";
/// Domain-separation tag for withdrawal (deregister) signatures (`v1`).
pub const WITHDRAW_DOMAIN: &[u8] = b"magnetite/discovery/withdraw/v1";
/// Longest TTL a tracker will honour for a single announce (10 minutes).
///
/// A tracker holds *soft* state: an ad is a lease a node must keep renewing by
/// heartbeat. Capping the TTL means a node that dies cannot leave a stale entry
/// in the phonebook for longer than this.
pub const MAX_AD_TTL_SECS: u64 = 600;

fn push_bytes(buf: &mut Vec<u8>, b: &[u8]) {
    buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
    buf.extend_from_slice(b);
}

impl SessionAd {
    /// Canonical, serialization-independent bytes for this ad.
    ///
    /// Built field-by-field (never from JSON) so two peers on different serde
    /// versions still agree byte-for-byte on what was signed.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(192);
        b.extend_from_slice(&self.game.0);
        push_bytes(&mut b, self.node.0.as_bytes());
        b.extend_from_slice(&self.capacity.cpu_cores.to_le_bytes());
        b.extend_from_slice(&self.capacity.ram_mb.to_le_bytes());
        b.extend_from_slice(&self.capacity.bandwidth_mbps.to_le_bytes());
        b.extend_from_slice(&self.capacity.free_slots.to_le_bytes());
        b.extend_from_slice(&self.capacity.max_shards.to_le_bytes());
        b.extend_from_slice(&self.ping_hint.to_le_bytes());
        match &self.price {
            Some(p) => {
                b.push(1);
                b.extend_from_slice(&p.amount.to_le_bytes());
                push_bytes(&mut b, p.currency.as_bytes());
                push_bytes(&mut b, p.unit.as_bytes());
            }
            None => b.push(0),
        }
        match &self.chat_room {
            Some(r) => {
                b.push(1);
                push_bytes(&mut b, r.0.as_bytes());
            }
            None => b.push(0),
        }
        match &self.voice_room {
            Some(r) => {
                b.push(1);
                push_bytes(&mut b, r.0.as_bytes());
            }
            None => b.push(0),
        }
        // Node-declared labels are covered by the signature too. They are not
        // *verified* by anybody, but signing them means a relay cannot silently
        // relabel someone else's box as "eu-north" or as another operator.
        for opt in [&self.operator, &self.region] {
            match opt {
                Some(s) => {
                    b.push(1);
                    push_bytes(&mut b, s.as_bytes());
                }
                None => b.push(0),
            }
        }
        b
    }
}

/// A [`SessionAd`] signed by the key of the node that hosts it, with a lease.
///
/// A tracker is a phonebook, not an authority — but a phonebook that accepts
/// unsigned entries lets anyone list someone else's number. Every announce
/// carries the hosting node's key and a signature over the ad plus its lease
/// window, so a tracker can refuse forged entries **without** gaining any say
/// over who may host what.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedAd {
    /// The advertisement itself.
    pub ad: SessionAd,
    /// The hosting node's public key (the claimed author).
    pub node_key: PubKey,
    /// Unix seconds when this announce was minted.
    pub issued_at: u64,
    /// Unix seconds when the lease lapses; the tracker drops the ad after this.
    pub expires_at: u64,
    /// `node_key`'s signature over [`SignedAd::signing_bytes`].
    pub sig: Sig,
}

impl SignedAd {
    /// Canonical bytes covered by [`SignedAd::sig`].
    pub fn signing_bytes(&self) -> Vec<u8> {
        Self::payload(&self.ad, &self.node_key, self.issued_at, self.expires_at)
    }

    fn payload(ad: &SessionAd, node_key: &PubKey, issued_at: u64, expires_at: u64) -> Vec<u8> {
        let mut b = Vec::with_capacity(256);
        b.extend_from_slice(AD_DOMAIN);
        b.extend_from_slice(&ad.signing_bytes());
        b.extend_from_slice(&node_key.0);
        b.extend_from_slice(&issued_at.to_le_bytes());
        b.extend_from_slice(&expires_at.to_le_bytes());
        b
    }

    /// Sign an ad with the hosting node's identity, leasing it for `ttl_secs`.
    pub fn sign<I: Identity>(id: &I, ad: SessionAd, now: u64, ttl_secs: u64) -> Self {
        let node_key = id.pubkey();
        let expires_at = now.saturating_add(ttl_secs);
        let sig = id.sign(&Self::payload(&ad, &node_key, now, expires_at));
        Self {
            ad,
            node_key,
            issued_at: now,
            expires_at,
            sig,
        }
    }

    /// Verify authorship and the lease window. **Fails closed**: an unsigned,
    /// forged, expired, or absurdly long-lived ad is refused outright.
    ///
    /// `skew_secs` tolerates modest clock drift on `issued_at`.
    pub fn verify<I: Identity>(&self, now: u64, skew_secs: u64) -> Result<()> {
        if self.expires_at <= self.issued_at {
            return Err(SeamError::Invalid("ad lease window is empty".into()));
        }
        if self.expires_at.saturating_sub(self.issued_at) > MAX_AD_TTL_SECS {
            return Err(SeamError::Invalid(format!(
                "ad TTL exceeds the {MAX_AD_TTL_SECS}s maximum"
            )));
        }
        if self.expires_at <= now {
            return Err(SeamError::Invalid("ad lease already expired".into()));
        }
        if self.issued_at > now.saturating_add(skew_secs) {
            return Err(SeamError::Invalid("ad issued in the future".into()));
        }
        if !I::verify(&self.node_key, &self.signing_bytes(), &self.sig) {
            return Err(SeamError::InvalidSignature);
        }
        Ok(())
    }

    /// Whether this lease is still live at `now`.
    pub fn is_live_at(&self, now: u64) -> bool {
        self.expires_at > now
    }
}

/// A signed request to remove one's own ad from a tracker (clean shutdown).
///
/// Bound to `(game, node, node_key, issued_at)` so it cannot be replayed against
/// a *later* announce from the same node, and cannot retract anyone else's ad.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedWithdraw {
    /// Game content address of the ad being withdrawn.
    pub game: Hash,
    /// Node address of the ad being withdrawn.
    pub node: NodeAddr,
    /// The withdrawing node's key — must match the ad's `node_key`.
    pub node_key: PubKey,
    /// Unix seconds when the withdrawal was minted.
    pub issued_at: u64,
    /// Signature over [`SignedWithdraw::signing_bytes`].
    pub sig: Sig,
}

impl SignedWithdraw {
    /// Canonical bytes covered by [`SignedWithdraw::sig`].
    pub fn signing_bytes(&self) -> Vec<u8> {
        Self::payload(&self.game, &self.node, &self.node_key, self.issued_at)
    }

    fn payload(game: &Hash, node: &NodeAddr, node_key: &PubKey, issued_at: u64) -> Vec<u8> {
        let mut b = Vec::with_capacity(128);
        b.extend_from_slice(WITHDRAW_DOMAIN);
        b.extend_from_slice(&game.0);
        push_bytes(&mut b, node.0.as_bytes());
        b.extend_from_slice(&node_key.0);
        b.extend_from_slice(&issued_at.to_le_bytes());
        b
    }

    /// Sign a withdrawal for `(game, node)` with the hosting node's identity.
    pub fn sign<I: Identity>(id: &I, game: Hash, node: NodeAddr, now: u64) -> Self {
        let node_key = id.pubkey();
        let sig = id.sign(&Self::payload(&game, &node, &node_key, now));
        Self {
            game,
            node,
            node_key,
            issued_at: now,
            sig,
        }
    }

    /// Verify authorship and freshness. Fails closed.
    pub fn verify<I: Identity>(&self, now: u64, max_age_secs: u64) -> Result<()> {
        if self.issued_at > now.saturating_add(60) {
            return Err(SeamError::Invalid("withdrawal issued in the future".into()));
        }
        if now.saturating_sub(self.issued_at) > max_age_secs {
            return Err(SeamError::Invalid("withdrawal is stale".into()));
        }
        if !I::verify(&self.node_key, &self.signing_bytes(), &self.sig) {
            return Err(SeamError::InvalidSignature);
        }
        Ok(())
    }
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
    /// Whether `ad` survives this filter. Public so a tracker can apply the
    /// *same* predicate server-side as a bandwidth saver without the two sides
    /// ever disagreeing on what e.g. "has a free slot" means.
    pub fn accepts(&self, ad: &SessionAd) -> bool {
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
    /// Retract this node's own ad on a graceful shutdown.
    ///
    /// Best-effort and **optional**: the lease lapses on its own within
    /// [`MAX_AD_TTL_SECS`] regardless, so a provider that cannot retract (or a
    /// tracker that is unreachable at shutdown) costs a few stale minutes, not
    /// correctness. The default is a no-op for exactly that reason.
    async fn withdraw(&self, _ad: &SessionAd) -> Result<()> {
        Ok(())
    }
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
    async fn withdraw(&self, ad: &SessionAd) -> Result<()> {
        let mut ads = self.ads.lock().unwrap();
        ads.retain(|a| !(a.game == ad.game && a.node == ad.node));
        Ok(())
    }
}

/// Announce to, and query, **several phonebooks at once**.
///
/// Discovery is meant to be redundant (§3.4): a node lists itself on the LAN
/// *and* on whatever trackers it was pointed at, and a client merges what came
/// back. This wrapper makes that the normal case rather than a special one.
///
/// Failure semantics are deliberately lopsided, because the two directions have
/// different stakes:
///
/// * [`announce`](Discovery::announce) succeeds if **any** backend accepted it.
///   Being listed in one phonebook and not another is a partial outcome, not a
///   failure to host — a node must not refuse to serve because one tracker is
///   down. It only errors if *every* backend refused.
/// * [`find`](Discovery::find) merges results and de-duplicates on
///   `(game, node)`. A backend that errors contributes nothing; it never
///   removes what another backend found.
pub struct FanoutDiscovery {
    backends: Vec<Box<dyn Discovery + Send + Sync>>,
}

impl FanoutDiscovery {
    /// Fan out over `backends`. An empty list is a valid (silent) phonebook.
    pub fn new(backends: Vec<Box<dyn Discovery + Send + Sync>>) -> Self {
        Self { backends }
    }

    /// How many phonebooks this fans out to.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Whether there are no backends at all.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

#[async_trait::async_trait]
impl Discovery for FanoutDiscovery {
    async fn announce(&self, session: SessionAd) -> Result<()> {
        let mut last_err = None;
        let mut any_ok = false;
        for b in &self.backends {
            match b.announce(session.clone()).await {
                Ok(()) => any_ok = true,
                Err(e) => last_err = Some(e),
            }
        }
        if any_ok || self.backends.is_empty() {
            return Ok(());
        }
        Err(last_err.unwrap_or_else(|| SeamError::Transport("no discovery backends".into())))
    }

    async fn find(&self, game: Hash, filter: Filter) -> Vec<SessionAd> {
        let mut out: Vec<SessionAd> = Vec::new();
        for b in &self.backends {
            for ad in b.find(game, filter.clone()).await {
                if !out.iter().any(|a| a.node == ad.node && a.game == ad.game) {
                    out.push(ad);
                }
            }
        }
        out
    }

    async fn withdraw(&self, ad: &SessionAd) -> Result<()> {
        // Best-effort everywhere; a phonebook we cannot reach lapses on its own.
        for b in &self.backends {
            let _ = b.withdraw(ad).await;
        }
        Ok(())
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
    /// DELETE this node's own ad. Optional — see [`Discovery::withdraw`].
    async fn withdraw(&self, _base_url: &str, _ad: &SessionAd) -> Result<()> {
        Ok(())
    }
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
    async fn withdraw(&self, ad: &SessionAd) -> Result<()> {
        self.client.withdraw(&self.base_url, ad).await
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
            operator: Some("nordfjord".into()),
            region: Some("eu-north".into()),
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

    // ── Signed announce / withdraw ──────────────────────────────────────────

    use crate::identity::RawKeypairAuth;

    #[test]
    fn signed_ad_roundtrips_and_verifies() {
        let node = RawKeypairAuth::from_seed([9u8; 32]);
        let s = SignedAd::sign(&node, ad(b"snake", "n1", 4, 20, None), 1_000, 60);
        assert_eq!(s.expires_at, 1_060);
        s.verify::<RawKeypairAuth>(1_000, 30).unwrap();
        // Survives a JSON round-trip (this is the wire form the tracker sees).
        let json = serde_json::to_string(&s).unwrap();
        let back: SignedAd = serde_json::from_str(&json).unwrap();
        back.verify::<RawKeypairAuth>(1_050, 30).unwrap();
    }

    #[test]
    fn tampered_ad_body_fails_verification() {
        let node = RawKeypairAuth::from_seed([9u8; 32]);
        let mut s = SignedAd::sign(&node, ad(b"snake", "n1", 4, 20, None), 1_000, 60);
        // Re-point the ad at a different node address, keeping the signature.
        s.ad.node = NodeAddr("attacker.example:1".into());
        assert!(matches!(
            s.verify::<RawKeypairAuth>(1_000, 30),
            Err(SeamError::InvalidSignature)
        ));
    }

    #[test]
    fn ad_signed_by_one_key_cannot_be_claimed_by_another() {
        let honest = RawKeypairAuth::from_seed([1u8; 32]);
        let attacker = RawKeypairAuth::from_seed([2u8; 32]);
        let mut s = SignedAd::sign(&honest, ad(b"snake", "n1", 4, 20, None), 1_000, 60);
        s.node_key = attacker.pubkey();
        assert!(s.verify::<RawKeypairAuth>(1_000, 30).is_err());
    }

    #[test]
    fn node_declared_labels_are_signature_bound() {
        let node = RawKeypairAuth::from_seed([9u8; 32]);
        let s = SignedAd::sign(&node, ad(b"snake", "n1", 4, 20, None), 1_000, 60);
        s.verify::<RawKeypairAuth>(1_000, 30).unwrap();

        // A relay cannot relabel someone else's box as a different operator…
        let mut relabelled = s.clone();
        relabelled.ad.operator = Some("totally-not-nordfjord".into());
        assert!(relabelled.verify::<RawKeypairAuth>(1_000, 30).is_err());

        // …nor move it to a nicer-sounding region…
        let mut moved = s.clone();
        moved.ad.region = Some("lan".into());
        assert!(moved.verify::<RawKeypairAuth>(1_000, 30).is_err());

        // …nor strip the labels entirely.
        let mut stripped = s.clone();
        stripped.ad.operator = None;
        stripped.ad.region = None;
        assert!(stripped.verify::<RawKeypairAuth>(1_000, 30).is_err());
    }

    #[test]
    fn undeclared_labels_are_absent_not_invented() {
        let node = RawKeypairAuth::from_seed([9u8; 32]);
        let mut anon = ad(b"snake", "n1", 4, 20, None);
        anon.operator = None;
        anon.region = None;
        let s = SignedAd::sign(&node, anon, 1_000, 60);
        s.verify::<RawKeypairAuth>(1_000, 30).unwrap();

        // Absent on the wire, and absent after a round-trip — never defaulted
        // to a plausible-looking string.
        let back: SessionAd = serde_json::from_str(&serde_json::to_string(&s.ad).unwrap()).unwrap();
        assert_eq!(back.operator, None);
        assert_eq!(back.region, None);
    }

    #[tokio::test]
    async fn withdraw_removes_only_the_withdrawing_nodes_ad() {
        let d = LanDiscovery::new();
        let g = Hash::of(b"snake");
        let mine = ad(b"snake", "n1", 4, 20, None);
        d.announce(mine.clone()).await.unwrap();
        d.announce(ad(b"snake", "n2", 4, 20, None)).await.unwrap();

        d.withdraw(&mine).await.unwrap();
        let left = d.find(g, Filter::default()).await;
        assert_eq!(left.len(), 1, "only my own slot is retracted");
        assert_eq!(left[0].node, NodeAddr("n2".into()));
    }

    #[test]
    fn expired_and_overlong_leases_are_refused() {
        let node = RawKeypairAuth::from_seed([9u8; 32]);
        let s = SignedAd::sign(&node, ad(b"snake", "n1", 4, 20, None), 1_000, 60);
        assert!(s.verify::<RawKeypairAuth>(1_061, 30).is_err(), "expired");
        assert!(!s.is_live_at(1_061));

        let long = SignedAd::sign(
            &node,
            ad(b"snake", "n1", 4, 20, None),
            1_000,
            MAX_AD_TTL_SECS + 1,
        );
        assert!(
            long.verify::<RawKeypairAuth>(1_000, 30).is_err(),
            "a node must not be able to squat the phonebook forever"
        );
    }

    #[test]
    fn withdrawal_is_authored_and_fresh() {
        let node = RawKeypairAuth::from_seed([9u8; 32]);
        let other = RawKeypairAuth::from_seed([8u8; 32]);
        let g = Hash::of(b"snake");
        let w = SignedWithdraw::sign(&node, g, NodeAddr("n1".into()), 1_000);
        w.verify::<RawKeypairAuth>(1_010, 300).unwrap();
        assert!(w.verify::<RawKeypairAuth>(9_000, 300).is_err(), "stale");

        let mut forged = SignedWithdraw::sign(&other, g, NodeAddr("n1".into()), 1_000);
        forged.node_key = node.pubkey();
        assert!(
            forged.verify::<RawKeypairAuth>(1_010, 300).is_err(),
            "nobody may retract another node's ad"
        );
    }

    /// A phonebook that refuses everything (a tracker that is down).
    struct DeadDiscovery;
    #[async_trait::async_trait]
    impl Discovery for DeadDiscovery {
        async fn announce(&self, _s: SessionAd) -> Result<()> {
            Err(SeamError::Transport("tracker unreachable".into()))
        }
        async fn find(&self, _g: Hash, _f: Filter) -> Vec<SessionAd> {
            Vec::new()
        }
    }

    #[tokio::test]
    async fn fanout_lists_on_every_reachable_phonebook_and_merges_results() {
        let a = LanDiscovery::new();
        let b = LanDiscovery::new();
        let g = Hash::of(b"snake");
        // Both backends already know a *different* node…
        a.announce(ad(b"snake", "n1", 4, 20, None)).await.unwrap();
        b.announce(ad(b"snake", "n2", 4, 25, None)).await.unwrap();

        let fan = FanoutDiscovery::new(vec![
            Box::new(a),
            Box::new(b),
            Box::new(DeadDiscovery), // one is down; must not break anything
        ]);

        // …and a query merges them.
        let found = fan.find(g, Filter::default()).await;
        assert_eq!(found.len(), 2, "results merge across phonebooks");

        // A partial announce is still a success: one dead tracker must never
        // stop a node from hosting.
        fan.announce(ad(b"snake", "n3", 4, 30, None))
            .await
            .expect("listed somewhere is listed");
        assert_eq!(fan.find(g, Filter::default()).await.len(), 3);
    }

    #[tokio::test]
    async fn fanout_announce_fails_only_when_every_phonebook_refuses() {
        let fan = FanoutDiscovery::new(vec![Box::new(DeadDiscovery), Box::new(DeadDiscovery)]);
        assert!(
            fan.announce(ad(b"snake", "n1", 4, 20, None)).await.is_err(),
            "if nobody heard us, we are not listed and must say so"
        );
    }

    #[tokio::test]
    async fn fanout_dedups_the_same_node_seen_twice() {
        let a = LanDiscovery::new();
        let b = LanDiscovery::new();
        let same = ad(b"snake", "n1", 4, 20, None);
        a.announce(same.clone()).await.unwrap();
        b.announce(same.clone()).await.unwrap();
        let fan = FanoutDiscovery::new(vec![Box::new(a), Box::new(b)]);
        assert_eq!(
            fan.find(Hash::of(b"snake"), Filter::default()).await.len(),
            1,
            "one node listed on two trackers is still one node"
        );
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
