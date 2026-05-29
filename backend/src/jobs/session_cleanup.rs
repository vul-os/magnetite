// Session cleanup job — expires stale auth sessions; platform surface, not yet scheduled.
#![allow(dead_code)]

use sqlx::PgPool;
use std::time::Duration;
use tokio::time::interval;

pub async fn cleanup_expired_sessions(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn cleanup_old_matchmaking_entries(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM matchmaking_queue WHERE created_at < NOW() - INTERVAL '1 hour'")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

pub async fn cleanup_expired_password_reset_tokens(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM password_reset_tokens WHERE expires_at < NOW()")
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn cleanup_unverified_accounts(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM users WHERE verified = false AND created_at < NOW() - INTERVAL '7 days'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

async fn run_single_cleanup(pool: &PgPool) {
    let mut cleaned = Vec::new();

    match cleanup_expired_sessions(pool).await {
        Ok(n) if n > 0 => cleaned.push(("sessions", n)),
        Ok(_) => {}
        Err(e) => tracing::error!("Session cleanup failed: {}", e),
    }

    match cleanup_old_matchmaking_entries(pool).await {
        Ok(n) if n > 0 => cleaned.push(("matchmaking_queue", n)),
        Ok(_) => {}
        Err(e) => tracing::error!("Matchmaking queue cleanup failed: {}", e),
    }

    match cleanup_expired_password_reset_tokens(pool).await {
        Ok(n) if n > 0 => cleaned.push(("password_reset_tokens", n)),
        Ok(_) => {}
        Err(e) => tracing::error!("Password reset tokens cleanup failed: {}", e),
    }

    match cleanup_unverified_accounts(pool).await {
        Ok(n) if n > 0 => cleaned.push(("unverified_accounts", n)),
        Ok(_) => {}
        Err(e) => tracing::error!("Unverified accounts cleanup failed: {}", e),
    }

    if !cleaned.is_empty() {
        for (table, count) in cleaned {
            tracing::info!("Cleaned up {} expired {}", count, table);
        }
    }
}

pub async fn run_cleanup_jobs(pool: PgPool) {
    let mut interval = interval(Duration::from_secs(3600));

    loop {
        interval.tick().await;
        run_single_cleanup(&pool).await;
    }
}
