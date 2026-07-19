// The `CommsProvider` seam (DECENTRALIZATION.md §3.5).
//
// **Magnetite builds no chat, no voice, no video, no streaming.** Everything
// social is an adapter behind one trait, selected by `COMMS_PROVIDER`:
//
//     builtin (default) │ matrix │ jitsi │ livekit │ owncast
//
// `builtin` is the DEMOTED in-house stack — `services/communities.rs`,
// `services/streaming.rs`, `ws/comms.rs`, `ws/voice.rs`, `api/{communities,
// channels,messages,streaming}.rs` and MediaMTX. It still works, it is still
// the zero-configuration default, and it is deliberately NOT deleted. It is
// simply one provider among several rather than the only path.
//
// Three invariants hold no matter which provider is wired:
//
//   1. **Identity is the keypair.** Join credentials are always minted by the
//      node via `AuthProvider::mint_scoped_token` and only then projected into
//      the target system's format (`bridge.rs`). One login → SSO into comms.
//   2. **Nothing external is required.** Every non-builtin adapter is
//      config-gated; an unconfigured provider falls back to `builtin` with a
//      warning instead of failing. CI needs no Matrix/Jitsi/LiveKit.
//   3. **Media is per-node.** A room carries its OWN `media_host`; there is no
//      single global media server. Each operator runs their own.
//
// Guardrail (§6): no provider-specific type escapes this module. Callers see
// `RoomAddr`, `JoinCred`, [`ClientCred`] and [`RoomRecord`] only.

#![allow(dead_code)]

pub mod api;
pub mod bridge;
pub mod gate;
pub mod providers;

use std::sync::OnceLock;

use sqlx::PgPool;
use uuid::Uuid;

#[allow(unused_imports)]
pub use bridge::{node_pubkey, ClientCred};
#[allow(unused_imports)]
pub use magnetite_seams::comms::{CommsProvider, JoinCred, RoomAddr, RoomScope};
pub use magnetite_seams::identity::PubKey;

use crate::error::{AppError, Result};
use providers::{
    Adapter, BuiltinAdapter, JitsiAdapter, LiveKitAdapter, MatrixAdapter, OwncastAdapter,
};

/// The process-wide comms provider, chosen once from `COMMS_PROVIDER`.
///
/// Boxed as `dyn Adapter` so the concrete provider type never appears in a
/// signature outside this module.
pub fn provider() -> &'static (dyn Adapter + Send + Sync) {
    static P: OnceLock<Box<dyn Adapter + Send + Sync>> = OnceLock::new();
    P.get_or_init(|| select(&std::env::var("COMMS_PROVIDER").unwrap_or_else(|_| "builtin".into())))
        .as_ref()
}

/// Build the adapter named by `kind`, degrading to `builtin` when the named
/// provider is unknown or its service is not configured.
fn select(kind: &str) -> Box<dyn Adapter + Send + Sync> {
    let requested = kind.trim().to_ascii_lowercase();
    let built: Option<Box<dyn Adapter + Send + Sync>> = match requested.as_str() {
        "builtin" | "" => Some(Box::new(BuiltinAdapter::new())),
        "matrix" => MatrixAdapter::from_env().map(|a| Box::new(a) as _),
        "jitsi" => JitsiAdapter::from_env().map(|a| Box::new(a) as _),
        "livekit" => LiveKitAdapter::from_env().map(|a| Box::new(a) as _),
        "owncast" | "peertube" => OwncastAdapter::from_env().map(|a| Box::new(a) as _),
        other => {
            tracing::warn!("COMMS_PROVIDER={other} is not a known provider — using `builtin`");
            None
        }
    };
    match built {
        Some(a) => {
            tracing::info!("comms: provider `{}` active", a.name());
            a
        }
        None => {
            if !requested.is_empty() && requested != "builtin" {
                tracing::warn!(
                    "comms: COMMS_PROVIDER={requested} is not configured (missing its \
                     *_HOMESERVER / *_DOMAIN / *_URL env var) — falling back to the offline \
                     `builtin` provider"
                );
            }
            Box::new(BuiltinAdapter::new())
        }
    }
}

/// The active provider's name (`builtin` unless configured otherwise).
pub fn provider_name() -> &'static str {
    provider().name()
}

// ─── Room registry ───────────────────────────────────────────────────────────

/// A persisted room: the seam address plus the local rows it fronts.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct RoomRecord {
    pub id: Uuid,
    pub addr: String,
    pub provider: String,
    pub scope: String,
    pub scope_ref: Option<String>,
    /// This room's OWN media host — per-node, never a global media server.
    pub media_host: Option<String>,
    pub community_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    /// >0 → a verified payment receipt is required to join (§3.6).
    pub price_units: i64,
}

/// Encode a scope for storage as `(tag, ref)`.
fn scope_columns(scope: &RoomScope) -> (&'static str, Option<String>) {
    match scope {
        RoomScope::Match(h) => ("match", Some(h.to_hex())),
        RoomScope::Lobby => ("lobby", None),
        RoomScope::Community(id) => ("community", Some(id.clone())),
        RoomScope::Voice => ("voice", None),
        RoomScope::Video => ("video", None),
        RoomScope::Stream => ("stream", None),
    }
}

/// Create a room through the active provider and persist it.
///
/// `media_host` lets the caller pin the room to a specific media server; when
/// omitted the provider's configured default is recorded, so the room is still
/// self-describing rather than depending on process-wide env at read time.
pub async fn create_room(
    pool: &PgPool,
    scope: RoomScope,
    created_by: Option<Uuid>,
    community_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    media_host: Option<String>,
    price_units: i64,
) -> Result<RoomRecord> {
    let p = provider();
    let (tag, scope_ref) = scope_columns(&scope);
    let addr = p.create_room(scope).await;
    let host = media_host.or_else(|| p.default_media_host());
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO comms_rooms
            (id, addr, provider, scope, scope_ref, media_host,
             community_id, channel_id, price_units, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(id)
    .bind(&addr.0)
    .bind(p.name())
    .bind(tag)
    .bind(&scope_ref)
    .bind(&host)
    .bind(community_id)
    .bind(channel_id)
    .bind(price_units.max(0))
    .bind(created_by)
    .execute(pool)
    .await?;

    Ok(RoomRecord {
        id,
        addr: addr.0,
        provider: p.name().to_string(),
        scope: tag.to_string(),
        scope_ref,
        media_host: host,
        community_id,
        channel_id,
        price_units: price_units.max(0),
    })
}

/// Look a room up by id.
pub async fn get_room(pool: &PgPool, id: Uuid) -> Result<RoomRecord> {
    sqlx::query_as::<_, RoomRecord>(
        r#"SELECT id, addr, provider, scope, scope_ref, media_host,
                  community_id, channel_id, price_units
             FROM comms_rooms WHERE id = $1 AND closed_at IS NULL"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Room not found".to_string()))
}

/// Mint a join credential, enforcing the receipt gate for paid rooms.
///
/// This is the ONLY sanctioned way to obtain a `ClientCred`: it fuses the three
/// rules — pay first (§3.6), node mints (§3.1), provider renders (§3.5).
///
/// A PAID room additionally requires a PROVEN key ([`AccountKey::for_authorization`]).
/// Admission to a paid room is a purchased grant bound to a keypair, so a
/// naming-only derived key must not reach it — see [`AccountKey`]. Free rooms
/// only ever address, so unlinked legacy accounts still join them.
pub async fn join(
    pool: &PgPool,
    room_id: Uuid,
    user_id: Uuid,
    user_key: AccountKey,
) -> Result<ClientCred> {
    let room = get_room(pool, room_id).await?;

    let key = if room.price_units > 0 {
        // Fail closed BEFORE the receipt lookup: no proven key, no paid room.
        let key = user_key.for_authorization()?;
        gate::require_paid(pool, user_id, room.id, room.price_units).await?;
        key
    } else {
        user_key.for_addressing()
    };

    let p = provider();
    let addr = RoomAddr(room.addr.clone());
    let cred = p.issue_join_credential(&key, &addr).await;
    Ok(p.render(&cred, room.media_host.as_deref()))
}

/// Mint a credential for an address that is not (yet) in `comms_rooms`.
///
/// This is the bridge the DEMOTED builtin surfaces use: an existing
/// `voice_rooms` / `streams` row hands us its address and its own media host,
/// and gets back the same provider-agnostic credential a seam-native room would
/// produce. That is what makes the old stack one provider rather than the only
/// path — the endpoint's contract stops depending on it.
///
/// ADDRESSING ONLY: these legacy surfaces are already authorized by the caller's
/// session (and by their own membership rows) before we get here; the key merely
/// names the joiner to the provider. Anything that gates on payment or on key
/// possession must go through [`join`], which demands a proven key.
pub async fn credential_for(
    addr: &str,
    media_host: Option<&str>,
    user_key: AccountKey,
) -> ClientCred {
    let p = provider();
    let cred = p
        .issue_join_credential(&user_key.for_addressing(), &RoomAddr(addr.to_string()))
        .await;
    p.render(&cred, media_host)
}

/// Close a room: tear it down at the provider, then mark the row closed.
pub async fn close_room(pool: &PgPool, room_id: Uuid, user_id: Uuid) -> Result<()> {
    let room = get_room(pool, room_id).await?;
    let owned: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT created_by FROM comms_rooms WHERE id = $1")
            .bind(room_id)
            .fetch_optional(pool)
            .await?;
    if !matches!(owned, Some((Some(owner),)) if owner == user_id) {
        return Err(AppError::Forbidden(
            "Only the room creator can close this room".to_string(),
        ));
    }

    provider()
        .teardown(&RoomAddr(room.addr))
        .await
        .map_err(|e| AppError::Internal(format!("comms teardown failed: {e}")))?;

    sqlx::query("UPDATE comms_rooms SET closed_at = NOW() WHERE id = $1")
        .bind(room_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Where an account's comms key came from — and therefore what it may be used for.
///
/// This distinction is a SECURITY boundary, not bookkeeping. See [`AccountKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyProvenance {
    /// The account linked a real Ed25519 key it controls. Holding the matching
    /// secret is provable, so this key may back an authorization decision.
    Linked,
    /// A key derived deterministically from the account id (SHA-256 of a
    /// domain-separated account UUID). Nobody holds a secret for it — it is a
    /// public, guessable *name*. It may address and route; it may never prove.
    Derived,
}

/// The identity key a local account presents to comms, tagged with its provenance.
///
/// # Why this is a type and not a `PubKey`
///
/// A derived key NAMES an account, it does not PROVE one: it is a pure function
/// of a UUID, so anyone who learns the account id can recompute it and no secret
/// exists to sign with. Passing a bare `PubKey` around made "named" and "proven"
/// indistinguishable at the call site, which is exactly the shape of an
/// authorization bypass. The two uses are therefore separated in the type:
///
/// * [`AccountKey::for_addressing`] — infallible. Routing, room membership,
///   display names, builtin-provider handles. Nothing is granted by knowing it.
/// * [`AccountKey::for_authorization`] — fallible. Anything where possessing the
///   key IS the credential: paid-room admission, receipt ownership, wallet
///   binding, externally-minted scoped tokens. Derived keys are REJECTED here.
///
/// Legacy accounts that never linked a key keep working on the demoted builtin
/// path (free rooms, chat, voice) because that path only ever addresses.
///
/// TODO(identity): drop [`KeyProvenance::Derived`] entirely once every account
/// carries a real pubkey; then this type collapses back to a `PubKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountKey {
    key: PubKey,
    provenance: KeyProvenance,
}

impl AccountKey {
    /// A proven key the account demonstrably controls.
    pub fn linked(key: PubKey) -> Self {
        Self { key, provenance: KeyProvenance::Linked }
    }

    /// A naming-only key derived from an account id. Cannot authorize.
    pub fn derived(user_id: Uuid) -> Self {
        Self { key: derived_key(user_id), provenance: KeyProvenance::Derived }
    }

    pub fn provenance(&self) -> KeyProvenance {
        self.provenance
    }

    /// True only for keys whose secret the account actually holds.
    pub fn is_proven(&self) -> bool {
        self.provenance == KeyProvenance::Linked
    }

    /// The key, for ROUTING/ADDRESSING only (room handles, display, membership).
    ///
    /// Safe for every account because knowing this key grants nothing.
    pub fn for_addressing(&self) -> PubKey {
        self.key
    }

    /// The key, for AUTHORIZATION — where holding it is the credential.
    ///
    /// Fails closed for derived keys: they are guessable from a public account
    /// id, so honouring one would let anyone who knows a UUID act as that user.
    pub fn for_authorization(&self) -> Result<PubKey> {
        match self.provenance {
            KeyProvenance::Linked => Ok(self.key),
            KeyProvenance::Derived => Err(AppError::Validation(DERIVED_KEY_CANNOT_AUTHORIZE.to_string())),
        }
    }
}

/// Refusal message when a naming-only key is offered as a credential.
pub const DERIVED_KEY_CANNOT_AUTHORIZE: &str =
    "this account has no linked identity key — link a wallet/keypair before \
     using a credential that grants access (a derived account key names you, \
     it cannot prove you)";

/// The identity key a local account presents to comms.
///
/// Prefers the account's linked wallet/identity key (keypair identity is the
/// default under §3.1); falls back to a naming-only derived key so the demoted
/// builtin path keeps working for legacy accounts that have not linked one.
/// The returned [`AccountKey`] carries which of the two it is — callers must go
/// through [`AccountKey::for_authorization`] before treating it as a credential.
pub async fn user_key(pool: &PgPool, user_id: Uuid) -> AccountKey {
    let linked: Option<(Option<String>,)> =
        sqlx::query_as("SELECT wallet_address FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if let Some(pk) = linked
        .and_then(|r| r.0)
        .and_then(|h| PubKey::from_hex(h.trim_start_matches("0x")).ok())
    {
        return AccountKey::linked(pk);
    }
    AccountKey::derived(user_id)
}

/// Deterministic, non-secret key naming a local account.
///
/// PUBLIC INPUT, NO SECRET: this is `SHA-256(domain ‖ account uuid)`. It exists
/// so unlinked accounts still have a stable comms handle. Never authorize on it
/// — use [`AccountKey`], which enforces that at the type level.
pub(crate) fn derived_key(user_id: Uuid) -> PubKey {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"magnetite/comms/derived-account-key/v1");
    h.update(user_id.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&h.finalize());
    PubKey(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_unconfigured_providers_fall_back_to_builtin() {
        assert_eq!(select("builtin").name(), "builtin");
        assert_eq!(select("").name(), "builtin");
        assert_eq!(select("nonsense").name(), "builtin");
        // Named but not configured → still builtin, never an error.
        temp_env::with_vars(
            [
                ("MATRIX_HOMESERVER", None::<&str>),
                ("JITSI_DOMAIN", None),
                ("LIVEKIT_URL", None),
                ("OWNCAST_URL", None),
            ],
            || {
                assert_eq!(select("matrix").name(), "builtin");
                assert_eq!(select("jitsi").name(), "builtin");
                assert_eq!(select("livekit").name(), "builtin");
                assert_eq!(select("owncast").name(), "builtin");
            },
        );
    }

    #[test]
    fn configured_providers_are_selected_and_case_insensitive() {
        temp_env::with_var("JITSI_DOMAIN", Some("meet.example.org"), || {
            assert_eq!(select("Jitsi").name(), "jitsi");
            assert_eq!(select("  JITSI  ").name(), "jitsi");
        });
        temp_env::with_var("MATRIX_HOMESERVER", Some("https://m.example.org"), || {
            assert_eq!(select("matrix").name(), "matrix");
        });
    }

    #[test]
    fn scope_columns_round_trip_their_reference() {
        assert_eq!(scope_columns(&RoomScope::Lobby), ("lobby", None));
        let (tag, r) = scope_columns(&RoomScope::Community("abc".into()));
        assert_eq!((tag, r), ("community", Some("abc".to_string())));
        let (tag, r) = scope_columns(&RoomScope::Match(magnetite_seams::blobstore::Hash::of(b"g")));
        assert_eq!(tag, "match");
        assert_eq!(r.unwrap().len(), 64, "game hash is recorded in full");
    }

    #[test]
    fn derived_account_keys_are_stable_and_distinct() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert_eq!(derived_key(a), derived_key(a));
        assert_ne!(derived_key(a), derived_key(b));
    }
}
