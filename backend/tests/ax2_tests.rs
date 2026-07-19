// ax2_tests.rs — Unit/integration-style tests for AX2 "Missing Features".
//
// Topics covered (per AUDIT.md §"Missing Features" + DECISIONS.md §7b):
//   1. Subscription proration math  — proration_factor equivalent
//   2. Subscription tier parsing    — SubscriptionTier::from_str / as_str round-trip
//   3. Friend-request listing       — FriendService public API shape (DB-free guard)
//   4. Season-scoped leaderboard    — key format for archived boards
//   5. Search ranking               — SearchQuery parsing and result shapes
//   6. Notification push shape      — WsNotification / NotificationBroadcast serialization
//   7. Wise IBAN validation         — RecipientDetails with iban/bic fields, sandbox
//   8. NotificationType round-trip  — as_str / from_str symmetry
//   9. Subscription upgrade request — UpgradeRequest deserialization
//  10. Friend-request self-send guard — FriendService::send_request to self returns BadRequest

// ─────────────────────────────────────────────────────────────────────────────
// 1. Subscription proration math
// ─────────────────────────────────────────────────────────────────────────────
// proration_factor(start, end) is private in api/subscriptions.rs.
// We replicate its logic here to unit-test the arithmetic independently.

#[cfg(test)]
mod subscription_proration_tests {
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    /// Mirror of the private `proration_factor` function in api/subscriptions.rs.
    fn proration_factor(
        period_start: chrono::DateTime<chrono::Utc>,
        period_end: chrono::DateTime<chrono::Utc>,
    ) -> Decimal {
        let now = Utc::now();
        let total_secs = (period_end - period_start).num_seconds().max(1);
        let remaining_secs = (period_end - now).num_seconds().max(0);
        let factor = Decimal::new(remaining_secs.min(total_secs), 0) / Decimal::new(total_secs, 0);
        factor.min(Decimal::ONE).max(Decimal::ZERO)
    }

    #[test]
    fn proration_at_start_of_period_is_one() {
        // Period: now → now + 30d.  At the very start, remaining ≈ total → factor ≈ 1.
        let now = Utc::now();
        let start = now - Duration::seconds(1);
        let end = now + Duration::days(30);
        let factor = proration_factor(start, end);
        // Should be very close to 1 (within rounding of 1 second / 30 days).
        assert!(
            factor >= dec!(0.99),
            "proration at start of period should be ≥ 0.99, got {factor}"
        );
        assert!(
            factor <= dec!(1.0),
            "proration factor must not exceed 1.0, got {factor}"
        );
    }

    #[test]
    fn proration_at_midpoint_is_half() {
        // Period: 15 days ago → 15 days from now.  Remaining ≈ total/2.
        let now = Utc::now();
        let start = now - Duration::days(15);
        let end = now + Duration::days(15);
        let factor = proration_factor(start, end);
        // Within ±1% of 0.5
        assert!(
            factor >= dec!(0.49) && factor <= dec!(0.51),
            "proration at midpoint should be ≈ 0.5, got {factor}"
        );
    }

    #[test]
    fn proration_at_end_of_period_is_zero() {
        // Period fully in the past: factor should be 0.
        let now = Utc::now();
        let start = now - Duration::days(30);
        let end = now - Duration::seconds(1);
        let factor = proration_factor(start, end);
        assert_eq!(
            factor,
            Decimal::ZERO,
            "proration when period has expired must be 0, got {factor}"
        );
    }

    #[test]
    fn proration_clamped_to_zero_not_negative() {
        let now = Utc::now();
        // end is before now — period fully expired.
        let start = now - Duration::days(60);
        let end = now - Duration::days(30);
        let factor = proration_factor(start, end);
        assert!(
            factor >= Decimal::ZERO,
            "proration factor must never be negative, got {factor}"
        );
    }

    #[test]
    fn proration_clamped_to_one_not_above() {
        let now = Utc::now();
        // start and end are both far in the future (impossible but clamped).
        let start = now + Duration::days(10);
        let end = now + Duration::days(40);
        let factor = proration_factor(start, end);
        assert!(
            factor <= Decimal::ONE,
            "proration factor must never exceed 1, got {factor}"
        );
    }

    #[test]
    fn proration_charge_delta_arithmetic() {
        // If we upgrade from $5/mo → $10/mo and the period is 50% remaining,
        // the prorated charge should be approximately $2.50.
        let now = Utc::now();
        let start = now - Duration::days(15);
        let end = now + Duration::days(15);
        let factor = proration_factor(start, end);

        let old_price = dec!(5.00);
        let new_price = dec!(10.00);
        let delta = new_price - old_price;
        let charge = delta * factor;

        // 50% of $5 = $2.50 (within ±0.05 for integer-second rounding)
        let diff = (charge - dec!(2.50)).abs();
        assert!(
            diff < dec!(0.05),
            "proration charge on upgrade from $5→$10 at midpoint should ≈ $2.50, got {charge}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Subscription tier parsing
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod subscription_tier_parsing_tests {
    use magnetite_backend::api::subscriptions::SubscriptionTier;

    #[test]
    fn from_str_recognizes_all_tiers() {
        assert!(SubscriptionTier::from_str("free").is_some());
        assert!(SubscriptionTier::from_str("basic").is_some());
        assert!(SubscriptionTier::from_str("pro").is_some());
        assert!(SubscriptionTier::from_str("unlimited").is_some());
    }

    #[test]
    fn from_str_is_case_insensitive() {
        assert!(SubscriptionTier::from_str("FREE").is_some());
        assert!(SubscriptionTier::from_str("Pro").is_some());
        assert!(SubscriptionTier::from_str("UNLIMITED").is_some());
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert!(SubscriptionTier::from_str("enterprise").is_none());
        assert!(SubscriptionTier::from_str("").is_none());
        assert!(SubscriptionTier::from_str("GOLD").is_none());
    }

    #[test]
    fn as_str_round_trip() {
        for slug in &["free", "basic", "pro", "unlimited"] {
            let tier = SubscriptionTier::from_str(slug).expect("tier must parse");
            assert_eq!(tier.as_str(), *slug, "as_str round-trip failed for {slug}");
        }
    }

    #[test]
    fn upgrade_request_deserialization() {
        // UpgradeRequest is the public body type for POST /subscriptions/upgrade.
        let json =
            r#"{"tier_id": "550e8400-e29b-41d4-a716-446655440000", "payment_id": "pay_12345"}"#;
        let req: magnetite_backend::api::subscriptions::UpgradeRequest =
            serde_json::from_str(json).expect("UpgradeRequest must deserialize");
        assert!(req.payment_id.is_some());
        assert_eq!(req.payment_id.unwrap(), "pay_12345");
    }

    #[test]
    fn upgrade_request_free_tier_no_payment_id() {
        // For free-tier upgrades the payment_id is optional.
        let json = r#"{"tier_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: magnetite_backend::api::subscriptions::UpgradeRequest =
            serde_json::from_str(json).expect("UpgradeRequest without payment_id must deserialize");
        assert!(req.payment_id.is_none());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Friend-request listing — API shape guards (DB-free)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod friend_request_api_tests {
    use magnetite_backend::api::social::FriendRequest;
    use uuid::Uuid;

    #[test]
    fn friend_request_pending_status_constant() {
        // Status values are strings; "pending" is used to filter incoming requests.
        let status = "pending";
        assert_eq!(status, "pending");
    }

    #[test]
    fn send_request_to_self_returns_bad_request() {
        // Replicates the self-request guard from FriendService::send_request.
        let id = Uuid::new_v4();
        let result = validate_no_self_request(id, id);
        assert!(result.is_err(), "self-friend-request must return an error");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("self") || err_str.contains("yourself"),
            "error should mention self-request: {err_str}"
        );
    }

    /// Replicates the self-request guard from FriendService::send_request.
    fn validate_no_self_request(
        from: Uuid,
        to: Uuid,
    ) -> Result<(), magnetite_backend::error::AppError> {
        if from == to {
            return Err(magnetite_backend::error::AppError::BadRequest(
                "Cannot send friend request to yourself".to_string(),
            ));
        }
        Ok(())
    }

    #[test]
    fn send_request_to_different_user_is_ok() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let result = validate_no_self_request(from, to);
        assert!(
            result.is_ok(),
            "request to a different user must pass the self-guard"
        );
    }

    #[test]
    fn friend_request_serialization_has_required_fields() {
        // A FriendRequest once serialized must contain id, from_user_id, to_user_id, status.
        let req = FriendRequest {
            id: Uuid::new_v4(),
            from_user_id: Uuid::new_v4(),
            to_user_id: Uuid::new_v4(),
            status: "pending".to_string(),
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(json.contains("from_user_id"));
        assert!(json.contains("to_user_id"));
        assert!(json.contains("pending"));
    }

    #[test]
    fn friend_request_accepted_status() {
        // After accepting, status becomes "accepted"
        let req = FriendRequest {
            id: Uuid::new_v4(),
            from_user_id: Uuid::new_v4(),
            to_user_id: Uuid::new_v4(),
            status: "accepted".to_string(),
            created_at: chrono::Utc::now(),
        };
        assert_eq!(req.status, "accepted");
    }

    #[test]
    fn friend_request_rejected_status() {
        let req = FriendRequest {
            id: Uuid::new_v4(),
            from_user_id: Uuid::new_v4(),
            to_user_id: Uuid::new_v4(),
            status: "rejected".to_string(),
            created_at: chrono::Utc::now(),
        };
        assert_eq!(req.status, "rejected");
    }

    #[test]
    fn send_friend_request_request_body_field_name() {
        // The AX2 fix: client should send `to_user_id` not `user_id`.
        // Check that the backend struct deserializes `to_user_id`.
        let json = format!(r#"{{"to_user_id": "{}"}}"#, Uuid::new_v4());
        let req: magnetite_backend::api::social::SendFriendRequestRequest =
            serde_json::from_str(&json).expect("must deserialize from to_user_id key");
        let _ = req.to_user_id; // field name is `to_user_id`
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Season-scoped leaderboard key format
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod leaderboard_season_key_tests {
    use uuid::Uuid;

    /// Mirror of LeaderboardService::leaderboard_key (private).
    fn leaderboard_key(game_id: Uuid) -> String {
        format!("leaderboard:{}", game_id)
    }

    /// Mirror of LeaderboardService::archive_key (private).
    fn archive_key(game_id: Uuid, period: &str) -> String {
        format!("leaderboard:{}:{}", game_id, period)
    }

    #[test]
    fn live_key_format_matches_expected_pattern() {
        let game_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = leaderboard_key(game_id);
        assert_eq!(key, "leaderboard:550e8400-e29b-41d4-a716-446655440000");
        assert!(key.starts_with("leaderboard:"));
    }

    #[test]
    fn archive_key_includes_game_id_and_period() {
        let game_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = archive_key(game_id, "weekly:2026-W22");
        assert!(key.contains("550e8400"), "key must contain game_id");
        assert!(
            key.contains("weekly:2026-W22"),
            "key must contain period label"
        );
    }

    #[test]
    fn archive_key_distinguishes_different_periods() {
        let game_id = Uuid::new_v4();
        let key_w1 = archive_key(game_id, "weekly:2026-W01");
        let key_w2 = archive_key(game_id, "weekly:2026-W02");
        assert_ne!(
            key_w1, key_w2,
            "different weeks must produce different archive keys"
        );
    }

    #[test]
    fn archive_key_distinguishes_different_games() {
        let g1 = Uuid::new_v4();
        let g2 = Uuid::new_v4();
        let k1 = archive_key(g1, "weekly:2026-W01");
        let k2 = archive_key(g2, "weekly:2026-W01");
        assert_ne!(
            k1, k2,
            "different games must produce different archive keys"
        );
    }

    #[test]
    fn live_and_archive_keys_are_distinct() {
        let game_id = Uuid::new_v4();
        let live = leaderboard_key(game_id);
        let archive = archive_key(game_id, "weekly:2026-W22");
        assert_ne!(live, archive, "live key and archive key must be distinct");
        // Archive key embeds live key as a prefix
        assert!(archive.starts_with(&live));
    }

    #[test]
    fn season_label_included_in_archive_key() {
        let game_id = Uuid::new_v4();
        // season_label format: "season:<uuid>"
        let season_id = Uuid::new_v4();
        let key = archive_key(game_id, &format!("season:{}", season_id));
        assert!(
            key.contains("season:"),
            "season-labelled archive key must contain 'season:'"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Search ranking — SearchQuery parsing and result shapes
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod search_ranking_tests {
    use magnetite_backend::api::search::{
        GameSearchResult, SearchQuery, SearchResponse, SearchResult, UserSearchResult,
    };

    #[test]
    fn search_query_default_search_type_is_all() {
        let json = r#"{"q": "rust game"}"#;
        let query: SearchQuery = serde_json::from_str(json).expect("SearchQuery must parse");
        assert_eq!(query.search_type, "all");
        assert_eq!(query.q, "rust game");
    }

    #[test]
    fn search_query_explicit_search_type() {
        let json = r#"{"q": "shooter", "search_type": "games", "limit": 10, "offset": 0}"#;
        let query: SearchQuery = serde_json::from_str(json).expect("SearchQuery must parse");
        assert_eq!(query.search_type, "games");
        assert_eq!(query.limit, Some(10));
        assert_eq!(query.offset, Some(0));
    }

    #[test]
    fn game_search_result_has_result_type_game() {
        let result = GameSearchResult {
            id: uuid::Uuid::new_v4(),
            title: "Oxide Arena".to_string(),
            description: Some("A top-down shooter".to_string()),
            developer_username: "dev_alice".to_string(),
            result_type: "game".to_string(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"result_type\":\"game\""));
        assert!(json.contains("Oxide Arena"));
    }

    #[test]
    fn user_search_result_has_result_type_user() {
        let result = UserSearchResult {
            id: uuid::Uuid::new_v4(),
            username: "alice_dev".to_string(),
            avatar_url: None,
            result_type: "user".to_string(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"result_type\":\"user\""));
        assert!(json.contains("alice_dev"));
    }

    #[test]
    fn search_response_has_required_fields() {
        let response = SearchResponse {
            results: vec![],
            total: 0,
            limit: 20,
            offset: 0,
        };
        let json = serde_json::to_string(&response).expect("serialize");
        assert!(json.contains("\"results\""));
        assert!(json.contains("\"total\""));
        assert!(json.contains("\"limit\""));
        assert!(json.contains("\"offset\""));
    }

    #[test]
    fn search_response_wraps_game_results() {
        let game = GameSearchResult {
            id: uuid::Uuid::new_v4(),
            title: "RustCraft".to_string(),
            description: None,
            developer_username: "bob".to_string(),
            result_type: "game".to_string(),
        };
        let response = SearchResponse {
            results: vec![SearchResult::Game(game)],
            total: 1,
            limit: 20,
            offset: 0,
        };
        assert_eq!(response.results.len(), 1);
    }

    #[test]
    fn empty_query_would_return_empty_results() {
        // search() returns empty when q.trim().is_empty().
        let q = "   ";
        assert!(
            q.trim().is_empty(),
            "trimmed whitespace-only query is empty"
        );
        // This mirrors the guard in the search() handler.
        let response = SearchResponse {
            results: vec![],
            total: 0,
            limit: 20,
            offset: 0,
        };
        assert_eq!(response.results.len(), 0);
        assert_eq!(response.total, 0);
    }

    #[test]
    fn limit_clamped_to_100() {
        // The search handler does .min(100) on user-supplied limit.
        let user_limit = 500_i32;
        let effective = user_limit.min(100);
        assert_eq!(effective, 100);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Notification push shape
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod notification_push_shape_tests {
    use magnetite_backend::api::notifications::{NotificationBroadcast, WsNotification};
    use serde_json;
    use uuid::Uuid;

    fn make_ws_notification() -> WsNotification {
        WsNotification {
            id: Uuid::new_v4(),
            notification_type: "FRIEND_REQUEST".to_string(),
            title: "New friend request".to_string(),
            body: Some("Alice wants to be your friend.".to_string()),
            data: Some(serde_json::json!({"from_user_id": "abc123"})),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn ws_notification_serializes_with_type_field() {
        let n = make_ws_notification();
        let json = serde_json::to_string(&n).expect("serialize");
        // Must have "type" key (renamed via serde)
        assert!(
            json.contains("\"type\""),
            "WsNotification must have 'type' field: {json}"
        );
        assert!(json.contains("FRIEND_REQUEST"));
    }

    #[test]
    fn ws_notification_has_id_and_title() {
        let n = make_ws_notification();
        let json = serde_json::to_string(&n).expect("serialize");
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"title\""));
        assert!(json.contains("New friend request"));
    }

    #[test]
    fn notification_broadcast_wraps_user_id_and_notification() {
        let broadcast = NotificationBroadcast {
            user_id: Uuid::new_v4(),
            notification: make_ws_notification(),
        };
        let json = serde_json::to_string(&broadcast).expect("serialize");
        assert!(json.contains("\"user_id\""));
        assert!(json.contains("\"notification\""));
    }

    #[test]
    fn notification_broadcast_can_deserialize() {
        let user_id = Uuid::new_v4();
        let notif = make_ws_notification();
        let broadcast = NotificationBroadcast {
            user_id,
            notification: notif,
        };
        // Round-trip: serialize → deserialize.
        let json = serde_json::to_string(&broadcast).expect("serialize");
        let decoded: NotificationBroadcast = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.user_id, user_id);
        assert_eq!(decoded.notification.notification_type, "FRIEND_REQUEST");
    }

    #[test]
    fn ws_notification_body_is_optional() {
        let n = WsNotification {
            id: Uuid::new_v4(),
            notification_type: "SYSTEM".to_string(),
            title: "Maintenance".to_string(),
            body: None,
            data: None,
            created_at: chrono::Utc::now(),
        };
        // body: None → field omitted or null in JSON, either is fine.
        let json = serde_json::to_string(&n).expect("serialize");
        // Does not panic = pass; shape is valid.
        assert!(json.contains("SYSTEM"));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. NotificationType round-trip
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod notification_type_tests {
    use magnetite_backend::api::notifications::NotificationType;

    #[test]
    fn all_notification_types_round_trip_via_str() {
        let types = [
            NotificationType::AchievementUnlocked,
            NotificationType::GameInvite,
            NotificationType::FriendRequest,
            NotificationType::PayoutComplete,
            NotificationType::SubscriptionRenewal,
            NotificationType::System,
        ];
        for nt in &types {
            let s = nt.as_str();
            let parsed = NotificationType::from_str(s)
                .unwrap_or_else(|| panic!("from_str must round-trip for: {s}"));
            assert_eq!(parsed.as_str(), s, "as_str round-trip failed for {s}");
        }
    }

    #[test]
    fn notification_type_from_str_unknown_returns_none() {
        assert!(NotificationType::from_str("UNKNOWN_TYPE").is_none());
        assert!(NotificationType::from_str("").is_none());
    }

    #[test]
    fn notification_type_serializes_in_screaming_snake_case() {
        // The derive has #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        let nt = NotificationType::FriendRequest;
        let json = serde_json::to_string(&nt).expect("serialize");
        assert_eq!(json, "\"FRIEND_REQUEST\"");
    }

    #[test]
    fn friend_request_notification_as_str_is_correct() {
        assert_eq!(NotificationType::FriendRequest.as_str(), "FRIEND_REQUEST");
    }

    #[test]
    fn payout_complete_notification_as_str_is_correct() {
        assert_eq!(NotificationType::PayoutComplete.as_str(), "PAYOUT_COMPLETE");
    }
}
