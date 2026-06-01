// Auth service — Argon2 password hashing, JWT signing, API-key helpers, TOTP.
#![allow(dead_code)]

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::AppError;

// ─── TOTP (RFC 6238 / RFC 4226) ─────────────────────────────────────────────
//
// Decision: implemented inline using the `hmac` (0.12) + `sha1` (0.10) crates
// that are already (or are now) direct dependencies — avoids adding `totp-lite`
// or any other crate.  The algorithm is standard HMAC-SHA1 HOTP with a 30-second
// time step and a 6-digit code, which is compatible with Google Authenticator,
// Authy, and every RFC 6238-compliant authenticator app.

type HmacSha1 = Hmac<Sha1>;

/// Base32 alphabet used by RFC 4648 (upper-case, no padding check needed for
/// the length we generate — 20 bytes → 32 base32 chars, always padded evenly).
const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Encode raw bytes as RFC 4648 base32 (no padding).
pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buf: u64 = 0;
    let mut bits: u32 = 0;
    for &b in data {
        buf = (buf << 8) | b as u64;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(BASE32_ALPHABET[((buf >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(BASE32_ALPHABET[((buf << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

/// Decode RFC 4648 base32 (case-insensitive, ignores '=').
pub fn base32_decode(s: &str) -> Result<Vec<u8>, AppError> {
    let mut buf: u64 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::new();
    for c in s.chars() {
        if c == '=' {
            continue;
        }
        let val = BASE32_ALPHABET
            .iter()
            .position(|&x| x == c.to_ascii_uppercase() as u8)
            .ok_or_else(|| AppError::Validation(format!("Invalid base32 character: {c}")))?;
        buf = (buf << 5) | val as u64;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

/// Generate a fresh 20-byte (160-bit) TOTP secret, returned as base32.
pub fn generate_totp_secret() -> String {
    let secret: [u8; 20] = rand::thread_rng().gen();
    base32_encode(&secret)
}

/// Build an `otpauth://totp/…` URI suitable for a QR-code scan.
pub fn totp_uri(secret_b32: &str, account_name: &str, issuer: &str) -> String {
    format!(
        "otpauth://totp/{issuer}:{account}?secret={secret}&issuer={issuer}&algorithm=SHA1&digits=6&period=30",
        issuer = urlencoding::encode(issuer),
        account = urlencoding::encode(account_name),
        secret = secret_b32,
    )
}

/// Compute a 6-digit HOTP code for the given secret and counter.
fn hotp(secret_bytes: &[u8], counter: u64) -> Result<u32, AppError> {
    let mut mac = HmacSha1::new_from_slice(secret_bytes)
        .map_err(|_| AppError::Internal("HMAC key error".to_string()))?;
    mac.update(&counter.to_be_bytes());
    let result = mac.finalize().into_bytes();

    // Dynamic truncation (RFC 4226 §5.3)
    let offset = (result[19] & 0x0f) as usize;
    let code = ((result[offset] as u32 & 0x7f) << 24)
        | ((result[offset + 1] as u32) << 16)
        | ((result[offset + 2] as u32) << 8)
        | (result[offset + 3] as u32);
    Ok(code % 1_000_000)
}

/// Verify a 6-digit TOTP code against the stored base32 secret.
/// Accepts a 1-step window (±30 s) for clock skew.
pub fn verify_totp(secret_b32: &str, code: &str) -> Result<bool, AppError> {
    let trimmed = code.trim();
    if trimmed.len() != 6 {
        return Ok(false);
    }
    let provided: u32 = trimmed
        .parse()
        .map_err(|_| AppError::Validation("TOTP code must be 6 digits".to_string()))?;

    let secret_bytes = base32_decode(secret_b32)?;
    let step = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 30;

    for delta in [0i64, -1, 1] {
        let t = (step as i64 + delta) as u64;
        if hotp(&secret_bytes, t)? == provided {
            return Ok(true);
        }
    }
    Ok(false)
}

// ─── API Key helpers ─────────────────────────────────────────────────────────
//
// Keys are generated as 32 random bytes, hex-encoded → 64-char strings.
// We store only an SHA-256 hash.  The prefix (first 8 chars) is stored
// alongside the hash so listings can show "mgt_abc12345…" without exposing
// the secret.

/// Generate a random API key.  Returns `(plaintext, prefix, sha256_hex_hash)`.
pub fn generate_api_key() -> (String, String, String) {
    let raw: [u8; 32] = rand::thread_rng().gen();
    let key = format!("mgt_{}", hex::encode(raw));
    let prefix = key[..8].to_string();
    let hash = hex::encode(Sha256::digest(key.as_bytes()));
    (key, prefix, hash)
}

/// Hash a plaintext API key for lookup.
pub fn hash_api_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub wallet_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub async fn get_user_by_email(db: &sqlx::PgPool, email: &str) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(db)
        .await?;

    Ok(user)
}

pub async fn get_user_by_id(db: &sqlx::PgPool, id: Uuid) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await?;

    Ok(user)
}
