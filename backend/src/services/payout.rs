// Payout service — developer earnings distribution, platform surface, not yet wired.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::notifications::{broadcast_notification, Notification, NotificationType};
use crate::error::{AppError, Result};

fn platform_fee_percent() -> Decimal {
    Decimal::new(30, 2)
}

fn developer_share_percent() -> Decimal {
    Decimal::new(70, 2)
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

        let platform_fee = total_revenue * platform_fee_percent() / Decimal::new(100, 0);
        let developer_share = total_revenue * developer_share_percent() / Decimal::new(100, 0);

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
            "Processing payout {} for user {}",
            payout.id,
            payout.user_id
        );

        sqlx::query(
            r#"
            UPDATE payouts
            SET status = 'completed', processed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(payout.id)
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
            "Your payout of {} USDC has been processed",
            payout.amount
        ))
        .bind(serde_json::json!({ "payout_id": payout.id }))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        broadcast_notification(notification).await;

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
        let revenue = Decimal::new(10000, 2);
        let fee = revenue * platform_fee_percent() / Decimal::new(100, 0);
        let share = revenue * developer_share_percent() / Decimal::new(100, 0);
        assert_eq!(fee, Decimal::new(3000, 2));
        assert_eq!(share, Decimal::new(7000, 2));
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
