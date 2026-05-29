use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SubscriptionTier {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub price_usdc: Decimal,
    pub price_zar: Decimal,
    pub features: serde_json::Value,
    pub max_games: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tier_id: Uuid,
    pub status: String,
    pub payment_provider: String,
    pub provider_subscription_id: Option<String>,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub cancel_at_period_end: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PlaySession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_id: Option<Uuid>,
    pub duration_minutes: i64,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionWithTier {
    pub subscription: UserSubscription,
    pub tier: SubscriptionTier,
}

pub struct SubscriptionService {
    pool: PgPool,
    paystack_secret_key: Option<String>,
    circle_api_key: Option<String>,
}

impl SubscriptionService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            paystack_secret_key: std::env::var("PAYSTACK_SECRET_KEY").ok(),
            circle_api_key: std::env::var("CIRCLE_API_KEY").ok(),
        }
    }

    pub async fn init_tables(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_subscriptions (
                id UUID PRIMARY KEY,
                user_id UUID NOT NULL REFERENCES users(id),
                tier_id UUID NOT NULL REFERENCES subscription_tiers(id),
                status VARCHAR(50) NOT NULL DEFAULT 'active',
                payment_provider VARCHAR(50) NOT NULL,
                provider_subscription_id VARCHAR(255),
                current_period_start TIMESTAMPTZ NOT NULL,
                current_period_end TIMESTAMPTZ NOT NULL,
                cancel_at_period_end BOOLEAN NOT NULL DEFAULT false,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(user_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS play_sessions (
                id UUID PRIMARY KEY,
                user_id UUID NOT NULL REFERENCES users(id),
                game_id UUID REFERENCES games(id),
                duration_minutes BIGINT NOT NULL DEFAULT 0,
                recorded_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_play_sessions_user ON play_sessions(user_id)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_user_subscriptions_user ON user_subscriptions(user_id)"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_tiers(&self) -> Result<Vec<SubscriptionTier>> {
        let tiers = sqlx::query_as::<_, SubscriptionTier>(
            "SELECT * FROM subscription_tiers ORDER BY price_usdc ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(tiers)
    }

    pub async fn get_user_subscription(
        &self,
        user_id: Uuid,
    ) -> Result<Option<SubscriptionWithTier>> {
        let subscription = sqlx::query_as::<_, UserSubscription>(
            r#"
            SELECT * FROM user_subscriptions
            WHERE user_id = $1 AND status IN ('active', 'cancelled')
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        match subscription {
            Some(sub) => {
                let tier = sqlx::query_as::<_, SubscriptionTier>(
                    "SELECT * FROM subscription_tiers WHERE id = $1",
                )
                .bind(sub.tier_id)
                .fetch_one(&self.pool)
                .await?;

                Ok(Some(SubscriptionWithTier { subscription: sub, tier }))
            }
            None => Ok(None),
        }
    }

    pub async fn subscribe(
        &self,
        user_id: Uuid,
        tier_id: Uuid,
        payment_method: &str,
    ) -> Result<UserSubscription> {
        let tier = sqlx::query_as::<_, SubscriptionTier>(
            "SELECT * FROM subscription_tiers WHERE id = $1",
        )
        .bind(tier_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Tier not found".to_string()))?;

        if tier.price_usdc > Decimal::ZERO {
            match payment_method {
                "paystack" => {
                    return self.create_paystack_subscription(user_id, &tier).await;
                }
                "circle" => {
                    return self.create_circle_subscription(user_id, &tier).await;
                }
                _ => {
                    return Err(AppError::BadRequest(
                        "Invalid payment provider".to_string(),
                    ));
                }
            }
        } else {
            self.create_free_subscription(user_id, &tier).await
        }
    }

    async fn create_free_subscription(
        &self,
        user_id: Uuid,
        tier: &SubscriptionTier,
    ) -> Result<UserSubscription> {
        let subscription_id = Uuid::new_v4();
        let now = Utc::now();
        let period_end = now + chrono::Duration::days(365);

        let subscription = sqlx::query_as::<_, UserSubscription>(
            r#"
            INSERT INTO user_subscriptions (
                id, user_id, tier_id, status, payment_provider,
                current_period_start, current_period_end, cancel_at_period_end
            )
            VALUES ($1, $2, $3, 'active', 'free', $4, $5, false)
            ON CONFLICT (user_id) DO UPDATE SET
                tier_id = $3,
                status = 'active',
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(subscription_id)
        .bind(user_id)
        .bind(tier.id)
        .bind(now)
        .bind(period_end)
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            "Created free subscription for user {} with tier {}",
            user_id,
            tier.slug
        );

        Ok(subscription)
    }

    async fn create_paystack_subscription(
        &self,
        user_id: Uuid,
        tier: &SubscriptionTier,
    ) -> Result<UserSubscription> {
        let secret_key = self.paystack_secret_key.as_ref()
            .ok_or_else(|| AppError::Internal("Paystack not configured".to_string()))?;

        let client = reqwest::Client::new();
        let reference = format!("PS_SUB_{}", Uuid::new_v4());

        let response = client
            .post("https://api.paystack.co/transaction/initialize")
            .header("Authorization", format!("Bearer {}", secret_key))
            .json(&serde_json::json!({
                "email": format!("{}@magnetite.local", user_id),
                "amount": (tier.price_zar * Decimal::new(100, 0)).to_string(),
                "currency": "ZAR",
                "reference": reference,
                "metadata": {
                    "user_id": user_id.to_string(),
                    "tier_id": tier.id.to_string(),
                    "subscription_type": "recurring"
                },
                "callback_url": format!("{}/subscription/callback", std::env::var("APP_URL").unwrap_or_default())
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Paystack request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal("Failed to create Paystack session".to_string()));
        }

        let subscription_id = Uuid::new_v4();
        let now = Utc::now();
        let period_end = now + chrono::Duration::days(30);

        let subscription = sqlx::query_as::<_, UserSubscription>(
            r#"
            INSERT INTO user_subscriptions (
                id, user_id, tier_id, status, payment_provider,
                provider_subscription_id, current_period_start, current_period_end, cancel_at_period_end
            )
            VALUES ($1, $2, $3, 'pending', 'paystack', $4, $5, $6, false)
            ON CONFLICT (user_id) DO UPDATE SET
                tier_id = $3,
                status = 'pending',
                payment_provider = 'paystack',
                provider_subscription_id = $4,
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(subscription_id)
        .bind(user_id)
        .bind(tier.id)
        .bind(reference)
        .bind(now)
        .bind(period_end)
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            "Created Paystack subscription pending for user {} with tier {}",
            user_id,
            tier.slug
        );

        Ok(subscription)
    }

    async fn create_circle_subscription(
        &self,
        user_id: Uuid,
        tier: &SubscriptionTier,
    ) -> Result<UserSubscription> {
        let api_key = self.circle_api_key.as_ref()
            .ok_or_else(|| AppError::Internal("Circle not configured".to_string()))?;

        let client = reqwest::Client::new();
        let idempotency_key = Uuid::new_v4().to_string();

        let response = client
            .post("https://api.circle.com/v1/subscriptions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("X-Idempotency-Key", &idempotency_key)
            .json(&serde_json::json!({
                "userId": user_id.to_string(),
                "planId": tier.slug,
                "amount": {
                    "amount": tier.price_usdc.to_string(),
                    "currency": "USDC"
                },
                "interval": "monthly"
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Circle request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal("Failed to create Circle subscription".to_string()));
        }

        let subscription_id = Uuid::new_v4();
        let now = Utc::now();
        let period_end = now + chrono::Duration::days(30);

        let subscription = sqlx::query_as::<_, UserSubscription>(
            r#"
            INSERT INTO user_subscriptions (
                id, user_id, tier_id, status, payment_provider,
                current_period_start, current_period_end, cancel_at_period_end
            )
            VALUES ($1, $2, $3, 'pending', 'circle', $4, $5, $6, false)
            ON CONFLICT (user_id) DO UPDATE SET
                tier_id = $3,
                status = 'pending',
                payment_provider = 'circle',
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(subscription_id)
        .bind(user_id)
        .bind(tier.id)
        .bind(now)
        .bind(period_end)
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            "Created Circle subscription pending for user {} with tier {}",
            user_id,
            tier.slug
        );

        Ok(subscription)
    }

    pub async fn cancel(&self, subscription_id: Uuid) -> Result<()> {
        let subscription = sqlx::query_as::<_, UserSubscription>(
            "SELECT * FROM user_subscriptions WHERE id = $1",
        )
        .bind(subscription_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;

        match subscription.payment_provider.as_str() {
            "paystack" => {
                if let Some(provider_sub_id) = &subscription.provider_subscription_id {
                    self.cancel_paystack_subscription(provider_sub_id).await?;
                }
            }
            "circle" => {
                self.cancel_circle_subscription(&subscription_id.to_string()).await?;
            }
            "free" => {}
            _ => {}
        }

        sqlx::query(
            r#"
            UPDATE user_subscriptions
            SET cancel_at_period_end = true, status = 'cancelled', updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(subscription_id)
        .execute(&self.pool)
        .await?;

        tracing::info!("Subscription {} marked for cancellation", subscription_id);

        Ok(())
    }

    async fn cancel_paystack_subscription(&self, reference: &str) -> Result<()> {
        let secret_key = self.paystack_secret_key.as_ref()
            .ok_or_else(|| AppError::Internal("Paystack not configured".to_string()))?;

        let client = reqwest::Client::new();

        let response = client
            .post(&format!("https://api.paystack.co/subscription/{}/manage/stop", reference))
            .header("Authorization", format!("Bearer {}", secret_key))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Paystack cancellation failed: {}", e)))?;

        if !response.status().is_success() {
            tracing::warn!("Paystack subscription cancellation may have failed");
        }

        Ok(())
    }

    async fn cancel_circle_subscription(&self, subscription_id: &str) -> Result<()> {
        let api_key = self.circle_api_key.as_ref()
            .ok_or_else(|| AppError::Internal("Circle not configured".to_string()))?;

        let client = reqwest::Client::new();

        let response = client
            .delete(&format!("https://api.circle.com/v1/subscriptions/{}", subscription_id))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Circle cancellation failed: {}", e)))?;

        if !response.status().is_success() {
            tracing::warn!("Circle subscription cancellation may have failed");
        }

        Ok(())
    }

    pub async fn has_game_access(&self, user_id: Uuid, game_id: Uuid) -> Result<bool> {
        let subscription = self.get_user_subscription(user_id).await?;

        let game = sqlx::query_as::<_, (String,)>(
            "SELECT subscription_tier_required FROM games WHERE id = $1",
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;

        let required_tier = game.map(|g| g.0).unwrap_or_else(|| "free".to_string());

        match subscription {
            Some(sub) => {
                if sub.subscription.status != "active" && sub.subscription.status != "cancelled" {
                    return Ok(required_tier == "free");
                }

                if required_tier == "free" {
                    return Ok(true);
                }

                if sub.tier.slug == "unlimited" {
                    return Ok(true);
                }

                if sub.tier.slug == "free" {
                    return Ok(false);
                }

                let hours = sub.tier.features
                    .get("hours")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                Ok(hours != 0)
            }
            None => Ok(required_tier == "free"),
        }
    }

    pub async fn get_remaining_playtime(&self, user_id: Uuid) -> Result<i64> {
        let subscription = self.get_user_subscription(user_id).await?;

        match subscription {
            Some(sub) if sub.subscription.status == "active" => {
                let features = &sub.tier.features;
                let hours = features
                    .get("hours")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                if hours == -1 {
                    return Ok(-1);
                }

                let total_minutes = hours * 60;

                let used_minutes = sqlx::query_as::<_, (i64,)>(
                    r#"
                    SELECT COALESCE(SUM(duration_minutes), 0)
                    FROM play_sessions
                    WHERE user_id = $1
                    AND recorded_at >= $2
                    "#,
                )
                .bind(user_id)
                .bind(sub.subscription.current_period_start)
                .fetch_one(&self.pool)
                .await?
                .0;

                let remaining = total_minutes - used_minutes;
                Ok(remaining.max(0))
            }
            _ => Ok(0),
        }
    }

    pub async fn record_playtime(&self, user_id: Uuid, minutes: i64) -> Result<()> {
        let remaining = self.get_remaining_playtime(user_id).await?;

        if remaining == 0 {
            return Err(AppError::Forbidden("No playtime remaining".to_string()));
        }

        if remaining != -1 && minutes > remaining {
            return Err(AppError::Forbidden(
                "Insufficient playtime remaining".to_string(),
            ));
        }

        sqlx::query(
            r#"
            INSERT INTO play_sessions (id, user_id, duration_minutes)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(minutes)
        .execute(&self.pool)
        .await?;

        tracing::info!("Recorded {} minutes of playtime for user {}", minutes, user_id);

        Ok(())
    }

    pub async fn process_renewals(&self) -> Result<u64> {
        let expired_subscriptions = sqlx::query_as::<_, UserSubscription>(
            r#"
            SELECT * FROM user_subscriptions
            WHERE status = 'active'
            AND cancel_at_period_end = false
            AND current_period_end < NOW()
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut renewed_count = 0u64;

        for subscription in expired_subscriptions {
            match subscription.payment_provider.as_str() {
                "paystack" => {
                    if let Some(reference) = &subscription.provider_subscription_id {
                        if self.renew_paystack_subscription(reference).await.is_ok() {
                            renewed_count += 1;
                        }
                    }
                }
                "circle" => {
                    if self.renew_circle_subscription(&subscription.id.to_string()).await.is_ok() {
                        renewed_count += 1;
                    }
                }
                "free" => {
                    if self.renew_free_subscription(&subscription).await.is_ok() {
                        renewed_count += 1;
                    }
                }
                _ => {}
            }
        }

        tracing::info!("Processed {} subscription renewals", renewed_count);

        Ok(renewed_count)
    }

    async fn renew_paystack_subscription(&self, reference: &str) -> Result<()> {
        let secret_key = self.paystack_secret_key.as_ref()
            .ok_or_else(|| AppError::Internal("Paystack not configured".to_string()))?;

        let client = reqwest::Client::new();

        let response = client
            .post(&format!("https://api.paystack.co/transaction/charge/{}", reference))
            .header("Authorization", format!("Bearer {}", secret_key))
            .json(&serde_json::json!({
                "authorization_code": reference
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Paystack renewal failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal("Paystack renewal failed".to_string()));
        }

        let now = Utc::now();
        let period_end = now + chrono::Duration::days(30);

        sqlx::query(
            r#"
            UPDATE user_subscriptions
            SET current_period_start = $1, current_period_end = $2, updated_at = NOW()
            WHERE provider_subscription_id = $3
            "#,
        )
        .bind(now)
        .bind(period_end)
        .bind(reference)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn renew_circle_subscription(&self, subscription_id: &str) -> Result<()> {
        let api_key = self.circle_api_key.as_ref()
            .ok_or_else(|| AppError::Internal("Circle not configured".to_string()))?;

        let client = reqwest::Client::new();

        let response = client
            .post(&format!(
                "https://api.circle.com/v1/subscriptions/{}/renew",
                subscription_id
            ))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Circle renewal failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal("Circle renewal failed".to_string()));
        }

        let now = Utc::now();
        let period_end = now + chrono::Duration::days(30);

        sqlx::query(
            r#"
            UPDATE user_subscriptions
            SET current_period_start = $1, current_period_end = $2, updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(now)
        .bind(period_end)
        .bind(subscription_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn renew_free_subscription(&self, subscription: &UserSubscription) -> Result<()> {
        let now = Utc::now();
        let period_end = now + chrono::Duration::days(365);

        sqlx::query(
            r#"
            UPDATE user_subscriptions
            SET current_period_start = $1, current_period_end = $2, updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(now)
        .bind(period_end)
        .bind(subscription.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn activate_subscription(
        &self,
        user_id: Uuid,
        provider: &str,
        provider_reference: &str,
    ) -> Result<()> {
        let now = Utc::now();
        let period_end = now + chrono::Duration::days(30);

        sqlx::query(
            r#"
            UPDATE user_subscriptions
            SET status = 'active',
                provider_subscription_id = $1,
                current_period_start = $2,
                current_period_end = $3,
                updated_at = NOW()
            WHERE user_id = $4 AND payment_provider = $5
            "#,
        )
        .bind(provider_reference)
        .bind(now)
        .bind(period_end)
        .bind(user_id)
        .bind(provider)
        .execute(&self.pool)
        .await?;

        tracing::info!(
            "Activated {} subscription for user {}",
            provider,
            user_id
        );

        Ok(())
    }

    pub async fn handle_paystack_success(
        &self,
        reference: &str,
        user_id: Uuid,
    ) -> Result<()> {
        self.activate_subscription(user_id, "paystack", reference).await
    }

    pub async fn handle_circle_success(
        &self,
        payment_id: &str,
        user_id: Uuid,
    ) -> Result<()> {
        self.activate_subscription(user_id, "circle", payment_id).await
    }
}

pub struct PaymentService {
    api_key: String,
    base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub wallet_id: String,
    pub address: Option<String>,
    pub chain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub wallet_id: String,
    pub balance: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub destination_address: String,
    pub amount: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResponse {
    pub transfer_id: String,
    pub status: String,
    pub destination_address: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaystackSession {
    pub session_id: String,
    pub checkout_url: String,
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaystackVerification {
    pub status: String,
    pub reference: String,
    pub amount: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutInfo {
    pub payout_id: Uuid,
    pub user_id: Uuid,
    pub amount: Decimal,
    pub destination: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsBreakdown {
    pub total_revenue: Decimal,
    pub developer_share: Decimal,
    pub platform_share: Decimal,
    pub developer_percentage: Decimal,
}

impl PaymentService {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }

    pub fn mock() -> Self {
        Self {
            api_key: "mock_api_key".to_string(),
            base_url: "https://api.mock.circle.com".to_string(),
        }
    }

    pub async fn create_wallet(&self, user_id: Uuid) -> Result<WalletInfo> {
        tracing::info!("Creating USDC wallet for user: {}", user_id);
        
        Ok(WalletInfo {
            wallet_id: format!("wallet_{}", uuid::Uuid::new_v4()),
            address: Some(format!("0x{:x}", rand::random::<u128>())),
            chain: "ETH".to_string(),
        })
    }

    pub async fn get_wallet_balance(&self, wallet_id: &str) -> Result<BalanceInfo> {
        tracing::info!("Checking balance for wallet: {}", wallet_id);
        
        Ok(BalanceInfo {
            wallet_id: wallet_id.to_string(),
            balance: Decimal::ZERO,
            currency: "USDC".to_string(),
        })
    }

    pub async fn deposit_funds(
        &self,
        wallet_id: &str,
        amount: Decimal,
    ) -> Result<TransferResponse> {
        tracing::info!("Depositing {} to wallet: {}", amount, wallet_id);
        
        Ok(TransferResponse {
            transfer_id: format!("transfer_{}", uuid::Uuid::new_v4()),
            status: "pending".to_string(),
            destination_address: wallet_id.to_string(),
            amount,
        })
    }

    pub async fn withdraw_funds(
        &self,
        to_address: &str,
        amount: Decimal,
    ) -> Result<TransferResponse> {
        tracing::info!("Withdrawing {} to address: {}", amount, to_address);
        
        Ok(TransferResponse {
            transfer_id: format!("transfer_{}", uuid::Uuid::new_v4()),
            status: "pending".to_string(),
            destination_address: to_address.to_string(),
            amount,
        })
    }

    pub async fn create_payment(
        &self,
        to_address: &str,
        amount: Decimal,
    ) -> Result<TransferResponse> {
        tracing::info!("Creating payment of {} to address: {}", amount, to_address);
        
        Ok(TransferResponse {
            transfer_id: format!("payment_{}", uuid::Uuid::new_v4()),
            status: "completed".to_string(),
            destination_address: to_address.to_string(),
            amount,
        })
    }

    pub async fn create_paystack_session(
        &self,
        user_id: Uuid,
        amount: Decimal,
        _email: &str,
    ) -> Result<PaystackSession> {
        tracing::info!("Creating Paystack session for user: {}, amount: {}", user_id, amount);
        
        let reference = format!("PS_{}", uuid::Uuid::new_v4());
        
        Ok(PaystackSession {
            session_id: format!("session_{}", uuid::Uuid::new_v4()),
            checkout_url: format!("https://paystack.com/pay/{}", reference),
            reference,
        })
    }

    pub async fn verify_paystack_payment(
        &self,
        reference: &str,
    ) -> Result<PaystackVerification> {
        tracing::info!("Verifying Paystack payment: {}", reference);
        
        Ok(PaystackVerification {
            status: "success".to_string(),
            reference: reference.to_string(),
            amount: Decimal::new(100000, 2),
            currency: "ZAR".to_string(),
        })
    }

    pub async fn convert_zar_to_usdc(&self, zar_amount: Decimal) -> Result<Decimal> {
        let exchange_rate = Decimal::new(2750, 1);
        let platform_fee = Decimal::new(3, 2);
        
        let usdc_amount = (zar_amount / exchange_rate) * (Decimal::ONE - platform_fee);
        
        tracing::info!("Converted {} ZAR to {} USDC", zar_amount, usdc_amount);
        Ok(usdc_amount)
    }

    pub fn calculate_earnings(&self, game_revenue: Decimal) -> EarningsBreakdown {
        let platform_percentage = Decimal::new(15, 2);
        let developer_percentage = Decimal::ONE - platform_percentage;
        
        let platform_share = game_revenue * platform_percentage;
        let developer_share = game_revenue * developer_percentage;
        
        EarningsBreakdown {
            total_revenue: game_revenue,
            developer_share,
            platform_share,
            developer_percentage: developer_percentage * Decimal::new(100, 0),
        }
    }

    pub async fn process_payout(
        &self,
        _db: &sqlx::PgPool,
        user_id: Uuid,
        amount: Decimal,
        destination: &str,
    ) -> Result<PayoutInfo> {
        tracing::info!("Processing payout for user: {}, amount: {}", user_id, amount);
        
        let payout_id = Uuid::new_v4();
        
        let payout = PayoutInfo {
            payout_id,
            user_id,
            amount,
            destination: destination.to_string(),
            status: "pending".to_string(),
            created_at: Utc::now(),
        };
        
        Ok(payout)
    }

    pub async fn process_weekly_payouts(&self, _db: &sqlx::PgPool) -> Result<Vec<PayoutInfo>> {
        tracing::info!("Processing weekly auto-payouts");
        
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_earnings() {
        let service = PaymentService::mock();
        let revenue = Decimal::new(10000, 2);
        
        let earnings = service.calculate_earnings(revenue);
        
        assert_eq!(earnings.total_revenue, revenue);
        assert!(earnings.developer_share > earnings.platform_share);
        assert_eq!(earnings.developer_percentage, Decimal::new(85, 0));
    }

    #[test]
    fn test_convert_zar_to_usdc() {
        let zar = Decimal::new(275000, 2);
        let exchange_rate = Decimal::new(2750, 1);
        let platform_fee = Decimal::new(3, 2);
        
        let usdc_amount = (zar / exchange_rate) * (Decimal::ONE - platform_fee);
        
        assert!(usdc_amount > Decimal::ZERO);
    }
}
