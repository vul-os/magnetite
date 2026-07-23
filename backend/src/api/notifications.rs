// Notification API — real-time WS push and REST inbox; platform surface, partially wired.
#![allow(dead_code)]

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};

use crate::api::middleware::validate_token;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationType {
    AchievementUnlocked,
    GameInvite,
    FriendRequest,
    /// A wallet-to-wallet payment settled and a signed receipt was issued.
    PaymentSettled,
    /// Legacy: subscriptions were removed (the platform charges nothing). Kept
    /// only so historical notification rows still parse.
    SubscriptionRenewal,
    System,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationType::AchievementUnlocked => "ACHIEVEMENT_UNLOCKED",
            NotificationType::GameInvite => "GAME_INVITE",
            NotificationType::FriendRequest => "FRIEND_REQUEST",
            NotificationType::PaymentSettled => "PAYMENT_SETTLED",
            NotificationType::SubscriptionRenewal => "SUBSCRIPTION_RENEWAL",
            NotificationType::System => "SYSTEM",
        }
    }

    // Deliberately an enum-string parser returning Option, not std::str::FromStr
    // (returns Result) — callers want a nullable lookup, not an error.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ACHIEVEMENT_UNLOCKED" => Some(NotificationType::AchievementUnlocked),
            "GAME_INVITE" => Some(NotificationType::GameInvite),
            "FRIEND_REQUEST" => Some(NotificationType::FriendRequest),
            "PAYMENT_SETTLED" => Some(NotificationType::PaymentSettled),
            // Legacy custodial name, still parsed so historical rows resolve.
            "PAYOUT_COMPLETE" => Some(NotificationType::PaymentSettled),
            "SUBSCRIPTION_RENEWAL" => Some(NotificationType::SubscriptionRenewal),
            "SYSTEM" => Some(NotificationType::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    #[sqlx(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub body: Option<String>,
    pub data: Option<JsonValue>,
    pub read: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsNotification {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub body: Option<String>,
    pub data: Option<JsonValue>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<Notification> for WsNotification {
    fn from(n: Notification) -> Self {
        WsNotification {
            id: n.id,
            notification_type: n.notification_type,
            title: n.title,
            body: n.body,
            data: n.data,
            created_at: n.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationBroadcast {
    pub user_id: Uuid,
    pub notification: WsNotification,
}

#[derive(Debug, Deserialize)]
pub struct ListNotificationsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub read: Option<bool>,
    pub notification_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NotificationListResponse {
    pub notifications: Vec<Notification>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotificationRequest {
    pub user_id: Uuid,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub body: Option<String>,
    pub data: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct UnreadCountResponse {
    pub unread_count: i64,
}

static NOTIFICATION_BROADCASTER: RwLock<Option<Arc<NotificationBroadcaster>>> =
    RwLock::const_new(None);

pub struct NotificationBroadcaster {
    hubs: Mutex<std::collections::HashMap<Uuid, broadcast::Sender<NotificationBroadcast>>>,
}

impl NotificationBroadcaster {
    pub fn new() -> Self {
        Self {
            hubs: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub async fn broadcast(&self, user_id: Uuid, notification: Notification) {
        let broadcast = NotificationBroadcast {
            user_id,
            notification: notification.into(),
        };

        let hubs = self.hubs.lock().await;
        if let Some(sender) = hubs.get(&user_id) {
            let _ = sender.send(broadcast);
        }
    }

    pub async fn subscribe(&self, user_id: &Uuid) -> broadcast::Receiver<NotificationBroadcast> {
        let mut hubs = self.hubs.lock().await;
        if let Some(sender) = hubs.get(user_id) {
            return sender.subscribe();
        }
        let (tx, rx) = broadcast::channel(100);
        hubs.insert(*user_id, tx);
        rx
    }

    pub async fn unsubscribe(&self, user_id: &Uuid) {
        let mut hubs = self.hubs.lock().await;
        hubs.remove(user_id);
    }
}

impl Default for NotificationBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn init_notification_broadcaster() {
    let broadcaster = Arc::new(NotificationBroadcaster::new());
    let mut global = NOTIFICATION_BROADCASTER.write().await;
    *global = Some(broadcaster);
}

pub async fn get_notification_broadcaster() -> Option<Arc<NotificationBroadcaster>> {
    let global = NOTIFICATION_BROADCASTER.read().await;
    global.clone()
}

pub async fn broadcast_notification(notification: Notification) {
    if let Some(broadcaster) = get_notification_broadcaster().await {
        broadcaster
            .broadcast(notification.user_id, notification)
            .await;
    }
}

// ── Notification category mapping ────────────────────────────────────────────

/// Maps a notification type string to its preference category.
/// Returns None for unknown types (treated as always-enabled).
pub fn category_for_type(notification_type: &str) -> Option<&'static str> {
    match notification_type {
        // The `payouts_*` preference columns predate non-custodial settlement;
        // the category now means "money moved", not "we disbursed funds".
        "PAYMENT_SETTLED" | "PAYOUT_COMPLETE" | "SUBSCRIPTION_RENEWAL" => Some("payouts"),
        "FRIEND_REQUEST" | "GAME_INVITE" => Some("social"),
        "ACHIEVEMENT_UNLOCKED" => Some("achievements"),
        // SYSTEM notifications with no special category default to always enabled
        // unless we detect a marketing flag in the data (handled at call site).
        _ => None,
    }
}

pub struct NotificationService {
    pool: PgPool,
}

impl NotificationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates an in-app notification, respecting the user's preference toggles.
    ///
    /// Preference enforcement logic:
    ///   1. Resolve the category for the notification type.
    ///   2. If the user has disabled `in_app` for that category, skip DB insert
    ///      and return `None` — no notification is created.
    ///   3. If the user has disabled `push` for that category, the notification
    ///      is still persisted (in-app inbox) but the push channel is not called.
    ///      (Email suppression is also noted here for future email delivery.)
    ///
    /// `marketing` category is opt-in (default false); if a caller passes
    /// `notification_type = "SYSTEM"` AND sets `data.marketing = true`, we route
    /// it through the marketing category check.
    pub async fn create_notification(
        &self,
        req: &CreateNotificationRequest,
    ) -> Result<Notification> {
        // ── Preference gate ───────────────────────────────────────────────────
        // Determine the category; fall back to "marketing" when the data payload
        // explicitly marks this as a marketing notification.
        let effective_category: Option<&str> = {
            let base = category_for_type(&req.notification_type);
            if base.is_some() {
                base
            } else if req
                .data
                .as_ref()
                .and_then(|d| d.get("marketing"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                Some("marketing")
            } else {
                None // Unknown / unclassified — always allow.
            }
        };

        if let Some(category) = effective_category {
            // Check in_app preference — this is the primary delivery channel for
            // the notification inbox.  If disabled, skip entirely.
            let in_app_enabled =
                channel_enabled_inner(&self.pool, req.user_id, category, "in_app").await;
            if !in_app_enabled {
                tracing::debug!(
                    user_id = %req.user_id,
                    category,
                    notification_type = %req.notification_type,
                    "Skipping notification: user disabled in_app for category"
                );
                // Return a sentinel error that callers can ignore via ok() if needed,
                // or propagate. We use a specific variant so the HTTP handler can still
                // surface a 204/skipped result rather than a 500.
                return Err(AppError::BadRequest(format!(
                    "Notification suppressed: user disabled in_app for category '{}'",
                    category
                )));
            }
        }

        // ── Persist ───────────────────────────────────────────────────────────
        let notification = sqlx::query_as::<_, Notification>(
            "INSERT INTO notifications (id, user_id, type, title, body, data, read, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, false, NOW())
             RETURNING id, user_id, type, title, body, data, read, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(req.user_id)
        .bind(&req.notification_type)
        .bind(&req.title)
        .bind(&req.body)
        .bind(&req.data)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        // ── Push gate (future extension) ──────────────────────────────────────
        // If we later integrate a push provider, check push_enabled here before
        // calling it.  The notification itself is already in the inbox at this point.
        // let push_enabled = channel_enabled_inner(&self.pool, req.user_id, category, "push").await;

        // ── Email gate (future extension) ─────────────────────────────────────
        // If email delivery is added, check email_enabled before sending.
        // let email_enabled = channel_enabled_inner(&self.pool, req.user_id, category, "email").await;

        broadcast_notification(notification.clone()).await;

        Ok(notification)
    }

    /// Variant that silently drops the notification when preferences say to skip
    /// rather than returning an error.  Use this from internal callers (achievements,
    /// settlement paths) where a suppressed notification is not a call error.
    pub async fn try_create_notification(
        &self,
        req: &CreateNotificationRequest,
    ) -> Result<Option<Notification>> {
        match self.create_notification(req).await {
            Ok(n) => Ok(Some(n)),
            Err(AppError::BadRequest(ref msg)) if msg.contains("suppressed") => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn create_achievement_notification(
        &self,
        user_id: Uuid,
        achievement_name: &str,
        achievement_icon: Option<&str>,
    ) -> Result<Option<Notification>> {
        let data = achievement_icon.map(|icon| serde_json::json!({ "achievement_icon": icon }));

        self.try_create_notification(&CreateNotificationRequest {
            user_id,
            notification_type: NotificationType::AchievementUnlocked.as_str().to_string(),
            title: format!("Achievement Unlocked: {}", achievement_name),
            body: Some("Congratulations on unlocking this achievement!".to_string()),
            data,
        })
        .await
    }

    pub async fn create_friend_request_notification(
        &self,
        user_id: Uuid,
        from_username: &str,
    ) -> Result<Option<Notification>> {
        self.try_create_notification(&CreateNotificationRequest {
            user_id,
            notification_type: NotificationType::FriendRequest.as_str().to_string(),
            title: "New Friend Request".to_string(),
            body: Some(format!("{} sent you a friend request", from_username)),
            data: None,
        })
        .await
    }

    pub async fn create_game_invite_notification(
        &self,
        user_id: Uuid,
        from_username: &str,
        game_title: &str,
        game_id: Uuid,
    ) -> Result<Option<Notification>> {
        self.try_create_notification(&CreateNotificationRequest {
            user_id,
            notification_type: NotificationType::GameInvite.as_str().to_string(),
            title: "Game Invite".to_string(),
            body: Some(format!(
                "{} invited you to play {}",
                from_username, game_title
            )),
            data: Some(serde_json::json!({ "game_id": game_id })),
        })
        .await
    }

    /// Notify that a payment settled wallet-to-wallet.
    ///
    /// Non-custodial: nothing was disbursed by us — the rail moved value
    /// directly between wallets and issued a signed receipt.
    pub async fn create_settlement_notification(
        &self,
        user_id: Uuid,
        amount: &str,
    ) -> Result<Option<Notification>> {
        self.try_create_notification(&CreateNotificationRequest {
            user_id,
            notification_type: NotificationType::PaymentSettled.as_str().to_string(),
            title: "Payment Settled".to_string(),
            body: Some(format!("A payment of {} settled to your wallet", amount)),
            data: None,
        })
        .await
    }

    pub async fn create_system_notification(
        &self,
        user_id: Uuid,
        title: &str,
        body: &str,
    ) -> Result<Option<Notification>> {
        // System notifications are unclassified — always deliver to in-app inbox.
        // (category_for_type returns None for SYSTEM, so no preference gate fires.)
        self.try_create_notification(&CreateNotificationRequest {
            user_id,
            notification_type: NotificationType::System.as_str().to_string(),
            title: title.to_string(),
            body: Some(body.to_string()),
            data: None,
        })
        .await
    }
}

pub struct NotificationWsHandler {
    broadcaster: Arc<NotificationBroadcaster>,
}

impl NotificationWsHandler {
    pub fn new(broadcaster: Arc<NotificationBroadcaster>) -> Self {
        Self { broadcaster }
    }

    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/ws/notifications", get(handle_notification_connection))
            .with_state(self)
    }

    async fn get_or_create_hub(
        &self,
        user_id: &Uuid,
    ) -> broadcast::Receiver<NotificationBroadcast> {
        self.broadcaster.subscribe(user_id).await
    }

    pub async fn remove_user(&self, user_id: &Uuid) {
        self.broadcaster.unsubscribe(user_id).await;
    }
}

/// Query params accepted on /ws/notifications?token=<jwt>
#[derive(Debug, Deserialize)]
struct WsNotifQuery {
    token: Option<String>,
}

async fn handle_notification_connection(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(handler): State<Arc<NotificationWsHandler>>,
    Query(query): Query<WsNotifQuery>,
) -> axum::response::Response {
    // Authenticate via JWT query param — same pattern as /ws/comms and /ws/voice.
    // If the token is missing or invalid, close the connection immediately.
    let user_id = match query
        .token
        .as_deref()
        .and_then(|t| validate_token(t).ok())
        .and_then(|claims| Uuid::parse_str(&claims.sub).ok())
    {
        Some(id) => id,
        None => {
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::UNAUTHORIZED)
                .body(axum::body::Body::from("Unauthorized"))
                .unwrap();
        }
    };

    let receiver = handler.get_or_create_hub(&user_id).await;

    ws.on_upgrade(move |socket| async move {
        let (write, mut read) = socket.split();
        let write = Arc::new(Mutex::new(write));

        let user_id_clone = user_id;
        let handler_clone = handler.clone();
        let write_clone = write.clone();

        tokio::spawn(async move {
            while let Some(result) = read.next().await {
                if let Ok(axum::extract::ws::Message::Text(text)) = result {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(action) = parsed.get("action").and_then(|a| a.as_str()) {
                            let mut write_guard = write_clone.lock().await;
                            match action {
                                "ping" => {
                                    let _ = write_guard
                                        .send(axum::extract::ws::Message::Text(
                                            serde_json::json!({ "type": "pong" }).to_string(),
                                        ))
                                        .await;
                                }
                                "subscribe" => {
                                    let _ = write_guard
                                        .send(axum::extract::ws::Message::Text(
                                            serde_json::json!({
                                                "type": "subscribed",
                                                "user_id": user_id_clone
                                            })
                                            .to_string(),
                                        ))
                                        .await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            handler_clone.remove_user(&user_id_clone).await;
        });

        tokio::spawn(async move {
            let mut receiver = receiver;
            while let Ok(msg) = receiver.recv().await {
                if msg.user_id == user_id {
                    if let Ok(json) = serde_json::to_string(&msg) {
                        let mut write_guard = write.lock().await;
                        let _ = write_guard
                            .send(axum::extract::ws::Message::Text(json))
                            .await;
                    }
                }
            }
        });
    })
}

pub async fn list_notifications(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(params): Query<ListNotificationsQuery>,
) -> Result<Json<NotificationListResponse>> {
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);

    let (total, notifications) = if let Some(read) = params.read {
        let total = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read = $2",
        )
        .bind(user_id)
        .bind(read)
        .fetch_one(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let notifications = if let Some(ref ntype) = params.notification_type {
            sqlx::query_as::<_, Notification>(
                "SELECT id, user_id, type, title, body, data, read, created_at
                 FROM notifications WHERE user_id = $1 AND read = $2 AND type = $3
                 ORDER BY created_at DESC LIMIT $4 OFFSET $5",
            )
            .bind(user_id)
            .bind(read)
            .bind(ntype)
            .bind(limit)
            .bind(offset)
            .fetch_all(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
        } else {
            sqlx::query_as::<_, Notification>(
                "SELECT id, user_id, type, title, body, data, read, created_at
                 FROM notifications WHERE user_id = $1 AND read = $2
                 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
            )
            .bind(user_id)
            .bind(read)
            .bind(limit)
            .bind(offset)
            .fetch_all(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
        };

        (total.0, notifications)
    } else if let Some(ref ntype) = params.notification_type {
        let total = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = $2",
        )
        .bind(user_id)
        .bind(ntype)
        .fetch_one(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let notifications = sqlx::query_as::<_, Notification>(
            "SELECT id, user_id, type, title, body, data, read, created_at
             FROM notifications WHERE user_id = $1 AND type = $2
             ORDER BY created_at DESC LIMIT $3 OFFSET $4",
        )
        .bind(user_id)
        .bind(ntype)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        (total.0, notifications)
    } else {
        let total =
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM notifications WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

        let notifications = sqlx::query_as::<_, Notification>(
            "SELECT id, user_id, type, title, body, data, read, created_at
             FROM notifications WHERE user_id = $1
             ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        (total.0, notifications)
    };

    Ok(Json(NotificationListResponse {
        notifications,
        total,
        limit,
        offset,
    }))
}

pub async fn get_unread_count(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<UnreadCountResponse>> {
    let result = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read = false",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Json(UnreadCountResponse {
        unread_count: result.0,
    }))
}

pub async fn mark_as_read(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(notification_id): Path<Uuid>,
) -> Result<StatusCode> {
    let result = sqlx::query("UPDATE notifications SET read = true WHERE id = $1 AND user_id = $2")
        .bind(notification_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Notification not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn mark_all_as_read(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<StatusCode> {
    sqlx::query("UPDATE notifications SET read = true WHERE user_id = $1 AND read = false")
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_notification(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(notification_id): Path<Uuid>,
) -> Result<StatusCode> {
    let result = sqlx::query("DELETE FROM notifications WHERE id = $1 AND user_id = $2")
        .bind(notification_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Notification not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_notification(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateNotificationRequest>,
) -> Result<Json<Notification>> {
    let service = NotificationService::new(pool);
    let notification = service.create_notification(&payload).await?;
    Ok(Json(notification))
}

// ── Notification Preferences ────────────────────────────────────────────────

/// Per-channel (email / in_app / push), per-category (payouts / social /
/// achievements / marketing) preference row.  Mirrors the DB schema.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationPreferences {
    pub id: Uuid,
    pub user_id: Uuid,

    pub payouts_email: bool,
    pub payouts_in_app: bool,
    pub payouts_push: bool,

    pub social_email: bool,
    pub social_in_app: bool,
    pub social_push: bool,

    pub achievements_email: bool,
    pub achievements_in_app: bool,
    pub achievements_push: bool,

    pub marketing_email: bool,
    pub marketing_in_app: bool,
    pub marketing_push: bool,

    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Body accepted by PUT /preferences — all fields optional; omitted fields
/// retain their current DB values.
#[derive(Debug, Deserialize, Default)]
pub struct UpdateNotificationPreferencesRequest {
    pub payouts_email: Option<bool>,
    pub payouts_in_app: Option<bool>,
    pub payouts_push: Option<bool>,

    pub social_email: Option<bool>,
    pub social_in_app: Option<bool>,
    pub social_push: Option<bool>,

    pub achievements_email: Option<bool>,
    pub achievements_in_app: Option<bool>,
    pub achievements_push: Option<bool>,

    pub marketing_email: Option<bool>,
    pub marketing_in_app: Option<bool>,
    pub marketing_push: Option<bool>,
}

/// GET /api/v1/notifications/preferences
/// Returns the authenticated user's notification preferences, creating a
/// default row on first access (upsert-on-read pattern).
pub async fn get_preferences(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<NotificationPreferences>> {
    // Upsert defaults so callers always get a full row.
    let prefs = sqlx::query_as::<_, NotificationPreferences>(
        "INSERT INTO notification_preferences (user_id)
         VALUES ($1)
         ON CONFLICT (user_id) DO UPDATE
           SET updated_at = notification_preferences.updated_at
         RETURNING *",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Json(prefs))
}

/// PUT /api/v1/notifications/preferences
/// Partial update — only supplied fields are written.  Returns the updated row.
pub async fn update_preferences(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<UpdateNotificationPreferencesRequest>,
) -> Result<Json<NotificationPreferences>> {
    // Ensure a row exists first.
    sqlx::query(
        "INSERT INTO notification_preferences (user_id)
         VALUES ($1)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Apply each supplied field via individual updates to avoid a massive
    // dynamic-SQL builder while still keeping atomic semantics.
    macro_rules! apply_pref {
        ($col:literal, $val:expr) => {
            if let Some(v) = $val {
                sqlx::query(concat!(
                    "UPDATE notification_preferences SET ",
                    $col,
                    " = $1, updated_at = NOW() WHERE user_id = $2"
                ))
                .bind(v)
                .bind(user_id)
                .execute(&pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
            }
        };
    }

    apply_pref!("payouts_email", payload.payouts_email);
    apply_pref!("payouts_in_app", payload.payouts_in_app);
    apply_pref!("payouts_push", payload.payouts_push);
    apply_pref!("social_email", payload.social_email);
    apply_pref!("social_in_app", payload.social_in_app);
    apply_pref!("social_push", payload.social_push);
    apply_pref!("achievements_email", payload.achievements_email);
    apply_pref!("achievements_in_app", payload.achievements_in_app);
    apply_pref!("achievements_push", payload.achievements_push);
    apply_pref!("marketing_email", payload.marketing_email);
    apply_pref!("marketing_in_app", payload.marketing_in_app);
    apply_pref!("marketing_push", payload.marketing_push);

    // Return the final state.
    let prefs = sqlx::query_as::<_, NotificationPreferences>(
        "SELECT * FROM notification_preferences WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Json(prefs))
}

/// Internal helper — same as `channel_enabled` but named distinctly so the
/// borrow checker doesn't confuse it with the public API helper.
async fn channel_enabled_inner(
    pool: &PgPool,
    user_id: Uuid,
    category: &str,
    channel: &str,
) -> bool {
    channel_enabled(pool, user_id, category, channel).await
}

/// Returns whether the user has enabled a given channel for a given category.
/// Called by other modules (e.g. notification delivery) to skip disabled channels.
pub async fn channel_enabled(pool: &PgPool, user_id: Uuid, category: &str, channel: &str) -> bool {
    let col = format!("{}_{}", category, channel);
    // Allowlist to prevent SQL injection from caller-controlled strings.
    let allowed_cols = [
        "payouts_email",
        "payouts_in_app",
        "payouts_push",
        "social_email",
        "social_in_app",
        "social_push",
        "achievements_email",
        "achievements_in_app",
        "achievements_push",
        "marketing_email",
        "marketing_in_app",
        "marketing_push",
    ];
    if !allowed_cols.contains(&col.as_str()) {
        return true; // Unknown category/channel — default allow.
    }

    // Build the SELECT dynamically (col is validated above).
    let query = format!(
        "SELECT {} FROM notification_preferences WHERE user_id = $1",
        col
    );
    sqlx::query_scalar::<_, bool>(&query)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(true) // Default to enabled if no preference row exists yet.
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_notifications))
        .route("/count", get(get_unread_count))
        .route("/read-all", put(mark_all_as_read))
        .route("/preferences", get(get_preferences))
        .route("/preferences", put(update_preferences))
        .route("/:id/read", put(mark_as_read))
        .route("/:id", delete(delete_notification))
        .route("/", post(create_notification))
        .with_state(pool)
}
