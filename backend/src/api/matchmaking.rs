use axum::{
    extract::{Extension, State},
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

#[derive(Debug, Serialize)]
pub struct MatchmakingStatus {
    pub in_queue: bool,
    pub queue_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
    pub status: String,
    pub estimated_wait_seconds: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct JoinMatchmakingRequest {
    pub game_id: Uuid,
}

pub async fn join(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<JoinMatchmakingRequest>,
) -> Result<Json<response::ApiResponse<MatchmakingStatus>>> {
    let existing = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM matchmaking_queue WHERE user_id = $1 AND status = 'waiting'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Already in queue".to_string()));
    }

    let queue_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO matchmaking_queue (id, user_id, game_id, status, created_at)
         VALUES ($1, $2, $3, 'waiting', NOW())",
    )
    .bind(queue_id)
    .bind(user_id)
    .bind(payload.game_id)
    .execute(&pool)
    .await?;

    Ok(response::success_response(MatchmakingStatus {
        in_queue: true,
        queue_id: Some(queue_id),
        game_id: Some(payload.game_id),
        status: "waiting".to_string(),
        estimated_wait_seconds: Some(30),
    }))
}

pub async fn leave(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<MatchmakingStatus>>> {
    sqlx::query(
        "UPDATE matchmaking_queue SET status = 'cancelled' WHERE user_id = $1 AND status = 'waiting'",
    )
    .bind(user_id)
    .execute(&pool)
    .await?;

    Ok(response::success_response(MatchmakingStatus {
        in_queue: false,
        queue_id: None,
        game_id: None,
        status: "cancelled".to_string(),
        estimated_wait_seconds: None,
    }))
}

pub async fn get_status(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<MatchmakingStatus>>> {
    let queue_entry = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        "SELECT id, game_id, status FROM matchmaking_queue WHERE user_id = $1 AND status = 'waiting'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?;

    match queue_entry {
        Some((queue_id, game_id, status)) => Ok(response::success_response(MatchmakingStatus {
            in_queue: true,
            queue_id: Some(queue_id),
            game_id: Some(game_id),
            status,
            estimated_wait_seconds: Some(30),
        })),
        None => Ok(response::success_response(MatchmakingStatus {
            in_queue: false,
            queue_id: None,
            game_id: None,
            status: "not_in_queue".to_string(),
            estimated_wait_seconds: None,
        })),
    }
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route(
            "/join",
            post(join).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/leave",
            delete(leave).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/status",
            get(get_status).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
