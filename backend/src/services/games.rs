// Games service — game metadata, categories, play sessions.
// Decision: api/games.rs queries the DB directly (agents own that file separately); this module
// is the shared typed surface for services that need game data (matchmaking, points, etc.).
// Library surface — not every function is called from a handler; suppress the lint.
#![allow(dead_code)]
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

impl From<String> for GameStatus {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "active" => GameStatus::Active,
            "paused" => GameStatus::Paused,
            "archived" => GameStatus::Archived,
            _ => GameStatus::Draft,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Game {
    pub id: Uuid,
    pub developer_id: Uuid,
    pub github_repo: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub fee_per_session: Decimal,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Score {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub score: i64,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub user_id: Uuid,
    pub username: String,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSettings {
    pub platform_fee_percentage: Decimal,
    pub min_payout_amount: Decimal,
    pub max_deposit_amount: Decimal,
    pub max_withdraw_amount: Decimal,
    pub maintenance_mode: bool,
    pub registration_enabled: bool,
}

async fn get_setting(pool: &sqlx::PgPool, key: &str) -> Result<String, crate::error::AppError> {
    sqlx::query_scalar::<_, String>("SELECT value FROM platform_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Setting {} not found", key)))
}

pub async fn get_platform_settings(
    db: &sqlx::PgPool,
) -> Result<PlatformSettings, crate::error::AppError> {
    let platform_fee_percentage = get_setting(db, "platform_fee_percentage")
        .await?
        .parse::<Decimal>()
        .map_err(|_| {
            crate::error::AppError::BadRequest("Invalid platform_fee_percentage".to_string())
        })?;

    let min_payout_amount = get_setting(db, "min_payout_amount")
        .await?
        .parse::<Decimal>()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid min_payout_amount".to_string()))?;

    let max_deposit_amount = get_setting(db, "max_deposit_amount")
        .await?
        .parse::<Decimal>()
        .map_err(|_| {
            crate::error::AppError::BadRequest("Invalid max_deposit_amount".to_string())
        })?;

    let max_withdraw_amount = get_setting(db, "max_withdraw_amount")
        .await?
        .parse::<Decimal>()
        .map_err(|_| {
            crate::error::AppError::BadRequest("Invalid max_withdraw_amount".to_string())
        })?;

    let maintenance_mode = get_setting(db, "maintenance_mode")
        .await?
        .parse::<bool>()
        .map_err(|_| crate::error::AppError::BadRequest("Invalid maintenance_mode".to_string()))?;

    let registration_enabled = get_setting(db, "registration_enabled")
        .await?
        .parse::<bool>()
        .map_err(|_| {
            crate::error::AppError::BadRequest("Invalid registration_enabled".to_string())
        })?;

    Ok(PlatformSettings {
        platform_fee_percentage,
        min_payout_amount,
        max_deposit_amount,
        max_withdraw_amount,
        maintenance_mode,
        registration_enabled,
    })
}

pub async fn get_all_games(db: &sqlx::PgPool) -> Result<Vec<Game>, crate::error::AppError> {
    let games = sqlx::query_as::<_, Game>(
        "SELECT id, developer_id, github_repo, title, description, fee_per_session, status, created_at
         FROM games WHERE active = true ORDER BY created_at DESC",
    )
    .fetch_all(db)
    .await?;
    Ok(games)
}

pub async fn get_game_by_id(
    db: &sqlx::PgPool,
    id: Uuid,
) -> Result<Option<Game>, crate::error::AppError> {
    let game = sqlx::query_as::<_, Game>(
        "SELECT id, developer_id, github_repo, title, description, fee_per_session, status, created_at
         FROM games WHERE id = $1 AND active = true",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(game)
}

pub async fn create_game(
    db: &sqlx::PgPool,
    developer_id: Uuid,
    title: &str,
    description: Option<&str>,
    fee_per_session: Decimal,
) -> Result<Game, crate::error::AppError> {
    let game = sqlx::query_as::<_, Game>(
        "INSERT INTO games (id, developer_id, title, description, fee_per_session, status, active, created_at)
         VALUES ($1, $2, $3, $4, $5, 'draft', true, NOW())
         RETURNING id, developer_id, github_repo, title, description, fee_per_session, status, created_at",
    )
    .bind(Uuid::new_v4())
    .bind(developer_id)
    .bind(title)
    .bind(description)
    .bind(fee_per_session)
    .fetch_one(db)
    .await?;
    Ok(game)
}

pub async fn update_game(
    db: &sqlx::PgPool,
    id: Uuid,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<GameStatus>,
) -> Result<Game, crate::error::AppError> {
    let status_str = status.map(|s| match s {
        GameStatus::Draft => "draft",
        GameStatus::Active => "active",
        GameStatus::Paused => "paused",
        GameStatus::Archived => "archived",
    });
    let game = sqlx::query_as::<_, Game>(
        "UPDATE games SET
             title = COALESCE($1, title),
             description = COALESCE($2, description),
             status = COALESCE($3, status)
         WHERE id = $4 AND active = true
         RETURNING id, developer_id, github_repo, title, description, fee_per_session, status, created_at",
    )
    .bind(title)
    .bind(description)
    .bind(status_str)
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound(format!("Game {} not found", id)))?;
    Ok(game)
}

pub async fn get_leaderboard(
    db: &sqlx::PgPool,
    game_id: Uuid,
    limit: i32,
) -> Result<Vec<LeaderboardEntry>, crate::error::AppError> {
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
    .fetch_all(db)
    .await?;

    let leaderboard = entries
        .into_iter()
        .enumerate()
        .map(|(i, (user_id, username, score))| LeaderboardEntry {
            rank: (i + 1) as i32,
            user_id,
            username,
            score,
        })
        .collect();
    Ok(leaderboard)
}
