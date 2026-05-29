use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

use crate::error::Result;
use crate::db::DbPool;
use crate::services::session::Session;

#[derive(Clone)]
pub struct Cache {
    store: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

pub struct RedisCache {
    client: redis::Client,
    connection: Arc<RwLock<Option<redis::aio::Connection>>>,
}

impl RedisCache {
    pub fn new(redis_url: &str) -> Self {
        Self {
            client: redis::Client::open(redis_url.to_string()).expect("Failed to create Redis client"),
            connection: Arc::new(RwLock::new(None)),
        }
    }

    async fn get_connection(&self) -> Option<redis::aio::Connection> {
        let mut guard = self.connection.write().await;
        if guard.is_none() {
            match self.client.get_tokio_connection().await {
                Ok(conn) => {
                    *guard = Some(conn);
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to Redis: {}", e);
                    return None;
                }
            }
        }
        guard.clone()
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let conn = self.get_connection().await?;
        let mut conn = conn;
        match redis::cmd("GET").arg(key).query_async::<String>(&mut conn).await {
            Ok(value) => Some(value),
            Err(e) => {
                tracing::warn!("Redis GET error for key {}: {}", key, e);
                None
            }
        }
    }

    pub async fn set(&self, key: &str, value: String, ttl: Option<Duration>) {
        let conn = match self.get_connection().await {
            Some(c) => c,
            None => return,
        };
        let mut conn = conn;
        let result = if let Some(duration) = ttl {
            redis::cmd("SETEX")
                .arg(key)
                .arg(duration.as_secs())
                .arg(&value)
                .query_async::<()>(&mut conn)
                .await
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(&value)
                .query_async::<()>(&mut conn)
                .await
        };
        if let Err(e) = result {
            tracing::warn!("Redis SET error for key {}: {}", key, e);
        }
    }

    pub async fn delete(&self, key: &str) {
        let conn = match self.get_connection().await {
            Some(c) => c,
            None => return,
        };
        let mut conn = conn;
        if let Err(e) = redis::cmd("DEL").arg(key).query_async::<()>(&mut conn).await {
            tracing::warn!("Redis DEL error for key {}: {}", key, e);
        }
    }

    pub async fn clear(&self) {
        let conn = match self.get_connection().await {
            Some(c) => c,
            None => return,
        };
        let mut conn = conn;
        if let Err(e) = redis::cmd("FLUSHDB").query_async::<()>(&mut conn).await {
            tracing::warn!("Redis FLUSHDB error: {}", e);
        }
    }

    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let value = self.get(key).await?;
        serde_json::from_str(&value).ok()
    }

    pub async fn set_json<T: serde::Serialize>(&self, key: &str, value: &T, ttl: Option<Duration>) {
        if let Ok(json) = serde_json::to_string(value) {
            self.set(key, json, ttl).await;
        }
    }
}

pub async fn sliding_window_rate_limit(
    redis_cache: &RedisCache,
    key: &str,
    limit: u64,
    window_secs: u64,
) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let window_start = now.saturating_sub(window_secs * 1000);
    let redis_key = format!("ratelimit:{}", key);

    let conn = match redis_cache.get_connection().await {
        Some(c) => c,
        None => return true,
    };
    let mut conn = conn;

    let script = r#"
        local key = KEYS[1]
        local now = tonumber(ARGV[1])
        local window_start = tonumber(ARGV[2])
        local limit = tonumber(ARGV[3])
        local window_ms = tonumber(ARGV[4])

        redis.call('ZREMRANGEBYSCORE', key, 0, window_start)
        local count = redis.call('ZCARD', key)

        if count < limit then
            redis.call('ZADD', key, now, now .. ':' .. math.random())
            redis.call('PEXPIRE', key, window_ms)
            return 1
        end
        return 0
    "#;

    match redis::cmd("EVAL")
        .arg(script)
        .arg(1)
        .arg(&redis_key)
        .arg(now)
        .arg(window_start)
        .arg(limit)
        .arg(window_secs * 1000)
        .query_async::<i32>(&mut conn)
        .await
    {
        Ok(1) => true,
        Ok(0) => false,
        Err(e) => {
            tracing::warn!("Redis rate limit error: {}", e);
            true
        }
    }
}

struct CacheEntry {
    value: String,
    expires_at: Option<Instant>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let store = self.store.read().await;
        let entry = store.get(key)?;
        
        if let Some(expires_at) = entry.expires_at {
            if Instant::now() > expires_at {
                drop(store);
                self.store.write().await.remove(key);
                return None;
            }
        }
        
        Some(entry.value.clone())
    }

    pub async fn set(&self, key: &str, value: String, ttl: Option<Duration>) {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let entry = CacheEntry { value, expires_at };
        self.store.write().await.insert(key.to_string(), entry);
    }

    pub async fn delete(&self, key: &str) {
        self.store.write().await.remove(key);
    }

    pub async fn clear(&self) {
        self.store.write().await.clear();
    }

    pub async fn cleanup(&self) {
        let now = Instant::now();
        let mut store = self.store.write().await;
        store.retain(|_, entry| {
            entry.expires_at.map(|e| e > now).unwrap_or(true)
        });
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CacheKeys;

impl CacheKeys {
    pub const GAME_LIST: &'static str = "games:list";
    pub const LEADERBOARD: &'static str = "leaderboard";
    pub const USER_PROFILE: &'static str = "user:profile";
    pub const PLATFORM_SETTINGS: &'static str = "platform:settings";
}

pub struct CacheTTL;

impl CacheTTL {
    pub const GAME_LIST: Duration = Duration::from_secs(300);
    pub const LEADERBOARD: Duration = Duration::from_secs(60);
    pub const USER_PROFILE: Duration = Duration::from_secs(300);
    pub const PLATFORM_SETTINGS: Duration = Duration::from_secs(600);
}

pub async fn get_game_list_cached(cache: &Cache, db: &DbPool) -> Result<Vec<crate::services::games::Game>> {
    if let Some(cached) = cache.get(CacheKeys::GAME_LIST).await {
        if let Ok(games) = serde_json::from_str::<Vec<crate::services::games::Game>>(&cached) {
            return Ok(games);
        }
    }

    let games = crate::services::games::get_all_games(db).await?;
    let json = serde_json::to_string(&games).map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    cache.set(CacheKeys::GAME_LIST, json, Some(CacheTTL::GAME_LIST)).await;
    Ok(games)
}

pub async fn get_leaderboard_cached(
    cache: &Cache,
    db: &DbPool,
    game_id: uuid::Uuid,
    limit: i32,
) -> Result<Vec<crate::services::games::LeaderboardEntry>> {
    let key = format!("{}:{}:{}", CacheKeys::LEADERBOARD, game_id, limit);
    
    if let Some(cached) = cache.get(&key).await {
        if let Ok(entries) = serde_json::from_str::<Vec<crate::services::games::LeaderboardEntry>>(&cached) {
            return Ok(entries);
        }
    }

    let entries = crate::services::games::get_leaderboard(db, game_id, limit).await?;
    let json = serde_json::to_string(&entries).map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    cache.set(&key, json, Some(CacheTTL::LEADERBOARD)).await;
    Ok(entries)
}

pub async fn get_user_profile_cached(
    cache: &Cache,
    db: &DbPool,
    user_id: uuid::Uuid,
) -> Result<Option<crate::services::auth::User>> {
    let key = format!("{}:{}", CacheKeys::USER_PROFILE, user_id);
    
    if let Some(cached) = cache.get(&key).await {
        if let Ok(user) = serde_json::from_str::<crate::services::auth::User>(&cached) {
            return Ok(Some(user));
        }
    }

    let user = crate::services::auth::get_user_by_id(db, user_id).await?;
    if let Some(ref u) = user {
        let json = serde_json::to_string(u).map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        cache.set(&key, json, Some(CacheTTL::USER_PROFILE)).await;
    }
    Ok(user)
}

pub async fn get_platform_settings_cached(
    cache: &Cache,
    db: &DbPool,
) -> Result<crate::services::games::PlatformSettings> {
    if let Some(cached) = cache.get(CacheKeys::PLATFORM_SETTINGS).await {
        if let Ok(settings) = serde_json::from_str::<crate::services::games::PlatformSettings>(&cached) {
            return Ok(settings);
        }
    }

    let settings = crate::services::games::get_platform_settings(db).await?;
    let json = serde_json::to_string(&settings).map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    cache.set(CacheKeys::PLATFORM_SETTINGS, json, Some(CacheTTL::PLATFORM_SETTINGS)).await;
    Ok(settings)
}

pub async fn invalidate_game_cache(cache: &Cache) {
    cache.delete(CacheKeys::GAME_LIST).await;
}

pub async fn invalidate_leaderboard_cache(cache: &Cache, game_id: uuid::Uuid) {
    for limit in [10, 50, 100] {
        let key = format!("{}:{}:{}", CacheKeys::LEADERBOARD, game_id, limit);
        cache.delete(&key).await;
    }
}

pub async fn invalidate_user_profile_cache(cache: &Cache, user_id: uuid::Uuid) {
    let key = format!("{}:{}", CacheKeys::USER_PROFILE, user_id);
    cache.delete(&key).await;
}

pub async fn invalidate_platform_settings_cache(cache: &Cache) {
    cache.delete(CacheKeys::PLATFORM_SETTINGS).await;
}

pub struct SessionCacheKeys;

impl SessionCacheKeys {
    pub fn user_session(user_id: uuid::Uuid) -> String {
        format!("session:user:{}", user_id)
    }
}

pub struct SessionCacheTTL;

impl SessionCacheTTL {
    pub const SESSION: Duration = Duration::from_secs(300);
}

pub async fn get_user_session_cached(
    redis_cache: &RedisCache,
    db: &DbPool,
    user_id: uuid::Uuid,
) -> Result<Option<Session>> {
    let key = SessionCacheKeys::user_session(user_id);

    if let Some(session) = redis_cache.get_json::<Session>(&key).await {
        return Ok(Some(session));
    }

    let sessions = crate::services::session::list_user_sessions(
        db,
        user_id,
        None,
    ).await?;

    if let Some(session_info) = sessions.first() {
        if let Some(session) = crate::services::session::get_session_by_id(db, session_info.id).await? {
            redis_cache.set_json(&key, &session, Some(SessionCacheTTL::SESSION)).await;
            return Ok(Some(session));
        }
    }

    Ok(None)
}

pub async fn set_user_session(
    redis_cache: &RedisCache,
    user_id: uuid::Uuid,
    session: &Session,
) {
    let key = SessionCacheKeys::user_session(user_id);
    redis_cache.set_json(&key, session, Some(SessionCacheTTL::SESSION)).await;
}

pub async fn invalidate_user_session(
    redis_cache: &RedisCache,
    user_id: uuid::Uuid,
) {
    let key = SessionCacheKeys::user_session(user_id);
    redis_cache.delete(&key).await;
}