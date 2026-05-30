// Admin API — game approval, user management, platform moderation; platform surface.
#![allow(dead_code)]

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    middleware::from_fn_with_state,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::error::{AppError, Result};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub wallet_address: Option<String>,
    pub is_developer: bool,
    pub is_admin: bool,
    pub banned_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub banned: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminGame {
    pub id: Uuid,
    pub developer_id: Uuid,
    pub developer_username: Option<String>,
    pub github_repo: String,
    pub title: String,
    pub description: Option<String>,
    pub fee_per_session: Decimal,
    pub status: String,
    pub active: bool,
    pub featured_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewGameRequest {
    pub review_notes: String,
}

#[derive(Debug, Deserialize)]
pub struct ApproveGameRequest {
    pub approved: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FeatureGameRequest {
    pub featured: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RevenueDashboard {
    pub total_platform_revenue: Decimal,
    pub total_game_revenue: Decimal,
    pub total_withdrawals: Decimal,
    pub total_payouts: Decimal,
    pub active_users: i64,
    pub total_games: i64,
    pub pending_payouts: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminTransaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: Option<String>,
    pub game_id: Option<Uuid>,
    pub game_title: Option<String>,
    pub tx_type: String,
    pub amount: Decimal,
    pub status: String,
    pub payout_status: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ProcessPayoutRequest {
    pub user_id: Uuid,
    pub amount: Decimal,
    pub destination: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Payout {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: Option<String>,
    pub amount: Decimal,
    pub destination: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub processed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub database: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub total_users: i64,
    pub total_games: i64,
    pub total_transactions: i64,
    pub active_games: i64,
    pub pending_payouts: i64,
}

async fn require_admin(pool: &PgPool, user_id: Uuid) -> Result<()> {
    let is_admin = sqlx::query_scalar::<_, bool>("SELECT is_admin FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if !is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    Ok(())
}

pub async fn list_users(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse<AdminUser>>> {
    require_admin(&pool, user_id).await?;

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await?;

    let users = sqlx::query_as::<_, AdminUser>(
        "SELECT id, username, email, wallet_address, is_developer, is_admin, banned_at, created_at
         FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_pages = (total as f64 / limit as f64).ceil() as i64;

    Ok(Json(PaginatedResponse {
        data: users,
        page,
        limit,
        total,
        total_pages,
    }))
}

pub async fn get_user(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(target_user_id): Path<Uuid>,
) -> Result<Json<AdminUser>> {
    require_admin(&pool, user_id).await?;

    let user = sqlx::query_as::<_, AdminUser>(
        "SELECT id, username, email, wallet_address, is_developer, is_admin, banned_at, created_at
         FROM users WHERE id = $1",
    )
    .bind(target_user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(user))
}

pub async fn update_user_role(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(target_user_id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<Json<AdminUser>> {
    require_admin(&pool, user_id).await?;

    let new_is_admin = payload.role == "admin";

    let user = sqlx::query_as::<_, AdminUser>(
        "UPDATE users SET is_admin = $1, updated_at = NOW()
         WHERE id = $2
         RETURNING id, username, email, wallet_address, is_developer, is_admin, banned_at, created_at",
    )
    .bind(new_is_admin)
    .bind(target_user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(user))
}

pub async fn ban_user(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(target_user_id): Path<Uuid>,
    Json(payload): Json<BanRequest>,
) -> Result<Json<AdminUser>> {
    require_admin(&pool, user_id).await?;

    if target_user_id == user_id {
        return Err(AppError::BadRequest("Cannot ban yourself".to_string()));
    }

    let banned_at = if payload.banned {
        Some(chrono::Utc::now())
    } else {
        None
    };

    let user = sqlx::query_as::<_, AdminUser>(
        "UPDATE users SET banned_at = $1, updated_at = NOW()
         WHERE id = $2
         RETURNING id, username, email, wallet_address, is_developer, is_admin, banned_at, created_at",
    )
    .bind(banned_at)
    .bind(target_user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(user))
}

pub async fn list_games(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse<AdminGame>>> {
    require_admin(&pool, user_id).await?;

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games")
        .fetch_one(&pool)
        .await?;

    let games = sqlx::query_as::<_, AdminGame>(
        "SELECT g.id, g.developer_id, u.username as developer_username, g.github_repo, g.title,
                g.description, g.fee_per_session, g.status, g.active, g.featured_at,
                g.reviewed_at, g.reviewed_by, g.created_at
         FROM games g
         LEFT JOIN users u ON g.developer_id = u.id
         ORDER BY g.created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_pages = (total as f64 / limit as f64).ceil() as i64;

    Ok(Json(PaginatedResponse {
        data: games,
        page,
        limit,
        total,
        total_pages,
    }))
}

pub async fn review_game(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(_payload): Json<ReviewGameRequest>,
) -> Result<Json<AdminGame>> {
    require_admin(&pool, user_id).await?;

    let game = sqlx::query_as::<_, AdminGame>(
        "UPDATE games SET reviewed_at = NOW(), reviewed_by = $1, updated_at = NOW()
         WHERE id = $2
         RETURNING g.id, g.developer_id, u.username as developer_username, g.github_repo, g.title,
                g.description, g.fee_per_session, g.status, g.active, g.featured_at,
                g.reviewed_at, g.reviewed_by, g.created_at
         FROM games g
         LEFT JOIN users u ON g.developer_id = u.id
         WHERE g.id = $2",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    Ok(Json(game))
}

pub async fn approve_game(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<ApproveGameRequest>,
) -> Result<Json<AdminGame>> {
    require_admin(&pool, user_id).await?;

    let new_status = if payload.approved {
        "approved"
    } else {
        "rejected"
    };

    let game = sqlx::query_as::<_, AdminGame>(
        "UPDATE games SET status = $1, reviewed_at = NOW(), reviewed_by = $2, updated_at = NOW()
         WHERE id = $3
         RETURNING g.id, g.developer_id, u.username as developer_username, g.github_repo, g.title,
                g.description, g.fee_per_session, g.status, g.active, g.featured_at,
                g.reviewed_at, g.reviewed_by, g.created_at
         FROM games g
         LEFT JOIN users u ON g.developer_id = u.id
         WHERE g.id = $3",
    )
    .bind(new_status)
    .bind(user_id)
    .bind(game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    Ok(Json(game))
}

pub async fn feature_game(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<FeatureGameRequest>,
) -> Result<Json<AdminGame>> {
    require_admin(&pool, user_id).await?;

    let featured_at = if payload.featured {
        Some(chrono::Utc::now())
    } else {
        None
    };

    let game = sqlx::query_as::<_, AdminGame>(
        "UPDATE games SET featured_at = $1, updated_at = NOW()
         WHERE id = $2
         RETURNING g.id, g.developer_id, u.username as developer_username, g.github_repo, g.title,
                g.description, g.fee_per_session, g.status, g.active, g.featured_at,
                g.reviewed_at, g.reviewed_by, g.created_at
         FROM games g
         LEFT JOIN users u ON g.developer_id = u.id
         WHERE g.id = $2",
    )
    .bind(featured_at)
    .bind(game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    Ok(Json(game))
}

pub async fn revenue_dashboard(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<RevenueDashboard>> {
    require_admin(&pool, user_id).await?;

    let total_platform_revenue = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE type = 'platform_fee'",
    )
    .fetch_one(&pool)
    .await?;

    let total_game_revenue = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE type = 'game_fee'",
    )
    .fetch_one(&pool)
    .await?;

    let total_withdrawals = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM wallet_transactions WHERE tx_type = 'withdrawal' AND status = 'completed'",
    )
    .fetch_one(&pool)
    .await?;

    let total_payouts = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM payouts WHERE status = 'completed'",
    )
    .fetch_one(&pool)
    .await?;

    let active_users =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE banned_at IS NULL")
            .fetch_one(&pool)
            .await?;

    let total_games = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games")
        .fetch_one(&pool)
        .await?;

    let pending_payouts =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payouts WHERE status = 'pending'")
            .fetch_one(&pool)
            .await?;

    Ok(Json(RevenueDashboard {
        total_platform_revenue,
        total_game_revenue,
        total_withdrawals,
        total_payouts,
        active_users,
        total_games,
        pending_payouts,
    }))
}

pub async fn list_transactions(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse<AdminTransaction>>> {
    require_admin(&pool, user_id).await?;

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await?;

    let transactions = sqlx::query_as::<_, AdminTransaction>(
        "SELECT t.id, t.user_id, u.username, t.game_id, g.title as game_title,
                t.type as tx_type, t.amount, t.status, wt.payout_status, t.created_at
         FROM transactions t
         LEFT JOIN users u ON t.user_id = u.id
         LEFT JOIN games g ON t.game_id = g.id
         LEFT JOIN wallet_transactions wt ON wt.reference_id = t.id::text
         ORDER BY t.created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_pages = (total as f64 / limit as f64).ceil() as i64;

    Ok(Json(PaginatedResponse {
        data: transactions,
        page,
        limit,
        total,
        total_pages,
    }))
}

pub async fn process_payout(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<ProcessPayoutRequest>,
) -> Result<Json<Payout>> {
    require_admin(&pool, user_id).await?;

    let user_balance = sqlx::query_scalar::<_, Decimal>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USDC'",
    )
    .bind(payload.user_id)
    .fetch_optional(&pool)
    .await?
    .unwrap_or(Decimal::ZERO);

    if user_balance < payload.amount {
        return Err(AppError::InsufficientFunds(
            "Insufficient balance for payout".to_string(),
        ));
    }

    sqlx::query(
        "UPDATE wallet_balances SET balance = balance - $1 WHERE user_id = $2 AND currency = 'USDC'",
    )
    .bind(payload.amount)
    .bind(payload.user_id)
    .execute(&pool)
    .await?;

    let payout_id = Uuid::new_v4();
    let payout = sqlx::query_as::<_, Payout>(
        "INSERT INTO payouts (id, user_id, amount, destination, status, processed_at)
         VALUES ($1, $2, $3, $4, 'completed', NOW())
         RETURNING p.id, p.user_id, u.username, p.amount, p.destination, p.status, p.created_at, p.processed_at, p.cancelled_at
         FROM payouts p
         LEFT JOIN users u ON p.user_id = u.id
         WHERE p.id = $1",
    )
    .bind(payout_id)
    .bind(payload.user_id)
    .bind(payload.amount)
    .bind(&payload.destination)
    .fetch_one(&pool)
    .await?;

    Ok(Json(payout))
}

pub async fn cancel_payout(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(payout_id): Path<Uuid>,
) -> Result<Json<Payout>> {
    require_admin(&pool, user_id).await?;

    let payout = sqlx::query_as::<_, Payout>(
        "UPDATE payouts SET status = 'cancelled', cancelled_at = NOW()
         WHERE id = $1 AND status = 'pending'
         RETURNING p.id, p.user_id, u.username, p.amount, p.destination, p.status, p.created_at, p.processed_at, p.cancelled_at
         FROM payouts p
         LEFT JOIN users u ON p.user_id = u.id
         WHERE p.id = $1",
    )
    .bind(payout_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Payout not found or not cancellable".to_string()))?;

    Ok(Json(payout))
}

pub async fn health_check(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
) -> Result<Json<HealthResponse>> {
    let db_status = sqlx::query("SELECT 1")
        .fetch_optional(&pool)
        .await
        .map(|_| "healthy".to_string())
        .map_err(|_| "unhealthy".to_string())
        .unwrap_or_else(|s| s);

    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        database: db_status,
        timestamp: chrono::Utc::now(),
    }))
}

pub async fn get_metrics(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<MetricsResponse>> {
    require_admin(&pool, user_id).await?;

    let total_users = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await?;

    let total_games = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games")
        .fetch_one(&pool)
        .await?;

    let total_transactions = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await?;

    let active_games = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM games WHERE status = 'approved' AND active = true",
    )
    .fetch_one(&pool)
    .await?;

    let pending_payouts =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payouts WHERE status = 'pending'")
            .fetch_one(&pool)
            .await?;

    Ok(Json(MetricsResponse {
        total_users,
        total_games,
        total_transactions,
        active_games,
        pending_payouts,
    }))
}

#[derive(Debug, Serialize)]
pub struct AnalyticsOverview {
    pub total_users: i64,
    pub total_games: i64,
    pub total_play_sessions: i64,
    pub total_revenue_usdc: Decimal,
    pub active_users_24h: i64,
    pub new_users_today: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RevenueTimeSeries {
    pub date: String,
    pub platform_revenue: Decimal,
    pub game_revenue: Decimal,
    pub total_revenue: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RevenueByGame {
    pub game_id: Uuid,
    pub game_title: String,
    pub developer_username: Option<String>,
    pub total_revenue: Decimal,
    pub play_sessions: i64,
    pub platform_fee: Decimal,
    pub developer_payout: Decimal,
}

#[derive(Debug, Serialize)]
pub struct RevenueAnalytics {
    pub daily: Vec<RevenueTimeSeries>,
    pub weekly: Vec<RevenueTimeSeries>,
    pub monthly: Vec<RevenueTimeSeries>,
    pub by_game: Vec<RevenueByGame>,
    pub total_platform_revenue: Decimal,
    pub total_developer_payouts: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserTimeSeries {
    pub date: String,
    pub new_users: i64,
    pub active_users: i64,
}

#[derive(Debug, Serialize)]
pub struct RetentionMetric {
    pub period: String,
    pub day_1_retention: Option<f64>,
    pub day_7_retention: Option<f64>,
    pub day_30_retention: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct UserAnalytics {
    pub registrations_over_time: Vec<UserTimeSeries>,
    pub dau: i64,
    pub wau: i64,
    pub mau: i64,
    pub retention: Vec<RetentionMetric>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GameStatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PopularGame {
    pub game_id: Uuid,
    pub game_title: String,
    pub developer_username: Option<String>,
    pub player_count: i64,
    pub play_sessions: i64,
    pub revenue: Decimal,
}

#[derive(Debug, Serialize)]
pub struct GameAnalytics {
    pub games_by_status: Vec<GameStatusCount>,
    pub popular_games: Vec<PopularGame>,
    pub total_approved: i64,
    pub total_pending: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PerformanceMetric {
    pub endpoint: String,
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub request_count: i64,
    pub error_count: i64,
    pub error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct PerformanceAnalytics {
    pub api_response_times: Vec<PerformanceMetric>,
    pub total_requests_24h: i64,
    pub total_errors_24h: i64,
    pub overall_error_rate: f64,
    pub active_websocket_connections: i64,
}

pub async fn analytics_overview(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<AnalyticsOverview>> {
    require_admin(&pool, user_id).await?;

    let total_users = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await?;

    let total_games = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games")
        .fetch_one(&pool)
        .await?;

    let total_play_sessions = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM transactions WHERE type = 'play_session'",
    )
    .fetch_one(&pool)
    .await?;

    let total_revenue_usdc = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE type IN ('platform_fee', 'game_fee')",
    )
    .fetch_one(&pool)
    .await?;

    let active_users_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT user_id) FROM transactions WHERE created_at > NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(&pool)
    .await?;

    let new_users_today =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE created_at > CURRENT_DATE")
            .fetch_one(&pool)
            .await?;

    Ok(Json(AnalyticsOverview {
        total_users,
        total_games,
        total_play_sessions,
        total_revenue_usdc,
        active_users_24h,
        new_users_today,
    }))
}

pub async fn analytics_revenue(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<RevenueAnalytics>> {
    require_admin(&pool, user_id).await?;

    let daily = sqlx::query_as::<_, RevenueTimeSeries>(
        "SELECT 
            DATE(t.created_at) as date,
            COALESCE(SUM(CASE WHEN t.type = 'platform_fee' THEN t.amount ELSE 0 END), 0) as platform_revenue,
            COALESCE(SUM(CASE WHEN t.type = 'game_fee' THEN t.amount ELSE 0 END), 0) as game_revenue,
            COALESCE(SUM(t.amount), 0) as total_revenue
         FROM transactions t
         WHERE t.created_at > NOW() - INTERVAL '30 days'
         GROUP BY DATE(t.created_at)
         ORDER BY date DESC",
    )
    .fetch_all(&pool)
    .await?;

    let weekly = sqlx::query_as::<_, RevenueTimeSeries>(
        "SELECT 
            DATE_TRUNC('week', t.created_at)::date as date,
            COALESCE(SUM(CASE WHEN t.type = 'platform_fee' THEN t.amount ELSE 0 END), 0) as platform_revenue,
            COALESCE(SUM(CASE WHEN t.type = 'game_fee' THEN t.amount ELSE 0 END), 0) as game_revenue,
            COALESCE(SUM(t.amount), 0) as total_revenue
         FROM transactions t
         WHERE t.created_at > NOW() - INTERVAL '12 weeks'
         GROUP BY DATE_TRUNC('week', t.created_at)
         ORDER BY date DESC",
    )
    .fetch_all(&pool)
    .await?;

    let monthly = sqlx::query_as::<_, RevenueTimeSeries>(
        "SELECT 
            DATE_TRUNC('month', t.created_at)::date as date,
            COALESCE(SUM(CASE WHEN t.type = 'platform_fee' THEN t.amount ELSE 0 END), 0) as platform_revenue,
            COALESCE(SUM(CASE WHEN t.type = 'game_fee' THEN t.amount ELSE 0 END), 0) as game_revenue,
            COALESCE(SUM(t.amount), 0) as total_revenue
         FROM transactions t
         WHERE t.created_at > NOW() - INTERVAL '12 months'
         GROUP BY DATE_TRUNC('month', t.created_at)
         ORDER BY date DESC",
    )
    .fetch_all(&pool)
    .await?;

    let by_game = sqlx::query_as::<_, RevenueByGame>(
        "SELECT 
            g.id as game_id,
            g.title as game_title,
            u.username as developer_username,
            COALESCE(SUM(t.amount), 0) as total_revenue,
            COUNT(t.id) as play_sessions,
            COALESCE(SUM(CASE WHEN t.type = 'platform_fee' THEN t.amount ELSE 0 END), 0) as platform_fee,
            COALESCE(SUM(CASE WHEN t.type = 'game_fee' THEN t.amount ELSE 0 END), 0) as developer_payout
         FROM games g
         LEFT JOIN transactions t ON g.id = t.game_id AND t.type IN ('platform_fee', 'game_fee')
         LEFT JOIN users u ON g.developer_id = u.id
         GROUP BY g.id, g.title, u.username
         ORDER BY total_revenue DESC",
    )
    .fetch_all(&pool)
    .await?;

    let total_platform_revenue = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE type = 'platform_fee'",
    )
    .fetch_one(&pool)
    .await?;

    let total_developer_payouts = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE type = 'game_fee'",
    )
    .fetch_one(&pool)
    .await?;

    Ok(Json(RevenueAnalytics {
        daily,
        weekly,
        monthly,
        by_game,
        total_platform_revenue,
        total_developer_payouts,
    }))
}

pub async fn analytics_users(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<UserAnalytics>> {
    require_admin(&pool, user_id).await?;

    let registrations_over_time = sqlx::query_as::<_, UserTimeSeries>(
        "SELECT 
            DATE(created_at) as date,
            COUNT(*) as new_users,
            COUNT(*) as active_users
         FROM users
         WHERE created_at > NOW() - INTERVAL '30 days'
         GROUP BY DATE(created_at)
         ORDER BY date DESC",
    )
    .fetch_all(&pool)
    .await?;

    let dau = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT user_id) FROM transactions WHERE created_at > CURRENT_DATE",
    )
    .fetch_one(&pool)
    .await?;

    let wau = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT user_id) FROM transactions WHERE created_at > NOW() - INTERVAL '7 days'",
    )
    .fetch_one(&pool)
    .await?;

    let mau = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT user_id) FROM transactions WHERE created_at > NOW() - INTERVAL '30 days'",
    )
    .fetch_one(&pool)
    .await?;

    let day_1_retention = sqlx::query_scalar::<_, Option<f64>>(
        "WITH new_users AS (
            SELECT user_id, DATE(created_at) as signup_date
            FROM users WHERE created_at > NOW() - INTERVAL '14 days'
        ),
        returning_users AS (
            SELECT DISTINCT user_id, DATE(created_at) as activity_date
            FROM transactions WHERE created_at > NOW() - INTERVAL '14 days'
        )
        SELECT 
            CASE 
                WHEN COUNT(DISTINCT nu.user_id) > 0 
                THEN (COUNT(DISTINCT ru.user_id)::float / COUNT(DISTINCT nu.user_id)::float * 100)
                ELSE NULL 
            END as retention
        FROM new_users nu
        LEFT JOIN returning_users ru ON nu.user_id = ru.user_id 
            AND ru.activity_date = nu.signup_date + INTERVAL '1 day'",
    )
    .fetch_one(&pool)
    .await?;

    let retention = vec![RetentionMetric {
        period: "30d".to_string(),
        day_1_retention,
        day_7_retention: None,
        day_30_retention: None,
    }];

    Ok(Json(UserAnalytics {
        registrations_over_time,
        dau,
        wau,
        mau,
        retention,
    }))
}

pub async fn analytics_games(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<GameAnalytics>> {
    require_admin(&pool, user_id).await?;

    let games_by_status = sqlx::query_as::<_, GameStatusCount>(
        "SELECT status, COUNT(*) as count FROM games GROUP BY status",
    )
    .fetch_all(&pool)
    .await?;

    let popular_games = sqlx::query_as::<_, PopularGame>(
        "SELECT 
            g.id as game_id,
            g.title as game_title,
            u.username as developer_username,
            COUNT(DISTINCT t.user_id) as player_count,
            COUNT(t.id) as play_sessions,
            COALESCE(SUM(t.amount), 0) as revenue
         FROM games g
         LEFT JOIN transactions t ON g.id = t.game_id AND t.type IN ('platform_fee', 'game_fee')
         LEFT JOIN users u ON g.developer_id = u.id
         WHERE g.status = 'approved'
         GROUP BY g.id, g.title, u.username
         ORDER BY player_count DESC
         LIMIT 20",
    )
    .fetch_all(&pool)
    .await?;

    let total_approved =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games WHERE status = 'approved'")
            .fetch_one(&pool)
            .await?;

    let total_pending =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games WHERE status = 'pending'")
            .fetch_one(&pool)
            .await?;

    Ok(Json(GameAnalytics {
        games_by_status,
        popular_games,
        total_approved,
        total_pending,
    }))
}

pub async fn analytics_performance(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<PerformanceAnalytics>> {
    require_admin(&pool, user_id).await?;

    // The tables api_request_logs and websocket_connections do not exist yet.
    // Decision §4c: rewrite against existing tables rather than adding a migration for
    // infra-only tables that have no writers yet.  We derive proxy metrics from the
    // `transactions` table (which exists and has real data).
    let total_requests_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM transactions WHERE created_at > NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(&pool)
    .await?;

    let total_errors_24h: i64 = 0; // no error log table — return 0 until api_request_logs is added

    let overall_error_rate = 0.0_f64; // same — no data yet

    // Build a synthetic PerformanceMetric row from transaction stats.
    let tx_count_24h = total_requests_24h;
    let api_response_times = if tx_count_24h > 0 {
        vec![PerformanceMetric {
            endpoint: "transactions".to_string(),
            avg_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            request_count: tx_count_24h,
            error_count: 0,
            error_rate: 0.0,
        }]
    } else {
        vec![]
    };

    // active_websocket_connections: count voices / chat participants as a proxy.
    let active_websocket_connections = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM voice_participants WHERE left_at IS NULL",
    )
    .fetch_optional(&pool)
    .await?
    .unwrap_or(0);

    Ok(Json(PerformanceAnalytics {
        api_response_times,
        total_requests_24h,
        total_errors_24h,
        overall_error_rate,
        active_websocket_connections,
    }))
}

pub async fn seed_test_data(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<impl IntoResponse> {
    require_admin(&pool, user_id).await?;

    let admin_user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, is_admin, is_developer)
         VALUES ($1, 'admin', 'admin@test.com', $2, true, true)
         ON CONFLICT (username) DO NOTHING",
    )
    .bind(admin_user_id)
    .bind(crate::services::auth::hash_password("admin123").unwrap_or_default())
    .execute(&pool)
    .await?;

    let dev_user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, is_admin, is_developer)
         VALUES ($1, 'developer', 'dev@test.com', $2, false, true)
         ON CONFLICT (username) DO NOTHING",
    )
    .bind(dev_user_id)
    .bind(crate::services::auth::hash_password("dev123").unwrap_or_default())
    .execute(&pool)
    .await?;

    let regular_user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, is_admin, is_developer)
         VALUES ($1, 'player', 'player@test.com', $2, false, false)
         ON CONFLICT (username) DO NOTHING",
    )
    .bind(regular_user_id)
    .bind(crate::services::auth::hash_password("player123").unwrap_or_default())
    .execute(&pool)
    .await?;

    let game_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO games (id, developer_id, github_repo, title, description, fee_per_session, status, active)
         VALUES ($1, $2, 'https://github.com/test/game', 'Test Game', 'A test game', 1.0, 'approved', true)
         ON CONFLICT DO NOTHING",
    )
    .bind(game_id)
    .bind(dev_user_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO wallet_balances (id, user_id, currency, balance)
         VALUES ($1, $2, 'USDC', 1000.0)
         ON CONFLICT (user_id, currency) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(regular_user_id)
    .execute(&pool)
    .await?;

    let status = StatusCode::CREATED;
    Ok((
        status,
        Json(serde_json::json!({ "message": "Test data seeded successfully" })),
    ))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route(
            "/users",
            get(list_users).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/users/:id",
            get(get_user).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/users/:id/role",
            put(update_user_role).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/users/:id/ban",
            put(ban_user).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games",
            get(list_games).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id/review",
            put(review_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id/approve",
            put(approve_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id/feature",
            put(feature_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/revenue",
            get(revenue_dashboard).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/transactions",
            get(list_transactions).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/payouts/process",
            post(process_payout).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/payouts/:id/cancel",
            post(cancel_payout).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/health",
            get(health_check).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/metrics",
            get(get_metrics).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/seed",
            post(seed_test_data).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/analytics/overview",
            get(analytics_overview).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/analytics/revenue",
            get(analytics_revenue).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/analytics/users",
            get(analytics_users).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/analytics/games",
            get(analytics_games).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/analytics/performance",
            get(analytics_performance).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
