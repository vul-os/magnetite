//! Platform marketplace — in-game store items, purchases, and entitlements.
//!
//! The Magnetite marketplace enables developers to run **in-game stores** with
//! cosmetics, items, DLC, and season passes.  All purchases flow through the
//! platform's payment rails (Paystack fiat on-ramp / platform points) with a
//! 30% platform fee and revenue share paid out to the developer via Wise.
//!
//! # Concepts
//!
//! | Term | Description |
//! |---|---|
//! | [`StoreItem`] | A purchasable item in a developer's store (cosmetic, DLC, pass, …) |
//! | [`Entitlement`] | A player's permanent right to use an item after purchase |
//! | [`PurchaseRequest`] | Initiates a checkout for one item |
//! | [`PurchaseResult`] | Outcome of a completed purchase (success + entitlement, or failure) |
//!
//! # Wire protocol
//!
//! All messages travel over the existing platform WebSocket (`ws/`).
//! [`ClientMarketplaceMessage`] is sent by the game; [`ServerMarketplaceMessage`]
//! is received.
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::platform::marketplace::{
//!     MarketplaceClient, MarketplaceConfig, ClientMarketplaceMessage,
//! };
//!
//! let client = MarketplaceClient::new(MarketplaceConfig {
//!     user_id: "u-42".to_string(),
//!     game_id: "fps-starter".to_string(),
//!     auth_token: "jwt-here".to_string(),
//! });
//!
//! // Request the list of items in the store.
//! let msg = client.list_items_message(None);
//! assert!(matches!(msg, ClientMarketplaceMessage::ListItems { .. }));
//!
//! // Check a cached entitlement.
//! assert!(!client.has_entitlement("skin-neon"));
//! ```

use serde::{Deserialize, Serialize};

use super::UserId;

/// Opaque identifier for a store item.
pub type ItemId = String;

/// Opaque identifier for an entitlement.
pub type EntitlementId = String;

/// Opaque identifier for a purchase transaction.
pub type PurchaseId = String;

// ---------------------------------------------------------------------------
// Store item
// ---------------------------------------------------------------------------

/// The type / category of a store item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    /// A visual cosmetic (skin, emote, spray, …).
    Cosmetic,
    /// Downloadable content (new map, mission pack, …).
    Dlc,
    /// A recurring subscription pass (battle pass, season pass).
    Pass,
    /// An in-game consumable (boost, respawn token, …).
    Consumable,
    /// A bundle of multiple items.
    Bundle,
    /// Other / game-specific.
    Other,
}

/// A single item listed in a developer's in-game store.
///
/// ```rust
/// use magnetite_sdk::platform::marketplace::{ItemType, StoreItem};
///
/// let item = StoreItem {
///     id: "skin-neon".to_string(),
///     game_id: "fps-starter".to_string(),
///     name: "Neon Skin".to_string(),
///     description: "Electric neon finish for your primary weapon.".to_string(),
///     item_type: ItemType::Cosmetic,
///     price_usd_cents: 299,
///     price_points: Some(1500),
///     active: true,
///     image_url: None,
///     tags: vec!["weapon".to_string(), "cosmetic".to_string()],
/// };
/// assert_eq!(item.price_usd_cents, 299);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreItem {
    /// Unique item identifier (platform-assigned).
    pub id: ItemId,
    /// The game this item belongs to.
    pub game_id: String,
    /// Display name.
    pub name: String,
    /// Short description shown in the store UI.
    pub description: String,
    /// Category.
    pub item_type: ItemType,
    /// Price in US cents (e.g. 299 = $2.99). Zero means free.
    pub price_usd_cents: u32,
    /// Alternative price in platform points (if supported by the developer).
    pub price_points: Option<u64>,
    /// Whether the item is currently available for purchase.
    pub active: bool,
    /// Optional CDN URL to the item's cover image.
    pub image_url: Option<String>,
    /// Searchable tags (e.g. `["weapon", "cosmetic", "legendary"]`).
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Entitlement
// ---------------------------------------------------------------------------

/// A player's permanent right to access an item after purchase.
///
/// ```rust
/// use magnetite_sdk::platform::marketplace::Entitlement;
///
/// let e = Entitlement {
///     id: "ent-001".to_string(),
///     user_id: "u-42".to_string(),
///     item_id: "skin-neon".to_string(),
///     game_id: "fps-starter".to_string(),
///     purchase_id: "pur-abc".to_string(),
///     granted_at_ms: 1_700_000_000_000,
///     revoked: false,
/// };
/// assert!(!e.revoked);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entitlement {
    /// Unique entitlement identifier.
    pub id: EntitlementId,
    /// The user who owns this entitlement.
    pub user_id: UserId,
    /// The item that was purchased.
    pub item_id: ItemId,
    /// The game the item belongs to.
    pub game_id: String,
    /// The purchase transaction that created this entitlement.
    pub purchase_id: PurchaseId,
    /// Unix milliseconds when the entitlement was granted.
    pub granted_at_ms: u64,
    /// `true` if the entitlement was revoked (refund, fraud, …).
    pub revoked: bool,
}

// ---------------------------------------------------------------------------
// Purchase request / result
// ---------------------------------------------------------------------------

/// Initiate a purchase for a single store item.
///
/// ```rust
/// use magnetite_sdk::platform::marketplace::{PaymentMethod, PurchaseRequest};
///
/// let req = PurchaseRequest {
///     item_id: "skin-neon".to_string(),
///     payment_method: PaymentMethod::Usd,
///     idempotency_key: Some("client-nonce-001".to_string()),
/// };
/// assert_eq!(req.item_id, "skin-neon");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseRequest {
    /// The item to purchase.
    pub item_id: ItemId,
    /// Payment method to use.
    pub payment_method: PaymentMethod,
    /// Optional client-generated idempotency key to prevent double-charges.
    pub idempotency_key: Option<String>,
}

/// Payment method for a purchase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    /// Pay with fiat USD from the player's platform wallet (funded via Paystack on-ramp).
    Usd,
    /// Pay via Paystack directly (fiat on-ramp for supported regions).
    Paystack,
    /// Pay with platform points (deducted from the player's balance).
    Points,
}

/// Outcome of a completed purchase.
///
/// ```rust
/// use magnetite_sdk::platform::marketplace::{Entitlement, PurchaseResult};
///
/// let result = PurchaseResult::Success {
///     purchase_id: "pur-001".to_string(),
///     entitlement: Entitlement {
///         id: "ent-001".to_string(),
///         user_id: "u-42".to_string(),
///         item_id: "skin-neon".to_string(),
///         game_id: "fps-starter".to_string(),
///         purchase_id: "pur-001".to_string(),
///         granted_at_ms: 0,
///         revoked: false,
///     },
/// };
/// assert!(matches!(result, PurchaseResult::Success { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum PurchaseResult {
    /// The purchase completed successfully.
    Success {
        /// The platform purchase ID (keep for support requests).
        purchase_id: PurchaseId,
        /// The entitlement granted to the player.
        entitlement: Entitlement,
    },
    /// The purchase failed.
    Failure {
        /// Why it failed.
        code: PurchaseErrorCode,
        /// Human-readable description.
        message: String,
    },
}

/// Error codes for failed purchases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PurchaseErrorCode {
    /// The player is not authenticated.
    Unauthorized,
    /// The item does not exist or is not available.
    ItemUnavailable,
    /// The player already owns this item.
    AlreadyOwned,
    /// Payment was declined.
    PaymentDeclined,
    /// Insufficient platform-points balance.
    InsufficientPoints,
    /// The idempotency key was already used (purchase is a duplicate).
    DuplicateIdempotencyKey,
    /// Platform internal error.
    Internal,
}

// ---------------------------------------------------------------------------
// Client → Platform messages
// ---------------------------------------------------------------------------

/// Messages sent **from** in-game Rust code **to** the Magnetite marketplace.
///
/// ```rust
/// use magnetite_sdk::platform::marketplace::{
///     ClientMarketplaceMessage, PaymentMethod, PurchaseRequest,
/// };
///
/// let msg = ClientMarketplaceMessage::Purchase(PurchaseRequest {
///     item_id: "skin-neon".to_string(),
///     payment_method: PaymentMethod::Usd,
///     idempotency_key: None,
/// });
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ClientMarketplaceMessage = serde_json::from_str(&json).unwrap();
/// assert_eq!(msg, back);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMarketplaceMessage {
    /// List available store items for a game.
    ListItems {
        /// Filter by game ID (defaults to the client's configured game_id).
        game_id: Option<String>,
        /// Optional item type filter.
        item_type: Option<ItemType>,
        /// Cursor for pagination (item ID of the last item in the previous page).
        cursor: Option<ItemId>,
        /// Maximum items to return.
        limit: Option<u32>,
    },
    /// Initiate a purchase.
    Purchase(PurchaseRequest),
    /// List the player's entitlements for a game.
    ListEntitlements {
        /// The game to list entitlements for.
        game_id: Option<String>,
    },
    /// Check whether the player has a specific entitlement.
    CheckEntitlement {
        /// The item ID to check.
        item_id: ItemId,
    },
}

// ---------------------------------------------------------------------------
// Platform → Client messages
// ---------------------------------------------------------------------------

/// Messages sent **from** the Magnetite marketplace **to** in-game Rust code.
///
/// ```rust
/// use magnetite_sdk::platform::marketplace::ServerMarketplaceMessage;
///
/// let msg = ServerMarketplaceMessage::Items { items: vec![], has_more: false };
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ServerMarketplaceMessage = serde_json::from_str(&json).unwrap();
/// assert_eq!(msg, back);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMarketplaceMessage {
    /// A page of store items.
    Items {
        /// The items for this page.
        items: Vec<StoreItem>,
        /// Whether there are more items (pagination).
        has_more: bool,
    },
    /// Result of a purchase attempt.
    PurchaseResult(PurchaseResult),
    /// A list of the player's entitlements.
    Entitlements {
        /// The player's entitlements for the requested game.
        entitlements: Vec<Entitlement>,
    },
    /// Whether the player has the requested entitlement.
    EntitlementCheck {
        /// The item that was checked.
        item_id: ItemId,
        /// `true` if the player owns it.
        owned: bool,
    },
    /// A marketplace error.
    Error {
        /// Machine-readable error code.
        code: MarketplaceErrorCode,
        /// Human-readable description.
        message: String,
    },
}

/// Error codes for marketplace operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketplaceErrorCode {
    /// The request requires authentication.
    Unauthorized,
    /// The game or item was not found.
    NotFound,
    /// The operation is not permitted.
    Forbidden,
    /// The platform encountered an internal error.
    Internal,
    /// The request was malformed.
    BadRequest,
}

// ---------------------------------------------------------------------------
// Marketplace client
// ---------------------------------------------------------------------------

/// Configuration for the in-game marketplace client.
#[derive(Debug, Clone)]
pub struct MarketplaceConfig {
    /// The authenticated user's platform ID.
    pub user_id: UserId,
    /// The game the client is operating on behalf of.
    pub game_id: String,
    /// JWT / session token.
    pub auth_token: String,
}

/// Typed, stateful in-game marketplace client.
///
/// Caches entitlements locally so game code can call [`MarketplaceClient::has_entitlement`]
/// without a round-trip.
///
/// **No I/O is performed** — the caller sends the returned
/// [`ClientMarketplaceMessage`] via the platform WebSocket and passes received
/// bytes back into [`MarketplaceClient::handle_server_message`].
///
/// ```rust
/// use magnetite_sdk::platform::marketplace::{
///     Entitlement, MarketplaceClient, MarketplaceConfig, ServerMarketplaceMessage,
/// };
///
/// let mut client = MarketplaceClient::new(MarketplaceConfig {
///     user_id: "u-42".to_string(),
///     game_id: "fps-starter".to_string(),
///     auth_token: "tok".to_string(),
/// });
///
/// assert!(!client.has_entitlement("skin-neon"));
///
/// // Simulate receiving an entitlement list.
/// client.handle_server_message(ServerMarketplaceMessage::Entitlements {
///     entitlements: vec![Entitlement {
///         id: "ent-1".to_string(),
///         user_id: "u-42".to_string(),
///         item_id: "skin-neon".to_string(),
///         game_id: "fps-starter".to_string(),
///         purchase_id: "pur-1".to_string(),
///         granted_at_ms: 0,
///         revoked: false,
///     }],
/// });
///
/// assert!(client.has_entitlement("skin-neon"));
/// ```
#[derive(Debug, Clone)]
pub struct MarketplaceClient {
    config: MarketplaceConfig,
    /// Locally cached entitlements (item_id → Entitlement).
    entitlements: std::collections::HashMap<ItemId, Entitlement>,
    /// Locally cached store items from the last `ListItems` response.
    cached_items: Vec<StoreItem>,
}

impl MarketplaceClient {
    /// Create a new `MarketplaceClient`.
    pub fn new(config: MarketplaceConfig) -> Self {
        Self {
            config,
            entitlements: std::collections::HashMap::new(),
            cached_items: Vec::new(),
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

    /// Returns `true` if the player has a non-revoked entitlement for `item_id`.
    pub fn has_entitlement(&self, item_id: &str) -> bool {
        self.entitlements
            .get(item_id)
            .map(|e| !e.revoked)
            .unwrap_or(false)
    }

    /// Returns a reference to all cached entitlements.
    pub fn entitlements(&self) -> impl Iterator<Item = &Entitlement> {
        self.entitlements.values()
    }

    /// Returns a reference to the last received store item list.
    pub fn cached_items(&self) -> &[StoreItem] {
        &self.cached_items
    }

    /// Build a [`ClientMarketplaceMessage::ListItems`] request for this game.
    pub fn list_items_message(&self, item_type: Option<ItemType>) -> ClientMarketplaceMessage {
        ClientMarketplaceMessage::ListItems {
            game_id: Some(self.config.game_id.clone()),
            item_type,
            cursor: None,
            limit: Some(50),
        }
    }

    /// Build a [`ClientMarketplaceMessage::Purchase`] for an item.
    pub fn purchase_message(&self, req: PurchaseRequest) -> ClientMarketplaceMessage {
        ClientMarketplaceMessage::Purchase(req)
    }

    /// Build a [`ClientMarketplaceMessage::ListEntitlements`] for this game.
    pub fn list_entitlements_message(&self) -> ClientMarketplaceMessage {
        ClientMarketplaceMessage::ListEntitlements {
            game_id: Some(self.config.game_id.clone()),
        }
    }

    /// Build a [`ClientMarketplaceMessage::CheckEntitlement`] for an item.
    pub fn check_entitlement_message(&self, item_id: &str) -> ClientMarketplaceMessage {
        ClientMarketplaceMessage::CheckEntitlement {
            item_id: item_id.to_string(),
        }
    }

    /// Process a [`ServerMarketplaceMessage`] and update local caches.
    pub fn handle_server_message(&mut self, msg: ServerMarketplaceMessage) {
        match msg {
            ServerMarketplaceMessage::Items { items, .. } => {
                self.cached_items = items;
            }
            ServerMarketplaceMessage::Entitlements { entitlements } => {
                for ent in entitlements {
                    self.entitlements.insert(ent.item_id.clone(), ent);
                }
            }
            ServerMarketplaceMessage::EntitlementCheck { item_id, owned } => {
                if !owned {
                    self.entitlements.remove(&item_id);
                }
            }
            ServerMarketplaceMessage::PurchaseResult(PurchaseResult::Success {
                entitlement,
                ..
            }) => {
                self.entitlements
                    .insert(entitlement.item_id.clone(), entitlement);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> MarketplaceClient {
        MarketplaceClient::new(MarketplaceConfig {
            user_id: "u-test".to_string(),
            game_id: "fps-starter".to_string(),
            auth_token: "tok".to_string(),
        })
    }

    fn test_entitlement(item_id: &str) -> Entitlement {
        Entitlement {
            id: format!("ent-{}", item_id),
            user_id: "u-test".to_string(),
            item_id: item_id.to_string(),
            game_id: "fps-starter".to_string(),
            purchase_id: "pur-1".to_string(),
            granted_at_ms: 0,
            revoked: false,
        }
    }

    fn test_item(id: &str) -> StoreItem {
        StoreItem {
            id: id.to_string(),
            game_id: "fps-starter".to_string(),
            name: "Test Item".to_string(),
            description: "A test item.".to_string(),
            item_type: ItemType::Cosmetic,
            price_usd_cents: 99,
            price_points: None,
            active: true,
            image_url: None,
            tags: vec![],
        }
    }

    // -- Serde roundtrips --

    #[test]
    fn store_item_serde() {
        let item = test_item("skin-neon");
        let json = serde_json::to_string(&item).unwrap();
        let back: StoreItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn entitlement_serde() {
        let ent = test_entitlement("skin-neon");
        let json = serde_json::to_string(&ent).unwrap();
        let back: Entitlement = serde_json::from_str(&json).unwrap();
        assert_eq!(ent, back);
    }

    #[test]
    fn purchase_request_serde() {
        let req = PurchaseRequest {
            item_id: "skin-neon".to_string(),
            payment_method: PaymentMethod::Usd,
            idempotency_key: Some("nonce-1".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: PurchaseRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn purchase_result_success_serde() {
        let result = PurchaseResult::Success {
            purchase_id: "pur-1".to_string(),
            entitlement: test_entitlement("skin-neon"),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: PurchaseResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, back);
    }

    #[test]
    fn purchase_result_failure_serde() {
        let result = PurchaseResult::Failure {
            code: PurchaseErrorCode::PaymentDeclined,
            message: "card declined".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: PurchaseResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, back);
    }

    #[test]
    fn client_marketplace_message_all_variants_serde() {
        let msgs: Vec<ClientMarketplaceMessage> = vec![
            ClientMarketplaceMessage::ListItems {
                game_id: None,
                item_type: Some(ItemType::Cosmetic),
                cursor: None,
                limit: Some(20),
            },
            ClientMarketplaceMessage::Purchase(PurchaseRequest {
                item_id: "skin-1".to_string(),
                payment_method: PaymentMethod::Points,
                idempotency_key: None,
            }),
            ClientMarketplaceMessage::ListEntitlements { game_id: None },
            ClientMarketplaceMessage::CheckEntitlement {
                item_id: "skin-1".to_string(),
            },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ClientMarketplaceMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn server_marketplace_message_all_variants_serde() {
        let msgs: Vec<ServerMarketplaceMessage> = vec![
            ServerMarketplaceMessage::Items {
                items: vec![test_item("i-1")],
                has_more: false,
            },
            ServerMarketplaceMessage::PurchaseResult(PurchaseResult::Success {
                purchase_id: "p-1".to_string(),
                entitlement: test_entitlement("i-1"),
            }),
            ServerMarketplaceMessage::Entitlements {
                entitlements: vec![test_entitlement("i-1")],
            },
            ServerMarketplaceMessage::EntitlementCheck {
                item_id: "i-1".to_string(),
                owned: true,
            },
            ServerMarketplaceMessage::Error {
                code: MarketplaceErrorCode::NotFound,
                message: "item not found".to_string(),
            },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ServerMarketplaceMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn item_type_all_variants_serde() {
        let types = [
            ItemType::Cosmetic,
            ItemType::Dlc,
            ItemType::Pass,
            ItemType::Consumable,
            ItemType::Bundle,
            ItemType::Other,
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let back: ItemType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, &back);
        }
    }

    #[test]
    fn payment_method_all_variants_serde() {
        let methods = [
            PaymentMethod::Usd,
            PaymentMethod::Paystack,
            PaymentMethod::Points,
        ];
        for m in &methods {
            let json = serde_json::to_string(m).unwrap();
            let back: PaymentMethod = serde_json::from_str(&json).unwrap();
            assert_eq!(m, &back);
        }
    }

    // -- MarketplaceClient state machine --

    #[test]
    fn client_initial_no_entitlements() {
        let client = test_client();
        assert!(!client.has_entitlement("skin-neon"));
        assert_eq!(client.cached_items().len(), 0);
    }

    #[test]
    fn client_receives_entitlements() {
        let mut client = test_client();
        client.handle_server_message(ServerMarketplaceMessage::Entitlements {
            entitlements: vec![
                test_entitlement("skin-neon"),
                test_entitlement("emote-dance"),
            ],
        });
        assert!(client.has_entitlement("skin-neon"));
        assert!(client.has_entitlement("emote-dance"));
        assert!(!client.has_entitlement("dlc-map1"));
    }

    #[test]
    fn client_purchase_success_adds_entitlement() {
        let mut client = test_client();
        client.handle_server_message(ServerMarketplaceMessage::PurchaseResult(
            PurchaseResult::Success {
                purchase_id: "pur-1".to_string(),
                entitlement: test_entitlement("skin-neon"),
            },
        ));
        assert!(client.has_entitlement("skin-neon"));
    }

    #[test]
    fn client_entitlement_check_false_removes_cached() {
        let mut client = test_client();
        // First add via entitlement list.
        client.handle_server_message(ServerMarketplaceMessage::Entitlements {
            entitlements: vec![test_entitlement("skin-neon")],
        });
        assert!(client.has_entitlement("skin-neon"));
        // Server says not owned (e.g. revoked).
        client.handle_server_message(ServerMarketplaceMessage::EntitlementCheck {
            item_id: "skin-neon".to_string(),
            owned: false,
        });
        assert!(!client.has_entitlement("skin-neon"));
    }

    #[test]
    fn client_items_cached() {
        let mut client = test_client();
        client.handle_server_message(ServerMarketplaceMessage::Items {
            items: vec![test_item("i-1"), test_item("i-2")],
            has_more: false,
        });
        assert_eq!(client.cached_items().len(), 2);
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
            client.list_items_message(None),
            ClientMarketplaceMessage::ListItems { .. }
        ));
        assert!(matches!(
            client.list_entitlements_message(),
            ClientMarketplaceMessage::ListEntitlements { .. }
        ));
        assert!(matches!(
            client.check_entitlement_message("skin-1"),
            ClientMarketplaceMessage::CheckEntitlement { item_id } if item_id == "skin-1"
        ));
    }

    #[test]
    fn revoked_entitlement_not_owned() {
        let mut client = test_client();
        let mut ent = test_entitlement("skin-neon");
        ent.revoked = true;
        client.handle_server_message(ServerMarketplaceMessage::Entitlements {
            entitlements: vec![ent],
        });
        // Revoked entitlements are cached but not considered owned.
        assert!(!client.has_entitlement("skin-neon"));
    }

    #[test]
    fn purchase_error_codes_serde() {
        let codes = [
            PurchaseErrorCode::Unauthorized,
            PurchaseErrorCode::ItemUnavailable,
            PurchaseErrorCode::AlreadyOwned,
            PurchaseErrorCode::PaymentDeclined,
            PurchaseErrorCode::InsufficientPoints,
            PurchaseErrorCode::DuplicateIdempotencyKey,
            PurchaseErrorCode::Internal,
        ];
        for code in &codes {
            let json = serde_json::to_string(code).unwrap();
            let back: PurchaseErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, &back);
        }
    }
}
