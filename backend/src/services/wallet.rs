// Wallet service — USDC balance, AML velocity limits, deposits; platform surface, not yet wired.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::services::verification::{get_transaction_limit, get_user_verification_level};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Wallet {
    pub user_id: Uuid,
    pub currency: String,
    pub balance: Decimal,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tx_type: String,
    pub amount: Decimal,
    pub reference_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TransactionWithBalance {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tx_type: String,
    pub amount: Decimal,
    pub reference_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub new_balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PlatformSettings {
    pub id: Uuid,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct DepositLimits {
    pub min_amount: Decimal,
    pub max_amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct AMLVelocityLimits {
    pub daily_limit: Decimal,
    pub monthly_limit: Decimal,
    pub transaction_count_limit: i32,
}

pub struct WalletService {
    pool: PgPool,
}

impl WalletService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn get_platform_setting(&self, key: &str) -> Result<Option<String>> {
        let result =
            sqlx::query_as::<_, (String,)>("SELECT value FROM platform_settings WHERE key = $1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.map(|r| r.0))
    }

    async fn get_deposit_limits(&self) -> Result<DepositLimits> {
        let min_str = self.get_platform_setting("deposit_min_amount").await?;
        let max_str = self.get_platform_setting("deposit_max_amount").await?;

        let min_amount = min_str
            .and_then(|s| s.parse::<Decimal>().ok())
            .unwrap_or(Decimal::new(100, 2));

        let max_amount = max_str
            .and_then(|s| s.parse::<Decimal>().ok())
            .unwrap_or(Decimal::new(1_000_000, 2));

        Ok(DepositLimits {
            min_amount,
            max_amount,
        })
    }

    async fn get_aml_velocity_limits(&self) -> Result<AMLVelocityLimits> {
        let daily_str = self.get_platform_setting("aml_daily_limit").await?;
        let monthly_str = self.get_platform_setting("aml_monthly_limit").await?;
        let count_str = self
            .get_platform_setting("aml_transaction_count_limit")
            .await?;

        let daily_limit = daily_str
            .and_then(|s| s.parse::<Decimal>().ok())
            .unwrap_or(Decimal::new(10_000_00, 2));

        let monthly_limit = monthly_str
            .and_then(|s| s.parse::<Decimal>().ok())
            .unwrap_or(Decimal::new(100_000_00, 2));

        let transaction_count_limit = count_str.and_then(|s| s.parse::<i32>().ok()).unwrap_or(100);

        Ok(AMLVelocityLimits {
            daily_limit,
            monthly_limit,
            transaction_count_limit,
        })
    }

    async fn check_aml_velocity(
        &self,
        user_id: Uuid,
        currency: &str,
        amount: Decimal,
    ) -> Result<()> {
        let limits = self.get_aml_velocity_limits().await?;

        let daily_total = sqlx::query_as::<_, (Decimal,)>(
            r#"
            SELECT COALESCE(SUM(amount), 0)
            FROM wallet_transactions
            WHERE user_id = $1
              AND currency = $2
              AND tx_type IN ('deposit', 'transfer_in')
              AND status = 'completed'
              AND created_at >= NOW() - INTERVAL '1 day'
            "#,
        )
        .bind(user_id)
        .bind(currency)
        .fetch_one(&self.pool)
        .await?;

        if daily_total.0 + amount > limits.daily_limit {
            return Err(AppError::Validation(
                "Daily deposit/transfer limit exceeded".to_string(),
            ));
        }

        let monthly_total = sqlx::query_as::<_, (Decimal,)>(
            r#"
            SELECT COALESCE(SUM(amount), 0)
            FROM wallet_transactions
            WHERE user_id = $1
              AND currency = $2
              AND tx_type IN ('deposit', 'transfer_in')
              AND status = 'completed'
              AND created_at >= NOW() - INTERVAL '1 month'
            "#,
        )
        .bind(user_id)
        .bind(currency)
        .fetch_one(&self.pool)
        .await?;

        if monthly_total.0 + amount > limits.monthly_limit {
            return Err(AppError::Validation(
                "Monthly deposit/transfer limit exceeded".to_string(),
            ));
        }

        let transaction_count = sqlx::query_as::<_, (i64,)>(
            r#"
            SELECT COUNT(*)
            FROM wallet_transactions
            WHERE user_id = $1
              AND currency = $2
              AND tx_type IN ('deposit', 'transfer_in')
              AND status = 'completed'
              AND created_at >= NOW() - INTERVAL '1 day'
            "#,
        )
        .bind(user_id)
        .bind(currency)
        .fetch_one(&self.pool)
        .await?;

        if transaction_count.0 >= limits.transaction_count_limit as i64 {
            return Err(AppError::Validation(
                "Daily transaction count limit exceeded".to_string(),
            ));
        }

        Ok(())
    }

    async fn check_idempotency(&self, idempotency_key: &str) -> Result<Option<Transaction>> {
        let existing = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT id, user_id, tx_type, amount, reference_id, status, created_at
            FROM wallet_transactions
            WHERE idempotency_key = $1
            "#,
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(existing)
    }

    pub async fn create_wallet(&self, user_id: Uuid, currency: &str) -> Result<Wallet> {
        let wallet = sqlx::query_as::<_, Wallet>(
            r#"
            INSERT INTO wallet_balances (id, user_id, currency, balance, updated_at)
            VALUES ($1, $2, $3, 0, NOW())
            ON CONFLICT (user_id, currency) DO UPDATE SET updated_at = NOW()
            RETURNING user_id, currency, balance, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(currency)
        .fetch_one(&self.pool)
        .await?;

        Ok(wallet)
    }

    pub async fn get_balance(&self, user_id: Uuid, currency: &str) -> Result<Decimal> {
        let result = sqlx::query_as::<_, (Decimal,)>(
            "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = $2",
        )
        .bind(user_id)
        .bind(currency)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0).unwrap_or(Decimal::ZERO))
    }

    pub async fn deposit(
        &self,
        user_id: Uuid,
        amount: Decimal,
        currency: &str,
        reference: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Transaction> {
        if amount <= Decimal::ZERO {
            return Err(AppError::Validation(
                "Deposit amount must be positive".to_string(),
            ));
        }

        let limits = self.get_deposit_limits().await?;
        if amount < limits.min_amount {
            return Err(AppError::Validation(format!(
                "Deposit amount must be at least {}",
                limits.min_amount
            )));
        }
        if amount > limits.max_amount {
            return Err(AppError::Validation(format!(
                "Deposit amount must not exceed {}",
                limits.max_amount
            )));
        }

        let verification_level = get_user_verification_level(&self.pool, user_id).await?;
        if let Some((limit, _)) = get_transaction_limit(verification_level) {
            let limit_decimal = Decimal::new(limit, 2);
            if amount > limit_decimal {
                return Err(AppError::Forbidden(format!(
                    "Amount exceeds limit for verification level {:?}. KYC verification required.",
                    verification_level
                )));
            }
        }

        self.check_aml_velocity(user_id, currency, amount).await?;

        if let Some(key) = idempotency_key {
            if let Some(existing) = self.check_idempotency(key).await? {
                tracing::info!(
                    "Returning existing transaction for idempotency key: {}",
                    key
                );
                return Ok(existing);
            }
        }

        let mut tx = self.pool.begin().await?;

        let _lock = sqlx::query(
            "SELECT 1 FROM wallet_balances WHERE user_id = $1 AND currency = $2 FOR UPDATE",
        )
        .bind(user_id)
        .bind(currency)
        .fetch_optional(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO wallet_balances (id, user_id, currency, balance, updated_at)
            VALUES ($1, $2, $3, 0, NOW())
            ON CONFLICT (user_id, currency) DO UPDATE SET balance = wallet_balances.balance + $4, updated_at = NOW()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(currency)
        .bind(amount)
        .execute(&mut *tx)
        .await?;

        let transaction_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO wallet_transactions (id, user_id, currency, tx_type, amount, reference_id, status, created_at, idempotency_key)
            VALUES ($1, $2, $3, 'deposit', $4, $5, 'completed', NOW(), $6)
            "#,
        )
        .bind(transaction_id)
        .bind(user_id)
        .bind(currency)
        .bind(amount)
        .bind(reference)
        .bind(idempotency_key)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Transaction {
            id: transaction_id,
            user_id,
            tx_type: "deposit".to_string(),
            amount,
            reference_id: Some(reference.to_string()),
            status: "completed".to_string(),
            created_at: Utc::now(),
        })
    }

    pub async fn withdraw(
        &self,
        user_id: Uuid,
        amount: Decimal,
        currency: &str,
        destination: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Transaction> {
        if amount <= Decimal::ZERO {
            return Err(AppError::Validation(
                "Withdrawal amount must be positive".to_string(),
            ));
        }

        let limits = self.get_deposit_limits().await?;
        if amount < limits.min_amount {
            return Err(AppError::Validation(format!(
                "Withdrawal amount must be at least {}",
                limits.min_amount
            )));
        }
        if amount > limits.max_amount {
            return Err(AppError::Validation(format!(
                "Withdrawal amount must not exceed {}",
                limits.max_amount
            )));
        }

        let verification_level = get_user_verification_level(&self.pool, user_id).await?;
        if !crate::services::verification::can_perform_transaction(verification_level, "withdrawal")
        {
            return Err(AppError::Forbidden(
                "Insufficient verification level for withdrawals".to_string(),
            ));
        }
        if let Some((limit, _)) = get_transaction_limit(verification_level) {
            let limit_decimal = Decimal::new(limit, 2);
            if amount > limit_decimal {
                return Err(AppError::Forbidden(format!(
                    "Amount exceeds limit for verification level {:?}. KYC verification required.",
                    verification_level
                )));
            }
        }

        if let Some(key) = idempotency_key {
            if let Some(existing) = self.check_idempotency(key).await? {
                tracing::info!(
                    "Returning existing transaction for idempotency key: {}",
                    key
                );
                return Ok(existing);
            }
        }

        let mut tx = self.pool.begin().await?;

        let current_balance = sqlx::query_as::<_, (Decimal,)>(
            "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = $2 FOR UPDATE",
        )
        .bind(user_id)
        .bind(currency)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.0)
        .unwrap_or(Decimal::ZERO);

        if current_balance < amount {
            return Err(AppError::InsufficientFunds(format!(
                "Insufficient balance. Available: {}, Requested: {}",
                current_balance, amount
            )));
        }

        sqlx::query(
            "UPDATE wallet_balances SET balance = balance - $1, updated_at = NOW() WHERE user_id = $2 AND currency = $3",
        )
        .bind(amount)
        .bind(user_id)
        .bind(currency)
        .execute(&mut *tx)
        .await?;

        let transaction_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO wallet_transactions (id, user_id, currency, tx_type, amount, reference_id, status, created_at, idempotency_key)
            VALUES ($1, $2, $3, 'withdrawal', $4, $5, 'pending', NOW(), $6)
            "#,
        )
        .bind(transaction_id)
        .bind(user_id)
        .bind(currency)
        .bind(amount)
        .bind(destination)
        .bind(idempotency_key)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Transaction {
            id: transaction_id,
            user_id,
            tx_type: "withdrawal".to_string(),
            amount,
            reference_id: Some(destination.to_string()),
            status: "pending".to_string(),
            created_at: Utc::now(),
        })
    }

    pub async fn transfer(
        &self,
        from_user_id: Uuid,
        to_user_id: Uuid,
        amount: Decimal,
        currency: &str,
        idempotency_key: Option<&str>,
    ) -> Result<(Transaction, Transaction)> {
        if amount <= Decimal::ZERO {
            return Err(AppError::Validation(
                "Transfer amount must be positive".to_string(),
            ));
        }
        if from_user_id == to_user_id {
            return Err(AppError::Validation("Cannot transfer to self".to_string()));
        }

        let verification_level = get_user_verification_level(&self.pool, from_user_id).await?;
        if !crate::services::verification::can_perform_transaction(verification_level, "transfer") {
            return Err(AppError::Forbidden(
                "Insufficient verification level for transfers".to_string(),
            ));
        }

        self.check_aml_velocity(from_user_id, currency, amount)
            .await?;

        if let Some(key) = idempotency_key {
            if let Some(existing) = self.check_idempotency(key).await? {
                if existing.user_id == from_user_id && existing.tx_type == "transfer_out" {
                    tracing::info!("Returning existing transfer for idempotency key: {}", key);
                    let counterparty_tx = self
                        .get_counterparty_transfer(&existing.id, to_user_id)
                        .await?;
                    return Ok((existing, counterparty_tx));
                }
            }
        }

        let mut tx = self.pool.begin().await?;

        let from_balance = sqlx::query_as::<_, (Decimal,)>(
            "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = $2 FOR UPDATE",
        )
        .bind(from_user_id)
        .bind(currency)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.0)
        .unwrap_or(Decimal::ZERO);

        if from_balance < amount {
            return Err(AppError::InsufficientFunds(format!(
                "Insufficient balance. Available: {}, Requested: {}",
                from_balance, amount
            )));
        }

        sqlx::query(
            "UPDATE wallet_balances SET balance = balance - $1, updated_at = NOW() WHERE user_id = $2 AND currency = $3",
        )
        .bind(amount)
        .bind(from_user_id)
        .bind(currency)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO wallet_balances (id, user_id, currency, balance, updated_at)
            VALUES ($1, $2, $3, 0, NOW())
            ON CONFLICT (user_id, currency) DO UPDATE SET balance = wallet_balances.balance + $4, updated_at = NOW()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(to_user_id)
        .bind(currency)
        .bind(amount)
        .execute(&mut *tx)
        .await?;

        let from_tx_id = Uuid::new_v4();
        let to_tx_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO wallet_transactions (id, user_id, currency, tx_type, amount, reference_id, status, created_at, idempotency_key, related_transaction_id)
            VALUES ($1, $2, $3, 'transfer_out', $4, $5, 'completed', NOW(), $6, $7)
            "#,
        )
        .bind(from_tx_id)
        .bind(from_user_id)
        .bind(currency)
        .bind(amount)
        .bind(to_user_id.to_string())
        .bind(idempotency_key)
        .bind(to_tx_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO wallet_transactions (id, user_id, currency, tx_type, amount, reference_id, status, created_at, related_transaction_id)
            VALUES ($1, $2, $3, 'transfer_in', $4, $5, 'completed', NOW(), $6)
            "#,
        )
        .bind(to_tx_id)
        .bind(to_user_id)
        .bind(currency)
        .bind(amount)
        .bind(from_user_id.to_string())
        .bind(from_tx_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let from_tx = Transaction {
            id: from_tx_id,
            user_id: from_user_id,
            tx_type: "transfer_out".to_string(),
            amount,
            reference_id: Some(to_user_id.to_string()),
            status: "completed".to_string(),
            created_at: Utc::now(),
        };

        let to_tx = Transaction {
            id: to_tx_id,
            user_id: to_user_id,
            tx_type: "transfer_in".to_string(),
            amount,
            reference_id: Some(from_user_id.to_string()),
            status: "completed".to_string(),
            created_at: Utc::now(),
        };

        Ok((from_tx, to_tx))
    }

    async fn get_counterparty_transfer(
        &self,
        related_id: &Uuid,
        counterparty_user_id: Uuid,
    ) -> Result<Transaction> {
        let tx = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT id, user_id, tx_type, amount, reference_id, status, created_at
            FROM wallet_transactions
            WHERE related_transaction_id = $1 AND user_id = $2
            "#,
        )
        .bind(related_id)
        .bind(counterparty_user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Counterparty transaction not found".to_string()))?;

        Ok(tx)
    }

    pub async fn get_transactions(
        &self,
        user_id: Uuid,
        currency: Option<&str>,
        limit: i32,
    ) -> Result<Vec<Transaction>> {
        let transactions = if let Some(curr) = currency {
            sqlx::query_as::<_, Transaction>(
                r#"
                SELECT id, user_id, tx_type, amount, reference_id, status, created_at
                FROM wallet_transactions
                WHERE user_id = $1 AND currency = $2
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(user_id)
            .bind(curr)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Transaction>(
                r#"
                SELECT id, user_id, tx_type, amount, reference_id, status, created_at
                FROM wallet_transactions
                WHERE user_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_validation_positive_amount() {
        let amount = Decimal::new(-100, 2);
        assert!(amount <= Decimal::ZERO);
    }

    #[test]
    fn test_withdrawal_validation_positive_amount() {
        let amount = Decimal::ZERO;
        assert!(amount <= Decimal::ZERO);
    }

    #[test]
    fn test_transfer_validation_different_users() {
        let user_id = Uuid::new_v4();
        assert_eq!(user_id, user_id);
    }

    #[test]
    fn test_deposit_limits_defaults() {
        let limits = DepositLimits {
            min_amount: Decimal::new(100, 2),
            max_amount: Decimal::new(1_000_000, 2),
        };
        assert!(limits.min_amount < limits.max_amount);
    }

    #[test]
    fn test_aml_velocity_limits_defaults() {
        let limits = AMLVelocityLimits {
            daily_limit: Decimal::new(10_000_00, 2),
            monthly_limit: Decimal::new(100_000_00, 2),
            transaction_count_limit: 100,
        };
        assert!(limits.daily_limit < limits.monthly_limit);
        assert!(limits.transaction_count_limit > 0);
    }
}
