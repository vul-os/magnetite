// Wallet API — NON-CUSTODIAL (seam §3.6).
//
// This node holds no funds. There is no balance, no deposit, no withdrawal and no
// payout. A "wallet" here is nothing but the Ed25519 address a user has linked, so
// that a purchase can pay them (or charge them) directly, wallet→wallet.
//
// The fiat endpoints (`/deposit`, `/withdraw`) and the `wallet_balances` /
// `wallet_transactions` custody tables are GONE.

use axum::{
    extract::{Extension, State},
    middleware::from_fn_with_state,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::error::{AppError, Result};
use crate::services::payment;

/// The user's linked wallet — an address, never a balance.
#[derive(Debug, Serialize)]
pub struct LinkedWallet {
    pub user_id: Uuid,
    /// Hex-encoded Ed25519 public key, or `null` if the user has not linked one.
    pub wallet_address: Option<String>,
    /// Always `false` — this node never holds user funds.
    pub custodial: bool,
    /// Rail in use (`mock` by default; offline and deterministic).
    pub rail: String,
}

#[derive(Debug, Deserialize)]
pub struct LinkWalletRequest {
    /// Hex-encoded Ed25519 public key (with or without a `0x` prefix).
    pub wallet_address: String,
}

fn rail_name() -> String {
    std::env::var("PAYMENT_RAIL").unwrap_or_else(|_| "mock".to_string())
}

/// GET /api/v1/wallet — report the linked wallet address.
pub async fn get_wallet(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<LinkedWallet>> {
    let wallet = payment::wallet_of(&pool, user_id).await?;
    Ok(Json(LinkedWallet {
        user_id,
        wallet_address: wallet.map(|w| w.to_hex()),
        custodial: false,
        rail: rail_name(),
    }))
}

/// POST /api/v1/wallet/link — link (or replace) the user's wallet address.
///
/// TODO(chain): require a signed challenge proving control of the key before
/// accepting it, once `AuthProvider::challenge` is wired into this route.
pub async fn link_wallet(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<LinkWalletRequest>,
) -> Result<Json<LinkedWallet>> {
    let hex_key = payload.wallet_address.trim().trim_start_matches("0x");
    let parsed = payment::PubKey::from_hex(hex_key).map_err(|_| {
        AppError::Validation("wallet_address must be a 32-byte hex Ed25519 key".to_string())
    })?;

    sqlx::query("UPDATE users SET wallet_address = $1 WHERE id = $2")
        .bind(parsed.to_hex())
        .bind(user_id)
        .execute(&pool)
        .await?;

    Ok(Json(LinkedWallet {
        user_id,
        wallet_address: Some(parsed.to_hex()),
        custodial: false,
        rail: rail_name(),
    }))
}

/// GET /api/v1/wallet/receipts — the signed receipts this user paid for.
/// This replaces the custodial transaction ledger.
pub async fn get_receipts(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<Vec<serde_json::Value>>> {
    type Row = (
        Uuid,
        String,
        i64,
        i64,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
    );
    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, kind, total, protocol_fee, rail_pubkey, voided, created_at
         FROM payment_receipts WHERE buyer_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, kind, total, fee, rail_pubkey, voided, created_at)| {
                serde_json::json!({
                    "id": id,
                    "kind": kind,
                    "total": total,
                    "protocol_fee": fee,
                    "rail_pubkey": rail_pubkey,
                    "voided": voided,
                    "created_at": created_at,
                })
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct HostingFeeRequest {
    /// Operator wallet (hex Ed25519 pubkey) receiving the hosting fee.
    pub operator_pubkey: String,
    /// Fee in the rail's smallest unit (e.g. USDC cents).
    pub amount: u64,
    /// Server this fee buys access to.
    pub server_id: Uuid,
}

/// POST /api/v1/wallet/hosting/pay — pay an operator's hosting fee.
///
/// Scaffold for §3.6(b): the operator is paid per-seat/per-hour and joining a paid
/// server requires the resulting receipt (see `GET /wallet/hosting/:server_id`).
pub async fn pay_hosting_fee(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<HostingFeeRequest>,
) -> Result<Json<serde_json::Value>> {
    let operator = payment::PubKey::from_hex(payload.operator_pubkey.trim_start_matches("0x"))
        .map_err(|_| {
            AppError::Validation("operator_pubkey must be a 32-byte hex Ed25519 key".to_string())
        })?;

    let receipt = payment::charge_hosting_fee(
        &pool,
        user_id,
        &operator,
        payload.amount,
        Some(payload.server_id),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "paid": true,
        "total": receipt.total,
        "rail_pubkey": receipt.rail_pubkey.to_hex(),
        "server_id": payload.server_id,
    })))
}

/// GET /api/v1/wallet/hosting/:server_id — may the caller join this paid server?
pub async fn hosting_access(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    axum::extract::Path(server_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let allowed = payment::has_hosting_access(&pool, user_id, server_id).await?;
    Ok(Json(serde_json::json!({
        "server_id": server_id,
        "allowed": allowed,
    })))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route(
            "/",
            get(get_wallet).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/link",
            post(link_wallet).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/hosting/pay",
            post(pay_hosting_fee).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/hosting/:server_id",
            get(hosting_access).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/receipts",
            get(get_receipts).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
