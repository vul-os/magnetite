// Rate limiter middleware — Redis primary, in-memory fallback; config fields intentionally kept
// for future tuning even when not all are read by the current impl.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use redis::AsyncCommands;
use tokio::sync::Mutex;

pub struct RedisRateLimiter {
    client: redis::Client,
    default_limit: u32,
    window: Duration,
    fallback: Arc<RateLimiter>,
}

struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: u32,
    window: Duration,
}

impl RedisRateLimiter {
    pub fn new(redis_url: &str, default_limit: u32, window: Duration) -> Self {
        let client = redis::Client::open(redis_url).ok();
        Self {
            client: client.unwrap_or_else(|| {
                redis::Client::open("redis://localhost").expect("Valid redis URL")
            }),
            default_limit,
            window,
            fallback: Arc::new(RateLimiter {
                requests: Arc::new(Mutex::new(HashMap::new())),
                max_requests: default_limit,
                window,
            }),
        }
    }

    pub async fn check_rate_limit(&self, key: &str, limit: u32) -> bool {
        if let Some(mut conn) = self.client.get_multiplexed_async_connection().await.ok() {
            let _now = Instant::now();
            let window_ms = self.window.as_millis() as u64;
            let now_ms = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let window_start = now_ms.saturating_sub(window_ms);

            let redis_key = format!("ratelimit:{}", key);

            let lua_script = r#"
                local key = KEYS[1]
                local now = tonumber(ARGV[1])
                local window_start = tonumber(ARGV[2])
                local limit = tonumber(ARGV[3])
                local window_ms = tonumber(ARGV[4])

                redis.call('ZREMRANGEBYSCORE', key, 0, window_start)

                local count = redis.call('ZCARD', key)

                if count >= limit then
                    return 0
                end

                redis.call('ZADD', key, now, now .. ':' .. math.random())
                redis.call('PEXPIRE', key, window_ms)

                return 1
            "#;

            let result: Result<i32, _> = redis::Script::new(lua_script)
                .key(&redis_key)
                .arg(now_ms)
                .arg(window_start)
                .arg(limit)
                .arg(window_ms)
                .invoke_async(&mut conn)
                .await;

            match result {
                Ok(1) => return true,
                Ok(_) => return false,
                Err(_) => {}
            }
        }

        self.fallback.check_rate_limit_internal(key, limit).await
    }

    pub async fn get_remaining(&self, key: &str, limit: u32) -> u32 {
        if let Some(mut conn) = self.client.get_multiplexed_async_connection().await.ok() {
            let _now = Instant::now();
            let window_ms = self.window.as_millis() as u64;
            let now_ms = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let window_start = now_ms.saturating_sub(window_ms);

            let redis_key = format!("ratelimit:{}", key);

            let lua_script = r#"
                local key = KEYS[1]
                local window_start = tonumber(ARGV[1])

                redis.call('ZREMRANGEBYSCORE', key, 0, window_start)

                return redis.call('ZCARD', key)
            "#;

            let result: Result<u32, _> = redis::Script::new(lua_script)
                .key(&redis_key)
                .arg(window_start)
                .invoke_async(&mut conn)
                .await;

            if let Ok(count) = result {
                return limit.saturating_sub(count);
            }
        }

        self.fallback.get_remaining_internal(key, limit).await
    }

    pub async fn reset(&self, key: &str) {
        if let Some(mut conn) = self.client.get_multiplexed_async_connection().await.ok() {
            let redis_key = format!("ratelimit:{}", key);
            let _: Result<(), _> = conn.del(&redis_key).await;
        }

        self.fallback.reset_internal(key).await;
    }
}

impl RateLimiter {
    async fn check_rate_limit_internal(&self, key: &str, limit: u32) -> bool {
        let mut requests = self.requests.lock().await;
        let now = Instant::now();
        let window_start = now - self.window;

        let timestamps = requests.entry(key.to_string()).or_insert_with(Vec::new);
        timestamps.retain(|&t| t > window_start);

        if timestamps.len() >= limit as usize {
            return false;
        }

        timestamps.push(now);
        true
    }

    async fn get_remaining_internal(&self, key: &str, limit: u32) -> u32 {
        let requests = self.requests.lock().await;
        let now = Instant::now();
        let window_start = now - self.window;

        let timestamps = requests
            .get(key)
            .map(|v| v.iter().filter(|&&t| t > window_start).count())
            .unwrap_or(0);

        limit.saturating_sub(timestamps as u32)
    }

    async fn reset_internal(&self, key: &str) {
        let mut requests = self.requests.lock().await;
        requests.remove(key);
    }
}

pub struct RateLimitConfig {
    pub auth: (u32, Duration),
    pub wallet: (u32, Duration),
    pub games: (u32, Duration),
    pub default: (u32, Duration),
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            auth: (5, Duration::from_secs(60)),
            wallet: (30, Duration::from_secs(60)),
            games: (100, Duration::from_secs(60)),
            default: (200, Duration::from_secs(60)),
        }
    }
}

pub type SharedRateLimiter = Arc<RedisRateLimiter>;

pub fn create_rate_limiter(redis_url: &str, config: RateLimitConfig) -> SharedRateLimiter {
    Arc::new(RedisRateLimiter::new(
        redis_url,
        config.default.0,
        config.default.1,
    ))
}

pub fn get_rate_limit_config(path: &str) -> (u32, Duration) {
    if path.starts_with("/api/auth/") {
        (5, Duration::from_secs(60))
    } else if path.starts_with("/api/wallet/") {
        (30, Duration::from_secs(60))
    } else if path == "/api/games" || path.starts_with("/api/games") {
        (100, Duration::from_secs(60))
    } else {
        (200, Duration::from_secs(60))
    }
}

pub async fn rate_limit_middleware(
    State(limiter): State<SharedRateLimiter>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    let (limit, window) = get_rate_limit_config(path);

    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    let rate_key = format!("{}:{}", client_ip, path.replace("/", "_"));

    let allowed = limiter.check_rate_limit(&rate_key, limit).await;
    let remaining = limiter.get_remaining(&rate_key, limit).await;
    let reset_time = (SystemTime::now() + window)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    let mut response = if allowed {
        next.run(req).await
    } else {
        (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests").into_response()
    };

    let headers = response.headers_mut();
    headers.insert("X-RateLimit-Limit", HeaderValue::from(limit));
    headers.insert("X-RateLimit-Remaining", HeaderValue::from(remaining));
    headers.insert("X-RateLimit-Reset", HeaderValue::from(reset_time));

    response
}
