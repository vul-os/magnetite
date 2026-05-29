use sqlx::PgPool;
use std::time::Duration;
use tokio::time::interval;

pub async fn cleanup_old_notifications(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM notifications WHERE created_at < NOW() - INTERVAL '30 days'")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

pub async fn cleanup_read_notifications(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM notifications WHERE read = true AND created_at < NOW() - INTERVAL '7 days'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

async fn run_notification_cleanup(pool: &PgPool) {
    match cleanup_old_notifications(pool).await {
        Ok(n) if n > 0 => tracing::info!("Cleaned up {} old notifications", n),
        Ok(_) => {}
        Err(e) => tracing::error!("Notification cleanup failed: {}", e),
    }

    match cleanup_read_notifications(pool).await {
        Ok(n) if n > 0 => tracing::info!("Cleaned up {} read notifications", n),
        Ok(_) => {}
        Err(e) => tracing::error!("Read notification cleanup failed: {}", e),
    }
}

pub async fn run_cleanup_job(pool: PgPool) {
    let mut interval = interval(Duration::from_secs(3600));

    loop {
        interval.tick().await;
        run_notification_cleanup(&pool).await;
    }
}
