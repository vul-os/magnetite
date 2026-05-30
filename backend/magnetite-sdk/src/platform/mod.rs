//! Platform services surface — in-game Rust code calls these modules to access
//! the Magnetite platform's shared services.
//!
//! # Services
//!
//! | Module | Purpose |
//! |---|---|
//! | [`comms`] | Text chat, presence, and WebRTC voice signaling |
//! | [`points`] | Points / XP / score economy — award, spend, balance |
//! | [`marketplace`] | In-game store items, purchases, and entitlements |
//! | [`cloud_save`] | Per-player cloud save slots (opaque blobs) |
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

pub mod cloud_save;
pub mod comms;
pub mod marketplace;
pub mod points;

/// Shared primitive type — platform user identifier (opaque string).
///
/// Re-exported from [`comms`] for convenience; used across all platform modules.
pub use comms::UserId;

// Re-export the most commonly used comms types at the platform level.
pub use comms::{
    ChannelId, ChatMessage, ClientCommsMessage, CommsClient, CommsConfig, CommsConnectionState,
    CommsErrorCode, CommsEvent, CommunityId, MessageId, PresenceStatus, PresenceUpdate,
    ServerCommsMessage, TimestampMs, VoiceRoomId, VoiceSignal,
};

// Re-export key points types.
pub use points::{
    AwardPointsRequest, ClientPointsMessage, LedgerEntry, LedgerEntryKind, PointsBalance,
    PointsClient, PointsConfig, PointsErrorCode, ServerPointsMessage, SpendPointsRequest,
};

// Re-export key marketplace types.
pub use marketplace::{
    ClientMarketplaceMessage, Entitlement, ItemType, MarketplaceClient, MarketplaceConfig,
    MarketplaceErrorCode, PaymentMethod, PurchaseErrorCode, PurchaseRequest, PurchaseResult,
    ServerMarketplaceMessage, StoreItem,
};

// Re-export key cloud-save types.
pub use cloud_save::{
    ClientCloudSaveMessage, CloudSaveClient, CloudSaveConfig, CloudSaveErrorCode, SaveRequest,
    SaveSlot, SaveSlotMeta, ServerCloudSaveMessage,
};
