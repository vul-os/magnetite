// Middleware — auth, admin guards, rate limiting, CORS, logging.
// auth_middleware and decode_token are platform surface; main.rs uses per-route extractors instead.
#![allow(dead_code)]

pub mod cors;
pub mod logging;
pub mod rate_limit;

pub use cors::cors_layer;

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::db::DbPool;

pub async fn auth_middleware(
    State(pool): State<DbPool>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    let token = auth_header.ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = decode_token(token).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id: Uuid = claims
        .get("sub")
        .and_then(|s| s.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let is_admin = sqlx::query_scalar::<_, bool>(
        "SELECT is_admin FROM users WHERE id = $1 AND banned_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::UNAUTHORIZED)?
    .unwrap_or(false);

    let session_id = claims
        .get("session_id")
        .and_then(|s| s.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    req.extensions_mut().insert(user_id);
    req.extensions_mut().insert(is_admin);
    if let Some(sid) = session_id {
        req.extensions_mut().insert(sid);
    }

    Ok(next.run(req).await)
}

pub async fn admin_middleware(
    State(_pool): State<DbPool>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let is_admin = req.extensions().get::<bool>().copied();

    if is_admin != Some(true) {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(req).await)
}

fn decode_token(token: &str) -> Result<serde_json::Value, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let token_data = decode::<serde_json::Value>(
        token,
        &DecodingKey::from_secret("your-secret-key".as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}
