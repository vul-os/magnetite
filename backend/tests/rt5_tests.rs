// rt5_tests.rs — AGENT 5: REPLAY + TOURNAMENT tests.
//
// Coverage:
//   1. ReplayLog round-trip  — a well-formed ReplayLog serialises and deserialises cleanly.
//   2. verify_replay clean   — a deterministic game produces ReplayVerdict::Clean.
//   3. verify_replay tamper  — a tampered state_hash produces ReplayVerdict::Divergence.
//   4. verify_replay empty   — an empty log is Clean.
//   5. Tournament bracket generation logic — pure-logic: num_rounds = ceil(log2(players)).
//   6. Tournament bracket advance logic    — pure-logic: winner advances to next round.
//   7. TournamentStatus round-trip via Display + FromStr.
//   8. MatchConfig::auto topology selection.
//   9. ReplayLog::record accumulates entries correctly.
//  10. verify_replay with multiple players.

// NOTE: all tests that exercise ReplayLog / verify_replay are entirely in-process
// (no DB, no network).  Tournament bracket logic is tested as pure-logic mirrors
// of the functions in backend/src/api/tournaments.rs.

// ─────────────────────────────────────────────────────────────────────────────
// 1–4, 9–10  ReplayLog + verify_replay
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod replay_tests {
    use magnetite_sdk::authority::{
        verify_replay, AuthoritativeGame, GameExecutor, MatchConfig, NativeExecutor, RejectReason,
        ReplayLog, ReplayVerdict, StepCtx, Tick,
    };
    use magnetite_sdk::input::Input;
    use magnetite_sdk::state::PlayerId;

    // -------------------------------------------------------------------------
    // Minimal deterministic game for testing: a counter incremented by the
    // number of commands received each tick.
    // -------------------------------------------------------------------------

    struct CounterGame {
        counter: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct CounterSnap {
        counter: u64,
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
            CounterGame { counter: 0 }
        }

        fn validate(
            &self,
            _p: PlayerId,
            _i: &Input,
            _t: Tick,
        ) -> Result<Vec<CounterCmd>, RejectReason> {
            Ok(vec![CounterCmd])
        }

        fn step(&mut self, _ctx: &mut StepCtx, cmds: &[(PlayerId, CounterCmd)]) {
            self.counter += cmds.len() as u64;
        }

        fn snapshot(&self) -> CounterSnap {
            CounterSnap {
                counter: self.counter,
            }
        }

        fn restore(s: &CounterSnap, _cfg: &MatchConfig) -> Self {
            CounterGame { counter: s.counter }
        }

        fn delta(&self, _s: &CounterSnap) -> CounterDelta {
            CounterDelta {}
        }

        fn view_for(&self, _p: PlayerId) -> CounterView {
            CounterView {}
        }
    }

    // -------------------------------------------------------------------------
    // Helper: build a clean ReplayLog using CounterGame.
    // -------------------------------------------------------------------------

    fn build_clean_log(ticks: u64, players: &[PlayerId]) -> ReplayLog {
        let cfg = MatchConfig::auto(players.len() as u32);
        let mut exec = NativeExecutor::<CounterGame>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg);

        for tick in 1..=ticks {
            let inputs: Vec<(PlayerId, Input)> =
                players.iter().map(|&p| (p, Input::default())).collect();
            let out = exec.step(tick, &inputs);
            log.record(tick, inputs, out.state_hash);
        }
        log
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 1. ReplayLog round-trip: serialise → deserialise → content identical.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn replay_log_serde_roundtrip() {
        let p = PlayerId::new(1);
        let log = build_clean_log(5, &[p]);

        let json = serde_json::to_string(&log).expect("serialise ReplayLog");
        let log2: ReplayLog = serde_json::from_str(&json).expect("deserialise ReplayLog");

        assert_eq!(log.frames.len(), log2.frames.len(), "frame count");
        assert_eq!(
            log.state_hashes.len(),
            log2.state_hashes.len(),
            "hash count"
        );

        for (i, ((t1, _), (t2, _))) in log.frames.iter().zip(log2.frames.iter()).enumerate() {
            assert_eq!(t1, t2, "tick mismatch at frame {i}");
        }
        for (i, ((t1, h1), (t2, h2))) in log
            .state_hashes
            .iter()
            .zip(log2.state_hashes.iter())
            .enumerate()
        {
            assert_eq!(t1, t2, "hash tick mismatch at {i}");
            assert_eq!(h1, h2, "hash value mismatch at {i}");
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 2. verify_replay returns Clean for a well-formed log.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn verify_replay_clean_log() {
        let p = PlayerId::new(1);
        let log = build_clean_log(10, &[p]);
        assert_eq!(
            verify_replay::<CounterGame>(&log),
            ReplayVerdict::Clean,
            "clean log must verify as Clean"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 3. verify_replay returns Divergence when a state hash is tampered.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn verify_replay_tampered_hash_diverges() {
        let p = PlayerId::new(1);
        let mut log = build_clean_log(5, &[p]);

        // Corrupt the hash at tick 3 (index 2).
        assert!(
            log.state_hashes.len() >= 3,
            "log must have at least 3 ticks"
        );
        let (tick, ref mut hash) = log.state_hashes[2];
        *hash = hash.wrapping_add(1); // flip a bit — any change causes divergence

        let verdict = verify_replay::<CounterGame>(&log);
        match verdict {
            ReplayVerdict::Divergence { tick: dt, .. } => {
                assert_eq!(dt, tick, "divergence tick must match corrupted entry");
            }
            ReplayVerdict::Clean => panic!("tampered log must NOT verify as Clean"),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 4. verify_replay on an empty log is Clean.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn verify_replay_empty_log_is_clean() {
        let cfg = MatchConfig::auto(2);
        let log = ReplayLog::new(cfg);
        assert_eq!(
            verify_replay::<CounterGame>(&log),
            ReplayVerdict::Clean,
            "empty log must be Clean"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 9. ReplayLog::record accumulates entries correctly.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn replay_log_record_accumulates() {
        let cfg = MatchConfig::auto(2);
        let mut log = ReplayLog::new(cfg);

        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);

        log.record(1, vec![(p1, Input::default())], 0xABCD);
        log.record(
            2,
            vec![(p1, Input::default()), (p2, Input::default())],
            0xDEAD,
        );

        assert_eq!(log.frames.len(), 2);
        assert_eq!(log.state_hashes.len(), 2);

        // Tick 1 has one input; tick 2 has two.
        assert_eq!(log.frames[0].0, 1);
        assert_eq!(log.frames[0].1.len(), 1);
        assert_eq!(log.frames[1].0, 2);
        assert_eq!(log.frames[1].1.len(), 2);

        // Hashes stored correctly.
        assert_eq!(log.state_hashes[0], (1, 0xABCD));
        assert_eq!(log.state_hashes[1], (2, 0xDEAD));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 10. verify_replay with two players — determinism holds with multi-player.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn verify_replay_multiplay_clean() {
        let players = vec![PlayerId::new(1), PlayerId::new(2), PlayerId::new(3)];
        let log = build_clean_log(8, &players);
        assert_eq!(
            verify_replay::<CounterGame>(&log),
            ReplayVerdict::Clean,
            "multi-player log must verify as Clean"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Bonus: ReplayLog JSON contains the expected fields.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn replay_log_json_fields_present() {
        let p = PlayerId::new(42);
        let log = build_clean_log(3, &[p]);
        let json = serde_json::to_string(&log).expect("serialise");
        let val: serde_json::Value = serde_json::from_str(&json).expect("parse");

        assert!(val.get("config").is_some(), "must have 'config' field");
        assert!(val.get("frames").is_some(), "must have 'frames' field");
        assert!(
            val.get("state_hashes").is_some(),
            "must have 'state_hashes' field"
        );

        let frames = val["frames"].as_array().expect("frames is array");
        assert_eq!(frames.len(), 3, "three ticks recorded");

        let hashes = val["state_hashes"]
            .as_array()
            .expect("state_hashes is array");
        assert_eq!(hashes.len(), 3, "three hashes recorded");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5–7  Tournament bracket generation / advance (pure-logic)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tournament_bracket_tests {
    // Pure-logic mirrors of the backend's bracket math (tournaments.rs).
    //
    // The actual handler uses:
    //   let num_rounds = (num_players as f64).log2().ceil() as i32;
    //   for round in 1..=num_rounds {
    //       let matches_in_round = 2_i32.pow((num_rounds - round) as u32);
    //   }
    //
    // We mirror that logic here and verify it for common tournament sizes.

    fn num_rounds(num_players: usize) -> i32 {
        (num_players as f64).log2().ceil() as i32
    }

    fn matches_in_round(num_rounds: i32, round: i32) -> i32 {
        2_i32.pow((num_rounds - round) as u32)
    }

    fn total_matches(num_players: usize) -> i32 {
        let r = num_rounds(num_players);
        (1..=r).map(|round| matches_in_round(r, round)).sum()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 5. Bracket generation: correct round/match counts for power-of-2 sizes.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn bracket_generation_2_players() {
        // 2 players → 1 round, 1 match.
        assert_eq!(num_rounds(2), 1);
        assert_eq!(total_matches(2), 1);
    }

    #[test]
    fn bracket_generation_4_players() {
        // 4 players → 2 rounds: 2 matches in round 1, 1 match in round 2 = 3 total.
        assert_eq!(num_rounds(4), 2);
        assert_eq!(matches_in_round(2, 1), 2);
        assert_eq!(matches_in_round(2, 2), 1);
        assert_eq!(total_matches(4), 3);
    }

    #[test]
    fn bracket_generation_8_players() {
        // 8 players → 3 rounds: 4 + 2 + 1 = 7 total matches.
        assert_eq!(num_rounds(8), 3);
        assert_eq!(total_matches(8), 7);
    }

    #[test]
    fn bracket_generation_16_players() {
        // 16 players → 4 rounds: 8+4+2+1 = 15 total.
        assert_eq!(num_rounds(16), 4);
        assert_eq!(total_matches(16), 15);
    }

    #[test]
    fn bracket_generation_non_power_of_2() {
        // 5 players → ceil(log2(5)) = 3 rounds.
        assert_eq!(num_rounds(5), 3);
        // Matches: 4+2+1 = 7 (server seeds byes into extra slots).
        assert_eq!(total_matches(5), 7);
    }

    #[test]
    fn bracket_generation_3_players() {
        // 3 players → ceil(log2(3)) = 2 rounds.
        assert_eq!(num_rounds(3), 2);
        assert_eq!(total_matches(3), 3);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 6. Bracket advance: winner occupies next slot (pure logic).
    // ─────────────────────────────────────────────────────────────────────────

    // Mirror: the winner of match M in round R advances to match ceil(M/2)
    // in round R+1.  This is standard single-elimination bracket math.
    fn next_match_number(match_number: i32) -> i32 {
        (match_number as f64 / 2.0).ceil() as i32
    }

    #[test]
    fn bracket_advance_match1_to_round2() {
        // Match 1 winner → round 2, match 1.
        assert_eq!(next_match_number(1), 1);
    }

    #[test]
    fn bracket_advance_match2_to_round2() {
        // Match 2 winner → round 2, match 1.
        assert_eq!(next_match_number(2), 1);
    }

    #[test]
    fn bracket_advance_match3_to_round2_match2() {
        // Match 3 winner → round 2, match 2.
        assert_eq!(next_match_number(3), 2);
    }

    #[test]
    fn bracket_advance_match4_to_round2_match2() {
        // Match 4 winner → round 2, match 2.
        assert_eq!(next_match_number(4), 2);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 7. TournamentStatus Display + FromStr round-trip (mirrors the enum).
    // ─────────────────────────────────────────────────────────────────────────

    // Mirror the status names the real backend uses.
    #[derive(Debug, PartialEq, Eq)]
    enum TournamentStatus {
        Draft,
        Registration,
        InProgress,
        Completed,
        Cancelled,
    }

    impl std::fmt::Display for TournamentStatus {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let s = match self {
                TournamentStatus::Draft => "Draft",
                TournamentStatus::Registration => "Registration",
                TournamentStatus::InProgress => "InProgress",
                TournamentStatus::Completed => "Completed",
                TournamentStatus::Cancelled => "Cancelled",
            };
            write!(f, "{}", s)
        }
    }

    impl std::str::FromStr for TournamentStatus {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "Draft" => Ok(TournamentStatus::Draft),
                "Registration" => Ok(TournamentStatus::Registration),
                "InProgress" => Ok(TournamentStatus::InProgress),
                "Completed" => Ok(TournamentStatus::Completed),
                "Cancelled" => Ok(TournamentStatus::Cancelled),
                other => Err(format!("Unknown status: {}", other)),
            }
        }
    }

    #[test]
    fn status_display_round_trips() {
        let statuses = [
            TournamentStatus::Draft,
            TournamentStatus::Registration,
            TournamentStatus::InProgress,
            TournamentStatus::Completed,
            TournamentStatus::Cancelled,
        ];
        for s in statuses {
            let displayed = s.to_string();
            let parsed: TournamentStatus = displayed
                .parse()
                .unwrap_or_else(|e| panic!("parse failed for '{}': {}", displayed, e));
            assert_eq!(
                std::mem::discriminant(&s),
                std::mem::discriminant(&parsed),
                "round-trip failed for {}",
                displayed
            );
        }
    }

    #[test]
    fn status_invalid_string_returns_error() {
        let result: Result<TournamentStatus, _> = "Deleted".parse();
        assert!(
            result.is_err(),
            "'Deleted' must not parse to a valid status"
        );
    }

    #[test]
    fn status_display_values_match_expected_strings() {
        assert_eq!(TournamentStatus::Draft.to_string(), "Draft");
        assert_eq!(TournamentStatus::Registration.to_string(), "Registration");
        assert_eq!(TournamentStatus::InProgress.to_string(), "InProgress");
        assert_eq!(TournamentStatus::Completed.to_string(), "Completed");
        assert_eq!(TournamentStatus::Cancelled.to_string(), "Cancelled");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 8. MatchConfig::auto topology selection.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn match_config_auto_single_room_for_small_count() {
        use magnetite_sdk::authority::{MatchConfig, Topology};
        let cfg = MatchConfig::auto(4);
        assert!(
            matches!(cfg.topology, Topology::SingleRoom),
            "≤16 players → SingleRoom"
        );
        assert_eq!(cfg.max_players, 4);
    }

    #[test]
    fn match_config_auto_dedicated_for_medium_count() {
        use magnetite_sdk::authority::{MatchConfig, Topology};
        let cfg = MatchConfig::auto(64);
        assert!(
            matches!(cfg.topology, Topology::Dedicated { .. }),
            "17–256 players → Dedicated"
        );
    }

    #[test]
    fn match_config_auto_sharded_for_large_count() {
        use magnetite_sdk::authority::{MatchConfig, Topology};
        let cfg = MatchConfig::auto(1000);
        assert!(
            matches!(cfg.topology, Topology::Sharded { .. }),
            ">256 players → Sharded"
        );
    }
}
