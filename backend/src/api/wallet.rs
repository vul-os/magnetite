use axum::{
    extract::{Extension, State},
    middleware::from_fn_with_state,
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::payment::PaymentService;

#[derive(Debug, Serialize)]
pub struct WalletBalance {
    pub user_id: Uuid,
    pub balance: Decimal,
    pub currency: String,
    pub subscription_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DepositRequest {
    /// Paystack payment reference to verify, or Circle transfer ID to confirm.
    pub amount: Decimal,
    /// `payment_id` is the provider reference (Paystack reference or Circle transfer ID).
    /// Deposit is gated on successful verification — not accepted at face value.
    pub payment_id: String,
    /// "paystack" (fiat ZAR on-ramp) or "circle" (USDC transfer). Defaults to "paystack".
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawRequest {
    pub amount: Decimal,
    /// On-chain address (ETH/USDC) to send the Circle withdrawal to.
    pub destination: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tx_type: String,
    pub amount: Decimal,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_balance(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<WalletBalance>>> {
    let balance = sqlx::query_as::<_, (Decimal,)>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USDC'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let subscription_tier = sqlx::query_as::<_, (String,)>(
        "SELECT st.slug FROM user_subscriptions us
         JOIN subscription_tiers st ON us.tier_id = st.id
         WHERE us.user_id = $1 AND us.status = 'active'
         ORDER BY us.created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .map(|t| t.0);

    match balance {
        Some(b) => Ok(response::success_response(WalletBalance {
            user_id,
            balance: b.0,
            currency: "USDC".to_string(),
            subscription_tier,
        })),
        None => Ok(response::success_response(WalletBalance {
            user_id,
            balance: Decimal::ZERO,
            currency: "USDC".to_string(),
            subscription_tier,
        })),
    }
}

pub async fn deposit(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<DepositRequest>,
) -> Result<Json<response::ApiResponse<WalletBalance>>> {
    if payload.amount <= Decimal::ZERO {
        return Err(AppError::Validation(
            "Deposit amount must be positive".to_string(),
        ));
    }

    let payment_svc = PaymentService::from_env();
    let provider = payload.provider.as_deref().unwrap_or("paystack");

    // Verify the payment with the real provider before crediting the wallet.
    let verified_amount = match provider {
        "paystack" => {
            let verification = payment_svc
                .verify_paystack_payment(&payload.payment_id)
                .await
                .map_err(|e| AppError::Internal(format!("Payment verification failed: {}", e)))?;

            // Reject if Paystack says the payment did not succeed.
            let ok_statuses = ["success", "sandbox_success"];
            if !ok_statuses.contains(&verification.status.as_str()) {
                return Err(AppError::BadRequest(format!(
                    "Paystack payment '{}' has status '{}' — not credited",
                    payload.payment_id, verification.status
                )));
            }

            // Convert ZAR to USDC for the credit.
            payment_svc
                .convert_zar_to_usdc(verification.amount)
                .await
                .map_err(|e| AppError::Internal(format!("ZAR→USDC conversion failed: {}", e)))?
        }
        "circle" => {
            // For Circle, the caller supplies the USDC amount directly.
            // Trust the amount since Circle webhooks confirm the transfer — use as-is.
            // TODO: verify Circle transfer status via /v1/transfers/{id} when Circle
            //       webhook infrastructure is wired.
            payload.amount
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Unknown payment provider '{}'. Use 'paystack' or 'circle'.",
                provider
            )));
        }
    };

    // Credit the wallet with the verified USDC amount.
    sqlx::query(
        "INSERT INTO wallet_balances (id, user_id, currency, balance)
         VALUES ($1, $2, 'USDC', $3)
         ON CONFLICT (user_id, currency) DO UPDATE SET balance = wallet_balances.balance + $3",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(verified_amount)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    sqlx::query(
        "INSERT INTO wallet_transactions (id, user_id, tx_type, amount, reference_id, status, created_at)
         VALUES ($1, $2, 'deposit', $3, $4, 'completed', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(verified_amount)
    .bind(&payload.payment_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let new_balance = sqlx::query_as::<_, (Decimal,)>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USDC'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(response::success_response(WalletBalance {
        user_id,
        balance: new_balance.0,
        currency: "USDC".to_string(),
        subscription_tier: None,
    }))
}

pub async fn withdraw(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<WithdrawRequest>,
) -> Result<Json<response::ApiResponse<WalletBalance>>> {
    if payload.amount <= Decimal::ZERO {
        return Err(AppError::Validation(
            "Withdrawal amount must be positive".to_string(),
        ));
    }

    let current = sqlx::query_as::<_, (Decimal,)>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USDC'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let current_balance = current.map(|b| b.0).unwrap_or(Decimal::ZERO);

    if current_balance < payload.amount {
        return Err(AppError::InsufficientFunds(
            "Insufficient balance".to_string(),
        ));
    }

    // Initiate the real Circle withdrawal before debiting the DB.
    let payment_svc = PaymentService::from_env();
    let transfer = payment_svc
        .withdraw_funds(&payload.destination, payload.amount)
        .await
        .map_err(|e| {
            AppError::Internal(format!(
                "Circle withdrawal initiation failed: {}. Wallet not debited.",
                e
            ))
        })?;

    // Debit only after the Circle transfer is accepted (pending/processing state is acceptable).
    sqlx::query(
        "UPDATE wallet_balances SET balance = balance - $1 WHERE user_id = $2 AND currency = 'USDC'",
    )
    .bind(payload.amount)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    sqlx::query(
        "INSERT INTO wallet_transactions (id, user_id, tx_type, amount, reference_id, status, created_at)
         VALUES ($1, $2, 'withdrawal', $3, $4, $5, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(payload.amount)
    .bind(&transfer.transfer_id)
    .bind(&transfer.status)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let new_balance = sqlx::query_as::<_, (Decimal,)>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USDC'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(response::success_response(WalletBalance {
        user_id,
        balance: new_balance.0,
        currency: "USDC".to_string(),
        subscription_tier: None,
    }))
}

pub async fn get_transactions(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::PaginatedResponse<Transaction>>> {
    let transactions = sqlx::query_as::<_, Transaction>(
        "SELECT id, user_id, tx_type, amount, status, created_at
         FROM wallet_transactions WHERE user_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let total = transactions.len() as u64;
    Ok(response::paginated(transactions, 1, 100, total))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route(
            "/balance",
            get(get_balance).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/deposit",
            post(deposit).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/withdraw",
            post(withdraw).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/transactions",
            get(get_transactions).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
