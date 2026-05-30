// Verification-token cleanup job — purges expired and used tokens from
// `verification_tokens` on an interval, matching the notification_cleanup pattern.
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::interval;

use crate::services::verification;

async fn run_single_cleanup(pool: &PgPool) {
    // Remove tokens that have expired without being consumed.
    match verification::cleanup_expired_tokens(pool).await {
        Ok(n) if n > 0 => tracing::info!("Cleaned up {} expired verification tokens", n),
        Ok(_) => {}
        Err(e) => tracing::error!("Verification token cleanup failed: {}", e),
    }

    // Remove used tokens older than 24 hours; they are no longer needed.
    match verification::cleanup_used_tokens(pool, 24).await {
        Ok(n) if n > 0 => tracing::info!("Cleaned up {} used verification tokens", n),
        Ok(_) => {}
        Err(e) => tracing::error!("Used verification token cleanup failed: {}", e),
    }
}

/// Spawn via `tokio::spawn(verification_cleanup::run_cleanup_job(pool))`.
/// Runs every hour, matching the notification_cleanup interval.
pub async fn run_cleanup_job(pool: PgPool) {
    let mut ticker = interval(Duration::from_secs(3600));
    loop {
        ticker.tick().await;
        run_single_cleanup(&pool).await;
    }
}
