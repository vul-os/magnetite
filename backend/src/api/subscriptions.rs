// Subscriptions API — tier management and billing; Paystack fiat on-ramp + free/platform tiers.
#![allow(dead_code)]

use axum::{
    extract::{Extension, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::notifications::NotificationService;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::auth::get_user_by_id;
use crate::services::email::EmailService;
use crate::services::payment::PaymentService;

pub enum SubscriptionTier {
    Free,
    Basic,
    Pro,
    Unlimited,
}

impl SubscriptionTier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "free" => Some(SubscriptionTier::Free),
            "basic" => Some(SubscriptionTier::Basic),
            "pro" => Some(SubscriptionTier::Pro),
            "unlimited" => Some(SubscriptionTier::Unlimited),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionTier::Free => "free",
            SubscriptionTier::Basic => "basic",
            SubscriptionTier::Pro => "pro",
            SubscriptionTier::Unlimited => "unlimited",
        }
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SubscriptionTierDb {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub price_usdc: Decimal,
    pub price_zar: Decimal,
    pub features: serde_json::Value,
    pub max_games: Option<i32>,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionTierResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub price_usdc: Decimal,
    pub price_zar: Decimal,
    pub features: serde_json::Value,
    pub max_games: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tier_id: Uuid,
    pub status: String,
    pub current_period_start: chrono::DateTime<chrono::Utc>,
    pub current_period_end: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserSubscriptionResponse {
    pub id: Uuid,
    pub tier: SubscriptionTierResponse,
    pub status: String,
    pub current_period_start: chrono::DateTime<chrono::Utc>,
    pub current_period_end: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub tier_id: Uuid,
    /// Paystack payment reference to verify. Required for paid tiers.
    pub payment_id: Option<String>,
    /// Payment provider. Only "paystack" (or "platform" for free) is accepted.
    pub payment_provider: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpgradeRequest {
    pub tier_id: Uuid,
    /// Paystack payment reference for any proration charge.  Required if the
    /// new tier costs more than the current tier.
    pub payment_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UsageResponse {
    pub used_games: i64,
    pub max_games: Option<i32>,
    /// Remaining hours estimate based on current period length.
    pub remaining_days: i64,
}

#[derive(Debug, Serialize)]
pub struct HoursResponse {
    /// Total hours of compute included in the current subscription tier.
    pub included_hours: i64,
    /// Hours consumed so far this period (stub: returns 0 until usage tracking is built).
    pub used_hours: i64,
}

pub async fn list_tiers(
    State(pool): State<PgPool>,
) -> Result<Json<response::ApiResponse<Vec<SubscriptionTierResponse>>>> {
    let tiers = sqlx::query_as::<_, SubscriptionTierDb>(
        "SELECT id, name, slug, price_usdc, price_zar, features, max_games, is_active
         FROM subscription_tiers WHERE is_active = true ORDER BY price_usdc ASC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let response: Vec<SubscriptionTierResponse> = tiers
        .into_iter()
        .map(|t| SubscriptionTierResponse {
            id: t.id,
            name: t.name,
            slug: t.slug,
            price_usdc: t.price_usdc,
            price_zar: t.price_zar,
            features: t.features,
            max_games: t.max_games,
        })
        .collect();

    Ok(response::success_response(response))
}

pub async fn get_my_subscription(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<Option<UserSubscriptionResponse>>>> {
    let subscription = sqlx::query_as::<_, UserSubscription>(
        "SELECT us.id, us.user_id, us.tier_id, us.status, us.current_period_start, us.current_period_end, us.created_at
         FROM user_subscriptions us
         JOIN subscription_tiers st ON us.tier_id = st.id
         WHERE us.user_id = $1 AND us.status = 'active'
         ORDER BY us.created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    match subscription {
        Some(sub) => {
            let tier = sqlx::query_as::<_, SubscriptionTierDb>(
                "SELECT id, name, slug, price_usdc, price_zar, features, max_games, is_active
                 FROM subscription_tiers WHERE id = $1",
            )
            .bind(sub.tier_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            Ok(response::success_response(Some(UserSubscriptionResponse {
                id: sub.id,
                tier: SubscriptionTierResponse {
                    id: tier.id,
                    name: tier.name,
                    slug: tier.slug,
                    price_usdc: tier.price_usdc,
                    price_zar: tier.price_zar,
                    features: tier.features,
                    max_games: tier.max_games,
                },
                status: sub.status,
                current_period_start: sub.current_period_start,
                current_period_end: sub.current_period_end,
            })))
        }
        None => Ok(response::success_response(None)),
    }
}

pub async fn subscribe(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<SubscribeRequest>,
) -> Result<Json<response::ApiResponse<UserSubscriptionResponse>>> {
    let tier = sqlx::query_as::<_, SubscriptionTierDb>(
        "SELECT id, name, slug, price_usdc, price_zar, features, max_games, is_active
         FROM subscription_tiers WHERE id = $1 AND is_active = true",
    )
    .bind(payload.tier_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Subscription tier not found".to_string()))?;

    let existing = sqlx::query_as::<_, UserSubscription>(
        "SELECT id, user_id, tier_id, status, current_period_start, current_period_end, created_at
         FROM user_subscriptions WHERE user_id = $1 AND status = 'active'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Already subscribed".to_string()));
    }

    let is_paid = tier.price_usdc > Decimal::ZERO;
    let provider = payload
        .payment_provider
        .as_deref()
        .unwrap_or("paystack")
        .to_string();

    // For paid tiers, verify the Paystack payment before creating the subscription record.
    let (subscription_status, verified_payment_id) = if is_paid {
        let payment_id = payload.payment_id.as_deref().ok_or_else(|| {
            AppError::BadRequest("payment_id is required for paid subscription tiers".to_string())
        })?;

        let payment_svc = PaymentService::from_env();

        match provider.as_str() {
            "paystack" => {
                let verification = payment_svc
                    .verify_paystack_payment(payment_id)
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!("Paystack verification failed: {}", e))
                    })?;

                let ok_statuses = ["success", "sandbox_success"];
                if !ok_statuses.contains(&verification.status.as_str()) {
                    return Err(AppError::BadRequest(format!(
                        "Paystack payment '{}' has status '{}' — subscription not activated",
                        payment_id, verification.status
                    )));
                }

                ("active", payment_id.to_string())
            }
            _ => {
                return Err(AppError::BadRequest(format!(
                    "Unknown payment provider '{}'. Use 'paystack'.",
                    provider
                )));
            }
        }
    } else {
        // Free / platform tier — no payment required.
        ("active", String::new())
    };

    let subscription_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let period_end = now + chrono::Duration::days(30);

    let subscription = sqlx::query_as::<_, UserSubscription>(
        "INSERT INTO user_subscriptions (id, user_id, tier_id, status, current_period_start, current_period_end, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $5)
         RETURNING id, user_id, tier_id, status, current_period_start, current_period_end, created_at",
    )
    .bind(subscription_id)
    .bind(user_id)
    .bind(payload.tier_id)
    .bind(subscription_status)
    .bind(now)
    .bind(period_end)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Record the payment transaction with the correct provider.
    if is_paid && !verified_payment_id.is_empty() {
        let tx_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO subscription_transactions (id, user_subscription_id, amount, currency, status, payment_provider, payment_id, created_at)
             VALUES ($1, $2, $3, 'ZAR', 'completed', $4, $5, NOW())",
        )
        .bind(tx_id)
        .bind(subscription_id)
        .bind(tier.price_zar)
        .bind(&provider)
        .bind(&verified_payment_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    let notif_service = NotificationService::new(pool.clone());
    let _ = notif_service
        .create_subscription_renewal_notification(user_id, &tier.name)
        .await;

    // Send subscription-confirmation email — non-fatal: log on failure, do not roll back.
    match EmailService::from_env() {
        Ok(email_svc) => match get_user_by_id(&pool, user_id).await {
            Ok(Some(user)) => {
                if let Err(e) = email_svc
                    .send_subscription_confirmation_email(
                        &user.email,
                        &user.username,
                        &tier.name,
                        &period_end,
                    )
                    .await
                {
                    tracing::warn!(
                        subscription_id = %subscription_id,
                        user_id = %user_id,
                        "Failed to send subscription-confirmation email (non-fatal): {}",
                        e
                    );
                }
            }
            Ok(None) => {
                tracing::warn!(user_id = %user_id, "Subscription confirmation email skipped: user not found");
            }
            Err(e) => {
                tracing::warn!(user_id = %user_id, "Subscription confirmation email skipped: user lookup failed: {}", e);
            }
        },
        Err(e) => {
            tracing::warn!(user_id = %user_id, "Subscription confirmation email skipped: email service not configured: {}", e);
        }
    }

    Ok(response::success_response(UserSubscriptionResponse {
        id: subscription.id,
        tier: SubscriptionTierResponse {
            id: tier.id,
            name: tier.name,
            slug: tier.slug,
            price_usdc: tier.price_usdc,
            price_zar: tier.price_zar,
            features: tier.features,
            max_games: tier.max_games,
        },
        status: subscription.status,
        current_period_start: subscription.current_period_start,
        current_period_end: subscription.current_period_end,
    }))
}

pub async fn cancel_subscription(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<UserSubscription>>> {
    let subscription = sqlx::query_as::<_, UserSubscription>(
        "UPDATE user_subscriptions SET status = 'cancelled'
         WHERE user_id = $1 AND status = 'active'
         RETURNING id, user_id, tier_id, status, current_period_start, current_period_end, created_at",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("No active subscription found".to_string()))?;

    // Send cancellation email — non-fatal: log on failure, do not roll back.
    match EmailService::from_env() {
        Ok(email_svc) => {
            // Fetch tier name and user details for the email.
            let tier_name_result =
                sqlx::query_as::<_, (String,)>("SELECT name FROM subscription_tiers WHERE id = $1")
                    .bind(subscription.tier_id)
                    .fetch_optional(&pool)
                    .await;

            let tier_name = match tier_name_result {
                Ok(Some((name,))) => name,
                Ok(None) => "your plan".to_string(),
                Err(e) => {
                    tracing::warn!(
                        user_id = %user_id,
                        "Cancellation email: could not fetch tier name: {}",
                        e
                    );
                    "your plan".to_string()
                }
            };

            match get_user_by_id(&pool, user_id).await {
                Ok(Some(user)) => {
                    if let Err(e) = email_svc
                        .send_subscription_cancellation_email(
                            &user.email,
                            &user.username,
                            &tier_name,
                            &subscription.current_period_end,
                        )
                        .await
                    {
                        tracing::warn!(
                            user_id = %user_id,
                            "Failed to send subscription-cancellation email (non-fatal): {}",
                            e
                        );
                    }
                }
                Ok(None) => {
                    tracing::warn!(user_id = %user_id, "Cancellation email skipped: user not found");
                }
                Err(e) => {
                    tracing::warn!(user_id = %user_id, "Cancellation email skipped: user lookup failed: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, "Cancellation email skipped: email service not configured: {}", e);
        }
    }

    Ok(response::success_response(subscription))
}

/// POST /api/v1/subscriptions/upgrade — cancel the current tier and subscribe to a new one.
/// Basic proration stub: the caller supplies a Paystack payment_id covering any delta.
pub async fn upgrade_subscription(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<UpgradeRequest>,
) -> Result<Json<response::ApiResponse<UserSubscriptionResponse>>> {
    // Cancel any existing active subscription.
    sqlx::query(
        "UPDATE user_subscriptions SET status = 'cancelled' WHERE user_id = $1 AND status = 'active'",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Delegate to the subscribe handler logic by calling the DB directly.
    let tier = sqlx::query_as::<_, SubscriptionTierDb>(
        "SELECT id, name, slug, price_usdc, price_zar, features, max_games, is_active
         FROM subscription_tiers WHERE id = $1 AND is_active = true",
    )
    .bind(payload.tier_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Subscription tier not found".to_string()))?;

    let is_paid = tier.price_usdc > rust_decimal::Decimal::ZERO;

    if is_paid {
        let payment_id = payload.payment_id.as_deref().ok_or_else(|| {
            AppError::BadRequest("payment_id is required when upgrading to a paid tier".to_string())
        })?;
        let payment_svc = PaymentService::from_env();
        let verification = payment_svc
            .verify_paystack_payment(payment_id)
            .await
            .map_err(|e| AppError::Internal(format!("Paystack verification failed: {}", e)))?;
        let ok_statuses = ["success", "sandbox_success"];
        if !ok_statuses.contains(&verification.status.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Paystack payment '{}' has status '{}' — upgrade not activated",
                payment_id, verification.status
            )));
        }
    }

    let subscription_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let period_end = now + chrono::Duration::days(30);

    let subscription = sqlx::query_as::<_, UserSubscription>(
        "INSERT INTO user_subscriptions (id, user_id, tier_id, status, current_period_start, current_period_end, created_at)
         VALUES ($1, $2, $3, 'active', $4, $5, $4)
         RETURNING id, user_id, tier_id, status, current_period_start, current_period_end, created_at",
    )
    .bind(subscription_id)
    .bind(user_id)
    .bind(payload.tier_id)
    .bind(now)
    .bind(period_end)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(response::success_response(UserSubscriptionResponse {
        id: subscription.id,
        tier: SubscriptionTierResponse {
            id: tier.id,
            name: tier.name,
            slug: tier.slug,
            price_usdc: tier.price_usdc,
            price_zar: tier.price_zar,
            features: tier.features,
            max_games: tier.max_games,
        },
        status: subscription.status,
        current_period_start: subscription.current_period_start,
        current_period_end: subscription.current_period_end,
    }))
}

/// GET /api/v1/subscriptions/hours — stub; returns tier-level hour quota.
pub async fn subscription_hours(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<HoursResponse>>> {
    // Look up the active subscription tier to determine included hours.
    let tier_slug: Option<String> = sqlx::query_scalar(
        "SELECT st.slug FROM user_subscriptions us
         JOIN subscription_tiers st ON us.tier_id = st.id
         WHERE us.user_id = $1 AND us.status = 'active'
         ORDER BY us.created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let included_hours: i64 = match tier_slug.as_deref() {
        Some("basic") => 10,
        Some("pro") => 50,
        Some("unlimited") => 500,
        _ => 0, // free tier
    };

    Ok(response::success_response(HoursResponse {
        included_hours,
        used_hours: 0, // usage tracking is a future AX2 item
    }))
}

/// GET /api/v1/subscriptions/usage — stub; returns game-slot usage for the current subscription.
pub async fn subscription_usage(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<UsageResponse>>> {
    let row = sqlx::query_as::<_, (i32, Option<i32>, chrono::DateTime<chrono::Utc>)>(
        "SELECT st.max_games, st.max_games, us.current_period_end
         FROM user_subscriptions us
         JOIN subscription_tiers st ON us.tier_id = st.id
         WHERE us.user_id = $1 AND us.status = 'active'
         ORDER BY us.created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let (max_games_raw, max_games, period_end) = row.unwrap_or((0, None, chrono::Utc::now()));
    let _ = max_games_raw; // silence unused warning

    let used_games: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE developer_id = $1 AND active = true")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

    let remaining_days = (period_end - chrono::Utc::now()).num_days().max(0);

    Ok(response::success_response(UsageResponse {
        used_games,
        max_games,
        remaining_days,
    }))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        // Public: list tiers — also serves as /plans alias
        .route("/", get(list_tiers))
        .route("/plans", get(list_tiers))
        // Authenticated routes
        .route(
            "/me",
            get(get_my_subscription).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // /current is an alias for /me (frontend calls both)
        .route(
            "/current",
            get(get_my_subscription).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/",
            post(subscribe).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/",
            delete(cancel_subscription).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // POST /cancel — named alias for DELETE / (frontend calls this form)
        .route(
            "/cancel",
            post(cancel_subscription).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // POST /upgrade — cancel current tier and subscribe to a new one
        .route(
            "/upgrade",
            post(upgrade_subscription).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // GET /hours — tier-level compute-hour quota
        .route(
            "/hours",
            get(subscription_hours).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // GET /usage — game-slot usage for current subscription
        .route(
            "/usage",
            get(subscription_usage).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
