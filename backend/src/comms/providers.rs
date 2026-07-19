// The comms adapters (§3.5) — Magnetite builds none of this.
//
// Every social system is reached through one trait, `CommsProvider`. The
// adapters here are thin: they name a room, mint a node-signed join credential
// and render it into the target system's native format. No media bytes, no
// message storage, no signalling logic lives in Magnetite for anything except
// the DEMOTED `builtin` provider.
//
//   BuiltinProvider — wraps the old in-house stack (channels / voice_rooms /
//                     streams + ws/comms + ws/voice + MediaMTX). Default. Works
//                     fully offline with zero external services. NOT deleted —
//                     just no longer the only path.
//   MatrixProvider  — text / DMs / presence / spaces on a Matrix homeserver.
//   JitsiProvider   — voice + video SFU (room URL + JWT).
//   LiveKitProvider — voice + video at scale (room + access token).
//   OwncastProvider — live streaming / VOD.
//
// Every external provider is CONFIG-GATED: `from_env` returns `None` when its
// service is not configured, and the registry falls back to `builtin`. A
// default build never contacts anything.

use async_trait::async_trait;
use magnetite_seams::comms::{BuiltinProvider as SeamBuiltin, CommsProvider, JoinCred, RoomAddr, RoomScope};
use magnetite_seams::identity::PubKey;
use magnetite_seams::Result as SeamResult;
use rand::rngs::OsRng;
use rand::RngCore;

use super::bridge::{self, ClientCred};

/// Extra behaviour every Magnetite adapter adds on top of the bare seam trait:
/// a stable name, its default media host, and native credential rendering.
///
/// Kept in `comms/` so no provider-specific type ever escapes this module —
/// callers outside see `RoomAddr`, `JoinCred` and [`ClientCred`] only (§6).
pub trait Adapter: CommsProvider + Send + Sync {
    /// Stable provider name persisted on `comms_rooms.provider`.
    fn name(&self) -> &'static str;

    /// This provider's default media/service host, if it has one. A room row
    /// may override it — every operator runs their own media server (§3.5).
    fn default_media_host(&self) -> Option<String> {
        None
    }

    /// Project a seam credential into the client-usable shape.
    fn render(&self, cred: &JoinCred, media_host: Option<&str>) -> ClientCred {
        ClientCred::seam_only(
            self.name(),
            &cred.room.0,
            media_host
                .map(str::to_string)
                .or_else(|| self.default_media_host()),
            cred.token.clone(),
        )
    }
}

/// The room's provider-local name — the trailing segment of the address.
///
/// `matrix://#lobby:example.org` → `#lobby:example.org`, `livekit://abc` → `abc`.
pub fn room_name(addr: &RoomAddr) -> &str {
    match addr.0.split_once("://") {
        Some((_, rest)) => rest,
        None => &addr.0,
    }
}

fn fresh_id() -> String {
    let mut id = [0u8; 16];
    OsRng.fill_bytes(&mut id);
    hex::encode(id)
}

fn scope_tag(scope: &RoomScope) -> &'static str {
    match scope {
        RoomScope::Match(_) => "match",
        RoomScope::Lobby => "lobby",
        RoomScope::Community(_) => "community",
        RoomScope::Voice => "voice",
        RoomScope::Video => "video",
        RoomScope::Stream => "stream",
    }
}

// ─── builtin (the demoted in-house stack) ────────────────────────────────────

/// The old chat/voice/streaming stack, behind the seam.
///
/// Rooms are `builtin://<tag>/<hex>` and map onto the existing `channels`,
/// `voice_rooms` and `streams` rows served by `ws/comms.rs`, `ws/voice.rs` and
/// `api/streaming.rs`. Join credentials are node-signed like every other
/// provider's, so the identity story does not change when an operator later
/// switches to Matrix or LiveKit.
pub struct BuiltinAdapter {
    inner: SeamBuiltin<bridge::NodeAuth>,
}

impl Default for BuiltinAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinAdapter {
    pub fn new() -> Self {
        Self {
            inner: SeamBuiltin::new(bridge::NodeAuth),
        }
    }
    /// Whether the seam-level room is still live in this process.
    pub fn exists(&self, room: &RoomAddr) -> bool {
        self.inner.exists(room)
    }
}

#[async_trait]
impl CommsProvider for BuiltinAdapter {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr {
        self.inner.create_room(scope).await
    }
    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred {
        self.inner.issue_join_credential(user, room).await
    }
    async fn teardown(&self, room: &RoomAddr) -> SeamResult<()> {
        self.inner.teardown(room).await
    }
}

impl Adapter for BuiltinAdapter {
    fn name(&self) -> &'static str {
        "builtin"
    }
    /// Per-node: the operator's own media server, when they run one. There is
    /// deliberately no global default — an unset value simply means "this node
    /// serves no media", not "use the platform's".
    fn default_media_host(&self) -> Option<String> {
        std::env::var("MEDIA_SERVER_BASE_URL")
            .ok()
            .filter(|v| !v.is_empty())
    }
}

// ─── Matrix (text / DMs / presence / spaces) ─────────────────────────────────

/// Matrix homeserver adapter. Rooms are room aliases inside a Magnetite space.
pub struct MatrixAdapter {
    homeserver: String,
    server_name: String,
    alias_prefix: String,
    shared_secret: Option<String>,
}

impl MatrixAdapter {
    /// Config-gated: `None` unless `MATRIX_HOMESERVER` is set.
    pub fn from_env() -> Option<Self> {
        let homeserver = non_empty("MATRIX_HOMESERVER")?;
        // Server name defaults to the homeserver host (`https://x` → `x`).
        let server_name = non_empty("MATRIX_SERVER_NAME").unwrap_or_else(|| {
            homeserver
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_end_matches('/')
                .to_string()
        });
        Some(Self {
            homeserver,
            server_name,
            alias_prefix: non_empty("MATRIX_ALIAS_PREFIX").unwrap_or_else(|| "magnetite".into()),
            shared_secret: non_empty("MATRIX_SHARED_SECRET"),
        })
    }
}

#[async_trait]
impl CommsProvider for MatrixAdapter {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr {
        // The alias is deterministic and self-describing; actual room creation
        // is the homeserver's job (auto-created on first join, or provisioned
        // out of band by the operator).
        //
        // TODO(matrix): call POST /_matrix/client/v3/createRoom with an
        // application-service token so the room exists with the right power
        // levels and is filed under the Magnetite space, instead of relying on
        // the homeserver's auto-create-on-alias behaviour.
        RoomAddr(format!(
            "matrix://#{}-{}-{}:{}",
            self.alias_prefix,
            scope_tag(&scope),
            fresh_id(),
            self.server_name
        ))
    }

    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred {
        JoinCred {
            room: room.clone(),
            user: *user,
            token: bridge::mint_join_token(user, "matrix", &room.0).await,
        }
    }

    async fn teardown(&self, _room: &RoomAddr) -> SeamResult<()> {
        // Matrix rooms outlive the match by design (scrollback is the point).
        // TODO(matrix): optionally send m.room.tombstone for `Match` rooms.
        Ok(())
    }
}

impl Adapter for MatrixAdapter {
    fn name(&self) -> &'static str {
        "matrix"
    }
    fn default_media_host(&self) -> Option<String> {
        Some(self.homeserver.clone())
    }
    fn render(&self, cred: &JoinCred, media_host: Option<&str>) -> ClientCred {
        let hs = media_host.unwrap_or(&self.homeserver).to_string();
        let mut out = ClientCred::seam_only("matrix", &cred.room.0, Some(hs), cred.token.clone());
        out.token =
            bridge::render_matrix_login_token(&cred.token, &self.homeserver, self.shared_secret.as_deref());
        out
    }
}

// ─── Jitsi (voice + video SFU) ───────────────────────────────────────────────

/// Jitsi Meet adapter. A room is a meeting name on the operator's own Jitsi.
pub struct JitsiAdapter {
    domain: String,
    app_id: String,
    jwt_secret: Option<String>,
}

impl JitsiAdapter {
    /// Config-gated: `None` unless `JITSI_DOMAIN` is set.
    pub fn from_env() -> Option<Self> {
        Some(Self {
            domain: non_empty("JITSI_DOMAIN")?,
            app_id: non_empty("JITSI_APP_ID").unwrap_or_else(|| "magnetite".into()),
            jwt_secret: non_empty("JITSI_JWT_SECRET"),
        })
    }
}

#[async_trait]
impl CommsProvider for JitsiAdapter {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr {
        // Jitsi rooms spring into existence on first join — naming IS creation.
        RoomAddr(format!(
            "jitsi://magnetite-{}-{}",
            scope_tag(&scope),
            fresh_id()
        ))
    }
    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred {
        JoinCred {
            room: room.clone(),
            user: *user,
            token: bridge::mint_join_token(user, "jitsi", &room.0).await,
        }
    }
    async fn teardown(&self, _room: &RoomAddr) -> SeamResult<()> {
        // Jitsi reaps empty rooms itself; nothing to do.
        Ok(())
    }
}

impl Adapter for JitsiAdapter {
    fn name(&self) -> &'static str {
        "jitsi"
    }
    fn default_media_host(&self) -> Option<String> {
        Some(format!("https://{}", self.domain))
    }
    fn render(&self, cred: &JoinCred, media_host: Option<&str>) -> ClientCred {
        let name = room_name(&cred.room);
        let host = media_host
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://{}", self.domain));
        let mut out = ClientCred::seam_only(
            "jitsi",
            &cred.room.0,
            Some(format!("{}/{}", host.trim_end_matches('/'), name)),
            cred.token.clone(),
        );
        out.token = bridge::render_jitsi_jwt(
            &cred.token,
            name,
            &self.domain,
            &self.app_id,
            self.jwt_secret.as_deref(),
            false,
        );
        out
    }
}

// ─── LiveKit (voice + video at scale) ────────────────────────────────────────

/// LiveKit adapter — the scale path for voice/video.
pub struct LiveKitAdapter {
    url: String,
    api_key: Option<String>,
    api_secret: Option<String>,
}

impl LiveKitAdapter {
    /// Config-gated: `None` unless `LIVEKIT_URL` is set.
    pub fn from_env() -> Option<Self> {
        Some(Self {
            url: non_empty("LIVEKIT_URL")?,
            api_key: non_empty("LIVEKIT_API_KEY"),
            api_secret: non_empty("LIVEKIT_API_SECRET"),
        })
    }
}

#[async_trait]
impl CommsProvider for LiveKitAdapter {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr {
        // LiveKit auto-creates a room when the first participant joins with a
        // valid grant, so the address alone is sufficient.
        //
        // TODO(livekit): use the RoomService API to pre-create with explicit
        // empty-timeout / max-participants when the operator wants limits.
        RoomAddr(format!(
            "livekit://mag-{}-{}",
            scope_tag(&scope),
            fresh_id()
        ))
    }
    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred {
        JoinCred {
            room: room.clone(),
            user: *user,
            token: bridge::mint_join_token(user, "livekit", &room.0).await,
        }
    }
    async fn teardown(&self, _room: &RoomAddr) -> SeamResult<()> {
        // TODO(livekit): RoomService::delete_room once an API client is wired.
        Ok(())
    }
}

impl Adapter for LiveKitAdapter {
    fn name(&self) -> &'static str {
        "livekit"
    }
    fn default_media_host(&self) -> Option<String> {
        Some(self.url.clone())
    }
    fn render(&self, cred: &JoinCred, media_host: Option<&str>) -> ClientCred {
        let name = room_name(&cred.room);
        let mut out = ClientCred::seam_only(
            "livekit",
            &cred.room.0,
            Some(media_host.unwrap_or(&self.url).to_string()),
            cred.token.clone(),
        );
        out.token = bridge::render_livekit_token(
            &cred.token,
            name,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            true,
        );
        out
    }
}

// ─── Owncast (live streaming / VOD) ──────────────────────────────────────────

/// Owncast adapter — one-to-many live streaming, per-node by construction
/// (each operator runs their own Owncast instance).
pub struct OwncastAdapter {
    url: String,
    stream_key: Option<String>,
}

impl OwncastAdapter {
    /// Config-gated: `None` unless `OWNCAST_URL` is set.
    pub fn from_env() -> Option<Self> {
        Some(Self {
            url: non_empty("OWNCAST_URL")?,
            stream_key: non_empty("OWNCAST_STREAM_KEY"),
        })
    }
}

#[async_trait]
impl CommsProvider for OwncastAdapter {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr {
        RoomAddr(format!(
            "owncast://{}-{}",
            scope_tag(&scope),
            fresh_id()
        ))
    }
    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred {
        JoinCred {
            room: room.clone(),
            user: *user,
            token: bridge::mint_join_token(user, "owncast", &room.0).await,
        }
    }
    async fn teardown(&self, _room: &RoomAddr) -> SeamResult<()> {
        Ok(())
    }
}

impl Adapter for OwncastAdapter {
    fn name(&self) -> &'static str {
        "owncast"
    }
    fn default_media_host(&self) -> Option<String> {
        Some(self.url.clone())
    }
    fn render(&self, cred: &JoinCred, media_host: Option<&str>) -> ClientCred {
        let host = media_host.unwrap_or(&self.url).trim_end_matches('/').to_string();
        let mut out = ClientCred::seam_only(
            "owncast",
            &cred.room.0,
            // The HLS manifest a viewer opens; per-node by construction.
            Some(format!("{host}/hls/stream.m3u8")),
            cred.token.clone(),
        );
        // Owncast chat access tokens are minted by its admin API. Until that is
        // wired we hand back the broadcaster stream key only where configured.
        //
        // TODO(owncast): POST /api/admin/accesstokens/create for per-user chat
        // tokens instead of exposing the shared stream key.
        out.token = self.stream_key.clone();
        out
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn non_empty(key: &str) -> Option<String> {
    std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::blobstore::Hash;

    #[tokio::test]
    async fn builtin_is_offline_and_scope_tagged() {
        let a = BuiltinAdapter::new();
        let room = a.create_room(RoomScope::Match(Hash::of(b"g"))).await;
        assert_eq!(a.name(), "builtin");
        assert!(room.0.starts_with("builtin://match/"));
        assert!(a.exists(&room));

        let cred = a.issue_join_credential(&PubKey([1u8; 32]), &room).await;
        assert!(cred.token.is_valid_at(bridge::now_unix()));
        a.teardown(&room).await.unwrap();
        assert!(!a.exists(&room));
    }

    #[tokio::test]
    async fn matrix_alias_is_well_formed() {
        let m = MatrixAdapter {
            homeserver: "https://matrix.example.org".into(),
            server_name: "example.org".into(),
            alias_prefix: "magnetite".into(),
            shared_secret: None,
        };
        let room = m.create_room(RoomScope::Lobby).await;
        assert!(room.0.starts_with("matrix://#magnetite-lobby-"));
        assert!(room.0.ends_with(":example.org"));
        // No shared secret configured → no native token, but the seam token stands.
        let cred = m.issue_join_credential(&PubKey([2u8; 32]), &room).await;
        let rendered = m.render(&cred, None);
        assert_eq!(rendered.kind, "matrix");
        assert!(rendered.token.is_none());
        assert!(rendered.seam_token.is_valid_at(bridge::now_unix()));
    }

    #[tokio::test]
    async fn jitsi_render_uses_per_room_media_host_when_present() {
        let j = JitsiAdapter {
            domain: "meet.default".into(),
            app_id: "magnetite".into(),
            jwt_secret: Some("shhh".into()),
        };
        let room = j.create_room(RoomScope::Voice).await;
        let cred = j.issue_join_credential(&PubKey([3u8; 32]), &room).await;

        let default_host = j.render(&cred, None);
        assert!(default_host.url.unwrap().starts_with("https://meet.default/"));

        // A room carrying its own media host wins — per-node media (§3.5).
        let own = j.render(&cred, Some("https://meet.operator.example"));
        assert!(own
            .url
            .unwrap()
            .starts_with("https://meet.operator.example/magnetite-voice-"));
        assert!(own.token.is_some(), "JWT renders when a secret is set");
    }

    #[tokio::test]
    async fn livekit_grant_renders_only_with_key_and_secret() {
        let lk = LiveKitAdapter {
            url: "wss://lk.example".into(),
            api_key: None,
            api_secret: None,
        };
        let room = lk.create_room(RoomScope::Video).await;
        let cred = lk.issue_join_credential(&PubKey([4u8; 32]), &room).await;
        assert!(lk.render(&cred, None).token.is_none());

        let lk2 = LiveKitAdapter {
            url: "wss://lk.example".into(),
            api_key: Some("k".into()),
            api_secret: Some("s".into()),
        };
        assert!(lk2.render(&cred, None).token.is_some());
    }

    #[test]
    fn room_name_strips_the_scheme() {
        assert_eq!(room_name(&RoomAddr("livekit://abc".into())), "abc");
        assert_eq!(room_name(&RoomAddr("plain".into())), "plain");
    }

    #[test]
    fn external_adapters_are_config_gated() {
        // With no env set, every external adapter declines to construct — this
        // is what keeps the default build free of external services.
        temp_env::with_vars(
            [
                ("MATRIX_HOMESERVER", None::<&str>),
                ("JITSI_DOMAIN", None),
                ("LIVEKIT_URL", None),
                ("OWNCAST_URL", None),
            ],
            || {
                assert!(MatrixAdapter::from_env().is_none());
                assert!(JitsiAdapter::from_env().is_none());
                assert!(LiveKitAdapter::from_env().is_none());
                assert!(OwncastAdapter::from_env().is_none());
            },
        );
    }
}
