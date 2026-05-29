use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use sqlx::PgPool;
use std::sync::atomic::Ordering;

pub fn create_metrics_router(pool: PgPool) -> Router<PgPool> {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(pool)
}

pub async fn metrics(State(pool): State<PgPool>) -> Json<serde_json::Value> {
    let db_pool_size = pool.size() as u64;
    let db_idle_connections = pool.num_idle() as u64;

    Json(serde_json::json!({
        "db_pool_size": db_pool_size,
        "db_idle_connections": db_idle_connections
    }))
}