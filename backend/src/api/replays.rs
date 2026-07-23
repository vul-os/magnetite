// Replay API — store and serve authoritative match ReplayLogs.
//
// Routes (mounted at /api/v1):
//   POST   /replays                   — store a ReplayLog (auth required)
//   GET    /games/:id/replays         — list replays for a game (public)
//   GET    /replays/:id               — fetch full ReplayLog JSON (public)

use axum::{
    extract::{Extension, Path, Query, State},
    middleware::from_fn_with_state,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response::{self, PaginatedResponse};
use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Thin DB row returned for list endpoints — does NOT include the full JSONB blob.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReplaySummary {
    pub id: Uuid,
    pub game_id: Uuid,
    pub match_id: Option<Uuid>,
    pub recorded_by: Uuid,
    pub state_hash_final: Option<i64>,
    pub duration_ticks: i64,
    pub created_at: DateTime<Utc>,
}

/// Full row including the raw replay JSON; returned by GET /replays/:id.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReplayRow {
    pub id: Uuid,
    pub game_id: Uuid,
    pub match_id: Option<Uuid>,
    pub recorded_by: Uuid,
    pub replay_json: Value,
    pub state_hash_final: Option<i64>,
    pub duration_ticks: i64,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / response shapes
// ---------------------------------------------------------------------------

/// Body for POST /replays.
///
/// `replay_log` must deserialise to the SDK `ReplayLog` shape:
/// ```json
/// {
///   "config":  { … MatchConfig … },
///   "frames":  [ [tick, [[player_id, input], …]], … ],
///   "state_hashes": [ [tick, hash_u64], … ]
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct StoreReplayRequest {
    /// UUID of the game this replay belongs to.
    pub game_id: Uuid,
    /// Optional tournament match UUID.
    pub match_id: Option<Uuid>,
    /// The full ReplayLog as JSON (validated structurally below).
    pub replay_log: Value,
}

#[derive(Debug, Deserialize)]
pub struct ListReplaysQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Light structural validation that `value` looks like a ReplayLog.
/// We don't run full serde deserialization here (that would require the SDK as
/// a dependency of the backend binary); we just assert the three required keys
/// are present and correctly typed.
fn validate_replay_log(value: &Value) -> std::result::Result<(i64, i64), String> {
    let obj = value
        .as_object()
        .ok_or("replay_log must be a JSON object")?;

    // config must be an object
    obj.get("config")
        .and_then(|v| v.as_object())
        .ok_or("replay_log.config must be an object")?;

    // frames must be an array
    let frames = obj
        .get("frames")
        .and_then(|v| v.as_array())
        .ok_or("replay_log.frames must be an array")?;

    // state_hashes must be an array
    let hashes = obj
        .get("state_hashes")
        .and_then(|v| v.as_array())
        .ok_or("replay_log.state_hashes must be an array")?;

    if frames.len() != hashes.len() {
        return Err(format!(
            "frames ({}) and state_hashes ({}) must have the same length",
            frames.len(),
            hashes.len()
        ));
    }

    let duration_ticks = frames.len() as i64;

    // Extract the final state hash — each entry is [tick, hash_as_number].
    let state_hash_final: Option<i64> = hashes.last().and_then(|entry| {
        entry
            .as_array()
            .and_then(|pair| pair.get(1))
            .and_then(|h| h.as_i64())
    });

    Ok((duration_ticks, state_hash_final.unwrap_or(0)))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/replays — store a finished match's ReplayLog.
///
/// Requires Bearer auth. The caller (typically the match server / web client)
/// supplies `game_id`, optional `match_id`, and the full `replay_log` JSON.
/// The handler validates the shape and persists it to the `replays` table.
pub async fn store_replay(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<StoreReplayRequest>,
) -> Result<Json<crate::api::response::ApiResponse<ReplaySummary>>> {
    // 1. Verify the game exists.
    sqlx::query_scalar::<_, bool>("SELECT active FROM games WHERE id = $1")
        .bind(payload.game_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    // 2. Validate replay_log shape and extract metadata.
    let (duration_ticks, state_hash_final) =
        validate_replay_log(&payload.replay_log).map_err(AppError::Validation)?;

    // 3. Insert.
    let replay_id = Uuid::new_v4();
    let row = sqlx::query_as::<_, ReplaySummary>(
        "INSERT INTO replays
             (id, game_id, match_id, recorded_by, replay_json,
              state_hash_final, duration_ticks)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, game_id, match_id, recorded_by,
                   state_hash_final, duration_ticks, created_at",
    )
    .bind(replay_id)
    .bind(payload.game_id)
    .bind(payload.match_id)
    .bind(user_id)
    .bind(&payload.replay_log)
    .bind(state_hash_final)
    .bind(duration_ticks)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(row))
}

/// GET /api/v1/games/:id/replays — paginated list of replays for a game.
///
/// Public — no auth required.
pub async fn list_game_replays(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Query(q): Query<ListReplaysQuery>,
) -> Result<Json<PaginatedResponse<ReplaySummary>>> {
    // Verify game exists (returns 404 for unknown IDs rather than an empty list).
    sqlx::query_scalar::<_, bool>("SELECT active FROM games WHERE id = $1")
        .bind(game_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(100);
    let offset = ((page - 1) * per_page) as i64;

    let rows = sqlx::query_as::<_, ReplaySummary>(
        "SELECT id, game_id, match_id, recorded_by,
                state_hash_final, duration_ticks, created_at
         FROM replays
         WHERE game_id = $1
         ORDER BY created_at DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(game_id)
    .bind(per_page as i64)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM replays WHERE game_id = $1")
        .bind(game_id)
        .fetch_one(&pool)
        .await?;

    Ok(response::paginated(rows, page, per_page, total as u64))
}

/// GET /api/v1/replays/:id — fetch the full ReplayLog JSON for playback.
///
/// Public — no auth required.
pub async fn get_replay(
    State(pool): State<PgPool>,
    Path(replay_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<ReplayRow>>> {
    let row = sqlx::query_as::<_, ReplayRow>(
        "SELECT id, game_id, match_id, recorded_by, replay_json,
                state_hash_final, duration_ticks, created_at
         FROM replays
         WHERE id = $1",
    )
    .bind(replay_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Replay not found".to_string()))?;

    Ok(response::success_response(row))
}

// ---------------------------------------------------------------------------
// Routers
// ---------------------------------------------------------------------------

/// Router mounted at /api/v1/replays — owns POST / and GET /:id.
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route(
            "/",
            axum::routing::post(store_replay).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route("/:id", get(get_replay))
        .with_state(pool)
}

/// Sub-router mounted under /api/v1/games/:id/replays.
/// Axum does NOT allow a path param in the nest() prefix, so we accept the
/// game_id via Path extractor inside the handler. This router is merged into
/// the games namespace in main.rs via:
///
///   .nest("/games/:id/replays", replays::game_replays_router(pool.clone()))
///
/// The `:id` in the nest prefix is captured as part of the path; the handler
/// extracts it with `Path(game_id): Path<Uuid>`.
pub fn game_replays_router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_game_replays))
        .with_state(pool)
}
