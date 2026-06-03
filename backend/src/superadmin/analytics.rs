// In-house request analytics.
//
// `record_analytics` is a tower middleware mounted on the whole app. For every
// non-infrastructure request it captures method/path/status/latency, the
// best-effort client IP (enriched offline via the GeoIP resolver), and the
// authenticated user (decoded best-effort from the bearer token). The write is
// fire-and-forget on a spawned task so it never adds latency to the response.
//
// The query helpers below back the super-admin Analytics page.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{ConnectInfo, State},
    http::Request,
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::geo::GeoResolver;

pub struct AnalyticsState {
    pub pool: PgPool,
    pub geo: Arc<GeoResolver>,
    pub trust_proxy: bool,
    pub enabled: bool,
    /// Fraction of successful (<400) requests to persist; errors are always kept.
    pub sample_rate: f64,
    /// Header carrying a CDN-resolved country code (e.g. Cloudflare `CF-IPCountry`),
    /// used as a geo fallback when no GeoIP database resolves the IP.
    pub country_header: String,
}

impl AnalyticsState {
    pub fn from_env(pool: PgPool, geo: Arc<GeoResolver>, trust_proxy: bool) -> Self {
        let enabled = std::env::var("ANALYTICS_ENABLED")
            .map(|v| v != "false")
            .unwrap_or(true);
        let sample_rate = std::env::var("ANALYTICS_SAMPLE_RATE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|r| r.clamp(0.0, 1.0))
            .unwrap_or(1.0);
        let country_header = std::env::var("GEO_COUNTRY_HEADER")
            .ok()
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_else(|| "cf-ipcountry".to_string())
            .to_lowercase();
        Self {
            pool,
            geo,
            trust_proxy,
            enabled,
            sample_rate,
            country_header,
        }
    }
}

/// CDN-provided 2-letter country code, normalised. Ignores placeholders like
/// `XX`/`T1` that CDNs use for unknown/anonymised sources.
fn cdn_country(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(name)?.to_str().ok()?.trim().to_uppercase();
    if raw.len() == 2 && raw != "XX" && raw != "T1" && raw.chars().all(|c| c.is_ascii_alphabetic())
    {
        Some(raw)
    } else {
        None
    }
}

/// True for paths we never want to record (infra, the panel itself, static assets).
fn skip_path(path: &str) -> bool {
    path.starts_with("/superadmin")
        || path.starts_with("/metrics")
        || path.starts_with("/health")
        || path.starts_with("/assets")
        || path.starts_with("/favicon")
        || path == "/robots.txt"
}

pub async fn record_analytics(
    State(state): State<Arc<AnalyticsState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if !state.enabled {
        return next.run(request).await;
    }

    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();
    let headers = request.headers().clone();

    // Skip infra/static and CORS preflight before doing any work.
    if method == "OPTIONS" || skip_path(&path) {
        return next.run(request).await;
    }

    let ip = super::auth::client_ip(&headers, Some(peer.ip()), state.trust_proxy);
    let user_id = best_effort_user(&headers);
    let user_agent = header_str(&headers, "user-agent");
    let referer = header_str(&headers, "referer");
    // CDN-resolved country (only trusted behind a proxy) — geo fallback.
    let cdn_country = if state.trust_proxy {
        cdn_country(&headers, &state.country_header)
    } else {
        None
    };

    let started = Instant::now();
    let response = next.run(request).await;
    let duration_ms = started.elapsed().as_millis().min(i32::MAX as u128) as i32;
    let status = response.status().as_u16() as i32;

    // Sampling: always keep errors (>=400); sample the rest at sample_rate.
    if status < 400 && state.sample_rate < 1.0 && rand::random::<f64>() >= state.sample_rate {
        return response;
    }

    // Fire-and-forget: geo lookup + insert off the request path.
    let state2 = Arc::clone(&state);
    tokio::spawn(async move {
        let loc = state2.geo.lookup(&ip);
        let country = loc.country.or(cdn_country);
        let ip_opt = if ip.is_empty() { None } else { Some(ip) };
        let _ = sqlx::query(
            "INSERT INTO analytics_events
               (ip, country, region, city, method, path, status, duration_ms, user_id, user_agent, referer)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(ip_opt)
        .bind(country)
        .bind(loc.region)
        .bind(loc.city)
        .bind(method)
        .bind(path)
        .bind(status)
        .bind(duration_ms)
        .bind(user_id)
        .bind(user_agent)
        .bind(referer)
        .execute(&state2.pool)
        .await;
    });

    response
}

fn header_str(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(512).collect())
}

/// Best-effort decode of the authenticated user from the bearer token. Returns
/// `None` for anonymous/invalid requests — never errors.
fn best_effort_user(headers: &axum::http::HeaderMap) -> Option<Uuid> {
    let token = crate::api::middleware::extract_token_from_header(headers).ok()?;
    let claims = crate::api::middleware::validate_token(&token).ok()?;
    Uuid::parse_str(&claims.sub).ok()
}

// ── Query helpers for the Analytics page ────────────────────────────────────

#[derive(Debug, FromRow)]
pub struct AnalyticsOverview {
    pub total: i64,
    pub last_24h: i64,
    pub unique_ips: i64,
    pub unique_users: i64,
    pub error_rate_pct: f64,
    pub avg_duration_ms: f64,
}

pub async fn overview(pool: &PgPool) -> AnalyticsOverview {
    sqlx::query_as::<_, AnalyticsOverview>(
        "SELECT
           COUNT(*)::bigint AS total,
           COUNT(*) FILTER (WHERE occurred_at > NOW() - INTERVAL '24 hours')::bigint AS last_24h,
           COUNT(DISTINCT ip)::bigint AS unique_ips,
           COUNT(DISTINCT user_id)::bigint AS unique_users,
           COALESCE(100.0 * COUNT(*) FILTER (WHERE status >= 500) / NULLIF(COUNT(*),0), 0)::float8 AS error_rate_pct,
           COALESCE(AVG(duration_ms), 0)::float8 AS avg_duration_ms
         FROM analytics_events",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(AnalyticsOverview {
        total: 0,
        last_24h: 0,
        unique_ips: 0,
        unique_users: 0,
        error_rate_pct: 0.0,
        avg_duration_ms: 0.0,
    })
}

#[derive(Debug, FromRow)]
pub struct LabelCount {
    pub label: Option<String>,
    pub count: i64,
}

pub async fn top_countries(pool: &PgPool, limit: i64) -> Vec<LabelCount> {
    sqlx::query_as::<_, LabelCount>(
        "SELECT COALESCE(country, 'Unknown') AS label, COUNT(*)::bigint AS count
         FROM analytics_events
         GROUP BY country ORDER BY count DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn top_paths(pool: &PgPool, limit: i64) -> Vec<LabelCount> {
    sqlx::query_as::<_, LabelCount>(
        "SELECT path AS label, COUNT(*)::bigint AS count
         FROM analytics_events
         GROUP BY path ORDER BY count DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

#[derive(Debug, FromRow)]
pub struct DailyCount {
    pub day: DateTime<Utc>,
    pub count: i64,
}

pub async fn requests_by_day(pool: &PgPool, days: i64) -> Vec<DailyCount> {
    sqlx::query_as::<_, DailyCount>(
        "SELECT DATE_TRUNC('day', occurred_at) AS day, COUNT(*)::bigint AS count
         FROM analytics_events
         WHERE occurred_at > NOW() - ($1 || ' days')::interval
         GROUP BY 1 ORDER BY 1",
    )
    .bind(days.to_string())
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

#[derive(Debug, FromRow)]
pub struct RecentEvent {
    pub occurred_at: DateTime<Utc>,
    pub ip: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub method: String,
    pub path: String,
    pub status: i32,
    pub duration_ms: Option<i32>,
    pub username: Option<String>,
}

pub async fn recent_events(pool: &PgPool, limit: i64) -> Vec<RecentEvent> {
    sqlx::query_as::<_, RecentEvent>(
        "SELECT a.occurred_at, a.ip, a.country, a.city, a.method, a.path, a.status,
                a.duration_ms, u.username
         FROM analytics_events a
         LEFT JOIN users u ON u.id = a.user_id
         ORDER BY a.occurred_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

#[derive(Debug, FromRow)]
pub struct TopUser {
    pub username: Option<String>,
    pub count: i64,
    pub last_seen: DateTime<Utc>,
}

pub async fn top_users(pool: &PgPool, limit: i64) -> Vec<TopUser> {
    sqlx::query_as::<_, TopUser>(
        "SELECT u.username, COUNT(*)::bigint AS count, MAX(a.occurred_at) AS last_seen
         FROM analytics_events a
         JOIN users u ON u.id = a.user_id
         GROUP BY u.username ORDER BY count DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}
