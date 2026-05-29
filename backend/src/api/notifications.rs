// Notification API — real-time WS push and REST inbox; platform surface, partially wired.
#![allow(dead_code)]

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
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
    PayoutComplete,
    SubscriptionRenewal,
    System,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationType::AchievementUnlocked => "ACHIEVEMENT_UNLOCKED",
            NotificationType::GameInvite => "GAME_INVITE",
            NotificationType::FriendRequest => "FRIEND_REQUEST",
            NotificationType::PayoutComplete => "PAYOUT_COMPLETE",
            NotificationType::SubscriptionRenewal => "SUBSCRIPTION_RENEWAL",
            NotificationType::System => "SYSTEM",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ACHIEVEMENT_UNLOCKED" => Some(NotificationType::AchievementUnlocked),
            "GAME_INVITE" => Some(NotificationType::GameInvite),
            "FRIEND_REQUEST" => Some(NotificationType::FriendRequest),
            "PAYOUT_COMPLETE" => Some(NotificationType::PayoutComplete),
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

pub struct NotificationService {
    pool: PgPool,
}

impl NotificationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_notification(
        &self,
        req: &CreateNotificationRequest,
    ) -> Result<Notification> {
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

        broadcast_notification(notification.clone()).await;

        Ok(notification)
    }

    pub async fn create_achievement_notification(
        &self,
        user_id: Uuid,
        achievement_name: &str,
        achievement_icon: Option<&str>,
    ) -> Result<Notification> {
        let data = achievement_icon.map(|icon| serde_json::json!({ "achievement_icon": icon }));

        self.create_notification(&CreateNotificationRequest {
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
    ) -> Result<Notification> {
        self.create_notification(&CreateNotificationRequest {
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
    ) -> Result<Notification> {
        self.create_notification(&CreateNotificationRequest {
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

    pub async fn create_payout_notification(
        &self,
        user_id: Uuid,
        amount: &str,
    ) -> Result<Notification> {
        self.create_notification(&CreateNotificationRequest {
            user_id,
            notification_type: NotificationType::PayoutComplete.as_str().to_string(),
            title: "Payout Complete".to_string(),
            body: Some(format!("Your payout of {} USDC has been processed", amount)),
            data: None,
        })
        .await
    }

    pub async fn create_subscription_renewal_notification(
        &self,
        user_id: Uuid,
        tier_name: &str,
    ) -> Result<Notification> {
        self.create_notification(&CreateNotificationRequest {
            user_id,
            notification_type: NotificationType::SubscriptionRenewal.as_str().to_string(),
            title: "Subscription Renewed".to_string(),
            body: Some(format!("Your {} subscription has been renewed", tier_name)),
            data: None,
        })
        .await
    }

    pub async fn create_system_notification(
        &self,
        user_id: Uuid,
        title: &str,
        body: &str,
    ) -> Result<Notification> {
        self.create_notification(&CreateNotificationRequest {
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

async fn handle_notification_connection(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(handler): State<Arc<NotificationWsHandler>>,
    Extension(user_id): Extension<Uuid>,
) -> axum::response::Response {
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

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_notifications))
        .route("/count", get(get_unread_count))
        .route("/read-all", put(mark_all_as_read))
        .route("/:id/read", put(mark_as_read))
        .route("/:id", delete(delete_notification))
        .route("/", post(create_notification))
        .with_state(pool)
}
