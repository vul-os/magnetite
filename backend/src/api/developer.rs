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
use crate::api::response;
use crate::api::templates;
use crate::error::{AppError, Result};
use crate::services::distribution as dist_svc;
use crate::services::payment;

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
pub struct GameAnalytics {
    pub game_id: Uuid,
    pub daily_active_players: Vec<DailyPlayerData>,
    pub session_duration_stats: SessionStats,
    pub revenue_breakdown: RevenueBreakdown,
    /// 30-day daily revenue buckets (date → USD amount).
    pub daily_revenue: Vec<DailyRevenue>,
    /// 30-day daily playtime buckets (date → total minutes).
    pub daily_playtime: Vec<DailyPlaytime>,
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

/// One daily revenue bucket for a developer's game.
#[derive(Debug, Serialize)]
pub struct DailyRevenue {
    pub date: String,
    pub revenue_usd: Decimal,
}

/// One daily playtime bucket for a developer's game (total minutes played by all users).
#[derive(Debug, Serialize)]
pub struct DailyPlaytime {
    pub date: String,
    pub total_minutes: i64,
}

// ---------------------------------------------------------------------------
// Wise recipient request/response types
// ---------------------------------------------------------------------------

/// POST /api/v1/developer/wise-recipient body.
///
/// Exactly one of the payment-method groups must be populated:
///   • Email: set `email`
///   • IBAN (SEPA/international): set `iban`; optionally set `bic` for non-SEPA routes
///   • US ACH: set `routing_number` + `account_number`
/// Stored Wise recipient row returned to clients (no sensitive bank details).
// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

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

/// GET /api/v1/developer/earnings
///
/// Non-custodial: there is no accrued balance and no payout queue. A developer is
/// paid at the instant of sale, wallet→wallet. "Earnings" is therefore the sum of
/// the signed, non-voided receipts for their games, and `total_paid == total_earnings`.
pub async fn get_earnings_summary(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<EarningsSummary>> {
    // Receipt totals are in the rail's smallest unit (cents) -> back to USD.
    let cents: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(r.total - r.protocol_fee), 0)::bigint
         FROM payment_receipts r
         JOIN games g ON g.id = r.game_id
         WHERE g.developer_id = $1 AND r.voided = false",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    let total_earnings = Decimal::new(cents, 2);

    let recent_earnings = sqlx::query_as::<_, (Uuid, String, i64, i64)>(
        "SELECT g.id, g.title,
                COALESCE(SUM(r.total - r.protocol_fee) FILTER (WHERE r.voided = false), 0)::bigint,
                COUNT(DISTINCT gs.user_id)
         FROM games g
         LEFT JOIN payment_receipts r ON g.id = r.game_id
         LEFT JOIN game_sessions gs ON g.id = gs.game_id
         WHERE g.developer_id = $1
         GROUP BY g.id",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let earnings_by_game: Vec<EarningsByGame> = recent_earnings
        .into_iter()
        .map(|(game_id, title, cents, players)| EarningsByGame {
            game_id,
            game_title: title,
            total_earnings: Decimal::new(cents, 2),
            player_count: players,
        })
        .collect();

    Ok(Json(EarningsSummary {
        total_earnings,
        // Always zero: nothing is ever held on the developer's behalf.
        pending_payout: Decimal::ZERO,
        total_paid: total_earnings,
        recent_earnings: earnings_by_game,
    }))
}

/// GET /api/v1/developer/wallet — the developer's linked payout wallet address.
/// This is the address sales are paid to directly; the platform never holds funds.
pub async fn get_developer_wallet(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let wallet = payment::wallet_of(&pool, user_id).await?;
    Ok(Json(serde_json::json!({
        "wallet_address": wallet.map(|w| w.to_hex()),
        "custodial": false,
        "note": "Sales settle wallet-to-wallet at point of sale. There are no payouts.",
    })))
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

    // ── Daily revenue time-series (30 days) ──────────────────────────────────
    let daily_revenue_rows = sqlx::query_as::<_, (String, Decimal)>(
        "SELECT DATE(created_at)::text AS date, COALESCE(SUM(amount), 0) AS revenue_usd
         FROM game_revenue
         WHERE game_id = $1 AND created_at >= NOW() - INTERVAL '30 days'
         GROUP BY DATE(created_at)
         ORDER BY date",
    )
    .bind(game_id)
    .fetch_all(&pool)
    .await?;

    let daily_revenue: Vec<DailyRevenue> = daily_revenue_rows
        .into_iter()
        .map(|(date, revenue_usd)| DailyRevenue { date, revenue_usd })
        .collect();

    // ── Daily playtime time-series (30 days) ─────────────────────────────────
    // game_sessions stores started_at + ended_at; derive minutes from the interval.
    let daily_playtime_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT DATE(started_at)::text AS date,
                COALESCE(SUM(EXTRACT(EPOCH FROM (ended_at - started_at)) / 60)::bigint, 0) AS total_minutes
         FROM game_sessions
         WHERE game_id = $1
           AND started_at >= NOW() - INTERVAL '30 days'
           AND ended_at IS NOT NULL
         GROUP BY DATE(started_at)
         ORDER BY date",
    )
    .bind(game_id)
    .fetch_all(&pool)
    .await?;

    let daily_playtime: Vec<DailyPlaytime> = daily_playtime_rows
        .into_iter()
        .map(|(date, total_minutes)| DailyPlaytime {
            date,
            total_minutes,
        })
        .collect();

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
        daily_revenue,
        daily_playtime,
    }))
}

// ---------------------------------------------------------------------------
// GDS — Scaffold a new game from a template
// ---------------------------------------------------------------------------

/// POST /api/v1/developer/games/scaffold — body.
#[derive(Debug, Deserialize)]
pub struct ScaffoldGameRequest {
    /// Template id: one of "arcade" | "authoritative" | "fps" | "motorsport".
    pub template_id: String,
    /// Display title for the new game record.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Scaffold info returned to the developer alongside the new game id.
#[derive(Debug, Serialize)]
pub struct ScaffoldInfo {
    /// The `magnetite new` command the developer should run locally.
    pub cli_command: String,
    /// On-disk template crate path (relative to the repo root).
    pub template_path: String,
    /// Canonical template repo reference.
    pub template_repo: String,
    /// Starter file list (informational manifest).
    pub starter_files: Vec<String>,
    /// Additional instructions shown to the developer.
    pub instructions: String,
}

/// Full response for POST /developer/games/scaffold.
#[derive(Debug, Serialize)]
pub struct ScaffoldResponse {
    pub game_id: Uuid,
    pub scaffold: ScaffoldInfo,
}

/// POST /api/v1/developer/games/scaffold
///
/// Creates a `games` record for the authenticated developer seeded from the
/// chosen template, then records the scaffold details in `game_scaffolds`.
/// Returns the new game id plus everything the developer needs to run
/// `magnetite new` locally and point their CLI at this game record.
pub async fn scaffold_game(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<ScaffoldGameRequest>,
) -> Result<Json<ScaffoldResponse>> {
    if payload.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".to_string()));
    }

    let template = templates::find_template(&payload.template_id).ok_or_else(|| {
        AppError::Validation(format!(
            "Unknown template '{}'. Valid values: arcade, authoritative, fps, motorsport",
            payload.template_id
        ))
    })?;

    // Sanitise the title into a valid Rust crate name for the CLI command.
    let crate_name: String = payload
        .title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let crate_name = crate_name.trim_matches('_').to_string();
    let crate_name = if crate_name.is_empty() {
        "my_game".to_string()
    } else {
        crate_name
    };

    // Insert the games row (no github_repo yet — the developer will connect one later).
    let game_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO games (id, developer_id, github_repo, title, description, template_id, status, active, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, 'draft', true, NOW())",
    )
    .bind(game_id)
    .bind(user_id)
    .bind(format!("pending/{}", game_id)) // placeholder until GitHub repo is connected
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(template.id)
    .execute(&pool)
    .await?;

    let cli_command = format!("magnetite new {} --template {}", crate_name, template.id);

    let manifest = serde_json::json!({
        "template_id": template.id,
        "template_path": template.template_path,
        "starter_files": template.starter_files,
    });

    // Record the scaffold action.
    sqlx::query(
        "INSERT INTO game_scaffolds (id, game_id, developer_id, template_id, cli_command, template_repo, manifest, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(game_id)
    .bind(user_id)
    .bind(template.id)
    .bind(&cli_command)
    .bind(template.template_repo)
    .bind(&manifest)
    .execute(&pool)
    .await?;

    let scaffold = ScaffoldInfo {
        cli_command,
        template_path: template.template_path.to_string(),
        template_repo: template.template_repo.to_string(),
        starter_files: template
            .starter_files
            .iter()
            .map(|s| s.to_string())
            .collect(),
        instructions: format!(
            "1. Install the Magnetite CLI: `cargo install magnetite-cli`\n\
             2. Scaffold your game: `magnetite new {} --template {}`\n\
             3. Connect a GitHub repo via POST /api/v1/github/repos\n\
             4. Trigger your first build: POST /api/v1/distribution/{}/build\n\
             5. Check build status: GET /api/v1/developer/games/{}/build-status",
            crate_name, template.id, game_id, game_id
        ),
    };

    Ok(Json(ScaffoldResponse { game_id, scaffold }))
}

// ---------------------------------------------------------------------------
// GDS — Developer-facing build / version / promote wrappers
// ---------------------------------------------------------------------------
// These are thin wrappers that proxy the distribution service behind the
// developer auth middleware and surface them under the /developer/games/:id/
// namespace, so the frontend only needs to know one base path.

/// POST /api/v1/developer/games/:id/build
///
/// Trigger a WASM build for the game. Creates a new `game_versions` row with
/// status "pending" and queues it for the self-hosted runner.
pub async fn trigger_build(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<crate::api::distribution::RegisterVersionRequest>,
) -> Result<Json<response::ApiResponse<dist_svc::GameVersion>>> {
    // Confirm ownership.
    sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND developer_id = $2 AND active = true",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    if payload.version.is_empty() {
        return Err(AppError::Validation("version is required".to_string()));
    }
    if payload.commit_sha.is_empty() {
        return Err(AppError::Validation("commit_sha is required".to_string()));
    }

    let version = dist_svc::register_version(
        &pool,
        game_id,
        &payload.version,
        &payload.commit_sha,
        payload.release_notes.as_deref(),
    )
    .await?;

    Ok(response::success_response(version))
}

/// GET /api/v1/developer/games/:id/build-status
///
/// Returns the current build status summary for a game (owned by the authed developer).
pub async fn get_game_build_status(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<crate::api::distribution::BuildStatusSummary>>> {
    // Confirm ownership.
    sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND developer_id = $2 AND active = true",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    crate::api::distribution::get_build_status_summary(State(pool), Path(game_id)).await
}

/// GET /api/v1/developer/games/:id/versions
///
/// List all registered versions for a game owned by the authed developer.
pub async fn list_game_versions(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<response::PaginatedResponse<dist_svc::GameVersion>>> {
    // Confirm ownership.
    sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND developer_id = $2 AND active = true",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let versions = dist_svc::list_versions(&pool, game_id).await?;
    let total = versions.len() as u64;
    Ok(response::paginated(versions, 1, 50, total))
}

/// PUT /api/v1/developer/games/:game_id/versions/:version_id/promote
///
/// Promote a version to live. Verifies ownership before delegating to the
/// distribution service.
pub async fn promote_game_version(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path((game_id, version_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<response::ApiResponse<dist_svc::GameVersion>>> {
    // Confirm ownership.
    sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND developer_id = $2 AND active = true",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let promoted = dist_svc::promote_version(&pool, game_id, version_id).await?;
    Ok(response::success_response(promoted))
}

/// PUT /api/v1/developer/games/:game_id/versions/:version_id/rollback
///
/// Roll back to a specific version by promoting it (un-promotes any currently
/// live version for this game first, then sets the target as live).
pub async fn rollback_game_version(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path((game_id, version_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<response::ApiResponse<dist_svc::GameVersion>>> {
    // Confirm ownership.
    sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND developer_id = $2 AND active = true",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    // Rollback = demote current live, then promote the target.
    sqlx::query(
        "UPDATE game_versions SET is_live = false, updated_at = NOW()
         WHERE game_id = $1 AND is_live = true",
    )
    .bind(game_id)
    .execute(&pool)
    .await?;

    let promoted = dist_svc::promote_version(&pool, game_id, version_id).await?;
    Ok(response::success_response(promoted))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

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
            "/wallet",
            get(get_developer_wallet).layer(from_fn_with_state(
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
        // Canonical analytics route: /api/v1/developer/games/:id/analytics
        .route(
            "/games/:id/analytics",
            get(get_game_analytics).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // GDS — scaffold a new game from a template
        .route(
            "/games/scaffold",
            post(scaffold_game).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // GDS — build / version / promote / rollback wrappers (ownership-checked)
        .route(
            "/games/:id/build",
            post(trigger_build).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id/build-status",
            get(get_game_build_status).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:id/versions",
            get(list_game_versions).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:game_id/versions/:version_id/promote",
            put(promote_game_version).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/games/:game_id/versions/:version_id/rollback",
            put(rollback_game_version).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
