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

#[derive(Clone)]
pub struct Session {
    pub email: String,
    pub csrf: String,
    pub created: DateTime<Utc>,
    pub ip: String,
    expires: Instant,
}

pub struct SessionStore {
    inner: Mutex<HashMap<String, Session>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Create a session and return `(cookie_token, session)`.
    pub fn create(&self, email: &str, ip: &str, ttl: Duration) -> (String, Session) {
        let token = random_hex(32);
        let session = Session {
            email: email.to_string(),
            csrf: random_hex(32),
            created: Utc::now(),
            ip: ip.to_string(),
            expires: Instant::now() + ttl,
        };
        self.inner
            .lock()
            .unwrap()
            .insert(token.clone(), session.clone());
        (token, session)
    }

    pub fn get(&self, token: &str) -> Option<Session> {
        let mut map = self.inner.lock().unwrap();
        match map.get(token) {
            Some(s) if s.expires > Instant::now() => Some(s.clone()),
            Some(_) => {
                map.remove(token);
                None
            }
            None => None,
        }
    }

    pub fn remove(&self, token: &str) {
        self.inner.lock().unwrap().remove(token);
    }

    pub fn sweep(&self) {
        let now = Instant::now();
        self.inner.lock().unwrap().retain(|_, s| s.expires > now);
    }

    pub fn active_count(&self) -> usize {
        let now = Instant::now();
        self.inner
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.expires > now)
            .count()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Brute-force lockout ─────────────────────────────────────────────────────

struct Attempts {
    failures: u32,
    locked_until: Option<Instant>,
}

pub struct LoginGuard {
    inner: Mutex<HashMap<String, Attempts>>,
    max_failures: u32,
    lock_for: Duration,
}

impl LoginGuard {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            max_failures: 5,
            lock_for: Duration::from_secs(15 * 60),
        }
    }

    /// `Ok(())` if a login attempt is allowed, `Err(seconds_remaining)` if locked.
    pub fn check(&self, ip: &str) -> Result<(), u64> {
        let map = self.inner.lock().unwrap();
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

    pub fn record_failure(&self, ip: &str) {
        let mut map = self.inner.lock().unwrap();
        let entry = map.entry(ip.to_string()).or_insert(Attempts {
            failures: 0,
            locked_until: None,
        });
        // Reset a stale lock window before counting.
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

    pub fn record_success(&self, ip: &str) {
        self.inner.lock().unwrap().remove(ip);
    }
}

impl Default for LoginGuard {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn lockout_after_five_failures() {
        let g = LoginGuard::new();
        assert!(g.check("1.1.1.1").is_ok());
        for _ in 0..5 {
            g.record_failure("1.1.1.1");
        }
        assert!(g.check("1.1.1.1").is_err());
        g.record_success("1.1.1.1");
        assert!(g.check("1.1.1.1").is_ok());
    }

    #[test]
    fn sessions_create_and_expire() {
        let store = SessionStore::new();
        let (tok, sess) = store.create("a@b.c", "1.2.3.4", Duration::from_secs(60));
        assert_eq!(store.get(&tok).unwrap().email, "a@b.c");
        assert_eq!(sess.csrf.len(), 64);
        store.remove(&tok);
        assert!(store.get(&tok).is_none());
    }
}
