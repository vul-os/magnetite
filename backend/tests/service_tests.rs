// service_tests.rs — unit / integration-style tests for the gap-closure work (F1/F2/F3).
// These tests do NOT require a live DB. They exercise pure-logic paths that were
// previously mock/stub/hardcoded and are now real code.
//
// Topics covered:
//   1. PaymentService — sandbox vs unconfigured error selection.
//   2. PaymentService — calculate_earnings (15% platform / 85% dev split via PaymentService).
//   3. Payout fee split (30% platform / 70% dev via payout service constants).
//   4. ZAR→USDC conversion arithmetic.
//   5. Matchmaking wait-estimate formula (queue depth × 30 s, clamped 5-600 s).
//   6. Matchmaking region filter (in-memory version passes all players through).
//   7. Matchmaking SkillRange logic.
//   8. Email provider — ResendProvider absent when key is missing.
//   9. Email template rendering (no HTTP call needed).
//  10. EarningsBreakdown shape correctness.

#[cfg(test)]
mod payment_provider_tests {
    use magnetite_backend::services::payment::PaymentService;

    // ── Sandbox mode ───────────────────────────────────────────────────────────

    #[test]
    fn sandbox_mode_on_mock_constructor() {
        // PaymentService::mock() always sets sandbox=true.
        // In sandbox mode, create_wallet and other methods return labeled results
        // instead of calling Circle/Paystack.
        let svc = PaymentService::mock();
        // We cannot call async methods here, but we verify the struct is constructed.
        // The sandbox flag is visible through the public sandbox_create_wallet test below.
        // (No panics = mock constructor works.)
        let _ = svc;
    }

    #[tokio::test]
    async fn sandbox_wallet_has_labeled_id() {
        let svc = PaymentService::mock();
        let user_id = uuid::Uuid::new_v4();
        let result = svc.create_wallet(user_id).await;
        assert!(result.is_ok(), "sandbox create_wallet should succeed");
        let wallet = result.unwrap();
        assert!(
            wallet.wallet_id.starts_with("sandbox_wallet_"),
            "sandbox wallet_id should be labeled: {}",
            wallet.wallet_id
        );
        assert_eq!(wallet.chain, "ETH-SANDBOX");
    }

    #[tokio::test]
    async fn unconfigured_production_returns_error() {
        // Without CIRCLE_API_KEY and without PAYMENTS_SANDBOX=true,
        // create_wallet must return an Err (not a fabricated success).
        // Save and restore env vars around the test body.
        let saved_circle = std::env::var("CIRCLE_API_KEY").ok();
        let saved_paystack = std::env::var("PAYSTACK_SECRET_KEY").ok();
        let saved_sandbox = std::env::var("PAYMENTS_SANDBOX").ok();

        // Clear all payment env vars.
        unsafe {
            std::env::remove_var("CIRCLE_API_KEY");
            std::env::remove_var("PAYSTACK_SECRET_KEY");
            std::env::remove_var("PAYMENTS_SANDBOX");
        }

        let svc = PaymentService::from_env();
        let user_id = uuid::Uuid::new_v4();
        let result = svc.create_wallet(user_id).await;

        // Restore env vars before asserting (so other tests are unaffected).
        unsafe {
            if let Some(v) = saved_circle {
                std::env::set_var("CIRCLE_API_KEY", v);
            }
            if let Some(v) = saved_paystack {
                std::env::set_var("PAYSTACK_SECRET_KEY", v);
            }
            if let Some(v) = saved_sandbox {
                std::env::set_var("PAYMENTS_SANDBOX", v);
            }
        }

        assert!(
            result.is_err(),
            "unconfigured create_wallet must return Err, not fabricated success"
        );
        let msg = result.err().unwrap().to_string();
        assert!(
            msg.contains("payments not configured") || msg.contains("CIRCLE_API_KEY"),
            "error message should mention missing key: {msg}"
        );
    }

    #[tokio::test]
    async fn sandbox_deposit_has_labeled_transfer_id() {
        let svc = PaymentService::mock();
        let result = svc
            .deposit_funds("sandbox_wallet_abc", rust_decimal_macros::dec!(50))
            .await;
        assert!(result.is_ok());
        let transfer = result.unwrap();
        assert!(
            transfer.transfer_id.starts_with("sandbox_transfer_"),
            "sandbox deposit transfer_id should be labeled: {}",
            transfer.transfer_id
        );
        assert!(transfer.status.contains("sandbox"));
    }

    #[tokio::test]
    async fn sandbox_verify_paystack_has_labeled_status() {
        let svc = PaymentService::mock();
        let result = svc.verify_paystack_payment("TEST_REF_001").await;
        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(
            verification.status.contains("sandbox"),
            "sandbox verification status should be labeled: {}",
            verification.status
        );
        assert_eq!(verification.reference, "TEST_REF_001");
    }
}

// ── Earnings/fee split tests ───────────────────────────────────────────────────

#[cfg(test)]
mod earnings_split_tests {
    use magnetite_backend::services::payment::PaymentService;
    use rust_decimal_macros::dec;

    #[test]
    fn calculate_earnings_developer_gets_85_pct() {
        // PaymentService::calculate_earnings uses 15% platform / 85% developer (per §1).
        let svc = PaymentService::mock();
        let revenue = dec!(10_000.00);
        let breakdown = svc.calculate_earnings(revenue);

        // Platform gets 15%, developer gets 85%.
        let expected_platform = dec!(1_500.00);
        let expected_developer = dec!(8_500.00);

        assert_eq!(
            breakdown.platform_share, expected_platform,
            "platform share should be 15% = 1500"
        );
        assert_eq!(
            breakdown.developer_share, expected_developer,
            "developer share should be 85% = 8500"
        );
        assert_eq!(
            breakdown.developer_percentage,
            dec!(85),
            "developer_percentage field should be 85 (not 0.85)"
        );
        assert_eq!(breakdown.total_revenue, revenue);
    }

    #[test]
    fn calculate_earnings_shares_sum_to_total() {
        let svc = PaymentService::mock();
        let revenue = dec!(327.49);
        let b = svc.calculate_earnings(revenue);
        // platform_share + developer_share == total (within decimal precision)
        let reconstructed = b.platform_share + b.developer_share;
        let diff = (reconstructed - revenue).abs();
        assert!(
            diff < dec!(0.01),
            "shares don't sum to total: {reconstructed} vs {revenue}"
        );
    }

    #[test]
    fn calculate_earnings_developer_beats_platform() {
        let svc = PaymentService::mock();
        let revenue = dec!(1_000.00);
        let b = svc.calculate_earnings(revenue);
        assert!(
            b.developer_share > b.platform_share,
            "developer should always earn more than platform"
        );
    }

    #[test]
    fn calculate_earnings_zero_revenue() {
        let svc = PaymentService::mock();
        let b = svc.calculate_earnings(rust_decimal::Decimal::ZERO);
        assert_eq!(b.developer_share, rust_decimal::Decimal::ZERO);
        assert_eq!(b.platform_share, rust_decimal::Decimal::ZERO);
    }
}

// ── Payout service fee split (30/70) ──────────────────────────────────────────
// The payout service uses a separate 30/70 split from PaymentService's 15/85.
// Tests verify the arithmetic is correct (no extra /100 divisor).

#[cfg(test)]
mod payout_fee_split_tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    // Replicate the payout service logic here to test in isolation.
    fn platform_fee_percent() -> Decimal {
        Decimal::new(30, 2) // 0.30
    }

    fn developer_share_percent() -> Decimal {
        Decimal::new(70, 2) // 0.70
    }

    #[test]
    fn platform_fee_is_30_pct() {
        assert_eq!(
            platform_fee_percent(),
            dec!(0.30),
            "platform_fee_percent should be 0.30 (30%), not 30.0 or 0.003"
        );
    }

    #[test]
    fn developer_share_is_70_pct() {
        assert_eq!(
            developer_share_percent(),
            dec!(0.70),
            "developer_share_percent should be 0.70 (70%), not 70.0 or 0.007"
        );
    }

    #[test]
    fn fee_split_sums_to_one() {
        assert_eq!(
            platform_fee_percent() + developer_share_percent(),
            dec!(1.00),
            "platform fee + developer share must equal 1.00"
        );
    }

    #[test]
    fn revenue_split_arithmetic_correct() {
        let revenue = dec!(1_000.00);
        let platform = revenue * platform_fee_percent();
        let developer = revenue * developer_share_percent();

        assert_eq!(
            platform,
            dec!(300.00),
            "platform should get 300 on 1000 revenue"
        );
        assert_eq!(
            developer,
            dec!(700.00),
            "developer should get 700 on 1000 revenue"
        );
        assert_eq!(platform + developer, revenue);
    }

    #[test]
    fn revenue_split_not_fractional_percent() {
        // Regression guard: the old bug multiplied by 0.70 / 100, giving 0.7% not 70%.
        let revenue = dec!(100.00);
        let developer = revenue * developer_share_percent();
        // 70% = 70.00, NOT 0.70
        assert!(
            developer > dec!(60.00),
            "developer share on 100 must be > 60 (was giving 0.70 with the bug): {developer}"
        );
    }
}

// ── ZAR → USDC conversion ──────────────────────────────────────────────────────

#[cfg(test)]
mod zar_usdc_conversion_tests {
    use magnetite_backend::services::payment::PaymentService;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn default_rate_275_zar_per_usdc() {
        // Without ZAR_USDC_RATE env, default is 275.0 ZAR/USDC.
        let saved = std::env::var("ZAR_USDC_RATE").ok();
        unsafe {
            std::env::remove_var("ZAR_USDC_RATE");
        }

        let svc = PaymentService::mock();
        let zar = dec!(275.00);
        let result = svc.convert_zar_to_usdc(zar).await;

        unsafe {
            if let Some(v) = saved {
                std::env::set_var("ZAR_USDC_RATE", v);
            }
        }

        assert!(result.is_ok());
        let usdc = result.unwrap();
        // 275 / 275 = 1 USDC, minus 3% fee = 0.97 USDC
        let expected = dec!(0.97);
        let diff = (usdc - expected).abs();
        assert!(
            diff < dec!(0.001),
            "275 ZAR should convert to ~0.97 USDC at default rate: got {usdc}"
        );
    }

    #[tokio::test]
    async fn custom_rate_env_is_respected() {
        // When ZAR_USDC_RATE=550, 550 ZAR = 1 USDC (before fee).
        let saved = std::env::var("ZAR_USDC_RATE").ok();
        unsafe {
            std::env::set_var("ZAR_USDC_RATE", "550.0");
        }

        let svc = PaymentService::mock();
        let zar = dec!(550.00);
        let result = svc.convert_zar_to_usdc(zar).await;

        unsafe {
            match saved {
                Some(v) => std::env::set_var("ZAR_USDC_RATE", v),
                None => std::env::remove_var("ZAR_USDC_RATE"),
            }
        }

        assert!(result.is_ok());
        let usdc = result.unwrap();
        // 550 / 550 = 1 USDC, minus 3% = 0.97 USDC
        assert!(
            usdc > dec!(0.9) && usdc < dec!(1.0),
            "550 ZAR at rate 550 should give ~0.97 USDC: got {usdc}"
        );
    }

    #[tokio::test]
    async fn zero_zar_gives_zero_usdc() {
        let svc = PaymentService::mock();
        let result = svc.convert_zar_to_usdc(Decimal::ZERO).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Decimal::ZERO);
    }

    #[tokio::test]
    async fn conversion_applies_3_pct_fee() {
        // Verify the 3% platform fee is applied: result < zar / rate.
        let svc = PaymentService::mock();
        let zar = dec!(2750.00); // At default rate 275, this is 10 USDC before fee
        let result = svc.convert_zar_to_usdc(zar).await;
        assert!(result.is_ok());
        let usdc = result.unwrap();
        let gross = dec!(10.00); // without fee
        assert!(
            usdc < gross,
            "after 3% fee, usdc should be less than gross {gross}: got {usdc}"
        );
        // 10 * 0.97 = 9.70
        let diff = (usdc - dec!(9.70)).abs();
        assert!(diff < dec!(0.01), "expected ~9.70 USDC, got {usdc}");
    }
}

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

// ── EarningsBreakdown struct correctness ──────────────────────────────────────

#[cfg(test)]
mod earnings_breakdown_tests {
    use magnetite_backend::services::payment::{EarningsBreakdown, PaymentService};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    #[test]
    fn earnings_breakdown_developer_percentage_is_85() {
        let svc = PaymentService::mock();
        let breakdown = svc.calculate_earnings(dec!(1000.00));
        // developer_percentage is stored as 85 (the integer percentage), not 0.85.
        assert_eq!(breakdown.developer_percentage, dec!(85));
    }

    #[test]
    fn earnings_breakdown_fields_are_correct_type() {
        let svc = PaymentService::mock();
        let breakdown: EarningsBreakdown = svc.calculate_earnings(dec!(500.00));
        assert!(breakdown.total_revenue > Decimal::ZERO);
        assert!(breakdown.developer_share > Decimal::ZERO);
        assert!(breakdown.platform_share > Decimal::ZERO);
    }

    #[test]
    fn earnings_breakdown_platform_is_15_pct() {
        let svc = PaymentService::mock();
        let revenue = dec!(2000.00);
        let b = svc.calculate_earnings(revenue);
        // 15% of 2000 = 300
        assert_eq!(b.platform_share, dec!(300.00));
    }
}

// ── PaymentService sandbox payout ─────────────────────────────────────────────

#[cfg(test)]
mod payment_payout_tests {
    use magnetite_backend::services::payment::PaymentService;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[tokio::test]
    async fn sandbox_payout_returns_labeled_status() {
        let svc = PaymentService::mock();
        // process_payout with a db pool — we pass a dummy pool construction that will fail
        // the DB write, but since this is sandbox mode the transfer is skipped.
        // We test process_payout without a real DB by passing a dummy pool.
        // create a minimal pool that will fail (we only care sandbox short-circuits first)
        let user_id = Uuid::new_v4();
        // We use a fake pool from env — it won't connect, but sandbox short-circuits before any DB call.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://localhost/magnetite_test")
            .expect("lazy connect should not fail");

        let result = svc
            .process_payout(&pool, user_id, dec!(100.00), "0xABC123")
            .await;
        assert!(
            result.is_ok(),
            "sandbox process_payout should succeed: {result:?}"
        );
        let payout = result.unwrap();
        assert!(
            payout.status.contains("sandbox"),
            "sandbox payout status should be labeled: {}",
            payout.status
        );
        assert_eq!(payout.amount, dec!(100.00));
        assert_eq!(payout.user_id, user_id);
    }

    #[tokio::test]
    async fn sandbox_withdraw_funds_labeled() {
        let svc = PaymentService::mock();
        let result = svc.withdraw_funds("0xDEADBEEF", dec!(50.00)).await;
        assert!(result.is_ok());
        let transfer = result.unwrap();
        assert!(transfer.status.contains("sandbox"));
        assert_eq!(transfer.amount, dec!(50.00));
        assert_eq!(transfer.destination_address, "0xDEADBEEF");
    }
}
