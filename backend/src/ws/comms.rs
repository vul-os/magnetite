// Comms WebSocket handler — real-time chat, typing indicators, and presence.
//
// Clients connect to /ws/comms?token=<jwt> and then send JSON frames.
// Each frame has a "type" tag:
//
//   JoinChannel    { channel_id }
//   LeaveChannel   { channel_id }
//   SendMessage    { channel_id, content, reply_to_id? }
//   TypingStart    { channel_id }
//   TypingStop     { channel_id }
//   SetPresence    { status, activity? }
//   Ping
//
// Server pushes back frames with the same type namespace plus:
//   MessageCreated { channel_id, message }
//   TypingNotify   { channel_id, user_id }
//   PresenceUpdate { user_id, status, activity? }
//   Error          { code, message }
//   Pong
//
// Architecture note:
//   A `tokio::sync::broadcast` channel per text-channel carries outbound frames
//   to all subscribers.  A global `ChannelRegistry` (Arc<Mutex<HashMap>>) owns
//   the senders.  This is the correct approach for <~1 000 concurrent users per
//   channel; at larger scale, replace the in-process registry with Redis Pub/Sub
//   or a message bus (e.g. NATS) so multiple backend replicas can share state.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::api::middleware::validate_token;
use crate::api::reviews::content_flag_reasons;
use crate::services::communities as svc;
use crate::services::presence as presence_svc;

// ---------------------------------------------------------------------------
// Message protocol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientFrame {
    JoinChannel {
        channel_id: Uuid,
    },
    LeaveChannel {
        channel_id: Uuid,
    },
    SendMessage {
        channel_id: Uuid,
        content: String,
        reply_to_id: Option<Uuid>,
    },
    TypingStart {
        channel_id: Uuid,
    },
    TypingStop {
        channel_id: Uuid,
    },
    SetPresence {
        status: String,
        activity: Option<String>,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFrame {
    MessageCreated {
        channel_id: Uuid,
        message: MessagePayload,
    },
    TypingNotify {
        channel_id: Uuid,
        user_id: Uuid,
    },
    PresenceUpdate {
        user_id: Uuid,
        status: String,
        activity: Option<String>,
    },
    Error {
        code: String,
        message: String,
    },
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePayload {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub reply_to_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<svc::Message> for MessagePayload {
    fn from(m: svc::Message) -> Self {
        MessagePayload {
            id: m.id,
            channel_id: m.channel_id,
            author_id: m.author_id,
            content: m.content,
            reply_to_id: m.reply_to_id,
            created_at: m.created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Channel broadcast registry
// ---------------------------------------------------------------------------

/// Per-channel broadcast sender.  Each subscriber gets its own `Receiver`.
#[derive(Clone)]
pub struct ChannelRegistry {
    channels: Arc<Mutex<HashMap<Uuid, broadcast::Sender<ServerFrame>>>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn subscribe(&self, channel_id: Uuid) -> broadcast::Receiver<ServerFrame> {
        let mut map = self.channels.lock().await;
        if let Some(tx) = map.get(&channel_id) {
            return tx.subscribe();
        }
        let (tx, rx) = broadcast::channel(512);
        map.insert(channel_id, tx);
        rx
    }

    pub async fn broadcast(&self, channel_id: Uuid, frame: ServerFrame) {
        let map = self.channels.lock().await;
        if let Some(tx) = map.get(&channel_id) {
            let _ = tx.send(frame);
        }
    }

    /// Purge channels that have no active subscribers.
    #[allow(dead_code)]
    pub async fn gc(&self) {
        let mut map = self.channels.lock().await;
        map.retain(|_, tx| tx.receiver_count() > 0);
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AppState for the comms router
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CommsState {
    pub pool: PgPool,
    pub registry: ChannelRegistry,
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<CommsState>,
    Query(query): Query<WsQuery>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_comms_socket(socket, state, query.token))
}

async fn handle_comms_socket(
    socket: axum::extract::ws::WebSocket,
    state: CommsState,
    token: Option<String>,
) {
    // Authenticate via JWT in query string.
    let user_id = match token.as_deref().and_then(|t| validate_token(t).ok()) {
        Some(claims) => match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => return,
        },
        None => return,
    };

    // Mark user online.
    let _ = presence_svc::set_presence(&state.pool, user_id, "online", None, None).await;

    let (mut write, mut read) = socket.split();

    // Per-connection outbound channel.  Frames from any subscribed broadcast
    // channel are forwarded to this sender and drained by the write task.
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<ServerFrame>(256);

    // Write task — serialize and forward frames to the WebSocket.
    let write_task = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&frame) {
                if write
                    .send(axum::extract::ws::Message::Text(json))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });

    // Track which channels this connection has joined so we can unsubscribe on
    // disconnect.  Each entry holds an abort handle for the forward task.
    let mut joined: HashMap<Uuid, tokio::task::AbortHandle> = HashMap::new();

    // Read loop.
    while let Some(Ok(msg)) = read.next().await {
        let text = match msg {
            axum::extract::ws::Message::Text(t) => t,
            axum::extract::ws::Message::Close(_) => break,
            _ => continue,
        };

        let frame: ClientFrame = match serde_json::from_str(&text) {
            Ok(f) => f,
            Err(_) => {
                let _ = out_tx
                    .send(ServerFrame::Error {
                        code: "BAD_FRAME".to_string(),
                        message: "Invalid JSON frame".to_string(),
                    })
                    .await;
                continue;
            }
        };

        match frame {
            ClientFrame::JoinChannel { channel_id } => {
                if joined.contains_key(&channel_id) {
                    continue;
                }
                // Subscribe to the broadcast channel and forward to per-connection out_tx.
                let mut rx = state.registry.subscribe(channel_id).await;
                let fwd_tx = out_tx.clone();
                let handle = tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(frame) => {
                                if fwd_tx.send(frame).await.is_err() {
                                    break;
                                }
                            }
                            // Lagged: skip missed frames and continue.
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                })
                .abort_handle();
                joined.insert(channel_id, handle);
            }

            ClientFrame::LeaveChannel { channel_id } => {
                if let Some(handle) = joined.remove(&channel_id) {
                    handle.abort();
                }
            }

            ClientFrame::SendMessage {
                channel_id,
                content,
                reply_to_id,
            } => {
                // ── Auto-flag heuristic ───────────────────────────────────────
                // Run before persisting so we can attach flag metadata alongside
                // the message.  Failure to insert the flag is never fatal.
                let flag_reasons = content_flag_reasons(&content);
                if !flag_reasons.is_empty() {
                    let reason_str = flag_reasons.join(", ");
                    tracing::info!(
                        channel_id = %channel_id,
                        author_id  = %user_id,
                        reasons    = %reason_str,
                        "Auto-flagging chat message"
                    );
                    let pool_ref = state.pool.clone();
                    let content_snapshot = content.clone();
                    tokio::spawn(async move {
                        if let Err(e) = sqlx::query(
                            "INSERT INTO chat_flags
                                 (channel_id, author_id, content, flag_reasons, status)
                             VALUES ($1, $2, $3, $4, 'pending')",
                        )
                        .bind(channel_id)
                        .bind(user_id)
                        .bind(&content_snapshot)
                        .bind(&reason_str)
                        .execute(&pool_ref)
                        .await
                        {
                            tracing::warn!(
                                channel_id = %channel_id,
                                error = %e,
                                "Failed to insert chat_flag (non-fatal)"
                            );
                        }
                    });
                }

                match svc::post_message(&state.pool, channel_id, user_id, &content, reply_to_id)
                    .await
                {
                    Ok(msg) => {
                        let payload = MessagePayload::from(msg);
                        let broadcast_frame = ServerFrame::MessageCreated {
                            channel_id,
                            message: payload,
                        };
                        state.registry.broadcast(channel_id, broadcast_frame).await;
                    }
                    Err(e) => {
                        let _ = out_tx
                            .send(ServerFrame::Error {
                                code: "SEND_FAILED".to_string(),
                                message: e.to_string(),
                            })
                            .await;
                    }
                }
            }

            ClientFrame::TypingStart { channel_id } => {
                if joined.contains_key(&channel_id) {
                    state
                        .registry
                        .broadcast(
                            channel_id,
                            ServerFrame::TypingNotify {
                                channel_id,
                                user_id,
                            },
                        )
                        .await;
                }
            }

            ClientFrame::TypingStop { .. } => {
                // Typing-stop is intentionally not broadcast to reduce noise;
                // clients use a client-side timeout (e.g. 3 s after last TypingStart).
            }

            ClientFrame::SetPresence { status, activity } => {
                let _ = presence_svc::set_presence(
                    &state.pool,
                    user_id,
                    &status,
                    activity.as_deref(),
                    None,
                )
                .await;
                // Broadcast presence to all joined channels so member lists update.
                for &channel_id in joined.keys() {
                    state
                        .registry
                        .broadcast(
                            channel_id,
                            ServerFrame::PresenceUpdate {
                                user_id,
                                status: status.clone(),
                                activity: activity.clone(),
                            },
                        )
                        .await;
                }
            }

            ClientFrame::Ping => {
                let _ = out_tx.send(ServerFrame::Pong).await;
            }
        }
    }

    // Cleanup on disconnect.
    for (_, handle) in joined {
        handle.abort();
    }
    write_task.abort();
    let _ = presence_svc::set_offline(&state.pool, user_id).await;
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool) -> Router {
    let registry = ChannelRegistry::new();
    let state = CommsState { pool, registry };
    Router::new()
        .route("/ws/comms", get(ws_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_frame_deserialize_ping() {
        let json = r#"{"type":"ping"}"#;
        let frame: ClientFrame = serde_json::from_str(json).unwrap();
        matches!(frame, ClientFrame::Ping);
    }

    #[test]
    fn test_client_frame_deserialize_send_message() {
        let json = r#"{"type":"send_message","channel_id":"00000000-0000-0000-0000-000000000001","content":"hello","reply_to_id":null}"#;
        let frame: ClientFrame = serde_json::from_str(json).unwrap();
        if let ClientFrame::SendMessage { content, .. } = frame {
            assert_eq!(content, "hello");
        } else {
            panic!("Expected SendMessage");
        }
    }

    #[test]
    fn test_server_frame_serialize_pong() {
        let frame = ServerFrame::Pong;
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("pong"));
    }

    #[test]
    fn test_message_payload_from_message() {
        let now = chrono::Utc::now();
        let msg = svc::Message {
            id: Uuid::new_v4(),
            channel_id: Uuid::new_v4(),
            author_id: Uuid::new_v4(),
            content: "test".to_string(),
            edited_at: None,
            deleted: false,
            reply_to_id: None,
            attachments: None,
            created_at: now,
        };
        let payload = MessagePayload::from(msg.clone());
        assert_eq!(payload.content, "test");
        assert_eq!(payload.id, msg.id);
    }

    #[tokio::test]
    async fn test_channel_registry_subscribe_and_broadcast() {
        let registry = ChannelRegistry::new();
        let channel_id = Uuid::new_v4();
        let mut rx = registry.subscribe(channel_id).await;

        registry.broadcast(channel_id, ServerFrame::Pong).await;

        let received = rx.try_recv().unwrap();
        matches!(received, ServerFrame::Pong);
    }
}
