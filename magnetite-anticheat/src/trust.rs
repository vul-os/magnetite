//! # Trust score aggregation
//!
//! Tracks a per-player mutable trust score. The score increases when violations
//! are flagged and decays naturally over time. When thresholds are crossed the
//! system emits escalating [`AntiCheatEvent`]s: `Warn` → `Kick` → `Ban`.
//!
//! ## Score lifecycle
//!
//! ```text
//! 0 ──── warn_threshold ──── kick_threshold ──── ban_threshold ────▶ score
//!                 ▲                    ▲                    ▲
//!             Warn event          Kick event           Ban event
//! ```
//!
//! Each call to [`TrustScoreMap::flag`] increments the player's score by 1 and
//! returns the appropriate event. [`TrustScoreMap::decay`] should be called once
//! per tick per player to slowly reduce the score back toward zero for players
//! who behave normally after a violation.
//!
//! ## Example
//!
//! ```rust
//! use magnetite_anticheat::trust::{TrustScoreMap, AntiCheatEvent};
//! use magnetite_sdk::authority::RejectReason;
//! use magnetite_sdk::state::PlayerId;
//!
//! let mut scores = TrustScoreMap::new(3, 6, 10, 600, 1);
//! let pid = PlayerId::new(1);
//!
//! // First 3 flags → Warn.
//! for i in 0..3 {
//!     scores.flag(pid, RejectReason::RateLimited, i);
//! }
//! // Score is now 3 → at warn_threshold; next flag → Kick territory.
//! let event = scores.flag(pid, RejectReason::RateLimited, 3);
//! // After kick_threshold flags the event escalates.
//! ```

use std::collections::HashMap;

use magnetite_sdk::authority::{RejectReason, Tick};
use magnetite_sdk::state::PlayerId;

// ---------------------------------------------------------------------------
// AntiCheatEvent
// ---------------------------------------------------------------------------

/// Event emitted by [`TrustScoreMap::flag`] after accumulating a violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AntiCheatEvent {
    /// Score crossed no threshold yet — just reject the input.
    Reject,
    /// Score just crossed the warn threshold — log internally, reject input.
    Warn,
    /// Score crossed the kick threshold — runtime should kick the player.
    Kick,
    /// Score crossed the ban threshold — runtime should ban the player.
    Ban,
}

// ---------------------------------------------------------------------------
// PlayerTrust — internal per-player state
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct PlayerTrust {
    /// Accumulated flag score. Increments on each violation; decays over time.
    score: u32,
    /// The highest [`AntiCheatEvent`] level reached so far.
    max_level: AntiCheatEvent,
    /// Tick at which the last decay step was applied.
    last_decay_tick: Tick,
}

impl PlayerTrust {
    fn new(tick: Tick) -> Self {
        Self {
            score: 0,
            max_level: AntiCheatEvent::Reject,
            last_decay_tick: tick,
        }
    }
}

// ---------------------------------------------------------------------------
// TrustScoreMap
// ---------------------------------------------------------------------------

/// Per-player trust score registry.
///
/// Holds the mutable trust state for every active player. Intended to be owned
/// by [`crate::Anticheat`]; can also be used standalone for testing.
pub struct TrustScoreMap {
    players: HashMap<PlayerId, PlayerTrust>,
    warn_threshold: u32,
    kick_threshold: u32,
    ban_threshold: u32,
    decay_interval_ticks: u64,
    decay_amount: u32,
}

impl TrustScoreMap {
    /// Construct a new trust score map.
    ///
    /// # Parameters
    /// * `warn_threshold` — score at which a `Warn` event is issued.
    /// * `kick_threshold` — score at which a `Kick` event is issued.
    /// * `ban_threshold`  — score at which a `Ban` event is issued.
    /// * `decay_interval_ticks` — ticks between score decay steps.
    /// * `decay_amount`   — score points subtracted each decay step.
    pub fn new(
        warn_threshold: u32,
        kick_threshold: u32,
        ban_threshold: u32,
        decay_interval_ticks: u64,
        decay_amount: u32,
    ) -> Self {
        Self {
            players: HashMap::new(),
            warn_threshold,
            kick_threshold,
            ban_threshold,
            decay_interval_ticks,
            decay_amount,
        }
    }

    /// Record a violation for `player` at `tick` and return the resulting event.
    ///
    /// The player's score is incremented by 1 (reflecting a single flag event).
    /// The returned [`AntiCheatEvent`] reflects the highest threshold crossed so far.
    pub fn flag(&mut self, player: PlayerId, _reason: RejectReason, tick: Tick) -> AntiCheatEvent {
        let trust = self
            .players
            .entry(player)
            .or_insert_with(|| PlayerTrust::new(tick));
        trust.score = trust.score.saturating_add(1);

        let event = if trust.score >= self.ban_threshold {
            AntiCheatEvent::Ban
        } else if trust.score >= self.kick_threshold {
            AntiCheatEvent::Kick
        } else if trust.score >= self.warn_threshold {
            AntiCheatEvent::Warn
        } else {
            AntiCheatEvent::Reject
        };

        // Record the highest level ever reached for this player.
        if event_level(&event) > event_level(&trust.max_level) {
            trust.max_level = event.clone();
        }

        event
    }

    /// Decay the trust score for `player` by `decay_amount` every
    /// `decay_interval_ticks` ticks.
    ///
    /// Should be called once per tick per player (e.g. from
    /// [`crate::Anticheat::inspect`]).
    pub fn decay(&mut self, player: PlayerId, tick: Tick) {
        if let Some(trust) = self.players.get_mut(&player) {
            let elapsed = tick.saturating_sub(trust.last_decay_tick);
            if elapsed >= self.decay_interval_ticks {
                trust.score = trust.score.saturating_sub(self.decay_amount);
                trust.last_decay_tick = tick;
            }
        }
    }

    /// Return the current trust score for a player (0 if never seen).
    pub fn score(&self, player: PlayerId) -> u32 {
        self.players.get(&player).map(|t| t.score).unwrap_or(0)
    }

    /// Return the highest anti-cheat event level reached by this player.
    pub fn max_level(&self, player: PlayerId) -> Option<&AntiCheatEvent> {
        self.players.get(&player).map(|t| &t.max_level)
    }

    /// Remove a player's trust record (e.g. after they disconnect).
    pub fn remove(&mut self, player: PlayerId) {
        self.players.remove(&player);
    }
}

/// Map an `AntiCheatEvent` to a numeric severity for comparison.
fn event_level(e: &AntiCheatEvent) -> u8 {
    match e {
        AntiCheatEvent::Reject => 0,
        AntiCheatEvent::Warn => 1,
        AntiCheatEvent::Kick => 2,
        AntiCheatEvent::Ban => 3,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::authority::RejectReason;
    use magnetite_sdk::state::PlayerId;

    fn make_scores() -> TrustScoreMap {
        // warn=3, kick=6, ban=10, decay every 100 ticks by 1
        TrustScoreMap::new(3, 6, 10, 100, 1)
    }

    #[test]
    fn initial_score_is_zero() {
        let scores = make_scores();
        assert_eq!(scores.score(PlayerId::new(1)), 0);
    }

    #[test]
    fn flag_increments_score() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        scores.flag(pid, RejectReason::RateLimited, 0);
        assert_eq!(scores.score(pid), 1);
        scores.flag(pid, RejectReason::RateLimited, 1);
        assert_eq!(scores.score(pid), 2);
    }

    #[test]
    fn flag_returns_reject_below_warn() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        let e = scores.flag(pid, RejectReason::RateLimited, 0);
        assert_eq!(e, AntiCheatEvent::Reject);
    }

    #[test]
    fn flag_returns_warn_at_threshold() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        // Flag 3 times to reach warn_threshold = 3.
        for tick in 0u64..3 {
            scores.flag(pid, RejectReason::RateLimited, tick);
        }
        assert_eq!(scores.score(pid), 3);
        // Score = warn_threshold → Warn.
        let e = scores.flag(pid, RejectReason::RateLimited, 3);
        // After 4 flags score = 4, still < kick (6) → Warn.
        assert_eq!(e, AntiCheatEvent::Warn);
    }

    #[test]
    fn flag_returns_kick_at_threshold() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        for tick in 0u64..6 {
            scores.flag(pid, RejectReason::RateLimited, tick);
        }
        // Score = 6 = kick_threshold.
        let e = scores.flag(pid, RejectReason::RateLimited, 6);
        assert_eq!(e, AntiCheatEvent::Kick);
    }

    #[test]
    fn flag_returns_ban_at_threshold() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        for tick in 0u64..10 {
            scores.flag(pid, RejectReason::RateLimited, tick);
        }
        // Score = 10 = ban_threshold.
        let e = scores.flag(pid, RejectReason::RateLimited, 10);
        assert_eq!(e, AntiCheatEvent::Ban);
    }

    #[test]
    fn decay_reduces_score() {
        let mut scores = make_scores(); // decay every 100 ticks
        let pid = PlayerId::new(1);
        // Build up score to 5.
        for tick in 0u64..5 {
            scores.flag(pid, RejectReason::RateLimited, tick);
        }
        assert_eq!(scores.score(pid), 5);
        // Trigger decay at tick 100.
        scores.decay(pid, 100);
        assert_eq!(scores.score(pid), 4);
        // Another decay at tick 200.
        scores.decay(pid, 200);
        assert_eq!(scores.score(pid), 3);
    }

    #[test]
    fn decay_does_not_go_below_zero() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        // 1 flag.
        scores.flag(pid, RejectReason::RateLimited, 0);
        assert_eq!(scores.score(pid), 1);
        // Decay twice — score should saturate at 0, not underflow.
        scores.decay(pid, 100);
        scores.decay(pid, 200);
        assert_eq!(scores.score(pid), 0);
    }

    #[test]
    fn decay_ignored_before_interval() {
        let mut scores = make_scores(); // decay every 100 ticks
        let pid = PlayerId::new(1);
        scores.flag(pid, RejectReason::RateLimited, 0);
        assert_eq!(scores.score(pid), 1);
        // Decay at tick 50 — too soon; score unchanged.
        scores.decay(pid, 50);
        assert_eq!(scores.score(pid), 1);
    }

    #[test]
    fn max_level_tracks_highest_event() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        // Reach kick.
        for tick in 0u64..7 {
            scores.flag(pid, RejectReason::RateLimited, tick);
        }
        assert_eq!(scores.max_level(pid), Some(&AntiCheatEvent::Kick));
    }

    #[test]
    fn remove_clears_player() {
        let mut scores = make_scores();
        let pid = PlayerId::new(1);
        scores.flag(pid, RejectReason::RateLimited, 0);
        scores.remove(pid);
        assert_eq!(scores.score(pid), 0);
        assert_eq!(scores.max_level(pid), None);
    }

    #[test]
    fn independent_players() {
        let mut scores = make_scores();
        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);

        for tick in 0u64..5 {
            scores.flag(p1, RejectReason::RateLimited, tick);
        }
        // p1 has score 5; p2 has 0.
        assert_eq!(scores.score(p1), 5);
        assert_eq!(scores.score(p2), 0);
    }
}
