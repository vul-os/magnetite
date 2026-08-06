// session_revoke_tests.rs — DELETE /api/v1/auth/sessions/:id must be user-scoped.
//
// Bug this guards against: `session::revoke_session(pool, session_id, user_id)`
// (backend/src/services/session.rs) already did the right, safe thing —
// `DELETE FROM sessions WHERE id = $1 AND user_id = $2` — but was never wired
// to a route, so the "Revoke Session" control in the frontend had nothing to
// call. The route added in backend/src/api/auth.rs (`revoke_session`) takes
// `user_id` only from the authenticated `Extension`, never from the path, so
// that a caller cannot pass someone else's session id in the URL and delete
// it (an IDOR). These tests prove:
//   (a) a user can revoke their own session and it is actually gone from the
//       `sessions` table afterwards.
//   (b) a user CANNOT revoke another user's session — the row survives, and
//       the error returned is indistinguishable from revoking a session id
//       that does not exist at all (same `AppError::NotFound`, same message,
//       same 404 mapping — see error.rs), so a caller cannot use the response
//       to probe which session ids belong to someone else.
//
// `#[ignore]`d — needs `DATABASE_URL` against a migrated Postgres, matching
// every other DB-backed integration test in this crate (see
// admin_analytics_tests.rs).
//   DATABASE_URL=postgres://.../magnetite_test cargo test --test session_revoke_tests -- --ignored

#[cfg(test)]
mod live_db_tests {
    use axum::extract::{Extension, Path, State};
    use magnetite_backend::api::auth::revoke_session;
    use magnetite_backend::error::AppError;
    use sqlx::postgres::PgPoolOptions;
    use sqlx::PgPool;
    use uuid::Uuid;

    /// Connects to a real, already-migrated Postgres. Requires `DATABASE_URL`
    /// (see CONTRIBUTING.md). `#[ignore]`d so `cargo test` stays DB-free by
    /// default.
    async fn pool() -> PgPool {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run the #[ignore]d session revoke DB tests");
        PgPoolOptions::new()
            .max_connections(3)
            .connect(&url)
            .await
            .expect("failed to connect to DATABASE_URL")
    }

    async fn seed_user(pool: &PgPool) -> Uuid {
        let id = Uuid::new_v4();
        let username = format!("srtest_{}", id.simple());
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash)
             VALUES ($1, $2, $3, 'x')",
        )
        .bind(id)
        .bind(&username)
        .bind(format!("{username}@example.test"))
        .execute(pool)
        .await
        .expect("seed user");
        id
    }

    async fn seed_session(pool: &PgPool, user_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO sessions (id, user_id, refresh_token_hash, expires_at)
             VALUES ($1, $2, 'irrelevant-hash', NOW() + INTERVAL '7 days')",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("seed session");
        id
    }

    async fn session_exists(pool: &PgPool, session_id: Uuid) -> bool {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1)")
            .bind(session_id)
            .fetch_one(pool)
            .await
            .expect("session_exists query")
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL against a migrated Postgres"]
    async fn a_user_can_revoke_their_own_session() {
        let pool = pool().await;
        let user_id = seed_user(&pool).await;
        let session_id = seed_session(&pool, user_id).await;

        assert!(
            session_exists(&pool, session_id).await,
            "seed session must exist before revoking"
        );

        let result =
            revoke_session(State(pool.clone()), Extension(user_id), Path(session_id)).await;

        assert!(
            result.is_ok(),
            "owner revoking their own session must succeed"
        );
        assert!(
            !session_exists(&pool, session_id).await,
            "session row must be gone from `sessions` after the owner revokes it"
        );
    }

    /// The one that matters: a session id that is real, but belongs to a
    /// different user, must not be revocable — and the failure must look
    /// exactly like "not found", not "forbidden" or anything else that would
    /// confirm the id belongs to someone.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL against a migrated Postgres"]
    async fn a_user_cannot_revoke_another_users_session() {
        let pool = pool().await;
        let victim_id = seed_user(&pool).await;
        let attacker_id = seed_user(&pool).await;
        let victim_session_id = seed_session(&pool, victim_id).await;

        // The attacker calls the route with their own authenticated identity
        // (from `Extension`, exactly as `auth_middleware` would insert it) but
        // the victim's session id in the path.
        let result = revoke_session(
            State(pool.clone()),
            Extension(attacker_id),
            Path(victim_session_id),
        )
        .await;

        assert!(
            result.is_err(),
            "revoking a session that is not the caller's must fail"
        );
        let err_message = match result {
            Err(AppError::NotFound(msg)) => msg,
            Err(other) => panic!(
                "expected AppError::NotFound (so a foreign session id is \
                 indistinguishable from a nonexistent one), got: {other:?}"
            ),
            Ok(_) => unreachable!(),
        };

        assert!(
            session_exists(&pool, victim_session_id).await,
            "the victim's session must still exist — the attacker's request must not have deleted it"
        );

        // Revoking a session id that does not exist at all must produce the
        // exact same error, so the response never leaks "this id belongs to
        // someone else" vs. "this id was never real".
        let nonexistent_result = revoke_session(
            State(pool.clone()),
            Extension(attacker_id),
            Path(Uuid::new_v4()),
        )
        .await;
        let nonexistent_message = match nonexistent_result {
            Err(AppError::NotFound(msg)) => msg,
            Err(other) => {
                panic!("expected AppError::NotFound for a nonexistent session id, got: {other:?}")
            }
            Ok(_) => panic!("revoking a nonexistent session id must not succeed"),
        };
        assert_eq!(
            err_message, nonexistent_message,
            "revoking someone else's session must return the exact same message as \
             revoking a session id that never existed"
        );
    }
}
