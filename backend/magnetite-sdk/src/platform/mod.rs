//! Platform services surface — in-game Rust code calls these modules to access
//! the Magnetite platform's shared services.
//!
//! # Services
//!
//! | Module | Purpose |
//! |---|---|
//! | [`comms`] | Text chat, presence, and WebRTC voice signaling |
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::platform::comms::{CommsClient, CommsConfig};
//!
//! let client = CommsClient::new(CommsConfig {
//!     user_id: "u-1".to_string(),
//!     display_name: "Player One".to_string(),
//!     auth_token: "jwt-here".to_string(),
//! });
//!
//! assert_eq!(client.user_id(), "u-1");
//! ```

pub mod comms;

// Re-export the most commonly used comms types at the platform level.
pub use comms::{
    ChannelId, ChatMessage, ClientCommsMessage, CommsClient, CommsConfig, CommsConnectionState,
    CommsErrorCode, CommsEvent, CommunityId, MessageId, PresenceStatus, PresenceUpdate,
    ServerCommsMessage, TimestampMs, UserId, VoiceRoomId, VoiceSignal,
};
