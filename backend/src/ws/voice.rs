// Voice WebRTC Signaling — the backend acts as the SDP/ICE relay for peer-to-peer
// voice connections between participants in a voice room.
//
// ┌──────────────────────────────────────────────────────────────────────┐
// │  Architecture: MESH (small rooms) + SFU as documented scale path    │
// │                                                                      │
// │  Current implementation: each participant negotiates WebRTC directly │
// │  with every other participant (mesh).  The backend is a PURE         │
// │  SIGNALING SERVER — it never touches audio/video media.              │
// │                                                                      │
// │  Scale path: at ~8+ participants, switch to a Selective Forwarding  │
// │  Unit (SFU) such as LiveKit or mediasoup.  The signaling protocol   │
// │  here is SFU-compatible: replacing the backend relay with a         │
// │  `livekit-server` or `mediasoup` room just requires updating the    │
// │  Join handler to return an SFU token instead of broadcasting peer   │
// │  lists, and the client SDK to connect to the SFU transport.         │
// └──────────────────────────────────────────────────────────────────────┘
//
// Protocol (JSON frames over WebSocket /ws/voice?token=<jwt>&room=<room_token>):
//
// Client → Server:
//   JoinRoom   { room_token }             — client signals intent to join
//   Offer      { to_user_id, sdp }        — WebRTC offer SDP
//   Answer     { to_user_id, sdp }        — WebRTC answer SDP
//   IceCandidate { to_user_id, candidate, sdp_mid, sdp_mline_index }
//   Mute       { muted }                  — toggle mute; updates DB + notifies peers
//   LeaveRoom                             — explicit leave
//   Ping
//
// Server → Client:
//   ParticipantJoined  { user_id, participants: [user_id] }
//   ParticipantLeft    { user_id }
//   Offer              { from_user_id, sdp }
//   Answer             { from_user_id, sdp }
//   IceCandidate       { from_user_id, candidate, sdp_mid, sdp_mline_index }
//   MuteChanged        { user_id, muted }
//   RoomState          { participants: [{ user_id, is_muted }] }
//   Error              { code, message }
//   Pong

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

// ---------------------------------------------------------------------------
// Protocol frames
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientVoiceFrame {
    JoinRoom {
        room_token: String,
    },
    Offer {
        to_user_id: Uuid,
        sdp: String,
    },
    Answer {
        to_user_id: Uuid,
        sdp: String,
    },
    IceCandidate {
        to_user_id: Uuid,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    Mute {
        muted: bool,
    },
    LeaveRoom,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerVoiceFrame {
    ParticipantJoined {
        user_id: Uuid,
        participants: Vec<Uuid>,
    },
    ParticipantLeft {
        user_id: Uuid,
    },
    Offer {
        from_user_id: Uuid,
        sdp: String,
    },
    Answer {
        from_user_id: Uuid,
        sdp: String,
    },
    IceCandidate {
        from_user_id: Uuid,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    MuteChanged {
        user_id: Uuid,
        muted: bool,
    },
    RoomState {
        participants: Vec<ParticipantInfo>,
    },
    Error {
        code: String,
        message: String,
    },
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub user_id: Uuid,
    pub is_muted: bool,
}

// ---------------------------------------------------------------------------
// In-memory room registry
//
// `VoiceRoom` lives entirely in memory; the database rows in `voice_participants`
// are used for persistence / analytics but the signaling relay uses only the
// in-memory senders to avoid DB round-trips on every ICE candidate.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct RoomParticipant {
    user_id: Uuid,
    is_muted: bool,
    /// Each participant has a personal broadcast sender so we can address them
    /// individually (for 1-1 SDP relay without fan-out).
    tx: broadcast::Sender<ServerVoiceFrame>,
}

struct VoiceRoomState {
    #[allow(dead_code)]
    room_id: Uuid,
    participants: HashMap<Uuid, RoomParticipant>,
}

impl VoiceRoomState {
    fn new(room_id: Uuid) -> Self {
        Self {
            room_id,
            participants: HashMap::new(),
        }
    }

    fn add(&mut self, user_id: Uuid) -> broadcast::Receiver<ServerVoiceFrame> {
        if let Some(p) = self.participants.get(&user_id) {
            return p.tx.subscribe();
        }
        let (tx, rx) = broadcast::channel(64);
        self.participants.insert(
            user_id,
            RoomParticipant {
                user_id,
                is_muted: false,
                tx,
            },
        );
        rx
    }

    fn remove(&mut self, user_id: Uuid) {
        self.participants.remove(&user_id);
    }

    fn participant_ids(&self) -> Vec<Uuid> {
        self.participants.keys().cloned().collect()
    }

    fn state_snapshot(&self) -> Vec<ParticipantInfo> {
        self.participants
            .values()
            .map(|p| ParticipantInfo {
                user_id: p.user_id,
                is_muted: p.is_muted,
            })
            .collect()
    }

    /// Send a frame to a specific participant.
    fn send_to(&self, user_id: Uuid, frame: ServerVoiceFrame) {
        if let Some(p) = self.participants.get(&user_id) {
            let _ = p.tx.send(frame);
        }
    }

    /// Broadcast to all participants except the sender.
    fn broadcast_except(&self, except: Uuid, frame: ServerVoiceFrame) {
        for p in self.participants.values() {
            if p.user_id != except {
                let _ = p.tx.send(frame.clone());
            }
        }
    }

    fn set_muted(&mut self, user_id: Uuid, muted: bool) {
        if let Some(p) = self.participants.get_mut(&user_id) {
            p.is_muted = muted;
        }
    }
}

// ---------------------------------------------------------------------------
// Global voice room registry  (room_token → VoiceRoomState)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct VoiceRegistry {
    rooms: Arc<Mutex<HashMap<String, Arc<Mutex<VoiceRoomState>>>>>,
}

impl VoiceRegistry {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn get_or_create(&self, room_id: Uuid, room_token: &str) -> Arc<Mutex<VoiceRoomState>> {
        let mut rooms = self.rooms.lock().await;
        rooms
            .entry(room_token.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(VoiceRoomState::new(room_id))))
            .clone()
    }

    /// Remove rooms with no active participants.
    pub async fn gc(&self) {
        let mut rooms = self.rooms.lock().await;
        let mut to_remove = vec![];
        for (token, state) in rooms.iter() {
            if let Ok(s) = state.try_lock() {
                if s.participants.is_empty() {
                    to_remove.push(token.clone());
                }
            }
        }
        for token in to_remove {
            rooms.remove(&token);
        }
    }
}

impl Default for VoiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct VoiceState {
    pub pool: PgPool,
    pub registry: VoiceRegistry,
    pub gauges: Arc<crate::ws::gauges::WsGauges>,
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct VoiceWsQuery {
    pub token: Option<String>,
    pub room: Option<String>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn voice_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<VoiceState>,
    Query(query): Query<VoiceWsQuery>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_voice_socket(socket, state, query.token, query.room))
}

async fn handle_voice_socket(
    socket: axum::extract::ws::WebSocket,
    state: VoiceState,
    token: Option<String>,
    room_token_param: Option<String>,
) {
    let user_id = match token.as_deref().and_then(|t| validate_token(t).ok()) {
        Some(claims) => match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(_) => return,
        },
        None => return,
    };

    let room_token = match room_token_param {
        Some(r) => r,
        None => return,
    };

    // Look up voice_room row in DB to get the room UUID.
    let room_row = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM voice_rooms WHERE room_token = $1 AND is_active = true",
    )
    .bind(&room_token)
    .fetch_optional(&state.pool)
    .await;

    let room_id = match room_row {
        Ok(Some((id,))) => id,
        _ => return, // room not found or DB error — reject silently
    };

    // Count this accepted voice socket in the live WS gauge for its lifetime.
    let _conn = crate::ws::gauges::ConnGuard::new(Arc::clone(&state.gauges));

    let room_arc = state.registry.get_or_create(room_id, &room_token).await;

    // Persist participant in DB.
    let _ = sqlx::query(
        r#"
        INSERT INTO voice_participants (id, room_id, user_id, is_muted, joined_at)
        VALUES ($1, $2, $3, false, NOW())
        ON CONFLICT (room_id, user_id) DO UPDATE SET left_at = NULL, is_muted = false
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(room_id)
    .bind(user_id)
    .execute(&state.pool)
    .await;

    // Subscribe to per-participant outbound channel.
    let mut personal_rx = {
        let mut room = room_arc.lock().await;
        let rx = room.add(user_id);

        // Notify existing participants that a new peer has joined.
        let peers = room.participant_ids();
        room.broadcast_except(
            user_id,
            ServerVoiceFrame::ParticipantJoined {
                user_id,
                participants: peers.clone(),
            },
        );

        // Send the new participant the current room state.
        let snapshot = room.state_snapshot();
        if let Some(p) = room.participants.get(&user_id) {
            let _ = p.tx.send(ServerVoiceFrame::RoomState {
                participants: snapshot,
            });
        }

        rx
    };

    let (mut write, mut read) = socket.split();

    // Per-connection outbound mpsc (personal_rx feeds into this).
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<ServerVoiceFrame>(128);

    // Forward personal_rx → out_tx.
    let out_tx_fwd = out_tx.clone();
    let fwd_task = tokio::spawn(async move {
        loop {
            match personal_rx.recv().await {
                Ok(frame) => {
                    if out_tx_fwd.send(frame).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Write task.
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

    // Read loop.
    while let Some(Ok(msg)) = read.next().await {
        let text = match msg {
            axum::extract::ws::Message::Text(t) => t,
            axum::extract::ws::Message::Close(_) => break,
            _ => continue,
        };

        let frame: ClientVoiceFrame = match serde_json::from_str(&text) {
            Ok(f) => f,
            Err(_) => {
                let _ = out_tx
                    .send(ServerVoiceFrame::Error {
                        code: "BAD_FRAME".to_string(),
                        message: "Invalid JSON frame".to_string(),
                    })
                    .await;
                continue;
            }
        };

        match frame {
            ClientVoiceFrame::JoinRoom { .. } => {
                // Already joined during connection setup; re-send room state.
                let room = room_arc.lock().await;
                let snapshot = room.state_snapshot();
                let _ = out_tx
                    .send(ServerVoiceFrame::RoomState {
                        participants: snapshot,
                    })
                    .await;
            }

            ClientVoiceFrame::Offer { to_user_id, sdp } => {
                let room = room_arc.lock().await;
                room.send_to(
                    to_user_id,
                    ServerVoiceFrame::Offer {
                        from_user_id: user_id,
                        sdp,
                    },
                );
            }

            ClientVoiceFrame::Answer { to_user_id, sdp } => {
                let room = room_arc.lock().await;
                room.send_to(
                    to_user_id,
                    ServerVoiceFrame::Answer {
                        from_user_id: user_id,
                        sdp,
                    },
                );
            }

            ClientVoiceFrame::IceCandidate {
                to_user_id,
                candidate,
                sdp_mid,
                sdp_mline_index,
            } => {
                let room = room_arc.lock().await;
                room.send_to(
                    to_user_id,
                    ServerVoiceFrame::IceCandidate {
                        from_user_id: user_id,
                        candidate,
                        sdp_mid,
                        sdp_mline_index,
                    },
                );
            }

            ClientVoiceFrame::Mute { muted } => {
                {
                    let mut room = room_arc.lock().await;
                    room.set_muted(user_id, muted);
                    room.broadcast_except(
                        user_id,
                        ServerVoiceFrame::MuteChanged { user_id, muted },
                    );
                }
                let _ = sqlx::query(
                    "UPDATE voice_participants SET is_muted = $1 WHERE room_id = $2 AND user_id = $3 AND left_at IS NULL",
                )
                .bind(muted)
                .bind(room_id)
                .bind(user_id)
                .execute(&state.pool)
                .await;
            }

            ClientVoiceFrame::LeaveRoom => break,

            ClientVoiceFrame::Ping => {
                let _ = out_tx.send(ServerVoiceFrame::Pong).await;
            }
        }
    }

    // Cleanup.
    fwd_task.abort();
    write_task.abort();

    {
        let mut room = room_arc.lock().await;
        room.remove(user_id);
        room.broadcast_except(user_id, ServerVoiceFrame::ParticipantLeft { user_id });
    }

    let _ = sqlx::query(
        "UPDATE voice_participants SET left_at = NOW() WHERE room_id = $1 AND user_id = $2 AND left_at IS NULL",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(&state.pool)
    .await;

    state.registry.gc().await;
}

// ---------------------------------------------------------------------------
// Helper: create a voice room row (called from REST API or match provisioning)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub async fn create_voice_room(
    pool: &PgPool,
    channel_id: Option<Uuid>,
) -> Result<String, sqlx::Error> {
    let room_id = Uuid::new_v4();
    let token = format!("vr_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO voice_rooms (id, channel_id, room_token, is_active, created_at)
        VALUES ($1, $2, $3, true, NOW())
        "#,
    )
    .bind(room_id)
    .bind(channel_id)
    .bind(&token)
    .execute(pool)
    .await?;
    Ok(token)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool, gauges: Arc<crate::ws::gauges::WsGauges>) -> Router {
    let registry = VoiceRegistry::new();
    let state = VoiceState {
        pool,
        registry,
        gauges,
    };
    Router::new()
        .route("/ws/voice", get(voice_ws_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_voice_frame_ping() {
        let json = r#"{"type":"ping"}"#;
        let frame: ClientVoiceFrame = serde_json::from_str(json).unwrap();
        matches!(frame, ClientVoiceFrame::Ping);
    }

    #[test]
    fn test_client_voice_frame_mute() {
        let json = r#"{"type":"mute","muted":true}"#;
        let frame: ClientVoiceFrame = serde_json::from_str(json).unwrap();
        if let ClientVoiceFrame::Mute { muted } = frame {
            assert!(muted);
        } else {
            panic!("Expected Mute");
        }
    }

    #[test]
    fn test_server_voice_frame_pong_serialize() {
        let frame = ServerVoiceFrame::Pong;
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("pong"));
    }

    #[test]
    fn test_room_state_add_remove() {
        let room_id = Uuid::new_v4();
        let mut room = VoiceRoomState::new(room_id);
        let uid = Uuid::new_v4();
        let _rx = room.add(uid);
        assert_eq!(room.participant_ids().len(), 1);
        room.remove(uid);
        assert_eq!(room.participant_ids().len(), 0);
    }

    #[test]
    fn test_room_mute_state() {
        let room_id = Uuid::new_v4();
        let mut room = VoiceRoomState::new(room_id);
        let uid = Uuid::new_v4();
        let _rx = room.add(uid);
        room.set_muted(uid, true);
        let snap = room.state_snapshot();
        assert_eq!(snap.len(), 1);
        assert!(snap[0].is_muted);
    }

    #[tokio::test]
    async fn test_voice_registry_create() {
        let registry = VoiceRegistry::new();
        let rid = Uuid::new_v4();
        let r1 = registry.get_or_create(rid, "token_abc").await;
        let r2 = registry.get_or_create(rid, "token_abc").await;
        assert!(Arc::ptr_eq(&r1, &r2));
    }
}
