//! Platform cloud saves — persist and load arbitrary game state blobs.
//!
//! Cloud saves allow players to resume progress on any device.  The platform
//! stores **named save slots** per player per game; each slot contains an
//! opaque binary blob (JSON, MessagePack, custom format, …) plus metadata.
//!
//! # Design
//!
//! - **Opaque blobs:** the SDK does not interpret the save data; the game
//!   serialises/deserialises it however it likes.
//! - **Named slots:** a player may have multiple slots per game (e.g.
//!   `"slot_1"`, `"autosave"`, `"checkpoint_3"`).
//! - **Conflict resolution:** the backend uses a `version` counter and a
//!   `modified_at_ms` timestamp; callers should compare before overwriting.
//! - **Wire protocol:** all messages travel over the platform WebSocket
//!   (`ws/`), consistent with comms, points, and marketplace.
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::platform::cloud_save::{
//!     CloudSaveClient, CloudSaveConfig, SaveRequest,
//! };
//!
//! let mut client = CloudSaveClient::new(CloudSaveConfig {
//!     user_id: "u-42".to_string(),
//!     game_id: "fps-starter".to_string(),
//!     auth_token: "jwt-here".to_string(),
//! });
//!
//! // Build a save request.
//! let save_data = serde_json::json!({ "level": 3, "score": 9500 });
//! let payload = serde_json::to_vec(&save_data).unwrap();
//!
//! let msg = client.save_message(SaveRequest {
//!     slot: "autosave".to_string(),
//!     data: payload,
//!     version: None,
//! });
//!
//! use magnetite_sdk::platform::cloud_save::ClientCloudSaveMessage;
//! assert!(matches!(msg, ClientCloudSaveMessage::Save(_)));
//!
//! // Build a load request.
//! let load_msg = client.load_message("autosave");
//! assert!(matches!(load_msg, ClientCloudSaveMessage::Load { .. }));
//! ```

use serde::{Deserialize, Serialize};

use super::UserId;

/// Opaque identifier for a save-slot entry.
pub type SaveSlotId = String;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the in-game cloud-save client.
///
/// ```rust
/// use magnetite_sdk::platform::cloud_save::CloudSaveConfig;
///
/// let cfg = CloudSaveConfig {
///     user_id: "u-1".to_string(),
///     game_id: "fps-starter".to_string(),
///     auth_token: "tok".to_string(),
/// };
/// assert_eq!(cfg.game_id, "fps-starter");
/// ```
#[derive(Debug, Clone)]
pub struct CloudSaveConfig {
    /// The authenticated user's platform ID.
    pub user_id: UserId,
    /// The game that owns the save data.
    pub game_id: String,
    /// JWT / session token.
    pub auth_token: String,
}

// ---------------------------------------------------------------------------
// Save slot metadata
// ---------------------------------------------------------------------------

/// Metadata about a save slot (without the full data blob).
///
/// ```rust
/// use magnetite_sdk::platform::cloud_save::SaveSlotMeta;
///
/// let meta = SaveSlotMeta {
///     slot: "autosave".to_string(),
///     user_id: "u-42".to_string(),
///     game_id: "fps-starter".to_string(),
///     version: 7,
///     size_bytes: 1024,
///     created_at_ms: 1_700_000_000_000,
///     modified_at_ms: 1_700_000_100_000,
///     description: Some("Level 3 — boss checkpoint".to_string()),
/// };
/// assert_eq!(meta.version, 7);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveSlotMeta {
    /// Slot name (unique per user per game; e.g. `"slot_1"`, `"autosave"`).
    pub slot: String,
    /// The user this slot belongs to.
    pub user_id: UserId,
    /// The game this slot belongs to.
    pub game_id: String,
    /// Monotonically increasing version counter (starts at 1).
    pub version: u64,
    /// Size of the stored blob in bytes.
    pub size_bytes: u64,
    /// Unix milliseconds when the slot was first created.
    pub created_at_ms: u64,
    /// Unix milliseconds when the slot was last written.
    pub modified_at_ms: u64,
    /// Optional human-readable description (e.g. "Level 3 — checkpoint").
    pub description: Option<String>,
}

/// A full save slot — metadata plus the opaque data blob.
///
/// ```rust
/// use magnetite_sdk::platform::cloud_save::{SaveSlot, SaveSlotMeta};
///
/// let slot = SaveSlot {
///     meta: SaveSlotMeta {
///         slot: "slot_1".to_string(),
///         user_id: "u-1".to_string(),
///         game_id: "fps-starter".to_string(),
///         version: 1,
///         size_bytes: 4,
///         created_at_ms: 0,
///         modified_at_ms: 0,
///         description: None,
///     },
///     data: vec![1, 2, 3, 4],
/// };
/// assert_eq!(slot.data.len(), 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveSlot {
    /// Metadata about this slot.
    pub meta: SaveSlotMeta,
    /// The raw save data blob (game-defined format).
    ///
    /// Serialised as base64 in the JSON wire protocol.
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Request to save (write or overwrite) a slot.
///
/// ```rust
/// use magnetite_sdk::platform::cloud_save::SaveRequest;
///
/// let req = SaveRequest {
///     slot: "autosave".to_string(),
///     data: vec![0xDE, 0xAD, 0xBE, 0xEF],
///     version: Some(3),
/// };
/// assert_eq!(req.slot, "autosave");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveRequest {
    /// The slot name to write to.
    pub slot: String,
    /// The new save data blob.
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
    /// If set, the save will be rejected with [`CloudSaveErrorCode::VersionConflict`]
    /// if the current server version does not match.  Pass `None` to force-overwrite.
    pub version: Option<u64>,
}

// ---------------------------------------------------------------------------
// Client → Platform messages
// ---------------------------------------------------------------------------

/// Messages sent **from** in-game Rust code **to** the Magnetite cloud-save service.
///
/// ```rust
/// use magnetite_sdk::platform::cloud_save::{ClientCloudSaveMessage, SaveRequest};
///
/// let msg = ClientCloudSaveMessage::Save(SaveRequest {
///     slot: "autosave".to_string(),
///     data: vec![1, 2, 3],
///     version: None,
/// });
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ClientCloudSaveMessage = serde_json::from_str(&json).unwrap();
/// assert_eq!(msg, back);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCloudSaveMessage {
    /// Write or overwrite a save slot.
    Save(SaveRequest),
    /// Load a save slot by name.
    Load {
        /// The slot name to load.
        slot: String,
    },
    /// Delete a save slot.
    Delete {
        /// The slot name to delete.
        slot: String,
    },
    /// List all save slots for the current user + game.
    ListSlots,
}

// ---------------------------------------------------------------------------
// Platform → Client messages
// ---------------------------------------------------------------------------

/// Messages sent **from** the Magnetite cloud-save service **to** in-game Rust code.
///
/// ```rust
/// use magnetite_sdk::platform::cloud_save::ServerCloudSaveMessage;
///
/// let msg = ServerCloudSaveMessage::Slots { slots: vec![] };
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ServerCloudSaveMessage = serde_json::from_str(&json).unwrap();
/// assert_eq!(msg, back);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerCloudSaveMessage {
    /// The save was accepted and written.
    Saved {
        /// Updated metadata for the slot.
        meta: SaveSlotMeta,
    },
    /// The requested save data.
    Loaded(SaveSlot),
    /// The slot was deleted.
    Deleted {
        /// The slot that was deleted.
        slot: String,
    },
    /// All save slots for the user + game.
    Slots {
        /// Metadata list (no data blobs — fetch individual slots with [`ClientCloudSaveMessage::Load`]).
        slots: Vec<SaveSlotMeta>,
    },
    /// The save was rejected due to a version conflict.
    VersionConflict {
        /// The slot that had a conflict.
        slot: String,
        /// The version on the server.
        server_version: u64,
        /// The version the client expected.
        client_version: u64,
    },
    /// An error from the cloud-save service.
    Error {
        /// Machine-readable error code.
        code: CloudSaveErrorCode,
        /// Human-readable description.
        message: String,
    },
}

/// Error codes for cloud-save operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloudSaveErrorCode {
    /// The request requires authentication.
    Unauthorized,
    /// The slot was not found.
    NotFound,
    /// The blob is too large (platform limit is ~1 MiB per slot).
    TooLarge,
    /// A version conflict was detected (see [`ServerCloudSaveMessage::VersionConflict`]).
    VersionConflict,
    /// The platform encountered an internal error.
    Internal,
    /// The request was malformed.
    BadRequest,
}

// ---------------------------------------------------------------------------
// Cloud-save client
// ---------------------------------------------------------------------------

/// Typed, stateful in-game cloud-save client.
///
/// Caches slot metadata locally so game code can display the save-slot list
/// without an additional round-trip.
///
/// **No I/O is performed** — the caller sends the returned
/// [`ClientCloudSaveMessage`] and passes received bytes back into
/// [`CloudSaveClient::handle_server_message`].
///
/// ```rust
/// use magnetite_sdk::platform::cloud_save::{
///     CloudSaveClient, CloudSaveConfig, SaveRequest, ServerCloudSaveMessage,
///     SaveSlotMeta,
/// };
///
/// let mut client = CloudSaveClient::new(CloudSaveConfig {
///     user_id: "u-42".to_string(),
///     game_id: "fps-starter".to_string(),
///     auth_token: "tok".to_string(),
/// });
///
/// assert!(client.slots().is_empty());
///
/// // Simulate a successful save response.
/// client.handle_server_message(ServerCloudSaveMessage::Saved {
///     meta: SaveSlotMeta {
///         slot: "autosave".to_string(),
///         user_id: "u-42".to_string(),
///         game_id: "fps-starter".to_string(),
///         version: 1,
///         size_bytes: 128,
///         created_at_ms: 0,
///         modified_at_ms: 0,
///         description: None,
///     },
/// });
///
/// assert_eq!(client.slots().len(), 1);
/// assert!(client.slot_meta("autosave").is_some());
/// ```
#[derive(Debug, Clone)]
pub struct CloudSaveClient {
    config: CloudSaveConfig,
    /// Locally cached slot metadata (slot name → SaveSlotMeta).
    slots: std::collections::HashMap<String, SaveSlotMeta>,
    /// The last loaded slot's full data (for quick access by game code).
    last_loaded: Option<SaveSlot>,
}

impl CloudSaveClient {
    /// Create a new `CloudSaveClient`.
    pub fn new(config: CloudSaveConfig) -> Self {
        Self {
            config,
            slots: std::collections::HashMap::new(),
            last_loaded: None,
        }
    }

    /// The authenticated user ID.
    pub fn user_id(&self) -> &str {
        &self.config.user_id
    }

    /// The game this client is associated with.
    pub fn game_id(&self) -> &str {
        &self.config.game_id
    }

    /// All cached slot metadata.
    pub fn slots(&self) -> Vec<&SaveSlotMeta> {
        self.slots.values().collect()
    }

    /// Look up metadata for a specific slot.
    pub fn slot_meta(&self, slot: &str) -> Option<&SaveSlotMeta> {
        self.slots.get(slot)
    }

    /// The last slot that was loaded (full data blob included).
    pub fn last_loaded(&self) -> Option<&SaveSlot> {
        self.last_loaded.as_ref()
    }

    /// Build a [`ClientCloudSaveMessage::Save`] with the given data.
    pub fn save_message(&self, req: SaveRequest) -> ClientCloudSaveMessage {
        ClientCloudSaveMessage::Save(req)
    }

    /// Build a [`ClientCloudSaveMessage::Load`] request for a slot.
    pub fn load_message(&self, slot: &str) -> ClientCloudSaveMessage {
        ClientCloudSaveMessage::Load {
            slot: slot.to_string(),
        }
    }

    /// Build a [`ClientCloudSaveMessage::Delete`] for a slot.
    pub fn delete_message(&self, slot: &str) -> ClientCloudSaveMessage {
        ClientCloudSaveMessage::Delete {
            slot: slot.to_string(),
        }
    }

    /// Build a [`ClientCloudSaveMessage::ListSlots`] request.
    pub fn list_slots_message(&self) -> ClientCloudSaveMessage {
        ClientCloudSaveMessage::ListSlots
    }

    /// Process a [`ServerCloudSaveMessage`] and update local caches.
    pub fn handle_server_message(&mut self, msg: ServerCloudSaveMessage) {
        match msg {
            ServerCloudSaveMessage::Saved { meta } => {
                self.slots.insert(meta.slot.clone(), meta);
            }
            ServerCloudSaveMessage::Loaded(slot) => {
                self.slots.insert(slot.meta.slot.clone(), slot.meta.clone());
                self.last_loaded = Some(slot);
            }
            ServerCloudSaveMessage::Deleted { slot } => {
                self.slots.remove(&slot);
                if let Some(last) = &self.last_loaded {
                    if last.meta.slot == slot {
                        self.last_loaded = None;
                    }
                }
            }
            ServerCloudSaveMessage::Slots { slots } => {
                for meta in slots {
                    self.slots.insert(meta.slot.clone(), meta);
                }
            }
            // Version conflict and errors do not mutate local state.
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// base64 serde helper (no extra deps — hand-rolled)
// ---------------------------------------------------------------------------

mod base64_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Encode `Vec<u8>` to a standard base64 string.
    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        // Manual base64 encoding — avoids adding the `base64` crate.
        let encoded = encode_base64(bytes);
        encoded.serialize(s)
    }

    /// Decode a base64 string to `Vec<u8>`.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        decode_base64(&s).map_err(serde::de::Error::custom)
    }

    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn encode_base64(input: &[u8]) -> String {
        let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
        for chunk in input.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

            let combined = (b0 << 16) | (b1 << 8) | b2;

            out.push(ALPHABET[((combined >> 18) & 0x3F) as usize] as char);
            out.push(ALPHABET[((combined >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[((combined >> 6) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[(combined & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    fn decode_base64(input: &str) -> Result<Vec<u8>, &'static str> {
        let input = input.trim_end_matches('=');
        let mut out = Vec::with_capacity(input.len() * 3 / 4);
        let mut buf = 0u32;
        let mut bits = 0u32;

        for ch in input.chars() {
            let val = match ch {
                'A'..='Z' => (ch as u32) - 65,
                'a'..='z' => (ch as u32) - 71,
                '0'..='9' => (ch as u32) + 4,
                '+' => 62,
                '/' => 63,
                _ => return Err("invalid base64 character"),
            };
            buf = (buf << 6) | val;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((buf >> bits) & 0xFF) as u8);
            }
        }

        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> CloudSaveClient {
        CloudSaveClient::new(CloudSaveConfig {
            user_id: "u-test".to_string(),
            game_id: "fps-starter".to_string(),
            auth_token: "tok".to_string(),
        })
    }

    fn test_meta(slot: &str, version: u64) -> SaveSlotMeta {
        SaveSlotMeta {
            slot: slot.to_string(),
            user_id: "u-test".to_string(),
            game_id: "fps-starter".to_string(),
            version,
            size_bytes: 16,
            created_at_ms: 0,
            modified_at_ms: 0,
            description: None,
        }
    }

    fn test_slot(slot: &str) -> SaveSlot {
        SaveSlot {
            meta: test_meta(slot, 1),
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        }
    }

    // -- base64 helper coverage via public serde interface --

    /// Verify that arbitrary binary blobs survive a JSON serde roundtrip
    /// (exercises the private base64 encode/decode helpers indirectly).
    #[test]
    fn base64_roundtrip_via_save_slot_serde() {
        // "Hello, Magnetite!" as bytes
        let original = b"Hello, Magnetite!".to_vec();
        let slot = SaveSlot {
            meta: test_meta("b64-test", 1),
            data: original.clone(),
        };
        let json = serde_json::to_string(&slot).unwrap();
        let back: SaveSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.data, original);
    }

    #[test]
    fn base64_empty_data_roundtrip() {
        let slot = SaveSlot {
            meta: test_meta("empty", 1),
            data: vec![],
        };
        let json = serde_json::to_string(&slot).unwrap();
        let back: SaveSlot = serde_json::from_str(&json).unwrap();
        assert!(back.data.is_empty());
    }

    #[test]
    fn base64_all_byte_values_roundtrip() {
        let all_bytes: Vec<u8> = (0u8..=255).collect();
        let slot = SaveSlot {
            meta: test_meta("all-bytes", 1),
            data: all_bytes.clone(),
        };
        let json = serde_json::to_string(&slot).unwrap();
        let back: SaveSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.data, all_bytes);
    }

    // -- Serde roundtrips --

    #[test]
    fn save_slot_meta_serde() {
        let meta = test_meta("autosave", 3);
        let json = serde_json::to_string(&meta).unwrap();
        let back: SaveSlotMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, back);
    }

    #[test]
    fn save_slot_serde() {
        let slot = test_slot("slot_1");
        let json = serde_json::to_string(&slot).unwrap();
        let back: SaveSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(slot, back);
    }

    #[test]
    fn save_request_serde() {
        let req = SaveRequest {
            slot: "autosave".to_string(),
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
            version: Some(2),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: SaveRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn client_cloud_save_message_all_variants_serde() {
        let msgs: Vec<ClientCloudSaveMessage> = vec![
            ClientCloudSaveMessage::Save(SaveRequest {
                slot: "s1".to_string(),
                data: vec![1, 2, 3],
                version: None,
            }),
            ClientCloudSaveMessage::Load {
                slot: "s1".to_string(),
            },
            ClientCloudSaveMessage::Delete {
                slot: "s1".to_string(),
            },
            ClientCloudSaveMessage::ListSlots,
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ClientCloudSaveMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn server_cloud_save_message_all_variants_serde() {
        let msgs: Vec<ServerCloudSaveMessage> = vec![
            ServerCloudSaveMessage::Saved {
                meta: test_meta("autosave", 1),
            },
            ServerCloudSaveMessage::Loaded(test_slot("slot_1")),
            ServerCloudSaveMessage::Deleted {
                slot: "slot_1".to_string(),
            },
            ServerCloudSaveMessage::Slots {
                slots: vec![test_meta("slot_1", 1)],
            },
            ServerCloudSaveMessage::VersionConflict {
                slot: "slot_1".to_string(),
                server_version: 5,
                client_version: 3,
            },
            ServerCloudSaveMessage::Error {
                code: CloudSaveErrorCode::TooLarge,
                message: "blob exceeds 1MiB".to_string(),
            },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ServerCloudSaveMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn error_code_all_variants_serde() {
        let codes = [
            CloudSaveErrorCode::Unauthorized,
            CloudSaveErrorCode::NotFound,
            CloudSaveErrorCode::TooLarge,
            CloudSaveErrorCode::VersionConflict,
            CloudSaveErrorCode::Internal,
            CloudSaveErrorCode::BadRequest,
        ];
        for code in &codes {
            let json = serde_json::to_string(code).unwrap();
            let back: CloudSaveErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, &back);
        }
    }

    // -- CloudSaveClient state machine --

    #[test]
    fn client_initial_empty() {
        let client = test_client();
        assert!(client.slots().is_empty());
        assert!(client.last_loaded().is_none());
    }

    #[test]
    fn client_save_response_adds_slot() {
        let mut client = test_client();
        client.handle_server_message(ServerCloudSaveMessage::Saved {
            meta: test_meta("autosave", 1),
        });
        assert_eq!(client.slots().len(), 1);
        assert!(client.slot_meta("autosave").is_some());
    }

    #[test]
    fn client_load_response_caches_data() {
        let mut client = test_client();
        client.handle_server_message(ServerCloudSaveMessage::Loaded(test_slot("slot_1")));
        assert!(client.last_loaded().is_some());
        assert_eq!(
            client.last_loaded().unwrap().data,
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    #[test]
    fn client_delete_removes_slot_and_last_loaded() {
        let mut client = test_client();
        client.handle_server_message(ServerCloudSaveMessage::Loaded(test_slot("slot_1")));
        assert!(client.last_loaded().is_some());
        client.handle_server_message(ServerCloudSaveMessage::Deleted {
            slot: "slot_1".to_string(),
        });
        assert!(client.slot_meta("slot_1").is_none());
        assert!(client.last_loaded().is_none());
    }

    #[test]
    fn client_slots_list_populates_cache() {
        let mut client = test_client();
        client.handle_server_message(ServerCloudSaveMessage::Slots {
            slots: vec![test_meta("slot_1", 1), test_meta("autosave", 3)],
        });
        assert_eq!(client.slots().len(), 2);
        assert_eq!(client.slot_meta("autosave").unwrap().version, 3);
    }

    #[test]
    fn client_save_updates_existing_slot_version() {
        let mut client = test_client();
        client.handle_server_message(ServerCloudSaveMessage::Saved {
            meta: test_meta("slot_1", 1),
        });
        client.handle_server_message(ServerCloudSaveMessage::Saved {
            meta: test_meta("slot_1", 2),
        });
        assert_eq!(client.slot_meta("slot_1").unwrap().version, 2);
    }

    #[test]
    fn client_user_and_game_id() {
        let client = test_client();
        assert_eq!(client.user_id(), "u-test");
        assert_eq!(client.game_id(), "fps-starter");
    }

    #[test]
    fn client_build_messages() {
        let client = test_client();
        assert!(matches!(
            client.save_message(SaveRequest {
                slot: "s".to_string(),
                data: vec![],
                version: None,
            }),
            ClientCloudSaveMessage::Save(_)
        ));
        assert!(matches!(
            client.load_message("autosave"),
            ClientCloudSaveMessage::Load { slot } if slot == "autosave"
        ));
        assert!(matches!(
            client.delete_message("slot_1"),
            ClientCloudSaveMessage::Delete { slot } if slot == "slot_1"
        ));
        assert!(matches!(
            client.list_slots_message(),
            ClientCloudSaveMessage::ListSlots
        ));
    }

    #[test]
    fn client_version_conflict_does_not_mutate_state() {
        let mut client = test_client();
        client.handle_server_message(ServerCloudSaveMessage::Saved {
            meta: test_meta("slot_1", 3),
        });
        // Conflict should be a no-op for local state.
        client.handle_server_message(ServerCloudSaveMessage::VersionConflict {
            slot: "slot_1".to_string(),
            server_version: 5,
            client_version: 3,
        });
        // The cached version is still 3 (not mutated by the conflict).
        assert_eq!(client.slot_meta("slot_1").unwrap().version, 3);
    }
}
