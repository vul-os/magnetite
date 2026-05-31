//! Shared data types for the arena shooter.

use magnetite_sdk::state::PlayerId;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Arena constants
// ---------------------------------------------------------------------------

/// Arena half-width (world units). Players are clamped to [-ARENA_WIDTH/2, ARENA_WIDTH/2].
pub const ARENA_WIDTH: f32 = 200.0;
/// Arena half-height (world units). Players are clamped to [-ARENA_HEIGHT/2, ARENA_HEIGHT/2].
pub const ARENA_HEIGHT: f32 = 200.0;
/// Maximum player movement per tick (world units).
pub const MAX_SPEED: f32 = 4.0;
/// Starting and maximum health points.
pub const MAX_HP: f32 = 100.0;
/// Damage dealt by a projectile hit.
pub const HIT_DAMAGE: f32 = 25.0;
/// Projectile travel speed (world units per tick).
pub const PROJECTILE_SPEED: f32 = 12.0;
/// Projectile lifetime (ticks before expiry).
pub const PROJECTILE_LIFETIME_TICKS: u32 = 40;
/// Minimum ticks between consecutive shots.
pub const SHOOT_COOLDOWN_TICKS: u64 = 12;
/// Projectile collision radius (world units).
pub const PROJECTILE_RADIUS: f32 = 1.5;
/// Player collision radius (world units).
pub const PLAYER_RADIUS: f32 = 3.0;

// ---------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------

/// Per-player game state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShooterPlayer {
    pub id: PlayerId,
    /// World-space X position.
    pub x: f32,
    /// World-space Y position (top-down — no Z).
    pub y: f32,
    /// Facing angle in radians (0 = right, π/2 = up).
    pub angle: f32,
    /// Current health points.
    pub hp: f32,
    /// Whether the player is still alive.
    pub alive: bool,
    /// Last tick the player fired.
    pub last_shot_tick: u64,
    /// Session score (kills).
    pub score: i64,
}

impl ShooterPlayer {
    /// Construct a fresh player at a given spawn position.
    pub fn spawn(id: PlayerId, x: f32, y: f32) -> Self {
        Self {
            id,
            x,
            y,
            angle: 0.0,
            hp: MAX_HP,
            alive: true,
            last_shot_tick: 0,
            score: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Projectile
// ---------------------------------------------------------------------------

/// An in-flight projectile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Projectile {
    /// Unique projectile ID (from DeterministicRng).
    pub id: u64,
    /// Owning player.
    pub owner: PlayerId,
    /// Current X position.
    pub x: f32,
    /// Current Y position.
    pub y: f32,
    /// Velocity X per tick.
    pub vx: f32,
    /// Velocity Y per tick.
    pub vy: f32,
    /// Ticks remaining before expiry.
    pub ticks_left: u32,
}

// ---------------------------------------------------------------------------
// Snapshot  (full authoritative state)
// ---------------------------------------------------------------------------

/// Full serialisable game state — used for snapshots, restore, and replay.
///
/// Field order is fixed (struct derives) — important for deterministic hashing
/// via `serde_json` (declaration order = serialisation order).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaSnapshot {
    /// All players (sorted by player id for determinism).
    pub players: Vec<ShooterPlayer>,
    /// All in-flight projectiles (sorted by id for determinism).
    pub projectiles: Vec<Projectile>,
    /// Current match tick (informational, not used for hashing).
    pub tick: u64,
}

// ---------------------------------------------------------------------------
// Delta  (compact per-tick diff)
// ---------------------------------------------------------------------------

/// Compact state diff broadcast every tick.
///
/// Encodes only what changed this tick to minimise bandwidth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArenaDelta {
    /// Players whose state changed (position, hp, alive, score).
    pub changed_players: Vec<ShooterPlayer>,
    /// Projectile IDs that expired or hit something this tick.
    pub removed_projectile_ids: Vec<u64>,
    /// New projectiles spawned this tick.
    pub new_projectiles: Vec<Projectile>,
}

// ---------------------------------------------------------------------------
// View  (per-player interest-filtered view)
// ---------------------------------------------------------------------------

/// Per-player view of the world.
///
/// This is the ONLY data transmitted to a given player. In a real game you
/// would apply fog-of-war or visibility rules; here we transmit everything
/// to keep the reference simple (arena is small, no fog of war).
#[derive(Debug, Clone, Serialize)]
pub struct ArenaView {
    /// The requesting player's own state.
    pub self_state: Option<ShooterPlayer>,
    /// All other players (useful for rendering enemies).
    pub other_players: Vec<ShooterPlayer>,
    /// All projectiles currently in flight.
    pub projectiles: Vec<Projectile>,
    /// Current tick (for client-side interpolation).
    pub tick: u64,
}

// ---------------------------------------------------------------------------
// Command  (validated authoritative command)
// ---------------------------------------------------------------------------

/// Server-authoritative game command — the output of `validate`.
///
/// The game runs on commands, never on raw client input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ArenaCommand {
    /// Move the player by a delta clamped to [`MAX_SPEED`].
    Move {
        /// Clamped X-axis delta (world units).
        dx: f32,
        /// Clamped Y-axis delta (world units).
        dy: f32,
    },
    /// Rotate the player to face `angle` (radians).
    Aim { angle: f32 },
    /// Fire a projectile in the direction the player is currently facing.
    Shoot,
}
