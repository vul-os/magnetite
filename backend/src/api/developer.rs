use axum::{
    extract::{Extension, Path, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::error::{AppError, Result};

#[derive(Debug, Deserialize)]
pub struct RegisterDeveloperRequest {
    pub accept_terms: bool,
}

#[derive(Debug, Serialize)]
pub struct DeveloperInfo {
    pub user_id: Uuid,
    pub is_developer: bool,
    pub developer_terms_accepted: bool,
    pub registered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_games: i64,
    pub total_earnings: Decimal,
    pub total_players: i64,
    pub revenue_chart: Vec<RevenueDataPoint>,
}

#[derive(Debug, Serialize)]
pub struct RevenueDataPoint {
    pub date: String,
    pub revenue: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeveloperGame {
    pub id: Uuid,
    pub developer_id: Uuid,
    pub github_repo: String,
    pub title: String,
    pub description: Option<String>,
    pub fee_per_session: Decimal,
    pub status: String,
    pub active: bool,
    pub total_players: i64,
    pub total_revenue: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGameStatusRequest {
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct EarningsSummary {
    pub total_earnings: Decimal,
    pub pending_payout: Decimal,
    pub total_paid: Decimal,
    pub recent_earnings: Vec<EarningsByGame>,
}

#[derive(Debug, Serialize)]
pub struct EarningsByGame {
    pub game_id: Uuid,
    pub game_title: String,
    pub total_earnings: Decimal,
    pub player_count: i64,
}

#[derive(Debug, Serialize)]
pub struct PayoutRequest {
    pub id: Uuid,
    pub amount: Decimal,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePayoutRequest {
    pub amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct PayoutHistory {
    pub payouts: Vec<PayoutRequest>,
    pub total_paid: Decimal,
}

#[derive(Debug, Serialize)]
pub struct GameAnalytics {
    pub game_id: Uuid,
    pub daily_active_players: Vec<DailyPlayerData>,
    pub session_duration_stats: SessionStats,
    pub revenue_breakdown: RevenueBreakdown,
}

#[derive(Debug, Serialize)]
pub struct DailyPlayerData {
    pub date: String,
    pub active_players: i64,
    pub new_players: i64,
}

#[derive(Debug, Serialize)]
pub struct SessionStats {
    pub avg_duration_secs: f64,
    pub total_sessions: i64,
    pub avg_score: f64,
}

#[derive(Debug, Serialize)]
pub struct RevenueBreakdown {
    pub total_revenue: Decimal,
    pub platform_fee: Decimal,
    pub developer_earnings: Decimal,
    pub session_count: i64,
}

pub async fn register_developer(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<RegisterDeveloperRequest>,
) -> Result<Json<DeveloperInfo>> {
    if !payload.accept_terms {
        return Err(AppError::Validation(
            "Developer terms must be accepted".to_string(),
        ));
    }

    let result = sqlx::query_as::<_, (bool, Option<DateTime<Utc>>)>(
        "UPDATE users SET is_developer = true, developer_terms_accepted = true, developer_registered_at = NOW()
         WHERE id = $1
         RETURNING is_developer, developer_registered_at",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(DeveloperInfo {
        user_id,
        is_developer: result.0,
        developer_terms_accepted: payload.accept_terms,
        registered_at: result.1,
    }))
}

pub async fn get_dashboard(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<DashboardStats>> {
    let total_games = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM games WHERE developer_id = $1 AND active = true",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?
    .0;

    let total_earnings: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM game_revenue WHERE developer_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let total_players = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(DISTINCT user_id) FROM game_sessions gs
         JOIN games g ON gs.game_id = g.id
         WHERE g.developer_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?
    .0;

    let revenue_chart = sqlx::query_as::<_, (String, Decimal)>(
        "SELECT DATE(created_at)::text as date, COALESCE(SUM(amount), 0) as revenue
         FROM game_revenue
         WHERE developer_id = $1 AND created_at >= NOW() - INTERVAL '30 days'
         GROUP BY DATE(created_at)
         ORDER BY date",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let chart_data: Vec<RevenueDataPoint> = revenue_chart
        .into_iter()
        .map(|(date, revenue)| RevenueDataPoint { date, revenue })
        .collect();

    Ok(Json(DashboardStats {
        total_games,
        total_earnings,
        total_players,
        revenue_chart: chart_data,
    }))
}

pub async fn list_developer_games(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<Vec<DeveloperGame>>> {
    let games = sqlx::query_as::<_, DeveloperGame>(
        "SELECT g.id, g.developer_id, g.github_repo, g.title, g.description, g.fee_per_session,
                g.status, g.active, g.created_at,
                COALESCE(COUNT(DISTINCT gs.user_id)::i64, 0) as total_players,
                COALESCE(SUM(gr.amount), 0)::decimal as total_revenue
         FROM games g
         LEFT JOIN game_sessions gs ON g.id = gs.game_id
         LEFT JOIN game_revenue gr ON g.id = gr.game_id
         WHERE g.developer_id = $1 AND g.active = true
         GROUP BY g.id",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(games))
}

pub async fn update_game_status(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<UpdateGameStatusRequest>,
) -> Result<Json<DeveloperGame>> {
    let valid_statuses = [
        "draft",
        "pending",
        "approved",
        "active",
        "suspended",
        "archived",
    ];
    if !valid_statuses.contains(&payload.status.as_str()) {
        return Err(AppError::Validation("Invalid status value".to_string()));
    }

    let game = sqlx::query_as::<_, DeveloperGame>(
        "UPDATE games SET status = $1
         WHERE id = $2 AND developer_id = $3 AND active = true
         RETURNING id, developer_id, github_repo, title, description, fee_per_session,
                   status, active, created_at,
                   COALESCE((SELECT COUNT(DISTINCT user_id) FROM game_sessions WHERE game_id = $2), 0)::i64 as total_players,
                   COALESCE((SELECT SUM(amount) FROM game_revenue WHERE game_id = $2), 0)::decimal as total_revenue",
    )
    .bind(&payload.status)
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    Ok(Json(game))
}

pub async fn delete_game(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<()>> {
    let result = sqlx::query(
        "UPDATE games SET active = false WHERE id = $1 AND developer_id = $2 AND active = true",
    )
    .bind(game_id)
    .bind(user_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Game not found".to_string()));
    }

    Ok(Json(()))
}

pub async fn get_earnings_summary(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<EarningsSummary>> {
    let total_earnings: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM game_revenue WHERE developer_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let pending_payout: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM payouts WHERE user_id = $1 AND status = 'pending'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let total_paid: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM payouts WHERE user_id = $1 AND status = 'completed'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let recent_earnings = sqlx::query_as::<_, (Uuid, String, Decimal, i64)>(
        "SELECT g.id, g.title, COALESCE(SUM(gr.amount), 0), COUNT(DISTINCT gs.user_id)
         FROM games g
         LEFT JOIN game_revenue gr ON g.id = gr.game_id
         LEFT JOIN game_sessions gs ON g.id = gs.game_id
         WHERE g.developer_id = $1
         GROUP BY g.id",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let earnings_by_game: Vec<EarningsByGame> = recent_earnings
        .into_iter()
        .map(|(game_id, title, earnings, players)| EarningsByGame {
            game_id,
            game_title: title,
            total_earnings: earnings,
            player_count: players,
        })
        .collect();

    Ok(Json(EarningsSummary {
        total_earnings,
        pending_payout,
        total_paid,
        recent_earnings: earnings_by_game,
    }))
}

pub async fn request_payout(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<CreatePayoutRequest>,
) -> Result<Json<PayoutRequest>> {
    if payload.amount <= Decimal::ZERO {
        return Err(AppError::Validation("Amount must be positive".to_string()));
    }

    let available: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM game_revenue WHERE developer_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let pending: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM payouts WHERE user_id = $1 AND status = 'pending'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let payout_total: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM payouts WHERE user_id = $1 AND status = 'completed'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let balance = available - pending - payout_total;
    if payload.amount > balance {
        return Err(AppError::InsufficientFunds(
            "Insufficient earnings balance".to_string(),
        ));
    }

    let payout_id = Uuid::new_v4();
    let payout =
        sqlx::query_as::<_, (Uuid, Decimal, String, DateTime<Utc>, Option<DateTime<Utc>>)>(
            "INSERT INTO payouts (id, user_id, amount, status, requested_at)
         VALUES ($1, $2, $3, 'pending', NOW())
         RETURNING id, amount, status, requested_at, processed_at",
        )
        .bind(payout_id)
        .bind(user_id)
        .bind(payload.amount)
        .fetch_one(&pool)
        .await?;

    Ok(Json(PayoutRequest {
        id: payout.0,
        amount: payout.1,
        status: payout.2,
        requested_at: payout.3,
        processed_at: payout.4,
    }))
}

pub async fn get_payout_history(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<PayoutHistory>> {
    let payouts =
        sqlx::query_as::<_, (Uuid, Decimal, String, DateTime<Utc>, Option<DateTime<Utc>>)>(
            "SELECT id, amount, status, requested_at, processed_at
         FROM payouts WHERE user_id = $1
         ORDER BY requested_at DESC",
        )
        .bind(user_id)
        .fetch_all(&pool)
        .await?;

    let payout_list: Vec<PayoutRequest> = payouts
        .into_iter()
        .map(
            |(id, amount, status, requested_at, processed_at)| PayoutRequest {
                id,
                amount,
                status,
                requested_at,
                processed_at,
            },
        )
        .collect();

    let total_paid: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM payouts WHERE user_id = $1 AND status = 'completed'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(PayoutHistory {
        payouts: payout_list,
        total_paid,
    }))
}

pub async fn get_game_analytics(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<GameAnalytics>> {
    let game = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND developer_id = $2 AND active = true",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let daily_players = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT DATE(gs.created_at)::text as date,
                COUNT(DISTINCT gs.user_id) as active_players,
                COUNT(DISTINCT CASE WHEN DATE(gs.created_at) = DATE(g.created_at) THEN gs.user_id END) as new_players
         FROM game_sessions gs
         JOIN games g ON gs.game_id = g.id
         WHERE gs.game_id = $1 AND gs.created_at >= NOW() - INTERVAL '30 days'
         GROUP BY DATE(gs.created_at)
         ORDER BY date",
    )
    .bind(game_id)
    .fetch_all(&pool)
    .await?;

    let daily_active: Vec<DailyPlayerData> = daily_players
        .into_iter()
        .map(|(date, active, new)| DailyPlayerData {
            date,
            active_players: active,
            new_players: new,
        })
        .collect();

    let session_stats = sqlx::query_as::<_, (Option<f64>, Option<i64>, Option<f64>)>(
        "SELECT AVG(EXTRACT(EPOCH FROM (ended_at - started_at))) as avg_duration,
                COUNT(*) as total_sessions,
                AVG(score) as avg_score
         FROM game_sessions
         WHERE game_id = $1 AND ended_at IS NOT NULL",
    )
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    let total_revenue: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::numeric FROM game_revenue WHERE game_id = $1",
    )
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    let session_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM game_sessions WHERE game_id = $1")
            .bind(game_id)
            .fetch_one(&pool)
            .await?;

    let platform_fee = total_revenue * Decimal::from_str_exact("0.30").unwrap_or(Decimal::ZERO);
    let developer_earnings = total_revenue - platform_fee;

    Ok(Json(GameAnalytics {
        game_id: game.0,
        daily_active_players: daily_active,
        session_duration_stats: SessionStats {
            avg_duration_secs: session_stats.0.unwrap_or(0.0),
            total_sessions: session_stats.1.unwrap_or(0),
            avg_score: session_stats.2.unwrap_or(0.0),
        },
        revenue_breakdown: RevenueBreakdown {
            total_revenue,
            platform_fee,
            developer_earnings,
            session_count,
        },
    }))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route(
            "/register",
            post(register_developer).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/dashboard",
            get(get_dashboard).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games",
            get(list_developer_games).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id/status",
            put(update_game_status).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id",
            delete(delete_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/earnings",
            get(get_earnings_summary).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/payouts",
            get(get_payout_history).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/payouts",
            post(request_payout).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id/players",
            get(get_game_analytics).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
