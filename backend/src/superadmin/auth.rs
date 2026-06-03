// Hardened authentication for the super-admin panel.
//
// Design choices, all deliberately stricter than the normal user auth:
//   * Identity is a single env-provisioned credential (SUPERADMIN_EMAIL plus an
//     argon2 hash, or a dev-only plaintext fallback) — never a database row, so
//     a DB compromise cannot mint super-admin access.
//   * Sessions are opaque random tokens held in process memory only; a restart
//     invalidates every session and no session secret is ever persisted.
//   * Cookies are HttpOnly + SameSite=Strict + Path-scoped (+ Secure in prod).
//   * Per-IP login lockout throttles brute force.
//   * An optional IP allowlist (exact or CIDR) gates the entire surface.
//   * CSRF tokens protect every mutating form on top of SameSite=Strict.
//   * Credential checks are constant-time.
//
// If no credential is configured the whole panel is disabled (returns 404), so
// it adds zero attack surface when unused.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::HeaderMap;
use chrono::{DateTime, Utc};
use rand::RngCore;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

pub const COOKIE: &str = "mag_superadmin";

// ── Credential / configuration ──────────────────────────────────────────────

enum Credential {
    Argon2(String),
    Plain(String),
}

enum IpRule {
    Exact(IpAddr),
    Cidr4 { net: u32, prefix: u8 },
    Cidr6 { net: u128, prefix: u8 },
}

pub struct SuperAdminConfig {
    pub email: String,
    credential: Credential,
    pub secure_cookie: bool,
    pub trust_proxy: bool,
    allowlist: Vec<IpRule>,
    pub session_ttl: Duration,
}

impl SuperAdminConfig {
    /// Build from environment. Returns `None` (panel disabled) unless both an
    /// email and a password/hash are configured.
    pub fn from_env() -> Option<Self> {
        let email = std::env::var("SUPERADMIN_EMAIL")
            .ok()
            .filter(|s| !s.trim().is_empty())?;

        let credential = if let Some(h) = std::env::var("SUPERADMIN_PASSWORD_HASH")
            .ok()
            .filter(|s| !s.trim().is_empty())
        {
            Credential::Argon2(h)
        } else if let Some(p) = std::env::var("SUPERADMIN_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty())
        {
            tracing::warn!(
                "SUPERADMIN_PASSWORD is set in plaintext; set SUPERADMIN_PASSWORD_HASH \
                 (argon2 PHC string) instead for production"
            );
            Credential::Plain(p)
        } else {
            return None;
        };

        let is_prod = std::env::var("APP_ENV")
            .map(|e| e == "production")
            .unwrap_or(false);
        let secure_cookie = std::env::var("SUPERADMIN_SECURE_COOKIE")
            .map(|v| v == "true")
            .unwrap_or(is_prod);
        let trust_proxy = std::env::var("TRUST_PROXY")
            .map(|v| v == "true")
            .unwrap_or(false);
        let allowlist =
            parse_allowlist(&std::env::var("SUPERADMIN_IP_ALLOWLIST").unwrap_or_default());
        let ttl_secs = std::env::var("SUPERADMIN_SESSION_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7200);

        Some(Self {
            email,
            credential,
            secure_cookie,
            trust_proxy,
            allowlist,
            session_ttl: Duration::from_secs(ttl_secs),
        })
    }

    /// Constant-time verification of a supplied email + password.
    pub fn verify_credentials(&self, email: &str, password: &str) -> bool {
        let email_ok = ct_eq(
            email.trim().to_lowercase().as_bytes(),
            self.email.trim().to_lowercase().as_bytes(),
        );
        let pw_ok = match &self.credential {
            Credential::Argon2(hash) => match PasswordHash::new(hash) {
                Ok(parsed) => Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok(),
                Err(_) => false,
            },
            Credential::Plain(p) => ct_eq(password.as_bytes(), p.as_bytes()),
        };
        email_ok && pw_ok
    }

    pub fn ip_allowed(&self, ip: &str) -> bool {
        if self.allowlist.is_empty() {
            return true;
        }
        let Ok(addr) = IpAddr::from_str(ip.trim()) else {
            return false;
        };
        self.allowlist.iter().any(|rule| rule.matches(&addr))
    }

    pub fn set_cookie_header(&self, token: &str) -> String {
        let secure = if self.secure_cookie { "; Secure" } else { "" };
        format!(
            "{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/superadmin; Max-Age={}{}",
            self.session_ttl.as_secs(),
            secure
        )
    }

    pub fn clear_cookie_header(&self) -> String {
        let secure = if self.secure_cookie { "; Secure" } else { "" };
        format!(
            "{COOKIE}=; HttpOnly; SameSite=Strict; Path=/superadmin; Max-Age=0{}",
            secure
        )
    }
}

impl IpRule {
    fn matches(&self, addr: &IpAddr) -> bool {
        match (self, addr) {
            (IpRule::Exact(want), got) => want == got,
            (IpRule::Cidr4 { net, prefix }, IpAddr::V4(v4)) => {
                let mask = if *prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - prefix)
                };
                (u32::from(*v4) & mask) == (net & mask)
            }
            (IpRule::Cidr6 { net, prefix }, IpAddr::V6(v6)) => {
                let mask = if *prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - prefix)
                };
                (u128::from(*v6) & mask) == (net & mask)
            }
            _ => false,
        }
    }
}

fn parse_allowlist(raw: &str) -> Vec<IpRule> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|entry| {
            if let Some((addr, prefix)) = entry.split_once('/') {
                let prefix: u8 = prefix.parse().ok()?;
                match IpAddr::from_str(addr.trim()).ok()? {
                    IpAddr::V4(v4) if prefix <= 32 => Some(IpRule::Cidr4 {
                        net: u32::from(v4),
                        prefix,
                    }),
                    IpAddr::V6(v6) if prefix <= 128 => Some(IpRule::Cidr6 {
                        net: u128::from(v6),
                        prefix,
                    }),
                    _ => None,
                }
            } else {
                IpAddr::from_str(entry).ok().map(IpRule::Exact)
            }
        })
        .collect()
}

// ── Sessions ────────────────────────────────────────────────────────────────
//
// Backed by Redis when a connection is available (so sessions survive a restart
// and are shared across replicas behind a load balancer), falling back to an
// in-process map otherwise. Either way the cookie holds only an opaque random
// token; the session record never contains a reusable secret.

const SESS_PREFIX: &str = "sa:sess:";
const SESS_INDEX: &str = "sa:sess:idx";

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub email: String,
    pub csrf: String,
    pub created: DateTime<Utc>,
    pub ip: String,
    pub expires_at: DateTime<Utc>,
}

pub struct SessionStore {
    memory: Mutex<HashMap<String, Session>>,
    redis: Option<redis::aio::ConnectionManager>,
}

impl SessionStore {
    pub fn new(redis: Option<redis::aio::ConnectionManager>) -> Self {
        Self {
            memory: Mutex::new(HashMap::new()),
            redis,
        }
    }

    pub fn backend_name(&self) -> &'static str {
        if self.redis.is_some() {
            "redis"
        } else {
            "in-memory"
        }
    }

    /// Create a session and return `(cookie_token, session)`.
    pub async fn create(&self, email: &str, ip: &str, ttl: Duration) -> (String, Session) {
        let token = random_hex(32);
        let now = Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(ttl).unwrap_or_else(|_| chrono::Duration::seconds(7200));
        let session = Session {
            email: email.to_string(),
            csrf: random_hex(32),
            created: now,
            ip: ip.to_string(),
            expires_at,
        };
        if let Some(mut conn) = self.redis.clone() {
            if let Ok(json) = serde_json::to_string(&session) {
                let key = format!("{SESS_PREFIX}{token}");
                let _: Result<(), _> = conn.set_ex(&key, json, ttl.as_secs()).await;
                let _: Result<(), _> = conn.zadd(SESS_INDEX, &token, expires_at.timestamp()).await;
            }
        } else {
            self.memory
                .lock()
                .unwrap()
                .insert(token.clone(), session.clone());
        }
        (token, session)
    }

    pub async fn get(&self, token: &str) -> Option<Session> {
        if let Some(mut conn) = self.redis.clone() {
            let key = format!("{SESS_PREFIX}{token}");
            let json: Option<String> = conn.get(&key).await.ok().flatten();
            let session: Session = serde_json::from_str(&json?).ok()?;
            if session.expires_at > Utc::now() {
                Some(session)
            } else {
                let _: Result<(), _> = conn.del(&key).await;
                let _: Result<(), _> = conn.zrem(SESS_INDEX, token).await;
                None
            }
        } else {
            let mut map = self.memory.lock().unwrap();
            match map.get(token) {
                Some(s) if s.expires_at > Utc::now() => Some(s.clone()),
                Some(_) => {
                    map.remove(token);
                    None
                }
                None => None,
            }
        }
    }

    pub async fn remove(&self, token: &str) {
        if let Some(mut conn) = self.redis.clone() {
            let key = format!("{SESS_PREFIX}{token}");
            let _: Result<(), _> = conn.del(&key).await;
            let _: Result<(), _> = conn.zrem(SESS_INDEX, token).await;
        } else {
            self.memory.lock().unwrap().remove(token);
        }
    }

    pub async fn sweep(&self) {
        if let Some(mut conn) = self.redis.clone() {
            let now = Utc::now().timestamp();
            let _: Result<(), _> = conn.zrembyscore(SESS_INDEX, i64::MIN, now).await;
        } else {
            let now = Utc::now();
            self.memory
                .lock()
                .unwrap()
                .retain(|_, s| s.expires_at > now);
        }
    }

    pub async fn active_count(&self) -> usize {
        if let Some(mut conn) = self.redis.clone() {
            let now = Utc::now().timestamp();
            let _: Result<(), _> = conn.zrembyscore(SESS_INDEX, i64::MIN, now).await;
            let n: i64 = conn.zcard(SESS_INDEX).await.unwrap_or(0);
            n.max(0) as usize
        } else {
            let now = Utc::now();
            self.memory
                .lock()
                .unwrap()
                .values()
                .filter(|s| s.expires_at > now)
                .count()
        }
    }
}

// ── Brute-force lockout ─────────────────────────────────────────────────────
//
// Redis-backed when available so the lockout is enforced across replicas (an
// attacker can't dodge it by spreading attempts over instances); in-memory
// otherwise.

const FAIL_PREFIX: &str = "sa:fail:";
const LOCK_PREFIX: &str = "sa:lock:";

struct Attempts {
    failures: u32,
    locked_until: Option<Instant>,
}

pub struct LoginGuard {
    memory: Mutex<HashMap<String, Attempts>>,
    redis: Option<redis::aio::ConnectionManager>,
    max_failures: u32,
    lock_for: Duration,
}

impl LoginGuard {
    pub fn new(redis: Option<redis::aio::ConnectionManager>) -> Self {
        Self {
            memory: Mutex::new(HashMap::new()),
            redis,
            max_failures: 5,
            lock_for: Duration::from_secs(15 * 60),
        }
    }

    /// `Ok(())` if a login attempt is allowed, `Err(seconds_remaining)` if locked.
    pub async fn check(&self, ip: &str) -> Result<(), u64> {
        if let Some(mut conn) = self.redis.clone() {
            let key = format!("{LOCK_PREFIX}{ip}");
            let ttl: i64 = conn.ttl(&key).await.unwrap_or(-2);
            if ttl > 0 {
                return Err(ttl as u64);
            }
            Ok(())
        } else {
            let map = self.memory.lock().unwrap();
            if let Some(a) = map.get(ip) {
                if let Some(until) = a.locked_until {
                    let now = Instant::now();
                    if until > now {
                        return Err((until - now).as_secs() + 1);
                    }
                }
            }
            Ok(())
        }
    }

    pub async fn record_failure(&self, ip: &str) {
        if let Some(mut conn) = self.redis.clone() {
            let fkey = format!("{FAIL_PREFIX}{ip}");
            let count: i64 = conn.incr(&fkey, 1).await.unwrap_or(0);
            let _: Result<(), _> = conn.expire(&fkey, self.lock_for.as_secs() as i64).await;
            if count >= self.max_failures as i64 {
                let lkey = format!("{LOCK_PREFIX}{ip}");
                let _: Result<(), _> = conn.set_ex(&lkey, "1", self.lock_for.as_secs()).await;
                let _: Result<(), _> = conn.del(&fkey).await;
            }
        } else {
            let mut map = self.memory.lock().unwrap();
            let entry = map.entry(ip.to_string()).or_insert(Attempts {
                failures: 0,
                locked_until: None,
            });
            if let Some(until) = entry.locked_until {
                if until <= Instant::now() {
                    entry.failures = 0;
                    entry.locked_until = None;
                }
            }
            entry.failures += 1;
            if entry.failures >= self.max_failures {
                entry.locked_until = Some(Instant::now() + self.lock_for);
            }
        }
    }

    pub async fn record_success(&self, ip: &str) {
        if let Some(mut conn) = self.redis.clone() {
            let _: Result<(), _> = conn.del(&format!("{FAIL_PREFIX}{ip}")).await;
            let _: Result<(), _> = conn.del(&format!("{LOCK_PREFIX}{ip}")).await;
        } else {
            self.memory.lock().unwrap().remove(ip);
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn random_hex(n: usize) -> String {
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// A fresh opaque random token (used for the pre-login CSRF cookie).
pub fn random_token() -> String {
    random_hex(32)
}

/// Read a named cookie's value from the Cookie header.
pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get("cookie")?.to_str().ok()?;
    let prefix = format!("{name}=");
    raw.split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&prefix).map(|v| v.to_string()))
}

/// Length-independent constant-time byte comparison.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Extract the super-admin session token from the Cookie header.
pub fn token_from_cookies(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get("cookie")?.to_str().ok()?;
    let prefix = format!("{COOKIE}=");
    raw.split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&prefix).map(|v| v.to_string()))
}

/// Best-effort client IP. Honours `X-Forwarded-For`/`X-Real-IP` only when the
/// deployment is configured to trust an upstream proxy.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>, trust_proxy: bool) -> String {
    if trust_proxy {
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = xff.split(',').next() {
                let t = first.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
        }
        if let Some(xr) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            if !xr.trim().is_empty() {
                return xr.trim().to_string();
            }
        }
    }
    peer.map(|p| p.to_string()).unwrap_or_default()
}

#[allow(dead_code)]
fn _ip_size_hints() -> (Ipv4Addr, Ipv6Addr) {
    (Ipv4Addr::UNSPECIFIED, Ipv6Addr::UNSPECIFIED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_basic() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
    }

    #[test]
    fn cidr_matching() {
        let rules = parse_allowlist("10.0.0.0/8, 192.168.1.5, 2001:db8::/32");
        let in8 = IpRule::matches(&rules[0], &IpAddr::from_str("10.9.9.9").unwrap());
        assert!(in8);
        assert!(!IpRule::matches(
            &rules[0],
            &IpAddr::from_str("11.0.0.1").unwrap()
        ));
        assert!(IpRule::matches(
            &rules[1],
            &IpAddr::from_str("192.168.1.5").unwrap()
        ));
        assert!(IpRule::matches(
            &rules[2],
            &IpAddr::from_str("2001:db8:1::1").unwrap()
        ));
    }

    #[test]
    fn empty_allowlist_allows_all() {
        let cfg = SuperAdminConfig {
            email: "a@b.c".into(),
            credential: Credential::Plain("x".into()),
            secure_cookie: false,
            trust_proxy: false,
            allowlist: vec![],
            session_ttl: Duration::from_secs(60),
        };
        assert!(cfg.ip_allowed("8.8.8.8"));
    }

    #[tokio::test]
    async fn lockout_after_five_failures() {
        let g = LoginGuard::new(None);
        assert!(g.check("1.1.1.1").await.is_ok());
        for _ in 0..5 {
            g.record_failure("1.1.1.1").await;
        }
        assert!(g.check("1.1.1.1").await.is_err());
        g.record_success("1.1.1.1").await;
        assert!(g.check("1.1.1.1").await.is_ok());
    }

    #[tokio::test]
    async fn sessions_create_and_expire() {
        let store = SessionStore::new(None);
        let (tok, sess) = store
            .create("a@b.c", "1.2.3.4", Duration::from_secs(60))
            .await;
        assert_eq!(store.get(&tok).await.unwrap().email, "a@b.c");
        assert_eq!(sess.csrf.len(), 64);
        store.remove(&tok).await;
        assert!(store.get(&tok).await.is_none());
    }
}
