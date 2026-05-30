//! # Level Geometry
//!
//! Static level definition for the FPS starter: a simple rectangular arena
//! with several box obstacles.
//!
//! ## Server-authoritative use
//!
//! [`spawn_point_for`], [`clamp_position`], and [`floor_at`] are called from
//! the authoritative [`crate::FpsGame`] — they must be pure and have no Bevy
//! or rapier dependency.
//!
//! ## Bevy / rapier3d use (`native` / `wasm` features)
//!
//! The [`LevelDescriptor`] contains `BoxCollider` descriptors that the Bevy
//! startup system converts into `rapier3d` `Collider` components.  See
//! [`crate::bevy_client`] for the Bevy integration.

use magnetite_sdk::state::{GameState, PlayerId, PlayerState, Position};

// ---------------------------------------------------------------------------
// Arena dimensions
// ---------------------------------------------------------------------------

/// Half-width of the square arena (metres).
pub const ARENA_HALF: f32 = 40.0;

/// Arena floor Y (world-space).
pub const FLOOR_Y: f32 = 0.0;

/// Arena ceiling Y.
pub const CEILING_Y: f32 = 6.0;

// ---------------------------------------------------------------------------
// Collider descriptors (used by bevy_client to spawn rapier3d colliders)
// ---------------------------------------------------------------------------

/// An axis-aligned box collider descriptor.
///
/// `center` + `half_extents` match `rapier3d::geometry::Collider::cuboid`.
#[derive(Debug, Clone)]
pub struct BoxCollider {
    pub center: Position,
    /// Half-extents (x, y, z) for `Collider::cuboid(hx, hy, hz)`.
    pub half_extents: [f32; 3],
    /// Whether this collider is a solid wall (true) or a floor tile (false).
    pub is_wall: bool,
}

/// A complete level description: walls, cover boxes, and spawn points.
#[derive(Debug, Clone)]
pub struct LevelDescriptor {
    pub colliders: Vec<BoxCollider>,
    pub spawn_points: Vec<Position>,
}

/// Return the canonical level descriptor.
///
/// The simple starter level is a flat rectangular arena with four outer walls
/// and several box obstacles to provide cover.
pub fn level_descriptor() -> LevelDescriptor {
    let wall_thickness = 0.5;
    let h = ARENA_HALF;
    let floor_y = FLOOR_Y;
    let ceil_y = CEILING_Y;
    let wall_half_y = (ceil_y - floor_y) / 2.0;
    let wall_center_y = floor_y + wall_half_y;

    LevelDescriptor {
        colliders: vec![
            // ── Outer walls ──────────────────────────────────────────────────
            // North wall (+Z)
            BoxCollider {
                center: Position { x: 0.0, y: wall_center_y, z: h },
                half_extents: [h + wall_thickness, wall_half_y, wall_thickness],
                is_wall: true,
            },
            // South wall (-Z)
            BoxCollider {
                center: Position { x: 0.0, y: wall_center_y, z: -h },
                half_extents: [h + wall_thickness, wall_half_y, wall_thickness],
                is_wall: true,
            },
            // East wall (+X)
            BoxCollider {
                center: Position { x: h, y: wall_center_y, z: 0.0 },
                half_extents: [wall_thickness, wall_half_y, h + wall_thickness],
                is_wall: true,
            },
            // West wall (-X)
            BoxCollider {
                center: Position { x: -h, y: wall_center_y, z: 0.0 },
                half_extents: [wall_thickness, wall_half_y, h + wall_thickness],
                is_wall: true,
            },
            // ── Floor ────────────────────────────────────────────────────────
            BoxCollider {
                center: Position { x: 0.0, y: floor_y - 0.25, z: 0.0 },
                half_extents: [h, 0.25, h],
                is_wall: false,
            },
            // ── Cover boxes (symmetrical for fair spawning) ──────────────────
            // Centre block
            BoxCollider {
                center: Position { x: 0.0, y: 1.0, z: 0.0 },
                half_extents: [2.5, 1.0, 2.5],
                is_wall: true,
            },
            // NE quad box
            BoxCollider {
                center: Position { x: 15.0, y: 0.75, z: 15.0 },
                half_extents: [3.0, 0.75, 1.5],
                is_wall: true,
            },
            // NW quad box
            BoxCollider {
                center: Position { x: -15.0, y: 0.75, z: 15.0 },
                half_extents: [3.0, 0.75, 1.5],
                is_wall: true,
            },
            // SE quad box
            BoxCollider {
                center: Position { x: 15.0, y: 0.75, z: -15.0 },
                half_extents: [1.5, 0.75, 3.0],
                is_wall: true,
            },
            // SW quad box
            BoxCollider {
                center: Position { x: -15.0, y: 0.75, z: -15.0 },
                half_extents: [1.5, 0.75, 3.0],
                is_wall: true,
            },
            // Mid-lane pillars
            BoxCollider {
                center: Position { x: 8.0, y: 1.5, z: 0.0 },
                half_extents: [0.5, 1.5, 0.5],
                is_wall: true,
            },
            BoxCollider {
                center: Position { x: -8.0, y: 1.5, z: 0.0 },
                half_extents: [0.5, 1.5, 0.5],
                is_wall: true,
            },
        ],
        spawn_points: SPAWN_POINTS.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Spawn points
// ---------------------------------------------------------------------------

/// Pre-defined spawn points (alternating between team A and team B sides).
const SPAWN_POINTS: [Position; 8] = [
    Position { x: -30.0, y: 0.0, z: -30.0 },
    Position { x: 30.0, y: 0.0, z: 30.0 },
    Position { x: -30.0, y: 0.0, z: 30.0 },
    Position { x: 30.0, y: 0.0, z: -30.0 },
    Position { x: -20.0, y: 0.0, z: 0.0 },
    Position { x: 20.0, y: 0.0, z: 0.0 },
    Position { x: 0.0, y: 0.0, z: -20.0 },
    Position { x: 0.0, y: 0.0, z: 20.0 },
];

/// Choose a spawn point for `player_id` that is furthest from any alive enemy.
///
/// Falls back to round-robin selection when no enemies are alive (e.g., first
/// player to join).
pub fn spawn_point_for(player_id: PlayerId, state: &GameState) -> Position {
    let n = SPAWN_POINTS.len();
    let idx = (player_id.as_u64() as usize) % n;

    // Find the spawn furthest from any living enemy.
    let enemies: Vec<&PlayerState> = state
        .players
        .iter()
        .filter(|p| p.id != player_id && p.alive)
        .collect();

    if enemies.is_empty() {
        return SPAWN_POINTS[idx];
    }

    // Score each spawn: max distance to nearest enemy (higher = safer).
    SPAWN_POINTS
        .iter()
        .max_by(|a, b| {
            let score_a = min_dist_to_enemies(*a, &enemies);
            let score_b = min_dist_to_enemies(*b, &enemies);
            score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .unwrap_or(SPAWN_POINTS[idx])
}

fn min_dist_to_enemies(pos: &Position, enemies: &[&PlayerState]) -> f32 {
    enemies
        .iter()
        .map(|e| pos.distance_to(e.position))
        .fold(f32::INFINITY, f32::min)
}

// ---------------------------------------------------------------------------
// Collision helpers — pure-Rust, no rapier
// ---------------------------------------------------------------------------

/// Return the floor Y at position `p`.
///
/// In the simple arena, the floor is always at [`FLOOR_Y`].  A more complex
/// level would use a height-map or rapier `scene_query` here.
#[inline]
pub fn floor_at(_pos: Position) -> f32 {
    FLOOR_Y
}

/// Clamp `ps.position` to the arena bounds (XZ plane only).
///
/// Called from `FpsGame::handle_input` after movement to prevent players
/// from walking through the outer walls.
pub fn clamp_position(ps: &mut PlayerState) {
    let bound = ARENA_HALF - 0.5; // keep player inside the wall
    ps.position.x = ps.position.x.clamp(-bound, bound);
    ps.position.z = ps.position.z.clamp(-bound, bound);
    // Y is handled by the gravity integration system.
    ps.position.y = ps.position.y.clamp(FLOOR_Y, CEILING_Y);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::state::{GameState, PlayerState, Rotation};

    fn make_alive_player(id: u64, x: f32, z: f32) -> PlayerState {
        PlayerState {
            id: PlayerId::new(id),
            position: Position { x, y: 0.0, z },
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        }
    }

    #[test]
    fn level_descriptor_has_colliders_and_spawns() {
        let desc = level_descriptor();
        assert!(!desc.colliders.is_empty(), "level must have colliders");
        assert!(!desc.spawn_points.is_empty(), "level must have spawn points");
    }

    #[test]
    fn spawn_points_inside_arena() {
        for sp in SPAWN_POINTS.iter() {
            assert!(
                sp.x.abs() < ARENA_HALF && sp.z.abs() < ARENA_HALF,
                "spawn point {:?} must be inside the arena",
                sp
            );
        }
    }

    #[test]
    fn spawn_point_for_empty_state_returns_valid_point() {
        let state = GameState::default();
        let sp = spawn_point_for(PlayerId::new(0), &state);
        assert!(sp.x.abs() < ARENA_HALF);
        assert!(sp.z.abs() < ARENA_HALF);
    }

    #[test]
    fn spawn_point_for_picks_safe_spawn() {
        let mut state = GameState::default();
        // Enemy at one corner.
        state.players.push(make_alive_player(99, -30.0, -30.0));
        let sp = spawn_point_for(PlayerId::new(1), &state);
        // The chosen spawn should be far from the enemy.
        let enemy_pos = Position { x: -30.0, y: 0.0, z: -30.0 };
        let dist = sp.distance_to(enemy_pos);
        assert!(dist > 20.0, "should spawn far from the enemy; got dist={dist}");
    }

    #[test]
    fn floor_at_always_returns_floor_y() {
        let pos = Position { x: 5.0, y: 100.0, z: -5.0 };
        assert!((floor_at(pos) - FLOOR_Y).abs() < f32::EPSILON);
    }

    #[test]
    fn clamp_position_keeps_player_inside_bounds() {
        let mut ps = PlayerState {
            id: PlayerId::new(0),
            position: Position { x: 999.0, y: -5.0, z: -999.0 },
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        };
        clamp_position(&mut ps);
        assert!(ps.position.x.abs() <= ARENA_HALF);
        assert!(ps.position.z.abs() <= ARENA_HALF);
        assert!(ps.position.y >= FLOOR_Y);
        assert!(ps.position.y <= CEILING_Y);
    }
}
