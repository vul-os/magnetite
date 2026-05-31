//! # `magnetite-anticheat`
//!
//! First-class composable anti-cheat for the Magnetite platform.
//!
//! ## Architecture
//!
//! ```text
//! Client Input
//!      │
//!      ▼
//! ┌──────────────────────────────────────┐
//! │  ValidatorChain (sdk built-ins +     │
//! │  anticheat built-ins chained in seq) │
//! └──────────────────────┬───────────────┘
//!                        │ Ok / Err(RejectReason)
//!                        ▼
//! ┌──────────────────────────────────────┐
//! │  Anticheat::inspect(player, input,   │
//! │  tick) -> Decision                   │
//! │   • passes Ok inputs through         │
//! │   • feeds flags to TrustScoreMap     │
//! └──────────────────────┬───────────────┘
//!                        │
//!                        ▼
//! ┌──────────────────────────────────────┐
//! │  TrustScoreMap — per-player          │
//! │  decay + escalation (Warn→Kick→Ban)  │
//! │  emits AntiCheatEvent                │
//! └──────────────────────────────────────┘
//! ```
//!
//! ## Quick start
//!
//! ```rust
//! use magnetite_anticheat::{
//!     Anticheat, AnticheatConfig, Decision,
//!     validators::{AimbotSnap, PositionTeleport, FireRateCooldown, InputFlood},
//! };
//! use magnetite_sdk::authority::{RateLimit, InputSchema, ValidatorChain};
//! use magnetite_sdk::state::PlayerId;
//! use magnetite_sdk::input::Input;
//!
//! let chain = ValidatorChain::new()
//!     .add(RateLimit::new(120))
//!     .add(InputSchema::default())
//!     .add(AimbotSnap::new(45.0))
//!     .add(PositionTeleport::new(20.0))
//!     .add(FireRateCooldown::new(5))
//!     .add(InputFlood::new(30));
//!
//! let mut ac = Anticheat::new(chain, AnticheatConfig::default());
//!
//! let pid = PlayerId::new(1);
//! match ac.inspect(pid, &Input::default(), 0) {
//!     Decision::Allow => {}
//!     Decision::Reject(reason) => eprintln!("rejected: {reason}"),
//!     Decision::Kick(pid) => eprintln!("kick player {pid:?}"),
//!     Decision::Ban(pid) => eprintln!("ban player {pid:?}"),
//! }
//! ```

pub mod replay_verifier;
pub mod trust;
pub mod validators;

use magnetite_sdk::authority::{RejectReason, Tick, Validator, ValidatorChain};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

use trust::{AntiCheatEvent, TrustScoreMap};

// ---------------------------------------------------------------------------
// Decision — the top-level inspect output
// ---------------------------------------------------------------------------

/// The authoritative action the runtime should take after the anti-cheat
/// pipeline evaluates a single player input.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Input passes all checks — let it through.
    Allow,
    /// Input is malformed or violates a rule — reject this frame silently.
    Reject(RejectReason),
    /// The player's trust score has escalated to the kick threshold.
    Kick(PlayerId),
    /// The player's trust score has escalated to the ban threshold.
    Ban(PlayerId),
}

// ---------------------------------------------------------------------------
// AnticheatConfig
// ---------------------------------------------------------------------------

/// Tunable thresholds for the [`Anticheat`] pipeline.
#[derive(Debug, Clone)]
pub struct AnticheatConfig {
    /// Number of flag events before issuing a `Warn` (default: 3).
    pub warn_threshold: u32,
    /// Number of flag events before issuing a `Kick` (default: 8).
    pub kick_threshold: u32,
    /// Number of flag events before issuing a `Ban` (default: 15).
    pub ban_threshold: u32,
    /// How many ticks between trust-score decay steps (default: 600 ticks = ~10s @ 60Hz).
    pub decay_interval_ticks: u64,
    /// How many flag points to subtract per decay step (default: 1).
    pub decay_amount: u32,
}

impl Default for AnticheatConfig {
    fn default() -> Self {
        Self {
            warn_threshold: 3,
            kick_threshold: 8,
            ban_threshold: 15,
            decay_interval_ticks: 600,
            decay_amount: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Anticheat — the top-level composable pipeline
// ---------------------------------------------------------------------------

/// Top-level anti-cheat pipeline.
///
/// Wraps a [`ValidatorChain`] (which aggregates any [`Validator`] impls — both
/// SDK built-ins and anticheat-specific ones) and a [`TrustScoreMap`].
///
/// Call [`Anticheat::inspect`] once per player input in the server tick loop.
pub struct Anticheat {
    chain: ValidatorChain,
    trust: TrustScoreMap,
    config: AnticheatConfig,
}

impl Anticheat {
    /// Construct a new pipeline with the given validator chain and config.
    pub fn new(chain: ValidatorChain, config: AnticheatConfig) -> Self {
        let trust = TrustScoreMap::new(
            config.warn_threshold,
            config.kick_threshold,
            config.ban_threshold,
            config.decay_interval_ticks,
            config.decay_amount,
        );
        Self {
            chain,
            trust,
            config: config.clone(),
        }
    }

    /// Evaluate one player input frame through the full anti-cheat pipeline.
    ///
    /// 1. Runs the [`ValidatorChain`] on the input.
    /// 2. On failure, records a flag in the [`TrustScoreMap`].
    /// 3. Decays trust scores (called once per tick per player via this path).
    /// 4. Returns the appropriate [`Decision`].
    pub fn inspect(&mut self, player: PlayerId, input: &Input, tick: Tick) -> Decision {
        // Apply per-tick trust decay for this player.
        self.trust.decay(player, tick);

        match self.chain.check(player, input, tick) {
            Ok(()) => Decision::Allow,
            Err(reason) => {
                // Record the violation flag and check for escalation.
                let event = self.trust.flag(player, reason.clone(), tick);
                match event {
                    AntiCheatEvent::Reject => Decision::Reject(reason),
                    AntiCheatEvent::Warn => Decision::Reject(reason),
                    AntiCheatEvent::Kick => Decision::Kick(player),
                    AntiCheatEvent::Ban => Decision::Ban(player),
                }
            }
        }
    }

    /// Direct access to the trust score map (e.g. for external flag injection
    /// from replay verification or other signals).
    pub fn trust_mut(&mut self) -> &mut TrustScoreMap {
        &mut self.trust
    }

    /// Read-only access to the config.
    pub fn config(&self) -> &AnticheatConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validators::{AimbotSnap, FireRateCooldown, InputFlood, PositionTeleport};
    use magnetite_sdk::authority::{InputSchema, RateLimit, ValidatorChain};
    use magnetite_sdk::input::{Input, KeyState, MouseState};
    use magnetite_sdk::state::PlayerId;

    fn make_ac() -> Anticheat {
        let chain = ValidatorChain::new()
            .add(RateLimit::new(120))
            .add(InputSchema::default())
            .add(AimbotSnap::new(45.0))
            .add(PositionTeleport::new(20.0))
            .add(FireRateCooldown::new(5))
            .add(InputFlood::new(30));
        Anticheat::new(chain, AnticheatConfig::default())
    }

    #[test]
    fn clean_input_is_allowed() {
        let mut ac = make_ac();
        let pid = PlayerId::new(1);
        let result = ac.inspect(pid, &Input::default(), 0);
        assert_eq!(result, Decision::Allow);
    }

    #[test]
    fn aimbot_snap_triggers_reject() {
        let mut ac = make_ac();
        let pid = PlayerId::new(2);

        // Snap: huge look delta in one frame exceeds the threshold.
        let snap_input = Input {
            mouse: MouseState {
                delta_x: 500.0, // far above 45.0 deg threshold
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let result = ac.inspect(pid, &snap_input, 0);
        // Should be Reject (first violation is below kick/ban thresholds by default).
        assert!(
            matches!(
                result,
                Decision::Reject(_) | Decision::Kick(_) | Decision::Ban(_)
            ),
            "expected rejection for aimbot snap, got {result:?}"
        );
    }

    #[test]
    fn position_teleport_triggers_reject() {
        let mut ac = make_ac();
        let pid = PlayerId::new(3);

        // First frame establishes position via normal delta.
        let normal = Input {
            mouse: MouseState {
                delta_x: 1.0,
                delta_y: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let _ = ac.inspect(pid, &normal, 0);

        // Second frame: huge position jump.
        let teleport = Input {
            mouse: MouseState {
                delta_x: 9999.0,
                delta_y: 9999.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = ac.inspect(pid, &teleport, 1);
        assert!(
            matches!(
                result,
                Decision::Reject(_) | Decision::Kick(_) | Decision::Ban(_)
            ),
            "expected rejection for teleport, got {result:?}"
        );
    }

    #[test]
    fn fire_rate_cooldown_triggers_reject() {
        let mut ac = make_ac();
        let pid = PlayerId::new(4);

        let firing = Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // First shot allowed.
        assert_eq!(ac.inspect(pid, &firing, 0), Decision::Allow);
        // Immediate second shot is rejected (cooldown = 5 ticks).
        let result = ac.inspect(pid, &firing, 1);
        assert!(
            matches!(
                result,
                Decision::Reject(_) | Decision::Kick(_) | Decision::Ban(_)
            ),
            "expected fire-rate rejection, got {result:?}"
        );
    }

    #[test]
    fn input_flood_triggers_reject() {
        let chain = ValidatorChain::new().add(InputFlood::new(3)); // very low cap for testing
        let mut ac = Anticheat::new(chain, AnticheatConfig::default());
        let pid = PlayerId::new(5);

        // First 3 inputs in same tick-window: ok.
        for tick in 0u64..3 {
            assert_eq!(ac.inspect(pid, &Input::default(), tick), Decision::Allow);
        }
        // 4th: flood detected.
        let result = ac.inspect(pid, &Input::default(), 3);
        assert!(
            matches!(
                result,
                Decision::Reject(_) | Decision::Kick(_) | Decision::Ban(_)
            ),
            "expected flood rejection, got {result:?}"
        );
    }

    #[test]
    fn trust_escalation_to_kick() {
        // Use a config where kick threshold is very low.
        let chain = ValidatorChain::new().add(AimbotSnap::new(0.0001)); // always rejects
        let cfg = AnticheatConfig {
            warn_threshold: 2,
            kick_threshold: 4,
            ban_threshold: 100,
            decay_interval_ticks: 100_000, // no decay during test
            decay_amount: 1,
        };
        let mut ac = Anticheat::new(chain, cfg);
        let pid = PlayerId::new(10);

        let bad = Input {
            mouse: MouseState {
                delta_x: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut kicked = false;
        for tick in 0u64..10 {
            match ac.inspect(pid, &bad, tick) {
                Decision::Kick(_) | Decision::Ban(_) => {
                    kicked = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(kicked, "should have been kicked after repeated violations");
    }

    #[test]
    fn trust_escalation_to_ban() {
        let chain = ValidatorChain::new().add(AimbotSnap::new(0.0001)); // always rejects
        let cfg = AnticheatConfig {
            warn_threshold: 1,
            kick_threshold: 2,
            ban_threshold: 4,
            decay_interval_ticks: 100_000,
            decay_amount: 1,
        };
        let mut ac = Anticheat::new(chain, cfg);
        let pid = PlayerId::new(11);

        let bad = Input {
            mouse: MouseState {
                delta_x: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut banned = false;
        for tick in 0u64..20 {
            if let Decision::Ban(_) = ac.inspect(pid, &bad, tick) {
                banned = true;
                break;
            }
        }
        assert!(banned, "should have been banned after many violations");
    }
}
