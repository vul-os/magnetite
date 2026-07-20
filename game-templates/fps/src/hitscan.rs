//! # Hitscan / Projectile Raycasting
//!
//! Pure-Rust, no-Bevy hitscan helpers for the server-authoritative game loop.
//! In the `native`/`wasm` Bevy build the same logic runs on the server; the
//! client uses `bevy_rapier3d` for visual queries only.
//!
//! ## Algorithm
//!
//! A **hitscan** is a ray cast that resolves instantly within a single tick:
//!
//! 1. Build a ray from `origin` in the direction encoded by `rotation`
//!    (yaw + pitch → unit vector).
//! 2. For each living enemy player, compute the closest point on the ray to
//!    the player's capsule (simplified to a sphere of radius [`HIT_RADIUS`]).
//! 3. If the closest distance is ≤ [`HIT_RADIUS`], record a hit.
//! 4. Among all hits, keep the **closest** to the shooter.
//! 5. Apply damage; check for headshot (hit Y > player Y + [`HEADSHOT_Y_OFFSET`]).
//!
//! This is intentionally simple — rapier3d `QueryPipeline::cast_ray` replaces
//! this in the `native`/`wasm` feature builds for accurate mesh colliders.
//!
//! ## Projectile path (future)
//!
//! For a projectile (rocket, grenade) call [`Projectile::step`] each tick until
//! it hits something or expires.

use magnetite_sdk::state::{GameState, PlayerId, Position, Rotation};

use crate::ShotResult;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Half-width of the player capsule for hitscan intersection (metres).
pub const HIT_RADIUS: f32 = 0.4;

/// Maximum hitscan range (metres).
pub const MAX_RANGE: f32 = 500.0;

/// Eye-height offset above `position.y` for the hitbox centre.
pub const BODY_CENTER_Y: f32 = 0.9;

/// Height above `position.y` where "headshot" zone begins.
const HEADSHOT_Y_OFFSET: f32 = 1.6;

// ---------------------------------------------------------------------------
// Ray
// ---------------------------------------------------------------------------

/// A world-space ray for hitscan calculations.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Position,
    /// Normalised direction vector.
    pub direction: [f32; 3],
}

impl Ray {
    /// Build a ray from a position and Euler rotation (yaw/pitch in **radians**).
    ///
    /// Convention (Y-up, right-hand):
    /// - yaw=0, pitch=0 → looking -Z
    /// - yaw rotates around Y; pitch rotates around local X
    pub fn from_rotation(origin: Position, rotation: Rotation) -> Self {
        let yaw = rotation.yaw;
        let pitch = rotation.pitch;
        // Forward direction:
        //   dx = -sin(yaw) * cos(pitch)
        //   dy =  sin(pitch)
        //   dz = -cos(yaw) * cos(pitch)
        let cp = pitch.cos();
        let dx = -yaw.sin() * cp;
        let dy = pitch.sin();
        let dz = -yaw.cos() * cp;
        // Already normalised given unit pitch/yaw.
        Self {
            origin,
            direction: [dx, dy, dz],
        }
    }

    /// Point on the ray at parameter `t`.
    #[inline]
    pub fn at(&self, t: f32) -> Position {
        Position {
            x: self.origin.x + self.direction[0] * t,
            y: self.origin.y + self.direction[1] * t,
            z: self.origin.z + self.direction[2] * t,
        }
    }

    /// Closest distance from the ray to point `p`, and the ray parameter `t`
    /// at the closest point.
    pub fn closest_to_point(&self, p: Position) -> (f32, f32) {
        let ox = p.x - self.origin.x;
        let oy = p.y - self.origin.y;
        let oz = p.z - self.origin.z;
        let t = ox * self.direction[0] + oy * self.direction[1] + oz * self.direction[2];
        let t = t.max(0.0); // only forward along the ray
        let closest = self.at(t);
        let dist =
            ((closest.x - p.x).powi(2) + (closest.y - p.y).powi(2) + (closest.z - p.z).powi(2))
                .sqrt();
        (dist, t)
    }
}

// ---------------------------------------------------------------------------
// Hitscan cast
// ---------------------------------------------------------------------------

/// Cast a hitscan ray from `origin` in the direction of `rotation`.
///
/// Checks all living players *except* `shooter_id` for intersection with a
/// [`HIT_RADIUS`]-sphere centred on `player.position + [0, BODY_CENTER_Y, 0]`.
///
/// Returns a [`ShotResult`] with the closest hit (or a miss).
pub fn cast(
    origin: Position,
    rotation: Rotation,
    state: &GameState,
    shooter_id: PlayerId,
) -> ShotResult {
    let ray = Ray::from_rotation(origin, rotation);

    let mut best_t = MAX_RANGE;
    let mut best_result: Option<ShotResult> = None;

    for ps in state.players.iter() {
        if ps.id == shooter_id || !ps.alive {
            continue;
        }

        // Check body sphere first.
        let body_center = Position {
            x: ps.position.x,
            y: ps.position.y + BODY_CENTER_Y,
            z: ps.position.z,
        };
        let (dist, t) = ray.closest_to_point(body_center);

        if dist <= HIT_RADIUS && t < best_t && t <= MAX_RANGE {
            let hit_pos = ray.at(t);

            // Determine headshot: hit Y above headshot threshold.
            let headshot = hit_pos.y >= ps.position.y + HEADSHOT_Y_OFFSET;
            let damage = if headshot {
                HITSCAN_BASE_DAMAGE * crate::HEADSHOT_MULT
            } else {
                HITSCAN_BASE_DAMAGE
            };

            best_t = t;
            best_result = Some(ShotResult {
                hit_player: Some(ps.id),
                hit_pos,
                damage,
                headshot,
            });
        }
    }

    best_result.unwrap_or(ShotResult {
        hit_player: None,
        hit_pos: ray.at(best_t.min(MAX_RANGE)),
        damage: 0.0,
        headshot: false,
    })
}

/// Base hitscan damage (mirrored from lib.rs const for use in this module).
const HITSCAN_BASE_DAMAGE: f32 = 25.0;

// ---------------------------------------------------------------------------
// Projectile (tick-based)
// ---------------------------------------------------------------------------

/// A tick-based projectile (rocket, grenade, bullet with travel time).
///
/// Each tick the caller should call [`Projectile::step`] and check
/// [`Projectile::alive`] to determine if the projectile has expired.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Projectile {
    /// Current world-space position.
    pub position: Position,
    /// Velocity vector (m/s).
    pub velocity: [f32; 3],
    /// Shooter.
    pub owner: PlayerId,
    /// Damage on impact.
    pub damage: f32,
    /// Blast radius for AoE (0 = no splash).
    pub splash_radius: f32,
    /// Ticks remaining before the projectile self-destructs.
    pub ttl: u32,
}

use serde::{Deserialize, Serialize};

impl Projectile {
    /// Create a new projectile fired from `origin` along `direction` at `speed` m/s.
    pub fn new(
        owner: PlayerId,
        origin: Position,
        direction: [f32; 3],
        speed: f32,
        damage: f32,
        splash_radius: f32,
        ttl_ticks: u32,
    ) -> Self {
        let len = (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2))
            .sqrt()
            .max(1e-6);
        let velocity = [
            direction[0] / len * speed,
            direction[1] / len * speed,
            direction[2] / len * speed,
        ];
        Self {
            position: origin,
            velocity,
            owner,
            damage,
            splash_radius,
            ttl: ttl_ticks,
        }
    }

    /// Advance the projectile by one tick (dt = 1/60 s).
    ///
    /// Returns the [`ShotResult`] if the projectile hit something this tick,
    /// or `None` if it is still in flight.
    pub fn step(&mut self, state: &GameState) -> Option<ShotResult> {
        if self.ttl == 0 {
            return None;
        }
        self.ttl -= 1;

        const DT: f32 = 1.0 / 60.0;
        // Simple Euler integration — rapier3d replaces this in Bevy builds.
        self.position.x += self.velocity[0] * DT;
        self.position.y += self.velocity[1] * DT;
        self.position.z += self.velocity[2] * DT;

        // Simple sphere collision against all living players.
        for ps in state.players.iter() {
            if ps.id == self.owner || !ps.alive {
                continue;
            }
            let body = Position {
                x: ps.position.x,
                y: ps.position.y + BODY_CENTER_Y,
                z: ps.position.z,
            };
            let dist = ((self.position.x - body.x).powi(2)
                + (self.position.y - body.y).powi(2)
                + (self.position.z - body.z).powi(2))
            .sqrt();

            if dist <= HIT_RADIUS + self.splash_radius {
                self.ttl = 0; // destroy the projectile
                return Some(ShotResult {
                    hit_player: Some(ps.id),
                    hit_pos: self.position,
                    damage: self.damage,
                    headshot: false,
                });
            }
        }

        // Floor collision.
        if self.position.y <= 0.0 {
            self.ttl = 0;
        }

        None
    }

    /// Whether the projectile is still in flight.
    #[inline]
    pub fn alive(&self) -> bool {
        self.ttl > 0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::state::{GameState, PlayerState, Rotation};

    fn make_player(id: u64, x: f32, y: f32, z: f32) -> PlayerState {
        PlayerState {
            id: PlayerId::new(id),
            position: Position { x, y, z },
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        }
    }

    #[test]
    fn ray_direction_zero_yaw_zero_pitch() {
        // yaw=0, pitch=0 → looking -Z.
        let ray = Ray::from_rotation(
            Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Rotation {
                yaw: 0.0,
                pitch: 0.0,
                roll: 0.0,
            },
        );
        assert!((ray.direction[0]).abs() < 1e-5, "dx should be ~0");
        assert!((ray.direction[1]).abs() < 1e-5, "dy should be ~0");
        assert!((ray.direction[2] + 1.0).abs() < 1e-5, "dz should be ~-1");
    }

    #[test]
    fn ray_at_zero_is_origin() {
        let ray = Ray::from_rotation(
            Position {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            Rotation::default(),
        );
        let p = ray.at(0.0);
        assert!((p.x - 1.0).abs() < 1e-5);
        assert!((p.y - 2.0).abs() < 1e-5);
        assert!((p.z - 3.0).abs() < 1e-5);
    }

    #[test]
    fn hitscan_misses_when_no_enemies() {
        let mut state = GameState::default();
        state.players.push(make_player(0, 0.0, 0.0, 0.0));
        let origin = Position {
            x: 0.0,
            y: 1.7,
            z: 0.0,
        };
        let rot = Rotation {
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let result = cast(origin, rot, &state, PlayerId::new(0));
        assert!(result.hit_player.is_none());
        assert!((result.damage).abs() < f32::EPSILON);
    }

    #[test]
    fn hitscan_hits_player_directly_in_front() {
        let mut state = GameState::default();
        // Shooter at origin, target 5m in front (-Z).
        // Both at y=0 → body centers at BODY_CENTER_Y (0.9m).
        // Shoot from body-center height so the horizontal ray intersects the target body.
        state.players.push(make_player(0, 0.0, 0.0, 0.0));
        state.players.push(make_player(1, 0.0, 0.0, -5.0));

        // Origin at body-center height (0.9m), aimed horizontally (-Z).
        let origin = Position {
            x: 0.0,
            y: BODY_CENTER_Y,
            z: 0.0,
        };
        let rot = Rotation {
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let result = cast(origin, rot, &state, PlayerId::new(0));

        assert_eq!(result.hit_player, Some(PlayerId::new(1)));
        assert!(result.damage > 0.0);
    }

    #[test]
    fn hitscan_skips_dead_player() {
        let mut state = GameState::default();
        state.players.push(make_player(0, 0.0, 0.0, 0.0));
        let mut dead = make_player(1, 0.0, 0.0, -5.0);
        dead.alive = false;
        state.players.push(dead);

        let origin = Position {
            x: 0.0,
            y: BODY_CENTER_Y,
            z: 0.0,
        };
        let rot = Rotation {
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let result = cast(origin, rot, &state, PlayerId::new(0));
        assert!(result.hit_player.is_none(), "dead player must not be hit");
    }

    #[test]
    fn hitscan_picks_closest_target() {
        let mut state = GameState::default();
        state.players.push(make_player(0, 0.0, 0.0, 0.0));
        state.players.push(make_player(1, 0.0, 0.0, -5.0));
        state.players.push(make_player(2, 0.0, 0.0, -3.0)); // closer

        let origin = Position {
            x: 0.0,
            y: BODY_CENTER_Y,
            z: 0.0,
        };
        let rot = Rotation {
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let result = cast(origin, rot, &state, PlayerId::new(0));

        // Should hit player 2 (closer, only 3m away vs 5m).
        assert_eq!(result.hit_player, Some(PlayerId::new(2)));
    }

    #[test]
    fn projectile_steps_forward() {
        let shooter = PlayerId::new(0);
        let mut proj = Projectile::new(
            shooter,
            Position {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            [0.0, 0.0, -1.0],
            10.0,
            50.0,
            0.0,
            60,
        );
        let state = GameState::default();
        proj.step(&state);
        assert!(
            proj.position.z < 0.0,
            "projectile should move in -Z direction"
        );
        assert!(proj.alive());
    }

    #[test]
    fn projectile_expires_after_ttl() {
        let shooter = PlayerId::new(0);
        let mut proj = Projectile::new(
            shooter,
            Position {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            [0.0, 0.0, -1.0],
            1.0,
            25.0,
            0.0,
            2,
        );
        let state = GameState::default();
        proj.step(&state);
        assert!(proj.alive());
        proj.step(&state);
        assert!(!proj.alive(), "projectile should expire after ttl ticks");
    }
}
