// Verification service — email/password-reset tokens, wired to auth flow.
#![allow(dead_code)]

use chrono::{DateTime, Duration, Utc};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[derive(Default)]
pub enum VerificationLevel {
    #[default]
    Unverified,
    EmailVerified,
    PhoneVerified,
    KYCVerified,
}


impl std::fmt::Display for VerificationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationLevel::Unverified => write!(f, "unverified"),
            VerificationLevel::EmailVerified => write!(f, "email_verified"),
            VerificationLevel::PhoneVerified => write!(f, "phone_verified"),
            VerificationLevel::KYCVerified => write!(f, "kyc_verified"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationTokenType {
    EmailVerification,
    PasswordReset,
}

impl VerificationTokenType {
    pub fn as_str(&self) -> &'static str {
        match self {
            VerificationTokenType::EmailVerification => "email_verification",
            VerificationTokenType::PasswordReset => "password_reset",
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct VerificationToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub token_type: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

const VERIFICATION_TOKEN_SIZE: usize = 32;
const EMAIL_VERIFICATION_TOKEN_EXPIRY_SECS: i64 = 24 * 60 * 60;
const PASSWORD_RESET_TOKEN_EXPIRY_SECS: i64 = 60 * 60;

pub fn generate_secure_token(size: usize) -> String {
    let mut rng = ChaCha20Rng::from_entropy();
    let bytes: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
    base64::encode_config(&bytes, base64::URL_SAFE_NO_PAD)
}

pub fn generate_verification_token() -> String {
    generate_secure_token(VERIFICATION_TOKEN_SIZE)
}

pub async fn create_verification_token(
    db: &sqlx::PgPool,
    user_id: Uuid,
    token_type: VerificationTokenType,
) -> Result<String> {
    let token = generate_verification_token();
    let expires_at = Utc::now()
        + Duration::seconds(match token_type {
            VerificationTokenType::EmailVerification => EMAIL_VERIFICATION_TOKEN_EXPIRY_SECS,
            VerificationTokenType::PasswordReset => PASSWORD_RESET_TOKEN_EXPIRY_SECS,
        });

    sqlx::query(
        r#"
        INSERT INTO verification_tokens (id, user_id, token, token_type, expires_at, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(&token)
    .bind(token_type.as_str())
    .bind(expires_at)
    .execute(db)
    .await?;

    Ok(token)
}

pub async fn generate_email_verification_token(db: &sqlx::PgPool, user_id: Uuid) -> Result<String> {
    create_verification_token(db, user_id, VerificationTokenType::EmailVerification).await
}

pub async fn generate_password_reset_token(db: &sqlx::PgPool, user_id: Uuid) -> Result<String> {
    create_verification_token(db, user_id, VerificationTokenType::PasswordReset).await
}

pub async fn verify_email_token(db: &sqlx::PgPool, token: &str) -> Result<Uuid> {
    let token_record = sqlx::query_as::<_, VerificationToken>(
        r#"
        SELECT * FROM verification_tokens
        WHERE token = $1 AND token_type = 'email_verification' AND used_at IS NULL AND expires_at > NOW()
        "#,
    )
    .bind(token)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid or expired verification token".to_string()))?;

    sqlx::query(
        r#"
        UPDATE verification_tokens SET used_at = NOW() WHERE id = $1
        "#,
    )
    .bind(token_record.id)
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        UPDATE users SET email_verified = true WHERE id = $1
        "#,
    )
    .bind(token_record.user_id)
    .execute(db)
    .await?;

    Ok(token_record.user_id)
}

pub async fn verify_password_reset_token(db: &sqlx::PgPool, token: &str) -> Result<Uuid> {
    let token_record = sqlx::query_as::<_, VerificationToken>(
        r#"
        SELECT * FROM verification_tokens
        WHERE token = $1 AND token_type = 'password_reset' AND used_at IS NULL AND expires_at > NOW()
        "#,
    )
    .bind(token)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid or expired reset token".to_string()))?;

    sqlx::query(
        r#"
        UPDATE verification_tokens SET used_at = NOW() WHERE id = $1
        "#,
    )
    .bind(token_record.id)
    .execute(db)
    .await?;

    Ok(token_record.user_id)
}

pub async fn get_user_verification_level(
    db: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<VerificationLevel> {
    let result = sqlx::query_as::<_, (bool, Option<String>)>(
        r#"
        SELECT email_verified, NULL as kyc_status FROM users WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    match result {
        Some((email_verified, _)) => {
            if email_verified {
                Ok(VerificationLevel::EmailVerified)
            } else {
                Ok(VerificationLevel::Unverified)
            }
        }
        None => Err(AppError::NotFound("User not found".to_string())),
    }
}

pub async fn cleanup_expired_tokens(db: &sqlx::PgPool) -> Result<u64> {
    let result =
        sqlx::query("DELETE FROM verification_tokens WHERE expires_at < NOW() AND used_at IS NULL")
            .execute(db)
            .await?;

    Ok(result.rows_affected())
}

pub async fn cleanup_used_tokens(db: &sqlx::PgPool, older_than_hours: i64) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM verification_tokens
        WHERE used_at IS NOT NULL AND created_at < NOW() - INTERVAL '1 hour' * $1
        "#,
    )
    .bind(older_than_hours)
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}

pub fn can_perform_transaction(level: VerificationLevel, transaction_type: &str) -> bool {
    match (level, transaction_type) {
        (VerificationLevel::Unverified, "read") => true,
        (VerificationLevel::Unverified, _) => false,
        (VerificationLevel::EmailVerified, _) => true,
        (VerificationLevel::PhoneVerified, _) => true,
        (VerificationLevel::KYCVerified, _) => true,
    }
}

pub fn get_transaction_limit(level: VerificationLevel) -> Option<(i64, String)> {
    match level {
        VerificationLevel::Unverified => Some((100_000, "cents".to_string())),
        VerificationLevel::EmailVerified => Some((10_000_000, "cents".to_string())),
        VerificationLevel::PhoneVerified => Some((100_000_000, "cents".to_string())),
        VerificationLevel::KYCVerified => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_token_type_as_str() {
        assert_eq!(
            VerificationTokenType::EmailVerification.as_str(),
            "email_verification"
        );
        assert_eq!(
            VerificationTokenType::PasswordReset.as_str(),
            "password_reset"
        );
    }

    #[test]
    fn test_generate_secure_token() {
        let token1 = generate_secure_token(32);
        let token2 = generate_secure_token(32);
        assert_ne!(token1, token2);
        assert_eq!(token1.len(), 43);
    }

    #[test]
    fn test_verification_level_display() {
        assert_eq!(VerificationLevel::Unverified.to_string(), "unverified");
        assert_eq!(
            VerificationLevel::EmailVerified.to_string(),
            "email_verified"
        );
        assert_eq!(
            VerificationLevel::PhoneVerified.to_string(),
            "phone_verified"
        );
        assert_eq!(VerificationLevel::KYCVerified.to_string(), "kyc_verified");
    }

    #[test]
    fn test_can_perform_transaction() {
        assert!(can_perform_transaction(
            VerificationLevel::Unverified,
            "read"
        ));
        assert!(!can_perform_transaction(
            VerificationLevel::Unverified,
            "write"
        ));
        assert!(can_perform_transaction(
            VerificationLevel::EmailVerified,
            "write"
        ));
    }

    #[test]
    fn test_get_transaction_limit() {
        assert_eq!(
            get_transaction_limit(VerificationLevel::Unverified),
            Some((100_000, "cents".to_string()))
        );
        assert_eq!(
            get_transaction_limit(VerificationLevel::EmailVerified),
            Some((10_000_000, "cents".to_string()))
        );
        assert_eq!(get_transaction_limit(VerificationLevel::KYCVerified), None);
    }
}
