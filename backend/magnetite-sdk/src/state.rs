//! Game state types — strongly typed, versioned, and snapshot-capable.
//!
//! These types form the serializable backbone of every Magnetite game:
//! - [`GameState`] is the canonical server state at a given tick.
//! - [`Snapshot`] wraps `GameState` with tick metadata for save/replay and
//!   client-side rollback / reconciliation.
//! - [`PlayerState`] carries per-player transform, health, and a free-form
//!   `custom` payload so each game can extend it without losing the common fields.
//!
//! # Serialization
//!
//! All types derive [`serde::Serialize`] / [`serde::Deserialize`], so they can
//! be sent over the wire (see [`crate::protocol`]), persisted to disk, or stored
//! in a replay buffer.
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::state::{GameState, PlayerId, PlayerState, Position, Rotation, Snapshot};
//!
//! let mut state = GameState::default();
//! state.players.push(PlayerState {
//!     id: PlayerId::new(1),
//!     position: Position { x: 0.0, y: 0.0, z: 0.0 },
//!     rotation: Rotation { pitch: 0.0, yaw: 0.0, roll: 0.0 },
//!     health: 100.0,
//!     max_health: 100.0,
//!     alive: true,
//!     score: 0,
//!     custom: serde_json::Value::Null,
//! });
//!
//! let snap = Snapshot::new(42, state.clone());
//! assert_eq!(snap.tick, 42);
//! assert!(snap.verify());
//! ```

use serde::{Deserialize, Serialize};

/// An opaque, copy-able player identifier backed by a `u64`.
///
/// Use [`PlayerId::new`] to construct from a raw integer; the inner value is
/// intentionally not `pub` to keep the abstraction stable.
///
/// ```rust
/// use magnetite_sdk::state::PlayerId;
/// let pid = PlayerId::new(7);
/// assert_eq!(pid.as_u64(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(u64);

impl PlayerId {
    /// Construct a [`PlayerId`] from a raw `u64`.
    #[inline]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the raw numeric identifier.
    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Player({})", self.0)
    }
}

/// 3-D position in world-space (metres).
///
/// Coordinate convention: right-handed Y-up (same as Bevy's default).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position {
    /// Euclidean distance to another position.
    #[inline]
    pub fn distance_to(self, other: Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Euler angles in degrees.
///
/// - `pitch`: nose-up / nose-down.
/// - `yaw`: left / right turn.
/// - `roll`: bank.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rotation {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

/// Per-player state that every Magnetite game carries.
///
/// Extend game-specific data by serialising into the `custom` field — it is a
/// raw [`serde_json::Value`] so the common fields are always accessible to the
/// platform without knowing the game's internals.
///
/// ```rust
/// use magnetite_sdk::state::{PlayerId, PlayerState, Position, Rotation};
/// use serde_json::json;
///
/// let ps = PlayerState {
///     id: PlayerId::new(1),
///     position: Position::default(),
///     rotation: Rotation::default(),
///     health: 80.0,
///     max_health: 100.0,
///     alive: true,
///     score: 250,
///     custom: json!({ "ammo": 30, "class": "soldier" }),
/// };
/// assert!(ps.alive);
/// assert_eq!(ps.score, 250);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    /// Stable identifier, unique within a session.
    pub id: PlayerId,
    /// World-space position.
    pub position: Position,
    /// View direction.
    pub rotation: Rotation,
    /// Current health points (≥ 0.0).
    pub health: f32,
    /// Maximum health points.
    pub max_health: f32,
    /// `false` when the player is dead / spectating.
    pub alive: bool,
    /// Session score (game-defined meaning).
    pub score: i64,
    /// Game-specific payload — serialise whatever you need here.
    pub custom: serde_json::Value,
}

impl PlayerState {
    /// Returns the player's health as a fraction in `[0.0, 1.0]`.
    #[inline]
    pub fn health_fraction(&self) -> f32 {
        if self.max_health <= 0.0 {
            0.0
        } else {
            (self.health / self.max_health).clamp(0.0, 1.0)
        }
    }
}

/// The authoritative server-side game state at a single instant.
///
/// The platform only reads `players` and `tick`; all other simulation data
/// belongs in `world`.
///
/// ```rust
/// use magnetite_sdk::state::{GameState, PlayerId, PlayerState, Position, Rotation};
///
/// let mut state = GameState::default();
/// assert!(state.players.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    /// The server tick at which this state was authoritative.
    pub tick: u64,
    /// All connected (or recently disconnected) players.
    pub players: Vec<PlayerState>,
    /// Arbitrary world/simulation data the game wants to persist and broadcast.
    ///
    /// Leave as `Null` for simple games; use a custom struct serialised via
    /// `serde_json::to_value` for complex ones.
    pub world: serde_json::Value,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            tick: 0,
            players: Vec::new(),
            world: serde_json::Value::Null,
        }
    }
}

impl GameState {
    /// Find a player by id.
    #[inline]
    pub fn player(&self, id: PlayerId) -> Option<&PlayerState> {
        self.players.iter().find(|p| p.id == id)
    }

    /// Find a player mutably by id.
    #[inline]
    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut PlayerState> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    /// Remove a player by id; returns the removed [`PlayerState`] if found.
    pub fn remove_player(&mut self, id: PlayerId) -> Option<PlayerState> {
        if let Some(pos) = self.players.iter().position(|p| p.id == id) {
            Some(self.players.swap_remove(pos))
        } else {
            None
        }
    }
}

/// A versioned snapshot of [`GameState`] used for save/restore, replay, and
/// client-side rollback-and-reconciliation (GGPO-style netcode).
///
/// The `checksum` field provides a fast integrity check: it is computed from
/// the JSON representation of the enclosed state. It is *not* a
/// cryptographic hash — use it only to detect accidental state divergence.
///
/// # Example
///
/// ```rust
/// use magnetite_sdk::state::{GameState, Snapshot};
///
/// let state = GameState::default();
/// let snap = Snapshot::new(0, state.clone());
/// assert!(snap.verify(), "freshly-created snapshot must be valid");
///
/// // Serialise, send over the wire, deserialise — still valid.
/// let json = serde_json::to_string(&snap).unwrap();
/// let snap2: Snapshot = serde_json::from_str(&json).unwrap();
/// assert!(snap2.verify());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// The tick this snapshot was captured at.
    pub tick: u64,
    /// The full game state.
    pub state: GameState,
    /// Fast integrity checksum (non-cryptographic).
    pub checksum: u32,
}

impl Snapshot {
    /// Capture `state` at `tick`, computing the checksum automatically.
    pub fn new(tick: u64, state: GameState) -> Self {
        let checksum = Self::compute_checksum(&state);
        Self {
            tick,
            state,
            checksum,
        }
    }

    /// Returns `true` when the stored checksum matches the current state.
    pub fn verify(&self) -> bool {
        self.checksum == Self::compute_checksum(&self.state)
    }

    fn compute_checksum(state: &GameState) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        serde_json::to_string(state)
            .unwrap_or_default()
            .hash(&mut hasher);
        hasher.finish() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_player() -> PlayerState {
        PlayerState {
            id: PlayerId::new(1),
            position: Position {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            rotation: Rotation {
                pitch: 0.0,
                yaw: 90.0,
                roll: 0.0,
            },
            health: 75.0,
            max_health: 100.0,
            alive: true,
            score: 500,
            custom: serde_json::Value::Null,
        }
    }

    #[test]
    fn player_id_roundtrip() {
        let pid = PlayerId::new(42);
        assert_eq!(pid.as_u64(), 42);
        let json = serde_json::to_string(&pid).unwrap();
        let pid2: PlayerId = serde_json::from_str(&json).unwrap();
        assert_eq!(pid, pid2);
    }

    #[test]
    fn position_distance() {
        let a = Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = Position {
            x: 3.0,
            y: 4.0,
            z: 0.0,
        };
        let d = a.distance_to(b);
        assert!((d - 5.0).abs() < 1e-4, "expected 5.0, got {d}");
    }

    #[test]
    fn health_fraction_clamped() {
        let mut ps = sample_player();
        assert!((ps.health_fraction() - 0.75).abs() < 1e-6);
        ps.health = 150.0;
        assert_eq!(ps.health_fraction(), 1.0);
        ps.health = -10.0;
        assert_eq!(ps.health_fraction(), 0.0);
    }

    #[test]
    fn game_state_player_lookup() {
        let mut gs = GameState::default();
        gs.players.push(sample_player());

        assert!(gs.player(PlayerId::new(1)).is_some());
        assert!(gs.player(PlayerId::new(99)).is_none());

        gs.player_mut(PlayerId::new(1)).unwrap().score += 100;
        assert_eq!(gs.player(PlayerId::new(1)).unwrap().score, 600);
    }

    #[test]
    fn game_state_remove_player() {
        let mut gs = GameState::default();
        gs.players.push(sample_player());
        let removed = gs.remove_player(PlayerId::new(1));
        assert!(removed.is_some());
        assert!(gs.players.is_empty());
    }

    #[test]
    fn snapshot_verify_pass() {
        let state = GameState {
            tick: 7,
            players: vec![sample_player()],
            world: serde_json::Value::Null,
        };
        let snap = Snapshot::new(7, state);
        assert!(snap.verify());
    }

    #[test]
    fn snapshot_verify_tampered() {
        let state = GameState::default();
        let mut snap = Snapshot::new(0, state);
        // Tamper with the checksum.
        snap.checksum = snap.checksum.wrapping_add(1);
        assert!(!snap.verify());
    }

    #[test]
    fn snapshot_serde_roundtrip() {
        let state = GameState {
            tick: 5,
            players: vec![sample_player()],
            world: serde_json::Value::Null,
        };
        let snap = Snapshot::new(5, state);
        let json = serde_json::to_string(&snap).unwrap();
        let snap2: Snapshot = serde_json::from_str(&json).unwrap();
        assert!(snap2.verify());
        assert_eq!(snap.tick, snap2.tick);
    }
}
