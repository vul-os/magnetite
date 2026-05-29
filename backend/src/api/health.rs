use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sqlx::PgPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct HealthState {
    pub start_time: Instant,
    pub request_counter: Arc<AtomicU64>,
    pub version: String,
}

impl HealthState {
    pub fn new(version: String) -> Self {
        Self {
            start_time: Instant::now(),
            request_counter: Arc::new(AtomicU64::new(0)),
            version,
        }
    }

    pub fn increment_requests(&self) {
        self.request_counter.fetch_add(1, Ordering::Relaxed);
    }
}

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn ready(State(pool): State<PgPool>) -> Json<serde_json::Value> {
    let db_check = sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .is_ok();

    Json(serde_json::json!({
        "ready": db_check,
        "checks": {
            "database": db_check
        }
    }))
}

pub async fn live() -> StatusCode {
    StatusCode::OK
}
