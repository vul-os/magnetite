use axum::{
    extract::{Extension, Query, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{Result, AppError};
use crate::services::session::{self, SessionInfo, AccessToken, RefreshToken, TokenPair, ACCESS_TOKEN_EXPIRY_SECS, REFRESH_TOKEN_EXPIRY_SECS};

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

    let (user_agent, ip_address) = get_client_info(&headers);
    let tokens = session::create_session(&pool, user_id, &payload.email, user_agent, ip_address).await?;

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
    let user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, username, email, password_hash FROM users WHERE username = $1",
    )
    .bind(&payload.username)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::Authentication("Invalid credentials".to_string()))?;

    if !crate::services::auth::verify_password(&payload.password, &user.3) {
        return Err(AppError::Authentication("Invalid credentials".to_string()));
    }

    let (user_agent, ip_address) = get_client_info(&headers);
    let tokens = session::create_session(&pool, user.0, &user.2, user_agent, ip_address).await?;

    Ok(response::success_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: tokens.expires_at,
        user_id: user.0,
    }))
}

pub async fn refresh(
    State(pool): State<PgPool>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<response::ApiResponse<TokenPair>>> {
    let session = session::validate_refresh_token(&pool, &payload.refresh_token).await?;

    let user = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, email FROM users WHERE id = $1",
    )
    .bind(session.user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let new_refresh_token = session::generate_refresh_token();
    let new_refresh_token_hash = session::hash_refresh_token(&new_refresh_token)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let new_expires_at = chrono::Utc::now() + chrono::Duration::seconds(REFRESH_TOKEN_EXPIRY_SECS);

    sqlx::query(
        r#"
        UPDATE sessions
        SET refresh_token_hash = $1, expires_at = $2
        WHERE id = $3
        "#,
    )
    .bind(&new_refresh_token_hash)
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
    Ok(response::success_response(serde_json::json!({ "message": "Session revoked" })))
}

pub async fn logout_all(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let count = session::revoke_all_user_sessions(&pool, user_id).await?;
    Ok(response::success_response(serde_json::json!({ "message": format!("{} sessions revoked", count) })))
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

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/refresh", post(refresh))
        .route("/logout", delete(logout).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/logout-all", delete(logout_all).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/sessions", get(list_sessions).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/me", get(me).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .with_state(pool)
}