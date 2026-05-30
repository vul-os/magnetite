// Payout service — developer earnings distribution via Circle USDC disbursement.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::notifications::{broadcast_notification, Notification, NotificationType};
use crate::error::{AppError, Result};
use crate::services::auth::get_user_by_id;
use crate::services::email::EmailService;

// Fee percentages are expressed as fractions (e.g. 0.30 = 30%), NOT as integer basis points.
// Do NOT add a /100 divisor when multiplying against revenue — these are already in [0,1].
fn platform_fee_percent() -> Decimal {
    Decimal::new(30, 2) // 0.30 = 30%
}

fn developer_share_percent() -> Decimal {
    Decimal::new(70, 2) // 0.70 = 70%
}

fn minimum_payout_amount() -> Decimal {
    Decimal::new(2500, 2)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Period {
    Weekly,
    Monthly,
}

impl Period {
    fn interval_string(&self) -> &'static str {
        match self {
            Period::Weekly => "1 week",
            Period::Monthly => "1 month",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsSummary {
    pub total_revenue: Decimal,
    pub platform_fee: Decimal,
    pub developer_share: Decimal,
    pub previous_balance: Decimal,
    pub available_for_payout: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payout {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: Decimal,
    pub destination: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutRequest {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: Decimal,
    pub destination: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

pub struct PayoutService {
    pool: PgPool,
}

impl PayoutService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn calculate_earnings(
        &self,
        user_id: Uuid,
        period: &Period,
    ) -> Result<EarningsSummary> {
        let period_start = match period {
            Period::Weekly => Utc::now() - chrono::Duration::days(7),
            Period::Monthly => Utc::now() - chrono::Duration::days(30),
        };

        let total_revenue = sqlx::query_as::<_, (Decimal,)>(
            r#"
            SELECT COALESCE(SUM(amount), 0)
            FROM developer_earnings
            WHERE user_id = $1
              AND created_at >= $2
            "#,
        )
        .bind(user_id)
        .bind(period_start)
        .fetch_one(&self.pool)
        .await?
        .0;

        // platform_fee_percent() and developer_share_percent() are already fractional (0.30/0.70).
        // Do NOT divide by 100 again — that would give 0.3% and 0.7% instead of 30% and 70%.
        let platform_fee = total_revenue * platform_fee_percent();
        let developer_share = total_revenue * developer_share_percent();

        let previous_paid = sqlx::query_as::<_, (Decimal,)>(
            r#"
            SELECT COALESCE(SUM(amount), 0)
            FROM payouts
            WHERE user_id = $1
              AND status = 'completed'
              AND created_at >= $2
            "#,
        )
        .bind(user_id)
        .bind(period_start)
        .fetch_one(&self.pool)
        .await?
        .0;

        let previous_balance = sqlx::query_as::<_, (Decimal,)>(
            r#"
            SELECT COALESCE(balance, 0)
            FROM developer_balances
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.0)
        .unwrap_or(Decimal::ZERO);

        let available_for_payout = developer_share + previous_balance - previous_paid;

        Ok(EarningsSummary {
            total_revenue,
            platform_fee,
            developer_share,
            previous_balance,
            available_for_payout,
        })
    }

    pub async fn request_payout(
        &self,
        user_id: Uuid,
        amount: Decimal,
        destination: &str,
    ) -> Result<PayoutRequest> {
        if amount < minimum_payout_amount() {
            return Err(AppError::Validation(format!(
                "Minimum payout amount is {} USDC",
                minimum_payout_amount()
            )));
        }

        let balance = sqlx::query_as::<_, (Decimal,)>(
            "SELECT COALESCE(balance, 0) FROM developer_balances WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.0)
        .unwrap_or(Decimal::ZERO);

        if balance < amount {
            return Err(AppError::InsufficientFunds(format!(
                "Insufficient balance. Available: {}, Requested: {}",
                balance, amount
            )));
        }

        let existing_pending = sqlx::query_as::<_, (Uuid,)>(
            r#"
            SELECT id FROM payouts
            WHERE user_id = $1 AND status = 'pending'
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        if existing_pending.is_some() {
            return Err(AppError::Validation(
                "A pending payout request already exists".to_string(),
            ));
        }

        let payout_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO payouts (id, user_id, amount, destination, status, created_at)
            VALUES ($1, $2, $3, $4, 'pending', NOW())
            "#,
        )
        .bind(payout_id)
        .bind(user_id)
        .bind(amount)
        .bind(destination)
        .execute(&self.pool)
        .await?;

        sqlx::query("UPDATE developer_balances SET balance = balance - $1 WHERE user_id = $2")
            .bind(amount)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(PayoutRequest {
            id: payout_id,
            user_id,
            amount,
            destination: destination.to_string(),
            status: "pending".to_string(),
            created_at: Utc::now(),
        })
    }

    pub async fn process_pending_payouts(&self) -> Result<u64> {
        let pending_payouts = sqlx::query_as::<_, Payout>(
            r#"
            SELECT id, user_id, amount, destination, status, created_at, processed_at
            FROM payouts
            WHERE status = 'pending'
            ORDER BY created_at ASC
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut processed_count = 0;

        for payout in pending_payouts {
            let result = self.process_single_payout(&payout).await;
            if result.is_ok() {
                processed_count += 1;
            }
        }

        Ok(processed_count)
    }

    async fn process_single_payout(&self, payout: &Payout) -> Result<()> {
        tracing::info!(
            "Processing payout {} for user {} (amount={} USDC, dest={})",
            payout.id,
            payout.user_id,
            payout.amount,
            payout.destination
        );

        // Check for sandbox mode first.
        let sandbox = std::env::var("PAYMENTS_SANDBOX")
            .map(|v| v == "true")
            .unwrap_or(false);

        let circle_api_key = std::env::var("CIRCLE_API_KEY").ok();

        let transfer_id = if sandbox {
            // Sandbox: return a clearly-labeled result without calling Circle.
            tracing::info!(
                "[SANDBOX] Skipping real Circle disbursement for payout {}",
                payout.id
            );
            format!("sandbox_transfer_{}", Uuid::new_v4())
        } else {
            // Production: require CIRCLE_API_KEY; return ProviderUnconfigured if absent.
            let api_key = circle_api_key.ok_or_else(|| {
                AppError::Internal(
                    "payments not configured: CIRCLE_API_KEY is unset (set PAYMENTS_SANDBOX=true for local dev)"
                        .to_string(),
                )
            })?;

            // Call Circle /v1/transfers to disburse USDC to the developer's destination address.
            let idempotency_key = format!("payout-{}", payout.id);
            let client = reqwest::Client::new();
            let resp = client
                .post("https://api.circle.com/v1/transfers")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "destination": {
                        "type": "blockchain",
                        "address": payout.destination,
                        "chain": "ETH"
                    },
                    "amount": {
                        "amount": payout.amount.to_string(),
                        "currency": "USDC"
                    }
                }))
                .send()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Circle disbursement request failed: {}", e))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::error!(
                    "Circle disbursement failed for payout {}: HTTP {} — {}",
                    payout.id,
                    status,
                    body
                );
                // Mark the payout as failed so it isn't retried endlessly.
                let _ = sqlx::query(
                    "UPDATE payouts SET status = 'failed', processed_at = NOW() WHERE id = $1",
                )
                .bind(payout.id)
                .execute(&self.pool)
                .await;
                return Err(AppError::Internal(format!(
                    "Circle disbursement failed (HTTP {}): {}",
                    status, body
                )));
            }

            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| AppError::Internal(format!("Circle response parse error: {}", e)))?;

            body["data"]["id"].as_str().unwrap_or("unknown").to_string()
        };

        // Mark completed with the provider transfer id stored in the reference column.
        sqlx::query(
            r#"
            UPDATE payouts
            SET status = 'completed', processed_at = NOW(), destination = $2
            WHERE id = $1
            "#,
        )
        .bind(payout.id)
        .bind(&transfer_id)
        .execute(&self.pool)
        .await?;

        let notification = sqlx::query_as::<_, Notification>(
            "INSERT INTO notifications (id, user_id, type, title, body, data, read, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, false, NOW())
             RETURNING id, user_id, type, title, body, data, read, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(payout.user_id)
        .bind(NotificationType::PayoutComplete.as_str())
        .bind("Payout Complete")
        .bind(format!(
            "Your payout of {} USDC has been processed (transfer: {})",
            payout.amount, transfer_id
        ))
        .bind(serde_json::json!({ "payout_id": payout.id, "transfer_id": transfer_id }))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        broadcast_notification(notification).await;

        // Send transactional email — non-fatal: log on failure, do not roll back the payout.
        match EmailService::from_env() {
            Ok(email_svc) => match get_user_by_id(&self.pool, payout.user_id).await {
                Ok(Some(user)) => {
                    let amount_str = payout.amount.to_string();
                    if let Err(e) = email_svc
                        .send_payout_complete_email(
                            &user.email,
                            &user.username,
                            &amount_str,
                            &payout.destination,
                            &transfer_id,
                        )
                        .await
                    {
                        tracing::warn!(
                            payout_id = %payout.id,
                            user_id = %payout.user_id,
                            "Failed to send payout-complete email (non-fatal): {}",
                            e
                        );
                    }
                }
                Ok(None) => {
                    tracing::warn!(
                        payout_id = %payout.id,
                        user_id = %payout.user_id,
                        "Payout-complete email skipped: user not found"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        payout_id = %payout.id,
                        user_id = %payout.user_id,
                        "Payout-complete email skipped: user lookup failed: {}",
                        e
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    payout_id = %payout.id,
                    "Payout-complete email skipped: email service not configured: {}",
                    e
                );
            }
        }

        Ok(())
    }

    pub async fn get_payout_history(&self, user_id: Uuid) -> Result<Vec<Payout>> {
        let payouts = sqlx::query_as::<_, Payout>(
            r#"
            SELECT id, user_id, amount, destination, status, created_at, processed_at
            FROM payouts
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT 100
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(payouts)
    }

    pub async fn get_pending_payout(&self, user_id: Uuid) -> Result<Option<PayoutRequest>> {
        let payout = sqlx::query_as::<_, Payout>(
            r#"
            SELECT id, user_id, amount, destination, status, created_at, processed_at
            FROM payouts
            WHERE user_id = $1 AND status = 'pending'
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(payout.map(|p| PayoutRequest {
            id: p.id,
            user_id: p.user_id,
            amount: p.amount,
            destination: p.destination,
            status: p.status,
            created_at: p.created_at,
        }))
    }

    pub async fn cancel_payout(&self, payout_id: Uuid) -> Result<()> {
        let payout = sqlx::query_as::<_, Payout>(
            r#"
            SELECT id, user_id, amount, destination, status, created_at, processed_at
            FROM payouts
            WHERE id = $1 AND status = 'pending'
            FOR UPDATE
            "#,
        )
        .bind(payout_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Payout not found or already processed".to_string()))?;

        let mut tx = self.pool.begin().await?;

        sqlx::query("UPDATE developer_balances SET balance = balance + $1 WHERE user_id = $2")
            .bind(payout.amount)
            .bind(payout.user_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"
            UPDATE payouts SET status = 'cancelled' WHERE id = $1
            "#,
        )
        .bind(payout_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_fee_calculation() {
        // platform_fee_percent() = 0.30, developer_share_percent() = 0.70 (already fractional).
        // Multiply directly against revenue — no additional /100.
        let revenue = Decimal::new(10000, 2); // 100.00 USDC
        let fee = revenue * platform_fee_percent();
        let share = revenue * developer_share_percent();
        // 30% of 100.00 = 30.00; 70% of 100.00 = 70.00
        assert_eq!(
            fee,
            Decimal::new(3000, 2),
            "platform fee must be 30.00 USDC (30%)"
        );
        assert_eq!(
            share,
            Decimal::new(7000, 2),
            "developer share must be 70.00 USDC (70%)"
        );
    }

    #[test]
    fn test_minimum_payout_amount() {
        let amount = Decimal::new(2000, 2);
        assert!(amount < minimum_payout_amount());
    }

    #[test]
    fn test_period_interval() {
        assert_eq!(Period::Weekly.interval_string(), "1 week");
        assert_eq!(Period::Monthly.interval_string(), "1 month");
    }
}
