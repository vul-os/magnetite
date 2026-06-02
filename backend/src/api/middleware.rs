// Auth/admin middleware helpers — optional_auth and guard functions; platform surface.
#![allow(dead_code)]

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Check whether `user_id` is an admin, using the DB as the authoritative source.
async fn check_is_admin(pool: &PgPool, user_id: Uuid) -> Result<bool> {
    let is_admin = sqlx::query_scalar::<_, bool>("SELECT is_admin FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .unwrap_or(false);
    Ok(is_admin)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub session_id: Option<String>,
    pub exp: i64,
    pub iat: i64,
}

const BEARER_PREFIX: &str = "Bearer ";

pub fn extract_token_from_header(headers: &HeaderMap) -> Result<String> {
    let auth_header = headers
        .get("Authorization")
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?
        .to_str()
        .map_err(|_| AppError::Unauthorized("Invalid Authorization header".to_string()))?;

    if !auth_header.starts_with(BEARER_PREFIX) {
        return Err(AppError::Unauthorized(
            "Invalid Authorization header format".to_string(),
        ));
    }

    Ok(auth_header[BEARER_PREFIX.len()..].to_string())
}

pub fn validate_token(token: &str) -> Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(crate::config::get_jwt_secret().as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| AppError::Authentication(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

pub async fn auth_guard(headers: HeaderMap) -> Result<Uuid> {
    let token = extract_token_from_header(&headers)?;
    let claims = validate_token(&token)?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid user ID in token".to_string()))?;

    Ok(user_id)
}

pub async fn optional_auth(headers: HeaderMap) -> Option<Uuid> {
    let token = match extract_token_from_header(&headers) {
        Ok(t) => t,
        Err(_) => return None,
    };

    let claims = match validate_token(&token) {
        Ok(c) => c,
        Err(_) => return None,
    };

    Uuid::parse_str(&claims.sub).ok()
}

/// Dummy synchronous guard — kept for call-sites that do not have pool access.
/// Prefer `admin_guard_with_pool` for middleware use.
pub async fn admin_guard(_user_id: Uuid) -> Result<()> {
    // Callers that can provide a pool should use admin_guard_with_pool instead.
    Err(AppError::Forbidden("Admin access required".to_string()))
}

/// DB-backed admin guard — checks the `is_admin` flag in the users table.
pub async fn admin_guard_with_pool(pool: &PgPool, user_id: Uuid) -> Result<()> {
    if check_is_admin(pool, user_id).await? {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin access required".to_string()))
    }
}

pub async fn auth_middleware(
    State(pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match extract_token_from_header(&request.headers().clone()) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    let claims = match validate_token(&token) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return AppError::Unauthorized("Invalid user ID in token".to_string()).into_response()
        }
    };

    // Session revocation check: if the JWT contains a session_id, confirm the session
    // still exists in the DB (deleted on logout, so revoked tokens stop working immediately).
    // A missing session_id claim (legacy tokens) is still accepted — they expire naturally.
    if let Some(sid_str) = &claims.session_id {
        if let Ok(session_id) = Uuid::parse_str(sid_str) {
            // Skip check for nil UUID (tokens created before session association).
            if !session_id.is_nil() {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1 AND expires_at > NOW())",
                )
                .bind(session_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(false);

                if !exists {
                    return AppError::Unauthorized(
                        "Session has been revoked or expired".to_string(),
                    )
                    .into_response();
                }
            }
        }
    }

    request.extensions_mut().insert(Extension(user_id));
    next.run(request).await
}

pub async fn admin_middleware(
    State(pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Response {
    let user_id = match auth_guard(request.headers().clone()).await {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    // Check the DB for the real is_admin flag — do NOT rely on the JWT claim alone.
    match admin_guard_with_pool(&pool, user_id).await {
        Ok(_) => {}
        Err(e) => return e.into_response(),
    };

    request.extensions_mut().insert(Extension(user_id));
    next.run(request).await
}
