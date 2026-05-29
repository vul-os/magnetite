use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

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
        return Err(AppError::Unauthorized("Invalid Authorization header format".to_string()));
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

pub async fn admin_guard(user_id: Uuid) -> Result<()> {
    Err(AppError::Forbidden("Admin access required".to_string()))
}

pub async fn auth_middleware(
    State(_pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Response {
    let user_id = match auth_guard(request.headers().clone()).await {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    request.extensions_mut().insert(Extension(user_id));
    next.run(request).await
}

pub async fn admin_middleware(
    State(_pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Response {
    let user_id = match auth_guard(request.headers().clone()).await {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    match admin_guard(user_id).await {
        Ok(_) => {}
        Err(e) => return e.into_response(),
    };

    request.extensions_mut().insert(Extension(user_id));
    next.run(request).await
}
