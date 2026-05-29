// Session service — token generation, refresh, revocation; platform surface, partially wired.
#![allow(dead_code)]

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64;
use chrono::{DateTime, Duration, Utc};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::AppError;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub token: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    pub token: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub email: String,
    pub session_id: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Serialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_current: bool,
}

const REFRESH_TOKEN_SIZE: usize = 32;
pub const ACCESS_TOKEN_EXPIRY_SECS: i64 = 15 * 60;
pub const REFRESH_TOKEN_EXPIRY_SECS: i64 = 7 * 24 * 60 * 60;

pub fn generate_secure_token(size: usize) -> String {
    let mut rng = ChaCha20Rng::from_entropy();
    let bytes: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
    base64::encode_config(&bytes, base64::URL_SAFE_NO_PAD)
}

pub fn hash_refresh_token(
    token: &str,
) -> std::result::Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2.hash_password(token.as_bytes(), &salt)?.to_string())
}

pub fn verify_refresh_token(token: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(token.as_bytes(), &parsed_hash)
        .is_ok()
}

pub fn generate_access_token(user_id: Uuid, session_id: Uuid, email: &str) -> Result<String> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let now = Utc::now().timestamp();
    let payload = AccessTokenClaims {
        sub: user_id.to_string(),
        email: email.to_string(),
        session_id: session_id.to_string(),
        exp: now + ACCESS_TOKEN_EXPIRY_SECS,
        iat: now,
    };

    encode(
        &Header::default(),
        &payload,
        &EncodingKey::from_secret(crate::config::get_jwt_secret().as_bytes()),
    )
    .map_err(AppError::from)
}

pub fn generate_refresh_token() -> String {
    generate_secure_token(REFRESH_TOKEN_SIZE)
}

pub fn decode_access_token(token: &str) -> Result<AccessTokenClaims> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let token_data = decode::<AccessTokenClaims>(
        token,
        &DecodingKey::from_secret(crate::config::get_jwt_secret().as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

pub fn generate_tokens(user_id: Uuid, email: &str) -> Result<(AccessToken, RefreshToken)> {
    let access_token_str = generate_access_token(user_id, Uuid::nil(), email)?;
    let refresh_token_str = generate_refresh_token();

    Ok((
        AccessToken {
            token: access_token_str,
            expires_in: ACCESS_TOKEN_EXPIRY_SECS,
        },
        RefreshToken {
            token: refresh_token_str,
            expires_in: REFRESH_TOKEN_EXPIRY_SECS,
        },
    ))
}

pub async fn create_session(
    db: &sqlx::PgPool,
    user_id: Uuid,
    email: &str,
    user_agent: Option<String>,
    ip_address: Option<String>,
) -> Result<AuthTokens> {
    let session_id = Uuid::new_v4();
    let refresh_token = generate_refresh_token();
    let refresh_token_hash =
        hash_refresh_token(&refresh_token).map_err(|e| AppError::Internal(e.to_string()))?;
    let expires_at = Utc::now() + Duration::seconds(REFRESH_TOKEN_EXPIRY_SECS);

    sqlx::query(
        r#"
        INSERT INTO sessions (id, user_id, refresh_token_hash, user_agent, ip_address, expires_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(&refresh_token_hash)
    .bind(&user_agent)
    .bind(&ip_address)
    .bind(expires_at)
    .execute(db)
    .await?;

    let access_token = generate_access_token(user_id, session_id, email)?;

    Ok(AuthTokens {
        access_token,
        refresh_token,
        expires_at,
    })
}

pub async fn validate_refresh_token(db: &sqlx::PgPool, token: &str) -> Result<Session> {
    let sessions = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE expires_at > NOW()")
        .fetch_all(db)
        .await?;

    let session = sessions
        .iter()
        .find(|s| verify_refresh_token(token, &s.refresh_token_hash))
        .ok_or_else(|| AppError::Unauthorized("Invalid refresh token".to_string()))?
        .clone();

    Ok(session)
}

pub async fn refresh_session(
    db: &sqlx::PgPool,
    refresh_token: &str,
    rotate: bool,
) -> Result<AuthTokens> {
    let session = validate_refresh_token(db, refresh_token).await?;

    let user = sqlx::query_as::<_, (Uuid, String)>("SELECT id, email FROM users WHERE id = $1")
        .bind(session.user_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let new_access_token = generate_access_token(user.0, session.id, &user.1)?;

    if rotate {
        let new_refresh_token = generate_refresh_token();
        let new_refresh_token_hash = hash_refresh_token(&new_refresh_token)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let new_expires_at = Utc::now() + Duration::seconds(REFRESH_TOKEN_EXPIRY_SECS);

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
        .execute(db)
        .await?;

        Ok(AuthTokens {
            access_token: new_access_token,
            refresh_token: new_refresh_token,
            expires_at: new_expires_at,
        })
    } else {
        Ok(AuthTokens {
            access_token: new_access_token,
            refresh_token: refresh_token.to_string(),
            expires_at: session.expires_at,
        })
    }
}

pub async fn revoke_session(db: &sqlx::PgPool, session_id: Uuid, user_id: Uuid) -> Result<()> {
    let result = sqlx::query("DELETE FROM sessions WHERE id = $1 AND user_id = $2")
        .bind(session_id)
        .bind(user_id)
        .execute(db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Session not found".to_string()));
    }

    Ok(())
}

pub async fn revoke_token(db: &sqlx::PgPool, token: &str) -> Result<()> {
    let session = validate_refresh_token(db, token).await?;

    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(session.id)
        .execute(db)
        .await?;

    Ok(())
}

pub async fn revoke_all_sessions(db: &sqlx::PgPool, user_id: Uuid) -> Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(result.rows_affected())
}

pub async fn revoke_all_user_sessions(db: &sqlx::PgPool, user_id: Uuid) -> Result<u64> {
    revoke_all_sessions(db, user_id).await
}

pub async fn list_user_sessions(
    db: &sqlx::PgPool,
    user_id: Uuid,
    current_session_id: Option<Uuid>,
) -> Result<Vec<SessionInfo>> {
    let sessions = sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE user_id = $1 AND expires_at > NOW() ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(sessions
        .into_iter()
        .map(|s| SessionInfo {
            id: s.id,
            user_agent: s.user_agent,
            ip_address: s.ip_address,
            created_at: s.created_at,
            expires_at: s.expires_at,
            is_current: Some(s.id) == current_session_id,
        })
        .collect())
}

pub async fn get_session_by_id(db: &sqlx::PgPool, session_id: Uuid) -> Result<Option<Session>> {
    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(db)
        .await?;

    Ok(session)
}

pub async fn cleanup_expired_sessions(db: &sqlx::PgPool) -> Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
        .execute(db)
        .await?;

    Ok(result.rows_affected())
}
