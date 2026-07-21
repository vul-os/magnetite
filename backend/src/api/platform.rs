// Platform settings API — admin-controlled config.
//
// NON-CUSTODIAL: the fee/payout/deposit/withdraw settings that used to live here
// (platform_fee_percentage, min_payout_amount, max_deposit_amount,
// max_withdraw_amount) were removed — there is no platform-held balance, no
// deposit, no withdrawal and no payout to configure. What remains are the two
// operational toggles that never described money.
use axum::{
    extract::{Extension, State},
    middleware::from_fn_with_state,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;

use crate::error::{AppError, Result};

#[derive(Debug, Serialize)]
pub struct PlatformSettings {
    pub maintenance_mode: bool,
    pub registration_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlatformSettings {
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
    let maintenance_mode = get_setting(&pool, "maintenance_mode")
        .await?
        .parse::<bool>()
        .map_err(|_| AppError::BadRequest("Invalid maintenance_mode".to_string()))?;

    let registration_enabled = get_setting(&pool, "registration_enabled")
        .await?
        .parse::<bool>()
        .map_err(|_| AppError::BadRequest("Invalid registration_enabled".to_string()))?;

    Ok(Json(PlatformSettings {
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
