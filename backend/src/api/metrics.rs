// Metrics API — Prometheus-format scrape endpoint.
//
// GET /metrics returns text/plain; version=0.0.4 (the standard Prometheus exposition format).
//
// Metrics exposed:
//   http_requests_total{method,path,status}            — counter
//   http_request_errors_total{method,path,status}      — counter  (status >= 400)
//   http_request_duration_seconds{method,path,status}  — histogram (buckets: default)
//   active_websocket_connections                        — gauge
//   active_game_sessions                               — gauge
//   db_pool_size                                        — gauge
//   db_pool_idle                                        — gauge
//
// The WS/session/DB gauges are refreshed synchronously on every scrape so they always
// reflect the live state at scrape time (no background updater needed).

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use std::sync::{atomic::Ordering, Arc};

use crate::ws::gauges::WsGauges;

pub struct MetricsState {
    pub handle: PrometheusHandle,
    pub pool: PgPool,
    pub ws_gauges: Arc<WsGauges>,
}

/// GET /metrics — Prometheus scrape endpoint.
pub async fn metrics(State(state): State<Arc<MetricsState>>) -> impl IntoResponse {
    // Refresh live gauges synchronously right before rendering so Prometheus
    // always sees up-to-date values.
    let ws_conns = state.ws_gauges.ws_connections.load(Ordering::Relaxed);
    let game_sessions = state.ws_gauges.game_sessions.load(Ordering::Relaxed);
    let pool_size = state.pool.size() as u64;
    let pool_idle = state.pool.num_idle() as u64;

    metrics::gauge!("active_websocket_connections").set(ws_conns as f64);
    metrics::gauge!("active_game_sessions").set(game_sessions as f64);
    metrics::gauge!("db_pool_size").set(pool_size as f64);
    metrics::gauge!("db_pool_idle").set(pool_idle as f64);

    let body = state.handle.render();

    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}
