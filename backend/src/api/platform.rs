// Platform settings API — admin-controlled config.
use axum::{
    extract::{Extension, State},
    middleware::from_fn_with_state,
    routing::{get, put},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;

use crate::error::{AppError, Result};

#[derive(Debug, Serialize)]
pub struct PlatformSettings {
    pub platform_fee_percentage: Decimal,
    pub min_payout_amount: Decimal,
    pub max_deposit_amount: Decimal,
    pub max_withdraw_amount: Decimal,
    pub maintenance_mode: bool,
    pub registration_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlatformSettings {
    pub platform_fee_percentage: Option<Decimal>,
    pub min_payout_amount: Option<Decimal>,
    pub max_deposit_amount: Option<Decimal>,
    pub max_withdraw_amount: Option<Decimal>,
    pub maintenance_mode: Option<bool>,
    pub registration_enabled: Option<bool>,
}

async fn get_setting(pool: &PgPool, key: &str) -> Result<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM platform_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Setting {} not found", key)))
}

async fn set_setting(pool: &PgPool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO platform_settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_settings(State(pool): State<PgPool>) -> Result<Json<PlatformSettings>> {
    let platform_fee_percentage = get_setting(&pool, "platform_fee_percentage")
        .await?
        .parse::<Decimal>()
        .map_err(|_| AppError::BadRequest("Invalid platform_fee_percentage".to_string()))?;

    let min_payout_amount = get_setting(&pool, "min_payout_amount")
        .await?
        .parse::<Decimal>()
        .map_err(|_| AppError::BadRequest("Invalid min_payout_amount".to_string()))?;

    let max_deposit_amount = get_setting(&pool, "max_deposit_amount")
        .await?
        .parse::<Decimal>()
        .map_err(|_| AppError::BadRequest("Invalid max_deposit_amount".to_string()))?;

    let max_withdraw_amount = get_setting(&pool, "max_withdraw_amount")
        .await?
        .parse::<Decimal>()
        .map_err(|_| AppError::BadRequest("Invalid max_withdraw_amount".to_string()))?;

    let maintenance_mode = get_setting(&pool, "maintenance_mode")
        .await?
        .parse::<bool>()
        .map_err(|_| AppError::BadRequest("Invalid maintenance_mode".to_string()))?;

    let registration_enabled = get_setting(&pool, "registration_enabled")
        .await?
        .parse::<bool>()
        .map_err(|_| AppError::BadRequest("Invalid registration_enabled".to_string()))?;

    Ok(Json(PlatformSettings {
        platform_fee_percentage,
        min_payout_amount,
        max_deposit_amount,
        max_withdraw_amount,
        maintenance_mode,
        registration_enabled,
    }))
}

pub async fn update_settings(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<UpdatePlatformSettings>,
) -> Result<Json<PlatformSettings>> {
    let is_admin = sqlx::query_scalar::<_, bool>("SELECT is_admin FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if !is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    if let Some(fee) = payload.platform_fee_percentage {
        if fee < Decimal::ZERO || fee > Decimal::from(100) {
            return Err(AppError::BadRequest(
                "platform_fee_percentage must be between 0 and 100".to_string(),
            ));
        }
        set_setting(&pool, "platform_fee_percentage", &fee.to_string()).await?;
    }

    if let Some(amount) = payload.min_payout_amount {
        if amount < Decimal::ZERO {
            return Err(AppError::BadRequest(
                "min_payout_amount cannot be negative".to_string(),
            ));
        }
        set_setting(&pool, "min_payout_amount", &amount.to_string()).await?;
    }

    if let Some(amount) = payload.max_deposit_amount {
        if amount < Decimal::ZERO {
            return Err(AppError::BadRequest(
                "max_deposit_amount cannot be negative".to_string(),
            ));
        }
        set_setting(&pool, "max_deposit_amount", &amount.to_string()).await?;
    }

    if let Some(amount) = payload.max_withdraw_amount {
        if amount < Decimal::ZERO {
            return Err(AppError::BadRequest(
                "max_withdraw_amount cannot be negative".to_string(),
            ));
        }
        set_setting(&pool, "max_withdraw_amount", &amount.to_string()).await?;
    }

    if let Some(mode) = payload.maintenance_mode {
        set_setting(&pool, "maintenance_mode", &mode.to_string()).await?;
    }

    if let Some(enabled) = payload.registration_enabled {
        set_setting(&pool, "registration_enabled", &enabled.to_string()).await?;
    }

    get_settings(State(pool)).await
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/settings", get(get_settings))
        .route(
            "/settings",
            put(update_settings).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
