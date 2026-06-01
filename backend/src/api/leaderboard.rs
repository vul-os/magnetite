// Leaderboard API — global and per-game rankings; mounted at /api/v1/leaderboard.
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    middleware::from_fn_with_state,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::leaderboard::LeaderboardService;

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub timeframe: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitScoreRequest {
    pub score: i64,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardEntryResponse {
    pub rank: i64,
    pub user_id: Uuid,
    pub username: String,
    pub score: i64,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntryResponse>,
    pub total_count: i64,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct SubmitScoreResponse {
    pub rank: i64,
    pub is_personal_best: bool,
    pub score: i64,
}

#[derive(Debug, Serialize)]
pub struct UserRankResponse {
    pub rank: i64,
    pub score: i64,
    pub username: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TimeframeFilter {
    pub timeframe: String,
}

fn get_timeframe_filter(timeframe: &str) -> (String, String) {
    match timeframe {
        "daily" => (
            "recorded_at >= NOW() - INTERVAL '1 day'".to_string(),
            "daily".to_string(),
        ),
        "weekly" => (
            "recorded_at >= NOW() - INTERVAL '7 days'".to_string(),
            "weekly".to_string(),
        ),
        "monthly" => (
            "recorded_at >= NOW() - INTERVAL '30 days'".to_string(),
            "monthly".to_string(),
        ),
        _ => ("1=1".to_string(), "alltime".to_string()),
    }
}

pub async fn get_leaderboard(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Json<response::PaginatedResponse<LeaderboardEntryResponse>>> {
    let limit = query.limit.unwrap_or(100).min(1000) as i32;
    let offset = query.offset.unwrap_or(0) as i32;
    let timeframe = query.timeframe.as_deref().unwrap_or("alltime");
    let (timeframe_cond, _timeframe_name) = get_timeframe_filter(timeframe);

    let entries = sqlx::query_as::<_, (Uuid, String, i64)>(&format!(
        "SELECT u.id, u.username, ghs.score
             FROM game_high_scores ghs
             JOIN users u ON ghs.user_id = u.id
             WHERE ghs.game_id = $1 AND {}
             ORDER BY ghs.score DESC
             LIMIT $2 OFFSET $3",
        timeframe_cond
    ))
    .bind(game_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_count: i64 = sqlx::query_as::<_, (i64,)>(&format!(
        "SELECT COUNT(*) FROM game_high_scores ghs WHERE ghs.game_id = $1 AND {}",
        timeframe_cond
    ))
    .bind(game_id)
    .fetch_one(&pool)
    .await?
    .0;

    let leaderboard_entries: Vec<LeaderboardEntryResponse> = entries
        .into_iter()
        .enumerate()
        .map(|(i, (user_id, username, score))| LeaderboardEntryResponse {
            rank: (offset + i as i32 + 1) as i64,
            user_id,
            username,
            score,
        })
        .collect();

    let page = ((offset as u32) / (limit as u32)).max(1);
    Ok(response::paginated(
        leaderboard_entries,
        page,
        limit as u32,
        total_count as u64,
    ))
}

pub async fn submit_score(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<SubmitScoreRequest>,
) -> Result<Json<response::ApiResponse<SubmitScoreResponse>>> {
    use crate::api::middleware::extract_token_from_header;
    use crate::api::middleware::validate_token;

    let token = extract_token_from_header(&headers)?;
    let claims = validate_token(&token)?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid user ID".to_string()))?;

    let existing = sqlx::query_as::<_, (i64,)>(
        "SELECT score FROM game_high_scores WHERE game_id = $1 AND user_id = $2",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?;

    let is_personal_best = existing.map(|e| payload.score > e.0).unwrap_or(true);

    // Only update Postgres when it's a new personal best (keep existing high-score semantics).
    if is_personal_best {
        sqlx::query(
            "INSERT INTO game_high_scores (game_id, user_id, score, recorded_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (game_id, user_id)
             DO UPDATE SET score = $3, recorded_at = NOW()",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(payload.score)
        .execute(&pool)
        .await?;
    }

    // Mirror the score into Redis sorted set for fast leaderboard reads.
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost".to_string());
    let lb_svc = LeaderboardService::new(&redis_url);
    let redis_result = lb_svc.submit_score(game_id, user_id, payload.score).await;
    let redis_rank = redis_result.ok().map(|r| r.rank);

    // Fall back to a Postgres-computed rank if Redis is unavailable.
    let rank = if let Some(r) = redis_rank {
        r
    } else {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) + 1 FROM game_high_scores WHERE game_id = $1 AND score > $2",
        )
        .bind(game_id)
        .bind(payload.score)
        .fetch_one(&pool)
        .await?
        .0
    };

    Ok(response::success_response(SubmitScoreResponse {
        rank,
        is_personal_best,
        score: payload.score,
    }))
}

pub async fn get_my_rank(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<response::ApiResponse<UserRankResponse>>> {
    use crate::api::middleware::extract_token_from_header;
    use crate::api::middleware::validate_token;

    let token = extract_token_from_header(&headers)?;
    let claims = validate_token(&token)?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid user ID".to_string()))?;

    let result = sqlx::query_as::<_, (String, i64)>(
        "SELECT u.username, ghs.score
         FROM game_high_scores ghs
         JOIN users u ON ghs.user_id = u.id
         WHERE ghs.game_id = $1 AND ghs.user_id = $2",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("No score found for this game".to_string()))?;

    let rank: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) + 1 FROM game_high_scores WHERE game_id = $1 AND score > $2",
    )
    .bind(game_id)
    .bind(result.1)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(UserRankResponse {
        rank: rank.0,
        score: result.1,
        username: result.0,
    }))
}

pub async fn get_friends_scores(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<response::PaginatedResponse<LeaderboardEntryResponse>>> {
    use crate::api::middleware::extract_token_from_header;
    use crate::api::middleware::validate_token;

    let token = extract_token_from_header(&headers)?;
    let claims = validate_token(&token)?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid user ID".to_string()))?;

    let friends =
        sqlx::query_as::<_, (Uuid,)>("SELECT friend_id FROM friendships WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

    let friend_ids: Vec<Uuid> = friends.into_iter().map(|(id,)| id).collect();

    if friend_ids.is_empty() {
        return Ok(response::paginated(vec![], 1, 100, 0));
    }

    let entries = sqlx::query_as::<_, (Uuid, String, i64)>(
        "SELECT u.id, u.username, ghs.score
         FROM game_high_scores ghs
         JOIN users u ON ghs.user_id = u.id
         WHERE ghs.game_id = $1 AND ghs.user_id = ANY($2)
         ORDER BY ghs.score DESC",
    )
    .bind(game_id)
    .bind(&friend_ids)
    .fetch_all(&pool)
    .await?;

    let leaderboard_entries: Vec<LeaderboardEntryResponse> = entries
        .into_iter()
        .enumerate()
        .map(|(i, (user_id, username, score))| LeaderboardEntryResponse {
            rank: (i + 1) as i64,
            user_id,
            username,
            score,
        })
        .collect();

    let total = leaderboard_entries.len() as u64;
    Ok(response::paginated(leaderboard_entries, 1, 100, total))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/:game_id", get(get_leaderboard))
        .route(
            "/:game_id/scores",
            post(submit_score).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:game_id/me",
            get(get_my_rank).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:game_id/friends",
            get(get_friends_scores).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
