// Messages API — list/post channel messages and DMs with cursor pagination.
#![allow(dead_code)]

use axum::{
    extract::{Extension, Path, Query, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::communities as svc;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PostMessageRequest {
    pub content: String,
    pub reply_to_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ListMessagesQuery {
    /// Cursor: return messages older than this message ID.
    pub before: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<svc::Message>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct PostDmRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct DmMessagesResponse {
    pub messages: Vec<svc::DmMessage>,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct DmThreadsResponse {
    pub threads: Vec<svc::DmThread>,
}

// ---------------------------------------------------------------------------
// Channel message handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/channels/:channel_id/messages?before=&limit=
pub async fn list_messages(
    State(pool): State<PgPool>,
    Path(channel_id): Path<Uuid>,
    Query(q): Query<ListMessagesQuery>,
) -> Result<Json<response::ApiResponse<MessagesResponse>>> {
    let limit = q.limit.unwrap_or(50).min(100);
    let fetch_n = limit + 1; // fetch one extra to detect has_more
    let mut messages = svc::list_messages(&pool, channel_id, q.before, fetch_n).await?;
    let has_more = messages.len() as i64 > limit;
    if has_more {
        messages.truncate(limit as usize);
    }
    // Return in chronological order (oldest first) for the chat UI.
    messages.reverse();
    Ok(response::success_response(MessagesResponse {
        messages,
        has_more,
    }))
}

/// POST /api/v1/channels/:channel_id/messages
pub async fn post_message(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(channel_id): Path<Uuid>,
    Json(payload): Json<PostMessageRequest>,
) -> Result<Json<response::ApiResponse<svc::Message>>> {
    if payload.content.trim().is_empty() {
        return Err(AppError::Validation(
            "Message content cannot be empty".to_string(),
        ));
    }
    let msg = svc::post_message(
        &pool,
        channel_id,
        user_id,
        &payload.content,
        payload.reply_to_id,
    )
    .await?;
    Ok(response::success_response(msg))
}

/// DELETE /api/v1/channels/:channel_id/messages/:message_id
pub async fn delete_message(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path((_channel_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<response::ApiResponse<()>>> {
    svc::delete_message(&pool, message_id, user_id).await?;
    Ok(response::success_response(()))
}

// ---------------------------------------------------------------------------
// DM handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/dms  — list all DM threads for the current user
pub async fn list_dm_threads(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<DmThreadsResponse>>> {
    let threads = svc::list_dm_threads(&pool, user_id).await?;
    Ok(response::success_response(DmThreadsResponse { threads }))
}

/// POST /api/v1/dms/:other_user_id  — send a DM (creates thread if needed)
pub async fn send_dm(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(other_user_id): Path<Uuid>,
    Json(payload): Json<PostDmRequest>,
) -> Result<Json<response::ApiResponse<svc::DmMessage>>> {
    if user_id == other_user_id {
        return Err(AppError::BadRequest("Cannot DM yourself".to_string()));
    }
    let thread = svc::get_or_create_dm_thread(&pool, user_id, other_user_id).await?;
    let msg = svc::post_dm_message(&pool, thread.id, user_id, &payload.content).await?;
    Ok(response::success_response(msg))
}

/// GET /api/v1/dms/:other_user_id/messages?before=&limit=
pub async fn list_dm_messages(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(other_user_id): Path<Uuid>,
    Query(q): Query<ListMessagesQuery>,
) -> Result<Json<response::ApiResponse<DmMessagesResponse>>> {
    let thread = svc::get_or_create_dm_thread(&pool, user_id, other_user_id).await?;
    let limit = q.limit.unwrap_or(50).min(100);
    let fetch_n = limit + 1;
    let mut messages = svc::list_dm_messages(&pool, thread.id, q.before, fetch_n).await?;
    let has_more = messages.len() as i64 > limit;
    if has_more {
        messages.truncate(limit as usize);
    }
    messages.reverse();
    Ok(response::success_response(DmMessagesResponse {
        messages,
        has_more,
    }))
}

// ---------------------------------------------------------------------------
// Routers
// ---------------------------------------------------------------------------

/// Channel messages router — nested under /channels/:channel_id/messages
pub fn channel_messages_router(pool: PgPool) -> Router {
    let auth_routes = Router::new()
        .route("/", post(post_message))
        .route("/:message_id", delete(delete_message))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ));

    let public_routes = Router::new().route("/", get(list_messages));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .with_state(pool)
}

/// DMs router — mounted at /dms
pub fn dms_router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_dm_threads))
        .route("/:other_user_id", post(send_dm))
        .route("/:other_user_id/messages", get(list_dm_messages))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ))
        .with_state(pool)
}
