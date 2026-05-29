// Games service — game metadata, categories, play sessions; platform surface, not yet wired.
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

pub async fn get_all_games(_db: &sqlx::PgPool) -> Result<Vec<Game>, crate::error::AppError> {
    todo!()
}

pub async fn get_game_by_id(
    _db: &sqlx::PgPool,
    _id: Uuid,
) -> Result<Option<Game>, crate::error::AppError> {
    todo!()
}

pub async fn create_game(
    _db: &sqlx::PgPool,
    _developer_id: Uuid,
    _title: &str,
    _description: Option<&str>,
    _fee_per_session: Decimal,
) -> Result<Game, crate::error::AppError> {
    todo!()
}

pub async fn update_game(
    _db: &sqlx::PgPool,
    _id: Uuid,
    _title: Option<&str>,
    _description: Option<&str>,
    _status: Option<GameStatus>,
) -> Result<Game, crate::error::AppError> {
    todo!()
}

pub async fn get_leaderboard(
    _db: &sqlx::PgPool,
    _game_id: Uuid,
    _limit: i32,
) -> Result<Vec<LeaderboardEntry>, crate::error::AppError> {
    todo!()
}
