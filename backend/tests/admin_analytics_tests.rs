// admin_analytics_tests.rs — Admin analytics must read live-written tables.
//
// Bug this guards against: `revenue_dashboard`, `analytics_overview`,
// `analytics_revenue` and `list_transactions` (backend/src/api/admin.rs) used to
// read the legacy `transactions` table (`platform_fee` / `game_fee` rows).
// Nothing has written to that table since the non-custodial payment pivot, so
// every one of those endpoints always reported zero — silently, since the
// queries were syntactically valid and just summed an empty set. The fix reads
// `payment_receipts` (and `play_sessions` for activity counts) instead, which
// ARE written by the live checkout/session paths.
//
// Tests are split in two:
//   * Shape tests (no DB) — prove the "no platform-fee / revenue-cut metrics"
//     rule from the payment pivot: the JSON wire shape must not resurrect
//     `platform_fee` / `total_platform_revenue` / `total_game_revenue` framing.
//     These always run in `cargo test`.
//   * Live tests (`#[ignore]`, need `DATABASE_URL`) — seed a real
//     `payment_receipts` row (plus a poison row in the legacy `transactions`
//     table) and assert the handler's response moved by exactly the seeded
//     amount, proving the query reads the right table. Run with
//     `cargo test -- --ignored` against a migrated Postgres (see
//     CONTRIBUTING.md's `DATABASE_URL=postgres://.../magnetite_test`).

#[cfg(test)]
mod shape_tests {
    use magnetite_backend::api::admin::{
        AdminTransaction, RevenueAnalytics, RevenueByGame, RevenueDashboard, RevenueTimeSeries,
    };
    use rust_decimal::Decimal;
    use uuid::Uuid;

    #[test]
    fn revenue_dashboard_has_no_platform_fee_field() {
        let dashboard = RevenueDashboard {
            total_settled_units: 1000,
            total_protocol_fee_units: 0,
            total_developer_settled_units: 1000,
            voided_receipts: 0,
            active_users: 1,
            total_games: 1,
        };
        let json = serde_json::to_value(&dashboard).unwrap();
        assert!(
            json.get("total_platform_revenue").is_none(),
            "RevenueDashboard must not resurrect the deleted platform-fee/game-fee split"
        );
        assert!(
            json.get("total_game_revenue").is_none(),
            "RevenueDashboard must not resurrect the deleted platform-fee/game-fee split"
        );
        // The real, honest fields must be present.
        assert_eq!(json["total_settled_units"], 1000);
        assert_eq!(json["total_developer_settled_units"], 1000);
    }

    #[test]
    fn revenue_by_game_reports_developer_settled_not_a_platform_cut() {
        let row = RevenueByGame {
            game_id: Uuid::new_v4(),
            game_title: "Test Game".to_string(),
            developer_username: Some("dev".to_string()),
            total_revenue: Decimal::new(1000, 2),
            receipt_count: 3,
            protocol_fee: Decimal::new(0, 2),
            developer_settled: Decimal::new(1000, 2),
        };
        let json = serde_json::to_value(&row).unwrap();
        assert!(
            json.get("platform_fee").is_none(),
            "RevenueByGame must not carry a platform_fee field (no platform cut model)"
        );
        assert!(json.get("protocol_fee").is_some());
        assert!(json.get("developer_settled").is_some());
        // `play_sessions` was a mislabelled count of legacy transaction rows;
        // it must be gone in favour of the honestly-named `receipt_count`.
        assert!(json.get("play_sessions").is_none());
        assert_eq!(json["receipt_count"], 3);
    }

    #[test]
    fn revenue_analytics_totals_have_no_platform_revenue_field() {
        let analytics = RevenueAnalytics {
            daily: vec![],
            weekly: vec![],
            monthly: vec![],
            by_game: vec![],
            total_protocol_fee: Decimal::ZERO,
            total_developer_settled: Decimal::ZERO,
        };
        let json = serde_json::to_value(&analytics).unwrap();
        assert!(json.get("total_platform_revenue").is_none());
        assert!(json.get("total_protocol_fee").is_some());
    }

    #[test]
    fn revenue_time_series_has_no_platform_or_game_revenue_fields() {
        let point = RevenueTimeSeries {
            date: "2026-07-20".to_string(),
            protocol_fee: Decimal::ZERO,
            developer_settled: Decimal::new(500, 2),
            total_revenue: Decimal::new(500, 2),
        };
        let json = serde_json::to_value(&point).unwrap();
        assert!(json.get("platform_revenue").is_none());
        assert!(json.get("game_revenue").is_none());
    }

    #[test]
    fn admin_transaction_is_receipt_shaped_not_legacy_transaction_shaped() {
        let txn = AdminTransaction {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            username: Some("buyer".to_string()),
            game_id: None,
            game_title: None,
            kind: "item_purchase".to_string(),
            total: Decimal::new(499, 2),
            protocol_fee: Decimal::ZERO,
            payee: Some("abc123".to_string()),
            rail_pubkey: "deadbeef".to_string(),
            voided: false,
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_value(&txn).unwrap();
        // The old shape read `tx_type` / `amount` / `status` off the legacy
        // `transactions` table. The new shape is receipt-native.
        assert!(json.get("tx_type").is_none());
        assert!(json.get("amount").is_none());
        assert!(json.get("status").is_none());
        assert_eq!(json["kind"], "item_purchase");
        assert_eq!(json["voided"], false);
    }
}

// NOTE: `revenue_dashboard` / `analytics_overview` / `analytics_revenue` sum
// PLATFORM-WIDE totals by design (no per-game/per-user filter), and these tests
// assert on before/after deltas of those global sums. Run this file's ignored
// tests single-threaded so they do not observe each other's seeded rows:
//   DATABASE_URL=postgres://.../magnetite_test cargo test --test admin_analytics_tests -- --ignored --test-threads=1
#[cfg(test)]
mod live_db_tests {
    use axum::extract::{Extension, Query, State};
    use magnetite_backend::api::admin::{
        analytics_overview, analytics_revenue, list_transactions, revenue_dashboard,
        PaginationQuery,
    };
    use rust_decimal::Decimal;
    use sqlx::postgres::PgPoolOptions;
    use sqlx::PgPool;
    use uuid::Uuid;

    /// Connects to a real, already-migrated Postgres. Requires `DATABASE_URL`
    /// (see CONTRIBUTING.md). These tests are `#[ignore]`d so `cargo test` stays
    /// DB-free by default, matching every other integration test in this crate.
    async fn pool() -> PgPool {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run the #[ignore]d admin analytics DB tests");
        PgPoolOptions::new()
            .max_connections(3)
            .connect(&url)
            .await
            .expect("failed to connect to DATABASE_URL")
    }

    async fn seed_user(pool: &PgPool, is_admin: bool, is_developer: bool) -> Uuid {
        let id = Uuid::new_v4();
        let username = format!("aatest_{}", id.simple());
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, is_admin, is_developer)
             VALUES ($1, $2, $3, 'x', $4, $5)",
        )
        .bind(id)
        .bind(&username)
        .bind(format!("{username}@example.test"))
        .bind(is_admin)
        .bind(is_developer)
        .execute(pool)
        .await
        .expect("seed user");
        id
    }

    async fn seed_game(pool: &PgPool, developer_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO games (id, developer_id, github_repo, title, status, active)
             VALUES ($1, $2, 'https://github.com/test/repo', 'AA Test Game', 'approved', true)",
        )
        .bind(id)
        .bind(developer_id)
        .execute(pool)
        .await
        .expect("seed game");
        id
    }

    /// Seeds a settled, non-voided `payment_receipts` row — the live-written
    /// non-custodial ledger these endpoints must read.
    async fn seed_receipt(
        pool: &PgPool,
        buyer_id: Uuid,
        game_id: Uuid,
        total_cents: i64,
        protocol_fee_cents: i64,
        payee_wallet: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let payouts = serde_json::json!([{ "wallet": payee_wallet, "amount": total_cents - protocol_fee_cents }]);
        sqlx::query(
            "INSERT INTO payment_receipts
                (id, kind, buyer_id, buyer_pubkey, game_id, total, protocol_fee, payouts,
                 nonce, rail_pubkey, sig, rail, voided, created_at)
             VALUES ($1, 'item_purchase', $2, $3, $4, $5, $6, $7,
                     $8, $9, $10, 'mock', false, NOW())",
        )
        .bind(id)
        .bind(buyer_id)
        .bind("b".repeat(64))
        .bind(game_id)
        .bind(total_cents)
        .bind(protocol_fee_cents)
        .bind(payouts)
        .bind(format!("nonce-{id}"))
        .bind("r".repeat(64))
        .bind("s".repeat(128))
        .execute(pool)
        .await
        .expect("seed receipt");
        id
    }

    /// Seeds a poison row in the legacy `transactions` table with an amount far
    /// larger than any seeded receipt (dollars, not cents — `transactions.amount`
    /// is `DECIMAL(10,6)`, capped under 10^4). If any rewritten endpoint still
    /// reads this table, its totals jump by ~9999 instead of the real receipt
    /// amount — making a regression back to the old table loud, not silent.
    async fn seed_legacy_poison_transaction(pool: &PgPool, user_id: Uuid, game_id: Uuid) {
        sqlx::query(
            "INSERT INTO transactions (id, user_id, game_id, type, amount)
             VALUES ($1, $2, $3, 'platform_fee', 9999)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(game_id)
        .execute(pool)
        .await
        .expect("seed legacy poison row");
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL against a migrated Postgres"]
    async fn revenue_dashboard_reflects_a_seeded_receipt_not_the_legacy_table() {
        let pool = pool().await;
        let admin_id = seed_user(&pool, true, false).await;
        let dev_id = seed_user(&pool, false, true).await;
        let buyer_id = seed_user(&pool, false, false).await;
        let game_id = seed_game(&pool, dev_id).await;

        let before = revenue_dashboard(State(pool.clone()), Extension(admin_id))
            .await
            .expect("revenue_dashboard before seed");

        seed_receipt(&pool, buyer_id, game_id, 1234, 34, &"d".repeat(64)).await;
        seed_legacy_poison_transaction(&pool, buyer_id, game_id).await;

        let after = revenue_dashboard(State(pool.clone()), Extension(admin_id))
            .await
            .expect("revenue_dashboard after seed");

        assert_eq!(
            after.total_settled_units - before.total_settled_units,
            1234,
            "total_settled_units must move by exactly the seeded receipt total \
             (1234 cents) — a jump near 9999 would mean it is still reading \
             the legacy `transactions` table"
        );
        assert_eq!(
            after.total_protocol_fee_units - before.total_protocol_fee_units,
            34
        );
        assert_eq!(
            after.total_developer_settled_units - before.total_developer_settled_units,
            1200
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL against a migrated Postgres"]
    async fn analytics_overview_counts_real_play_sessions_and_receipts() {
        let pool = pool().await;
        let admin_id = seed_user(&pool, true, false).await;
        let dev_id = seed_user(&pool, false, true).await;
        let buyer_id = seed_user(&pool, false, false).await;
        let game_id = seed_game(&pool, dev_id).await;

        let before = analytics_overview(State(pool.clone()), Extension(admin_id))
            .await
            .expect("analytics_overview before seed");

        seed_receipt(&pool, buyer_id, game_id, 500, 0, &"e".repeat(64)).await;
        seed_legacy_poison_transaction(&pool, buyer_id, game_id).await;

        // Real, live-written play session — the honest source for activity
        // counts (the legacy `transactions` table was never a real source for
        // this either, `type = 'play_session'` rows were never written by
        // anything).
        sqlx::query(
            "INSERT INTO play_sessions (id, game_id, user_id, status, started_at)
             VALUES ($1, $2, $3, 'active', NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(game_id)
        .bind(buyer_id)
        .execute(&pool)
        .await
        .expect("seed play session");

        let after = analytics_overview(State(pool.clone()), Extension(admin_id))
            .await
            .expect("analytics_overview after seed");

        assert_eq!(after.total_play_sessions - before.total_play_sessions, 1);
        assert_eq!(
            after.active_users_24h - before.active_users_24h,
            1,
            "active_users_24h must move by 1 for our fresh, never-before-seen buyer"
        );
        assert_eq!(
            after.total_revenue_usd - before.total_revenue_usd,
            Decimal::new(500, 2)
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL against a migrated Postgres"]
    async fn list_transactions_returns_the_seeded_receipt_not_legacy_rows() {
        let pool = pool().await;
        let admin_id = seed_user(&pool, true, false).await;
        let dev_id = seed_user(&pool, false, true).await;
        let buyer_id = seed_user(&pool, false, false).await;
        let game_id = seed_game(&pool, dev_id).await;
        let payee = "f".repeat(64);

        let receipt_id = seed_receipt(&pool, buyer_id, game_id, 4999, 0, &payee).await;
        seed_legacy_poison_transaction(&pool, buyer_id, game_id).await;

        let page = list_transactions(
            State(pool.clone()),
            Extension(admin_id),
            Query(PaginationQuery {
                page: Some(1),
                limit: Some(200),
            }),
        )
        .await
        .expect("list_transactions");

        let found = page
            .data
            .iter()
            .find(|t| t.id == receipt_id)
            .expect("seeded receipt must appear in the admin transactions list");

        assert_eq!(found.kind, "item_purchase");
        assert_eq!(found.total, Decimal::new(4999, 2));
        assert_eq!(found.protocol_fee, Decimal::ZERO);
        assert_eq!(found.payee.as_deref(), Some(payee.as_str()));
        assert!(!found.voided);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL against a migrated Postgres"]
    async fn analytics_revenue_by_game_reports_real_developer_settled_amount() {
        let pool = pool().await;
        let admin_id = seed_user(&pool, true, false).await;
        let dev_id = seed_user(&pool, false, true).await;
        let buyer_id = seed_user(&pool, false, false).await;
        let game_id = seed_game(&pool, dev_id).await;

        seed_receipt(&pool, buyer_id, game_id, 1000, 50, &"a".repeat(64)).await;

        let analytics = analytics_revenue(State(pool.clone()), Extension(admin_id))
            .await
            .expect("analytics_revenue");

        let row = analytics
            .by_game
            .iter()
            .find(|g| g.game_id == game_id)
            .expect("seeded game must appear in by_game breakdown");

        assert_eq!(row.total_revenue, Decimal::new(1000, 2));
        assert_eq!(row.protocol_fee, Decimal::new(50, 2));
        assert_eq!(row.developer_settled, Decimal::new(950, 2));
        assert_eq!(row.receipt_count, 1);
    }
}
