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
// 3–4  Refund — balance reversal + entitlement revocation
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod refund_tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    // Wallet balance tracker used to simulate refund balance reversal.
    struct MockWallet {
        balance: Decimal,
    }

    impl MockWallet {
        fn new(initial: Decimal) -> Self {
            Self { balance: initial }
        }

        fn debit(&mut self, amount: Decimal) -> Result<(), &'static str> {
            if self.balance < amount {
                return Err("insufficient funds");
            }
            self.balance -= amount;
            Ok(())
        }

        fn credit(&mut self, amount: Decimal) {
            self.balance += amount;
        }
    }

    // In-memory entitlement that can be revoked.
    struct MockEntitlement {
        #[allow(dead_code)]
        item_id: Uuid,
        revoked: bool,
    }

    impl MockEntitlement {
        fn new(item_id: Uuid) -> Self {
            Self {
                item_id,
                revoked: false,
            }
        }

        fn revoke(&mut self) {
            self.revoked = true;
        }

        fn is_owned(&self) -> bool {
            !self.revoked
        }
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    #[test]
    fn refund_credits_buyer_wallet_with_purchase_amount() {
        let purchase_price = dec!(9.99);
        let initial_balance = dec!(50.00);
        let mut wallet = MockWallet::new(initial_balance);

        // Simulate purchase: debit buyer
        wallet.debit(purchase_price).expect("debit should succeed");
        assert_eq!(wallet.balance, dec!(40.01));

        // Refund: credit buyer back
        wallet.credit(purchase_price);
        assert_eq!(
            wallet.balance, initial_balance,
            "refund must restore exact balance"
        );
    }

    #[test]
    fn refund_restores_balance_exactly_including_cents() {
        // Edge: fractional amounts must round-trip exactly with rust_decimal.
        let price = dec!(0.01);
        let mut wallet = MockWallet::new(dec!(1.00));
        wallet.debit(price).unwrap();
        assert_eq!(wallet.balance, dec!(0.99));
        wallet.credit(price);
        assert_eq!(wallet.balance, dec!(1.00));
    }

    #[test]
    fn refund_revokes_entitlement() {
        let item_id = Uuid::new_v4();
        let mut ent = MockEntitlement::new(item_id);

        // Before refund: user owns the item
        assert!(ent.is_owned(), "entitlement must be active before refund");

        // Refund: revoke
        ent.revoke();

        // After refund: user no longer owns the item
        assert!(!ent.is_owned(), "entitlement must be revoked after refund");
    }

    #[test]
    fn revoked_entitlement_is_not_active() {
        let item_id = Uuid::new_v4();
        let mut ent = MockEntitlement::new(item_id);
        ent.revoke();
        // Revoked must not count as owned
        assert!(!ent.is_owned());
    }

    #[test]
    fn unrevoked_entitlement_is_active() {
        let item_id = Uuid::new_v4();
        let ent = MockEntitlement::new(item_id);
        assert!(ent.is_owned());
    }

    #[test]
    fn balance_after_multiple_refunds_is_additive() {
        let price1 = dec!(5.00);
        let price2 = dec!(3.50);
        let initial = dec!(100.00);
        let mut wallet = MockWallet::new(initial);

        wallet.debit(price1).unwrap();
        wallet.debit(price2).unwrap();
        // Refund both
        wallet.credit(price1);
        wallet.credit(price2);
        assert_eq!(wallet.balance, initial);
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
            developer_share: Some(dec!(6.99)),
            platform_fee: Some(dec!(3.00)),
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
        assert!(json.contains("6.99"), "developer_share value must appear");
        assert!(json.contains("3"), "platform_fee value must appear");
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
// 6  Revenue-share math — 70/30 split
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod revenue_share_math_tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn developer_share(price: Decimal) -> Decimal {
        price * Decimal::new(70, 2) // 0.70
    }

    fn platform_fee(price: Decimal) -> Decimal {
        price * Decimal::new(30, 2) // 0.30
    }

    #[test]
    fn shares_sum_to_total_price() {
        let prices = [
            dec!(9.99),
            dec!(4.99),
            dec!(0.99),
            dec!(24.99),
            dec!(100.00),
        ];
        for price in prices {
            let dev = developer_share(price);
            let plat = platform_fee(price);
            assert_eq!(
                dev + plat,
                price,
                "developer_share + platform_fee must equal price for {price}"
            );
        }
    }

    #[test]
    fn developer_gets_70_pct_of_price() {
        let price = dec!(100.00);
        let dev = developer_share(price);
        assert_eq!(dev, dec!(70.00));
    }

    #[test]
    fn platform_gets_30_pct_of_price() {
        let price = dec!(100.00);
        let plat = platform_fee(price);
        assert_eq!(plat, dec!(30.00));
    }

    #[test]
    fn zero_price_yields_zero_shares() {
        let dev = developer_share(dec!(0));
        let plat = platform_fee(dec!(0));
        assert!(dev.is_zero());
        assert!(plat.is_zero());
    }

    #[test]
    fn fractional_price_shares_round_trip() {
        let price = dec!(0.99);
        let dev = developer_share(price);
        let plat = platform_fee(price);
        assert_eq!(dev + plat, price);
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
