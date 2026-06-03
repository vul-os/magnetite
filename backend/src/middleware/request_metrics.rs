// Request-metrics middleware — records HTTP request count, error count, and latency histogram.
//
// Each request gets:
//   - A UUID request-id inserted into extensions and echoed as `X-Request-Id` response header.
//   - Structured tracing log with method / path-pattern / status / latency / request_id.
//   - Prometheus counters + histogram via the `metrics` facade:
//       http_requests_total{method, path, status}
//       http_request_errors_total{method, path, status}   (status >= 400)
//       http_request_duration_seconds{method, path, status}  (histogram)
//
// The middleware replaces the existing `log_request` fn in `middleware/logging.rs` for
// metrics purposes while keeping the logging behaviour.  Both can coexist: this one
// adds the metrics recording layer on top.

use axum::{
    body::Body,
    extract::MatchedPath,
    http::{Request, Response},
    middleware::Next,
};
use std::time::Instant;
use uuid::Uuid;

/// Axum `from_fn` middleware.  Install it with:
///
/// ```ignore
/// .layer(axum::middleware::from_fn(record_request_metrics))
/// ```
pub async fn record_request_metrics(mut req: Request<Body>, next: Next) -> Response<Body> {
    let start = Instant::now();

    // Assign request-id and inject into extensions so downstream handlers can
    // surface it in their own responses / logs.
    let request_id = Uuid::new_v4();
    req.extensions_mut().insert(request_id);

    let method = req.method().clone();
    let method_str = method.as_str().to_string();

    // Prefer the Axum matched-path pattern (e.g. `/api/v1/games/:id`) over the
    // raw URI so cardinality stays bounded.  Fall back to raw path if the router
    // hasn't matched yet (e.g. 404s).
    let path_pattern = req
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    let mut response = next.run(req).await;

    let status = response.status();
    let status_u16 = status.as_u16();
    let status_str = status_u16.to_string();
    let latency = start.elapsed();
    let latency_secs = latency.as_secs_f64();

    // ── Prometheus metrics ──────────────────────────────────────────────────
    let labels = [
        ("method", method_str.clone()),
        ("path", path_pattern.clone()),
        ("status", status_str.clone()),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_request_duration_seconds", &labels).record(latency_secs);

    if status_u16 >= 400 {
        metrics::counter!("http_request_errors_total", &labels).increment(1);
    }

    // ── Attach X-Request-Id to the response ─────────────────────────────────
    response.headers_mut().insert(
        "x-request-id",
        axum::http::HeaderValue::from_str(&request_id.to_string())
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("")),
    );

    // ── Structured tracing log ───────────────────────────────────────────────
    let log_level = if status_u16 >= 500 {
        "ERROR"
    } else if status_u16 >= 400 {
        "WARN"
    } else {
        "INFO"
    };

    match log_level {
        "ERROR" => tracing::error!(
            request_id = %request_id,
            method = %method_str,
            path = %path_pattern,
            status = %status_u16,
            duration_ms = %latency.as_millis(),
            "request failed"
        ),
        "WARN" => tracing::warn!(
            request_id = %request_id,
            method = %method_str,
            path = %path_pattern,
            status = %status_u16,
            duration_ms = %latency.as_millis(),
            "request completed with non-success status"
        ),
        _ => tracing::info!(
            request_id = %request_id,
            method = %method_str,
            path = %path_pattern,
            status = %status_u16,
            duration_ms = %latency.as_millis(),
            "request completed"
        ),
    }

    response
}
