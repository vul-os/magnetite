use axum::{
    extract::{Path, Query, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response::{self, PaginatedResponse};
use crate::error::{AppError, Result};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Game {
    pub id: Uuid,
    pub developer_id: Uuid,
    pub github_repo: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Rich game metadata returned to the frontend play page, including the
/// current live version and artifact availability.
#[derive(Debug, Serialize)]
pub struct GamePlayMetadata {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub github_repo: String,
    pub live_version: Option<String>,
    pub has_playable_artifact: bool,
    pub artifact_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGameRequest {
    pub github_repo: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGameRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub user_id: Uuid,
    pub username: String,
    pub score: i64,
}

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<i32>,
}

pub async fn list_games(State(pool): State<PgPool>) -> Result<Json<PaginatedResponse<Game>>> {
    let games = sqlx::query_as::<_, Game>(
        "SELECT id, developer_id, github_repo, title, description, status, active, created_at
         FROM games WHERE active = true",
    )
    .fetch_all(&pool)
    .await?;
    let total = games.len() as u64;
    Ok(response::paginated(games, 1, 20, total))
}

pub async fn create_game(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateGameRequest>,
) -> Result<Json<crate::api::response::ApiResponse<Game>>> {
    let game_id = Uuid::new_v4();
    let game = sqlx::query_as::<_, Game>(
        "INSERT INTO games (id, developer_id, github_repo, title, description, status, active, created_at)
         VALUES ($1, '00000000-0000-0000-0000-000000000000', $2, $3, $4, 'draft', true, NOW())
         RETURNING id, developer_id, github_repo, title, description, status, active, created_at",
    )
    .bind(game_id)
    .bind(&payload.github_repo)
    .bind(&payload.title)
    .bind(&payload.description)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(game))
}

pub async fn get_game(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<Game>>> {
    let game = sqlx::query_as::<_, Game>(
        "SELECT id, developer_id, github_repo, title, description, status, active, created_at
         FROM games WHERE id = $1 AND active = true",
    )
    .bind(game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;
    Ok(response::success_response(game))
}

pub async fn update_game(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<UpdateGameRequest>,
) -> Result<Json<crate::api::response::ApiResponse<Game>>> {
    let game = sqlx::query_as::<_, Game>(
        "UPDATE games SET
         title = COALESCE($1, title),
         description = COALESCE($2, description),
         status = COALESCE($3, status)
         WHERE id = $4 AND active = true
         RETURNING id, developer_id, github_repo, title, description, status, active, created_at",
    )
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.status)
    .bind(game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    Ok(response::success_response(game))
}

pub async fn delete_game(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<()>>> {
    sqlx::query("UPDATE games SET active = false WHERE id = $1")
        .bind(game_id)
        .execute(&pool)
        .await?;

    Ok(response::success_response(()))
}

pub async fn get_leaderboard(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Json<crate::api::response::PaginatedResponse<LeaderboardEntry>>> {
    let limit = query.limit.unwrap_or(100) as i32;

    let entries = sqlx::query_as::<_, (Uuid, String, i64)>(
        "SELECT u.id, u.username, ghs.score
         FROM game_high_scores ghs
         JOIN users u ON ghs.user_id = u.id
         WHERE ghs.game_id = $1
         ORDER BY ghs.score DESC
         LIMIT $2",
    )
    .bind(game_id)
    .bind(limit)
    .fetch_all(&pool)
    .await?;

    let total: i64 =
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM game_high_scores WHERE game_id = $1")
            .bind(game_id)
            .fetch_one(&pool)
            .await?
            .0;

    let leaderboard: Vec<LeaderboardEntry> = entries
        .into_iter()
        .enumerate()
        .map(|(i, (user_id, username, score))| LeaderboardEntry {
            rank: (i + 1) as i32,
            user_id,
            username,
            score,
        })
        .collect();

    Ok(response::paginated(
        leaderboard,
        1,
        limit as u32,
        total as u64,
    ))
}

pub async fn get_game_play_metadata(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<GamePlayMetadata>>> {
    let game = sqlx::query_as::<_, Game>(
        "SELECT id, developer_id, github_repo, title, description, status, active, created_at
         FROM games WHERE id = $1 AND active = true",
    )
    .bind(game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    // Fetch the live version string if one exists.
    let live_version: Option<String> = sqlx::query_scalar(
        "SELECT version FROM game_versions WHERE game_id = $1 AND is_live = true
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(game_id)
    .fetch_optional(&pool)
    .await?;

    // Check whether a playable (successful) artifact exists for the live version.
    let artifact_info: Option<(String,)> = if live_version.is_some() {
        sqlx::query_as(
            "SELECT ga.artifact_type
             FROM game_artifacts ga
             JOIN game_versions gv ON ga.version_id = gv.id
             WHERE gv.game_id = $1 AND gv.is_live = true AND ga.build_status = 'success'
             ORDER BY ga.created_at DESC LIMIT 1",
        )
        .bind(game_id)
        .fetch_optional(&pool)
        .await?
    } else {
        None
    };

    let has_playable_artifact = artifact_info.is_some();
    let artifact_type = artifact_info.map(|(t,)| t);

    Ok(response::success_response(GamePlayMetadata {
        id: game.id,
        title: game.title,
        description: game.description,
        status: game.status,
        github_repo: game.github_repo,
        live_version,
        has_playable_artifact,
        artifact_type,
    }))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_games))
        .route(
            "/",
            post(create_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route("/:id", get(get_game))
        .route(
            "/:id",
            put(update_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:id",
            delete(delete_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::admin_middleware,
            )),
        )
        .route("/:id/leaderboard", get(get_leaderboard))
        .route("/:id/play-metadata", get(get_game_play_metadata))
        .with_state(pool)
}
