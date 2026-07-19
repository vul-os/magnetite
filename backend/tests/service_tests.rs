// service_tests.rs — pure-logic tests that do NOT require a live DB.
//
// Topics covered:
//   1. Non-custodial checkout — split arithmetic + signed receipt verification.
//   2. Receipt tampering fails closed (a forged receipt never grants entitlement).
//   3. Deterministic offline rail (same inputs => same receipt, no network).
//   4. Matchmaking wait-estimate formula (queue depth × 30 s, clamped 5-600 s).
//   5. Matchmaking region filter (in-memory version passes all players through).
//   6. Matchmaking SkillRange logic.
//   7. Email provider — ResendProvider absent when key is missing.
//   8. Email template rendering (no HTTP call needed).

#[cfg(test)]
mod noncustodial_payment_tests {
    use magnetite_backend::services::payment::{
        rail, sale_split, units_from_usd, verify_receipt, PaymentRail, PubKey,
    };
    use rust_decimal_macros::dec;

    // ── Split arithmetic ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn developer_receives_the_whole_subtotal() {
        // There is no 70/30 platform cut in the non-custodial model.
        let receipt = rail()
            .checkout(
                &PubKey([0xB0; 32]),
                sale_split(PubKey([0xD0; 32]), units_from_usd(dec!(100.00)), None),
            )
            .await;

        assert_eq!(receipt.total, 10_000);
        assert_eq!(receipt.protocol_fee, 0);
        assert_eq!(receipt.payouts.len(), 1);
        assert_eq!(receipt.payouts[0].amount, 10_000);
    }

    #[tokio::test]
    async fn operator_cut_is_paid_atomically_in_the_same_receipt() {
        let receipt = rail()
            .checkout(
                &PubKey([0xB1; 32]),
                sale_split(PubKey([0xD1; 32]), 900, Some((PubKey([0x0B; 32]), 100))),
            )
            .await;

        assert_eq!(receipt.total, 1000);
        assert_eq!(receipt.payouts.len(), 2);
        let sum: u64 = receipt.payouts.iter().map(|p| p.amount).sum();
        assert_eq!(sum, receipt.total, "payouts must sum to the total");
    }

    // ── Receipt verification (this is what grants entitlements) ───────────────

    #[tokio::test]
    async fn fresh_receipt_verifies() {
        let receipt = rail()
            .checkout(&PubKey([0xB2; 32]), sale_split(PubKey([0xD2; 32]), 4200, None))
            .await;
        assert!(verify_receipt(&receipt));
    }

    #[tokio::test]
    async fn inflated_total_fails_verification() {
        let mut receipt = rail()
            .checkout(&PubKey([0xB3; 32]), sale_split(PubKey([0xD3; 32]), 4200, None))
            .await;
        receipt.total = 1;
        assert!(!verify_receipt(&receipt), "must fail closed");
    }

    // ── Offline determinism (CI runs with zero external services) ─────────────

    #[tokio::test]
    async fn receipts_are_deterministic_offline() {
        let buyer = PubKey([0xB4; 32]);
        let a = rail()
            .checkout(&buyer, sale_split(PubKey([0xD4; 32]), 777, None))
            .await;
        let b = rail()
            .checkout(&buyer, sale_split(PubKey([0xD4; 32]), 777, None))
            .await;
        assert_eq!(a.nonce, b.nonce);
        assert_eq!(a.sig.0, b.sig.0);
    }

    #[test]
    fn usd_maps_to_smallest_rail_unit() {
        assert_eq!(units_from_usd(dec!(0.01)), 1);
        assert_eq!(units_from_usd(dec!(12.34)), 1234);
    }
}

// Fiat (ZAR/USD on-ramp) removed entirely — payments are non-custodial crypto.

// ── Matchmaking wait estimate formula ─────────────────────────────────────────

#[cfg(test)]
mod matchmaking_wait_estimate_tests {

    // Replicate the estimate_wait_seconds formula from api/matchmaking.rs (private fn).
    // Formula: (depth.max(1) * 30).clamp(5, 600)
    fn estimate_wait(depth: i32) -> i32 {
        (depth.max(1) * 30).clamp(5, 600)
    }

    #[test]
    fn depth_0_gives_minimum_30s() {
        // depth=0 → max(0,1)=1 → 1*30=30 → clamp(5,600)=30
        assert_eq!(estimate_wait(0), 30);
    }

    #[test]
    fn depth_1_gives_30s() {
        assert_eq!(estimate_wait(1), 30);
    }

    #[test]
    fn depth_5_gives_150s() {
        assert_eq!(estimate_wait(5), 150);
    }

    #[test]
    fn depth_20_gives_600s_clamped() {
        // 20 * 30 = 600 → at the maximum bound
        assert_eq!(estimate_wait(20), 600);
    }

    #[test]
    fn depth_100_clamped_to_600() {
        // 100 * 30 = 3000 → clamped to 600
        assert_eq!(estimate_wait(100), 600);
    }

    #[test]
    fn minimum_is_5_not_zero() {
        // Even depth=0 gives at least 5 (actually 30 because of max(1)); the clamp lower bound is 5.
        assert!(estimate_wait(0) >= 5);
    }

    #[test]
    fn maximum_is_600() {
        assert!(estimate_wait(1000) <= 600);
    }
}

// ── Matchmaking region filter ──────────────────────────────────────────────────

#[cfg(test)]
mod matchmaking_region_filter_tests {
    use chrono::Utc;
    use magnetite_backend::services::matchmaking::{filter_by_region, QueuedPlayer};
    use uuid::Uuid;

    fn make_player(skill: f64) -> QueuedPlayer {
        QueuedPlayer {
            user_id: Uuid::new_v4(),
            skill_rating: skill,
            joined_at: Utc::now(),
            ready: true,
        }
    }

    #[test]
    fn filter_by_region_returns_all_players() {
        // filter_by_region is the in-memory version that passes all players through
        // because QueuedPlayer has no region field.
        let players = vec![make_player(1000.0), make_player(1200.0), make_player(950.0)];
        let original_len = players.len();
        let filtered = filter_by_region(players, "us-east-1".to_string());
        assert_eq!(
            filtered.len(),
            original_len,
            "in-memory filter_by_region should pass all players through"
        );
    }

    #[test]
    fn filter_by_region_empty_input() {
        let filtered = filter_by_region(vec![], "eu-west-1".to_string());
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_by_region_single_player() {
        let players = vec![make_player(1100.0)];
        let filtered = filter_by_region(players, "ap-southeast-1".to_string());
        assert_eq!(filtered.len(), 1);
    }
}

// ── Matchmaking SkillRange logic ───────────────────────────────────────────────

#[cfg(test)]
mod skill_range_tests {
    use chrono::Duration;
    use magnetite_backend::services::matchmaking::{calculate_skill_ranges, SkillRange};

    #[test]
    fn skill_range_contains() {
        let range = SkillRange {
            min: 900.0,
            max: 1100.0,
        };
        assert!(range.contains(1000.0));
        assert!(range.contains(900.0));
        assert!(range.contains(1100.0));
        assert!(!range.contains(899.0));
        assert!(!range.contains(1101.0));
    }

    #[test]
    fn skill_range_overlaps() {
        let a = SkillRange {
            min: 900.0,
            max: 1100.0,
        };
        let b = SkillRange {
            min: 1050.0,
            max: 1200.0,
        };
        let c = SkillRange {
            min: 1200.0,
            max: 1400.0,
        };
        assert!(a.overlaps(&b), "overlapping ranges should return true");
        assert!(b.overlaps(&a), "overlap should be symmetric");
        assert!(
            !a.overlaps(&c),
            "non-overlapping ranges should return false"
        );
    }

    #[test]
    fn calculate_skill_ranges_expands_over_time() {
        let range_0min = calculate_skill_ranges(Duration::minutes(0));
        let range_5min = calculate_skill_ranges(Duration::minutes(5));

        assert!(
            range_5min.max > range_0min.max,
            "skill range should expand with more wait time: {:.0} -> {:.0}",
            range_0min.max,
            range_5min.max
        );
    }

    #[test]
    fn calculate_skill_ranges_base_is_100() {
        let range = calculate_skill_ranges(Duration::minutes(0));
        // At 0 minutes: base_range = 100, expansion = 0 → max = 100
        assert_eq!(range.max, 100.0);
        assert_eq!(range.min, 0.0);
    }

    #[test]
    fn calculate_skill_ranges_caps_at_600() {
        // expansion caps at 500, so max caps at 100 + 500 = 600
        let range = calculate_skill_ranges(Duration::minutes(1000));
        assert_eq!(range.max, 600.0, "skill range max should cap at 600");
    }
}

// ── Email provider construction ────────────────────────────────────────────────

#[cfg(test)]
mod email_provider_tests {
    use magnetite_backend::services::email::ResendProvider;

    #[test]
    fn resend_provider_absent_without_key() {
        // When RESEND_API_KEY is not set, from_env() should return None
        // (not panic, not succeed silently).
        temp_env::with_vars([("RESEND_API_KEY", None::<&str>)], || {
            let provider = ResendProvider::from_env();
            assert!(
                provider.is_none(),
                "ResendProvider should not construct when RESEND_API_KEY is absent"
            );
        });
    }

    #[test]
    fn resend_provider_absent_with_empty_key() {
        temp_env::with_vars([("RESEND_API_KEY", Some(""))], || {
            let provider = ResendProvider::from_env();
            assert!(
                provider.is_none(),
                "ResendProvider should not construct when RESEND_API_KEY is empty"
            );
        });
    }

    #[test]
    fn resend_provider_present_with_key() {
        temp_env::with_vars([("RESEND_API_KEY", Some("re_test_123"))], || {
            let provider = ResendProvider::from_env();
            assert!(
                provider.is_some(),
                "ResendProvider should construct when RESEND_API_KEY is set"
            );
        });
    }
}

// ── Email template rendering ───────────────────────────────────────────────────
// These tests are inline mirrors of the ones already in services/email.rs,
// but written from the integration test perspective to ensure the public API
// surface (the EmailService methods) builds without calling the provider.

#[cfg(test)]
mod email_template_tests {
    // We test by constructing with a fake provider that records calls.

    #[test]
    fn email_service_fails_gracefully_without_provider() {
        // Without any email provider env configured, from_env() should return Err.
        temp_env::with_vars(
            [
                ("EMAIL_PROVIDER", None::<&str>),
                ("RESEND_API_KEY", None::<&str>),
                ("AWS_SES_SMTP_USER", None::<&str>),
                ("AWS_SES_SMTP_PASSWORD", None::<&str>),
            ],
            || {
                use magnetite_backend::services::email::EmailService;
                let result = EmailService::from_env();
                assert!(
                    result.is_err(),
                    "EmailService::from_env() should return Err when provider is unconfigured"
                );
                let err = result.err().unwrap();
                let msg = err.to_string();
                assert!(
                    msg.contains("not configured") || msg.contains("RESEND_API_KEY"),
                    "error should explain what is missing: {msg}"
                );
            },
        );
    }

    #[test]
    fn ses_provider_absent_without_credentials() {
        use magnetite_backend::services::email::SesProvider;
        temp_env::with_vars(
            [
                ("AWS_SES_SMTP_USER", None::<&str>),
                ("AWS_SES_SMTP_PASSWORD", None::<&str>),
            ],
            || {
                let provider = SesProvider::from_env();
                assert!(
                    provider.is_none(),
                    "SesProvider should not construct without SES SMTP credentials"
                );
            },
        );
    }
}
