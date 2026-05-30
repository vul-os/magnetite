//! Platform points / XP / score economy.
//!
//! The Magnetite platform maintains a ledger of **points** for every player.
//! Points are the universal currency of the platform economy — used for:
//! - **Rewards:** games award points on match completion, achievements, and
//!   seasonal milestones.
//! - **Spending:** players spend points in the in-game marketplace or on
//!   cosmetic unlocks.
//! - **Ranks & seasons:** the leaderboard aggregates points into seasonal
//!   rankings that reset periodically.
//!
//! # Wire protocol
//!
//! Clients send [`ClientPointsMessage`] over the existing platform WebSocket
//! (the same `ws/` layer used by comms).  The backend responds with
//! [`ServerPointsMessage`].  Requests can also be made via HTTP (REST) — the
//! SDK models both patterns; the caller chooses the transport.
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::platform::points::{
//!     AwardPointsRequest, PointsClient, PointsConfig, SpendPointsRequest,
//! };
//!
//! let client = PointsClient::new(PointsConfig {
//!     user_id: "u-42".to_string(),
//!     auth_token: "jwt-here".to_string(),
//! });
//!
//! // Build an award request.
//! let req = client.award(AwardPointsRequest {
//!     amount: 500,
//!     reason: "match_win".to_string(),
//!     game_id: Some("fps-starter".to_string()),
//!     idempotency_key: Some("match-99-win".to_string()),
//! });
//! assert_eq!(req.amount, 500);
//!
//! // Build a spend request.
//! let spend = client.spend(SpendPointsRequest {
//!     amount: 200,
//!     reason: "cosmetic_unlock".to_string(),
//!     item_id: Some("skin-flare".to_string()),
//! });
//! assert_eq!(spend.amount, 200);
//! ```

use serde::{Deserialize, Serialize};

use super::UserId;

/// Opaque identifier for a points ledger entry.
pub type LedgerId = String;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the in-game points client.
///
/// ```rust
/// use magnetite_sdk::platform::points::PointsConfig;
///
/// let cfg = PointsConfig {
///     user_id: "u-1".to_string(),
///     auth_token: "tok".to_string(),
/// };
/// assert_eq!(cfg.user_id, "u-1");
/// ```
#[derive(Debug, Clone)]
pub struct PointsConfig {
    /// The authenticated user's platform ID.
    pub user_id: UserId,
    /// JWT / session token for authenticating requests.
    pub auth_token: String,
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Request to award points to the authenticated player.
///
/// Points are awarded by the **server-side game logic** — client code must
/// never award its own points unilaterally; instead the game server sends
/// an award on behalf of the player.
///
/// ```rust
/// use magnetite_sdk::platform::points::AwardPointsRequest;
///
/// let req = AwardPointsRequest {
///     amount: 100,
///     reason: "kill_streak_5".to_string(),
///     game_id: Some("fps-starter".to_string()),
///     idempotency_key: None,
/// };
/// assert_eq!(req.amount, 100);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwardPointsRequest {
    /// Points to award (must be > 0).
    pub amount: u64,
    /// Machine-readable reason code (e.g. `"match_win"`, `"daily_login"`).
    pub reason: String,
    /// The game that triggered the award (for per-game analytics).
    pub game_id: Option<String>,
    /// Optional idempotency key — the same key is never credited twice.
    pub idempotency_key: Option<String>,
}

/// Request to spend points from the authenticated player's balance.
///
/// ```rust
/// use magnetite_sdk::platform::points::SpendPointsRequest;
///
/// let req = SpendPointsRequest {
///     amount: 250,
///     reason: "item_purchase".to_string(),
///     item_id: Some("emote-moonwalk".to_string()),
/// };
/// assert_eq!(req.amount, 250);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendPointsRequest {
    /// Points to deduct (must be > 0).
    pub amount: u64,
    /// Machine-readable reason code (e.g. `"item_purchase"`, `"season_pass"`).
    pub reason: String,
    /// Optional reference to the item or feature being unlocked.
    pub item_id: Option<String>,
}

/// Current points balance returned by the platform.
///
/// ```rust
/// use magnetite_sdk::platform::points::PointsBalance;
///
/// let balance = PointsBalance {
///     user_id: "u-42".to_string(),
///     balance: 1500,
///     lifetime_earned: 3000,
///     lifetime_spent: 1500,
/// };
/// assert_eq!(balance.balance, 1500);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointsBalance {
    /// The user whose balance this is.
    pub user_id: UserId,
    /// Current spendable balance.
    pub balance: u64,
    /// Total points ever awarded (all time).
    pub lifetime_earned: u64,
    /// Total points ever spent (all time).
    pub lifetime_spent: u64,
}

/// A single entry in the points ledger (an award or spend event).
///
/// ```rust
/// use magnetite_sdk::platform::points::{LedgerEntry, LedgerEntryKind};
///
/// let entry = LedgerEntry {
///     id: "le-001".to_string(),
///     user_id: "u-42".to_string(),
///     kind: LedgerEntryKind::Award,
///     amount: 500,
///     reason: "match_win".to_string(),
///     game_id: None,
///     timestamp_ms: 1_700_000_000_000,
///     balance_after: 1500,
/// };
/// assert_eq!(entry.amount, 500);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Unique entry identifier.
    pub id: LedgerId,
    /// The user this entry belongs to.
    pub user_id: UserId,
    /// Whether this was an award or a spend.
    pub kind: LedgerEntryKind,
    /// Points awarded or spent.
    pub amount: u64,
    /// Machine-readable reason code.
    pub reason: String,
    /// The game that triggered this entry (if any).
    pub game_id: Option<String>,
    /// Wall-clock timestamp (Unix milliseconds).
    pub timestamp_ms: u64,
    /// Running balance **after** this entry was applied.
    pub balance_after: u64,
}

/// Whether a ledger entry is an award or a spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEntryKind {
    /// Points were credited to the account.
    Award,
    /// Points were debited from the account.
    Spend,
}

// ---------------------------------------------------------------------------
// Client → Platform messages
// ---------------------------------------------------------------------------

/// Messages sent **from** in-game Rust code **to** the Magnetite platform
/// for points operations.
///
/// ```rust
/// use magnetite_sdk::platform::points::{AwardPointsRequest, ClientPointsMessage};
///
/// let msg = ClientPointsMessage::Award(AwardPointsRequest {
///     amount: 100,
///     reason: "daily_bonus".to_string(),
///     game_id: None,
///     idempotency_key: None,
/// });
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ClientPointsMessage = serde_json::from_str(&json).unwrap();
/// assert_eq!(msg, back);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientPointsMessage {
    /// Award points to the player (server-authoritative; game server only).
    Award(AwardPointsRequest),
    /// Spend points (player-initiated; server validates balance).
    Spend(SpendPointsRequest),
    /// Request the player's current balance.
    GetBalance,
    /// Request the player's recent ledger entries.
    GetLedger {
        /// Maximum number of entries to return (default 20).
        limit: Option<u32>,
    },
}

// ---------------------------------------------------------------------------
// Platform → Client messages
// ---------------------------------------------------------------------------

/// Messages sent **from** the Magnetite platform **to** in-game Rust code
/// in response to points operations.
///
/// ```rust
/// use magnetite_sdk::platform::points::{PointsBalance, ServerPointsMessage};
///
/// let msg = ServerPointsMessage::Balance(PointsBalance {
///     user_id: "u-1".to_string(),
///     balance: 200,
///     lifetime_earned: 500,
///     lifetime_spent: 300,
/// });
/// let json = serde_json::to_string(&msg).unwrap();
/// let back: ServerPointsMessage = serde_json::from_str(&json).unwrap();
/// assert_eq!(msg, back);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerPointsMessage {
    /// Points were successfully awarded.
    Awarded {
        /// The ledger entry created for this award.
        entry: LedgerEntry,
    },
    /// Points were successfully spent.
    Spent {
        /// The ledger entry created for this spend.
        entry: LedgerEntry,
    },
    /// Current balance (response to [`ClientPointsMessage::GetBalance`]).
    Balance(PointsBalance),
    /// Recent ledger entries (response to [`ClientPointsMessage::GetLedger`]).
    Ledger {
        /// Entries, newest first.
        entries: Vec<LedgerEntry>,
    },
    /// Insufficient balance for a spend request.
    InsufficientBalance {
        /// The current balance (less than the requested amount).
        current: u64,
        /// The amount that was requested.
        requested: u64,
    },
    /// The request was rejected.
    Error {
        /// Machine-readable error code.
        code: PointsErrorCode,
        /// Human-readable description.
        message: String,
    },
}

/// Error codes for points operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PointsErrorCode {
    /// The request requires authentication.
    Unauthorized,
    /// The operation is not permitted (e.g. client trying to self-award).
    Forbidden,
    /// The idempotency key was already used.
    DuplicateIdempotencyKey,
    /// The amount was zero or negative.
    InvalidAmount,
    /// The platform encountered an internal error.
    Internal,
    /// The request was malformed.
    BadRequest,
}

// ---------------------------------------------------------------------------
// Points client
// ---------------------------------------------------------------------------

/// Typed, stateful in-game points client.
///
/// **No I/O is performed** — the caller sends the returned
/// [`ClientPointsMessage`] via the platform WebSocket and passes received
/// bytes back into [`PointsClient::handle_server_message`].
///
/// ```rust
/// use magnetite_sdk::platform::points::{
///     AwardPointsRequest, PointsClient, PointsConfig, ServerPointsMessage,
///     PointsBalance,
/// };
///
/// let client = PointsClient::new(PointsConfig {
///     user_id: "u-1".to_string(),
///     auth_token: "tok".to_string(),
/// });
///
/// // Build an award request.
/// let msg = client.award(AwardPointsRequest {
///     amount: 100,
///     reason: "win".to_string(),
///     game_id: None,
///     idempotency_key: None,
/// });
/// assert_eq!(msg.amount, 100);
///
/// // Handle the server's balance response.
/// let server_msg = ServerPointsMessage::Balance(PointsBalance {
///     user_id: "u-1".to_string(),
///     balance: 1100,
///     lifetime_earned: 1100,
///     lifetime_spent: 0,
/// });
/// let mut c2 = client.clone();
/// c2.handle_server_message(server_msg);
/// assert_eq!(c2.cached_balance(), Some(1100));
/// ```
#[derive(Debug, Clone)]
pub struct PointsClient {
    config: PointsConfig,
    /// Locally cached balance (updated from server messages).
    cached_balance: Option<u64>,
}

impl PointsClient {
    /// Create a new `PointsClient`.
    pub fn new(config: PointsConfig) -> Self {
        Self {
            config,
            cached_balance: None,
        }
    }

    /// The authenticated user ID.
    pub fn user_id(&self) -> &str {
        &self.config.user_id
    }

    /// The locally cached balance, or `None` if no balance response has been
    /// received yet.
    pub fn cached_balance(&self) -> Option<u64> {
        self.cached_balance
    }

    /// Build an [`AwardPointsRequest`] (to be sent by the server-side game).
    pub fn award(&self, req: AwardPointsRequest) -> AwardPointsRequest {
        req
    }

    /// Build a [`ClientPointsMessage::Award`].
    pub fn award_message(&self, req: AwardPointsRequest) -> ClientPointsMessage {
        ClientPointsMessage::Award(req)
    }

    /// Build a [`ClientPointsMessage::Spend`].
    pub fn spend(&self, req: SpendPointsRequest) -> SpendPointsRequest {
        req
    }

    /// Build a [`ClientPointsMessage::Spend`] message.
    pub fn spend_message(&self, req: SpendPointsRequest) -> ClientPointsMessage {
        ClientPointsMessage::Spend(req)
    }

    /// Build a [`ClientPointsMessage::GetBalance`] request.
    pub fn get_balance_message(&self) -> ClientPointsMessage {
        ClientPointsMessage::GetBalance
    }

    /// Build a [`ClientPointsMessage::GetLedger`] request.
    pub fn get_ledger_message(&self, limit: Option<u32>) -> ClientPointsMessage {
        ClientPointsMessage::GetLedger { limit }
    }

    /// Process a [`ServerPointsMessage`] and update local state.
    ///
    /// Updates [`PointsClient::cached_balance`] when a balance or awarded/spent
    /// message is received.
    pub fn handle_server_message(&mut self, msg: ServerPointsMessage) {
        match &msg {
            ServerPointsMessage::Balance(b) => {
                self.cached_balance = Some(b.balance);
            }
            ServerPointsMessage::Awarded { entry } => {
                self.cached_balance = Some(entry.balance_after);
            }
            ServerPointsMessage::Spent { entry } => {
                self.cached_balance = Some(entry.balance_after);
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

    fn test_client() -> PointsClient {
        PointsClient::new(PointsConfig {
            user_id: "u-test".to_string(),
            auth_token: "tok-abc".to_string(),
        })
    }

    fn test_entry(kind: LedgerEntryKind, amount: u64, balance_after: u64) -> LedgerEntry {
        LedgerEntry {
            id: "le-1".to_string(),
            user_id: "u-test".to_string(),
            kind,
            amount,
            reason: "test".to_string(),
            game_id: None,
            timestamp_ms: 0,
            balance_after,
        }
    }

    // -- Serde roundtrips --

    #[test]
    fn award_request_serde() {
        let req = AwardPointsRequest {
            amount: 100,
            reason: "match_win".to_string(),
            game_id: Some("fps".to_string()),
            idempotency_key: Some("k-1".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: AwardPointsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn spend_request_serde() {
        let req = SpendPointsRequest {
            amount: 250,
            reason: "cosmetic".to_string(),
            item_id: Some("skin-1".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: SpendPointsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn balance_serde() {
        let balance = PointsBalance {
            user_id: "u-1".to_string(),
            balance: 1000,
            lifetime_earned: 2000,
            lifetime_spent: 1000,
        };
        let json = serde_json::to_string(&balance).unwrap();
        let back: PointsBalance = serde_json::from_str(&json).unwrap();
        assert_eq!(balance, back);
    }

    #[test]
    fn ledger_entry_serde() {
        let entry = test_entry(LedgerEntryKind::Award, 500, 1500);
        let json = serde_json::to_string(&entry).unwrap();
        let back: LedgerEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn client_points_message_all_variants_serde() {
        let msgs: Vec<ClientPointsMessage> = vec![
            ClientPointsMessage::Award(AwardPointsRequest {
                amount: 10,
                reason: "r".to_string(),
                game_id: None,
                idempotency_key: None,
            }),
            ClientPointsMessage::Spend(SpendPointsRequest {
                amount: 5,
                reason: "s".to_string(),
                item_id: None,
            }),
            ClientPointsMessage::GetBalance,
            ClientPointsMessage::GetLedger { limit: Some(10) },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ClientPointsMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn server_points_message_all_variants_serde() {
        let msgs: Vec<ServerPointsMessage> = vec![
            ServerPointsMessage::Awarded {
                entry: test_entry(LedgerEntryKind::Award, 100, 600),
            },
            ServerPointsMessage::Spent {
                entry: test_entry(LedgerEntryKind::Spend, 50, 550),
            },
            ServerPointsMessage::Balance(PointsBalance {
                user_id: "u-1".to_string(),
                balance: 550,
                lifetime_earned: 600,
                lifetime_spent: 50,
            }),
            ServerPointsMessage::Ledger { entries: vec![] },
            ServerPointsMessage::InsufficientBalance {
                current: 10,
                requested: 100,
            },
            ServerPointsMessage::Error {
                code: PointsErrorCode::Unauthorized,
                message: "not authed".to_string(),
            },
        ];
        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let back: ServerPointsMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    #[test]
    fn error_code_all_variants_serde() {
        let codes = [
            PointsErrorCode::Unauthorized,
            PointsErrorCode::Forbidden,
            PointsErrorCode::DuplicateIdempotencyKey,
            PointsErrorCode::InvalidAmount,
            PointsErrorCode::Internal,
            PointsErrorCode::BadRequest,
        ];
        for code in &codes {
            let json = serde_json::to_string(code).unwrap();
            let back: PointsErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, &back);
        }
    }

    // -- PointsClient state machine --

    #[test]
    fn client_initial_balance_is_none() {
        let client = test_client();
        assert_eq!(client.cached_balance(), None);
    }

    #[test]
    fn client_balance_updated_on_balance_message() {
        let mut client = test_client();
        client.handle_server_message(ServerPointsMessage::Balance(PointsBalance {
            user_id: "u-test".to_string(),
            balance: 750,
            lifetime_earned: 750,
            lifetime_spent: 0,
        }));
        assert_eq!(client.cached_balance(), Some(750));
    }

    #[test]
    fn client_balance_updated_on_awarded() {
        let mut client = test_client();
        client.handle_server_message(ServerPointsMessage::Awarded {
            entry: test_entry(LedgerEntryKind::Award, 200, 200),
        });
        assert_eq!(client.cached_balance(), Some(200));
    }

    #[test]
    fn client_balance_updated_on_spent() {
        let mut client = test_client();
        // First award some points.
        client.handle_server_message(ServerPointsMessage::Awarded {
            entry: test_entry(LedgerEntryKind::Award, 500, 500),
        });
        // Then spend.
        client.handle_server_message(ServerPointsMessage::Spent {
            entry: test_entry(LedgerEntryKind::Spend, 100, 400),
        });
        assert_eq!(client.cached_balance(), Some(400));
    }

    #[test]
    fn client_user_id() {
        let client = test_client();
        assert_eq!(client.user_id(), "u-test");
    }

    #[test]
    fn client_build_messages() {
        let client = test_client();
        let award_msg = client.award_message(AwardPointsRequest {
            amount: 50,
            reason: "r".to_string(),
            game_id: None,
            idempotency_key: None,
        });
        assert!(matches!(award_msg, ClientPointsMessage::Award(_)));

        let spend_msg = client.spend_message(SpendPointsRequest {
            amount: 10,
            reason: "s".to_string(),
            item_id: None,
        });
        assert!(matches!(spend_msg, ClientPointsMessage::Spend(_)));

        assert!(matches!(
            client.get_balance_message(),
            ClientPointsMessage::GetBalance
        ));
        assert!(matches!(
            client.get_ledger_message(Some(5)),
            ClientPointsMessage::GetLedger { limit: Some(5) }
        ));
    }
}
