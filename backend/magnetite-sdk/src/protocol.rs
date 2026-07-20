//! Versioned wire protocol for Magnetite games.
//!
//! All messages exchanged between clients and the server are represented as
//! [`Envelope`]s. An envelope carries a [`PROTOCOL_VERSION`] header so that
//! the server can reject stale clients before trying to deserialise the body.
//!
//! # Message flow
//!
//! ```text
//! Client                              Server
//!   │                                    │
//!   │──── Envelope { Connect }  ────────>│
//!   │<─── Envelope { Welcome }  ─────────│
//!   │                                    │
//!   │──── Envelope { InputFrame } ──────>│  (every client tick)
//!   │<─── Envelope { StateUpdate } ──────│  (every server tick broadcast)
//!   │                                    │
//!   │──── Envelope { Disconnect } ──────>│
//! ```
//!
//! # Encoding
//!
//! Messages are serialised with [`serde_json`] by default (human-readable,
//! easy to debug with browser DevTools). A future `binary` feature flag will
//! add MessagePack for production AAA titles that need the extra throughput.
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::protocol::{ClientMessage, Envelope, ServerMessage, PROTOCOL_VERSION};
//! use magnetite_sdk::state::PlayerId;
//!
//! let env = Envelope::new(ClientMessage::Connect {
//!     player_id: PlayerId::new(1),
//!     token: "auth-token-xyz".to_string(),
//! });
//!
//! let bytes = env.encode().unwrap();
//! let decoded: Envelope<ClientMessage> = Envelope::decode(&bytes).unwrap();
//! assert_eq!(decoded.version, PROTOCOL_VERSION);
//! ```

use serde::{Deserialize, Serialize};

use crate::input::Input;
use crate::state::{GameState, PlayerId, Snapshot};

/// Monotonically increasing protocol version.
///
/// Bump this whenever the wire format changes in a backwards-incompatible way.
pub const PROTOCOL_VERSION: u16 = 1;

/// A framing envelope that wraps every message with version metadata.
///
/// Generic over `T` so the same struct is used for both [`ClientMessage`] and
/// [`ServerMessage`], enabling a single codec path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Wire protocol version — must match [`PROTOCOL_VERSION`].
    pub version: u16,
    /// Monotonic sequence number (sender-local, wraps at `u64::MAX`).
    pub seq: u64,
    /// Wall-clock time at the sender in milliseconds since Unix epoch.
    pub timestamp_ms: u64,
    /// The actual message payload.
    pub body: T,
}

impl<T: Serialize + for<'de> Deserialize<'de>> Envelope<T> {
    /// Wrap `body` in an envelope with the current [`PROTOCOL_VERSION`].
    ///
    /// `seq` and `timestamp_ms` are initialised to `0` — the caller should
    /// set them from their own monotonic counter / clock.
    pub fn new(body: T) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            seq: 0,
            timestamp_ms: 0,
            body,
        }
    }

    /// Encode this envelope to a JSON byte vector.
    ///
    /// Returns an error if `T` cannot be serialised (should never happen for
    /// the SDK's own message types).
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Decode an envelope from a JSON byte slice.
    ///
    /// Returns an error when the bytes are not valid JSON or the schema does
    /// not match `T`.
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Returns `true` when the envelope's version matches [`PROTOCOL_VERSION`].
    #[inline]
    pub fn is_current_version(&self) -> bool {
        self.version == PROTOCOL_VERSION
    }
}

// ---------------------------------------------------------------------------
// Client → Server messages
// ---------------------------------------------------------------------------

/// Messages sent by a game client to the authoritative server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Initial handshake — client announces itself and presents an auth token.
    ///
    /// The server responds with [`ServerMessage::Welcome`] on success or
    /// [`ServerMessage::Error`] on failure.
    Connect {
        player_id: PlayerId,
        /// Platform-issued JWT or session token.
        token: String,
    },

    /// Graceful disconnect.
    Disconnect,

    /// One tick's worth of input from this client.
    ///
    /// Clients should send one `InputFrame` per tick even when no keys are
    /// held, so the server can distinguish "idle" from "connection lost".
    InputFrame { input: Input },

    /// Request a full state snapshot (e.g. on reconnect).
    RequestSnapshot,

    /// Client acknowledges receipt of server state at `tick`.
    ///
    /// The server uses this for interest-management: it will not re-send data
    /// the client has already confirmed.
    Ack { tick: u64 },

    /// Ping — the server responds with [`ServerMessage::Pong`].
    Ping {
        /// Echo'd verbatim in the Pong so the client can compute RTT.
        client_time_ms: u64,
    },
}

// ---------------------------------------------------------------------------
// Server → Client messages
// ---------------------------------------------------------------------------

/// Messages sent by the authoritative server to game clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Sent in response to a successful [`ClientMessage::Connect`].
    Welcome {
        /// The player's assigned id (may differ from what the client requested).
        player_id: PlayerId,
        /// The full game state at the moment of join.
        state: GameState,
        /// Server tick rate in Hz.
        tick_rate: u32,
    },

    /// A compact per-tick state broadcast.
    ///
    /// For large games the server only sends the *delta* relevant to each
    /// client (see the interest-management traits in [`crate::networking`]).
    StateUpdate {
        /// The server tick this state corresponds to.
        tick: u64,
        /// The (partial or full) game state.
        state: GameState,
    },

    /// A full [`Snapshot`] in response to [`ClientMessage::RequestSnapshot`]
    /// or after a periodic save-point.
    FullSnapshot(Snapshot),

    /// Another player joined.
    PlayerJoined { player_id: PlayerId },

    /// A player disconnected or was kicked.
    PlayerLeft { player_id: PlayerId },

    /// Input echo: the server confirms it processed the client's input at
    /// `sequence` and the resulting authoritative tick is `tick`.
    ///
    /// The client uses this to discard prediction frames up to `sequence`.
    InputAck { sequence: u64, tick: u64 },

    /// Response to [`ClientMessage::Ping`].
    Pong {
        /// Echo of the client's timestamp — client computes RTT as
        /// `now - client_time_ms`.
        client_time_ms: u64,
        /// Server wall-clock time at the moment of the pong.
        server_time_ms: u64,
    },

    /// An error condition. The client should surface this to the user.
    Error { code: ErrorCode, message: String },
}

/// Structured error codes for [`ServerMessage::Error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// The client sent an unsupported protocol version.
    ProtocolMismatch,
    /// The auth token was invalid or expired.
    Unauthorized,
    /// The session is full.
    SessionFull,
    /// A message was received out of sequence.
    OutOfSequence,
    /// The server encountered an internal error.
    Internal,
    /// The client sent malformed data.
    BadRequest,
}

// ---------------------------------------------------------------------------
// Authoritative netcode frames  (MOAT — additive, does not break existing API)
// ---------------------------------------------------------------------------

use crate::authority::{MatchConfig, RejectReason, Tick};

/// Messages from the game client to the authoritative server.
///
/// These frames are the **low-latency realtime path** layered on top of the
/// existing handshake messages in [`ClientMessage`]. The runtime sends and
/// receives these at the tick rate; the existing `Connect` / `Disconnect`
/// messages in [`ClientMessage`] remain the control-plane path.
///
/// # Wire encoding
///
/// Tagged with `"type"` (snake_case) — same convention as [`ClientMessage`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientNet {
    /// One tick's worth of authoritative input.
    ///
    /// `seq` is a monotonically increasing client-local counter used by the
    /// server to match `Ack` / `Reject` responses and by the client to drive
    /// [`crate::networking::PredictionBuffer`] reconciliation.
    InputFrame {
        /// Client-local sequence number (monotonically increasing).
        seq: u32,
        /// The authoritative tick this input targets.
        tick: Tick,
        /// The raw input frame.
        input: Input,
    },

    /// Optional first frame: ask the node to prove which node key it holds.
    ///
    /// The node answers with [`ServerNet::NodeIdentity`], signing the client's
    /// `nonce`. A client that pinned a node key out of band (from a signed
    /// discovery ad, or from a [`ServerNet::Redirect`]'s `target_key`) uses this
    /// to refuse a node that cannot produce the matching signature — an address
    /// is a hint, the key is the identity.
    Hello {
        /// Client-chosen random nonce, echoed back inside the node's signature.
        nonce: String,
    },

    /// Present a signed session redirect at the **target** node after a shard
    /// migrated, to be admitted to that shard with session continuity.
    ///
    /// `redirect` is the JSON body of the runtime's `cluster::SignedRedirect`
    /// exactly as it arrived in [`ServerNet::Redirect`]. It is opaque here: the
    /// SDK deliberately does not know how to validate it, because validation is
    /// the target node's job (`cluster::FollowAdmission::admit`) and must not be
    /// reimplemented anywhere else.
    Follow {
        /// Verbatim `SignedRedirect` JSON, including its embedded follow token.
        redirect: serde_json::Value,
    },

    /// A **client-attested** sensor event (seam §3.7, [`magnetite_seams::input`]).
    ///
    /// This is the wire ingress for [`InputClass::Attested`] — a camera gesture,
    /// an IMU reading, anything a client *asserts* rather than a command the
    /// host can re-derive.
    ///
    /// # This frame is not, and can never be, replay-verifiable
    ///
    /// It exists on a strictly separate path from [`ClientNet::InputFrame`] and
    /// the two must never merge. `InputFrame` carries deterministic commands
    /// that land in a `ReplayLog`, so `verify_replay` can prove tampering from
    /// the record alone. An attested event has no such property: the pixels
    /// that produced it are gone and were never authoritative. Admitting one of
    /// these down the deterministic path would leave `verify_replay` still
    /// passing while no longer meaning anything — which is why the server route
    /// feeds `AttestedEventInput` and nothing else.
    ///
    /// # What the signature buys
    ///
    /// `signed.sig` proves **authorship** — "this key sent this" — and stops a
    /// player forging events in someone else's name or a relay editing them in
    /// flight. It does not make the sensor reading true. A cheater synthesising
    /// events signs them with their own genuine key and verifies every time;
    /// `magnetite_seams::input`'s own test
    /// `a_plausible_synthetic_event_is_indistinguishable_from_a_real_one` pins
    /// that ceiling. This frame is **not anti-cheat and not security**.
    ///
    /// Unsigned attested events have no wire representation on purpose: `signed`
    /// is required, so a frame without it fails to deserialize and is refused.
    ///
    /// # Wire shape
    ///
    /// ```json
    /// {"type":"attested_event","signed":{
    ///   "event":{"player":"<64 hex>","kind":"swing","confidence":0.725,
    ///            "vector":[0.125,-0.0625,0.0],"speed_mps":6.5,
    ///            "t_capture_ms":1763000000123,"seq":42},
    ///   "player_key":"<64 hex>","sig":"<128 hex>"}}
    /// ```
    ///
    /// `PubKey`/`Sig` use hand-written hex serializers, so those are strings and
    /// not byte arrays. `vector` and `speed_mps` are `Option` with no
    /// `serde(default)`: the keys must be present, `null` when absent.
    ///
    /// [`InputClass::Attested`]: magnetite_seams::input::InputClass::Attested
    #[cfg(feature = "scaling")]
    AttestedEvent {
        /// The signed sensor claim. Deserializes directly as
        /// [`magnetite_seams::input::SignedAttestedEvent`].
        signed: Box<magnetite_seams::input::SignedAttestedEvent>,
    },
}

/// Messages from the authoritative server to game clients.
///
/// Sent every tick (or on the snapshot cadence for full snapshots).
/// The client uses these to drive [`crate::networking::PredictionBuffer`]
/// reconciliation and interest-filtered rendering.
///
/// # Wire encoding
///
/// Tagged with `"type"` (snake_case).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerNet {
    /// Sent once when a client successfully joins a match.
    ///
    /// Contains the player's authoritative id and the full match configuration
    /// so the client can initialise its local prediction state.
    Welcome {
        /// The authoritative player id assigned to this client.
        player_id: PlayerId,
        /// Full match configuration (topology, tick rate, seed, …).
        config: MatchConfig,
    },

    /// A full serialised snapshot of the authoritative game state.
    ///
    /// Broadcast every [`MatchConfig::snapshot_every`] ticks. Clients use this
    /// as a rollback point for prediction reconciliation.
    Snapshot {
        /// The tick this snapshot was captured at.
        tick: Tick,
        /// Canonical serialisation of the game snapshot (opaque bytes —
        /// the format is determined by [`crate::authority::AuthoritativeGame::Snapshot`]).
        full: Vec<u8>,
    },

    /// A compact interest-filtered delta for this player.
    ///
    /// Sent every tick. The client applies the delta on top of the last
    /// acknowledged snapshot. The format is game-defined (JSON by default).
    Delta {
        /// The current authoritative tick.
        tick: Tick,
        /// The tick the delta is relative to.
        since_tick: Tick,
        /// Serialised delta bytes (game-specific format).
        diff: Vec<u8>,
    },

    /// Acknowledgement: the server processed the client's input at `seq` and
    /// it resulted in authoritative tick `tick`.
    ///
    /// The client discards all prediction frames with sequence ≤ `seq` from
    /// [`crate::networking::PredictionBuffer`].
    Ack {
        /// The client-local sequence number that was processed.
        seq: u32,
        /// The authoritative tick the input contributed to.
        tick: Tick,
    },

    /// The server rejected the client's input at `seq`.
    ///
    /// The client should discard the corresponding prediction frame and
    /// reconcile on the next snapshot.
    Reject {
        /// The client-local sequence number that was rejected.
        seq: u32,
        /// Why the input was rejected.
        reason: RejectReason,
    },

    /// The node's answer to [`ClientNet::Hello`]: its node key plus a signature
    /// over the client's nonce, proving it holds the matching secret key.
    ///
    /// The client MUST compare `node_key` against the key it pinned and MUST
    /// verify `sig`. Both hex-encoded; the signed bytes are
    /// `b"magnetite-node-hello-v1" || nonce_bytes || node_key_bytes`.
    NodeIdentity {
        /// Hex-encoded Ed25519 node public key.
        node_key: String,
        /// The nonce from the client's `Hello`, echoed.
        nonce: String,
        /// Hex-encoded Ed25519 signature over the bytes described above.
        sig: String,
    },

    /// "Your shard moved; reconnect here." Delivered on the still-authenticated
    /// session immediately after a migration commits, and is the last frame the
    /// node sends on that connection.
    ///
    /// `redirect` is the JSON body of the runtime's `cluster::SignedRedirect`.
    /// The client MUST verify its issuer signature against the node key it is
    /// already talking to, MUST refuse an expired one, and MUST pin
    /// `target_key` on the new connection — a blindly-followed redirect is a
    /// hijack onto an attacker's node.
    Redirect {
        /// Verbatim `SignedRedirect` JSON, including its embedded follow token.
        redirect: serde_json::Value,
    },

    /// A [`ClientNet::AttestedEvent`] passed signature verification and
    /// plausibility screening, and was queued for the sim.
    ///
    /// **Acceptance is not a verdict of honesty.** It means only "signed by the
    /// key it names, and not physically impossible" — see
    /// [`magnetite_seams::input::PlausibilityGate`]. Do not surface this to a
    /// player as validation of anything.
    AttestedAck {
        /// The `event.seq` of the accepted event (per-player, `u64`).
        seq: u64,
    },

    /// A [`ClientNet::AttestedEvent`] was dropped. The client is told rather
    /// than left to infer it from silence.
    ///
    /// # Why this is not [`ServerNet::Reject`]
    ///
    /// `Reject` carries the client-local `u32` input sequence and instructs the
    /// client to discard every `PredictionBuffer` frame at or below it. Attested
    /// events number in a *different, unrelated* `u64` counter space, so
    /// answering one on `Reject` would silently evict correct prediction state.
    /// The counters are separate because the input classes are separate; the
    /// response channel follows.
    AttestedReject {
        /// The `event.seq` this refers to, or `0` when the frame was too
        /// malformed to recover one.
        seq: u64,
        /// Why it was dropped: a bad signature, a failed plausibility check, a
        /// malformed frame, or connection-level rate limiting.
        ///
        /// Every reason means "refused", **never** "cheating proven". A host may
        /// drop and rate-limit on these; it holds no proof and must not claim
        /// one.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PlayerId;

    #[test]
    fn envelope_roundtrip_client_message() {
        let msg = ClientMessage::Connect {
            player_id: PlayerId::new(1),
            token: "tok".to_string(),
        };
        let env = Envelope::new(msg);
        let bytes = env.encode().unwrap();
        let decoded: Envelope<ClientMessage> = Envelope::decode(&bytes).unwrap();
        assert!(decoded.is_current_version());
        assert_eq!(decoded.version, PROTOCOL_VERSION);
    }

    #[test]
    fn envelope_roundtrip_server_message() {
        let msg = ServerMessage::Pong {
            client_time_ms: 100,
            server_time_ms: 105,
        };
        let env = Envelope::new(msg);
        let bytes = env.encode().unwrap();
        let decoded: Envelope<ServerMessage> = Envelope::decode(&bytes).unwrap();
        assert!(decoded.is_current_version());
    }

    #[test]
    fn envelope_version_check() {
        let env: Envelope<ClientMessage> = Envelope {
            version: 0,
            seq: 0,
            timestamp_ms: 0,
            body: ClientMessage::Disconnect,
        };
        assert!(!env.is_current_version());
    }

    #[test]
    fn error_code_serde() {
        let code = ErrorCode::Unauthorized;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"UNAUTHORIZED\"");
        let code2: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(code, code2);
    }

    #[test]
    fn server_message_tagged_serde() {
        let msg = ServerMessage::PlayerJoined {
            player_id: PlayerId::new(7),
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Check that the tag is present.
        assert!(json.contains("\"type\""));
        assert!(json.contains("player_joined"));
    }
}
