// HTTP surface for the comms seam (§3.5).
//
// Deliberately tiny: create a room, join a room, close a room, and report which
// provider this node runs. Clients ask the node WHERE to talk and get a
// credential; the talking itself happens against Matrix/Jitsi/LiveKit/Owncast —
// or, on the demoted default, against this node's own `ws/comms` + `ws/voice`.
//
// Nothing here names a provider type. The response carries a `kind` string, a
// URL and an opaque token, so a client written against `builtin` keeps working
// unchanged when an operator switches the node to LiveKit.

use axum::{
    extract::{Extension, Path, State},
    middleware::from_fn_with_state,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{bridge, gate, ClientCred, RoomScope};
use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};

/// What this node runs, so a client can adapt before it asks for a room.
#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    /// `builtin` | `matrix` | `jitsi` | `livekit` | `owncast`.
    pub provider: &'static str,
    /// This node's IdP public key — every join credential is signed by it.
    pub node_pubkey: String,
    /// The provider's default media/service host, if it has one. Individual
    /// rooms may override it; there is no global media server.
    pub media_host: Option<String>,
    /// True when the active provider needs no external service at all.
    pub offline_capable: bool,
}

/// GET /api/v1/comms/provider — public capability probe.
pub async fn provider_info() -> Json<response::ApiResponse<ProviderInfo>> {
    let p = super::provider();
    response::success_response(ProviderInfo {
        provider: p.name(),
        node_pubkey: bridge::node_pubkey().to_hex(),
        media_host: p.default_media_host(),
        offline_capable: p.name() == "builtin",
    })
}

#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    /// `match` | `lobby` | `community` | `voice` | `video` | `stream`.
    pub scope: String,
    /// Game content hash for `match`, community id for `community`.
    pub scope_ref: Option<String>,
    pub community_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    /// Pin this room to a specific media server (per-node media). Omit to use
    /// the provider's configured default.
    pub media_host: Option<String>,
    /// >0 makes this a paid room: joining requires a verified receipt (§3.6).
    pub price_units: Option<i64>,
}

fn parse_scope(req: &CreateRoomRequest) -> Result<RoomScope> {
    Ok(match req.scope.as_str() {
        "lobby" => RoomScope::Lobby,
        "voice" => RoomScope::Voice,
        "video" => RoomScope::Video,
        "stream" => RoomScope::Stream,
        "community" => RoomScope::Community(
            req.scope_ref
                .clone()
                .or_else(|| req.community_id.map(|c| c.to_string()))
                .ok_or_else(|| {
                    AppError::Validation("community scope needs scope_ref".to_string())
                })?,
        ),
        "match" => {
            let raw = req.scope_ref.as_deref().ok_or_else(|| {
                AppError::Validation("match scope needs scope_ref (game hash)".to_string())
            })?;
            RoomScope::Match(
                magnetite_seams::blobstore::Hash::from_hex(raw)
                    .map_err(|e| AppError::Validation(format!("invalid game hash: {e}")))?,
            )
        }
        other => {
            return Err(AppError::Validation(format!("unknown room scope `{other}`")));
        }
    })
}

/// POST /api/v1/comms/rooms — create a room through the active provider.
pub async fn create_room(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(body): Json<CreateRoomRequest>,
) -> Result<Json<response::ApiResponse<super::RoomRecord>>> {
    let scope = parse_scope(&body)?;
    let room = super::create_room(
        &pool,
        scope,
        Some(user_id),
        body.community_id,
        body.channel_id,
        body.media_host.clone(),
        body.price_units.unwrap_or(0),
    )
    .await?;
    Ok(response::success_response(room))
}

/// GET /api/v1/comms/rooms/:id — room metadata (address, provider, media host).
pub async fn get_room(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<super::RoomRecord>>> {
    Ok(response::success_response(super::get_room(&pool, id).await?))
}

/// POST /api/v1/comms/rooms/:id/join — mint a join credential.
///
/// Paid rooms return a validation error until the caller holds a verified,
/// non-voided receipt for the room (§3.6). The credential is short-lived and
/// bound to both the room and the target system.
pub async fn join_room(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<ClientCred>>> {
    let key = super::user_key(&pool, user_id).await;
    let cred = super::join(&pool, id, user_id, key).await?;
    Ok(response::success_response(cred))
}

/// POST /api/v1/comms/rooms/:id/close — tear a room down (creator only).
pub async fn close_room(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    super::close_room(&pool, id, user_id).await?;
    Ok(response::success_response(
        serde_json::json!({ "closed": true, "room_id": id }),
    ))
}

/// GET /api/v1/comms/rooms/:id/paid — whether the caller has cleared the gate.
///
/// Lets a client show "pay to join" before it burns a join attempt.
pub async fn paid_status(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let room = super::get_room(&pool, id).await?;
    let paid = gate::has_paid(&pool, user_id, room.id, room.price_units).await;
    Ok(response::success_response(serde_json::json!({
        "room_id": room.id,
        "price_units": room.price_units,
        "paid": paid,
    })))
}

pub fn router(pool: PgPool) -> Router {
    let auth_routes = Router::new()
        .route("/rooms", post(create_room))
        .route("/rooms/:id/join", post(join_room))
        .route("/rooms/:id/close", post(close_room))
        .route("/rooms/:id/paid", get(paid_status))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ));

    let public_routes = Router::new()
        .route("/provider", get(provider_info))
        .route("/rooms/:id", get(get_room));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .with_state(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(scope: &str, scope_ref: Option<&str>) -> CreateRoomRequest {
        CreateRoomRequest {
            scope: scope.to_string(),
            scope_ref: scope_ref.map(str::to_string),
            community_id: None,
            channel_id: None,
            media_host: None,
            price_units: None,
        }
    }

    #[test]
    fn scopes_parse_and_reject_cleanly() {
        assert!(matches!(
            parse_scope(&req("lobby", None)).unwrap(),
            RoomScope::Lobby
        ));
        assert!(matches!(
            parse_scope(&req("voice", None)).unwrap(),
            RoomScope::Voice
        ));
        // match needs a real game hash.
        assert!(parse_scope(&req("match", None)).is_err());
        assert!(parse_scope(&req("match", Some("not-hex"))).is_err());
        let h = magnetite_seams::blobstore::Hash::of(b"game").to_hex();
        assert!(matches!(
            parse_scope(&req("match", Some(&h))).unwrap(),
            RoomScope::Match(_)
        ));
        // community needs a reference.
        assert!(parse_scope(&req("community", None)).is_err());
        assert!(parse_scope(&req("community", Some("c1"))).is_ok());
        // unknown scopes never silently become lobbies.
        assert!(parse_scope(&req("wormhole", None)).is_err());
    }
}
