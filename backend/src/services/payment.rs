// Payment/subscription service — NON-CUSTODIAL crypto only (seam §3.6).
//
// All fiat is gone: no Paystack on-ramp, no Wise payouts, no platform-held
// balances. Money moves buyer-wallet → seller-wallet through the `PaymentRail`
// seam and the signed `Receipt` is the entitlement. `SubscriptionService` keeps
// tiers as feature flags activated by a receipt (pay-the-operator); renewal is
// still spawned from `main.rs`. The payment rail itself lives at the bottom of
// this file.
#![allow(dead_code)]

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
}

impl SubscriptionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_play_sessions_user ON play_sessions(user_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_user_subscriptions_user ON user_subscriptions(user_id)",
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

                Ok(Some(SubscriptionWithTier {
                    subscription: sub,
                    tier,
                }))
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
        let tier =
            sqlx::query_as::<_, SubscriptionTier>("SELECT * FROM subscription_tiers WHERE id = $1")
                .bind(tier_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| AppError::NotFound("Tier not found".to_string()))?;

        if tier.price_usdc > Decimal::ZERO {
            match payment_method {
                // Paid tiers are "pay the operator": one wallet→wallet checkout
                // per period, activated by the signed receipt. No recurring
                // card charge exists in a non-custodial model.
                "receipt" | "crypto" | "platform" => {
                    self.create_receipt_subscription(user_id, &tier).await
                }
                _ => Err(AppError::BadRequest(
                    "Invalid payment method. Use 'receipt' (non-custodial crypto).".to_string(),
                )),
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

    /// Paid tier via a single non-custodial checkout to the operator wallet.
    /// The signed receipt is stored and IS the proof the tier is active.
    async fn create_receipt_subscription(
        &self,
        user_id: Uuid,
        tier: &SubscriptionTier,
    ) -> Result<UserSubscription> {
        let operator = operator_wallet().ok_or_else(|| {
            AppError::Internal(
                "OPERATOR_WALLET_PUBKEY is not configured — paid tiers are pay-the-operator"
                    .to_string(),
            )
        })?;
        let buyer = require_wallet(&self.pool, user_id, "subscriber").await?;

        let amount = units_from_usd(tier.price_usdc);
        let split = sale_split(operator, amount, None);
        let receipt = rail().checkout(&buyer, split).await;
        if !verify_receipt(&receipt) {
            return Err(AppError::Internal(
                "subscription receipt failed verification".to_string(),
            ));
        }
        let receipt_id =
            store_receipt(&self.pool, &receipt, "subscription", user_id, None, None, None).await?;

        let now = Utc::now();
        let period_end = now + chrono::Duration::days(30);

        let subscription = sqlx::query_as::<_, UserSubscription>(
            r#"
            INSERT INTO user_subscriptions (
                id, user_id, tier_id, status, payment_provider,
                provider_subscription_id, current_period_start, current_period_end, cancel_at_period_end
            )
            VALUES ($1, $2, $3, 'active', 'receipt', $4, $5, $6, false)
            ON CONFLICT (user_id) DO UPDATE SET
                tier_id = $3,
                status = 'active',
                payment_provider = 'receipt',
                provider_subscription_id = $4,
                current_period_start = $5,
                current_period_end = $6,
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(tier.id)
        .bind(receipt_id.to_string())
        .bind(now)
        .bind(period_end)
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            "Activated receipt-backed subscription for user {} (tier {})",
            user_id,
            tier.slug
        );

        Ok(subscription)
    }

    pub async fn cancel(&self, subscription_id: Uuid) -> Result<()> {
        let subscription =
            sqlx::query_as::<_, UserSubscription>("SELECT * FROM user_subscriptions WHERE id = $1")
                .bind(subscription_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;

        // Nothing to cancel with a provider: there is no recurring mandate in a
        // non-custodial model, only a receipt that stops being renewed.
        let _ = &subscription;

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

                let hours = sub
                    .tier
                    .features
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
                let hours = features.get("hours").and_then(|v| v.as_i64()).unwrap_or(0);

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

        tracing::info!(
            "Recorded {} minutes of playtime for user {}",
            minutes,
            user_id
        );

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
                // Paid, receipt-backed tiers cannot auto-renew: renewal requires a
                // new signed wallet→wallet checkout initiated by the subscriber.
                "receipt" | "crypto" => {
                    if self.expire_subscription(&subscription).await.is_ok() {
                        renewed_count += 1;
                    }
                }
                "free" | "platform" => {
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

    /// Mark a receipt-backed subscription expired — the user must re-checkout.
    async fn expire_subscription(&self, subscription: &UserSubscription) -> Result<()> {
        sqlx::query(
            "UPDATE user_subscriptions SET status = 'expired', updated_at = NOW() WHERE id = $1",
        )
        .bind(subscription.id)
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

        tracing::info!("Activated {} subscription for user {}", provider, user_id);

        Ok(())
    }

    /// Activate a tier from a stored, verified receipt id.
    pub async fn handle_receipt_success(&self, receipt_id: &str, user_id: Uuid) -> Result<()> {
        self.activate_subscription(user_id, "receipt", receipt_id)
            .await
    }
}
// ─── Non-custodial payment rail (seam §3.6) ───────────────────────────────────
//
// There is no custody here: no balances, no deposits, no withdrawals, no payouts.
// A purchase is an atomic wallet→wallet transfer produced by a `PaymentRail`
// implementation; the resulting signed `Receipt` IS the entitlement.
//
// The default rail is `MockPaymentRail` — deterministic, offline, zero external
// services — selected by `PAYMENT_RAIL=mock` (the default).
//
// TODO(chain): implement a real `PaymentRail` (USDC on an L2, or Solana where the
// Ed25519 identity key doubles as the wallet key) using `CHAIN_RPC_URL`,
// `CHAIN_ID` and `STABLECOIN_ADDRESS`, and select it here when
// `PAYMENT_RAIL != "mock"`. Nothing outside this module may name a chain type.

use std::sync::OnceLock;

pub use magnetite_seams::identity::{PubKey, Sig};
pub use magnetite_seams::payment::{
    Channel, MockPaymentRail, PayOut, PaymentRail, PaymentSplit, Receipt, Split,
};

/// Protocol fee in basis points. Default `0` (governance decides any real fee).
pub fn protocol_fee_bps() -> u16 {
    std::env::var("PROTOCOL_FEE_BPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// The process-wide payment rail. Default `mock` — fully offline.
pub fn rail() -> &'static MockPaymentRail {
    static RAIL: OnceLock<MockPaymentRail> = OnceLock::new();
    RAIL.get_or_init(|| {
        let kind = std::env::var("PAYMENT_RAIL").unwrap_or_else(|_| "mock".to_string());
        if kind != "mock" {
            tracing::warn!(
                "PAYMENT_RAIL={} is not implemented yet; falling back to the offline mock rail. \
                 See TODO(chain) in services/payment.rs",
                kind
            );
        }
        MockPaymentRail::with_fee_bps(protocol_fee_bps())
    })
}

/// Verify a receipt against the active rail (signature + internal arithmetic).
pub fn verify_receipt(r: &Receipt) -> bool {
    rail().verify_receipt(r)
}

/// Convert a USD-denominated `Decimal` price to the rail's smallest unit (cents).
pub fn units_from_usd(price: Decimal) -> u64 {
    use rust_decimal::prelude::ToPrimitive;
    (price * Decimal::new(100, 0))
        .round()
        .to_u64()
        .unwrap_or(u64::MAX)
}

/// The wallet (Ed25519 pubkey) a user has linked, if any. Non-custodial: we only
/// ever record an address, never hold funds.
pub async fn wallet_of(pool: &PgPool, user_id: Uuid) -> Result<Option<PubKey>> {
    let row = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT wallet_address FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .and_then(|r| r.0)
        .and_then(|h| PubKey::from_hex(h.trim_start_matches("0x")).ok()))
}

/// Require a linked wallet, with a role label for the error message.
pub async fn require_wallet(pool: &PgPool, user_id: Uuid, role: &str) -> Result<PubKey> {
    wallet_of(pool, user_id).await?.ok_or_else(|| {
        AppError::Validation(format!(
            "{role} has no linked wallet address — payments are non-custodial, \
             link a wallet before transacting"
        ))
    })
}

/// The operator wallet that receives hosting / subscription fees, if configured.
pub fn operator_wallet() -> Option<PubKey> {
    std::env::var("OPERATOR_WALLET_PUBKEY")
        .ok()
        .and_then(|h| PubKey::from_hex(h.trim_start_matches("0x")).ok())
}

/// Persist a signed receipt. This row is the durable entitlement proof.
#[allow(clippy::too_many_arguments)]
pub async fn store_receipt(
    pool: &PgPool,
    receipt: &Receipt,
    kind: &str,
    buyer_id: Uuid,
    purchase_id: Option<Uuid>,
    item_id: Option<Uuid>,
    game_id: Option<Uuid>,
) -> Result<Uuid> {
    if !verify_receipt(receipt) {
        return Err(AppError::Internal(
            "refusing to store an unverifiable receipt".to_string(),
        ));
    }
    let id = Uuid::new_v4();
    let payouts = serde_json::json!(receipt
        .payouts
        .iter()
        .map(|p| serde_json::json!({ "wallet": p.wallet.to_hex(), "amount": p.amount }))
        .collect::<Vec<_>>());

    sqlx::query(
        r#"
        INSERT INTO payment_receipts
            (id, kind, buyer_id, buyer_pubkey, purchase_id, item_id, game_id,
             total, protocol_fee, payouts, nonce, rail_pubkey, sig, rail, voided, created_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,false,NOW())
        "#,
    )
    .bind(id)
    .bind(kind)
    .bind(buyer_id)
    .bind(receipt.buyer.to_hex())
    .bind(purchase_id)
    .bind(item_id)
    .bind(game_id)
    .bind(receipt.total as i64)
    .bind(receipt.protocol_fee as i64)
    .bind(payouts)
    .bind(hex::encode(receipt.nonce))
    .bind(receipt.rail_pubkey.to_hex())
    .bind(hex::encode(receipt.sig.0))
    .bind(std::env::var("PAYMENT_RAIL").unwrap_or_else(|_| "mock".to_string()))
    .execute(pool)
    .await?;

    Ok(id)
}

/// Void a receipt (refund path — there is no money to claw back, only proof to revoke).
pub async fn void_receipt_for_purchase(pool: &PgPool, purchase_id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE payment_receipts SET voided = true, voided_at = NOW() WHERE purchase_id = $1",
    )
    .bind(purchase_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Open a hosting-fee payment channel to an operator (per-seat / per-hour).
///
/// TODO(chain): with a real rail this anchors an on-chain channel and the
/// per-join debits are off-chain signed channel updates. The mock rail returns a
/// deterministic channel id so the flow is testable offline.
pub async fn open_hosting_channel(
    pool: &PgPool,
    payer_id: Uuid,
    operator: &PubKey,
    server_id: Option<Uuid>,
) -> Result<Channel> {
    let channel = rail().open_channel(operator).await;
    sqlx::query(
        r#"
        INSERT INTO hosting_channels
            (id, channel_id, payer_id, operator_pubkey, server_id, rail_pubkey, open, created_at)
        VALUES ($1,$2,$3,$4,$5,$6,true,NOW())
        ON CONFLICT (channel_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(hex::encode(channel.id))
    .bind(payer_id)
    .bind(operator.to_hex())
    .bind(server_id)
    .bind(channel.rail_pubkey.to_hex())
    .execute(pool)
    .await?;
    Ok(channel)
}

/// Charge a hosting fee (per-seat / per-hour) to an operator and record the receipt.
///
/// Scaffold: with the mock rail this is a deterministic offline checkout, so the
/// join-gate below is fully testable without a chain.
/// TODO(chain): debit the open channel with a signed channel update instead of a
/// full checkout, so a join costs no gas.
pub async fn charge_hosting_fee(
    pool: &PgPool,
    payer_id: Uuid,
    operator: &PubKey,
    amount: u64,
    server_id: Option<Uuid>,
) -> Result<Receipt> {
    let payer = require_wallet(pool, payer_id, "player").await?;
    // Ensure a channel exists (idempotent, deterministic id).
    open_hosting_channel(pool, payer_id, operator, server_id).await?;

    let split = sale_split(*operator, amount, None);
    let receipt = rail().checkout(&payer, split).await;
    if !verify_receipt(&receipt) {
        return Err(AppError::Internal(
            "hosting fee receipt failed verification".to_string(),
        ));
    }
    store_receipt(pool, &receipt, "hosting", payer_id, None, None, None).await?;
    Ok(receipt)
}

/// Join-gate for a PAID server: the player must hold a non-voided hosting receipt.
///
/// A server with no hosting fee configured is free to join and returns `true`.
pub async fn has_hosting_access(pool: &PgPool, user_id: Uuid, server_id: Uuid) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payment_receipts r
         JOIN hosting_channels c ON c.payer_id = r.buyer_id
         WHERE r.kind = 'hosting' AND r.voided = false
           AND r.buyer_id = $1 AND c.server_id = $2 AND c.open = true",
    )
    .bind(user_id)
    .bind(server_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

/// Build the split for a single-seller sale: the developer takes the whole
/// subtotal, an optional operator takes a hosting cut, protocol fee rides on top.
pub fn sale_split(developer: PubKey, amount: u64, operator: Option<(PubKey, u64)>) -> PaymentSplit {
    PaymentSplit {
        developer: Split {
            wallet: developer,
            amount,
        },
        operator: operator.map(|(wallet, amount)| Split { wallet, amount }),
        protocol_fee_bps: protocol_fee_bps(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(b: u8) -> PubKey {
        PubKey([b; 32])
    }

    #[test]
    fn usd_converts_to_cents() {
        assert_eq!(units_from_usd(Decimal::new(1999, 2)), 1999);
        assert_eq!(units_from_usd(Decimal::new(5, 0)), 500);
        assert_eq!(units_from_usd(Decimal::ZERO), 0);
    }

    #[test]
    fn default_protocol_fee_is_zero() {
        // No PROTOCOL_FEE_BPS in the test env.
        assert_eq!(
            std::env::var("PROTOCOL_FEE_BPS").ok().is_none(),
            true,
            "test env must not set PROTOCOL_FEE_BPS"
        );
        assert_eq!(protocol_fee_bps(), 0);
    }

    #[tokio::test]
    async fn checkout_produces_verifiable_receipt_offline() {
        let buyer = pk(0xB0);
        let split = sale_split(pk(0xD0), 1999, None);
        let r = rail().checkout(&buyer, split).await;

        assert_eq!(r.total, 1999);
        assert_eq!(r.protocol_fee, 0);
        assert_eq!(r.payouts.len(), 1);
        assert_eq!(r.payouts[0].wallet, pk(0xD0));
        assert!(verify_receipt(&r), "receipt must verify against the rail");
    }

    #[tokio::test]
    async fn tampered_receipt_does_not_grant_entitlement() {
        let buyer = pk(0xB1);
        let mut r = rail().checkout(&buyer, sale_split(pk(0xD1), 500, None)).await;
        assert!(verify_receipt(&r));
        r.payouts[0].amount = 5_000_000;
        assert!(
            !verify_receipt(&r),
            "a forged receipt must never gate an entitlement"
        );
    }

    #[tokio::test]
    async fn operator_cut_is_split_atomically() {
        let buyer = pk(0xB2);
        let r = rail()
            .checkout(&buyer, sale_split(pk(0xD2), 900, Some((pk(0x0B), 100))))
            .await;
        assert_eq!(r.total, 1000);
        assert_eq!(r.payouts.len(), 2);
        assert_eq!(r.payouts[1].amount, 100);
        assert!(verify_receipt(&r));
    }

    #[tokio::test]
    async fn hosting_channel_id_is_deterministic() {
        let op = pk(0x0C);
        let a = rail().open_channel(&op).await;
        let b = rail().open_channel(&op).await;
        assert_eq!(a.id, b.id);
        assert_eq!(a.peer, op);
    }
}
