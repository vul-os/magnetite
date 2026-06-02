// Webhooks API — Paystack, GitHub event handlers; platform surface.
#![allow(dead_code)]

use axum::{
    body::Bytes,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    middleware::from_fn_with_state,
    routing::{delete, get, post},
    Json, Router,
};
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::notifications::NotificationService;
use crate::error::{AppError, Result};

type HmacSha256 = Hmac<Sha256>;

const PAYSTACK_HEADER: &str = "x-paystack-signature";

fn compute_hmac_bytes(secret: &str, payload: &[u8]) -> Vec<u8> {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

fn compute_hmac_sha256(secret: &str, payload: &[u8]) -> String {
    hex::encode(compute_hmac_bytes(secret, payload))
}

/// Constant-time HMAC-SHA256 signature verification.
/// Uses `hmac::Mac::verify_slice` which is constant-time, avoiding timing side-channels.
fn verify_paystack_signature(secret: &str, signature: &str, payload: &[u8]) -> bool {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    // Decode the hex signature to raw bytes for constant-time comparison.
    match hex::decode(signature) {
        Ok(sig_bytes) => mac.verify_slice(&sig_bytes).is_ok(),
        Err(_) => false,
    }
}

/// Constant-time HMAC-SHA256 verification supporting optional "sha256=" prefix.
fn verify_hmac_signature(secret: &str, signature: &str, payload: &[u8]) -> bool {
    let sig_hex = if let Some(stripped) = signature.strip_prefix("sha256=") {
        stripped
    } else {
        signature
    };
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    match hex::decode(sig_hex) {
        Ok(sig_bytes) => mac.verify_slice(&sig_bytes).is_ok(),
        Err(_) => false,
    }
}

#[derive(Debug, Deserialize)]
pub struct PaystackWebhook {
    pub event: String,
    pub data: PaystackTransaction,
}

#[derive(Debug, Deserialize)]
pub struct PaystackTransaction {
    pub id: i64,
    pub reference: String,
    pub amount: i64,
    pub currency: String,
    pub status: String,
    pub customer: PaystackCustomer,
    pub metadata: Option<PaystackMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct PaystackCustomer {
    pub id: i64,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct PaystackMetadata {
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub status: String,
    pub message: String,
}

pub async fn handle_paystack(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>> {
    let secret_key = std::env::var("PAYSTACK_SECRET_KEY")
        .map_err(|_| AppError::Internal("PAYSTACK_SECRET_KEY not configured".to_string()))?;

    let payload = body.to_vec();

    let signature = headers
        .get(PAYSTACK_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing Paystack signature".to_string()))?;

    if !verify_paystack_signature(&secret_key, signature, &payload) {
        return Err(AppError::Unauthorized(
            "Invalid Paystack signature".to_string(),
        ));
    }

    let event: PaystackWebhook = serde_json::from_slice(&payload)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse Paystack webhook: {}", e)))?;

    tracing::info!("Paystack webhook received: event={}", event.event);

    match event.event.as_str() {
        "charge.success" => {
            let user_id = event
                .data
                .metadata
                .as_ref()
                .and_then(|m| m.user_id)
                .ok_or_else(|| AppError::BadRequest("Missing user_id in metadata".to_string()))?;

            // Idempotency: skip if this reference was already processed.
            let existing: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM wallet_transactions WHERE reference_id = $1 LIMIT 1",
            )
            .bind(&event.data.reference)
            .fetch_optional(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            if existing.is_some() {
                tracing::info!(
                    "Paystack webhook charge.success for reference '{}' already processed — skipping",
                    event.data.reference
                );
                return Ok(Json(WebhookResponse {
                    status: "ok".to_string(),
                    message: "Already processed".to_string(),
                }));
            }

            // Paystack webhook amount is in kobo (ZAR cents); convert to ZAR then to USD.
            let zar_amount = Decimal::from(event.data.amount) / Decimal::from(100);
            let zar_usd_rate: Decimal = std::env::var("ZAR_USD_RATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| Decimal::new(54, 3)); // 0.054
            let amount = zar_amount * zar_usd_rate;

            sqlx::query(
                "INSERT INTO wallet_transactions (id, user_id, tx_type, amount, reference_id, status, created_at)
                 VALUES ($1, $2, 'deposit', $3, $4, 'completed', NOW())",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(amount)
            .bind(&event.data.reference)
            .execute(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            sqlx::query(
                "INSERT INTO wallet_balances (id, user_id, currency, balance)
                 VALUES ($1, $2, 'USD', $3)
                 ON CONFLICT (user_id, currency) DO UPDATE SET balance = wallet_balances.balance + $3",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(amount)
            .execute(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            let notif_service = NotificationService::new(pool.clone());
            let _ = notif_service
                .create_system_notification(
                    user_id,
                    "Deposit Received",
                    &format!("Your wallet has been credited with {} USD", amount),
                )
                .await;

            tracing::info!(
                "Credited user {} wallet with {} USD from Paystack",
                user_id,
                amount
            );
        }
        "transfer.success" => {
            let _user_id = event
                .data
                .metadata
                .as_ref()
                .and_then(|m| m.user_id)
                .ok_or_else(|| AppError::BadRequest("Missing user_id in metadata".to_string()))?;

            sqlx::query(
                "UPDATE wallet_transactions SET status = 'completed' WHERE reference_id = $1",
            )
            .bind(&event.data.reference)
            .execute(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            tracing::info!("Marked withdrawal {} as completed", event.data.reference);
        }
        _ => {
            tracing::info!("Unhandled Paystack event: {}", event.event);
        }
    }

    Ok(Json(WebhookResponse {
        status: "ok".to_string(),
        message: format!("Processed Paystack event: {}", event.event),
    }))
}

#[derive(Debug, Deserialize)]
pub struct GameWebhook {
    pub event: String,
    pub game_id: Uuid,
    pub server_id: Option<String>,
    pub data: GameEventData,
}

#[derive(Debug, Deserialize)]
pub struct GameEventData {
    pub user_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub score: Option<i64>,
    pub timestamp: Option<String>,
    pub reason: Option<String>,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct GameWebhookResponse {
    pub status: String,
    pub processed: bool,
}

pub async fn handle_game(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<GameWebhookResponse>> {
    let webhook_secret = std::env::var("GAME_WEBHOOK_SECRET")
        .map_err(|_| AppError::Internal("GAME_WEBHOOK_SECRET not configured".to_string()))?;

    let payload = body.to_vec();

    let signature = headers
        .get("x-game-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing game signature".to_string()))?;

    if !verify_hmac_signature(&webhook_secret, signature, &payload) {
        return Err(AppError::Unauthorized("Invalid game signature".to_string()));
    }

    let event: GameWebhook = serde_json::from_slice(&payload)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse game webhook: {}", e)))?;

    tracing::info!(
        "Game webhook received: event={}, game_id={}",
        event.event,
        event.game_id
    );

    let mut processed = true;

    match event.event.as_str() {
        "session.start" => {
            if let (Some(user_id), Some(session_id)) = (event.data.user_id, event.data.session_id) {
                sqlx::query(
                    "INSERT INTO game_sessions (id, user_id, game_id, server_id, started_at, status)
                     VALUES ($1, $2, $3, $4, NOW(), 'active')
                     ON CONFLICT (id) DO UPDATE SET status = 'active'",
                )
                .bind(session_id)
                .bind(user_id)
                .bind(event.game_id)
                .bind(&event.server_id)
                .execute(&pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                tracing::info!("Game session started: session_id={}", session_id);
            }
        }
        "session.end" => {
            if let Some(session_id) = event.data.session_id {
                sqlx::query(
                    "UPDATE game_sessions SET ended_at = NOW(), status = 'completed' WHERE id = $1",
                )
                .bind(session_id)
                .execute(&pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                tracing::info!("Game session ended: session_id={}", session_id);
            }
        }
        "score.submit" => {
            if let (Some(user_id), Some(score)) = (event.data.user_id, event.data.score) {
                sqlx::query(
                    "INSERT INTO leaderboard_scores (id, user_id, game_id, score, submitted_at)
                     VALUES ($1, $2, $3, $4, NOW())",
                )
                .bind(Uuid::new_v4())
                .bind(user_id)
                .bind(event.game_id)
                .bind(score)
                .execute(&pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                tracing::info!(
                    "Score submitted: user_id={}, game_id={}, score={}",
                    user_id,
                    event.game_id,
                    score
                );
            }
        }
        "anticheat.flag" => {
            if let (Some(user_id), Some(reason)) = (event.data.user_id, &event.data.reason) {
                sqlx::query(
                    "INSERT INTO anti_cheat_flags (id, user_id, game_id, reason, details, flagged_at)
                     VALUES ($1, $2, $3, $4, $5, NOW())",
                )
                .bind(Uuid::new_v4())
                .bind(user_id)
                .bind(event.game_id)
                .bind(reason)
                .bind(&event.data.details)
                .execute(&pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                tracing::warn!(
                    "Anti-cheat flag: user_id={}, game_id={}, reason={}",
                    user_id,
                    event.game_id,
                    reason
                );
            }
        }
        _ => {
            tracing::info!("Unhandled game event: {}", event.event);
            processed = false;
        }
    }

    Ok(Json(GameWebhookResponse {
        status: "ok".to_string(),
        processed,
    }))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub secret: String,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterWebhookRequest {
    pub url: String,
    pub events: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WebhookEndpointResponse {
    pub id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookChallenge {
    pub challenge: String,
}

pub async fn list_endpoints(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<Vec<WebhookEndpointResponse>>> {
    let endpoints = sqlx::query_as::<_, WebhookEndpoint>(
        "SELECT id, user_id, url, events, secret, active, created_at
         FROM webhook_endpoints WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let response: Vec<WebhookEndpointResponse> = endpoints
        .into_iter()
        .map(|e| WebhookEndpointResponse {
            id: e.id,
            url: e.url,
            events: e.events,
            active: e.active,
            created_at: e.created_at,
        })
        .collect();

    Ok(Json(response))
}

pub async fn register_endpoint(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<RegisterWebhookRequest>,
) -> Result<Json<WebhookEndpointResponse>> {
    if payload.url.is_empty() {
        return Err(AppError::Validation("URL is required".to_string()));
    }

    if payload.events.is_empty() {
        return Err(AppError::Validation(
            "At least one event must be selected".to_string(),
        ));
    }

    let valid_events = ["payment.*", "kyc.*", "session.*", "score.*", "anticheat.*"];
    for event in &payload.events {
        if !valid_events.contains(&event.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid event type: {}",
                event
            )));
        }
    }

    // Generate a 32-byte CSPRNG URL-safe secret (256 bits of entropy).
    // UUID v4 only provides 122 bits and uses a non-URL-safe format.
    let secret = crate::services::session::generate_secure_token(32);

    let endpoint = sqlx::query_as::<_, (Uuid, chrono::DateTime<chrono::Utc>)>(
        "INSERT INTO webhook_endpoints (id, user_id, url, events, secret, active, created_at)
         VALUES ($1, $2, $3, $4, $5, true, NOW())
         RETURNING id, created_at",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(&payload.url)
    .bind(&payload.events)
    .bind(&secret)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let client = reqwest::Client::new();
    let challenge = uuid::Uuid::new_v4().to_string();

    match client
        .post(&payload.url)
        .json(&serde_json::json!({
            "type": "challenge",
            "challenge": challenge,
            "endpoint_id": endpoint.0
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                tracing::info!("Webhook endpoint verified: {}", endpoint.0);
            } else {
                tracing::warn!(
                    "Webhook endpoint challenge failed with status: {}",
                    resp.status()
                );
            }
        }
        Err(e) => {
            tracing::warn!("Failed to verify webhook endpoint: {}", e);
        }
    }

    Ok(Json(WebhookEndpointResponse {
        id: endpoint.0,
        url: payload.url,
        events: payload.events,
        active: true,
        created_at: endpoint.1,
    }))
}

pub async fn delete_endpoint(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(endpoint_id): Path<Uuid>,
) -> Result<StatusCode> {
    let result = sqlx::query("DELETE FROM webhook_endpoints WHERE id = $1 AND user_id = $2")
        .bind(endpoint_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Webhook endpoint not found".to_string()));
    }

    tracing::info!("Webhook endpoint deleted: {}", endpoint_id);
    Ok(StatusCode::NO_CONTENT)
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        // Inbound webhook receivers — no auth (called by external services, HMAC-signed).
        .route("/paystack", post(handle_paystack))
        .route("/game", post(handle_game))
        // Endpoint management — requires authentication.
        .route(
            "/endpoints",
            get(list_endpoints).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/endpoints",
            post(register_endpoint).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/endpoints/:id",
            delete(delete_endpoint).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
