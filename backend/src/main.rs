mod api;
mod config;
mod db;
mod error;
mod jobs;
mod middleware;
mod services;
mod ws;

use axum::{
    middleware::{from_fn, from_fn_with_state},
    routing::get,
    Router,
};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::api::achievements;
use crate::api::admin;
use crate::api::auth;
use crate::api::categories;
use crate::api::channels;
use crate::api::communities;
use crate::api::developer;
use crate::api::distribution;
use crate::api::games;
use crate::api::github;
use crate::api::health;
use crate::api::leaderboard;
use crate::api::marketplace;
use crate::api::matchmaking;
use crate::api::messages;
use crate::api::metrics;
use crate::api::notifications;
use crate::api::oauth;
use crate::api::points;
use crate::api::provisioning;
use crate::api::social;
use crate::api::streaming;
use crate::api::subscriptions;
use crate::api::versioning;
use crate::api::wallet;
use crate::api::webhooks;
use crate::jobs::notification_cleanup;
use crate::jobs::session_cleanup;
use crate::jobs::verification_cleanup;
use crate::middleware::cors_layer;
use crate::middleware::logging::log_request;
use crate::middleware::rate_limit::{create_rate_limiter, rate_limit_middleware, RateLimitConfig};
use crate::services::payment::SubscriptionService;
use crate::services::payout::PayoutService;
use crate::ws::comms;
use crate::ws::game as ws_game;
use crate::ws::voice;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = db::get_db_pool().await;
    db::init_db(&pool).await.expect("Failed to run migrations");
    let rate_limit_config = RateLimitConfig::default();
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost".to_string());
    let rate_limiter = create_rate_limiter(&redis_url, rate_limit_config);

    let versioning_state = Arc::new(());

    let api_v1 = Router::new()
        .nest("/auth", auth::router(pool.clone()))
        .nest("/wallet", wallet::router(pool.clone()))
        .nest("/games", games::router(pool.clone()))
        .nest("/distribution", distribution::router(pool.clone()))
        .nest("/provisioning", provisioning::router(pool.clone()))
        .nest("/categories", categories::router(pool.clone()))
        .nest("/leaderboard", leaderboard::router(pool.clone()))
        .nest("/matchmaking", matchmaking::router(pool.clone()))
        .nest("/developer", developer::router(pool.clone()))
        .nest("/admin", admin::router(pool.clone()))
        .nest("/oauth", oauth::router(pool.clone()))
        .nest("/github", github::router(pool.clone()))
        .nest("/webhooks", webhooks::router(pool.clone()))
        .nest("/achievements", achievements::router(pool.clone()))
        .nest("/friends", social::router(pool.clone()))
        .nest("/invites", social::invites_router(pool.clone()))
        .nest("/users", social::users_router(pool.clone()))
        .nest("/subscriptions", subscriptions::router(pool.clone()))
        .nest("/notifications", notifications::router(pool.clone()))
        // Wave 8: points economy + developer marketplace
        .nest("/points", points::router(pool.clone()))
        .nest("/marketplace", marketplace::router(pool.clone()))
        // Wave 6: comms core — communities, channels, messages, DMs
        .nest("/communities", communities::router(pool.clone()))
        .nest(
            "/communities/:community_id/channels",
            channels::router(pool.clone()),
        )
        .nest(
            "/channels/:channel_id/messages",
            messages::channel_messages_router(pool.clone()),
        )
        .nest("/dms", messages::dms_router(pool.clone()))
        // Wave 9: streaming egress + HLS watch
        .nest("/streams", streaming::router(pool.clone()))
        .route("/health", get(health_check))
        .layer(axum::middleware::from_fn_with_state(
            versioning_state.clone(),
            versioning::versioning_middleware,
        ));

    let health_metrics = Router::new()
        .route("/health/ready", get(health::ready))
        .route("/health/live", get(health::live))
        .route("/metrics", get(metrics::metrics))
        .with_state(pool.clone());

    notifications::init_notification_broadcaster().await;
    let notification_broadcaster = Arc::new(notifications::NotificationBroadcaster::new());
    let notification_ws_handler = Arc::new(notifications::NotificationWsHandler::new(
        notification_broadcaster,
    ));

    let game_ws_handler = std::sync::Arc::new(ws_game::GameWsHandler::new(pool.clone()));

    let app = Router::new()
        .nest("/api/v1", api_v1)
        .merge(health_metrics)
        .merge(notification_ws_handler.router())
        // Wave 6: real-time comms and voice signaling WebSocket endpoints
        .merge(comms::router(pool.clone()))
        .merge(voice::router(pool.clone()))
        // Game WebSocket: mount the game loop router (was unmounted — now wired)
        .merge(game_ws_handler.router())
        .layer(cors_layer())
        .layer(from_fn_with_state(
            rate_limiter.clone(),
            rate_limit_middleware,
        ))
        .layer(from_fn(log_request));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("Server running on {}", listener.local_addr().unwrap());

    // ── Background jobs (same tokio::spawn + interval pattern as notification_cleanup) ──
    tokio::spawn(notification_cleanup::run_cleanup_job(pool.clone()));

    // Payout batch: process pending developer payouts every hour.
    let payout_pool = pool.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            ticker.tick().await;
            let svc = PayoutService::new(payout_pool.clone());
            match svc.process_pending_payouts().await {
                Ok(n) if n > 0 => tracing::info!("Processed {} pending payouts", n),
                Ok(_) => {}
                Err(e) => tracing::error!("Payout batch failed: {}", e),
            }
        }
    });

    // Subscription renewal: process expired subscriptions every hour.
    let renewal_pool = pool.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            ticker.tick().await;
            let svc = SubscriptionService::new(renewal_pool.clone());
            match svc.process_renewals().await {
                Ok(n) if n > 0 => tracing::info!("Renewed {} subscriptions", n),
                Ok(_) => {}
                Err(e) => tracing::error!("Subscription renewal failed: {}", e),
            }
        }
    });

    // Session + token cleanup: expire stale sessions, password-reset tokens,
    // matchmaking entries, and unverified accounts every hour (mirrors notification_cleanup).
    tokio::spawn(session_cleanup::run_cleanup_jobs(pool.clone()));

    // Verification-token cleanup: purge expired and old used email/password-reset
    // tokens every hour so the verification_tokens table stays lean.
    tokio::spawn(verification_cleanup::run_cleanup_job(pool.clone()));

    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "ok"
}
