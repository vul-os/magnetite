// Auth API — registration, login, token refresh, logout, email verification, password reset,
//            API keys, 2FA TOTP.
#![allow(dead_code)]

use axum::{
    extract::{Extension, Path, Query, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::auth as auth_svc;
use crate::services::email::EmailService;
use crate::services::session::{
    self, AccessToken, RefreshToken, SessionInfo, TokenPair, ACCESS_TOKEN_EXPIRY_SECS,
    REFRESH_TOKEN_EXPIRY_SECS,
};
use crate::services::verification;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Required when the account has TOTP 2FA enabled.
    pub totp_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub user_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct TokenRefreshResponse {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SessionListQuery {
    pub session_id: Option<Uuid>,
}

// --- Email verification / password-reset requests -------------------------

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

// --------------------------------------------------------------------------

fn get_client_info(headers: &axum::http::HeaderMap) -> (Option<String>, Option<String>) {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let ip_address = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    (user_agent, ip_address)
}

pub async fn register(
    State(pool): State<PgPool>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<response::ApiResponse<AuthResponse>>> {
    let password_hash = crate::services::auth::hash_password(&payload.password)?;
    let user_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, created_at) VALUES ($1, $2, $3, $4, NOW())",
    )
    .bind(user_id)
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .execute(&pool)
    .await?;

    // Generate and send verification email. A failure here is logged but does not fail registration —
    // users can re-request verification. This keeps registration resilient when email is unconfigured.
    let token_result = verification::generate_email_verification_token(&pool, user_id).await;
    match token_result {
        Ok(token) => match EmailService::from_env() {
            Ok(svc) => {
                if let Err(e) = svc
                    .send_verification_email(&payload.email, &payload.username, &token)
                    .await
                {
                    tracing::warn!(
                        user_id = %user_id,
                        "Failed to send verification email: {}", e
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    user_id = %user_id,
                    "Email provider not configured, skipping verification email: {}", e
                );
            }
        },
        Err(e) => {
            tracing::warn!(user_id = %user_id, "Failed to generate verification token: {}", e);
        }
    }

    let (user_agent, ip_address) = get_client_info(&headers);
    let tokens =
        session::create_session(&pool, user_id, &payload.email, user_agent, ip_address).await?;

    Ok(response::success_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: tokens.expires_at,
        user_id,
    }))
}

pub async fn login(
    State(pool): State<PgPool>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<response::ApiResponse<AuthResponse>>> {
    // Fetch id, username, email, password_hash, email_verified, totp_enabled, totp_secret
    let user = sqlx::query_as::<_, (Uuid, String, String, String, bool, bool, Option<String>)>(
        r#"SELECT id, username, email, password_hash, email_verified, totp_enabled, totp_secret
           FROM users WHERE username = $1"#,
    )
    .bind(&payload.username)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::Authentication("Invalid credentials".to_string()))?;

    let (user_id, _username, email, password_hash, email_verified, totp_enabled, totp_secret) =
        user;

    // 1. Verify password first (constant-time early rejection, same error message).
    if !crate::services::auth::verify_password(&payload.password, &password_hash) {
        return Err(AppError::Authentication("Invalid credentials".to_string()));
    }

    // 2. Enforce email verification — unverified accounts are blocked at login.
    //    Decision (AX1-A2): block login (not restricted token) — simpler, no middleware changes.
    if !email_verified {
        return Err(AppError::Authentication(
            "Email not verified. Please check your inbox and verify your email before logging in."
                .to_string(),
        ));
    }

    // 3. Enforce TOTP if enabled.
    if totp_enabled {
        let code = payload
            .totp_code
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string();
        if code.is_empty() {
            return Err(AppError::Authentication(
                "2fa_required: this account has 2FA enabled; provide a totp_code".to_string(),
            ));
        }
        let stored = totp_secret
            .ok_or_else(|| AppError::Internal("TOTP enabled but no secret stored".to_string()))?;
        if !auth_svc::verify_totp_stored(&stored, &code)? {
            return Err(AppError::Authentication("Invalid TOTP code".to_string()));
        }
    }

    let (user_agent, ip_address) = get_client_info(&headers);
    let tokens = session::create_session(&pool, user_id, &email, user_agent, ip_address).await?;

    Ok(response::success_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: tokens.expires_at,
        user_id,
    }))
}

pub async fn refresh(
    State(pool): State<PgPool>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<response::ApiResponse<TokenPair>>> {
    let session = session::validate_refresh_token(&pool, &payload.refresh_token).await?;

    let user = sqlx::query_as::<_, (Uuid, String)>("SELECT id, email FROM users WHERE id = $1")
        .bind(session.user_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let new_refresh_token = session::generate_refresh_token();
    let new_prefix = session::token_prefix(&new_refresh_token);
    let new_refresh_token_hash = session::hash_refresh_token(&new_refresh_token)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let new_expires_at = chrono::Utc::now() + chrono::Duration::seconds(REFRESH_TOKEN_EXPIRY_SECS);

    sqlx::query(
        r#"
        UPDATE sessions
        SET refresh_token_hash = $1, token_prefix = $2, expires_at = $3
        WHERE id = $4
        "#,
    )
    .bind(&new_refresh_token_hash)
    .bind(&new_prefix)
    .bind(new_expires_at)
    .bind(session.id)
    .execute(&pool)
    .await?;

    let new_access_token = session::generate_access_token(user.0, session.id, &user.1)?;

    Ok(response::success_response(TokenPair {
        access_token: AccessToken {
            token: new_access_token,
            expires_in: ACCESS_TOKEN_EXPIRY_SECS,
        },
        refresh_token: RefreshToken {
            token: new_refresh_token,
            expires_in: REFRESH_TOKEN_EXPIRY_SECS,
        },
    }))
}

pub async fn logout(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Json(payload): Json<LogoutRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    session::revoke_token(&pool, &payload.refresh_token).await?;
    Ok(response::success_response(
        serde_json::json!({ "message": "Session revoked" }),
    ))
}

pub async fn logout_all(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let count = session::revoke_all_user_sessions(&pool, user_id).await?;
    Ok(response::success_response(
        serde_json::json!({ "message": format!("{} sessions revoked", count) }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

pub async fn list_sessions(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(query): Query<SessionListQuery>,
) -> Result<Json<response::ApiResponse<Vec<SessionInfo>>>> {
    let sessions = session::list_user_sessions(&pool, user_id, query.session_id).await?;
    Ok(response::success_response(sessions))
}

pub async fn me(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<UserResponse>>> {
    let user = sqlx::query_as::<_, (Uuid, String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, username, email, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(response::success_response(UserResponse {
        id: user.0,
        username: user.1,
        email: user.2,
        created_at: user.3,
    }))
}

/// POST /auth/forgot-password — generates a password-reset token and sends the reset email.
/// Always returns 200 to avoid leaking account existence (logs if email not found).
pub async fn forgot_password(
    State(pool): State<PgPool>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let user =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, username FROM users WHERE email = $1")
            .bind(&payload.email)
            .fetch_optional(&pool)
            .await?;

    match user {
        None => {
            // Don't reveal whether the email exists
            tracing::debug!(email = %payload.email, "forgot-password: email not found, returning 200 silently");
        }
        Some((user_id, username)) => {
            match verification::generate_password_reset_token(&pool, user_id).await {
                Ok(token) => match EmailService::from_env() {
                    Ok(svc) => {
                        if let Err(e) = svc
                            .send_password_reset_email(&payload.email, &username, &token)
                            .await
                        {
                            tracing::error!(user_id = %user_id, "Failed to send password reset email: {}", e);
                            return Err(AppError::Internal(
                                "Failed to send password reset email".to_string(),
                            ));
                        }
                    }
                    Err(e) => {
                        tracing::error!("Email provider not configured for password reset: {}", e);
                        return Err(AppError::Internal(
                            "Email service is not configured".to_string(),
                        ));
                    }
                },
                Err(e) => {
                    tracing::error!(user_id = %user_id, "Failed to generate reset token: {}", e);
                    return Err(AppError::Internal(
                        "Failed to generate password reset token".to_string(),
                    ));
                }
            }
        }
    }

    Ok(response::success_response(
        serde_json::json!({ "message": "If that email exists, a password reset link has been sent" }),
    ))
}

/// POST /auth/reset-password — consumes a reset token and updates the password.
pub async fn reset_password(
    State(pool): State<PgPool>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    if payload.new_password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let user_id = verification::verify_password_reset_token(&pool, &payload.token).await?;

    let new_hash = crate::services::auth::hash_password(&payload.new_password)?;
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&new_hash)
        .bind(user_id)
        .execute(&pool)
        .await?;

    // Revoke all existing sessions so the attacker (if any) is kicked out.
    let _ = session::revoke_all_user_sessions(&pool, user_id).await;

    Ok(response::success_response(
        serde_json::json!({ "message": "Password reset successfully. Please log in again." }),
    ))
}

/// POST /auth/verify-email — consumes an email-verification token; sends welcome email on success.
pub async fn verify_email(
    State(pool): State<PgPool>,
    Json(payload): Json<VerifyEmailRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let user_id = verification::verify_email_token(&pool, &payload.token).await?;

    // Fetch user info to send welcome email
    let user =
        sqlx::query_as::<_, (String, String)>("SELECT username, email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&pool)
            .await?;

    if let Some((username, email)) = user {
        match EmailService::from_env() {
            Ok(svc) => {
                if let Err(e) = svc.send_welcome_email(&email, &username).await {
                    tracing::warn!(user_id = %user_id, "Failed to send welcome email: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!(user_id = %user_id, "Email provider not configured, skipping welcome email: {}", e);
            }
        }
    }

    Ok(response::success_response(
        serde_json::json!({ "message": "Email verified successfully" }),
    ))
}

/// POST /auth/resend-verification — re-generates and sends the verification email.
pub async fn resend_verification(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let user = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT username, email, email_verified FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if user.2 {
        return Err(AppError::BadRequest(
            "Email is already verified".to_string(),
        ));
    }

    let token = verification::generate_email_verification_token(&pool, user_id).await?;
    let svc = EmailService::from_env()?;
    svc.send_verification_email(&user.1, &user.0, &token)
        .await?;

    Ok(response::success_response(
        serde_json::json!({ "message": "Verification email sent" }),
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// API Keys
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyCreatedResponse {
    pub id: Uuid,
    pub name: String,
    pub key: String, // plaintext — shown once only
    pub prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// POST /api/auth/api-keys — create a new API key.
/// The plaintext key is returned exactly once; only its SHA-256 hash is stored.
pub async fn create_api_key(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<Json<response::ApiResponse<ApiKeyCreatedResponse>>> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "API key name must not be empty".to_string(),
        ));
    }

    let (plaintext, prefix, key_hash) = auth_svc::generate_api_key();
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"
        INSERT INTO api_keys (id, user_id, name, key_hash, prefix, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(&name)
    .bind(&key_hash)
    .bind(&prefix)
    .bind(now)
    .execute(&pool)
    .await?;

    Ok(response::success_response(ApiKeyCreatedResponse {
        id,
        name,
        key: plaintext,
        prefix,
        created_at: now,
    }))
}

/// GET /api/auth/api-keys — list all non-revoked API keys (no secrets).
pub async fn list_api_keys(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<Vec<ApiKeyInfo>>>> {
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        r#"
        SELECT id, name, prefix, created_at, last_used_at
        FROM api_keys
        WHERE user_id = $1 AND revoked_at IS NULL
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let keys = rows
        .into_iter()
        .map(|(id, name, prefix, created_at, last_used_at)| ApiKeyInfo {
            id,
            name,
            prefix,
            created_at,
            last_used_at,
        })
        .collect();

    Ok(response::success_response(keys))
}

/// DELETE /api/auth/api-keys/:id — revoke an API key.
pub async fn revoke_api_key(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(key_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let result = sqlx::query(
        r#"
        UPDATE api_keys
        SET revoked_at = NOW()
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
        "#,
    )
    .bind(key_id)
    .bind(user_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "API key not found or already revoked".to_string(),
        ));
    }

    Ok(response::success_response(
        serde_json::json!({ "message": "API key revoked" }),
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// 2FA TOTP
// ═══════════════════════════════════════════════════════════════════════════

const TOTP_ISSUER: &str = "Magnetite";

#[derive(Debug, Serialize)]
pub struct TotpSetupResponse {
    /// Base32-encoded TOTP secret; store securely, show in QR-code / manual entry.
    pub secret: String,
    /// otpauth:// URI — encode this into a QR code for authenticator apps.
    pub uri: String,
}

#[derive(Debug, Deserialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct TotpDisableRequest {
    pub code: String,
}

/// POST /api/auth/2fa/setup — generate a new TOTP secret and store it as pending.
/// The secret is NOT active until /2fa/verify succeeds.
pub async fn totp_setup(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<TotpSetupResponse>>> {
    // Fetch the username to use as the account label in the otpauth URI.
    let (username,) = sqlx::query_as::<_, (String,)>("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let secret = auth_svc::generate_totp_secret();
    let uri = auth_svc::totp_uri(&secret, &username, TOTP_ISSUER);
    // Encrypt the secret before storage (AX1-A1 decision).
    let encrypted = auth_svc::encrypt_totp_secret(&secret);

    // Store pending secret (totp_enabled stays false until verify).
    sqlx::query(
        r#"
        UPDATE users
        SET totp_secret = $1, totp_enabled = FALSE
        WHERE id = $2
        "#,
    )
    .bind(&encrypted)
    .bind(user_id)
    .execute(&pool)
    .await?;

    Ok(response::success_response(TotpSetupResponse {
        // Return the plaintext base32 secret so the user can scan it.
        secret,
        uri,
    }))
}

/// POST /api/auth/2fa/verify — verify a TOTP code and activate 2FA.
pub async fn totp_verify(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<TotpVerifyRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let (secret, enabled): (Option<String>, bool) = sqlx::query_as::<_, (Option<String>, bool)>(
        "SELECT totp_secret, totp_enabled FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if enabled {
        return Err(AppError::BadRequest("2FA is already enabled".to_string()));
    }

    let secret = secret.ok_or_else(|| {
        AppError::BadRequest("No pending TOTP setup found. Call /2fa/setup first.".to_string())
    })?;

    // Use stored (possibly encrypted) secret for verification.
    if !auth_svc::verify_totp_stored(&secret, &payload.code)? {
        return Err(AppError::Authentication("Invalid TOTP code".to_string()));
    }

    sqlx::query("UPDATE users SET totp_enabled = TRUE WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await?;

    Ok(response::success_response(
        serde_json::json!({ "message": "2FA enabled successfully" }),
    ))
}

/// POST /api/auth/2fa/disable — verify a TOTP code and disable 2FA.
pub async fn totp_disable(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<TotpDisableRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let (secret, enabled): (Option<String>, bool) = sqlx::query_as::<_, (Option<String>, bool)>(
        "SELECT totp_secret, totp_enabled FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if !enabled {
        return Err(AppError::BadRequest("2FA is not enabled".to_string()));
    }

    let secret = secret.ok_or_else(|| AppError::Internal("TOTP secret missing".to_string()))?;

    // Use stored (possibly encrypted) secret for verification.
    if !auth_svc::verify_totp_stored(&secret, &payload.code)? {
        return Err(AppError::Authentication("Invalid TOTP code".to_string()));
    }

    sqlx::query("UPDATE users SET totp_enabled = FALSE, totp_secret = NULL WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await?;

    Ok(response::success_response(
        serde_json::json!({ "message": "2FA disabled successfully" }),
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// Password update
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// PUT /api/auth/password — update password for the authenticated user.
/// Verifies the current password before setting the new hash.
pub async fn update_password(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<UpdatePasswordRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    if payload.new_password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let (password_hash,) =
        sqlx::query_as::<_, (String,)>("SELECT password_hash FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if !auth_svc::verify_password(&payload.current_password, &password_hash) {
        return Err(AppError::Authentication(
            "Current password is incorrect".to_string(),
        ));
    }

    let new_hash = auth_svc::hash_password(&payload.new_password)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&new_hash)
        .bind(user_id)
        .execute(&pool)
        .await?;

    Ok(response::success_response(
        serde_json::json!({ "message": "Password updated successfully" }),
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// Linked accounts (OAuth identities)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize)]
pub struct LinkedAccountResponse {
    pub id: Uuid,
    pub provider: String,
    pub email: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LinkAccountRequest {
    pub provider: String,
    pub provider_id: String,
    pub email: Option<String>,
}

/// GET /api/auth/linked-accounts — list all OAuth identities for this user.
pub async fn list_linked_accounts(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<Vec<LinkedAccountResponse>>>> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, provider, email, created_at
           FROM oauth_identities WHERE user_id = $1
           ORDER BY created_at ASC"#,
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let accounts = rows
        .into_iter()
        .map(|(id, provider, email, created_at)| LinkedAccountResponse {
            id,
            provider,
            email,
            created_at,
        })
        .collect();

    Ok(response::success_response(accounts))
}

/// POST /api/auth/linked-accounts — link a new OAuth identity to this account.
pub async fn link_account(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<LinkAccountRequest>,
) -> Result<Json<response::ApiResponse<LinkedAccountResponse>>> {
    let valid_providers = ["google", "github", "discord", "gitlab"];
    if !valid_providers.contains(&payload.provider.as_str()) {
        return Err(AppError::Validation(format!(
            "Unknown provider '{}'. Valid: {}",
            payload.provider,
            valid_providers.join(", ")
        )));
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"INSERT INTO oauth_identities (id, user_id, provider, provider_id, email, created_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           ON CONFLICT (provider, provider_id) DO UPDATE
           SET user_id = EXCLUDED.user_id, email = EXCLUDED.email"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(&payload.provider)
    .bind(&payload.provider_id)
    .bind(&payload.email)
    .bind(now)
    .execute(&pool)
    .await?;

    Ok(response::success_response(LinkedAccountResponse {
        id,
        provider: payload.provider,
        email: payload.email,
        created_at: now,
    }))
}

/// DELETE /api/auth/linked-accounts/:id — unlink an OAuth identity.
pub async fn unlink_account(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(identity_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let result = sqlx::query("DELETE FROM oauth_identities WHERE id = $1 AND user_id = $2")
        .bind(identity_id)
        .bind(user_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Linked account not found".to_string()));
    }

    Ok(response::success_response(
        serde_json::json!({ "message": "Account unlinked" }),
    ))
}

// ═══════════════════════════════════════════════════════════════════════════

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/refresh", post(refresh))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
        .route("/verify-email", post(verify_email))
        .route(
            "/logout",
            delete(logout).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/logout-all",
            delete(logout_all).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/sessions",
            get(list_sessions).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/me",
            get(me).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/resend-verification",
            post(resend_verification).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // Password update
        .route(
            "/password",
            put(update_password).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // Linked accounts (OAuth identities)
        .route(
            "/linked-accounts",
            get(list_linked_accounts)
                .post(link_account)
                .layer(from_fn_with_state(
                    pool.clone(),
                    middleware::auth_middleware,
                )),
        )
        .route(
            "/linked-accounts/:id",
            delete(unlink_account).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // API keys — all auth-guarded
        .route(
            "/api-keys",
            post(create_api_key)
                .get(list_api_keys)
                .layer(from_fn_with_state(
                    pool.clone(),
                    middleware::auth_middleware,
                )),
        )
        .route(
            "/api-keys/:id",
            delete(revoke_api_key).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // 2FA TOTP — all auth-guarded
        .route(
            "/2fa/setup",
            post(totp_setup).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/2fa/verify",
            post(totp_verify).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/2fa/disable",
            post(totp_disable).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // DELETE /2fa — alias for totp_disable (frontend calls DELETE /auth/2fa per AUDIT.md)
        .route(
            "/2fa",
            delete(totp_disable).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
