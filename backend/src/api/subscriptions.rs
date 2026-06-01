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

/// Proration factor for an upgrade/downgrade: remaining-period fraction.
/// Returns a value in [0.0, 1.0].
fn proration_factor(
    period_start: chrono::DateTime<chrono::Utc>,
    period_end: chrono::DateTime<chrono::Utc>,
) -> Decimal {
    let now = chrono::Utc::now();
    let total_secs = (period_end - period_start).num_seconds().max(1);
    let remaining_secs = (period_end - now).num_seconds().max(0);
    // remaining / total, clamped to [0, 1]
    let factor = Decimal::new(remaining_secs.min(total_secs), 0) / Decimal::new(total_secs, 0);
    factor.min(Decimal::ONE).max(Decimal::ZERO)
}

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
    pub payment_provider: String,
    pub cancel_at_period_end: bool,
    pub current_period_start: chrono::DateTime<chrono::Utc>,
    pub current_period_end: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserSubscriptionResponse {
    pub id: Uuid,
    pub tier: SubscriptionTierResponse,
    pub status: String,
    /// When true the subscription will not renew; it expires at current_period_end.
    pub cancel_at_period_end: bool,
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

/// Shared type for downgrade requests — same shape as upgrade.
pub type DowngradeRequest = UpgradeRequest;

#[derive(Debug, Serialize)]
pub struct ChangeTierResponse {
    pub subscription: UserSubscriptionResponse,
    /// Prorated ZAR amount charged (positive) or credited (negative) for this tier change.
    pub prorated_amount_zar: Decimal,
    /// Human-readable description of what happened.
    pub message: String,
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
        "SELECT us.id, us.user_id, us.tier_id, us.status,
                COALESCE(us.payment_provider, 'free') AS payment_provider,
                COALESCE(us.cancel_at_period_end, false) AS cancel_at_period_end,
                us.current_period_start, us.current_period_end, us.created_at
         FROM user_subscriptions us
         WHERE us.user_id = $1 AND us.status IN ('active', 'cancel_pending')
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
                cancel_at_period_end: sub.cancel_at_period_end,
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
        "SELECT id, user_id, tier_id, status,
                COALESCE(payment_provider, 'free') AS payment_provider,
                COALESCE(cancel_at_period_end, false) AS cancel_at_period_end,
                current_period_start, current_period_end, created_at
         FROM user_subscriptions WHERE user_id = $1 AND status IN ('active', 'cancel_pending')",
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
        "INSERT INTO user_subscriptions
             (id, user_id, tier_id, status, payment_provider, cancel_at_period_end,
              current_period_start, current_period_end, created_at)
         VALUES ($1, $2, $3, $4, $5, false, $6, $7, $6)
         RETURNING id, user_id, tier_id, status,
                   COALESCE(payment_provider, 'free') AS payment_provider,
                   COALESCE(cancel_at_period_end, false) AS cancel_at_period_end,
                   current_period_start, current_period_end, created_at",
    )
    .bind(subscription_id)
    .bind(user_id)
    .bind(payload.tier_id)
    .bind(subscription_status)
    .bind(&provider)
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
        cancel_at_period_end: subscription.cancel_at_period_end,
        current_period_start: subscription.current_period_start,
        current_period_end: subscription.current_period_end,
    }))
}

pub async fn cancel_subscription(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<UserSubscription>>> {
    // Cancel-at-period-end: set the flag and move to 'cancel_pending'.
    // The renewal job (SubscriptionService::process_renewals) will set the final
    // 'cancelled' status when current_period_end passes.
    let subscription = sqlx::query_as::<_, UserSubscription>(
        "UPDATE user_subscriptions
         SET status = 'cancel_pending',
             cancel_at_period_end = true,
             updated_at = NOW()
         WHERE user_id = $1 AND status = 'active'
         RETURNING id, user_id, tier_id, status,
                   COALESCE(payment_provider, 'free') AS payment_provider,
                   COALESCE(cancel_at_period_end, true) AS cancel_at_period_end,
                   current_period_start, current_period_end, created_at",
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

/// Core logic for tier changes (upgrade and downgrade).
///
/// Proration: prorated_delta = (new_price_zar − old_price_zar) × remaining_fraction.
/// Positive delta (upgrade)  → requires payment_id covering the top-up charge.
/// Negative delta (downgrade) → records a credit transaction (no Paystack refund in v1).
///
/// The old subscription is immediately cancelled. The new subscription inherits the
/// remaining period so the user does not lose paid days.
async fn change_subscription_tier(
    pool: &PgPool,
    user_id: Uuid,
    new_tier_id: Uuid,
    payment_id: Option<&str>,
    direction: &str,
) -> Result<ChangeTierResponse> {
    // Load target tier.
    let new_tier = sqlx::query_as::<_, SubscriptionTierDb>(
        "SELECT id, name, slug, price_usdc, price_zar, features, max_games, is_active
         FROM subscription_tiers WHERE id = $1 AND is_active = true",
    )
    .bind(new_tier_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Subscription tier not found".to_string()))?;

    // Load current active (or cancel_pending) subscription.
    let current = sqlx::query_as::<_, UserSubscription>(
        "SELECT id, user_id, tier_id, status,
                COALESCE(payment_provider, 'free') AS payment_provider,
                COALESCE(cancel_at_period_end, false) AS cancel_at_period_end,
                current_period_start, current_period_end, created_at
         FROM user_subscriptions
         WHERE user_id = $1 AND status IN ('active', 'cancel_pending')
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Compute proration factor and ZAR delta.
    let (prorated_amount_zar, old_period_end) = if let Some(ref cur) = current {
        let old_tier = sqlx::query_as::<_, SubscriptionTierDb>(
            "SELECT id, name, slug, price_usdc, price_zar, features, max_games, is_active
             FROM subscription_tiers WHERE id = $1",
        )
        .bind(cur.tier_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        let factor = proration_factor(cur.current_period_start, cur.current_period_end);
        let delta = new_tier.price_zar - old_tier.price_zar;
        let prorated = (delta * factor).round_dp(2);
        (prorated, Some(cur.current_period_end))
    } else {
        (Decimal::ZERO, None)
    };

    // Verify Paystack payment when a top-up is required.
    if prorated_amount_zar > Decimal::ZERO {
        let pid = payment_id.ok_or_else(|| {
            AppError::BadRequest(format!(
                "payment_id is required for {} (prorated charge: {} ZAR)",
                direction, prorated_amount_zar
            ))
        })?;
        let payment_svc = PaymentService::from_env();
        let verification = payment_svc
            .verify_paystack_payment(pid)
            .await
            .map_err(|e| AppError::Internal(format!("Paystack verification failed: {}", e)))?;
        let ok_statuses = ["success", "sandbox_success"];
        if !ok_statuses.contains(&verification.status.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Paystack payment '{}' has status '{}' — {} not activated",
                pid, verification.status, direction
            )));
        }
    }

    // Deactivate current subscription.
    if let Some(ref cur) = current {
        sqlx::query(
            "UPDATE user_subscriptions SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
        )
        .bind(cur.id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    // Create new subscription inheriting the remaining period.
    let now = chrono::Utc::now();
    let period_end = old_period_end.unwrap_or_else(|| now + chrono::Duration::days(30));
    let provider = if new_tier.price_zar > Decimal::ZERO {
        "paystack"
    } else {
        "free"
    };
    let new_subscription_id = Uuid::new_v4();
    let new_sub = sqlx::query_as::<_, UserSubscription>(
        "INSERT INTO user_subscriptions
             (id, user_id, tier_id, status, payment_provider, cancel_at_period_end,
              current_period_start, current_period_end, created_at)
         VALUES ($1, $2, $3, 'active', $4, false, $5, $6, $5)
         RETURNING id, user_id, tier_id, status,
                   COALESCE(payment_provider, 'free') AS payment_provider,
                   COALESCE(cancel_at_period_end, false) AS cancel_at_period_end,
                   current_period_start, current_period_end, created_at",
    )
    .bind(new_subscription_id)
    .bind(user_id)
    .bind(new_tier_id)
    .bind(provider)
    .bind(now)
    .bind(period_end)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Record the proration transaction.
    if prorated_amount_zar != Decimal::ZERO {
        let tx_type = if prorated_amount_zar > Decimal::ZERO {
            direction
        } else {
            "downgrade_credit"
        };
        let tx_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO subscription_transactions
                 (id, user_subscription_id, amount, currency, status,
                  payment_provider, payment_id, transaction_type, created_at)
             VALUES ($1, $2, $3, 'ZAR', 'completed', 'paystack', $4, $5, NOW())",
        )
        .bind(tx_id)
        .bind(new_subscription_id)
        .bind(prorated_amount_zar)
        .bind(payment_id.unwrap_or(""))
        .bind(tx_type)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    let message = if prorated_amount_zar > Decimal::ZERO {
        format!(
            "Upgraded to {}. Prorated charge of {} ZAR applied for remaining period.",
            new_tier.name, prorated_amount_zar
        )
    } else if prorated_amount_zar < Decimal::ZERO {
        format!(
            "Downgraded to {}. Prorated credit of {} ZAR recorded.",
            new_tier.name,
            prorated_amount_zar.abs()
        )
    } else {
        format!("Switched to {} tier.", new_tier.name)
    };

    Ok(ChangeTierResponse {
        subscription: UserSubscriptionResponse {
            id: new_sub.id,
            tier: SubscriptionTierResponse {
                id: new_tier.id,
                name: new_tier.name,
                slug: new_tier.slug,
                price_usdc: new_tier.price_usdc,
                price_zar: new_tier.price_zar,
                features: new_tier.features,
                max_games: new_tier.max_games,
            },
            status: new_sub.status,
            cancel_at_period_end: new_sub.cancel_at_period_end,
            current_period_start: new_sub.current_period_start,
            current_period_end: new_sub.current_period_end,
        },
        prorated_amount_zar,
        message,
    })
}

/// POST /api/v1/subscriptions/upgrade — upgrade to a higher tier with proration.
/// `payment_id` is required when the prorated charge is positive.
pub async fn upgrade_subscription(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<UpgradeRequest>,
) -> Result<Json<response::ApiResponse<ChangeTierResponse>>> {
    let result = change_subscription_tier(
        &pool,
        user_id,
        payload.tier_id,
        payload.payment_id.as_deref(),
        "upgrade",
    )
    .await?;
    Ok(response::success_response(result))
}

/// POST /api/v1/subscriptions/downgrade — downgrade to a lower tier with proration credit.
/// No payment required; the credit is recorded in subscription_transactions.
pub async fn downgrade_subscription(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<DowngradeRequest>,
) -> Result<Json<response::ApiResponse<ChangeTierResponse>>> {
    let result = change_subscription_tier(
        &pool,
        user_id,
        payload.tier_id,
        payload.payment_id.as_deref(),
        "downgrade",
    )
    .await?;
    Ok(response::success_response(result))
}

/// GET /api/v1/subscriptions/hours — stub; returns tier-level hour quota.
pub async fn subscription_hours(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<HoursResponse>>> {
    // Look up the active (or cancel_pending) subscription tier to determine included hours.
    let tier_slug: Option<String> = sqlx::query_scalar(
        "SELECT st.slug FROM user_subscriptions us
         JOIN subscription_tiers st ON us.tier_id = st.id
         WHERE us.user_id = $1 AND us.status IN ('active', 'cancel_pending')
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
         WHERE us.user_id = $1 AND us.status IN ('active', 'cancel_pending')
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
        // POST /upgrade — upgrade to a higher tier (with proration)
        .route(
            "/upgrade",
            post(upgrade_subscription).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // POST /downgrade — downgrade to a lower tier (with proration credit)
        .route(
            "/downgrade",
            post(downgrade_subscription).layer(from_fn_with_state(
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
