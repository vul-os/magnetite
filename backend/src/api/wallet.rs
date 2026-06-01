// Wallet API — fiat USD balance, Paystack deposits, Wise-payout withdrawals.
//
// Deposit: verify a Paystack payment reference, credit USD balance.
// Withdraw: insert a payout_requests row (status=pending) for the Wise payout job (main.rs).

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
    /// Kept for API compatibility; the actual credited amount is derived from the
    /// Paystack-VERIFIED amount converted at the server-authoritative ZAR_USD_RATE.
    /// A caller cannot self-credit an arbitrary amount by manipulating this field.
    #[allow(dead_code)]
    pub amount: Decimal,
    /// Paystack payment reference to verify before crediting the USD balance.
    pub payment_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawRequest {
    pub amount: Decimal,
    /// Payout destination — Wise recipient details (e.g. email or bank).
    /// Stored on the payout_requests row; the Wise payout job resolves the actual recipient.
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
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USD'",
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
            currency: "USD".to_string(),
            subscription_tier,
        })),
        None => Ok(response::success_response(WalletBalance {
            user_id,
            balance: Decimal::ZERO,
            currency: "USD".to_string(),
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

    // --- Idempotency: reject if this payment_id has already been credited. ---
    let existing: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM wallet_transactions WHERE reference_id = $1 LIMIT 1")
            .bind(&payload.payment_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::BadRequest(format!(
            "Payment '{}' has already been credited — replay rejected",
            payload.payment_id
        )));
    }

    let payment_svc = PaymentService::from_env();

    // Verify the Paystack payment before crediting.
    let verification = payment_svc
        .verify_paystack_payment(&payload.payment_id)
        .await
        .map_err(|e| AppError::Internal(format!("Payment verification failed: {}", e)))?;

    let ok_statuses = ["success", "sandbox_success"];
    if !ok_statuses.contains(&verification.status.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Paystack payment '{}' has status '{}' — not credited",
            payload.payment_id, verification.status
        )));
    }

    // Use the Paystack-VERIFIED amount (in ZAR) converted to USD at the server-authoritative
    // exchange rate.  The rate is read from the ZAR_USD_RATE env var (default: 0.054, i.e.
    // 1 ZAR ≈ 0.054 USD at time of writing).  A user cannot self-credit an arbitrary amount
    // by manipulating the JSON body — the credited amount is always derived from the amount
    // that Paystack actually charged.
    let zar_usd_rate: Decimal = std::env::var("ZAR_USD_RATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| Decimal::new(54, 3)); // 0.054

    let credit_amount = verification.amount * zar_usd_rate;

    if credit_amount <= Decimal::ZERO {
        return Err(AppError::Internal(
            "Computed credit amount is zero — check ZAR_USD_RATE env var".to_string(),
        ));
    }

    // Credit the wallet with the Paystack-verified USD amount.
    sqlx::query(
        "INSERT INTO wallet_balances (id, user_id, currency, balance)
         VALUES ($1, $2, 'USD', $3)
         ON CONFLICT (user_id, currency) DO UPDATE SET balance = wallet_balances.balance + $3",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(credit_amount)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    sqlx::query(
        "INSERT INTO wallet_transactions (id, user_id, tx_type, amount, reference_id, status, created_at)
         VALUES ($1, $2, 'deposit', $3, $4, 'completed', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(credit_amount)
    .bind(&payload.payment_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let new_balance = sqlx::query_as::<_, (Decimal,)>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USD'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(response::success_response(WalletBalance {
        user_id,
        balance: new_balance.0,
        currency: "USD".to_string(),
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
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USD'",
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

    // Debit the balance and record a pending withdrawal transaction.
    sqlx::query(
        "UPDATE wallet_balances SET balance = balance - $1 WHERE user_id = $2 AND currency = 'USD'",
    )
    .bind(payload.amount)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let payout_id = Uuid::new_v4();

    // Insert a payout_requests row so the Wise payout job (spawned in main.rs) picks it up.
    sqlx::query(
        "INSERT INTO payout_requests (id, developer_id, amount, currency, destination, status, created_at)
         VALUES ($1, $2, $3, 'USD', $4, 'pending', NOW())",
    )
    .bind(payout_id)
    .bind(user_id)
    .bind(payload.amount)
    .bind(&payload.destination)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    sqlx::query(
        "INSERT INTO wallet_transactions (id, user_id, tx_type, amount, reference_id, status, created_at)
         VALUES ($1, $2, 'withdrawal', $3, $4, 'pending', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(payload.amount)
    .bind(payout_id.to_string())
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let new_balance = sqlx::query_as::<_, (Decimal,)>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USD'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(response::success_response(WalletBalance {
        user_id,
        balance: new_balance.0,
        currency: "USD".to_string(),
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
