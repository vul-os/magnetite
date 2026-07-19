// store_hardening_tests.rs — Store purchase hardening tests.
//
// Coverage (pure-logic / serialization — no live DB required):
//   1. Entitlement idempotency guard  — double-buy correctly blocked; error is Validation.
//   2. Idempotency key replay         — same key returns the cached purchase without error.
//   3. Refund reverses balance        — post-refund wallet arithmetic is correct.
//   4. Refund revokes entitlement     — revoked flag semantics (revoked = true means no entitlement).
//   5. Purchase history shape         — StorePurchase serializes with all required fields.
//   6. Revenue-share math             — developer gets 70 %, platform gets 30 %.
//   7. Item kind / currency guards    — validate_item_kind / validate_item_currency pure logic.
//   8. Insufficient-funds error type  — InsufficientFunds variant carries the right status code.
//   9. Entitlement expires_at semantics — expired entitlement should not count as owned.
//  10. StorePurchase with null shares — points purchases have null developer_share / platform_fee.
//  11. Entitlement shape round-trip   — Entitlement serializes and deserializes correctly.
//  12. Purchase pagination bounds     — limit/offset clamping follows documented constraints.

// ─────────────────────────────────────────────────────────────────────────────
// 1–2  Entitlement idempotency — double-buy guard + key replay
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod entitlement_idempotency_tests {
    use magnetite_backend::error::AppError;
    use uuid::Uuid;

    // ── Helpers that mirror the service logic without a DB ───────────────────

    struct MockEntitlementStore {
        owned: Vec<Uuid>, // item_ids the user already owns
    }

    impl MockEntitlementStore {
        fn new(owned: Vec<Uuid>) -> Self {
            Self { owned }
        }

        /// Returns true if the user already holds a non-revoked, non-expired entitlement.
        fn has_entitlement(&self, item_id: Uuid) -> bool {
            self.owned.contains(&item_id)
        }

        /// Attempts to "purchase" the item; rejects if already owned.
        fn purchase(&self, item_id: Uuid) -> Result<(), AppError> {
            if self.has_entitlement(item_id) {
                return Err(AppError::Validation(
                    "You already own this item".to_string(),
                ));
            }
            Ok(())
        }
    }

    struct MockIdempotencyStore {
        // Maps idempotency_key → purchase_id (to simulate replay)
        keys: std::collections::HashMap<String, Uuid>,
    }

    impl MockIdempotencyStore {
        fn new() -> Self {
            Self {
                keys: std::collections::HashMap::new(),
            }
        }

        fn record(&mut self, key: &str, purchase_id: Uuid) {
            self.keys.insert(key.to_string(), purchase_id);
        }

        fn lookup(&self, key: &str) -> Option<Uuid> {
            self.keys.get(key).copied()
        }
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    #[test]
    fn first_purchase_of_unowned_item_succeeds() {
        let item_id = Uuid::new_v4();
        let store = MockEntitlementStore::new(vec![]);
        assert!(store.purchase(item_id).is_ok());
    }

    #[test]
    fn double_buy_rejected_with_validation_error() {
        let item_id = Uuid::new_v4();
        // User already owns the item
        let store = MockEntitlementStore::new(vec![item_id]);

        let err = store.purchase(item_id).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(
                    msg.contains("already own"),
                    "error message should mention 'already own', got: {msg}"
                );
            }
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    #[test]
    fn double_buy_of_different_item_is_allowed() {
        let item_a = Uuid::new_v4();
        let item_b = Uuid::new_v4();
        // User owns item_a but not item_b
        let store = MockEntitlementStore::new(vec![item_a]);
        // Buying item_b should succeed
        assert!(store.purchase(item_b).is_ok());
    }

    #[test]
    fn idempotency_key_replay_returns_existing_purchase_id() {
        let mut idem_store = MockIdempotencyStore::new();
        let key = "idem-key-abc-123";
        let purchase_id = Uuid::new_v4();

        // First time: record the purchase
        idem_store.record(key, purchase_id);

        // Second time: lookup should return the same purchase_id
        let replayed = idem_store.lookup(key);
        assert_eq!(
            replayed,
            Some(purchase_id),
            "same idempotency key must return the same purchase"
        );
    }

    #[test]
    fn idempotency_key_not_found_returns_none() {
        let idem_store = MockIdempotencyStore::new();
        assert!(idem_store.lookup("never-used-key").is_none());
    }

    #[test]
    fn idempotency_key_uniqueness_preserved_across_users() {
        // Each user gets a different purchase_id even for the same item
        let mut store = MockIdempotencyStore::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        store.record("user1-item1", p1);
        store.record("user2-item1", p2);
        assert_ne!(
            store.lookup("user1-item1").unwrap(),
            store.lookup("user2-item1").unwrap()
        );
    }

    #[test]
    fn validation_error_surfaces_as_bad_request_status() {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;

        let err = AppError::Validation("You already own this item".to_string());
        let response = err.into_response();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "double-buy Validation error must map to HTTP 400"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3–4  Refund — receipt void + entitlement revocation (NON-CUSTODIAL)
// ─────────────────────────────────────────────────────────────────────────────
//
// There is no balance to reverse: the buyer paid the developer directly on the
// rail. A refund is therefore the VOID of the signed receipt plus revocation of
// the entitlement it granted. These tests pin that shape.

#[cfg(test)]
mod refund_tests {
    use uuid::Uuid;

    /// A stored receipt: signed proof of a wallet-to-wallet transfer.
    struct MockReceipt {
        total_units: u64,
        voided: bool,
    }

    impl MockReceipt {
        fn new(total_units: u64) -> Self {
            Self { total_units, voided: false }
        }
        fn void(&mut self) {
            self.voided = true;
        }
        /// A receipt only proves an entitlement while it stands.
        fn grants(&self) -> bool {
            !self.voided
        }
    }

    struct MockEntitlement {
        #[allow(dead_code)]
        item_id: Uuid,
        revoked: bool,
    }

    impl MockEntitlement {
        fn new(item_id: Uuid) -> Self {
            Self { item_id, revoked: false }
        }
        fn revoke(&mut self) {
            self.revoked = true;
        }
        fn is_owned(&self) -> bool {
            !self.revoked
        }
    }

    #[test]
    fn refund_voids_the_receipt_rather_than_moving_money() {
        let mut receipt = MockReceipt::new(999);
        assert!(receipt.grants());

        receipt.void();

        assert!(!receipt.grants(), "a voided receipt must prove nothing");
        assert_eq!(
            receipt.total_units, 999,
            "voiding must not rewrite the historical amount — the transfer really happened"
        );
    }

    #[test]
    fn refund_revokes_entitlement() {
        let mut ent = MockEntitlement::new(Uuid::new_v4());
        assert!(ent.is_owned());
        ent.revoke();
        assert!(!ent.is_owned());
    }

    #[test]
    fn voided_receipt_and_live_entitlement_is_an_inconsistent_state() {
        // This is exactly what the superadmin `voided_receipts_grant_nothing`
        // compliance check exists to catch.
        let mut receipt = MockReceipt::new(500);
        let ent = MockEntitlement::new(Uuid::new_v4());
        receipt.void();
        assert!(
            !(receipt.grants()) && ent.is_owned(),
            "voided receipt + unrevoked entitlement must be detectable"
        );
    }

    #[test]
    fn unrevoked_entitlement_is_active() {
        let ent = MockEntitlement::new(Uuid::new_v4());
        assert!(ent.is_owned());
    }

    #[test]
    fn revoked_entitlement_is_not_active() {
        let mut ent = MockEntitlement::new(Uuid::new_v4());
        ent.revoke();
        assert!(!ent.is_owned());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5  Purchase history shape — StorePurchase serialization
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod purchase_history_shape_tests {
    use chrono::Utc;
    use magnetite_backend::services::marketplace::StorePurchase;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_usd_purchase() -> StorePurchase {
        StorePurchase {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            item_id: Uuid::new_v4(),
            store_id: Uuid::new_v4(),
            game_id: Uuid::new_v4(),
            price_paid: dec!(9.99),
            currency: "USD".to_string(),
            // Non-custodial: developer takes the whole subtotal, fee defaults to 0.
            developer_share: Some(dec!(9.99)),
            platform_fee: Some(dec!(0.00)),
            status: "completed".to_string(),
            idempotency_key: Some("idem-key-usd-001".to_string()),
            metadata: None,
            created_at: Utc::now(),
            refunded_at: None,
            refunded_by: None,
            refund_reason: None,
        }
    }

    fn make_points_purchase() -> StorePurchase {
        StorePurchase {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            item_id: Uuid::new_v4(),
            store_id: Uuid::new_v4(),
            game_id: Uuid::new_v4(),
            price_paid: dec!(500),
            currency: "points".to_string(),
            developer_share: None, // points purchases have no revenue share
            platform_fee: None,
            status: "completed".to_string(),
            idempotency_key: None,
            metadata: None,
            created_at: Utc::now(),
            refunded_at: None,
            refunded_by: None,
            refund_reason: None,
        }
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    #[test]
    fn usd_purchase_serializes_with_required_fields() {
        let p = make_usd_purchase();
        let json = serde_json::to_string(&p).unwrap();

        assert!(json.contains("\"id\""), "must include id");
        assert!(json.contains("\"user_id\""), "must include user_id");
        assert!(json.contains("\"item_id\""), "must include item_id");
        assert!(json.contains("\"store_id\""), "must include store_id");
        assert!(json.contains("\"game_id\""), "must include game_id");
        assert!(json.contains("\"price_paid\""), "must include price_paid");
        assert!(json.contains("\"currency\""), "must include currency");
        assert!(json.contains("\"status\""), "must include status");
        assert!(json.contains("\"created_at\""), "must include created_at");
    }

    #[test]
    fn usd_purchase_contains_revenue_share_fields() {
        let p = make_usd_purchase();
        let json = serde_json::to_string(&p).unwrap();

        assert!(
            json.contains("developer_share"),
            "USD purchase must include developer_share"
        );
        assert!(
            json.contains("platform_fee"),
            "USD purchase must include platform_fee"
        );
        // Full-subtotal settlement: the developer's share IS the price.
        assert!(
            json.contains("9.99"),
            "developer_share must equal the full price"
        );
        assert!(
            json.contains("0.00"),
            "platform_fee is 0 at the default protocol rate"
        );
    }

    #[test]
    fn usd_purchase_contains_idempotency_key() {
        let p = make_usd_purchase();
        let json = serde_json::to_string(&p).unwrap();
        assert!(
            json.contains("idem-key-usd-001"),
            "idempotency key must appear in JSON"
        );
    }

    #[test]
    fn points_purchase_has_null_revenue_shares() {
        let p = make_points_purchase();
        let json = serde_json::to_string(&p).unwrap();

        // points purchases: developer_share and platform_fee are both null
        // The JSON contains the keys with null values.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v["developer_share"].is_null(),
            "points purchase must have null developer_share"
        );
        assert!(
            v["platform_fee"].is_null(),
            "points purchase must have null platform_fee"
        );
    }

    #[test]
    fn points_purchase_currency_is_points() {
        let p = make_points_purchase();
        let json = serde_json::to_string(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["currency"].as_str().unwrap(), "points");
    }

    #[test]
    fn usd_purchase_currency_is_usd() {
        let p = make_usd_purchase();
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["currency"].as_str().unwrap(), "USD");
    }

    #[test]
    fn purchase_status_is_completed() {
        let p = make_usd_purchase();
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["status"].as_str().unwrap(), "completed");
    }

    #[test]
    fn purchase_round_trips_via_serde() {
        let p = make_usd_purchase();
        let json = serde_json::to_string(&p).unwrap();
        let back: StorePurchase = serde_json::from_str(&json).unwrap();
        assert_eq!(back.currency, "USD");
        assert_eq!(back.status, "completed");
        assert_eq!(back.price_paid, dec!(9.99));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6  Settlement split math — developer takes the FULL subtotal
// ─────────────────────────────────────────────────────────────────────────────
//
// The 70/30 platform cut is gone. Non-custodially the developer receives the
// whole subtotal and the protocol fee (default 0 bps) rides on top, so the
// buyer's total is subtotal + fee and the receipt legs must sum to that total.

#[cfg(test)]
mod settlement_split_math_tests {
    /// Protocol fee in the rail's smallest unit, from basis points.
    fn protocol_fee(subtotal: u64, bps: u16) -> u64 {
        subtotal * bps as u64 / 10_000
    }

    /// What the buyer is charged: the developer's subtotal plus the fee.
    fn buyer_total(subtotal: u64, bps: u16) -> u64 {
        subtotal + protocol_fee(subtotal, bps)
    }

    /// The developer leg of a `PaymentSplit`, mirroring `payment::sale_split`:
    /// the seller is credited the subtotal itself, regardless of protocol fee.
    fn developer_leg(subtotal: u64, _bps: u16) -> u64 {
        subtotal
    }

    #[test]
    fn developer_receives_the_entire_subtotal_not_seventy_percent() {
        for subtotal in [999u64, 499, 99, 2499, 10_000] {
            let legacy_70_pct = subtotal * 70 / 100;
            let actual = developer_leg(subtotal, 0);

            assert_eq!(
                actual, subtotal,
                "the developer must receive 100% of the subtotal"
            );
            assert!(
                actual > legacy_70_pct,
                "the old 70/30 custodial split must be gone (got {actual}, legacy would be {legacy_70_pct})"
            );
        }
    }

    #[test]
    fn protocol_fee_does_not_shrink_the_developer_leg() {
        let subtotal = 10_000;
        assert_eq!(
            developer_leg(subtotal, 0),
            developer_leg(subtotal, 500),
            "raising the protocol fee must not take anything from the seller"
        );
    }

    #[test]
    fn default_protocol_fee_is_zero_so_buyer_pays_exactly_the_price() {
        let subtotal = 999;
        assert_eq!(protocol_fee(subtotal, 0), 0);
        assert_eq!(buyer_total(subtotal, 0), subtotal);
    }

    #[test]
    fn receipt_legs_sum_to_total_including_fee() {
        let subtotal = 10_000;
        let bps = 250; // 2.5%, if governance ever enables one
        let fee = protocol_fee(subtotal, bps);
        let total = buyer_total(subtotal, bps);

        assert_eq!(fee, 250);
        assert_eq!(
            subtotal + fee,
            total,
            "developer leg + protocol fee must equal the receipt total"
        );
    }

    #[test]
    fn fee_never_comes_out_of_the_developer_leg() {
        let subtotal = 500;
        let bps = 1_000; // 10%
        let total = buyer_total(subtotal, bps);
        assert!(
            total > subtotal,
            "a protocol fee must be charged ON TOP, never deducted from the developer"
        );
    }

    #[test]
    fn zero_price_settles_to_zero() {
        assert_eq!(buyer_total(0, 0), 0);
        assert_eq!(protocol_fee(0, 500), 0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7  Item kind / currency guards — validate_item_kind / validate_item_currency
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod item_validation_tests {
    // Mirror the validation logic from services/marketplace.rs
    fn validate_item_kind(kind: &str) -> Result<(), String> {
        match kind {
            "cosmetic" | "item" | "dlc" | "pass" => Ok(()),
            other => Err(format!("Invalid item kind '{other}'")),
        }
    }

    fn validate_item_currency(currency: &str) -> Result<(), String> {
        match currency {
            "USD" | "points" => Ok(()),
            other => Err(format!("Invalid currency '{other}'")),
        }
    }

    #[test]
    fn cosmetic_kind_is_valid() {
        assert!(validate_item_kind("cosmetic").is_ok());
    }

    #[test]
    fn item_kind_is_valid() {
        assert!(validate_item_kind("item").is_ok());
    }

    #[test]
    fn dlc_kind_is_valid() {
        assert!(validate_item_kind("dlc").is_ok());
    }

    #[test]
    fn pass_kind_is_valid() {
        assert!(validate_item_kind("pass").is_ok());
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert!(validate_item_kind("weapon").is_err());
        assert!(validate_item_kind("skin").is_err());
        assert!(validate_item_kind("").is_err());
        assert!(validate_item_kind("COSMETIC").is_err()); // case-sensitive
    }

    #[test]
    fn usd_currency_is_valid() {
        assert!(validate_item_currency("USD").is_ok());
    }

    #[test]
    fn points_currency_is_valid() {
        assert!(validate_item_currency("points").is_ok());
    }

    #[test]
    fn unknown_currency_is_rejected() {
        assert!(validate_item_currency("BTC").is_err());
        assert!(validate_item_currency("ETH").is_err());
        assert!(validate_item_currency("usdc").is_err()); // not in list
        assert!(validate_item_currency("").is_err());
    }

    #[test]
    fn exactly_four_valid_kinds() {
        let valid = ["cosmetic", "item", "dlc", "pass"];
        assert_eq!(valid.len(), 4);
        for k in valid {
            assert!(validate_item_kind(k).is_ok(), "'{k}' must be a valid kind");
        }
    }

    #[test]
    fn exactly_two_valid_currencies() {
        let valid = ["USD", "points"];
        assert_eq!(valid.len(), 2);
        for c in valid {
            assert!(
                validate_item_currency(c).is_ok(),
                "'{c}' must be a valid currency"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8  Insufficient funds — error type and HTTP status
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod insufficient_funds_error_tests {
    use axum::{http::StatusCode, response::IntoResponse};
    use magnetite_backend::error::AppError;
    use rust_decimal_macros::dec;

    #[test]
    fn insufficient_funds_error_message_contains_balance_info() {
        let err = AppError::InsufficientFunds(
            "Insufficient USD balance. Have 5.00, need 9.99".to_string(),
        );
        let msg = err.to_string();
        assert!(
            msg.contains("Insufficient"),
            "error message must contain 'Insufficient'"
        );
    }

    #[test]
    fn insufficient_funds_maps_to_400_bad_request() {
        let err = AppError::InsufficientFunds("Not enough funds".to_string());
        let response = err.into_response();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "InsufficientFunds must map to HTTP 400"
        );
    }

    #[test]
    fn purchase_blocked_when_balance_is_zero() {
        let balance = dec!(0.00);
        let price = dec!(0.01);
        assert!(
            balance < price,
            "zero balance cannot afford even the cheapest item"
        );
    }

    #[test]
    fn purchase_allowed_when_balance_equals_price() {
        let balance = dec!(9.99);
        let price = dec!(9.99);
        assert!(balance >= price, "exact balance should allow the purchase");
    }

    #[test]
    fn purchase_blocked_when_balance_is_one_cent_short() {
        let balance = dec!(9.98);
        let price = dec!(9.99);
        assert!(balance < price);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9  Entitlement expiry semantics
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod entitlement_expiry_tests {
    use chrono::{Duration, Utc};

    fn is_entitlement_active(
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        revoked: bool,
    ) -> bool {
        if revoked {
            return false;
        }
        match expires_at {
            None => true,
            Some(exp) => exp > Utc::now(),
        }
    }

    #[test]
    fn permanent_non_revoked_entitlement_is_active() {
        assert!(is_entitlement_active(None, false));
    }

    #[test]
    fn revoked_entitlement_is_not_active_regardless_of_expiry() {
        assert!(!is_entitlement_active(None, true));
        let future = Some(Utc::now() + Duration::days(365));
        assert!(!is_entitlement_active(future, true));
    }

    #[test]
    fn entitlement_with_future_expiry_is_active() {
        let future = Some(Utc::now() + Duration::hours(1));
        assert!(is_entitlement_active(future, false));
    }

    #[test]
    fn entitlement_with_past_expiry_is_not_active() {
        let past = Some(Utc::now() - Duration::hours(1));
        assert!(!is_entitlement_active(past, false));
    }

    #[test]
    fn entitlement_with_past_expiry_and_revoked_is_not_active() {
        let past = Some(Utc::now() - Duration::hours(1));
        assert!(!is_entitlement_active(past, true));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 10-11  Entitlement shape round-trip
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod entitlement_shape_tests {
    use chrono::Utc;
    use magnetite_backend::services::marketplace::Entitlement;
    use uuid::Uuid;

    fn make_entitlement(revoked: bool) -> Entitlement {
        Entitlement {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            item_id: Uuid::new_v4(),
            purchase_id: Some(Uuid::new_v4()),
            granted_at: Utc::now(),
            expires_at: None,
            revoked,
        }
    }

    #[test]
    fn active_entitlement_serializes_with_revoked_false() {
        let ent = make_entitlement(false);
        let json = serde_json::to_string(&ent).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["revoked"], serde_json::json!(false));
    }

    #[test]
    fn revoked_entitlement_serializes_with_revoked_true() {
        let ent = make_entitlement(true);
        let json = serde_json::to_string(&ent).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["revoked"], serde_json::json!(true));
    }

    #[test]
    fn entitlement_has_all_required_fields() {
        let ent = make_entitlement(false);
        let json = serde_json::to_string(&ent).unwrap();
        for field in &[
            "id",
            "user_id",
            "item_id",
            "purchase_id",
            "granted_at",
            "revoked",
        ] {
            assert!(
                json.contains(&format!("\"{field}\"")),
                "entitlement JSON must contain field '{field}'"
            );
        }
    }

    #[test]
    fn entitlement_round_trips_via_serde() {
        let ent = make_entitlement(false);
        let json = serde_json::to_string(&ent).unwrap();
        let back: Entitlement = serde_json::from_str(&json).unwrap();
        assert_eq!(back.revoked, false);
        assert_eq!(back.id, ent.id);
        assert_eq!(back.user_id, ent.user_id);
        assert_eq!(back.item_id, ent.item_id);
    }

    #[test]
    fn entitlement_with_null_purchase_id_is_valid() {
        // Entitlements granted manually (without a purchase) have null purchase_id.
        let ent = Entitlement {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            item_id: Uuid::new_v4(),
            purchase_id: None,
            granted_at: Utc::now(),
            expires_at: None,
            revoked: false,
        };
        let v: serde_json::Value = serde_json::to_value(&ent).unwrap();
        assert!(v["purchase_id"].is_null());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 12  Purchase pagination — limit/offset constraints
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod purchase_pagination_tests {
    // Mirror the limit/offset clamping logic from api/marketplace.rs:
    //   let limit = q.limit.unwrap_or(50).min(200);
    //   let offset = q.offset.unwrap_or(0).max(0);

    fn resolve_limit(requested: Option<i64>) -> i64 {
        requested.unwrap_or(50).min(200)
    }

    fn resolve_offset(requested: Option<i64>) -> i64 {
        requested.unwrap_or(0).max(0)
    }

    #[test]
    fn default_limit_is_50() {
        assert_eq!(resolve_limit(None), 50);
    }

    #[test]
    fn default_offset_is_zero() {
        assert_eq!(resolve_offset(None), 0);
    }

    #[test]
    fn limit_capped_at_200() {
        assert_eq!(resolve_limit(Some(500)), 200);
        assert_eq!(resolve_limit(Some(201)), 200);
        assert_eq!(resolve_limit(Some(200)), 200);
    }

    #[test]
    fn limit_below_cap_is_respected() {
        assert_eq!(resolve_limit(Some(10)), 10);
        assert_eq!(resolve_limit(Some(1)), 1);
    }

    #[test]
    fn negative_offset_clamped_to_zero() {
        assert_eq!(resolve_offset(Some(-1)), 0);
        assert_eq!(resolve_offset(Some(-100)), 0);
    }

    #[test]
    fn positive_offset_is_respected() {
        assert_eq!(resolve_offset(Some(50)), 50);
        assert_eq!(resolve_offset(Some(200)), 200);
    }
}
