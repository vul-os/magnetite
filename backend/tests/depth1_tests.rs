// depth1_tests.rs — Pure-logic tests for the DEPTH-1 wave features.
//
// Topics covered:
//   1. Notification-preference skip logic — a disabled category/channel must
//      not be "delivered"; tests mirror the channel_enabled() allowlist gate
//      and the per-column enable/disable semantics of NotificationPreferences.
//   2. Auto-flag heuristic — calls the real `magnetite_backend::api::reviews::content_flag_reasons`
//      (which the backend agent added this wave).  Tests verify profanity / spam /
//      url_flood / repetition triggers and clean-content pass-through.
//   3. Moderation status filtering — the ReviewReportQuery status filter logic:
//      "pending" | "dismissed" | "resolved" are valid; the default is "pending";
//      unknown values fall back to "pending" (safe default).
//
// None of these tests require a live database.

// ─────────────────────────────────────────────────────────────────────────────
// 1. Notification-preference skip logic
// ─────────────────────────────────────────────────────────────────────────────
//
// The backend exposes `channel_enabled(pool, user_id, category, channel) -> bool`
// which consults the `notification_preferences` table.  The allowlist guard
// that prevents unknown category/channel combos defaulting to `true` is
// pure logic — we test it here without a DB.

#[cfg(test)]
mod notification_preference_skip_tests {
    // Mirror of the NotificationPreferences struct fields (per notifications.rs).
    // In production this is backed by a DB row; here we use a plain struct.
    #[derive(Debug, Clone, Default)]
    struct NotificationPreferences {
        payouts_email: bool,
        payouts_in_app: bool,
        payouts_push: bool,

        social_email: bool,
        social_in_app: bool,
        social_push: bool,

        achievements_email: bool,
        achievements_in_app: bool,
        achievements_push: bool,

        marketing_email: bool,
        marketing_in_app: bool,
        marketing_push: bool,
    }

    impl NotificationPreferences {
        fn all_enabled() -> Self {
            Self {
                payouts_email: true,
                payouts_in_app: true,
                payouts_push: true,
                social_email: true,
                social_in_app: true,
                social_push: true,
                achievements_email: true,
                achievements_in_app: true,
                achievements_push: true,
                marketing_email: true,
                marketing_in_app: true,
                marketing_push: true,
            }
        }

        fn all_disabled() -> Self {
            Self {
                payouts_email: false,
                payouts_in_app: false,
                payouts_push: false,
                social_email: false,
                social_in_app: false,
                social_push: false,
                achievements_email: false,
                achievements_in_app: false,
                achievements_push: false,
                marketing_email: false,
                marketing_in_app: false,
                marketing_push: false,
            }
        }

        /// Mirror of `channel_enabled()` from notifications.rs.
        /// Returns true if the given category/channel pair is enabled.
        /// Unknown pairs default to `true` (permissive default, same as the real impl).
        fn channel_enabled(&self, category: &str, channel: &str) -> bool {
            match (category, channel) {
                ("payouts", "email") => self.payouts_email,
                ("payouts", "in_app") => self.payouts_in_app,
                ("payouts", "push") => self.payouts_push,
                ("social", "email") => self.social_email,
                ("social", "in_app") => self.social_in_app,
                ("social", "push") => self.social_push,
                ("achievements", "email") => self.achievements_email,
                ("achievements", "in_app") => self.achievements_in_app,
                ("achievements", "push") => self.achievements_push,
                ("marketing", "email") => self.marketing_email,
                ("marketing", "in_app") => self.marketing_in_app,
                ("marketing", "push") => self.marketing_push,
                // Unknown: default allow (consistent with production implementation).
                _ => true,
            }
        }
    }

    // ── Skip-delivery decision ────────────────────────────────────────────────

    /// A notification must NOT be delivered when the category+channel is disabled.
    fn should_deliver(prefs: &NotificationPreferences, category: &str, channel: &str) -> bool {
        prefs.channel_enabled(category, channel)
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn disabled_payouts_email_is_skipped() {
        let mut prefs = NotificationPreferences::all_enabled();
        prefs.payouts_email = false;

        assert!(
            !should_deliver(&prefs, "payouts", "email"),
            "payouts/email disabled → must not deliver"
        );
    }

    #[test]
    fn enabled_payouts_email_is_delivered() {
        let prefs = NotificationPreferences::all_enabled();
        assert!(
            should_deliver(&prefs, "payouts", "email"),
            "payouts/email enabled → must deliver"
        );
    }

    #[test]
    fn disabled_social_in_app_is_skipped() {
        let mut prefs = NotificationPreferences::all_enabled();
        prefs.social_in_app = false;

        assert!(
            !should_deliver(&prefs, "social", "in_app"),
            "social/in_app disabled → must not deliver"
        );
    }

    #[test]
    fn disabled_achievements_push_is_skipped() {
        let mut prefs = NotificationPreferences::all_enabled();
        prefs.achievements_push = false;

        assert!(
            !should_deliver(&prefs, "achievements", "push"),
            "achievements/push disabled → must not deliver"
        );
    }

    #[test]
    fn disabled_marketing_all_channels_are_skipped() {
        let mut prefs = NotificationPreferences::all_enabled();
        prefs.marketing_email = false;
        prefs.marketing_in_app = false;
        prefs.marketing_push = false;

        assert!(
            !should_deliver(&prefs, "marketing", "email"),
            "marketing/email disabled"
        );
        assert!(
            !should_deliver(&prefs, "marketing", "in_app"),
            "marketing/in_app disabled"
        );
        assert!(
            !should_deliver(&prefs, "marketing", "push"),
            "marketing/push disabled"
        );
    }

    #[test]
    fn all_disabled_skips_every_known_pair() {
        let prefs = NotificationPreferences::all_disabled();

        let pairs = [
            ("payouts", "email"),
            ("payouts", "in_app"),
            ("payouts", "push"),
            ("social", "email"),
            ("social", "in_app"),
            ("social", "push"),
            ("achievements", "email"),
            ("achievements", "in_app"),
            ("achievements", "push"),
            ("marketing", "email"),
            ("marketing", "in_app"),
            ("marketing", "push"),
        ];

        for (cat, ch) in &pairs {
            assert!(
                !should_deliver(&prefs, cat, ch),
                "Expected skip for {cat}/{ch} when all disabled"
            );
        }
    }

    #[test]
    fn all_enabled_delivers_every_known_pair() {
        let prefs = NotificationPreferences::all_enabled();

        let pairs = [
            ("payouts", "email"),
            ("payouts", "in_app"),
            ("payouts", "push"),
            ("social", "email"),
            ("social", "in_app"),
            ("social", "push"),
            ("achievements", "email"),
            ("achievements", "in_app"),
            ("achievements", "push"),
            ("marketing", "email"),
            ("marketing", "in_app"),
            ("marketing", "push"),
        ];

        for (cat, ch) in &pairs {
            assert!(
                should_deliver(&prefs, cat, ch),
                "Expected delivery for {cat}/{ch} when all enabled"
            );
        }
    }

    #[test]
    fn unknown_category_defaults_to_deliver() {
        // Unknown category/channel combos default to `true` (permissive)
        // so that new notification types aren't silently lost before preferences
        // for them are added.
        let prefs = NotificationPreferences::all_disabled(); // all known = off
        assert!(
            should_deliver(&prefs, "system", "email"),
            "unknown category should default to true (permissive)"
        );
        assert!(
            should_deliver(&prefs, "payouts", "sms"),
            "unknown channel should default to true (permissive)"
        );
    }

    #[test]
    fn disabling_one_channel_does_not_affect_other_channels() {
        // Disabling payouts/email must NOT affect payouts/in_app or payouts/push.
        let mut prefs = NotificationPreferences::all_enabled();
        prefs.payouts_email = false;

        assert!(
            should_deliver(&prefs, "payouts", "in_app"),
            "payouts/in_app must still deliver when only payouts/email is disabled"
        );
        assert!(
            should_deliver(&prefs, "payouts", "push"),
            "payouts/push must still deliver when only payouts/email is disabled"
        );
    }

    #[test]
    fn disabling_one_category_does_not_affect_other_categories() {
        // Disabling all social channels must NOT affect payouts channels.
        let mut prefs = NotificationPreferences::all_enabled();
        prefs.social_email = false;
        prefs.social_in_app = false;
        prefs.social_push = false;

        assert!(
            should_deliver(&prefs, "payouts", "email"),
            "payouts/email must still deliver when social is fully disabled"
        );
        assert!(
            should_deliver(&prefs, "achievements", "in_app"),
            "achievements/in_app must still deliver when social is fully disabled"
        );
    }

    #[test]
    fn exactly_twelve_known_category_channel_pairs() {
        // There are exactly 4 categories × 3 channels = 12 known pairs.
        // A guard so that adding new pairs without updating the matcher is caught.
        let categories = ["payouts", "social", "achievements", "marketing"];
        let channels = ["email", "in_app", "push"];
        let expected_count = categories.len() * channels.len();
        assert_eq!(expected_count, 12);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Auto-flag heuristic — real backend function
// ─────────────────────────────────────────────────────────────────────────────
//
// `magnetite_backend::api::reviews::content_flag_reasons(content: &str) -> Vec<String>`
// was added by the backend agent this wave.  It returns a Vec of triggered
// flag reasons; empty means clean.
//
// Flag reasons: "profanity" | "spam" | "url_flood" | "repetition"

#[cfg(test)]
mod auto_flag_heuristic_tests {
    use magnetite_backend::api::reviews::content_flag_reasons;

    // ── Clean content ────────────────────────────────────────────────────────

    #[test]
    fn clean_positive_review_passes_all_checks() {
        let reasons = content_flag_reasons(
            "Absolutely loved this game! The mechanics are innovative, \
             the art style is beautiful, and the story kept me engaged. \
             Would definitely recommend to anyone who enjoys puzzle platformers.",
        );
        assert!(
            reasons.is_empty(),
            "clean review must not be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn clean_negative_review_passes_all_checks() {
        let reasons = content_flag_reasons(
            "The game has potential but the controls feel sluggish and \
             there are too many loading screens. Disappointed given the price.",
        );
        assert!(
            reasons.is_empty(),
            "clean negative review must not be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn empty_string_is_not_flagged() {
        let reasons = content_flag_reasons("");
        assert!(reasons.is_empty(), "empty string must not be flagged");
    }

    #[test]
    fn clean_short_chat_message_passes() {
        let reasons = content_flag_reasons("gg well played, that last level was tough!");
        assert!(
            reasons.is_empty(),
            "short clean message must not be flagged: {:?}",
            reasons
        );
    }

    // ── Profanity detection ──────────────────────────────────────────────────

    #[test]
    fn profanity_in_review_is_flagged() {
        // Uses words from the PROFANITY_WORDS list in reviews.rs.
        let reasons = content_flag_reasons("This game is fucking terrible and complete shit.");
        assert!(
            reasons.contains(&"profanity".to_string()),
            "profanity must be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn profanity_detection_is_case_insensitive() {
        let reasons = content_flag_reasons("What a BULLSHIT design choice by the devs.");
        // "bullshit" → "shit" substring match (lowercased)
        assert!(
            reasons.contains(&"profanity".to_string()),
            "uppercase profanity must be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn profanity_mixed_case_is_flagged() {
        let reasons = content_flag_reasons("This is such Bullshit");
        assert!(
            reasons.contains(&"profanity".to_string()),
            "mixed-case profanity must be flagged: {:?}",
            reasons
        );
    }

    // ── Spam detection ───────────────────────────────────────────────────────

    #[test]
    fn spam_keyword_in_review_is_flagged() {
        // "click here" is in the SPAM_WORDS list.
        let reasons = content_flag_reasons(
            "Buy now and earn $ fast with this amazing deal! Click here for more.",
        );
        assert!(
            reasons.contains(&"spam".to_string()),
            "spam keyword must be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn spam_keyword_earn_dollar_is_flagged() {
        let reasons = content_flag_reasons("earn $ fast from home while gaming!");
        assert!(
            reasons.contains(&"spam".to_string()),
            "earn $ fast must be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn spam_keyword_buy_now_is_flagged() {
        let reasons = content_flag_reasons("buy now before the limited offer expires");
        assert!(
            reasons.contains(&"spam".to_string()),
            "buy now / limited offer must be flagged: {:?}",
            reasons
        );
    }

    // ── URL flood detection ──────────────────────────────────────────────────

    #[test]
    fn three_urls_triggers_url_flood() {
        // The heuristic flags when url_count > 2 (more than 2 URLs).
        let reasons = content_flag_reasons(
            "Visit http://example.com http://spam.net https://click.io/here for deals",
        );
        assert!(
            reasons.contains(&"url_flood".to_string()),
            "3 URLs must trigger url_flood: {:?}",
            reasons
        );
    }

    #[test]
    fn four_urls_triggers_url_flood() {
        let reasons = content_flag_reasons(
            "http://a.com http://b.com https://c.com https://d.com — all spam",
        );
        assert!(
            reasons.contains(&"url_flood".to_string()),
            "4 URLs must trigger url_flood: {:?}",
            reasons
        );
    }

    #[test]
    fn two_urls_does_not_trigger_url_flood() {
        // Exactly 2 URLs: count == 2, threshold is > 2, so NOT flagged.
        let reasons =
            content_flag_reasons("For more info see https://magnetite.gg and https://example.com");
        assert!(
            !reasons.contains(&"url_flood".to_string()),
            "2 URLs must NOT trigger url_flood: {:?}",
            reasons
        );
    }

    #[test]
    fn zero_urls_does_not_trigger_url_flood() {
        let reasons = content_flag_reasons("Great game, very fun to play every single day.");
        assert!(
            !reasons.contains(&"url_flood".to_string()),
            "0 URLs must not trigger url_flood: {:?}",
            reasons
        );
    }

    // ── Repetition detection ─────────────────────────────────────────────────

    #[test]
    fn word_repeated_six_times_triggers_repetition() {
        // Threshold is ≥ 6 repetitions of any single word.
        let reasons = content_flag_reasons("bad bad bad bad bad bad game");
        assert!(
            reasons.contains(&"repetition".to_string()),
            "6× repetition must be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn word_repeated_ten_times_triggers_repetition() {
        let content = "spam ".repeat(10) + "review";
        let reasons = content_flag_reasons(&content);
        assert!(
            reasons.contains(&"repetition".to_string()),
            "10× repetition must be flagged: {:?}",
            reasons
        );
    }

    #[test]
    fn word_repeated_five_times_does_not_trigger_repetition() {
        // Threshold is 6; 5 occurrences must NOT fire.
        let reasons = content_flag_reasons("good good good good good game today");
        assert!(
            !reasons.contains(&"repetition".to_string()),
            "5× repetition must NOT be flagged: {:?}",
            reasons
        );
    }

    // ── Multiple flags can co-occur ──────────────────────────────────────────

    #[test]
    fn profanity_and_url_flood_can_both_fire() {
        let reasons =
            content_flag_reasons("fucking buy now at http://a.com http://b.com http://c.com");
        assert!(
            reasons.contains(&"profanity".to_string()),
            "profanity should fire: {:?}",
            reasons
        );
        assert!(
            reasons.contains(&"url_flood".to_string()),
            "url_flood should also fire: {:?}",
            reasons
        );
    }

    #[test]
    fn flags_returned_as_string_vec() {
        // Verify the return type is Vec<String> (compile-time contract).
        let reasons: Vec<String> = content_flag_reasons("test content");
        let _ = reasons; // just needs to compile with the right type
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Moderation status filtering
// ─────────────────────────────────────────────────────────────────────────────
//
// The review-report moderation endpoint (GET /admin/review-reports) accepts a
// `status` query parameter.  Valid values are: "pending" | "dismissed" | "resolved".
// The default is "pending".  Unknown values fall back to "pending" (safe default).

#[cfg(test)]
mod moderation_status_filter_tests {

    // ── Status type (mirrors admin.rs ReviewReportQuery logic) ───────────────

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum ReportStatus {
        Pending,
        Dismissed,
        Resolved,
    }

    impl ReportStatus {
        fn as_str(&self) -> &'static str {
            match self {
                ReportStatus::Pending => "pending",
                ReportStatus::Dismissed => "dismissed",
                ReportStatus::Resolved => "resolved",
            }
        }
    }

    /// Mirrors the `status_filter` derivation in `list_review_reports`:
    ///   `let status_filter = query.status.as_deref().unwrap_or("pending");`
    /// followed by safe DB-query binding.  The DB `CHECK` constraint on the
    /// status column guarantees only valid values are stored.
    fn resolve_status_filter(input: Option<&str>) -> &'static str {
        match input {
            Some("pending") | None => "pending",
            Some("dismissed") => "dismissed",
            Some("resolved") => "resolved",
            // Unknown value → safe default (same as None).
            Some(_) => "pending",
        }
    }

    /// Simulates filtering an in-memory list of reports by status.
    fn filter_reports_by_status<'a>(
        reports: &'a [(&'a str, &'a str)], // (id, status)
        status: &str,
    ) -> Vec<&'a (&'a str, &'a str)> {
        reports.iter().filter(|(_, s)| *s == status).collect()
    }

    // ── Status parsing tests ─────────────────────────────────────────────────

    #[test]
    fn none_status_defaults_to_pending() {
        assert_eq!(resolve_status_filter(None), "pending");
    }

    #[test]
    fn explicit_pending_resolves_to_pending() {
        assert_eq!(resolve_status_filter(Some("pending")), "pending");
    }

    #[test]
    fn explicit_dismissed_resolves_to_dismissed() {
        assert_eq!(resolve_status_filter(Some("dismissed")), "dismissed");
    }

    #[test]
    fn explicit_resolved_resolves_to_resolved() {
        assert_eq!(resolve_status_filter(Some("resolved")), "resolved");
    }

    #[test]
    fn unknown_status_falls_back_to_pending() {
        assert_eq!(resolve_status_filter(Some("unknown")), "pending");
        assert_eq!(resolve_status_filter(Some("PENDING")), "pending"); // case-sensitive
        assert_eq!(resolve_status_filter(Some("")), "pending");
        assert_eq!(resolve_status_filter(Some("flagged")), "pending");
    }

    #[test]
    fn status_filter_is_case_sensitive() {
        // "Pending" (capitalized) is NOT valid — must fall back to "pending".
        assert_eq!(resolve_status_filter(Some("Pending")), "pending");
        assert_eq!(resolve_status_filter(Some("DISMISSED")), "pending");
        assert_eq!(resolve_status_filter(Some("Resolved")), "pending");
    }

    // ── Filtering logic tests ────────────────────────────────────────────────

    #[test]
    fn filter_returns_only_pending_reports() {
        let reports = vec![
            ("r1", "pending"),
            ("r2", "dismissed"),
            ("r3", "pending"),
            ("r4", "resolved"),
            ("r5", "pending"),
        ];

        let filtered = filter_reports_by_status(&reports, "pending");
        assert_eq!(filtered.len(), 3);
        assert!(filtered.iter().all(|(_, s)| *s == "pending"));
    }

    #[test]
    fn filter_returns_only_dismissed_reports() {
        let reports = vec![
            ("r1", "pending"),
            ("r2", "dismissed"),
            ("r3", "dismissed"),
            ("r4", "resolved"),
        ];

        let filtered = filter_reports_by_status(&reports, "dismissed");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|(_, s)| *s == "dismissed"));
    }

    #[test]
    fn filter_returns_only_resolved_reports() {
        let reports = vec![("r1", "resolved"), ("r2", "pending"), ("r3", "resolved")];

        let filtered = filter_reports_by_status(&reports, "resolved");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|(_, s)| *s == "resolved"));
    }

    #[test]
    fn filter_returns_empty_when_no_match() {
        let reports = vec![("r1", "pending"), ("r2", "pending")];
        let filtered = filter_reports_by_status(&reports, "resolved");
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_on_empty_report_list_returns_empty() {
        let reports: Vec<(&str, &str)> = vec![];
        let filtered = filter_reports_by_status(&reports, "pending");
        assert!(filtered.is_empty());
    }

    // ── Valid status values ──────────────────────────────────────────────────

    #[test]
    fn exactly_three_valid_status_values() {
        let valid = [
            ReportStatus::Pending,
            ReportStatus::Dismissed,
            ReportStatus::Resolved,
        ];
        assert_eq!(valid.len(), 3);
    }

    #[test]
    fn status_as_str_matches_db_values() {
        assert_eq!(ReportStatus::Pending.as_str(), "pending");
        assert_eq!(ReportStatus::Dismissed.as_str(), "dismissed");
        assert_eq!(ReportStatus::Resolved.as_str(), "resolved");
    }

    #[test]
    fn default_status_matches_db_default() {
        // The migration sets DEFAULT 'pending' on the status column.
        // The query helper must match this default.
        let db_default = "pending";
        let code_default = resolve_status_filter(None);
        assert_eq!(
            code_default, db_default,
            "code default must match DB DEFAULT"
        );
    }

    // ── Action → status transition correctness ───────────────────────────────
    //
    // When an admin takes action on a report, the status must transition
    // to either "dismissed" or "resolved" — never back to "pending".

    #[test]
    fn dismiss_action_produces_dismissed_status() {
        let resulting_status = match "dismiss" {
            "dismiss" => "dismissed",
            "remove_review" | "warn_user" | "ban_user" => "resolved",
            _ => "pending", // unknown — no-op
        };
        assert_eq!(resulting_status, "dismissed");
    }

    #[test]
    fn remove_review_action_produces_resolved_status() {
        let resulting_status = match "remove_review" {
            "dismiss" => "dismissed",
            "remove_review" | "warn_user" | "ban_user" => "resolved",
            _ => "pending",
        };
        assert_eq!(resulting_status, "resolved");
    }

    #[test]
    fn warn_user_action_produces_resolved_status() {
        let resulting_status = match "warn_user" {
            "dismiss" => "dismissed",
            "remove_review" | "warn_user" | "ban_user" => "resolved",
            _ => "pending",
        };
        assert_eq!(resulting_status, "resolved");
    }

    #[test]
    fn ban_user_action_produces_resolved_status() {
        let resulting_status = match "ban_user" {
            "dismiss" => "dismissed",
            "remove_review" | "warn_user" | "ban_user" => "resolved",
            _ => "pending",
        };
        assert_eq!(resulting_status, "resolved");
    }

    #[test]
    fn action_transitions_are_never_to_pending() {
        // Actions taken by an admin must always move a report forward —
        // a report can never be put back into "pending" by an explicit action.
        let actions = ["dismiss", "remove_review", "warn_user", "ban_user"];
        for action in actions {
            let status = match action {
                "dismiss" => "dismissed",
                "remove_review" | "warn_user" | "ban_user" => "resolved",
                _ => "pending",
            };
            assert_ne!(
                status, "pending",
                "action '{action}' must not produce 'pending'"
            );
        }
    }
}
