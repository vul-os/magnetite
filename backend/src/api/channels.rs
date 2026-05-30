// Channels API — CRUD for text and voice channels within a community.
#![allow(dead_code)]

use axum::{
    extract::{Extension, Path, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::communities as svc;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    /// "text" | "voice"
    pub kind: Option<String>,
    pub topic: Option<String>,
    pub is_private: Option<bool>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/communities/:community_id/channels
pub async fn list_channels(
    State(pool): State<PgPool>,
    Path(community_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<Vec<svc::Channel>>>> {
    let channels = svc::list_channels(&pool, community_id).await?;
    Ok(response::success_response(channels))
}

/// GET /api/v1/communities/:community_id/channels/:channel_id
pub async fn get_channel(
    State(pool): State<PgPool>,
    Path((_community_id, channel_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<response::ApiResponse<svc::Channel>>> {
    let channel = svc::get_channel(&pool, channel_id).await?;
    Ok(response::success_response(channel))
}

/// POST /api/v1/communities/:community_id/channels
pub async fn create_channel(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Path(community_id): Path<Uuid>,
    Json(payload): Json<CreateChannelRequest>,
) -> Result<Json<response::ApiResponse<svc::Channel>>> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("Channel name is required".to_string()));
    }
    let kind = payload.kind.as_deref().unwrap_or("text");
    let channel = svc::create_channel(
        &pool,
        community_id,
        payload.name.trim(),
        kind,
        payload.topic.as_deref(),
        payload.is_private.unwrap_or(false),
    )
    .await?;
    Ok(response::success_response(channel))
}

/// DELETE /api/v1/communities/:community_id/channels/:channel_id
pub async fn delete_channel(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Path((community_id, channel_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<response::ApiResponse<()>>> {
    svc::delete_channel(&pool, channel_id, community_id).await?;
    Ok(response::success_response(()))
}

// ---------------------------------------------------------------------------
// Router — nested under /communities/:community_id/channels
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool) -> Router {
    let auth_routes = Router::new()
        .route("/", post(create_channel))
        .route("/:channel_id", delete(delete_channel))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ));

    let public_routes = Router::new()
        .route("/", get(list_channels))
        .route("/:channel_id", get(get_channel));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .with_state(pool)
}
