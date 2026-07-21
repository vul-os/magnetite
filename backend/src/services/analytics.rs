// Analytics service — platform surface area for developer dashboards, not yet wired.
//
// Source of truth for money: `payment_receipts` (seam §3.6, non-custodial). The
// legacy `transactions` table (`type IN ('platform_fee', 'game_fee')`) has had no
// writer since the payment pivot — every query in this file used to read it and
// therefore always returned zero. There is also no platform cut to report: the
// platform takes `PROTOCOL_FEE_BPS` (default 0, real if an operator sets it) and
// nothing else, so "revenue" here means gross settled value and what actually
// reached developer/operator wallets — never a fabricated split.
#![allow(dead_code)]

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAnalytics {
    pub game_id: Uuid,
    pub total_plays: i64,
    pub unique_players: i64,
    pub total_revenue: Decimal,
    pub avg_session_duration_secs: f64,
    pub daily_stats: Vec<DailyStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStat {
    pub date: NaiveDate,
    pub plays: i32,
    pub new_players: i32,
    pub revenue: Decimal,
    pub avg_duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueBreakdown {
    pub game_id: Uuid,
    pub total_revenue: Decimal,
    /// Real sum of `payment_receipts.protocol_fee` — 0 unless PROTOCOL_FEE_BPS
    /// is configured above its default. Never a fabricated percentage.
    pub protocol_fees: Decimal,
    /// Settled to the developer's (and any operator's) wallet at checkout.
    pub developer_earnings: Decimal,
    pub transaction_count: i64,
    pub revenue_by_day: Vec<DailyRevenue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyRevenue {
    pub date: NaiveDate,
    pub gross_revenue: Decimal,
    pub protocol_fees: Decimal,
    pub developer_earnings: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionData {
    pub game_id: Uuid,
    pub cohort_date: NaiveDate,
    pub initial_users: i32,
    pub retention_by_day: Vec<DayRetention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayRetention {
    pub day_number: i32,
    pub active_users: i32,
    pub retention_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub developer_id: Uuid,
    pub total_games: i32,
    pub total_plays: i64,
    pub total_revenue: Decimal,
    pub total_players: i64,
    pub avg_session_duration_secs: f64,
    pub top_game: Option<GameStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GameStats {
    pub game_id: Uuid,
    pub title: String,
    pub total_plays: i64,
    pub unique_players: i64,
    pub total_revenue: Decimal,
    pub avg_session_duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct PlaySessionRow {
    pub total_plays: Option<i64>,
    pub unique_players: Option<i64>,
    pub total_duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct DailyPlayStat {
    pub date: NaiveDate,
    pub plays: i64,
    pub new_players: i64,
    pub total_duration_secs: Option<f64>,
}

/// Raw settled-cents row from `payment_receipts`. Kept as `i64` (the rail's
/// smallest unit) through every query and arithmetic step — only converted to
/// `Decimal` once, at the edge, for display. No float ever touches money here.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct ReceiptCentsRow {
    pub total_cents: Option<i64>,
    pub protocol_fee_cents: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct DailyReceiptCents {
    pub date: NaiveDate,
    pub total_cents: i64,
    pub protocol_fee_cents: i64,
    pub receipt_count: i64,
}

pub async fn get_game_analytics(
    db: &sqlx::PgPool,
    game_id: Uuid,
    days: i32,
) -> Result<GameAnalytics> {
    let start_date = Utc::now().date_naive() - chrono::Duration::days(days as i64);

    let row = sqlx::query_as::<_, PlaySessionRow>(
        r#"
        SELECT
            COUNT(*) as total_plays,
            COUNT(DISTINCT user_id) as unique_players,
            COALESCE(SUM(EXTRACT(EPOCH FROM (ended_at - started_at))), 0) as total_duration_secs
        FROM play_sessions
        WHERE game_id = $1 AND started_at >= $2
        "#,
    )
    .bind(game_id)
    .bind(start_date)
    .fetch_one(db)
    .await?;

    let total_plays = row.total_plays.unwrap_or(0);
    let unique_players = row.unique_players.unwrap_or(0);
    let total_duration_secs = row.total_duration_secs.unwrap_or(0.0);

    let avg_session_duration_secs = if total_plays > 0 {
        total_duration_secs / total_plays as f64
    } else {
        0.0
    };

    // Gross value settled wallet-to-wallet for this game's receipts. Voided
    // receipts backed a refunded entitlement, so they are excluded.
    let total_revenue_cents = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(total), 0)::bigint
        FROM payment_receipts
        WHERE game_id = $1 AND created_at >= $2 AND voided = false
        "#,
    )
    .bind(game_id)
    .bind(start_date)
    .fetch_one(db)
    .await?;
    let total_revenue = Decimal::new(total_revenue_cents, 2);

    let daily_stats_raw = sqlx::query_as::<_, DailyPlayStat>(
        r#"
        WITH new_players_per_day AS (
            SELECT
                DATE_TRUNC('day', ps.started_at) as date,
                COUNT(DISTINCT ps.user_id) FILTER (
                    WHERE ps.user_id NOT IN (
                        SELECT DISTINCT user_id
                        FROM play_sessions
                        WHERE game_id = $1 AND started_at < DATE_TRUNC('day', ps.started_at)
                    )
                ) as new_players
            FROM play_sessions ps
            WHERE ps.game_id = $1 AND ps.started_at >= $2
            GROUP BY DATE_TRUNC('day', ps.started_at)
        )
        SELECT
            DATE_TRUNC('day', ps.started_at)::date as date,
            COUNT(*) as plays,
            COALESCE(np.new_players, 0) as new_players,
            COALESCE(SUM(EXTRACT(EPOCH FROM (ps.ended_at - ps.started_at))), 0) as total_duration_secs
        FROM play_sessions ps
        LEFT JOIN new_players_per_day np ON np.date = DATE_TRUNC('day', ps.started_at)::date
        WHERE ps.game_id = $1 AND ps.started_at >= $2
        GROUP BY DATE_TRUNC('day', ps.started_at), np.new_players
        ORDER BY date
        "#,
    )
    .bind(game_id)
    .bind(start_date)
    .fetch_all(db)
    .await?;

    // Per-day settled revenue for this game, keyed by the same day buckets as
    // the play stats above. Merged in Rust rather than a second GROUP BY on the
    // play-session query so the two independent tables (`play_sessions`,
    // `payment_receipts`) each get their own straightforward query.
    let daily_revenue_raw = sqlx::query_as::<_, DailyReceiptCents>(
        r#"
        SELECT
            DATE_TRUNC('day', created_at)::date as date,
            COALESCE(SUM(total), 0)::bigint as total_cents,
            COALESCE(SUM(protocol_fee), 0)::bigint as protocol_fee_cents,
            COUNT(*) as receipt_count
        FROM payment_receipts
        WHERE game_id = $1 AND created_at >= $2 AND voided = false
        GROUP BY DATE_TRUNC('day', created_at)
        "#,
    )
    .bind(game_id)
    .bind(start_date)
    .fetch_all(db)
    .await?;

    let revenue_by_date: std::collections::HashMap<NaiveDate, i64> = daily_revenue_raw
        .into_iter()
        .map(|r| (r.date, r.total_cents))
        .collect();

    let daily_stats: Vec<DailyStat> = daily_stats_raw
        .into_iter()
        .map(|row| {
            let plays_i32: i32 = row.plays.min(i32::MAX as i64) as i32;
            let revenue_cents = revenue_by_date.get(&row.date).copied().unwrap_or(0);
            DailyStat {
                date: row.date,
                plays: plays_i32,
                new_players: row.new_players.min(i32::MAX as i64) as i32,
                revenue: Decimal::new(revenue_cents, 2),
                avg_duration_secs: if row.plays > 0 {
                    row.total_duration_secs.unwrap_or(0.0) / row.plays as f64
                } else {
                    0.0
                },
            }
        })
        .collect();

    Ok(GameAnalytics {
        game_id,
        total_plays,
        unique_players,
        total_revenue,
        avg_session_duration_secs,
        daily_stats,
    })
}

pub async fn get_revenue_breakdown(db: &sqlx::PgPool, game_id: Uuid) -> Result<RevenueBreakdown> {
    let totals = sqlx::query_as::<_, ReceiptCentsRow>(
        r#"
        SELECT
            COALESCE(SUM(total), 0)::bigint as total_cents,
            COALESCE(SUM(protocol_fee), 0)::bigint as protocol_fee_cents
        FROM payment_receipts
        WHERE game_id = $1 AND voided = false
        "#,
    )
    .bind(game_id)
    .fetch_one(db)
    .await?;

    let total_cents = totals.total_cents.unwrap_or(0);
    let protocol_fee_cents = totals.protocol_fee_cents.unwrap_or(0);
    let developer_cents = total_cents - protocol_fee_cents;

    let total_revenue = Decimal::new(total_cents, 2);
    let protocol_fees = Decimal::new(protocol_fee_cents, 2);
    let developer_earnings = Decimal::new(developer_cents, 2);

    let transaction_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM payment_receipts
        WHERE game_id = $1 AND voided = false
        "#,
    )
    .bind(game_id)
    .fetch_one(db)
    .await?;

    let revenue_by_day_raw = sqlx::query_as::<_, DailyReceiptCents>(
        r#"
        SELECT
            DATE_TRUNC('day', created_at)::date as date,
            COALESCE(SUM(total), 0)::bigint as total_cents,
            COALESCE(SUM(protocol_fee), 0)::bigint as protocol_fee_cents,
            COUNT(*) as receipt_count
        FROM payment_receipts
        WHERE game_id = $1 AND voided = false
        GROUP BY DATE_TRUNC('day', created_at)
        ORDER BY date
        "#,
    )
    .bind(game_id)
    .fetch_all(db)
    .await?;

    // Real per-day protocol fee / developer split — NOT a fabricated
    // percentage (the previous implementation multiplied real revenue by a
    // hardcoded 15%/85% split that no longer exists in the non-custodial model).
    let revenue_by_day: Vec<DailyRevenue> = revenue_by_day_raw
        .into_iter()
        .map(|row| DailyRevenue {
            date: row.date,
            gross_revenue: Decimal::new(row.total_cents, 2),
            protocol_fees: Decimal::new(row.protocol_fee_cents, 2),
            developer_earnings: Decimal::new(row.total_cents - row.protocol_fee_cents, 2),
        })
        .collect();

    Ok(RevenueBreakdown {
        game_id,
        total_revenue,
        protocol_fees,
        developer_earnings,
        transaction_count,
        revenue_by_day,
    })
}

pub async fn get_player_retention(
    db: &sqlx::PgPool,
    game_id: Uuid,
    cohort_date: NaiveDate,
) -> Result<RetentionData> {
    let cohort_start = cohort_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let cohort_end = (cohort_date + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    let initial_users = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(DISTINCT user_id)
        FROM play_sessions
        WHERE game_id = $1 AND started_at >= $2 AND started_at < $3
        "#,
    )
    .bind(game_id)
    .bind(cohort_start)
    .bind(cohort_end)
    .fetch_one(db)
    .await? as i32;

    if initial_users == 0 {
        return Ok(RetentionData {
            game_id,
            cohort_date,
            initial_users: 0,
            retention_by_day: vec![],
        });
    }

    #[derive(Debug, FromRow)]
    struct RetentionDay {
        day_number: i32,
        active_users: i64,
    }

    let retention_raw = sqlx::query_as::<_, RetentionDay>(
        r#"
        WITH cohort_users AS (
            SELECT DISTINCT user_id
            FROM play_sessions
            WHERE game_id = $1 AND started_at >= $2 AND started_at < $3
        ),
        daily_active AS (
            SELECT
                EXTRACT(DAY FROM (ps.started_at - $2::timestamp))::int as day_number,
                COUNT(DISTINCT ps.user_id) as active_users
            FROM play_sessions ps
            INNER JOIN cohort_users cu ON ps.user_id = cu.user_id
            WHERE ps.game_id = $1 AND ps.started_at >= $2
            GROUP BY EXTRACT(DAY FROM (ps.started_at - $2::timestamp))::int
        )
        SELECT day_number, active_users
        FROM daily_active
        ORDER BY day_number
        "#,
    )
    .bind(game_id)
    .bind(cohort_start)
    .bind(cohort_end)
    .fetch_all(db)
    .await?;

    let retention_by_day: Vec<DayRetention> = retention_raw
        .into_iter()
        .map(|row| {
            let retention_rate = (row.active_users as f64 / initial_users as f64) * 100.0;
            DayRetention {
                day_number: row.day_number,
                active_users: row.active_users.min(i32::MAX as i64) as i32,
                retention_rate,
            }
        })
        .collect();

    Ok(RetentionData {
        game_id,
        cohort_date,
        initial_users,
        retention_by_day,
    })
}

pub async fn get_dashboard_summary(
    db: &sqlx::PgPool,
    developer_id: Uuid,
) -> Result<DashboardSummary> {
    let games = sqlx::query_scalar::<_, i32>("SELECT COUNT(*) FROM games WHERE developer_id = $1")
        .bind(developer_id)
        .fetch_one(db)
        .await?;

    let game_ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM games WHERE developer_id = $1")
        .bind(developer_id)
        .fetch_all(db)
        .await?;

    if game_ids.is_empty() {
        return Ok(DashboardSummary {
            developer_id,
            total_games: 0,
            total_plays: 0,
            total_revenue: Decimal::ZERO,
            total_players: 0,
            avg_session_duration_secs: 0.0,
            top_game: None,
        });
    }

    let row = sqlx::query_as::<_, PlaySessionRow>(
        r#"
        SELECT
            COUNT(*) as total_plays,
            COUNT(DISTINCT user_id) as unique_players,
            COALESCE(SUM(EXTRACT(EPOCH FROM (ended_at - started_at))), 0) as total_duration_secs
        FROM play_sessions
        WHERE game_id = ANY($1)
        "#,
    )
    .bind(&game_ids)
    .fetch_one(db)
    .await?;

    let total_plays_val = row.total_plays.unwrap_or(0);
    let total_players_val = row.unique_players.unwrap_or(0);
    let total_duration_secs_val = row.total_duration_secs.unwrap_or(0.0);

    let avg_session_duration_secs = if total_plays_val > 0 {
        total_duration_secs_val / total_plays_val as f64
    } else {
        0.0
    };

    let total_revenue_cents = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(total), 0)::bigint
        FROM payment_receipts
        WHERE game_id = ANY($1) AND voided = false
        "#,
    )
    .bind(&game_ids)
    .fetch_one(db)
    .await?;
    let total_revenue = Decimal::new(total_revenue_cents, 2);

    let top_game = get_top_performing_games(db, developer_id, 1)
        .await?
        .into_iter()
        .next();

    Ok(DashboardSummary {
        developer_id,
        total_games: games,
        total_plays: total_plays_val,
        total_revenue,
        total_players: total_players_val,
        avg_session_duration_secs,
        top_game,
    })
}

pub async fn get_top_performing_games(
    db: &sqlx::PgPool,
    developer_id: Uuid,
    limit: i32,
) -> Result<Vec<GameStats>> {
    #[derive(Debug, FromRow)]
    struct GameStatsCents {
        game_id: Uuid,
        title: String,
        total_plays: i64,
        unique_players: i64,
        total_revenue_cents: i64,
        avg_session_duration_secs: f64,
    }

    let games = sqlx::query_as::<_, GameStatsCents>(
        r#"
        SELECT
            g.id as game_id,
            g.title,
            COALESCE(ps.total_plays, 0) as total_plays,
            COALESCE(ps.unique_players, 0) as unique_players,
            COALESCE(r.total_revenue_cents, 0) as total_revenue_cents,
            COALESCE(ps.avg_duration, 0) as avg_session_duration_secs
        FROM games g
        LEFT JOIN (
            SELECT
                game_id,
                COUNT(*) as total_plays,
                COUNT(DISTINCT user_id) as unique_players,
                COALESCE(AVG(EXTRACT(EPOCH FROM (ended_at - started_at))), 0) as avg_duration
            FROM play_sessions
            GROUP BY game_id
        ) ps ON g.id = ps.game_id
        LEFT JOIN (
            SELECT
                game_id,
                SUM(total)::bigint as total_revenue_cents
            FROM payment_receipts
            WHERE voided = false
            GROUP BY game_id
        ) r ON g.id = r.game_id
        WHERE g.developer_id = $1
        ORDER BY r.total_revenue_cents DESC NULLS LAST, ps.total_plays DESC NULLS LAST
        LIMIT $2
        "#,
    )
    .bind(developer_id)
    .bind(limit)
    .fetch_all(db)
    .await?;

    Ok(games
        .into_iter()
        .map(|g| GameStats {
            game_id: g.game_id,
            title: g.title,
            total_plays: g.total_plays,
            unique_players: g.unique_players,
            total_revenue: Decimal::new(g.total_revenue_cents, 2),
            avg_session_duration_secs: g.avg_session_duration_secs,
        })
        .collect())
}
