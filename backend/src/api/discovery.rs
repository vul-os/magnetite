// Discovery API (DECENTRALIZATION.md §3.4) — a **tracker**, not an authority.
//
// This endpoint set is a phonebook. Nodes self-advertise the sessions they host
// (`SessionAd`); clients query for sessions of a game. The tracker:
//
//   * decides NOTHING about who may host, what a game is, or what it costs,
//   * holds only SOFT state — every ad is a short lease that must be renewed by
//     heartbeat, so a dead node's entry evaporates on its own,
//   * is redundant and swappable — a client may query several trackers, or none
//     at all and use LAN discovery instead.
//
// The one thing it *does* enforce is authorship: an announce must be signed by
// the hosting node's key (`SignedAd`), and a re-announce or withdrawal for an
// existing `(game, node)` slot must come from the same key. A phonebook that
// accepts unsigned entries lets anyone list someone else's number, spoof a
// cheaper price, or advertise capacity a box does not have. Verification fails
// CLOSED — an unsigned, forged, expired, or over-long lease is refused.
//
// Route summary
// ─────────────
//   POST   /api/v1/discovery/announce   — node self-advertises (signed, leased)
//   GET    /api/v1/discovery/sessions   — query live ads (backs the server browser)
//   DELETE /api/v1/discovery/announce   — signed deregister on clean shutdown
//
// Restart behaviour: ads are persisted in `discovery_ads` with their expiry, so
// a tracker restart does not blank the phonebook — but nothing is authoritative
// about that table either. Every row lapses within `MAX_AD_TTL_SECS` unless the
// hosting node keeps heartbeating, so a tracker restored from an old backup
// converges to the truth (whatever nodes are actually up) within minutes.

use axum::{
    extract::{Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use magnetite_seams::discovery::{
    Capacity, Filter, NodeAddr, Price, SessionAd, SignedAd, SignedWithdraw,
};
use magnetite_seams::identity::RawKeypairAuth;
use magnetite_seams::{blobstore::Hash, comms::RoomAddr};

use crate::api::response::{self, ApiResponse};
use crate::error::{AppError, Result};

/// Clock skew tolerated on an announce's `issued_at`.
const CLOCK_SKEW_SECS: u64 = 120;
/// How long a signed withdrawal stays usable (replay window).
const WITHDRAW_MAX_AGE_SECS: u64 = 300;
/// Cap on how many ads one query returns.
const MAX_PAGE: i64 = 500;
/// Longest opaque string we will store from an ad (node addr, room addr, …).
const MAX_STR: usize = 255;

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// POST body: a signed ad plus optional live occupancy counters.
///
/// `players`/`max_players` are *display* hints for the server browser and are
/// deliberately OUTSIDE the signed `SessionAd` — they change every few seconds
/// and are already implied by `capacity.free_slots`, which IS signed.
#[derive(Debug, Deserialize)]
pub struct AnnounceRequest {
    #[serde(flatten)]
    pub signed: SignedAd,
    pub players: Option<i32>,
    pub max_players: Option<i32>,
}

/// What an announce returns: when the lease lapses and when to heartbeat.
#[derive(Debug, Serialize)]
pub struct AnnounceAck {
    pub id: Uuid,
    /// Unix seconds the tracker will keep this ad without a heartbeat.
    pub expires_at: i64,
    /// Suggested heartbeat interval (half the lease).
    pub heartbeat_in: i64,
}

/// One row of the phonebook.
///
/// The `SessionAd` fields are flattened so the payload is *exactly* the seam
/// shape (`game`, `node`, `capacity`, `ping_hint`, `price`, `chat_room`,
/// `voice_room`) with tracker bookkeeping alongside it.
#[derive(Debug, Serialize)]
pub struct AdView {
    pub id: Uuid,
    #[serde(flatten)]
    pub ad: SessionAd,
    /// The key that signed this ad — clients can re-verify authorship.
    pub node_key: String,
    pub players: Option<i32>,
    pub max_players: Option<i32>,
    pub expires_at: i64,
}

/// GET response envelope. `{ sessions: [...] }` under the standard `data` key.
#[derive(Debug, Serialize)]
pub struct SessionList {
    pub sessions: Vec<AdView>,
    /// How many live ads matched (before `limit`).
    pub total: i64,
}

/// Query string for the server browser.
#[derive(Debug, Default, Deserialize)]
pub struct SessionQuery {
    /// Content address (hex) to narrow to.
    pub game: Option<String>,
    /// Drop ads slower than this.
    pub max_ping: Option<u32>,
    /// Drop full ads.
    #[serde(default)]
    pub free_slots_only: bool,
    /// Drop ads that charge a hosting fee.
    #[serde(default)]
    pub free_only: bool,
    /// Drop ads pricier than this (smallest currency unit).
    pub max_price: Option<u64>,
    pub limit: Option<i64>,
}

impl SessionQuery {
    /// The seam-level [`Filter`] this query expresses. Filtering is a *client*
    /// concern — the tracker applies it only as a convenience/bandwidth saver,
    /// and `Discovery::find` applies the same filter again on the client.
    pub fn to_filter(&self) -> Filter {
        Filter {
            max_ping: self.max_ping,
            require_free_slot: self.free_slots_only,
            max_price: if self.free_only {
                Some(0)
            } else {
                self.max_price
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Pure validation (no database — unit-testable offline)
// ---------------------------------------------------------------------------

fn check_len(what: &str, s: &str) -> Result<()> {
    if s.is_empty() {
        return Err(AppError::Validation(format!("{what} must not be empty")));
    }
    if s.len() > MAX_STR {
        return Err(AppError::Validation(format!(
            "{what} exceeds {MAX_STR} bytes"
        )));
    }
    Ok(())
}

/// Sanity-check the *contents* of an ad. This is anti-garbage, not policy: the
/// tracker still expresses no opinion on price, capacity, or who may host.
pub fn validate_ad(ad: &SessionAd) -> Result<()> {
    check_len("node address", &ad.node.0)?;
    if let Some(p) = &ad.price {
        check_len("price currency", &p.currency)?;
        check_len("price unit", &p.unit)?;
    }
    if let Some(r) = &ad.chat_room {
        check_len("chat room", &r.0)?;
    }
    if let Some(r) = &ad.voice_room {
        check_len("voice room", &r.0)?;
    }
    if ad.capacity.free_slots > 1_000_000 || ad.capacity.max_shards > 100_000 {
        return Err(AppError::Validation("implausible capacity".into()));
    }
    Ok(())
}

/// Verify an announce envelope: authorship, lease window, and contents.
///
/// **Fails closed.** Every rejection path returns an error; there is no branch
/// that stores an ad whose signature did not verify.
pub fn verify_announce(signed: &SignedAd, now: u64) -> Result<()> {
    signed
        .verify::<RawKeypairAuth>(now, CLOCK_SKEW_SECS)
        .map_err(|e| AppError::Validation(format!("announce refused: {e}")))?;
    validate_ad(&signed.ad)
}

/// Verify a deregister envelope.
pub fn verify_withdraw(w: &SignedWithdraw, now: u64) -> Result<()> {
    w.verify::<RawKeypairAuth>(now, WITHDRAW_MAX_AGE_SECS)
        .map_err(|e| AppError::Validation(format!("withdrawal refused: {e}")))
}

/// One persisted ad row, as read back for the server browser.
#[derive(Debug, sqlx::FromRow)]
struct AdRow {
    id: Uuid,
    game_hash: String,
    node_addr: String,
    node_key: String,
    cpu_cores: i32,
    ram_mb: i64,
    bandwidth_mbps: i32,
    free_slots: i32,
    max_shards: i32,
    ping_hint: i32,
    price_amount: Option<i64>,
    price_currency: Option<String>,
    price_unit: Option<String>,
    chat_room: Option<String>,
    voice_room: Option<String>,
    players: Option<i32>,
    max_players: Option<i32>,
    expires_at: i64,
}

/// Rebuild a [`SessionAd`] from its stored columns.
#[allow(clippy::too_many_arguments)]
fn ad_from_row(
    game_hash: &str,
    node_addr: String,
    cpu_cores: i32,
    ram_mb: i64,
    bandwidth_mbps: i32,
    free_slots: i32,
    max_shards: i32,
    ping_hint: i32,
    price_amount: Option<i64>,
    price_currency: Option<String>,
    price_unit: Option<String>,
    chat_room: Option<String>,
    voice_room: Option<String>,
) -> Result<SessionAd> {
    Ok(SessionAd {
        game: Hash::from_hex(game_hash)
            .map_err(|e| AppError::Internal(format!("stored game hash is corrupt: {e}")))?,
        node: NodeAddr(node_addr),
        capacity: Capacity {
            cpu_cores: cpu_cores.max(0) as u32,
            ram_mb: ram_mb.max(0) as u64,
            bandwidth_mbps: bandwidth_mbps.max(0) as u32,
            free_slots: free_slots.max(0) as u32,
            max_shards: max_shards.max(0) as u32,
        },
        ping_hint: ping_hint.max(0) as u32,
        price: match (price_amount, price_currency, price_unit) {
            (Some(amount), Some(currency), Some(unit)) => Some(Price {
                amount: amount.max(0) as u64,
                currency,
                unit,
            }),
            _ => None,
        },
        chat_room: chat_room.map(RoomAddr),
        voice_room: voice_room.map(RoomAddr),
    })
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/discovery/announce
///
/// A node self-advertises a session it hosts. The ad must be signed by the
/// node's key; a re-announce for the same `(game, node)` refreshes the lease
/// but only when it comes from the SAME key — one node cannot overwrite
/// another's listing.
pub async fn announce(
    State(pool): State<PgPool>,
    Json(req): Json<AnnounceRequest>,
) -> Result<Json<ApiResponse<AnnounceAck>>> {
    let now = now_unix();
    verify_announce(&req.signed, now)?;

    let ad = &req.signed.ad;
    let node_key = req.signed.node_key.to_hex();
    let game_hex = ad.game.to_hex();
    let (price_amount, price_currency, price_unit) = match &ad.price {
        Some(p) => (
            Some(p.amount as i64),
            Some(p.currency.clone()),
            Some(p.unit.clone()),
        ),
        None => (None, None, None),
    };

    // Opportunistic sweep: expired leases are not truth, so never serve them.
    let _ = sqlx::query("DELETE FROM discovery_ads WHERE expires_at <= $1")
        .bind(now as i64)
        .execute(&pool)
        .await;

    // Upsert, but the WHERE clause makes the update a no-op for a different
    // key — so a hijack attempt affects 0 rows and is reported as a conflict.
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO discovery_ads (
            game_hash, node_addr, node_key,
            cpu_cores, ram_mb, bandwidth_mbps, free_slots, max_shards,
            ping_hint, price_amount, price_currency, price_unit,
            chat_room, voice_room, players, max_players,
            issued_at, expires_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
        ON CONFLICT (game_hash, node_addr) DO UPDATE SET
            cpu_cores = EXCLUDED.cpu_cores,
            ram_mb = EXCLUDED.ram_mb,
            bandwidth_mbps = EXCLUDED.bandwidth_mbps,
            free_slots = EXCLUDED.free_slots,
            max_shards = EXCLUDED.max_shards,
            ping_hint = EXCLUDED.ping_hint,
            price_amount = EXCLUDED.price_amount,
            price_currency = EXCLUDED.price_currency,
            price_unit = EXCLUDED.price_unit,
            chat_room = EXCLUDED.chat_room,
            voice_room = EXCLUDED.voice_room,
            players = EXCLUDED.players,
            max_players = EXCLUDED.max_players,
            issued_at = EXCLUDED.issued_at,
            expires_at = EXCLUDED.expires_at
        WHERE discovery_ads.node_key = EXCLUDED.node_key
        RETURNING id
        "#,
    )
    .bind(&game_hex)
    .bind(&ad.node.0)
    .bind(&node_key)
    .bind(ad.capacity.cpu_cores as i32)
    .bind(ad.capacity.ram_mb as i64)
    .bind(ad.capacity.bandwidth_mbps as i32)
    .bind(ad.capacity.free_slots as i32)
    .bind(ad.capacity.max_shards as i32)
    .bind(ad.ping_hint as i32)
    .bind(price_amount)
    .bind(price_currency)
    .bind(price_unit)
    .bind(ad.chat_room.as_ref().map(|r| r.0.clone()))
    .bind(ad.voice_room.as_ref().map(|r| r.0.clone()))
    .bind(req.players)
    .bind(req.max_players)
    .bind(req.signed.issued_at as i64)
    .bind(req.signed.expires_at as i64)
    .fetch_optional(&pool)
    .await?;

    let id = row
        .map(|r| r.0)
        .ok_or_else(|| {
            AppError::Forbidden(
                "this (game, node) slot is held by a different node key".to_string(),
            )
        })?;

    let expires_at = req.signed.expires_at as i64;
    Ok(response::success_response(AnnounceAck {
        id,
        expires_at,
        heartbeat_in: ((expires_at - now as i64) / 2).max(1),
    }))
}

/// GET /api/v1/discovery/sessions
///
/// Query the phonebook. Only live (unexpired) leases are ever returned.
pub async fn sessions(
    State(pool): State<PgPool>,
    Query(q): Query<SessionQuery>,
) -> Result<Json<ApiResponse<SessionList>>> {
    let now = now_unix() as i64;
    let limit = q.limit.unwrap_or(MAX_PAGE).clamp(1, MAX_PAGE);

    // Narrowing by game is done in SQL (it is the common case and the index);
    // the remaining predicates run through the seam `Filter` so the tracker and
    // the client agree on exactly what "has a free slot" means.
    let game_hex = match &q.game {
        Some(g) => Some(
            Hash::from_hex(g)
                .map_err(|_| {
                    AppError::BadRequest("game must be a 64-char BLAKE3 hex address".to_string())
                })?
                .to_hex(),
        ),
        None => None,
    };

    const COLS: &str = "id, game_hash, node_addr, node_key, cpu_cores, ram_mb, bandwidth_mbps, \
         free_slots, max_shards, ping_hint, price_amount, price_currency, price_unit, \
         chat_room, voice_room, players, max_players, expires_at";

    let rows: Vec<AdRow> = match &game_hex {
        Some(g) => {
            sqlx::query_as(&format!(
                "SELECT {COLS} FROM discovery_ads WHERE expires_at > $1 AND game_hash = $2 \
                 ORDER BY ping_hint ASC, expires_at DESC LIMIT $3"
            ))
            .bind(now)
            .bind(g)
            .bind(limit)
            .fetch_all(&pool)
            .await?
        }
        None => {
            sqlx::query_as(&format!(
                "SELECT {COLS} FROM discovery_ads WHERE expires_at > $1 \
                 ORDER BY ping_hint ASC, expires_at DESC LIMIT $2"
            ))
            .bind(now)
            .bind(limit)
            .fetch_all(&pool)
            .await?
        }
    };

    let filter = q.to_filter();
    let mut sessions = Vec::with_capacity(rows.len());
    for r in rows {
        let ad = ad_from_row(
            &r.game_hash,
            r.node_addr,
            r.cpu_cores,
            r.ram_mb,
            r.bandwidth_mbps,
            r.free_slots,
            r.max_shards,
            r.ping_hint,
            r.price_amount,
            r.price_currency,
            r.price_unit,
            r.chat_room,
            r.voice_room,
        )?;
        if !filter.accepts(&ad) {
            continue;
        }
        sessions.push(AdView {
            id: r.id,
            ad,
            node_key: r.node_key,
            players: r.players,
            max_players: r.max_players,
            expires_at: r.expires_at,
        });
    }

    let total = sessions.len() as i64;
    Ok(response::success_response(SessionList { sessions, total }))
}

/// DELETE /api/v1/discovery/announce
///
/// Clean-shutdown deregister. Requires a signature from the same node key that
/// owns the ad — the SQL predicate, not just the envelope check, enforces it.
pub async fn withdraw(
    State(pool): State<PgPool>,
    Json(w): Json<SignedWithdraw>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    let now = now_unix();
    verify_withdraw(&w, now)?;

    let affected = sqlx::query(
        "DELETE FROM discovery_ads WHERE game_hash = $1 AND node_addr = $2 AND node_key = $3",
    )
    .bind(w.game.to_hex())
    .bind(&w.node.0)
    .bind(w.node_key.to_hex())
    .execute(&pool)
    .await?
    .rows_affected();

    Ok(response::success_response(serde_json::json!({
        "removed": affected,
    })))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// The discovery router. Deliberately **unauthenticated** in the session sense:
/// a tracker has no accounts. Authorship is proven per-request by the node key
/// signature, which is stronger than a bearer token here — it binds the ad to
/// the box that will actually serve it.
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/announce", post(announce))
        .route("/announce", delete(withdraw))
        .route("/sessions", get(sessions))
        .with_state(pool)
}

// ---------------------------------------------------------------------------
// Tests (offline — no database, no network)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::identity::Identity;

    fn sample_ad(node: &str, free: u32, ping: u32, price: Option<u64>) -> SessionAd {
        SessionAd {
            game: Hash::of(b"a content-addressed game"),
            node: NodeAddr(node.into()),
            capacity: Capacity {
                cpu_cores: 8,
                ram_mb: 32768,
                bandwidth_mbps: 1000,
                free_slots: free,
                max_shards: 8,
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

    #[test]
    fn honest_announce_verifies() {
        let node = RawKeypairAuth::from_seed([3u8; 32]);
        let signed = SignedAd::sign(&node, sample_ad("box:9000", 4, 20, None), 1_000, 120);
        verify_announce(&signed, 1_000).unwrap();
    }

    #[test]
    fn unsigned_or_forged_announce_is_refused() {
        let honest = RawKeypairAuth::from_seed([3u8; 32]);
        let attacker = RawKeypairAuth::from_seed([4u8; 32]);

        // (a) Body swapped after signing — e.g. undercutting the price.
        let mut tampered =
            SignedAd::sign(&honest, sample_ad("box:9000", 4, 20, Some(500)), 1_000, 120);
        tampered.ad.price = None;
        assert!(verify_announce(&tampered, 1_000).is_err());

        // (b) Someone else's ad relabelled with the honest node's key.
        let mut spoofed = SignedAd::sign(&attacker, sample_ad("box:9000", 4, 20, None), 1_000, 120);
        spoofed.node_key = honest.pubkey();
        assert!(verify_announce(&spoofed, 1_000).is_err());

        // (c) A zeroed signature (the "unsigned" case) never passes.
        let mut unsigned = SignedAd::sign(&honest, sample_ad("box:9000", 4, 20, None), 1_000, 120);
        unsigned.sig = magnetite_seams::identity::Sig([0u8; 64]);
        assert!(verify_announce(&unsigned, 1_000).is_err());
    }

    #[test]
    fn expired_lease_is_refused() {
        let node = RawKeypairAuth::from_seed([3u8; 32]);
        let signed = SignedAd::sign(&node, sample_ad("box:9000", 4, 20, None), 1_000, 60);
        assert!(
            verify_announce(&signed, 2_000).is_err(),
            "a lapsed lease must not be re-admitted"
        );
    }

    #[test]
    fn garbage_ad_contents_are_refused() {
        let node = RawKeypairAuth::from_seed([3u8; 32]);

        let empty_node = SignedAd::sign(&node, sample_ad("", 4, 20, None), 1_000, 120);
        assert!(verify_announce(&empty_node, 1_000).is_err());

        let long = "x".repeat(MAX_STR + 1);
        let huge_node = SignedAd::sign(&node, sample_ad(&long, 4, 20, None), 1_000, 120);
        assert!(verify_announce(&huge_node, 1_000).is_err());

        let mut absurd = sample_ad("box:9000", 4, 20, None);
        absurd.capacity.free_slots = u32::MAX;
        let signed = SignedAd::sign(&node, absurd, 1_000, 120);
        assert!(verify_announce(&signed, 1_000).is_err());
    }

    #[test]
    fn withdrawal_must_be_signed_by_the_ad_owner() {
        let node = RawKeypairAuth::from_seed([3u8; 32]);
        let attacker = RawKeypairAuth::from_seed([4u8; 32]);
        let game = Hash::of(b"a content-addressed game");

        let ok = SignedWithdraw::sign(&node, game, NodeAddr("box:9000".into()), 1_000);
        verify_withdraw(&ok, 1_010).unwrap();

        let mut forged = SignedWithdraw::sign(&attacker, game, NodeAddr("box:9000".into()), 1_000);
        forged.node_key = node.pubkey();
        assert!(verify_withdraw(&forged, 1_010).is_err());

        assert!(
            verify_withdraw(&ok, 1_000 + WITHDRAW_MAX_AGE_SECS + 1).is_err(),
            "a withdrawal must not be replayable forever"
        );
    }

    #[test]
    fn query_maps_onto_the_seam_filter() {
        let q = SessionQuery {
            free_only: true,
            max_ping: Some(50),
            free_slots_only: true,
            ..Default::default()
        };
        let f = q.to_filter();
        assert_eq!(f.max_price, Some(0), "free_only means price <= 0");
        assert_eq!(f.max_ping, Some(50));
        assert!(f.require_free_slot);
    }

    #[test]
    fn ad_view_serializes_the_exact_seam_shape() {
        let view = AdView {
            id: Uuid::nil(),
            ad: sample_ad("box:9000", 4, 20, Some(500)),
            node_key: "ab".into(),
            players: Some(3),
            max_players: Some(16),
            expires_at: 1_060,
        };
        let v = serde_json::to_value(&view).unwrap();
        // Flattened SessionAd fields, verbatim from the seam.
        assert!(v["game"].is_string(), "game is a hex content address");
        assert_eq!(v["node"], "box:9000");
        assert_eq!(v["capacity"]["free_slots"], 4);
        assert_eq!(v["capacity"]["cpu_cores"], 8);
        assert_eq!(v["ping_hint"], 20);
        assert_eq!(v["price"]["amount"], 500);
        assert_eq!(v["price"]["currency"], "USDC");
        assert!(v["chat_room"].is_null());
        assert!(v["voice_room"].is_null());
        // Tracker bookkeeping rides alongside.
        assert_eq!(v["players"], 3);
        assert_eq!(v["max_players"], 16);
        assert_eq!(v["expires_at"], 1_060);
    }

    #[test]
    fn announce_request_parses_the_wire_form_a_node_sends() {
        let node = RawKeypairAuth::from_seed([3u8; 32]);
        let signed = SignedAd::sign(&node, sample_ad("box:9000", 4, 20, None), 1_000, 120);
        let mut body = serde_json::to_value(&signed).unwrap();
        body["players"] = serde_json::json!(2);
        body["max_players"] = serde_json::json!(16);

        let req: AnnounceRequest = serde_json::from_value(body).unwrap();
        assert_eq!(req.players, Some(2));
        assert_eq!(req.signed.ad.node, NodeAddr("box:9000".into()));
        verify_announce(&req.signed, 1_000).unwrap();
    }

    #[test]
    fn row_roundtrip_rebuilds_the_ad() {
        let ad = sample_ad("box:9000", 4, 20, Some(500));
        let back = ad_from_row(
            &ad.game.to_hex(),
            ad.node.0.clone(),
            8,
            32768,
            1000,
            4,
            8,
            20,
            Some(500),
            Some("USDC".into()),
            Some("per_seat".into()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(back, ad);
    }
}
