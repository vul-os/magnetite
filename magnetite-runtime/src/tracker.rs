//! HTTP [`TrackerClient`] — the real transport behind [`TrackerDiscovery`].
//!
//! [`LanDiscovery`](magnetite_seams::discovery::LanDiscovery) stays the
//! **zero-config default**: a node that is told nothing finds its neighbours on
//! the local network and needs no service at all. A tracker is strictly opt-in
//! (`TRACKER_URL`), redundant, and swappable — it is a phonebook, and losing it
//! costs you a hint, not a game.
//!
//! What this module adds over "POST some JSON" is **authorship**. Every announce
//! is a [`SignedAd`]: the ad body plus the hosting node's key and a lease
//! window, signed by that node. A tracker that accepts unsigned entries lets
//! anyone list a box they do not run, undercut its price, or advertise capacity
//! it does not have. The node signs here; the tracker verifies and fails closed.
//!
//! ```no_run
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use magnetite_runtime::tracker::HttpTrackerClient;
//! use magnetite_seams::discovery::TrackerDiscovery;
//! use magnetite_seams::identity::RawKeypairAuth;
//!
//! let node_key = RawKeypairAuth::generate();
//! let discovery = TrackerDiscovery::new(
//!     "https://tracker.example/api/v1/discovery",
//!     HttpTrackerClient::new(node_key),
//! );
//! # Ok(()) }
//! ```

use std::time::Duration;

use magnetite_seams::blobstore::Hash;
use magnetite_seams::discovery::{
    SessionAd, SignedAd, SignedWithdraw, TrackerClient, TrackerDiscovery, MAX_AD_TTL_SECS,
};
use magnetite_seams::identity::Identity;
use magnetite_seams::{Result, SeamError};

/// Environment variable that opts a node into a tracker. Unset ⇒ LAN only.
pub const TRACKER_URL_ENV: &str = "TRACKER_URL";

/// Default lease length requested on each announce. The node re-announces at
/// roughly half this interval, so one missed heartbeat is survivable and a dead
/// node vanishes from the phonebook within the lease.
pub const DEFAULT_TTL_SECS: u64 = 120;

/// Request timeout. A slow tracker must never stall a game server's boot.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// HTTP transport for [`TrackerDiscovery`], signing every announce with the
/// node's own key.
pub struct HttpTrackerClient<I: Identity + Send + Sync> {
    node_key: I,
    http: reqwest::Client,
    ttl_secs: u64,
}

impl<I: Identity + Send + Sync> HttpTrackerClient<I> {
    /// Build a client that signs announcements with `node_key`.
    pub fn new(node_key: I) -> Self {
        Self::with_ttl(node_key, DEFAULT_TTL_SECS)
    }

    /// Build a client with an explicit lease length (clamped to the protocol
    /// maximum — a node cannot ask to squat the phonebook indefinitely).
    pub fn with_ttl(node_key: I, ttl_secs: u64) -> Self {
        Self {
            node_key,
            http: reqwest::Client::builder()
                .timeout(HTTP_TIMEOUT)
                .build()
                .unwrap_or_default(),
            ttl_secs: ttl_secs.clamp(1, MAX_AD_TTL_SECS),
        }
    }

    /// The lease length this client requests.
    pub fn ttl_secs(&self) -> u64 {
        self.ttl_secs
    }

    /// Build the exact envelope this client would POST for `ad` at `now`.
    /// Exposed so the signing path is testable without a server.
    pub fn envelope(&self, ad: &SessionAd, now: u64) -> SignedAd {
        SignedAd::sign(&self.node_key, ad.clone(), now, self.ttl_secs)
    }

    /// Build a signed deregister envelope for `(game, node)`.
    pub fn withdrawal(&self, ad: &SessionAd, now: u64) -> SignedWithdraw {
        SignedWithdraw::sign(&self.node_key, ad.game, ad.node.clone(), now)
    }

    /// Deregister this node's ad on clean shutdown. Best-effort: the lease would
    /// lapse on its own anyway, so a failure here is not fatal.
    pub async fn withdraw(&self, base_url: &str, ad: &SessionAd) -> Result<()> {
        let body = self.withdrawal(ad, now_unix());
        let resp = self
            .http
            .delete(format!("{}/announce", base_url.trim_end_matches('/')))
            .json(&body)
            .send()
            .await
            .map_err(|e| SeamError::Transport(format!("tracker withdraw: {e}")))?;
        status_ok(resp).await
    }
}

async fn status_ok(resp: reqwest::Response) -> Result<()> {
    if resp.status().is_success() {
        return Ok(());
    }
    let code = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Err(SeamError::Transport(format!(
        "tracker rejected request: {code} {body}"
    )))
}

#[async_trait::async_trait]
impl<I: Identity + Send + Sync> TrackerClient for HttpTrackerClient<I> {
    async fn announce(&self, base_url: &str, ad: &SessionAd) -> Result<()> {
        let body = self.envelope(ad, now_unix());
        let resp = self
            .http
            .post(format!("{}/announce", base_url.trim_end_matches('/')))
            .json(&body)
            .send()
            .await
            .map_err(|e| SeamError::Transport(format!("tracker announce: {e}")))?;
        status_ok(resp).await
    }

    async fn find(&self, base_url: &str, game: &Hash) -> Result<Vec<SessionAd>> {
        let url = format!(
            "{}/sessions?game={}",
            base_url.trim_end_matches('/'),
            game.to_hex()
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SeamError::Transport(format!("tracker find: {e}")))?;
        if !resp.status().is_success() {
            return status_ok(resp).await.map(|_| Vec::new());
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SeamError::Transport(format!("tracker find: bad json: {e}")))?;
        Ok(parse_sessions(&body))
    }

    /// Retract this node's ad. Delegates to the signed
    /// [`HttpTrackerClient::withdraw`] so `TrackerDiscovery::withdraw` — and
    /// therefore the node's graceful-shutdown path — actually reaches the
    /// tracker instead of silently no-op'ing.
    async fn withdraw(&self, base_url: &str, ad: &SessionAd) -> Result<()> {
        HttpTrackerClient::withdraw(self, base_url, ad).await
    }
}

/// Pull `SessionAd`s out of a tracker's `GET /sessions` body.
///
/// Tolerant of the envelope (`{data:{sessions:[…]}}`, `{sessions:[…]}`, or a
/// bare array) and of extra bookkeeping fields (`id`, `node_key`, `expires_at`)
/// riding alongside the seam shape. Ads that do not parse are **skipped, not
/// guessed at** — discovery is a hint layer, and a malformed hint is no hint.
pub fn parse_sessions(body: &serde_json::Value) -> Vec<SessionAd> {
    let list = body
        .get("data")
        .and_then(|d| d.get("sessions"))
        .or_else(|| body.get("sessions"))
        .or_else(|| body.get("data"))
        .unwrap_or(body);
    match list.as_array() {
        Some(items) => items
            .iter()
            .filter_map(|v| serde_json::from_value::<SessionAd>(v.clone()).ok())
            .collect(),
        None => Vec::new(),
    }
}

/// Build a tracker-backed [`Discovery`](magnetite_seams::discovery::Discovery)
/// from the environment, or `None` when no tracker is configured.
///
/// **Zero-config default is LAN.** Callers treat `None` as "use `LanDiscovery`",
/// so nothing external is ever required to run a node.
pub fn from_env<I: Identity + Send + Sync>(
    node_key: I,
) -> Option<TrackerDiscovery<HttpTrackerClient<I>>> {
    let url = std::env::var(TRACKER_URL_ENV).ok()?;
    let url = url.trim().to_string();
    if url.is_empty() {
        return None;
    }
    Some(TrackerDiscovery::new(url, HttpTrackerClient::new(node_key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::comms::RoomAddr;
    use magnetite_seams::discovery::{Capacity, NodeAddr, Price};
    use magnetite_seams::identity::RawKeypairAuth;

    fn ad() -> SessionAd {
        SessionAd {
            game: Hash::of(b"content-addressed game"),
            node: NodeAddr("box.example:9000".into()),
            operator: Some("pareto".into()),
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
            chat_room: Some(RoomAddr("builtin://lobby/ab".into())),
            voice_room: None,
        }
    }

    #[test]
    fn announce_envelope_is_signed_by_this_node_and_verifies() {
        let key = RawKeypairAuth::from_seed([5u8; 32]);
        let expected = key.pubkey();
        let c = HttpTrackerClient::with_ttl(key, 120);
        let env = c.envelope(&ad(), 1_000);

        assert_eq!(env.node_key, expected, "the ad names this node's key");
        assert_eq!(env.expires_at, 1_120, "lease is issued_at + ttl");
        env.verify::<RawKeypairAuth>(1_000, 60)
            .expect("a tracker must accept our own honest announce");
    }

    #[test]
    fn an_announce_cannot_be_edited_in_flight() {
        let c = HttpTrackerClient::with_ttl(RawKeypairAuth::from_seed([5u8; 32]), 120);
        let mut env = c.envelope(&ad(), 1_000);
        // A man-in-the-middle drops the price to look like the cheapest host.
        env.ad.price = None;
        assert!(
            env.verify::<RawKeypairAuth>(1_000, 60).is_err(),
            "an edited ad must not verify"
        );
    }

    #[test]
    fn ttl_is_clamped_to_the_protocol_maximum() {
        let c = HttpTrackerClient::with_ttl(RawKeypairAuth::from_seed([5u8; 32]), u64::MAX);
        assert_eq!(c.ttl_secs(), MAX_AD_TTL_SECS);
        let env = c.envelope(&ad(), 1_000);
        env.verify::<RawKeypairAuth>(1_000, 60)
            .expect("a clamped lease is still acceptable");
    }

    #[test]
    fn withdrawal_is_bound_to_the_ad_slot() {
        let key = RawKeypairAuth::from_seed([5u8; 32]);
        let expected = key.pubkey();
        let c = HttpTrackerClient::new(key);
        let a = ad();
        let w = c.withdrawal(&a, 1_000);
        assert_eq!(w.game, a.game);
        assert_eq!(w.node, a.node);
        assert_eq!(w.node_key, expected);
        w.verify::<RawKeypairAuth>(1_005, 300).unwrap();
    }

    #[test]
    fn parse_sessions_reads_every_envelope_shape() {
        let one = serde_json::to_value(ad()).unwrap();
        let bare = serde_json::json!([one.clone()]);
        let wrapped = serde_json::json!({ "sessions": [one.clone()] });
        let api = serde_json::json!({ "success": true, "data": { "sessions": [one.clone()] } });

        for body in [&bare, &wrapped, &api] {
            let got = parse_sessions(body);
            assert_eq!(got.len(), 1, "envelope {body} should yield one ad");
            assert_eq!(got[0], ad());
        }
    }

    #[test]
    fn parse_sessions_keeps_ads_with_tracker_bookkeeping_and_drops_junk() {
        let mut with_extras = serde_json::to_value(ad()).unwrap();
        with_extras["id"] = serde_json::json!("00000000-0000-0000-0000-000000000000");
        with_extras["node_key"] = serde_json::json!("ab");
        with_extras["expires_at"] = serde_json::json!(1_120);

        let body = serde_json::json!({
            "sessions": [with_extras, serde_json::json!({"game": "not-a-hash"})]
        });
        let got = parse_sessions(&body);
        assert_eq!(got.len(), 1, "a malformed ad is skipped, never guessed at");
        assert_eq!(got[0], ad());
    }

    #[test]
    fn no_tracker_url_means_lan_only() {
        std::env::remove_var(TRACKER_URL_ENV);
        assert!(
            from_env(RawKeypairAuth::from_seed([5u8; 32])).is_none(),
            "a node must need no external service by default"
        );
        std::env::set_var(TRACKER_URL_ENV, "   ");
        assert!(
            from_env(RawKeypairAuth::from_seed([5u8; 32])).is_none(),
            "a blank TRACKER_URL is not a tracker"
        );
        std::env::remove_var(TRACKER_URL_ENV);
    }
}
