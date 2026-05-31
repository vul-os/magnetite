//! # Replay verification wrapper
//!
//! Wraps the SDK's [`magnetite_sdk::authority::verify_replay`] function with a
//! richer interface that surfaces the divergence tick and suspected player(s).
//!
//! ## How it works
//!
//! 1. The runtime records a [`magnetite_sdk::authority::ReplayLog`] — one entry
//!    per tick containing every player's input and the authoritative state hash.
//! 2. At match end (or on demand), [`ReplayVerifier::verify`] re-simulates the
//!    entire log from scratch.
//! 3. A [`VerificationResult::Divergence`] names the first tick where the
//!    re-simulated hash differs from the recorded hash. The player whose input
//!    was present at that tick is listed as `suspected_players` — this is a
//!    heuristic, not a guarantee of guilt.
//!
//! ## Example
//!
//! ```rust
//! use magnetite_anticheat::replay_verifier::{ReplayVerifier, VerificationResult};
//! use magnetite_sdk::authority::{
//!     AuthoritativeGame, DeterministicRng, GameExecutor, MatchConfig,
//!     NativeExecutor, RejectReason, ReplayLog, StepCtx, Tick,
//! };
//! use magnetite_sdk::input::Input;
//! use magnetite_sdk::state::PlayerId;
//!
//! // --- minimal test game ---
//! struct Counter { n: u64 }
//! #[derive(serde::Serialize, serde::Deserialize, Clone)] struct CSnap { n: u64 }
//! #[derive(serde::Serialize, serde::Deserialize)] struct CDelta {}
//! #[derive(serde::Serialize)] struct CView {}
//! #[derive(serde::Serialize, serde::Deserialize)] struct CCmd;
//!
//! impl AuthoritativeGame for Counter {
//!     type Snapshot = CSnap; type Delta = CDelta; type View = CView; type Command = CCmd;
//!     fn init(_: &MatchConfig) -> Self { Counter { n: 0 } }
//!     fn validate(&self, _: PlayerId, _: &Input, _: Tick) -> Result<Vec<CCmd>, RejectReason> { Ok(vec![CCmd]) }
//!     fn step(&mut self, _: &mut StepCtx, cmds: &[(PlayerId, CCmd)]) { self.n += cmds.len() as u64; }
//!     fn snapshot(&self) -> CSnap { CSnap { n: self.n } }
//!     fn restore(s: &CSnap, _: &MatchConfig) -> Self { Counter { n: s.n } }
//!     fn delta(&self, _: &CSnap) -> CDelta { CDelta {} }
//!     fn view_for(&self, _: PlayerId) -> CView { CView {} }
//! }
//!
//! let cfg = MatchConfig::auto(2);
//! let mut exec = NativeExecutor::<Counter>::new(cfg.clone());
//! let mut log  = ReplayLog::new(cfg);
//! let p = PlayerId::new(1);
//!
//! for tick in 1u64..=5 {
//!     let inputs = vec![(p, Input::default())];
//!     let out = exec.step(tick, &inputs);
//!     log.record(tick, inputs, out.state_hash);
//! }
//!
//! let verifier = ReplayVerifier::new();
//! assert_eq!(verifier.verify::<Counter>(&log), VerificationResult::Clean);
//! ```

use magnetite_sdk::authority::{AuthoritativeGame, ReplayLog, ReplayVerdict};
use magnetite_sdk::state::PlayerId;

// ---------------------------------------------------------------------------
// VerificationResult
// ---------------------------------------------------------------------------

/// The outcome of a replay re-simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    /// The entire replay is consistent — no tampering or nondeterminism detected.
    Clean,
    /// A state hash mismatch was found.
    Divergence {
        /// The tick at which the first divergence was detected.
        tick: u64,
        /// The state hash recorded during the original run.
        expected: u64,
        /// The state hash produced by re-simulation.
        got: u64,
        /// Heuristic: the player(s) whose input was present at the diverging tick.
        ///
        /// This is not proof of cheating — nondeterminism bugs in game code can
        /// also cause divergence. Treat as a signal for further investigation.
        suspected_players: Vec<PlayerId>,
    },
}

// ---------------------------------------------------------------------------
// ReplayVerifier
// ---------------------------------------------------------------------------

/// Wraps [`magnetite_sdk::authority::verify_replay`] with richer diagnostics.
///
/// `ReplayVerifier` is stateless — construct one and call [`verify`](Self::verify)
/// as many times as needed.
#[derive(Debug, Default, Clone)]
pub struct ReplayVerifier {
    _private: (),
}

impl ReplayVerifier {
    /// Construct a new verifier.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Re-simulate `log` and return a [`VerificationResult`].
    ///
    /// On divergence, the players whose inputs appear at the offending tick are
    /// returned as `suspected_players`.
    pub fn verify<G: AuthoritativeGame>(&self, log: &ReplayLog) -> VerificationResult {
        let verdict = magnetite_sdk::authority::verify_replay::<G>(log);

        match verdict {
            ReplayVerdict::Clean => VerificationResult::Clean,
            ReplayVerdict::Divergence {
                tick,
                expected,
                got,
            } => {
                // Identify players whose inputs were present at the diverging tick.
                let suspected_players = log
                    .frames
                    .iter()
                    .find(|(t, _)| *t == tick)
                    .map(|(_, inputs)| inputs.iter().map(|(pid, _)| *pid).collect())
                    .unwrap_or_default();

                VerificationResult::Divergence {
                    tick,
                    expected,
                    got,
                    suspected_players,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::authority::{
        AuthoritativeGame, GameExecutor, MatchConfig, NativeExecutor, RejectReason, ReplayLog,
        StepCtx, Tick,
    };
    use magnetite_sdk::input::Input;
    use magnetite_sdk::state::PlayerId;

    // --- Minimal counter game for testing -----------------------------------

    struct CounterGame {
        count: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct CounterSnap {
        count: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct CounterDelta {}

    #[derive(serde::Serialize)]
    struct CounterView {}

    #[derive(serde::Serialize, serde::Deserialize)]
    struct CounterCmd;

    impl AuthoritativeGame for CounterGame {
        type Snapshot = CounterSnap;
        type Delta = CounterDelta;
        type View = CounterView;
        type Command = CounterCmd;

        fn init(_cfg: &MatchConfig) -> Self {
            CounterGame { count: 0 }
        }

        fn validate(
            &self,
            _player: PlayerId,
            _input: &Input,
            _tick: Tick,
        ) -> Result<Vec<CounterCmd>, RejectReason> {
            Ok(vec![CounterCmd])
        }

        fn step(&mut self, _ctx: &mut StepCtx, cmds: &[(PlayerId, CounterCmd)]) {
            self.count += cmds.len() as u64;
        }

        fn snapshot(&self) -> CounterSnap {
            CounterSnap { count: self.count }
        }

        fn restore(snap: &CounterSnap, _cfg: &MatchConfig) -> Self {
            CounterGame { count: snap.count }
        }

        fn delta(&self, _since: &CounterSnap) -> CounterDelta {
            CounterDelta {}
        }

        fn view_for(&self, _player: PlayerId) -> CounterView {
            CounterView {}
        }
    }

    // --- Helpers ------------------------------------------------------------

    fn build_clean_log(ticks: u64) -> (ReplayLog, Vec<(u64, u64)>) {
        let cfg = MatchConfig::auto(2);
        let mut exec = NativeExecutor::<CounterGame>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg);
        let p = PlayerId::new(1);
        let mut hashes = Vec::new();

        for tick in 1..=ticks {
            let inputs = vec![(p, Input::default())];
            let out = exec.step(tick, &inputs);
            hashes.push((tick, out.state_hash));
            log.record(tick, inputs, out.state_hash);
        }

        (log, hashes)
    }

    // --- Tests --------------------------------------------------------------

    #[test]
    fn clean_log_returns_clean() {
        let (log, _) = build_clean_log(10);
        let v = ReplayVerifier::new();
        assert_eq!(v.verify::<CounterGame>(&log), VerificationResult::Clean);
    }

    #[test]
    fn tampered_hash_returns_divergence() {
        let (mut log, _) = build_clean_log(5);

        // Tamper: change the recorded hash for tick 3.
        for (tick, hash) in &mut log.state_hashes {
            if *tick == 3 {
                *hash = hash.wrapping_add(1);
            }
        }

        let v = ReplayVerifier::new();
        let result = v.verify::<CounterGame>(&log);

        match result {
            VerificationResult::Divergence {
                tick,
                suspected_players,
                ..
            } => {
                assert_eq!(tick, 3, "divergence should be at tick 3");
                assert!(
                    !suspected_players.is_empty(),
                    "should have suspected players"
                );
                assert!(
                    suspected_players.contains(&PlayerId::new(1)),
                    "player 1 was active at tick 3"
                );
            }
            VerificationResult::Clean => {
                panic!("expected Divergence, got Clean");
            }
        }
    }

    #[test]
    fn tampered_input_returns_divergence() {
        let cfg = MatchConfig::auto(2);
        let mut exec = NativeExecutor::<CounterGame>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg);
        let p = PlayerId::new(42);

        // Record 5 ticks honestly.
        for tick in 1u64..=5 {
            let inputs = vec![(p, Input::default())];
            let out = exec.step(tick, &inputs);
            log.record(tick, inputs, out.state_hash);
        }

        // Tamper: inject an extra player input at tick 2.
        for (tick, inputs) in &mut log.frames {
            if *tick == 2 {
                inputs.push((PlayerId::new(99), Input::default()));
            }
        }

        let v = ReplayVerifier::new();
        let result = v.verify::<CounterGame>(&log);
        // Injected extra input changes count → different state hash → Divergence.
        assert!(
            matches!(result, VerificationResult::Divergence { .. }),
            "expected Divergence for tampered input, got {result:?}"
        );
    }

    #[test]
    fn empty_log_returns_clean() {
        let cfg = MatchConfig::auto(2);
        let log = ReplayLog::new(cfg);
        let v = ReplayVerifier::new();
        assert_eq!(v.verify::<CounterGame>(&log), VerificationResult::Clean);
    }

    #[test]
    fn suspected_players_lists_all_players_at_tick() {
        let cfg = MatchConfig::auto(2);
        let mut exec = NativeExecutor::<CounterGame>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg);
        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);

        for tick in 1u64..=3 {
            let inputs = vec![(p1, Input::default()), (p2, Input::default())];
            let out = exec.step(tick, &inputs);
            log.record(tick, inputs, out.state_hash);
        }

        // Tamper tick 2.
        for (tick, hash) in &mut log.state_hashes {
            if *tick == 2 {
                *hash = hash.wrapping_add(1);
            }
        }

        let v = ReplayVerifier::new();
        match v.verify::<CounterGame>(&log) {
            VerificationResult::Divergence {
                tick,
                suspected_players,
                ..
            } => {
                assert_eq!(tick, 2);
                assert!(suspected_players.contains(&p1));
                assert!(suspected_players.contains(&p2));
            }
            VerificationResult::Clean => panic!("expected Divergence"),
        }
    }

    #[test]
    fn verifier_is_stateless_multiple_calls() {
        let (log, _) = build_clean_log(5);
        let v = ReplayVerifier::new();
        // Calling verify multiple times should give the same result.
        assert_eq!(v.verify::<CounterGame>(&log), VerificationResult::Clean);
        assert_eq!(v.verify::<CounterGame>(&log), VerificationResult::Clean);
    }
}
