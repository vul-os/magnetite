use axum::{body::Body, http::Request, middleware::Next, response::Response};
use std::time::Instant;
use uuid::Uuid;

pub async fn log_request(mut req: Request<Body>, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let request_id = Uuid::new_v4();

    let user_id = req.extensions().get::<Uuid>().copied();
    let ip_address = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h: &axum::http::HeaderValue| h.to_str().ok())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|h: &axum::http::HeaderValue| h.to_str().ok())
        })
        .map(|s: &str| s.to_string());
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h: &axum::http::HeaderValue| h.to_str().ok())
        .map(|s: &str| s.to_string());

    req.extensions_mut().insert(request_id);

    let mut response = next.run(req).await;

    let status = response.status();
    let status_u16 = status.as_u16();

    let headers = response.headers_mut();
    headers.insert(
        "x-request-id",
        axum::http::HeaderValue::from_str(&request_id.to_string())
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("")),
    );

    let log_level = if status_u16 >= 500 {
        "ERROR"
    } else if status_u16 >= 300 {
        // 3xx and 4xx both log as WARN.
        "WARN"
    } else {
        "INFO"
    };

    match log_level {
        "ERROR" => {
            tracing::error!(
                request_id = %request_id,
                method = %method,
                path = %path,
                status = %status_u16,
                duration_ms = %start.elapsed().as_millis(),
                user_id = ?user_id,
                ip_address = ?ip_address,
                user_agent = ?user_agent,
                "Request failed"
            );
        }
        "WARN" => {
            tracing::warn!(
                request_id = %request_id,
                method = %method,
                path = %path,
                status = %status_u16,
                duration_ms = %start.elapsed().as_millis(),
                user_id = ?user_id,
                ip_address = ?ip_address,
                user_agent = ?user_agent,
                "Request completed with non-success status"
            );
        }
        _ => {
            tracing::info!(
                request_id = %request_id,
                method = %method,
                path = %path,
                status = %status_u16,
                duration_ms = %start.elapsed().as_millis(),
                user_id = ?user_id,
                ip_address = ?ip_address,
                user_agent = ?user_agent,
                "Request completed"
            );
        }
    }

    response
}
