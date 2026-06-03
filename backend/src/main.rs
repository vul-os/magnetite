mod api;
mod config;
mod db;
mod error;
mod jobs;
mod middleware;
mod services;
mod superadmin;
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
use crate::api::platform;
use crate::api::points;
use crate::api::profile;
use crate::api::provisioning;
use crate::api::replays;
use crate::api::reviews;
use crate::api::search;
use crate::api::social;
use crate::api::streaming;
use crate::api::subscriptions;
use crate::api::templates;
use crate::api::tournaments;
use crate::api::versioning;
use crate::api::wallet;
use crate::api::webhooks;
use crate::api::wishlist;
use crate::jobs::backup;
use crate::jobs::notification_cleanup;
use crate::jobs::session_cleanup;
use crate::jobs::verification_cleanup;
use crate::middleware::cors_layer;
use crate::middleware::logging::log_request;
use crate::middleware::rate_limit::{create_rate_limiter, rate_limit_middleware, RateLimitConfig};
use crate::middleware::request_metrics::record_request_metrics;
use crate::services::payment::SubscriptionService;
use crate::services::payout::PayoutService;
use crate::ws::comms;
use crate::ws::game as ws_game;
use crate::ws::gauges::WsGauges;
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

    // ── Prometheus metrics recorder ─────────────────────────────────────────
    // install_recorder() installs the recorder as the global `metrics` facade
    // and returns a handle that can render Prometheus text on demand (used by
    // GET /metrics).  No background HTTP listener is started — we expose the
    // scrape endpoint through Axum instead.
    let prom_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder");

    let ws_gauges = Arc::new(WsGauges::new());
    let metrics_state = Arc::new(metrics::MetricsState {
        handle: prom_handle,
        pool: pool.clone(),
        ws_gauges: Arc::clone(&ws_gauges),
    });

    let versioning_state = Arc::new(());

    let api_v1 = Router::new()
        .nest("/auth", auth::router(pool.clone()))
        .nest("/wallet", wallet::router(pool.clone()))
        .nest(
            "/games",
            games::router(pool.clone()).merge(reviews::router(pool.clone())),
        )
        // Contact route at /api/v1/contact (not nested under /games)
        .route(
            "/contact",
            axum::routing::post(reviews::submit_contact).with_state(pool.clone()),
        )
        .nest("/distribution", distribution::router(pool.clone()))
        .nest("/provisioning", provisioning::router(pool.clone()))
        .nest("/categories", categories::router(pool.clone()))
        .nest("/leaderboard", leaderboard::router(pool.clone()))
        .nest("/matchmaking", matchmaking::router(pool.clone()))
        .nest("/developer", developer::router(pool.clone()))
        .nest("/templates", templates::router())
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
        // Stores namespace — mirrors /marketplace/stores/* so frontend client.stores.* calls resolve
        .nest("/stores", marketplace::stores_router(pool.clone()))
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
        // Community-scoped streams: GET/POST /communities/:id/streams
        .nest(
            "/communities/:community_id/streams",
            streaming::community_streams_router(pool.clone()),
        )
        // Voice rooms REST: GET /communities/:id/voice-rooms (in communities router above)
        // POST /voice-rooms/:id/join (returns room_token for WS connection)
        .nest(
            "/voice-rooms",
            communities::voice_rooms_router(pool.clone()),
        )
        // Profile API: GET/PUT /profile/me, public /profile/:id
        .nest("/profile", profile::router(pool.clone()))
        // Wishlist: GET/POST/DELETE /wishlist/:game_id
        .nest("/wishlist", wishlist::router(pool.clone()))
        // Search: GET /search?q=&search_type=&limit=&offset=
        .nest("/search", search::router(pool.clone()))
        // Users by-username: GET /users/by-username/:username (already in users_router above)
        // Platform settings (admin-only write, public read)
        .nest("/platform", platform::router(pool.clone()))
        // Tournaments: bracket management
        .nest("/tournaments", tournaments::router(pool.clone()))
        // Replays: store + serve authoritative match ReplayLogs
        .nest("/replays", replays::router(pool.clone()))
        // Replay list scoped under a game: GET /api/v1/games/:id/replays
        .nest(
            "/games/:id/replays",
            replays::game_replays_router(pool.clone()),
        )
        .route("/health", get(health_check))
        .layer(axum::middleware::from_fn_with_state(
            versioning_state.clone(),
            versioning::versioning_middleware,
        ));

    let health_ready: Router = Router::new()
        .route("/health/ready", get(health::ready))
        .with_state(pool.clone());
    let health_live: Router = Router::new().route("/health/live", get(health::live));
    let metrics_router: Router = Router::new()
        .route("/metrics", get(metrics::metrics))
        .with_state(Arc::clone(&metrics_state));
    let health_metrics = health_ready.merge(health_live).merge(metrics_router);

    notifications::init_notification_broadcaster().await;
    let notification_broadcaster = Arc::new(notifications::NotificationBroadcaster::new());
    let notification_ws_handler = Arc::new(notifications::NotificationWsHandler::new(
        notification_broadcaster,
    ));

    let game_ws_handler = std::sync::Arc::new(ws_game::GameWsHandler::new(
        pool.clone(),
        Arc::clone(&ws_gauges),
    ));

    // ── In-house analytics + super-admin control surface ────────────────────
    // One shared offline GeoIP resolver feeds both the analytics recorder (IP
    // enrichment) and the super-admin panel (geo display).
    let geo = Arc::new(superadmin::GeoResolver::from_env());
    let trust_proxy = std::env::var("TRUST_PROXY")
        .map(|v| v == "true")
        .unwrap_or(false);
    let analytics_state = Arc::new(superadmin::analytics::AnalyticsState::from_env(
        pool.clone(),
        Arc::clone(&geo),
        trust_proxy,
    ));

    let mut app = Router::new()
        .nest("/api/v1", api_v1)
        .merge(health_metrics)
        .merge(notification_ws_handler.router())
        // Wave 6: real-time comms and voice signaling WebSocket endpoints
        .merge(comms::router(pool.clone(), Arc::clone(&ws_gauges)))
        .merge(voice::router(pool.clone(), Arc::clone(&ws_gauges)))
        // Game WebSocket: mount the game loop router (was unmounted — now wired)
        .merge(game_ws_handler.router());

    // Hardened super-admin panel — mounted only when a super credential is set.
    match superadmin::router(pool.clone(), Arc::clone(&geo)) {
        Some(sa) => app = app.nest("/superadmin", sa),
        None => tracing::info!(
            "Super-admin panel disabled (set SUPERADMIN_EMAIL + SUPERADMIN_PASSWORD_HASH to enable)"
        ),
    }

    let app = app
        .layer(cors_layer())
        .layer(from_fn_with_state(
            rate_limiter.clone(),
            rate_limit_middleware,
        ))
        .layer(from_fn(log_request))
        .layer(from_fn(record_request_metrics))
        // In-house request analytics (skips infra/static/superadmin paths).
        .layer(from_fn_with_state(
            Arc::clone(&analytics_state),
            superadmin::analytics::record_analytics,
        ));

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

    // Database backup: pg_dump + S3/local storage every 6 hours.
    // Storage type controlled by BACKUP_STORAGE_TYPE (default: "local").
    // S3 path requires BACKUP_S3_BUCKET + BACKUP_S3_REGION; local uses BACKUP_LOCAL_DIR.
    let backup_pool = pool.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
        loop {
            ticker.tick().await;
            match backup::create_backup(&backup_pool).await {
                Ok(filename) => tracing::info!("Backup completed: {}", filename),
                Err(e) => tracing::warn!("Backup failed (non-fatal): {}", e),
            }
        }
    });

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn health_check() -> &'static str {
    "ok"
}
