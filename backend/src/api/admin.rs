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
    /// Gross value SETTLED wallet-to-wallet, summed from verified receipts.
    /// Non-custodial: we never held this money, we only witnessed the transfers.
    pub total_settled_units: i64,
    /// Sum of `payment_receipts.protocol_fee` — real, never fabricated. The
    /// platform takes no cut by default (`PROTOCOL_FEE_BPS=0`), so this is 0
    /// unless an operator has explicitly configured a nonzero fee.
    pub total_protocol_fee_units: i64,
    /// What actually reached developer (and operator) wallets: settled minus
    /// whatever protocol fee rode on top.
    pub total_developer_settled_units: i64,
    /// Receipts voided by a refund. There is no balance to claw back — voiding
    /// the signed receipt is what revokes the entitlement it granted.
    pub voided_receipts: i64,
    pub active_users: i64,
    pub total_games: i64,
}

/// A settled receipt, shaped for the admin Finance page's receipt table (it
/// reads `payment_receipts`, the live-written non-custodial ledger — the
/// legacy `transactions` table this endpoint used to read has had no writer
/// since the payment pivot).
#[derive(Debug, Serialize)]
pub struct AdminTransaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: Option<String>,
    pub game_id: Option<Uuid>,
    pub game_title: Option<String>,
    /// 'item_purchase' | 'subscription' | 'hosting'
    pub kind: String,
    /// Gross amount the rail settled, in dollars (converted from the receipt's
    /// integer smallest-unit `total`).
    pub total: Decimal,
    /// Real `payment_receipts.protocol_fee`, in dollars — 0 unless an operator
    /// has configured a nonzero `PROTOCOL_FEE_BPS`. Never fabricated.
    pub protocol_fee: Decimal,
    /// Hex wallet the sale was paid to (the developer, or operator for a
    /// hosting-fee receipt) — the first entry of the receipt's `payouts`.
    pub payee: Option<String>,
    pub rail_pubkey: String,
    pub voided: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Raw settled-cents row fetched from `payment_receipts`. Kept as `i64` through
/// the query and only converted to `Decimal` once, at the edge — no float ever
/// touches money here.
#[derive(Debug, sqlx::FromRow)]
struct AdminReceiptCentsRow {
    id: Uuid,
    user_id: Uuid,
    username: Option<String>,
    game_id: Option<Uuid>,
    game_title: Option<String>,
    kind: String,
    total_cents: i64,
    protocol_fee_cents: i64,
    payee: Option<String>,
    rail_pubkey: String,
    voided: bool,
    created_at: chrono::DateTime<chrono::Utc>,
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
    /// Verified, non-voided receipts — the non-custodial replacement for
    /// "pending payouts" (nothing is ever pending: settlement is atomic).
    pub settled_receipts: i64,
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
        "SELECT id, username, email, is_developer, is_admin, banned_at, created_at
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
        "SELECT id, username, email, is_developer, is_admin, banned_at, created_at
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
         RETURNING id, username, email, is_developer, is_admin, banned_at, created_at",
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
         RETURNING id, username, email, is_developer, is_admin, banned_at, created_at",
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

    // Settlement, not custody: sum what the rail actually moved. `payment_receipts`
    // is the live-written ledger post payment-pivot — the legacy `transactions`
    // table (`platform_fee` / `game_fee` rows) has had no writer since, so a query
    // against it always returned zero.
    let total_settled_units = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(total), 0)::bigint FROM payment_receipts WHERE voided = false",
    )
    .fetch_one(&pool)
    .await?;

    let total_protocol_fee_units = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(protocol_fee), 0)::bigint FROM payment_receipts WHERE voided = false",
    )
    .fetch_one(&pool)
    .await?;

    let total_developer_settled_units = total_settled_units - total_protocol_fee_units;

    let voided_receipts =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment_receipts WHERE voided = true")
            .fetch_one(&pool)
            .await?;

    let active_users =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE banned_at IS NULL")
            .fetch_one(&pool)
            .await?;

    let total_games = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games")
        .fetch_one(&pool)
        .await?;

    Ok(Json(RevenueDashboard {
        total_settled_units,
        total_protocol_fee_units,
        total_developer_settled_units,
        voided_receipts,
        active_users,
        total_games,
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

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment_receipts")
        .fetch_one(&pool)
        .await?;

    let rows = sqlx::query_as::<_, AdminReceiptCentsRow>(
        "SELECT r.id, r.buyer_id as user_id, u.username, r.game_id, g.title as game_title,
                r.kind, r.total as total_cents, r.protocol_fee as protocol_fee_cents,
                r.payouts->0->>'wallet' as payee, r.rail_pubkey, r.voided, r.created_at
         FROM payment_receipts r
         LEFT JOIN users u ON r.buyer_id = u.id
         LEFT JOIN games g ON r.game_id = g.id
         ORDER BY r.created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let transactions: Vec<AdminTransaction> = rows
        .into_iter()
        .map(|r| AdminTransaction {
            id: r.id,
            user_id: r.user_id,
            username: r.username,
            game_id: r.game_id,
            game_title: r.game_title,
            kind: r.kind,
            total: Decimal::new(r.total_cents, 2),
            protocol_fee: Decimal::new(r.protocol_fee_cents, 2),
            payee: r.payee,
            rail_pubkey: r.rail_pubkey,
            voided: r.voided,
            created_at: r.created_at,
        })
        .collect();

    let total_pages = (total as f64 / limit as f64).ceil() as i64;

    Ok(Json(PaginatedResponse {
        data: transactions,
        page,
        limit,
        total,
        total_pages,
    }))
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

    // `payment_receipts` is the live-written ledger; the legacy `transactions`
    // table has had no writer since the non-custodial payment pivot.
    let total_transactions = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment_receipts")
        .fetch_one(&pool)
        .await?;

    let active_games = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM games WHERE status = 'approved' AND active = true",
    )
    .fetch_one(&pool)
    .await?;

    let settled_receipts =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payment_receipts WHERE voided = false")
            .fetch_one(&pool)
            .await?;

    Ok(Json(MetricsResponse {
        total_users,
        total_games,
        total_transactions,
        active_games,
        settled_receipts,
    }))
}

#[derive(Debug, Serialize)]
pub struct AnalyticsOverview {
    pub total_users: i64,
    pub total_games: i64,
    pub total_play_sessions: i64,
    pub total_revenue_usd: Decimal,
    pub active_users_24h: i64,
    pub new_users_today: i64,
}

#[derive(Debug, Serialize)]
pub struct RevenueTimeSeries {
    pub date: String,
    /// Real sum of `payment_receipts.protocol_fee` for the period — 0 unless an
    /// operator has configured a nonzero `PROTOCOL_FEE_BPS`. Never fabricated.
    pub protocol_fee: Decimal,
    /// Settled to developer/operator wallets at checkout (never held by us).
    pub developer_settled: Decimal,
    pub total_revenue: Decimal,
}

/// Raw cents row for a `RevenueTimeSeries` bucket, fetched straight off
/// `payment_receipts`. Kept as `i64` until the single conversion to `Decimal`
/// below — no float touches money in this file.
#[derive(Debug, sqlx::FromRow)]
struct RevenueTimeSeriesCentsRow {
    date: String,
    total_cents: i64,
    protocol_fee_cents: i64,
}

fn cents_rows_to_time_series(rows: Vec<RevenueTimeSeriesCentsRow>) -> Vec<RevenueTimeSeries> {
    rows.into_iter()
        .map(|r| RevenueTimeSeries {
            date: r.date,
            protocol_fee: Decimal::new(r.protocol_fee_cents, 2),
            developer_settled: Decimal::new(r.total_cents - r.protocol_fee_cents, 2),
            total_revenue: Decimal::new(r.total_cents, 2),
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct RevenueByGame {
    pub game_id: Uuid,
    pub game_title: String,
    pub developer_username: Option<String>,
    pub total_revenue: Decimal,
    /// Count of settled (non-voided) receipts for this game.
    pub receipt_count: i64,
    /// Real `payment_receipts.protocol_fee` sum — 0 unless an operator has
    /// configured a nonzero `PROTOCOL_FEE_BPS`. Never fabricated.
    pub protocol_fee: Decimal,
    /// Settled to the developer's wallet at checkout (never held by us).
    pub developer_settled: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct RevenueByGameCentsRow {
    game_id: Uuid,
    game_title: String,
    developer_username: Option<String>,
    total_cents: i64,
    receipt_count: i64,
    protocol_fee_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct RevenueAnalytics {
    pub daily: Vec<RevenueTimeSeries>,
    pub weekly: Vec<RevenueTimeSeries>,
    pub monthly: Vec<RevenueTimeSeries>,
    pub by_game: Vec<RevenueByGame>,
    /// Real sum of `payment_receipts.protocol_fee` across all games — 0 unless
    /// an operator has configured a nonzero `PROTOCOL_FEE_BPS`.
    pub total_protocol_fee: Decimal,
    pub total_developer_settled: Decimal,
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

    // `play_sessions` is written by POST /games/:id/sessions (sessions.rs) and is
    // live data; the legacy `transactions` table (`type = 'play_session'`) never
    // reflected real play activity and has had no writer since the payment pivot.
    let total_play_sessions = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM play_sessions")
        .fetch_one(&pool)
        .await?;

    // Gross value settled wallet-to-wallet through verified, non-voided receipts.
    let total_revenue_cents = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(total), 0)::bigint FROM payment_receipts WHERE voided = false",
    )
    .fetch_one(&pool)
    .await?;
    let total_revenue_usd = Decimal::new(total_revenue_cents, 2);

    // Real activity proxy: distinct players who started a play session in the
    // last 24h. `transactions` was never a real source for this either way.
    let active_users_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT user_id) FROM play_sessions WHERE started_at > NOW() - INTERVAL '24 hours'",
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
        total_revenue_usd,
        active_users_24h,
        new_users_today,
    }))
}

pub async fn analytics_revenue(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<RevenueAnalytics>> {
    require_admin(&pool, user_id).await?;

    // `payment_receipts` is the live-written non-custodial ledger. The legacy
    // `transactions` table (`platform_fee` / `game_fee` rows) has had no writer
    // since the payment pivot, so every query below used to return zero. There
    // is also no platform cut to report any more: `protocol_fee` is real (0
    // unless an operator sets `PROTOCOL_FEE_BPS`), never a fabricated split.
    let daily_rows = sqlx::query_as::<_, RevenueTimeSeriesCentsRow>(
        "SELECT
            DATE(r.created_at)::text as date,
            COALESCE(SUM(r.total), 0)::bigint as total_cents,
            COALESCE(SUM(r.protocol_fee), 0)::bigint as protocol_fee_cents
         FROM payment_receipts r
         WHERE r.created_at > NOW() - INTERVAL '30 days' AND r.voided = false
         GROUP BY DATE(r.created_at)
         ORDER BY date DESC",
    )
    .fetch_all(&pool)
    .await?;
    let daily = cents_rows_to_time_series(daily_rows);

    let weekly_rows = sqlx::query_as::<_, RevenueTimeSeriesCentsRow>(
        "SELECT
            DATE_TRUNC('week', r.created_at)::date::text as date,
            COALESCE(SUM(r.total), 0)::bigint as total_cents,
            COALESCE(SUM(r.protocol_fee), 0)::bigint as protocol_fee_cents
         FROM payment_receipts r
         WHERE r.created_at > NOW() - INTERVAL '12 weeks' AND r.voided = false
         GROUP BY DATE_TRUNC('week', r.created_at)
         ORDER BY date DESC",
    )
    .fetch_all(&pool)
    .await?;
    let weekly = cents_rows_to_time_series(weekly_rows);

    let monthly_rows = sqlx::query_as::<_, RevenueTimeSeriesCentsRow>(
        "SELECT
            DATE_TRUNC('month', r.created_at)::date::text as date,
            COALESCE(SUM(r.total), 0)::bigint as total_cents,
            COALESCE(SUM(r.protocol_fee), 0)::bigint as protocol_fee_cents
         FROM payment_receipts r
         WHERE r.created_at > NOW() - INTERVAL '12 months' AND r.voided = false
         GROUP BY DATE_TRUNC('month', r.created_at)
         ORDER BY date DESC",
    )
    .fetch_all(&pool)
    .await?;
    let monthly = cents_rows_to_time_series(monthly_rows);

    let by_game_rows = sqlx::query_as::<_, RevenueByGameCentsRow>(
        "SELECT
            g.id as game_id,
            g.title as game_title,
            u.username as developer_username,
            COALESCE(SUM(r.total), 0)::bigint as total_cents,
            COUNT(r.id) as receipt_count,
            COALESCE(SUM(r.protocol_fee), 0)::bigint as protocol_fee_cents
         FROM games g
         LEFT JOIN payment_receipts r ON g.id = r.game_id AND r.voided = false
         LEFT JOIN users u ON g.developer_id = u.id
         GROUP BY g.id, g.title, u.username
         ORDER BY total_cents DESC",
    )
    .fetch_all(&pool)
    .await?;

    let by_game: Vec<RevenueByGame> = by_game_rows
        .into_iter()
        .map(|r| RevenueByGame {
            game_id: r.game_id,
            game_title: r.game_title,
            developer_username: r.developer_username,
            total_revenue: Decimal::new(r.total_cents, 2),
            receipt_count: r.receipt_count,
            protocol_fee: Decimal::new(r.protocol_fee_cents, 2),
            developer_settled: Decimal::new(r.total_cents - r.protocol_fee_cents, 2),
        })
        .collect();

    let total_protocol_fee_cents = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(protocol_fee), 0)::bigint FROM payment_receipts WHERE voided = false",
    )
    .fetch_one(&pool)
    .await?;

    let total_settled_cents = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(total), 0)::bigint FROM payment_receipts WHERE voided = false",
    )
    .fetch_one(&pool)
    .await?;

    let total_protocol_fee = Decimal::new(total_protocol_fee_cents, 2);
    let total_developer_settled =
        Decimal::new(total_settled_cents - total_protocol_fee_cents, 2);

    Ok(Json(RevenueAnalytics {
        daily,
        weekly,
        monthly,
        by_game,
        total_protocol_fee,
        total_developer_settled,
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
            SELECT id AS user_id, DATE(created_at) AS signup_date
            FROM users WHERE created_at > NOW() - INTERVAL '14 days'
        ),
        returning_users AS (
            SELECT DISTINCT user_id, DATE(created_at) AS activity_date
            FROM transactions WHERE created_at > NOW() - INTERVAL '14 days'
        )
        SELECT
            CASE
                WHEN COUNT(DISTINCT nu.user_id) > 0
                THEN (COUNT(DISTINCT ru.user_id)::float / COUNT(DISTINCT nu.user_id)::float * 100)
                ELSE NULL
            END AS retention
        FROM new_users nu
        LEFT JOIN returning_users ru ON nu.user_id = ru.user_id
            AND ru.activity_date = nu.signup_date + INTERVAL '1 day'",
    )
    .fetch_one(&pool)
    .await?;

    let day_7_retention = sqlx::query_scalar::<_, Option<f64>>(
        "WITH new_users AS (
            SELECT id AS user_id, DATE(created_at) AS signup_date
            FROM users WHERE created_at > NOW() - INTERVAL '37 days'
        ),
        returning_users AS (
            SELECT DISTINCT user_id, DATE(created_at) AS activity_date
            FROM transactions WHERE created_at > NOW() - INTERVAL '37 days'
        )
        SELECT
            CASE
                WHEN COUNT(DISTINCT nu.user_id) > 0
                THEN (COUNT(DISTINCT ru.user_id)::float / COUNT(DISTINCT nu.user_id)::float * 100)
                ELSE NULL
            END AS retention
        FROM new_users nu
        LEFT JOIN returning_users ru ON nu.user_id = ru.user_id
            AND ru.activity_date = nu.signup_date + INTERVAL '7 days'",
    )
    .fetch_one(&pool)
    .await?;

    let day_30_retention = sqlx::query_scalar::<_, Option<f64>>(
        "WITH new_users AS (
            SELECT id AS user_id, DATE(created_at) AS signup_date
            FROM users WHERE created_at > NOW() - INTERVAL '60 days'
        ),
        returning_users AS (
            SELECT DISTINCT user_id, DATE(created_at) AS activity_date
            FROM transactions WHERE created_at > NOW() - INTERVAL '60 days'
        )
        SELECT
            CASE
                WHEN COUNT(DISTINCT nu.user_id) > 0
                THEN (COUNT(DISTINCT ru.user_id)::float / COUNT(DISTINCT nu.user_id)::float * 100)
                ELSE NULL
            END AS retention
        FROM new_users nu
        LEFT JOIN returning_users ru ON nu.user_id = ru.user_id
            AND ru.activity_date = nu.signup_date + INTERVAL '30 days'",
    )
    .fetch_one(&pool)
    .await?;

    let retention = vec![RetentionMetric {
        period: "30d".to_string(),
        day_1_retention,
        day_7_retention,
        day_30_retention,
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

    // Non-custodial: there is no balance to seed. A test player pays from their
    // own wallet through the mock rail and receives a signed receipt.

    let status = StatusCode::CREATED;
    Ok((
        status,
        Json(serde_json::json!({ "message": "Test data seeded successfully" })),
    ))
}

// ── Review moderation ─────────────────────────────────────────────────────────
//
// GET  /admin/review-reports          — list all pending reports (paginated, filterable)
// POST /admin/review-reports/:id/action — dismiss / remove_review / warn / ban

#[derive(Debug, Deserialize)]
pub struct ReviewReportQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    /// Filter by report status: "pending", "dismissed", "resolved" (optional, default: "pending")
    pub status: Option<String>,
    /// Filter by reason string (case-insensitive substring, optional)
    pub reason: Option<String>,
    /// Filter by source: "user" | "auto_flag" (optional, omit for all)
    pub source: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminReviewReport {
    pub id: Uuid,
    pub review_id: Uuid,
    pub reporter_id: Option<Uuid>,
    pub reporter_username: Option<String>,
    /// The reported review's author id
    pub review_author_id: Option<Uuid>,
    /// The reported review's author username
    pub review_author_username: Option<String>,
    /// The rating of the reported review
    pub review_rating: Option<i32>,
    /// Content of the reported review (may be None for brief reviews)
    pub review_content: Option<String>,
    pub reason: String,
    pub status: String,
    /// 'user' for human-filed reports; 'auto_flag' for heuristic-triggered entries
    pub source: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewReportActionRequest {
    /// One of: "dismiss" | "remove_review" | "warn_user" | "ban_user"
    pub action: String,
    /// Optional note recorded on the report
    pub note: Option<String>,
}

pub async fn list_review_reports(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(query): Query<ReviewReportQuery>,
) -> Result<Json<PaginatedResponse<AdminReviewReport>>> {
    require_admin(&pool, user_id).await?;

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;
    let status_filter = query.status.as_deref().unwrap_or("pending");
    let reason_filter = query
        .reason
        .as_deref()
        .map(|r| format!("%{}%", r.to_lowercase()))
        .unwrap_or_else(|| "%".to_string());
    // Optional source filter — None means "all sources".
    let source_filter: Option<String> = query.source.clone();

    let total = if let Some(ref src) = source_filter {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM review_reports rr
             WHERE rr.status = $1 AND LOWER(rr.reason) LIKE $2
               AND COALESCE(rr.source, 'user') = $3",
        )
        .bind(status_filter)
        .bind(&reason_filter)
        .bind(src)
        .fetch_one(&pool)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM review_reports rr
             WHERE rr.status = $1 AND LOWER(rr.reason) LIKE $2",
        )
        .bind(status_filter)
        .bind(&reason_filter)
        .fetch_one(&pool)
        .await?
    };

    let reports = if let Some(ref src) = source_filter {
        sqlx::query_as::<_, AdminReviewReport>(
            "SELECT
                 rr.id,
                 rr.review_id,
                 rr.reporter_id,
                 reporter.username AS reporter_username,
                 r.user_id AS review_author_id,
                 author.username AS review_author_username,
                 r.rating AS review_rating,
                 r.content AS review_content,
                 rr.reason,
                 rr.status,
                 COALESCE(rr.source, 'user') AS source,
                 rr.created_at
             FROM review_reports rr
             LEFT JOIN users reporter ON reporter.id = rr.reporter_id
             LEFT JOIN reviews r      ON r.id = rr.review_id
             LEFT JOIN users author   ON author.id = r.user_id
             WHERE rr.status = $1 AND LOWER(rr.reason) LIKE $2
               AND COALESCE(rr.source, 'user') = $3
             ORDER BY rr.created_at DESC
             LIMIT $4 OFFSET $5",
        )
        .bind(status_filter)
        .bind(&reason_filter)
        .bind(src)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query_as::<_, AdminReviewReport>(
            "SELECT
                 rr.id,
                 rr.review_id,
                 rr.reporter_id,
                 reporter.username AS reporter_username,
                 r.user_id AS review_author_id,
                 author.username AS review_author_username,
                 r.rating AS review_rating,
                 r.content AS review_content,
                 rr.reason,
                 rr.status,
                 COALESCE(rr.source, 'user') AS source,
                 rr.created_at
             FROM review_reports rr
             LEFT JOIN users reporter ON reporter.id = rr.reporter_id
             LEFT JOIN reviews r      ON r.id = rr.review_id
             LEFT JOIN users author   ON author.id = r.user_id
             WHERE rr.status = $1 AND LOWER(rr.reason) LIKE $2
             ORDER BY rr.created_at DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(status_filter)
        .bind(&reason_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await?
    };

    let total_pages = (total as f64 / limit as f64).ceil() as i64;

    Ok(Json(PaginatedResponse {
        data: reports,
        page,
        limit,
        total,
        total_pages,
    }))
}

pub async fn act_on_review_report(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(report_id): Path<Uuid>,
    Json(payload): Json<ReviewReportActionRequest>,
) -> Result<axum::http::StatusCode> {
    require_admin(&pool, user_id).await?;

    // Fetch the report to get its review_id.
    // reporter_id is now nullable (NULL for auto-flag rows), so we only fetch review_id.
    let review_id =
        sqlx::query_scalar::<_, Uuid>("SELECT review_id FROM review_reports WHERE id = $1")
            .bind(report_id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Review report not found".to_string()))?;

    // Resolve the action.
    match payload.action.as_str() {
        "dismiss" => {
            // Mark as dismissed; leave review intact.
            sqlx::query(
                "UPDATE review_reports SET status = 'dismissed', resolved_by = $1,
                 resolved_at = NOW(), resolution_note = $2 WHERE id = $3",
            )
            .bind(user_id)
            .bind(&payload.note)
            .bind(report_id)
            .execute(&pool)
            .await?;
        }
        "remove_review" => {
            // Delete the review (cascade removes related helpful votes and other reports).
            sqlx::query("DELETE FROM reviews WHERE id = $1")
                .bind(review_id)
                .execute(&pool)
                .await?;
            // Mark all reports for this review as resolved.
            sqlx::query(
                "UPDATE review_reports SET status = 'resolved', resolved_by = $1,
                 resolved_at = NOW(), resolution_note = $2 WHERE review_id = $3",
            )
            .bind(user_id)
            .bind(&payload.note)
            .bind(review_id)
            .execute(&pool)
            .await?;
        }
        "warn_user" => {
            // Fetch the review author id.
            let author_id =
                sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM reviews WHERE id = $1")
                    .bind(review_id)
                    .fetch_optional(&pool)
                    .await?;
            if let Some(author_id) = author_id {
                // Record a system notification as the warning mechanism.
                let note = payload
                    .note
                    .as_deref()
                    .unwrap_or("Your review has been flagged by moderators.");
                sqlx::query(
                    "INSERT INTO notifications (id, user_id, type, title, body, read, created_at)
                     VALUES ($1, $2, 'SYSTEM', 'Moderation Warning', $3, false, NOW())",
                )
                .bind(Uuid::new_v4())
                .bind(author_id)
                .bind(note)
                .execute(&pool)
                .await?;
            }
            sqlx::query(
                "UPDATE review_reports SET status = 'resolved', resolved_by = $1,
                 resolved_at = NOW(), resolution_note = $2 WHERE id = $3",
            )
            .bind(user_id)
            .bind(&payload.note)
            .bind(report_id)
            .execute(&pool)
            .await?;
        }
        "ban_user" => {
            // Fetch the review author id.
            let author_id =
                sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM reviews WHERE id = $1")
                    .bind(review_id)
                    .fetch_optional(&pool)
                    .await?;
            if let Some(author_id) = author_id {
                sqlx::query("UPDATE users SET banned_at = NOW(), updated_at = NOW() WHERE id = $1")
                    .bind(author_id)
                    .execute(&pool)
                    .await?;
            }
            // Delete the review and mark all reports resolved.
            sqlx::query("DELETE FROM reviews WHERE id = $1")
                .bind(review_id)
                .execute(&pool)
                .await?;
            sqlx::query(
                "UPDATE review_reports SET status = 'resolved', resolved_by = $1,
                 resolved_at = NOW(), resolution_note = $2 WHERE review_id = $3",
            )
            .bind(user_id)
            .bind(&payload.note)
            .bind(review_id)
            .execute(&pool)
            .await?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown action '{}'. Valid: dismiss | remove_review | warn_user | ban_user",
                other
            )));
        }
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Chat flag moderation ──────────────────────────────────────────────────────
//
// GET  /admin/chat-flags              — list auto-flagged chat messages (paginated, filterable by status)
// POST /admin/chat-flags/:id/action   — dismiss | warn_user | ban_user

#[derive(Debug, Deserialize)]
pub struct ChatFlagQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    /// Filter by status: "pending" | "dismissed" | "resolved"
    pub status: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminChatFlag {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub author_id: Uuid,
    pub author_username: Option<String>,
    pub content: String,
    pub flag_reasons: String,
    pub status: String,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub resolution_note: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ChatFlagActionRequest {
    /// One of: "dismiss" | "warn_user" | "ban_user"
    pub action: String,
    pub note: Option<String>,
}

pub async fn list_chat_flags(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(query): Query<ChatFlagQuery>,
) -> Result<Json<PaginatedResponse<AdminChatFlag>>> {
    require_admin(&pool, user_id).await?;

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;
    let status_filter = query.status.as_deref().unwrap_or("pending");

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chat_flags WHERE status = $1")
        .bind(status_filter)
        .fetch_one(&pool)
        .await?;

    let flags = sqlx::query_as::<_, AdminChatFlag>(
        "SELECT
             cf.id,
             cf.channel_id,
             cf.author_id,
             u.username AS author_username,
             cf.content,
             cf.flag_reasons,
             cf.status,
             cf.resolved_by,
             cf.resolved_at,
             cf.resolution_note,
             cf.created_at
         FROM chat_flags cf
         LEFT JOIN users u ON u.id = cf.author_id
         WHERE cf.status = $1
         ORDER BY cf.created_at DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(status_filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_pages = (total as f64 / limit as f64).ceil() as i64;

    Ok(Json(PaginatedResponse {
        data: flags,
        page,
        limit,
        total,
        total_pages,
    }))
}

pub async fn act_on_chat_flag(
    State(pool): State<PgPool>,
    Extension(admin_id): Extension<Uuid>,
    Path(flag_id): Path<Uuid>,
    Json(payload): Json<ChatFlagActionRequest>,
) -> Result<axum::http::StatusCode> {
    require_admin(&pool, admin_id).await?;

    // Fetch the flag to get the author_id.
    let flag_row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT author_id, status FROM chat_flags WHERE id = $1",
    )
    .bind(flag_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Chat flag not found".to_string()))?;

    let (author_id, current_status) = flag_row;

    if current_status != "pending" {
        return Err(AppError::BadRequest(
            "Chat flag has already been resolved".to_string(),
        ));
    }

    match payload.action.as_str() {
        "dismiss" => {
            sqlx::query(
                "UPDATE chat_flags SET status = 'dismissed', resolved_by = $1,
                 resolved_at = NOW(), resolution_note = $2 WHERE id = $3",
            )
            .bind(admin_id)
            .bind(&payload.note)
            .bind(flag_id)
            .execute(&pool)
            .await?;
        }
        "warn_user" => {
            // Send a system notification as the warning.
            let note = payload
                .note
                .as_deref()
                .unwrap_or("Your chat message was flagged by moderators.");
            sqlx::query(
                "INSERT INTO notifications (id, user_id, type, title, body, read, created_at)
                 VALUES ($1, $2, 'SYSTEM', 'Moderation Warning', $3, false, NOW())",
            )
            .bind(Uuid::new_v4())
            .bind(author_id)
            .bind(note)
            .execute(&pool)
            .await?;

            sqlx::query(
                "UPDATE chat_flags SET status = 'resolved', resolved_by = $1,
                 resolved_at = NOW(), resolution_note = $2 WHERE id = $3",
            )
            .bind(admin_id)
            .bind(&payload.note)
            .bind(flag_id)
            .execute(&pool)
            .await?;
        }
        "ban_user" => {
            sqlx::query("UPDATE users SET banned_at = NOW(), updated_at = NOW() WHERE id = $1")
                .bind(author_id)
                .execute(&pool)
                .await?;

            sqlx::query(
                "UPDATE chat_flags SET status = 'resolved', resolved_by = $1,
                 resolved_at = NOW(), resolution_note = $2 WHERE id = $3",
            )
            .bind(admin_id)
            .bind(&payload.note)
            .bind(flag_id)
            .execute(&pool)
            .await?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown action '{}'. Valid: dismiss | warn_user | ban_user",
                other
            )));
        }
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
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
        // ── Review moderation ──────────────────────────────────────────────
        // GET  /admin/review-reports           — paginated list with report + review data
        // POST /admin/review-reports/:id/action — dismiss | remove_review | warn_user | ban_user
        .route(
            "/review-reports",
            get(list_review_reports).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/review-reports/:id/action",
            post(act_on_review_report).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // ── Chat flag moderation ────────────────────────────────────────────────
        // GET  /admin/chat-flags             — list auto-flagged messages (filter by status)
        // POST /admin/chat-flags/:id/action  — dismiss | warn_user | ban_user
        .route(
            "/chat-flags",
            get(list_chat_flags).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/chat-flags/:id/action",
            post(act_on_chat_flag).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
