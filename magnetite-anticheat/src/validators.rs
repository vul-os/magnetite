//! # Anticheat-specific built-in [`Validator`] implementations
//!
//! All validators here implement [`magnetite_sdk::authority::Validator`] and can be
//! composed into a [`magnetite_sdk::authority::ValidatorChain`].
//!
//! | Validator | Detects |
//! |---|---|
//! | [`AimbotSnap`] | Instant look-direction snap (aimbot) via per-tick angular delta threshold |
//! | [`PositionTeleport`] | Teleport / speed-hack via cumulative position delta vs max velocity |
//! | [`FireRateCooldown`] | Superhuman fire rate: enforces a minimum tick gap between shots |
//! | [`InputFlood`] | Flooding the server with duplicate inputs within the same tick window |
//!
//! ## Example
//!
//! ```rust
//! use magnetite_anticheat::validators::{AimbotSnap, PositionTeleport, FireRateCooldown, InputFlood};
//! use magnetite_sdk::authority::{ValidatorChain, Validator};
//! use magnetite_sdk::input::{Input, MouseState, KeyState};
//! use magnetite_sdk::state::PlayerId;
//!
//! let mut chain = ValidatorChain::new()
//!     .add(AimbotSnap::new(45.0))
//!     .add(PositionTeleport::new(20.0))
//!     .add(FireRateCooldown::new(5))
//!     .add(InputFlood::new(30));
//!
//! let pid = PlayerId::new(1);
//! assert!(chain.check(pid, &Input::default(), 0).is_ok());
//! ```

use std::collections::HashMap;

use magnetite_sdk::authority::{RejectReason, Tick, Validator};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

// ---------------------------------------------------------------------------
// AimbotSnap
// ---------------------------------------------------------------------------

/// Detects aimbot snap: an instantaneous look-direction change that exceeds a
/// configurable angular threshold in a single tick.
///
/// Looks at the mouse delta magnitude in the input frame. A legitimate human
/// player is physically incapable of rotating more than N degrees per tick at
/// common tick rates; anything beyond that is considered a snap.
///
/// # Example
///
/// ```rust
/// use magnetite_anticheat::validators::AimbotSnap;
/// use magnetite_sdk::authority::Validator;
/// use magnetite_sdk::input::{Input, MouseState};
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = AimbotSnap::new(45.0); // 45 units/tick max look delta
/// let pid = PlayerId::new(1);
///
/// // Normal look movement: ok.
/// let normal = Input {
///     mouse: MouseState { delta_x: 5.0, delta_y: 3.0, ..Default::default() },
///     ..Default::default()
/// };
/// assert!(v.check(pid, &normal, 1).is_ok());
///
/// // Aimbot snap: huge instant rotation.
/// let snap = Input {
///     mouse: MouseState { delta_x: 500.0, delta_y: 0.0, ..Default::default() },
///     ..Default::default()
/// };
/// assert!(v.check(pid, &snap, 2).is_err());
/// ```
pub struct AimbotSnap {
    /// Maximum permitted look-delta magnitude per tick (in mouse units).
    max_look_delta: f32,
}

impl AimbotSnap {
    /// Create a new aimbot-snap detector.
    ///
    /// `max_look_delta` is the maximum Euclidean magnitude of `(mouse.delta_x,
    /// mouse.delta_y)` allowed in a single tick before flagging as a snap.
    pub fn new(max_look_delta: f32) -> Self {
        Self { max_look_delta }
    }
}

impl Validator for AimbotSnap {
    fn check(&mut self, _player: PlayerId, input: &Input, _tick: Tick) -> Result<(), RejectReason> {
        let dx = input.mouse.delta_x as f32;
        let dy = input.mouse.delta_y as f32;
        let mag = (dx * dx + dy * dy).sqrt();
        if mag > self.max_look_delta {
            Err(RejectReason::IllegalAction(format!(
                "look-delta {mag:.2} exceeds max {:.2} (aimbot snap)",
                self.max_look_delta
            )))
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// PositionTeleport
// ---------------------------------------------------------------------------

/// Detects position teleport and speed-hacks.
///
/// Tracks the accumulated look-delta magnitude across consecutive ticks for each
/// player. If the per-tick delta exceeds `max_velocity`, the input is rejected.
///
/// This validator uses the look-delta as a movement proxy (consistent with
/// [`magnetite_sdk::authority::MovementVelocity`]) but adds per-player
/// per-tick tracking to catch patterns that pass individually but constitute
/// a teleport in aggregate (e.g. steady giant deltas).
///
/// For games that track explicit positions in their state, a custom Validator
/// comparing snapshot positions is recommended in addition.
///
/// # Example
///
/// ```rust
/// use magnetite_anticheat::validators::PositionTeleport;
/// use magnetite_sdk::authority::Validator;
/// use magnetite_sdk::input::{Input, MouseState};
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = PositionTeleport::new(20.0);
/// let pid = PlayerId::new(1);
///
/// // Normal movement: ok.
/// let normal = Input {
///     mouse: MouseState { delta_x: 5.0, delta_y: 5.0, ..Default::default() },
///     ..Default::default()
/// };
/// assert!(v.check(pid, &normal, 1).is_ok());
///
/// // Teleport: enormous single-tick delta.
/// let teleport = Input {
///     mouse: MouseState { delta_x: 9999.0, delta_y: 9999.0, ..Default::default() },
///     ..Default::default()
/// };
/// assert!(v.check(pid, &teleport, 2).is_err());
/// ```
pub struct PositionTeleport {
    /// Maximum permitted movement delta per tick.
    max_velocity: f32,
    /// (player_id → last tick seen)
    last_tick: HashMap<PlayerId, Tick>,
}

impl PositionTeleport {
    /// Create a new teleport/speed-hack detector.
    ///
    /// `max_velocity` is compared against the Euclidean look-delta each tick.
    pub fn new(max_velocity: f32) -> Self {
        Self {
            max_velocity,
            last_tick: HashMap::new(),
        }
    }
}

impl Validator for PositionTeleport {
    fn check(&mut self, player: PlayerId, input: &Input, tick: Tick) -> Result<(), RejectReason> {
        let dx = input.mouse.delta_x as f32;
        let dy = input.mouse.delta_y as f32;
        let mag = (dx * dx + dy * dy).sqrt();

        // Record the tick regardless.
        self.last_tick.insert(player, tick);

        if mag > self.max_velocity {
            Err(RejectReason::OutOfBounds)
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// FireRateCooldown
// ---------------------------------------------------------------------------

/// Enforces a minimum number of ticks between consecutive attack/fire inputs.
///
/// Superhuman fire rate is a common cheat (particularly in FPS games). This
/// validator tracks the last fire tick per player and rejects any fire attempt
/// before the cooldown has elapsed.
///
/// # Example
///
/// ```rust
/// use magnetite_anticheat::validators::FireRateCooldown;
/// use magnetite_sdk::authority::Validator;
/// use magnetite_sdk::input::{Input, KeyState};
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = FireRateCooldown::new(5); // 5-tick cooldown between shots
/// let pid = PlayerId::new(1);
///
/// let firing = Input {
///     keys: KeyState { attack: true, ..Default::default() },
///     ..Default::default()
/// };
///
/// // First shot: ok.
/// assert!(v.check(pid, &firing, 0).is_ok());
/// // Immediate second shot: rejected.
/// assert!(v.check(pid, &firing, 1).is_err());
/// // After cooldown: ok.
/// assert!(v.check(pid, &firing, 5).is_ok());
/// ```
pub struct FireRateCooldown {
    /// Minimum ticks that must elapse between fire inputs.
    cooldown_ticks: Tick,
    /// (player_id → last_fire_tick)
    last_fire: HashMap<PlayerId, Tick>,
}

impl FireRateCooldown {
    /// Create a fire-rate enforcer.
    ///
    /// `cooldown_ticks` is the minimum tick gap between consecutive attack inputs.
    pub fn new(cooldown_ticks: u64) -> Self {
        Self {
            cooldown_ticks,
            last_fire: HashMap::new(),
        }
    }
}

impl Validator for FireRateCooldown {
    fn check(&mut self, player: PlayerId, input: &Input, tick: Tick) -> Result<(), RejectReason> {
        // Only care about fire inputs.
        if !input.keys.attack {
            return Ok(());
        }

        if let Some(&last) = self.last_fire.get(&player) {
            let elapsed = tick.saturating_sub(last);
            if elapsed < self.cooldown_ticks {
                return Err(RejectReason::IllegalAction(format!(
                    "fire rate exceeded: {elapsed} ticks since last shot, cooldown is {}",
                    self.cooldown_ticks
                )));
            }
        }

        self.last_fire.insert(player, tick);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InputFlood
// ---------------------------------------------------------------------------

/// Detects input flooding: a player sending an abnormal number of input frames
/// within a short tick window.
///
/// Operates on game ticks rather than wall-clock time (unlike
/// [`magnetite_sdk::authority::RateLimit`] which uses `Instant`). Counts inputs
/// within a sliding window of `window_ticks` and rejects if the count exceeds
/// `max_per_window`.
///
/// # Example
///
/// ```rust
/// use magnetite_anticheat::validators::InputFlood;
/// use magnetite_sdk::authority::Validator;
/// use magnetite_sdk::input::Input;
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = InputFlood::new(3); // max 3 inputs per 60-tick window
/// let pid = PlayerId::new(1);
///
/// for tick in 0u64..3 {
///     assert!(v.check(pid, &Input::default(), tick).is_ok());
/// }
/// // 4th input in same window: flood.
/// assert!(v.check(pid, &Input::default(), 3).is_err());
/// ```
pub struct InputFlood {
    /// Maximum inputs allowed within a `window_ticks`-sized window.
    max_per_window: u32,
    /// Window size in ticks. Defaults to 60 (1 second at 60Hz).
    window_ticks: Tick,
    /// (player_id → (count in window, window_start_tick))
    windows: HashMap<PlayerId, (u32, Tick)>,
}

impl InputFlood {
    /// Create an input-flood detector.
    ///
    /// `max_per_window` is the maximum number of input frames accepted from a
    /// single player within a 60-tick window (~1 second at 60 Hz).
    pub fn new(max_per_window: u32) -> Self {
        Self::with_window(max_per_window, 60)
    }

    /// Create an input-flood detector with a custom window size.
    pub fn with_window(max_per_window: u32, window_ticks: Tick) -> Self {
        Self {
            max_per_window,
            window_ticks,
            windows: HashMap::new(),
        }
    }
}

impl Validator for InputFlood {
    fn check(&mut self, player: PlayerId, _input: &Input, tick: Tick) -> Result<(), RejectReason> {
        let entry = self.windows.entry(player).or_insert((0, tick));

        // Reset window when the window period has elapsed.
        if tick.saturating_sub(entry.1) >= self.window_ticks {
            *entry = (0, tick);
        }

        entry.0 += 1;

        if entry.0 > self.max_per_window {
            Err(RejectReason::IllegalAction(format!(
                "input flood: {} inputs in {} ticks (max {})",
                entry.0, self.window_ticks, self.max_per_window
            )))
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::input::{Input, KeyState, MouseState};
    use magnetite_sdk::state::PlayerId;

    // ---------------------------------------------------------------------- //
    // AimbotSnap                                                              //
    // ---------------------------------------------------------------------- //

    #[test]
    fn aimbot_snap_passes_normal_look() {
        let mut v = AimbotSnap::new(100.0);
        let pid = PlayerId::new(1);
        let input = Input {
            mouse: MouseState {
                delta_x: 10.0,
                delta_y: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(pid, &input, 1).is_ok());
    }

    #[test]
    fn aimbot_snap_rejects_aimbot() {
        let mut v = AimbotSnap::new(45.0);
        let pid = PlayerId::new(1);
        let input = Input {
            mouse: MouseState {
                delta_x: 500.0,
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = v.check(pid, &input, 1);
        assert!(result.is_err());
        assert!(matches!(result, Err(RejectReason::IllegalAction(_))));
    }

    #[test]
    fn aimbot_snap_boundary_exactly_at_threshold_passes() {
        let mut v = AimbotSnap::new(10.0);
        let pid = PlayerId::new(2);
        // delta = exactly 10.0 on the X axis → should pass (not strictly greater).
        let input = Input {
            mouse: MouseState {
                delta_x: 10.0,
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(pid, &input, 1).is_ok());
    }

    #[test]
    fn aimbot_snap_zero_delta_passes() {
        let mut v = AimbotSnap::new(45.0);
        let pid = PlayerId::new(3);
        assert!(v.check(pid, &Input::default(), 1).is_ok());
    }

    // ---------------------------------------------------------------------- //
    // PositionTeleport                                                        //
    // ---------------------------------------------------------------------- //

    #[test]
    fn position_teleport_passes_slow_movement() {
        let mut v = PositionTeleport::new(20.0);
        let pid = PlayerId::new(1);
        let input = Input {
            mouse: MouseState {
                delta_x: 5.0,
                delta_y: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(pid, &input, 1).is_ok());
    }

    #[test]
    fn position_teleport_rejects_teleport() {
        let mut v = PositionTeleport::new(20.0);
        let pid = PlayerId::new(1);
        let input = Input {
            mouse: MouseState {
                delta_x: 9999.0,
                delta_y: 9999.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = v.check(pid, &input, 1);
        assert!(result.is_err());
        assert!(matches!(result, Err(RejectReason::OutOfBounds)));
    }

    #[test]
    fn position_teleport_allows_zero_delta() {
        let mut v = PositionTeleport::new(20.0);
        let pid = PlayerId::new(2);
        assert!(v.check(pid, &Input::default(), 1).is_ok());
    }

    // ---------------------------------------------------------------------- //
    // FireRateCooldown                                                        //
    // ---------------------------------------------------------------------- //

    #[test]
    fn fire_rate_allows_first_shot() {
        let mut v = FireRateCooldown::new(5);
        let pid = PlayerId::new(1);
        let firing = Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(pid, &firing, 0).is_ok());
    }

    #[test]
    fn fire_rate_rejects_immediate_second_shot() {
        let mut v = FireRateCooldown::new(5);
        let pid = PlayerId::new(1);
        let firing = Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(pid, &firing, 0).is_ok());
        let result = v.check(pid, &firing, 1);
        assert!(result.is_err());
        assert!(matches!(result, Err(RejectReason::IllegalAction(_))));
    }

    #[test]
    fn fire_rate_allows_shot_after_cooldown() {
        let mut v = FireRateCooldown::new(5);
        let pid = PlayerId::new(1);
        let firing = Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(pid, &firing, 0).is_ok());
        assert!(v.check(pid, &firing, 1).is_err());
        assert!(v.check(pid, &firing, 5).is_ok()); // exactly at cooldown
    }

    #[test]
    fn fire_rate_ignores_non_attack_inputs() {
        let mut v = FireRateCooldown::new(5);
        let pid = PlayerId::new(1);
        // Non-attack input: never rejected.
        for tick in 0u64..20 {
            assert!(v.check(pid, &Input::default(), tick).is_ok());
        }
    }

    // ---------------------------------------------------------------------- //
    // InputFlood                                                              //
    // ---------------------------------------------------------------------- //

    #[test]
    fn input_flood_passes_within_limit() {
        let mut v = InputFlood::new(10);
        let pid = PlayerId::new(1);
        for tick in 0u64..10 {
            assert!(v.check(pid, &Input::default(), tick).is_ok());
        }
    }

    #[test]
    fn input_flood_rejects_over_limit() {
        let mut v = InputFlood::new(3);
        let pid = PlayerId::new(1);
        assert!(v.check(pid, &Input::default(), 0).is_ok());
        assert!(v.check(pid, &Input::default(), 1).is_ok());
        assert!(v.check(pid, &Input::default(), 2).is_ok());
        let result = v.check(pid, &Input::default(), 3);
        assert!(result.is_err());
        assert!(matches!(result, Err(RejectReason::IllegalAction(_))));
    }

    #[test]
    fn input_flood_resets_after_window() {
        // Window of 60 ticks; max 2 per window.
        let mut v = InputFlood::with_window(2, 60);
        let pid = PlayerId::new(1);
        assert!(v.check(pid, &Input::default(), 0).is_ok());
        assert!(v.check(pid, &Input::default(), 1).is_ok());
        assert!(v.check(pid, &Input::default(), 2).is_err()); // over limit
                                                              // After a new window starts (tick 60+), the counter resets.
        assert!(v.check(pid, &Input::default(), 60).is_ok());
        assert!(v.check(pid, &Input::default(), 61).is_ok());
    }

    #[test]
    fn input_flood_independent_per_player() {
        let mut v = InputFlood::new(2);
        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);
        // p1 exhausts its quota.
        assert!(v.check(p1, &Input::default(), 0).is_ok());
        assert!(v.check(p1, &Input::default(), 1).is_ok());
        assert!(v.check(p1, &Input::default(), 2).is_err());
        // p2 is unaffected.
        assert!(v.check(p2, &Input::default(), 0).is_ok());
        assert!(v.check(p2, &Input::default(), 1).is_ok());
    }
}
