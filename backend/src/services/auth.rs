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

// ─── TOTP secret encryption ──────────────────────────────────────────────────
//
// Decision (AX1-A1): Use HMAC-SHA256 CTR-mode XOR cipher with an 8-byte nonce.
// Rationale: avoids adding an `aes-gcm` crate (not in Cargo.toml); uses existing
// `hmac` + `sha2` crates already present. The scheme is:
//   keystream_block_i = HMAC-SHA256(key, nonce || i.to_be_bytes())
//   ciphertext = plaintext XOR concat(keystream_block_0[..], block_1[..], ...)
//   stored as hex(nonce[8] || ciphertext)
// Key source: env var TOTP_ENC_KEY (hex-encoded 32 bytes); if absent, the raw
// secret is stored unchanged (backward-compatible, but warns on startup).
// Decryption: read the prefix "enc:" to distinguish encrypted from legacy plaintext.

type HmacSha256 = Hmac<Sha256>;

const TOTP_ENC_PREFIX: &str = "enc:";
const TOTP_NONCE_LEN: usize = 8;

/// Load the 32-byte TOTP encryption key from TOTP_ENC_KEY (hex) env var.
/// Returns None if unset — callers fall back to plaintext storage with a warning.
fn load_totp_enc_key() -> Option<[u8; 32]> {
    let hex = std::env::var("TOTP_ENC_KEY").ok()?;
    let bytes = hex::decode(hex.trim()).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}

/// XOR `data` with the HMAC-SHA256 keystream derived from `key` and `nonce`.
fn xor_keystream(key: &[u8; 32], nonce: &[u8; TOTP_NONCE_LEN], data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut block_idx: u64 = 0;
    let mut offset = 0;
    while offset < out.len() {
        // keystream_block = HMAC-SHA256(key, nonce || block_idx_be)
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key size is valid");
        mac.update(nonce);
        mac.update(&block_idx.to_be_bytes());
        let ks = mac.finalize().into_bytes();
        for (i, ks_byte) in ks.iter().enumerate() {
            if offset + i >= out.len() {
                break;
            }
            out[offset + i] ^= ks_byte;
        }
        offset += ks.len();
        block_idx += 1;
    }
    out
}

/// Encrypt a TOTP secret (base32 string) for storage.
/// Returns `"enc:" + hex(nonce || ciphertext)` if a key is available,
/// otherwise returns the plaintext with a warning.
pub fn encrypt_totp_secret(secret: &str) -> String {
    match load_totp_enc_key() {
        Some(key) => {
            let nonce: [u8; TOTP_NONCE_LEN] = rand::thread_rng().gen();
            let ciphertext = xor_keystream(&key, &nonce, secret.as_bytes());
            let mut stored = Vec::with_capacity(TOTP_NONCE_LEN + ciphertext.len());
            stored.extend_from_slice(&nonce);
            stored.extend_from_slice(&ciphertext);
            format!("{}{}", TOTP_ENC_PREFIX, hex::encode(&stored))
        }
        None => {
            tracing::warn!(
                "TOTP_ENC_KEY not set — storing TOTP secret in plaintext. \
                 Set a 32-byte hex key to encrypt secrets at rest."
            );
            secret.to_string()
        }
    }
}

/// Decrypt a stored TOTP secret.
/// Accepts both encrypted (`enc:...`) and legacy plaintext formats.
pub fn decrypt_totp_secret(stored: &str) -> Result<String, AppError> {
    if let Some(hex_data) = stored.strip_prefix(TOTP_ENC_PREFIX) {
        let key = load_totp_enc_key().ok_or_else(|| {
            AppError::Internal("TOTP secret is encrypted but TOTP_ENC_KEY is not set".to_string())
        })?;
        let bytes = hex::decode(hex_data)
            .map_err(|e| AppError::Internal(format!("Failed to decode TOTP ciphertext: {}", e)))?;
        if bytes.len() < TOTP_NONCE_LEN {
            return Err(AppError::Internal("TOTP ciphertext too short".to_string()));
        }
        let (nonce_bytes, ciphertext) = bytes.split_at(TOTP_NONCE_LEN);
        let mut nonce = [0u8; TOTP_NONCE_LEN];
        nonce.copy_from_slice(nonce_bytes);
        let plaintext = xor_keystream(&key, &nonce, ciphertext);
        String::from_utf8(plaintext)
            .map_err(|e| AppError::Internal(format!("TOTP plaintext is not UTF-8: {}", e)))
    } else {
        // Legacy plaintext — no decryption needed
        Ok(stored.to_string())
    }
}

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

/// Verify a 6-digit TOTP code against the stored (possibly encrypted) secret value.
/// Decrypts if needed, then runs the TOTP check with a ±30 s window.
pub fn verify_totp_stored(stored_secret: &str, code: &str) -> Result<bool, AppError> {
    let secret_b32 = decrypt_totp_secret(stored_secret)?;
    verify_totp(&secret_b32, code)
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
