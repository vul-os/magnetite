//! Platform comms surface — typed client API for Magnetite's Discord-class
//! chat + voice services.
//!
//! In-game Rust code calls this module to:
//! - Join/leave a **text channel** and send/receive [`ChatMessage`]s.
//! - Receive **presence** updates (who is online, what they are doing).
//! - Join/leave a **voice room** and exchange WebRTC signaling (SDP + ICE)
//!   with the backend signaling server.
//!
//! # Wire protocol overview
//!
//! All messages travel over the platform WebSocket connection (the Axum `ws/`
//! layer).  The SDK serialises them as JSON — the same codec used everywhere
//! else in the platform.
//!
//! ```text
//! Client (in-game SDK)                 Magnetite backend (Axum WS)
//!   │                                            │
//!   │── ClientCommsMessage::JoinChannel ────────>│
//!   │<─ ServerCommsMessage::ChannelJoined ────────│
//!   │                                            │
//!   │── ClientCommsMessage::SendMessage ────────>│
//!   │<─ ServerCommsMessage::NewMessage ──────────│  (broadcast to all members)
//!   │                                            │
//!   │── ClientCommsMessage::JoinVoiceRoom ──────>│
//!   │<─ ServerCommsMessage::VoiceReady ──────────│
//!   │── ClientCommsMessage::Voice(Offer{sdp}) ──>│  (WebRTC signaling relay)
//!   │<─ ServerCommsMessage::Voice(Answer{sdp}) ──│
//!   │── ClientCommsMessage::Voice(Ice{..}) ─────>│
//!   │<─ ServerCommsMessage::Voice(Ice{..}) ──────│
//! ```
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::platform::comms::{
//!     ClientCommsMessage, CommsClient, CommsConfig, CommsEvent,
//! };
//!
//! let mut client = CommsClient::new(CommsConfig {
//!     user_id: "u_42".to_string(),
//!     display_name: "Alice".to_string(),
//!     auth_token: "jwt-here".to_string(),
//! });
//!
//! // Join a text channel and send a message.
//! let msg = client.join_channel("channel-001").unwrap();
//! assert!(matches!(msg, ClientCommsMessage::JoinChannel { .. }));
//!
//! let send = client.send_message("channel-001", "gg wp").unwrap();
//! assert!(matches!(send, ClientCommsMessage::SendMessage { .. }));
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shared primitive types
// ---------------------------------------------------------------------------

/// Opaque identifier for a community (server/guild).
pub type CommunityId = String;

/// Opaque identifier for a channel within a community.
pub type ChannelId = String;

/// Opaque identifier for a voice room.
pub type VoiceRoomId = String;

/// Opaque identifier for a platform user.
pub type UserId = String;

/// Opaque identifier for a chat message.
pub type MessageId = String;

/// Unix milliseconds timestamp.
pub type TimestampMs = u64;

// ---------------------------------------------------------------------------
// Chat message
// ---------------------------------------------------------------------------

/// A single chat message exchanged in a text channel.
///
/// ```rust
/// use magnetite_sdk::platform::comms::ChatMessage;
///
/// let msg = ChatMessage {
///     id: "msg-1".to_string(),
///     channel_id: "ch-001".to_string(),
///     author_id: "u-42".to_string(),
///     author_name: "Alice".to_string(),
///     content: "Hello, world!".to_string(),
///     timestamp_ms: 1_700_000_000_000,
///     edited: false,
/// };
///
/// let json = serde_json::to_string(&msg).unwrap();
/// let decoded: ChatMessage = serde_json::from_str(&json).unwrap();
/// assert_eq!(decoded.content, "Hello, world!");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message identifier assigned by the platform.
    pub id: MessageId,
    /// The channel this message belongs to.
    pub channel_id: ChannelId,
    /// User ID of the sender.
    pub author_id: UserId,
    /// Display name of the sender at send time (denormalised for speed).
    pub author_name: String,
    /// The message body (plain text; markdown rendering is a client concern).
    pub content: String,
    /// Wall-clock time at the moment the server persisted the message.
    pub timestamp_ms: TimestampMs,
    /// `true` when the message has been edited since first sent.
    pub edited: bool,
}

// ---------------------------------------------------------------------------
// Presence
// ---------------------------------------------------------------------------

/// Online/availability status of a user.
///
/// ```rust
/// use magnetite_sdk::platform::comms::PresenceStatus;
///
/// let s = PresenceStatus::InGame { game_id: "shooter-x".to_string() };
/// let json = serde_json::to_string(&s).unwrap();
/// let back: PresenceStatus = serde_json::from_str(&json).unwrap();
/// assert!(matches!(back, PresenceStatus::InGame { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PresenceStatus {
    /// User is online and idle (not in a game).
    Online,
    /// User is in a game.
    InGame {
        /// The platform game identifier.
        game_id: String,
    },
    /// User is in a voice room.
    InVoice {
        /// The voice room they are in.
        room_id: VoiceRoomId,
    },
    /// User is temporarily away.
    Away,
    /// User appears offline (invisible mode).
    Offline,
}

/// A presence update for one user.
///
/// ```rust
/// use magnetite_sdk::platform::comms::{PresenceStatus, PresenceUpdate};
///
/// let update = PresenceUpdate {
///     user_id: "u-7".to_string(),
///     display_name: "Bob".to_string(),
///     status: PresenceStatus::Online,
/// };
/// let json = serde_json::to_string(&update).unwrap();
/// let back: PresenceUpdate = serde_json::from_str(&json).unwrap();
/// assert_eq!(back.user_id, "u-7");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceUpdate {
    /// The user whose presence changed.
    pub user_id: UserId,
    /// Current display name (may change, denormalised).
    pub display_name: String,
    /// Their new status.
    pub status: PresenceStatus,
}

// ---------------------------------------------------------------------------
// WebRTC voice signaling
// ---------------------------------------------------------------------------

/// WebRTC signaling messages exchanged between peers via the backend relay.
///
/// The backend acts as the signaling server: it receives a signal from one
/// peer and relays it to the intended recipient(s).  For small rooms the
/// resulting topology is a **mesh** (each peer connects to every other peer
/// directly).  For large rooms the scale path is an SFU (e.g. LiveKit or
/// mediasoup) — documented in the backend architecture docs.
///
/// ```rust
/// use magnetite_sdk::platform::comms::VoiceSignal;
///
/// let offer = VoiceSignal::Offer {
///     from_user: "u-1".to_string(),
///     to_user: "u-2".to_string(),
///     sdp: "v=0\r\n...".to_string(),
/// };
/// let json = serde_json::to_string(&offer).unwrap();
/// let back: VoiceSignal = serde_json::from_str(&json).unwrap();
/// assert!(matches!(back, VoiceSignal::Offer { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "signal_type", rename_all = "snake_case")]
pub enum VoiceSignal {
    /// SDP offer from one peer to another (initiating the WebRTC handshake).
    Offer {
        /// The user sending the offer.
        from_user: UserId,
        /// The intended recipient.
        to_user: UserId,
        /// Full SDP offer string.
        sdp: String,
    },

    /// SDP answer — sent in response to an [`VoiceSignal::Offer`].
    Answer {
        /// The user sending the answer.
        from_user: UserId,
        /// The user who sent the original offer.
        to_user: UserId,
        /// Full SDP answer string.
        sdp: String,
    },

    /// ICE candidate — sent by either side during candidate gathering.
    Ice {
        /// The user sending this candidate.
        from_user: UserId,
        /// The intended recipient.
        to_user: UserId,
        /// The ICE candidate string (RFC 5245 format).
        candidate: String,
        /// SDP media line index this candidate belongs to.
        sdp_mid: Option<String>,
        /// SDP media line index (numeric form).
        sdp_m_line_index: Option<u16>,
    },

    /// Sent by the server when all candidates have been gathered (end of
    /// candidates for one side).
    EndOfCandidates {
        /// The user whose candidate gathering is complete.
        from_user: UserId,
        /// The intended recipient.
        to_user: UserId,
    },
}

// ---------------------------------------------------------------------------
// Client → Platform messages
// ---------------------------------------------------------------------------

/// Messages sent **from** in-game Rust code **to** the Magnetite platform.
///
/// Serialise with [`serde_json`] and send over the WebSocket connection.
///
/// ```rust
/// use magnetite_sdk::platform::comms::ClientCommsMessage;
///
/// let msg = ClientCommsMessage::JoinChannel {
///     channel_id: "ch-alpha".to_string(),
/// };
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ClientCommsMessage = serde_json::from_str(&json).unwrap();
/// assert!(matches!(back, ClientCommsMessage::JoinChannel { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCommsMessage {
    // -- Text channel messages --
    /// Subscribe to real-time messages in a text channel.
    JoinChannel {
        /// The channel to join.
        channel_id: ChannelId,
    },

    /// Unsubscribe from a text channel.
    LeaveChannel {
        /// The channel to leave.
        channel_id: ChannelId,
    },

    /// Send a chat message to a channel.
    SendMessage {
        /// Target channel.
        channel_id: ChannelId,
        /// Message body (plain text).
        content: String,
        /// Optional client-generated nonce for deduplication (echo'd back).
        nonce: Option<String>,
    },

    /// Delete a previously sent message (author or moderator only).
    DeleteMessage {
        /// The message to delete.
        message_id: MessageId,
    },

    // -- Presence messages --
    /// Update the local user's presence status.
    UpdatePresence {
        /// The new status.
        status: PresenceStatus,
    },

    // -- Voice room messages --
    /// Join a voice room and start signaling.
    JoinVoiceRoom {
        /// The voice room to join.
        room_id: VoiceRoomId,
        /// Whether this client wants to publish audio (vs. listen-only).
        publish_audio: bool,
    },

    /// Leave a voice room and tear down WebRTC connections.
    LeaveVoiceRoom {
        /// The voice room to leave.
        room_id: VoiceRoomId,
    },

    /// Relay a WebRTC signaling message to another peer.
    Voice(VoiceSignal),
}

// ---------------------------------------------------------------------------
// Platform → Client messages
// ---------------------------------------------------------------------------

/// Messages sent **from** the Magnetite platform **to** in-game Rust code.
///
/// Deserialise from the WebSocket connection.
///
/// ```rust
/// use magnetite_sdk::platform::comms::{ChatMessage, ServerCommsMessage};
///
/// let msg = ServerCommsMessage::ChannelJoined {
///     channel_id: "ch-alpha".to_string(),
///     recent_messages: vec![],
/// };
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ServerCommsMessage = serde_json::from_str(&json).unwrap();
/// assert!(matches!(back, ServerCommsMessage::ChannelJoined { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerCommsMessage {
    // -- Text channel events --
    /// Acknowledgement that the client successfully joined a channel.
    ChannelJoined {
        /// The channel that was joined.
        channel_id: ChannelId,
        /// The most recent messages (newest last), for initial rendering.
        recent_messages: Vec<ChatMessage>,
    },

    /// Acknowledgement that the client left a channel.
    ChannelLeft {
        /// The channel that was left.
        channel_id: ChannelId,
    },

    /// A new message arrived in a subscribed channel.
    NewMessage(ChatMessage),

    /// A message was deleted in a subscribed channel.
    MessageDeleted {
        /// The channel the message was in.
        channel_id: ChannelId,
        /// The message that was deleted.
        message_id: MessageId,
    },

    // -- Presence events --
    /// Presence status changed for a user in a shared context.
    PresenceChanged(PresenceUpdate),

    /// Bulk presence snapshot — sent on channel join so the client can
    /// populate the member list without N individual events.
    PresenceSnapshot {
        /// All members currently visible in this channel's community.
        members: Vec<PresenceUpdate>,
    },

    // -- Voice room events --
    /// Acknowledgement that the client joined a voice room; contains the
    /// list of peers already in the room so the client can initiate offers.
    VoiceReady {
        /// The voice room that was joined.
        room_id: VoiceRoomId,
        /// Users currently in the room (excluding the local user).
        peers: Vec<UserId>,
    },

    /// Another user joined the voice room.
    VoicePeerJoined {
        /// The voice room.
        room_id: VoiceRoomId,
        /// The new peer.
        user_id: UserId,
    },

    /// A user left the voice room.
    VoicePeerLeft {
        /// The voice room.
        room_id: VoiceRoomId,
        /// The user who left.
        user_id: UserId,
    },

    /// A relayed WebRTC signaling message from another peer.
    Voice(VoiceSignal),

    /// An error from the platform comms layer.
    Error {
        /// Machine-readable error code.
        code: CommsErrorCode,
        /// Human-readable description.
        message: String,
    },
}

/// Error codes from the platform comms layer.
///
/// ```rust
/// use magnetite_sdk::platform::comms::CommsErrorCode;
///
/// let code = CommsErrorCode::NotInChannel;
/// let json = serde_json::to_string(&code).unwrap();
/// let back: CommsErrorCode = serde_json::from_str(&json).unwrap();
/// assert_eq!(code, back);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommsErrorCode {
    /// The request requires authentication.
    Unauthorized,
    /// The channel or room was not found.
    NotFound,
    /// The channel or room is at capacity.
    CapacityExceeded,
    /// The operation was rejected by a permissions check.
    Forbidden,
    /// The client is not currently subscribed to the channel.
    NotInChannel,
    /// The client is not in the specified voice room.
    NotInVoiceRoom,
    /// The platform encountered an internal error.
    Internal,
    /// The request was malformed.
    BadRequest,
}

// ---------------------------------------------------------------------------
// Comms client state machine
// ---------------------------------------------------------------------------

/// Configuration for the in-game comms client.
///
/// ```rust
/// use magnetite_sdk::platform::comms::CommsConfig;
///
/// let cfg = CommsConfig {
///     user_id: "u-99".to_string(),
///     display_name: "Eve".to_string(),
///     auth_token: "bearer-xyz".to_string(),
/// };
/// assert_eq!(cfg.user_id, "u-99");
/// ```
#[derive(Debug, Clone)]
pub struct CommsConfig {
    /// Platform user ID (from the identity/auth service).
    pub user_id: UserId,
    /// Display name shown to other users.
    pub display_name: String,
    /// Auth token used to authenticate the WebSocket connection.
    pub auth_token: String,
}

/// Connection state of the comms client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommsConnectionState {
    /// Not yet connected.
    Disconnected,
    /// Connection established, not yet authenticated.
    Connected,
    /// Fully authenticated and ready.
    Ready,
}

/// An inbound event surfaced to game code by [`CommsClient::handle_server_message`].
///
/// The game's chat/voice UI layer should match on these variants.
#[derive(Debug, Clone, PartialEq)]
pub enum CommsEvent {
    /// Successfully joined a text channel.
    JoinedChannel {
        channel_id: ChannelId,
        recent_messages: Vec<ChatMessage>,
    },
    /// Left a text channel.
    LeftChannel { channel_id: ChannelId },
    /// A new chat message arrived.
    Message(ChatMessage),
    /// A message was deleted.
    MessageDeleted {
        channel_id: ChannelId,
        message_id: MessageId,
    },
    /// Presence changed for one user.
    Presence(PresenceUpdate),
    /// Bulk presence snapshot on channel join.
    PresenceSnapshot(Vec<PresenceUpdate>),
    /// Successfully joined a voice room; start sending offers to `peers`.
    VoiceReady {
        room_id: VoiceRoomId,
        peers: Vec<UserId>,
    },
    /// Another peer joined the voice room.
    VoicePeerJoined {
        room_id: VoiceRoomId,
        user_id: UserId,
    },
    /// A peer left the voice room.
    VoicePeerLeft {
        room_id: VoiceRoomId,
        user_id: UserId,
    },
    /// An inbound WebRTC signaling message (forward to the WebRTC layer).
    VoiceSignal(VoiceSignal),
    /// An error from the platform.
    Error {
        code: CommsErrorCode,
        message: String,
    },
}

/// Typed, stateful in-game comms client.
///
/// Maintains the local subscriptions and voice-room membership so the game
/// layer can query them without parsing raw messages.
///
/// **No I/O is performed by this struct** — the game integration layer is
/// responsible for sending the returned [`ClientCommsMessage`] bytes over the
/// actual WebSocket and passing received bytes into
/// [`CommsClient::handle_server_message`].  This makes the client usable in
/// both async (tokio) and sync (Bevy plugin) contexts.
///
/// ```rust
/// use magnetite_sdk::platform::comms::{
///     ClientCommsMessage, CommsClient, CommsConfig, CommsEvent,
///     ServerCommsMessage, PresenceStatus,
/// };
///
/// let mut client = CommsClient::new(CommsConfig {
///     user_id: "u-1".to_string(),
///     display_name: "Rust Dev".to_string(),
///     auth_token: "tok".to_string(),
/// });
///
/// // Build a join-channel message.
/// let out = client.join_channel("ch-main").unwrap();
/// assert!(matches!(out, ClientCommsMessage::JoinChannel { .. }));
/// assert!(client.is_in_channel("ch-main"));
///
/// // Simulate receiving a server ack.
/// let server_msg = ServerCommsMessage::ChannelJoined {
///     channel_id: "ch-main".to_string(),
///     recent_messages: vec![],
/// };
/// let event = client.handle_server_message(server_msg).unwrap();
/// assert!(matches!(event, CommsEvent::JoinedChannel { .. }));
/// ```
pub struct CommsClient {
    config: CommsConfig,
    state: CommsConnectionState,
    /// Text channels the local user is currently subscribed to.
    joined_channels: Vec<ChannelId>,
    /// Voice rooms the local user is currently in.
    joined_voice_rooms: Vec<VoiceRoomId>,
}

impl CommsClient {
    /// Create a new client with the given configuration.
    ///
    /// The client starts in [`CommsConnectionState::Disconnected`]; call
    /// [`CommsClient::mark_connected`] and [`CommsClient::mark_ready`] from
    /// your WS connection lifecycle callbacks.
    pub fn new(config: CommsConfig) -> Self {
        Self {
            config,
            state: CommsConnectionState::Disconnected,
            joined_channels: Vec::new(),
            joined_voice_rooms: Vec::new(),
        }
    }

    // -- Lifecycle --

    /// Mark the WebSocket connection as established (before auth).
    pub fn mark_connected(&mut self) {
        self.state = CommsConnectionState::Connected;
    }

    /// Mark the connection as fully authenticated and ready.
    pub fn mark_ready(&mut self) {
        self.state = CommsConnectionState::Ready;
    }

    /// Reset to disconnected state (e.g. on socket close).
    pub fn mark_disconnected(&mut self) {
        self.state = CommsConnectionState::Disconnected;
        self.joined_channels.clear();
        self.joined_voice_rooms.clear();
    }

    /// Current connection state.
    pub fn connection_state(&self) -> CommsConnectionState {
        self.state
    }

    // -- Text channel operations --

    /// Build a [`ClientCommsMessage::JoinChannel`] and record the subscription
    /// locally.  Returns an error string if already joined.
    pub fn join_channel(&mut self, channel_id: &str) -> Result<ClientCommsMessage, &'static str> {
        if self.joined_channels.iter().any(|c| c == channel_id) {
            return Err("already in channel");
        }
        self.joined_channels.push(channel_id.to_string());
        Ok(ClientCommsMessage::JoinChannel {
            channel_id: channel_id.to_string(),
        })
    }

    /// Build a [`ClientCommsMessage::LeaveChannel`] and clear the local
    /// subscription.  Returns an error string if not subscribed.
    pub fn leave_channel(&mut self, channel_id: &str) -> Result<ClientCommsMessage, &'static str> {
        let pos = self
            .joined_channels
            .iter()
            .position(|c| c == channel_id)
            .ok_or("not in channel")?;
        self.joined_channels.swap_remove(pos);
        Ok(ClientCommsMessage::LeaveChannel {
            channel_id: channel_id.to_string(),
        })
    }

    /// Build a [`ClientCommsMessage::SendMessage`].
    pub fn send_message(
        &mut self,
        channel_id: &str,
        content: &str,
    ) -> Result<ClientCommsMessage, &'static str> {
        if !self.joined_channels.iter().any(|c| c == channel_id) {
            return Err("not in channel");
        }
        Ok(ClientCommsMessage::SendMessage {
            channel_id: channel_id.to_string(),
            content: content.to_string(),
            nonce: None,
        })
    }

    /// Returns `true` when the client is subscribed to the given channel.
    pub fn is_in_channel(&self, channel_id: &str) -> bool {
        self.joined_channels.iter().any(|c| c == channel_id)
    }

    /// Snapshot of currently subscribed channel IDs.
    pub fn channels(&self) -> &[ChannelId] {
        &self.joined_channels
    }

    // -- Presence operations --

    /// Build a [`ClientCommsMessage::UpdatePresence`].
    pub fn update_presence(&self, status: PresenceStatus) -> ClientCommsMessage {
        ClientCommsMessage::UpdatePresence { status }
    }

    // -- Voice room operations --

    /// Build a [`ClientCommsMessage::JoinVoiceRoom`] and record membership.
    pub fn join_voice_room(
        &mut self,
        room_id: &str,
        publish_audio: bool,
    ) -> Result<ClientCommsMessage, &'static str> {
        if self.joined_voice_rooms.iter().any(|r| r == room_id) {
            return Err("already in voice room");
        }
        self.joined_voice_rooms.push(room_id.to_string());
        Ok(ClientCommsMessage::JoinVoiceRoom {
            room_id: room_id.to_string(),
            publish_audio,
        })
    }

    /// Build a [`ClientCommsMessage::LeaveVoiceRoom`] and clear membership.
    pub fn leave_voice_room(&mut self, room_id: &str) -> Result<ClientCommsMessage, &'static str> {
        let pos = self
            .joined_voice_rooms
            .iter()
            .position(|r| r == room_id)
            .ok_or("not in voice room")?;
        self.joined_voice_rooms.swap_remove(pos);
        Ok(ClientCommsMessage::LeaveVoiceRoom {
            room_id: room_id.to_string(),
        })
    }

    /// Build a [`ClientCommsMessage::Voice`] wrapping a [`VoiceSignal`].
    pub fn send_voice_signal(&self, signal: VoiceSignal) -> ClientCommsMessage {
        ClientCommsMessage::Voice(signal)
    }

    /// Returns `true` when the client is in the given voice room.
    pub fn is_in_voice_room(&self, room_id: &str) -> bool {
        self.joined_voice_rooms.iter().any(|r| r == room_id)
    }

    /// Snapshot of currently joined voice room IDs.
    pub fn voice_rooms(&self) -> &[VoiceRoomId] {
        &self.joined_voice_rooms
    }

    // -- Inbound message dispatch --

    /// Process a [`ServerCommsMessage`] received from the platform and return
    /// the corresponding [`CommsEvent`] for the game layer.
    ///
    /// Also updates local state (e.g. marks the client as having left a channel
    /// if the server confirms it).
    pub fn handle_server_message(
        &mut self,
        msg: ServerCommsMessage,
    ) -> Result<CommsEvent, &'static str> {
        match msg {
            ServerCommsMessage::ChannelJoined {
                channel_id,
                recent_messages,
            } => Ok(CommsEvent::JoinedChannel {
                channel_id,
                recent_messages,
            }),

            ServerCommsMessage::ChannelLeft { channel_id } => {
                // Ensure the channel is removed from local state even if the
                // client called leave_channel before receiving the ack.
                self.joined_channels.retain(|c| c != &channel_id);
                Ok(CommsEvent::LeftChannel { channel_id })
            }

            ServerCommsMessage::NewMessage(msg) => Ok(CommsEvent::Message(msg)),

            ServerCommsMessage::MessageDeleted {
                channel_id,
                message_id,
            } => Ok(CommsEvent::MessageDeleted {
                channel_id,
                message_id,
            }),

            ServerCommsMessage::PresenceChanged(update) => Ok(CommsEvent::Presence(update)),

            ServerCommsMessage::PresenceSnapshot { members } => {
                Ok(CommsEvent::PresenceSnapshot(members))
            }

            ServerCommsMessage::VoiceReady { room_id, peers } => {
                Ok(CommsEvent::VoiceReady { room_id, peers })
            }

            ServerCommsMessage::VoicePeerJoined { room_id, user_id } => {
                Ok(CommsEvent::VoicePeerJoined { room_id, user_id })
            }

            ServerCommsMessage::VoicePeerLeft { room_id, user_id } => {
                // Clean up local state if the local user was kicked.
                self.joined_voice_rooms.retain(|r| r != &room_id);
                Ok(CommsEvent::VoicePeerLeft { room_id, user_id })
            }

            ServerCommsMessage::Voice(signal) => Ok(CommsEvent::VoiceSignal(signal)),

            ServerCommsMessage::Error { code, message } => Ok(CommsEvent::Error { code, message }),
        }
    }

    /// The authenticated user ID.
    pub fn user_id(&self) -> &str {
        &self.config.user_id
    }

    /// The authenticated user's display name.
    pub fn display_name(&self) -> &str {
        &self.config.display_name
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helper --
    fn test_client() -> CommsClient {
        CommsClient::new(CommsConfig {
            user_id: "u-test".to_string(),
            display_name: "Tester".to_string(),
            auth_token: "tok-123".to_string(),
        })
    }

    // -----------------------------------------------------------------------
    // Serde roundtrip tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_message_roundtrip() {
        let msg = ChatMessage {
            id: "m-1".to_string(),
            channel_id: "ch-1".to_string(),
            author_id: "u-1".to_string(),
            author_name: "Alice".to_string(),
            content: "hello!".to_string(),
            timestamp_ms: 1_700_000_000_000,
            edited: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn presence_status_all_variants_roundtrip() {
        let statuses = [
            PresenceStatus::Online,
            PresenceStatus::InGame {
                game_id: "shooter-x".to_string(),
            },
            PresenceStatus::InVoice {
                room_id: "vr-99".to_string(),
            },
            PresenceStatus::Away,
            PresenceStatus::Offline,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let back: PresenceStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, &back);
        }
    }

    #[test]
    fn presence_update_roundtrip() {
        let update = PresenceUpdate {
            user_id: "u-7".to_string(),
            display_name: "Bob".to_string(),
            status: PresenceStatus::InGame {
                game_id: "fps-starter".to_string(),
            },
        };
        let json = serde_json::to_string(&update).unwrap();
        let back: PresenceUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(update, back);
    }

    #[test]
    fn voice_signal_offer_roundtrip() {
        let signal = VoiceSignal::Offer {
            from_user: "u-1".to_string(),
            to_user: "u-2".to_string(),
            sdp: "v=0\r\no=- 12345 2 IN IP4 127.0.0.1\r\n".to_string(),
        };
        let json = serde_json::to_string(&signal).unwrap();
        let back: VoiceSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(signal, back);
    }

    #[test]
    fn voice_signal_answer_roundtrip() {
        let signal = VoiceSignal::Answer {
            from_user: "u-2".to_string(),
            to_user: "u-1".to_string(),
            sdp: "v=0\r\no=- 99999 2 IN IP4 127.0.0.1\r\n".to_string(),
        };
        let json = serde_json::to_string(&signal).unwrap();
        let back: VoiceSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(signal, back);
    }

    #[test]
    fn voice_signal_ice_roundtrip() {
        let signal = VoiceSignal::Ice {
            from_user: "u-1".to_string(),
            to_user: "u-2".to_string(),
            candidate: "candidate:0 1 UDP 2130706431 192.168.1.1 54321 typ host".to_string(),
            sdp_mid: Some("0".to_string()),
            sdp_m_line_index: Some(0),
        };
        let json = serde_json::to_string(&signal).unwrap();
        let back: VoiceSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(signal, back);
    }

    #[test]
    fn voice_signal_end_of_candidates_roundtrip() {
        let signal = VoiceSignal::EndOfCandidates {
            from_user: "u-1".to_string(),
            to_user: "u-2".to_string(),
        };
        let json = serde_json::to_string(&signal).unwrap();
        let back: VoiceSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(signal, back);
    }

    #[test]
    fn client_comms_message_all_variants_roundtrip() {
        let msgs: Vec<ClientCommsMessage> = vec![
            ClientCommsMessage::JoinChannel {
                channel_id: "ch-1".to_string(),
            },
            ClientCommsMessage::LeaveChannel {
                channel_id: "ch-1".to_string(),
            },
            ClientCommsMessage::SendMessage {
                channel_id: "ch-1".to_string(),
                content: "hello".to_string(),
                nonce: Some("abc".to_string()),
            },
            ClientCommsMessage::DeleteMessage {
                message_id: "msg-99".to_string(),
            },
            ClientCommsMessage::UpdatePresence {
                status: PresenceStatus::Away,
            },
            ClientCommsMessage::JoinVoiceRoom {
                room_id: "vr-1".to_string(),
                publish_audio: true,
            },
            ClientCommsMessage::LeaveVoiceRoom {
                room_id: "vr-1".to_string(),
            },
            ClientCommsMessage::Voice(VoiceSignal::EndOfCandidates {
                from_user: "u-1".to_string(),
                to_user: "u-2".to_string(),
            }),
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ClientCommsMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn server_comms_message_all_variants_roundtrip() {
        let msgs: Vec<ServerCommsMessage> = vec![
            ServerCommsMessage::ChannelJoined {
                channel_id: "ch-1".to_string(),
                recent_messages: vec![],
            },
            ServerCommsMessage::ChannelLeft {
                channel_id: "ch-1".to_string(),
            },
            ServerCommsMessage::NewMessage(ChatMessage {
                id: "m-2".to_string(),
                channel_id: "ch-1".to_string(),
                author_id: "u-3".to_string(),
                author_name: "Charlie".to_string(),
                content: "wave 6!".to_string(),
                timestamp_ms: 0,
                edited: false,
            }),
            ServerCommsMessage::MessageDeleted {
                channel_id: "ch-1".to_string(),
                message_id: "m-2".to_string(),
            },
            ServerCommsMessage::PresenceChanged(PresenceUpdate {
                user_id: "u-3".to_string(),
                display_name: "Charlie".to_string(),
                status: PresenceStatus::Offline,
            }),
            ServerCommsMessage::PresenceSnapshot { members: vec![] },
            ServerCommsMessage::VoiceReady {
                room_id: "vr-1".to_string(),
                peers: vec!["u-4".to_string()],
            },
            ServerCommsMessage::VoicePeerJoined {
                room_id: "vr-1".to_string(),
                user_id: "u-5".to_string(),
            },
            ServerCommsMessage::VoicePeerLeft {
                room_id: "vr-1".to_string(),
                user_id: "u-5".to_string(),
            },
            ServerCommsMessage::Voice(VoiceSignal::Offer {
                from_user: "u-4".to_string(),
                to_user: "u-test".to_string(),
                sdp: "v=0\r\n".to_string(),
            }),
            ServerCommsMessage::Error {
                code: CommsErrorCode::NotFound,
                message: "channel not found".to_string(),
            },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ServerCommsMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn comms_error_code_roundtrip() {
        let codes = [
            CommsErrorCode::Unauthorized,
            CommsErrorCode::NotFound,
            CommsErrorCode::CapacityExceeded,
            CommsErrorCode::Forbidden,
            CommsErrorCode::NotInChannel,
            CommsErrorCode::NotInVoiceRoom,
            CommsErrorCode::Internal,
            CommsErrorCode::BadRequest,
        ];
        for code in &codes {
            let json = serde_json::to_string(code).unwrap();
            let back: CommsErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, &back);
        }
    }

    // -----------------------------------------------------------------------
    // CommsClient state machine tests
    // -----------------------------------------------------------------------

    #[test]
    fn client_initial_state_is_disconnected() {
        let client = test_client();
        assert_eq!(
            client.connection_state(),
            CommsConnectionState::Disconnected
        );
        assert!(client.channels().is_empty());
        assert!(client.voice_rooms().is_empty());
    }

    #[test]
    fn client_lifecycle_transitions() {
        let mut client = test_client();
        client.mark_connected();
        assert_eq!(client.connection_state(), CommsConnectionState::Connected);
        client.mark_ready();
        assert_eq!(client.connection_state(), CommsConnectionState::Ready);
        client.mark_disconnected();
        assert_eq!(
            client.connection_state(),
            CommsConnectionState::Disconnected
        );
    }

    #[test]
    fn client_join_channel_tracks_state() {
        let mut client = test_client();
        let msg = client.join_channel("ch-alpha").unwrap();
        assert!(
            matches!(msg, ClientCommsMessage::JoinChannel { channel_id } if channel_id == "ch-alpha")
        );
        assert!(client.is_in_channel("ch-alpha"));
        assert!(!client.is_in_channel("ch-beta"));
    }

    #[test]
    fn client_join_channel_rejects_duplicate() {
        let mut client = test_client();
        client.join_channel("ch-alpha").unwrap();
        let err = client.join_channel("ch-alpha");
        assert!(err.is_err());
    }

    #[test]
    fn client_leave_channel_clears_state() {
        let mut client = test_client();
        client.join_channel("ch-alpha").unwrap();
        let msg = client.leave_channel("ch-alpha").unwrap();
        assert!(
            matches!(msg, ClientCommsMessage::LeaveChannel { channel_id } if channel_id == "ch-alpha")
        );
        assert!(!client.is_in_channel("ch-alpha"));
    }

    #[test]
    fn client_leave_channel_rejects_unknown() {
        let mut client = test_client();
        assert!(client.leave_channel("ch-nope").is_err());
    }

    #[test]
    fn client_send_message_requires_channel_membership() {
        let mut client = test_client();
        assert!(client.send_message("ch-x", "hi").is_err());
        client.join_channel("ch-x").unwrap();
        let msg = client.send_message("ch-x", "hi").unwrap();
        assert!(matches!(msg, ClientCommsMessage::SendMessage { content, .. } if content == "hi"));
    }

    #[test]
    fn client_update_presence_builds_correct_message() {
        let client = test_client();
        let msg = client.update_presence(PresenceStatus::Away);
        assert!(matches!(
            msg,
            ClientCommsMessage::UpdatePresence {
                status: PresenceStatus::Away
            }
        ));
    }

    #[test]
    fn client_join_voice_room_tracks_state() {
        let mut client = test_client();
        let msg = client.join_voice_room("vr-1", true).unwrap();
        assert!(
            matches!(msg, ClientCommsMessage::JoinVoiceRoom { room_id, .. } if room_id == "vr-1")
        );
        assert!(client.is_in_voice_room("vr-1"));
        assert!(!client.is_in_voice_room("vr-2"));
    }

    #[test]
    fn client_join_voice_room_rejects_duplicate() {
        let mut client = test_client();
        client.join_voice_room("vr-1", true).unwrap();
        assert!(client.join_voice_room("vr-1", false).is_err());
    }

    #[test]
    fn client_leave_voice_room_clears_state() {
        let mut client = test_client();
        client.join_voice_room("vr-1", true).unwrap();
        let msg = client.leave_voice_room("vr-1").unwrap();
        assert!(matches!(msg, ClientCommsMessage::LeaveVoiceRoom { room_id } if room_id == "vr-1"));
        assert!(!client.is_in_voice_room("vr-1"));
    }

    #[test]
    fn client_leave_voice_room_rejects_unknown() {
        let mut client = test_client();
        assert!(client.leave_voice_room("vr-nope").is_err());
    }

    #[test]
    fn client_send_voice_signal() {
        let client = test_client();
        let signal = VoiceSignal::Offer {
            from_user: "u-test".to_string(),
            to_user: "u-other".to_string(),
            sdp: "v=0\r\n".to_string(),
        };
        let msg = client.send_voice_signal(signal.clone());
        assert!(matches!(msg, ClientCommsMessage::Voice(s) if s == signal));
    }

    #[test]
    fn client_handle_channel_joined_event() {
        let mut client = test_client();
        client.join_channel("ch-1").unwrap();
        let event = client
            .handle_server_message(ServerCommsMessage::ChannelJoined {
                channel_id: "ch-1".to_string(),
                recent_messages: vec![],
            })
            .unwrap();
        assert!(matches!(event, CommsEvent::JoinedChannel { .. }));
    }

    #[test]
    fn client_handle_channel_left_clears_channel() {
        let mut client = test_client();
        client.join_channel("ch-1").unwrap();
        // Simulate server-side leave (e.g. kicked).
        let event = client
            .handle_server_message(ServerCommsMessage::ChannelLeft {
                channel_id: "ch-1".to_string(),
            })
            .unwrap();
        assert!(matches!(event, CommsEvent::LeftChannel { .. }));
        assert!(!client.is_in_channel("ch-1"));
    }

    #[test]
    fn client_handle_new_message_event() {
        let mut client = test_client();
        client.join_channel("ch-1").unwrap();
        let chat = ChatMessage {
            id: "m-1".to_string(),
            channel_id: "ch-1".to_string(),
            author_id: "u-2".to_string(),
            author_name: "Dave".to_string(),
            content: "pong".to_string(),
            timestamp_ms: 42,
            edited: false,
        };
        let event = client
            .handle_server_message(ServerCommsMessage::NewMessage(chat.clone()))
            .unwrap();
        assert!(matches!(event, CommsEvent::Message(m) if m == chat));
    }

    #[test]
    fn client_handle_presence_changed_event() {
        let mut client = test_client();
        let update = PresenceUpdate {
            user_id: "u-5".to_string(),
            display_name: "Eve".to_string(),
            status: PresenceStatus::Online,
        };
        let event = client
            .handle_server_message(ServerCommsMessage::PresenceChanged(update.clone()))
            .unwrap();
        assert!(matches!(event, CommsEvent::Presence(u) if u == update));
    }

    #[test]
    fn client_handle_presence_snapshot_event() {
        let mut client = test_client();
        let event = client
            .handle_server_message(ServerCommsMessage::PresenceSnapshot { members: vec![] })
            .unwrap();
        assert!(matches!(event, CommsEvent::PresenceSnapshot(_)));
    }

    #[test]
    fn client_handle_voice_ready_event() {
        let mut client = test_client();
        client.join_voice_room("vr-1", true).unwrap();
        let event = client
            .handle_server_message(ServerCommsMessage::VoiceReady {
                room_id: "vr-1".to_string(),
                peers: vec!["u-2".to_string()],
            })
            .unwrap();
        assert!(matches!(event, CommsEvent::VoiceReady { .. }));
    }

    #[test]
    fn client_handle_voice_peer_joined() {
        let mut client = test_client();
        client.join_voice_room("vr-1", true).unwrap();
        let event = client
            .handle_server_message(ServerCommsMessage::VoicePeerJoined {
                room_id: "vr-1".to_string(),
                user_id: "u-3".to_string(),
            })
            .unwrap();
        assert!(matches!(event, CommsEvent::VoicePeerJoined { .. }));
    }

    #[test]
    fn client_handle_voice_peer_left_clears_room_if_kicked() {
        let mut client = test_client();
        client.join_voice_room("vr-1", true).unwrap();
        // Server says the local user left (e.g. kicked) — still matches VoicePeerLeft.
        let event = client
            .handle_server_message(ServerCommsMessage::VoicePeerLeft {
                room_id: "vr-1".to_string(),
                user_id: "u-test".to_string(),
            })
            .unwrap();
        assert!(matches!(event, CommsEvent::VoicePeerLeft { .. }));
        // Room is cleaned up from local state.
        assert!(!client.is_in_voice_room("vr-1"));
    }

    #[test]
    fn client_handle_voice_signal_event() {
        let mut client = test_client();
        let signal = VoiceSignal::Ice {
            from_user: "u-2".to_string(),
            to_user: "u-test".to_string(),
            candidate: "candidate:0 1 UDP 2130706431 10.0.0.1 1234 typ host".to_string(),
            sdp_mid: None,
            sdp_m_line_index: None,
        };
        let event = client
            .handle_server_message(ServerCommsMessage::Voice(signal.clone()))
            .unwrap();
        assert!(matches!(event, CommsEvent::VoiceSignal(s) if s == signal));
    }

    #[test]
    fn client_handle_error_event() {
        let mut client = test_client();
        let event = client
            .handle_server_message(ServerCommsMessage::Error {
                code: CommsErrorCode::Forbidden,
                message: "access denied".to_string(),
            })
            .unwrap();
        assert!(matches!(
            event,
            CommsEvent::Error {
                code: CommsErrorCode::Forbidden,
                ..
            }
        ));
    }

    #[test]
    fn client_disconnect_clears_all_state() {
        let mut client = test_client();
        client.mark_ready();
        client.join_channel("ch-1").unwrap();
        client.join_voice_room("vr-1", true).unwrap();
        client.mark_disconnected();
        assert!(client.channels().is_empty());
        assert!(client.voice_rooms().is_empty());
        assert_eq!(
            client.connection_state(),
            CommsConnectionState::Disconnected
        );
    }

    #[test]
    fn client_user_id_and_display_name() {
        let client = test_client();
        assert_eq!(client.user_id(), "u-test");
        assert_eq!(client.display_name(), "Tester");
    }
}
