// Identity bridge — the node acts as an IdP for comms (§3.5).
//
// One keypair login → SSO into every comms system. The flow is always:
//
//   player keypair  ──(AuthProvider::mint_scoped_token)──▶  seam `Token`
//        (audience = "matrix" | "jitsi" | "livekit" | "owncast" | "builtin",
//         scope    = ["room:join:<addr>", "ts:<unix>"])
//                              │
//                              ▼
//        rendered into each system's native credential format:
//          Matrix   → login token for the `m.login.token` / OpenID SSO flow
//          Jitsi    → HS256 JWT with the `context.user` + `room` claims
//          LiveKit  → HS256 JWT with the `video` grant claims
//          Owncast  → bearer token for the access-token API
//
// The seam token is ALWAYS the source of truth: it is Ed25519-signed by the
// node, audience-bound and short-lived. The rendered credential is a mechanical
// projection of it, never an independent grant. A provider that cannot render
// (missing shared secret) yields `None`, and callers degrade to the builtin
// provider rather than failing the request.

use std::sync::OnceLock;

use magnetite_seams::identity::{
    Audience, AuthProvider, Challenge, LoginResponse, PubKey, RawKeypairAuth, Scope, Session,
    Token,
};
use serde::{Deserialize, Serialize};

/// Unix seconds. Local so the seam's private clock helper stays private.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The node's own keypair, doubling as the comms IdP signing authority.
///
/// Seeded from `NODE_SIGNING_SEED` (64 hex chars) when present so a restart
/// keeps the same node identity; otherwise a fresh key is generated — which is
/// fine for a single-process dev run and requires zero configuration.
pub fn node_auth() -> &'static RawKeypairAuth {
    static AUTH: OnceLock<RawKeypairAuth> = OnceLock::new();
    AUTH.get_or_init(|| {
        match std::env::var("NODE_SIGNING_SEED")
            .ok()
            .and_then(|h| hex::decode(h.trim()).ok())
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
        {
            Some(seed) => RawKeypairAuth::from_seed(seed),
            None => {
                tracing::warn!(
                    "NODE_SIGNING_SEED unset — generating an ephemeral node comms identity. \
                     Set it (64 hex chars) so join credentials survive a restart."
                );
                RawKeypairAuth::generate()
            }
        }
    })
}

/// The node's public key — the issuer every minted join credential carries.
pub fn node_pubkey() -> PubKey {
    node_auth().node_pubkey()
}

/// Zero-sized handle to the process-wide node identity.
///
/// Exists so seam types generic over `A: AuthProvider` (e.g. the seam's
/// `BuiltinProvider`) can be wired to the one shared node key without cloning a
/// signing key or threading a lifetime through every adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct NodeAuth;

#[async_trait::async_trait]
impl AuthProvider for NodeAuth {
    async fn challenge(&self, pk: &PubKey) -> Challenge {
        node_auth().challenge(pk).await
    }
    async fn verify_login(&self, resp: LoginResponse) -> magnetite_seams::Result<Session> {
        node_auth().verify_login(resp).await
    }
    async fn mint_scoped_token(&self, pk: &PubKey, aud: Audience, scope: Scope) -> Token {
        node_auth().mint_scoped_token(pk, aud, scope).await
    }
}

/// Mint the canonical, audience-bound join token for `user` into `room`.
///
/// This is the single minting path: every provider funnels through it so the
/// identity story is identical no matter which comms system is wired.
pub async fn mint_join_token(user: &PubKey, audience: &str, room_addr: &str) -> Token {
    node_auth()
        .mint_scoped_token(
            user,
            Audience(audience.to_string()),
            Scope(vec![
                format!("room:join:{room_addr}"),
                format!("ts:{}", now_unix()),
            ]),
        )
        .await
}

/// A credential rendered into a shape a comms client can actually use.
///
/// Deliberately provider-agnostic on the wire: callers outside `comms/` see a
/// `kind` string, a URL and an opaque token — never a provider-specific type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientCred {
    /// Which adapter minted it: `builtin` | `matrix` | `jitsi` | `livekit` | `owncast`.
    pub kind: String,
    /// The room address (opaque; format belongs to the provider).
    pub room: String,
    /// Where the client connects. Per-node: comes from the room's own media
    /// host when it has one, not from a global media server.
    pub url: Option<String>,
    /// The native credential (JWT / login token / bearer), when renderable.
    pub token: Option<String>,
    /// The node-signed seam token, hex-JSON, always present. Any peer can verify
    /// it against `issuer` without trusting this node's TLS.
    pub seam_token: Token,
    /// Unix seconds after which both credentials are dead.
    pub expires_at: u64,
}

impl ClientCred {
    /// Build from a seam token with no native rendering (builtin / unconfigured).
    pub fn seam_only(kind: &str, room: &str, url: Option<String>, token: Token) -> Self {
        Self {
            kind: kind.to_string(),
            room: room.to_string(),
            url,
            expires_at: token.claims.expires_at,
            token: None,
            seam_token: token,
        }
    }
}

// ─── Native credential rendering ──────────────────────────────────────────────

/// Claims for a Jitsi JWT (`jitsi-meet-tokens` / `prosody` plugin shape).
#[derive(Serialize)]
struct JitsiClaims<'a> {
    aud: &'a str,
    iss: &'a str,
    sub: &'a str,
    room: &'a str,
    exp: u64,
    nbf: u64,
    context: JitsiContext,
}

#[derive(Serialize)]
struct JitsiContext {
    user: JitsiUser,
}

#[derive(Serialize)]
struct JitsiUser {
    id: String,
    name: String,
    moderator: String,
}

/// Render a Jitsi JWT from an already-minted seam token.
///
/// Returns `None` when `JITSI_JWT_SECRET` is unset — an unauthenticated Jitsi
/// deployment is a perfectly valid configuration, and the caller simply hands
/// the client a room URL with no token.
pub fn render_jitsi_jwt(
    tok: &Token,
    room: &str,
    domain: &str,
    app_id: &str,
    secret: Option<&str>,
    moderator: bool,
) -> Option<String> {
    let secret = secret?;
    let sub = tok.claims.subject.to_hex();
    let claims = JitsiClaims {
        aud: app_id,
        iss: app_id,
        sub: domain,
        room,
        exp: tok.claims.expires_at,
        nbf: tok.claims.issued_at.saturating_sub(5),
        context: JitsiContext {
            user: JitsiUser {
                // The player's keypair IS the account — no separate user store.
                id: sub.clone(),
                name: short_name(&sub),
                moderator: moderator.to_string(),
            },
        },
    };
    encode_hs256(&claims, secret)
}

/// Claims for a LiveKit access token (`video` grant).
#[derive(Serialize)]
struct LiveKitClaims<'a> {
    iss: &'a str,
    sub: &'a str,
    name: String,
    exp: u64,
    nbf: u64,
    video: LiveKitGrant<'a>,
}

#[derive(Serialize)]
struct LiveKitGrant<'a> {
    room: &'a str,
    #[serde(rename = "roomJoin")]
    room_join: bool,
    #[serde(rename = "canPublish")]
    can_publish: bool,
    #[serde(rename = "canSubscribe")]
    can_subscribe: bool,
    #[serde(rename = "canPublishData")]
    can_publish_data: bool,
}

/// Render a LiveKit access token. `None` without an API key/secret pair.
pub fn render_livekit_token(
    tok: &Token,
    room: &str,
    api_key: Option<&str>,
    api_secret: Option<&str>,
    can_publish: bool,
) -> Option<String> {
    let (key, secret) = (api_key?, api_secret?);
    let sub = tok.claims.subject.to_hex();
    let claims = LiveKitClaims {
        iss: key,
        sub: &sub,
        name: short_name(&sub),
        exp: tok.claims.expires_at,
        nbf: tok.claims.issued_at.saturating_sub(5),
        video: LiveKitGrant {
            room,
            room_join: true,
            can_publish,
            can_subscribe: true,
            can_publish_data: true,
        },
    };
    encode_hs256(&claims, secret)
}

/// Render a Matrix login token for the `m.login.token` flow.
///
/// A homeserver accepts this only when it is configured to trust this node as
/// an SSO/JWT identity provider (e.g. Synapse's `jwt_config` with the same
/// shared secret). Without `MATRIX_SHARED_SECRET` we return `None` and the
/// client is expected to log in to the homeserver itself.
pub fn render_matrix_login_token(
    tok: &Token,
    homeserver: &str,
    secret: Option<&str>,
) -> Option<String> {
    let secret = secret?;
    #[derive(Serialize)]
    struct MatrixClaims<'a> {
        iss: &'a str,
        aud: &'a str,
        sub: String,
        exp: u64,
        iat: u64,
    }
    let claims = MatrixClaims {
        iss: "magnetite",
        aud: homeserver,
        // Matrix localpart derived from the keypair — stable, no user table.
        sub: matrix_localpart(&tok.claims.subject),
        exp: tok.claims.expires_at,
        iat: tok.claims.issued_at,
    };
    encode_hs256(&claims, secret)
}

/// Deterministic Matrix localpart for a keypair: `mag_<first 32 hex chars>`.
///
/// Matrix localparts are case-insensitive and length-limited, so we take a
/// prefix of the hex pubkey; 128 bits of it is collision-safe for this purpose.
pub fn matrix_localpart(pk: &PubKey) -> String {
    format!("mag_{}", &pk.to_hex()[..32])
}

/// A short display name derived from a hex key (`a1b2c3d4…e5f6`).
fn short_name(hex_key: &str) -> String {
    if hex_key.len() <= 12 {
        return hex_key.to_string();
    }
    format!("{}…{}", &hex_key[..8], &hex_key[hex_key.len() - 4..])
}

fn encode_hs256<T: Serialize>(claims: &T, secret: &str) -> Option<String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| tracing::warn!("comms: failed to render JWT credential: {e}"))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok_for(aud: &str) -> Token {
        tokio_test::block_on(mint_join_token(&PubKey([7u8; 32]), aud, "room://x"))
    }

    #[test]
    fn seam_token_is_node_signed_and_audience_bound() {
        let t = tok_for("jitsi");
        assert!(t.is_valid_at(now_unix()));
        assert_eq!(t.claims.issuer, node_pubkey());
        assert_eq!(t.claims.audience, Audience("jitsi".into()));
        assert!(t.claims.scope.0.iter().any(|s| s.starts_with("room:join:")));
    }

    #[test]
    fn jitsi_jwt_renders_only_with_a_secret() {
        let t = tok_for("jitsi");
        assert!(render_jitsi_jwt(&t, "r", "meet.example", "magnetite", None, false).is_none());
        let jwt = render_jitsi_jwt(&t, "r", "meet.example", "magnetite", Some("s3cret"), true)
            .expect("renders with a secret");
        assert_eq!(jwt.split('.').count(), 3, "compact JWS");
    }

    #[test]
    fn livekit_token_needs_both_key_and_secret() {
        let t = tok_for("livekit");
        assert!(render_livekit_token(&t, "r", Some("key"), None, true).is_none());
        assert!(render_livekit_token(&t, "r", None, Some("sec"), true).is_none());
        assert!(render_livekit_token(&t, "r", Some("key"), Some("sec"), true).is_some());
    }

    #[test]
    fn matrix_localpart_is_deterministic_and_key_derived() {
        let pk = PubKey([9u8; 32]);
        assert_eq!(matrix_localpart(&pk), matrix_localpart(&pk));
        assert!(matrix_localpart(&pk).starts_with("mag_"));
        assert_ne!(matrix_localpart(&pk), matrix_localpart(&PubKey([8u8; 32])));
    }

    #[test]
    fn client_cred_expiry_tracks_the_seam_token() {
        let t = tok_for("builtin");
        let c = ClientCred::seam_only("builtin", "room://x", None, t.clone());
        assert_eq!(c.expires_at, t.claims.expires_at);
        assert!(c.token.is_none());
    }
}
