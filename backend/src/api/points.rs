// Points economy API — balance, award, spend, history, leaderboard, season ops.
// All mutating endpoints require JWT auth.

use axum::{
    extract::{Extension, Path, Query, State},
    middleware::from_fn_with_state,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::points::{LedgerEntry, PointBalance, PointsLeaderboardEntry, PointsService};

// ─── Request / response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AwardPointsRequest {
    pub user_id: Uuid,
    pub points: i64,
    pub reason: String,
    pub game_id: Option<Uuid>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SpendPointsRequest {
    pub points: i64,
    pub reason: String,
    pub game_id: Option<Uuid>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SeasonResetRequest {
    pub new_season_name: String,
}

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub user_id: Uuid,
    pub balance: i64,
    pub season_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct SeasonResetResponse {
    pub affected_users: u64,
    pub new_season_name: String,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /points/balance — authenticated user's own balance.
pub async fn get_my_balance(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<BalanceResponse>>> {
    let svc = PointsService::new(pool);
    let row: PointBalance = svc.get_balance_row(user_id).await?;
    Ok(response::success_response(BalanceResponse {
        user_id: row.user_id,
        balance: row.balance,
        season_id: row.season_id,
    }))
}

/// GET /points/balance/:user_id — public balance for any user.
pub async fn get_user_balance(
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<BalanceResponse>>> {
    let svc = PointsService::new(pool);
    let row = svc.get_balance_row(user_id).await?;
    Ok(response::success_response(BalanceResponse {
        user_id: row.user_id,
        balance: row.balance,
        season_id: row.season_id,
    }))
}

/// POST /points/award — award points to a user (admin / game-server use).
/// Requires auth; in production this endpoint should be further restricted
/// to service accounts or admin roles.
pub async fn award_points(
    State(pool): State<PgPool>,
    Extension(_caller_id): Extension<Uuid>,
    Json(payload): Json<AwardPointsRequest>,
) -> Result<Json<response::ApiResponse<LedgerEntry>>> {
    let svc = PointsService::new(pool);
    let entry = svc
        .award(
            payload.user_id,
            payload.points,
            &payload.reason,
            payload.game_id,
            payload.metadata,
        )
        .await?;
    Ok(response::success_response(entry))
}

/// POST /points/spend — spend caller's own points.
pub async fn spend_points(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<SpendPointsRequest>,
) -> Result<Json<response::ApiResponse<LedgerEntry>>> {
    let svc = PointsService::new(pool);
    let entry = svc
        .spend(
            user_id,
            payload.points,
            &payload.reason,
            payload.game_id,
            payload.metadata,
        )
        .await?;
    Ok(response::success_response(entry))
}

/// GET /points/history — caller's own ledger history.
pub async fn get_my_history(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<response::PaginatedResponse<LedgerEntry>>> {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0).max(0);

    let svc = PointsService::new(pool);
    let entries = svc.history(user_id, limit, offset).await?;
    let total = svc.history_count(user_id).await?;

    let page = if limit > 0 {
        (offset / limit + 1) as u32
    } else {
        1
    };
    Ok(response::paginated(
        entries,
        page,
        limit as u32,
        total as u64,
    ))
}

/// GET /points/history/:user_id — another user's history (admin use).
pub async fn get_user_history(
    State(pool): State<PgPool>,
    Extension(_caller_id): Extension<Uuid>,
    Path(user_id): Path<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<response::PaginatedResponse<LedgerEntry>>> {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0).max(0);

    let svc = PointsService::new(pool);
    let entries = svc.history(user_id, limit, offset).await?;
    let total = svc.history_count(user_id).await?;

    let page = if limit > 0 {
        (offset / limit + 1) as u32
    } else {
        1
    };
    Ok(response::paginated(
        entries,
        page,
        limit as u32,
        total as u64,
    ))
}

/// GET /points/leaderboard — global points leaderboard.
pub async fn get_leaderboard(
    State(pool): State<PgPool>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<Json<response::PaginatedResponse<PointsLeaderboardEntry>>> {
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0).max(0);

    let svc = PointsService::new(pool);
    let entries = svc.leaderboard(limit, offset).await?;
    let total = entries.len() as u64 + offset as u64; // approximate

    let page = if limit > 0 {
        (offset / limit + 1) as u32
    } else {
        1
    };
    Ok(response::paginated(entries, page, limit as u32, total))
}

/// GET /points/season — active season info.
pub async fn get_active_season(
    State(pool): State<PgPool>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = PointsService::new(pool);
    match svc.active_season().await? {
        Some(s) => Ok(response::success_response(
            serde_json::to_value(s).map_err(|e| AppError::Internal(e.to_string()))?,
        )),
        None => Ok(response::success_response(serde_json::Value::Null)),
    }
}

/// POST /points/season/reset — end current season, zero balances, start new one.
/// Requires auth (admin-only in production).
pub async fn season_reset(
    State(pool): State<PgPool>,
    Extension(_caller_id): Extension<Uuid>,
    Json(payload): Json<SeasonResetRequest>,
) -> Result<Json<response::ApiResponse<SeasonResetResponse>>> {
    if payload.new_season_name.trim().is_empty() {
        return Err(AppError::Validation(
            "new_season_name must not be empty".to_string(),
        ));
    }

    let svc = PointsService::new(pool);
    let affected = svc.season_reset(&payload.new_season_name).await?;

    Ok(response::success_response(SeasonResetResponse {
        affected_users: affected,
        new_season_name: payload.new_season_name,
    }))
}

// ─── Router ───────────────────────────────────────────────────────────────────

pub fn router(pool: PgPool) -> Router {
    Router::new()
        // Public
        .route("/balance/:user_id", get(get_user_balance))
        .route("/leaderboard", get(get_leaderboard))
        .route("/season", get(get_active_season))
        // Authenticated
        .route(
            "/balance",
            get(get_my_balance).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/award",
            post(award_points).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/spend",
            post(spend_points).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/history",
            get(get_my_history).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/history/:user_id",
            get(get_user_history).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/season/reset",
            post(season_reset).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
