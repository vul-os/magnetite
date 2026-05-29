use axum::{
    extract::{State, Extension},
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
use crate::error::{Result, AppError};

#[derive(Debug, Serialize)]
pub struct WalletBalance {
    pub user_id: Uuid,
    pub balance: Decimal,
    pub currency: String,
    pub subscription_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DepositRequest {
    pub amount: Decimal,
    pub payment_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawRequest {
    pub amount: Decimal,
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
    sqlx::query(
        "INSERT INTO wallet_balances (id, user_id, currency, balance)
         VALUES ($1, $2, 'USDC', $3)
         ON CONFLICT (user_id, currency) DO UPDATE SET balance = wallet_balances.balance + $3",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(payload.amount)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    sqlx::query(
        "INSERT INTO wallet_transactions (id, user_id, tx_type, amount, reference_id, status, created_at)
         VALUES ($1, $2, 'deposit', $3, $4, 'completed', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(payload.amount)
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
    let current = sqlx::query_as::<_, (Decimal,)>(
        "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USDC'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let current_balance = current.map(|b| b.0).unwrap_or(Decimal::ZERO);

    if current_balance < payload.amount {
        return Err(AppError::InsufficientFunds("Insufficient balance".to_string()));
    }

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
         VALUES ($1, $2, 'withdrawal', $3, $4, 'pending', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(payload.amount)
    .bind(&payload.destination)
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
        .route("/balance", get(get_balance).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/deposit", post(deposit).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/withdraw", post(withdraw).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/transactions", get(get_transactions).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .with_state(pool)
}
