use axum::http::Method;
use std::time::Duration;
use tower_http::cors::{AllowHeaders, AllowOrigin, Any, CorsLayer};

pub fn cors_layer() -> CorsLayer {
    let allowed_origins = get_allowed_origins();

    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(AllowHeaders::any())
        .expose_headers(Any)
        .max_age(Duration::from_secs(86400))
}

fn get_allowed_origins() -> AllowOrigin {
    if let Ok(origins) = std::env::var("CORS_ALLOWED_ORIGINS") {
        if origins.is_empty() {
            return AllowOrigin::any();
        }
        let origins: Vec<&str> = origins.split(',').map(|s| s.trim()).collect();
        if origins.iter().any(|o| *o == "*") {
            return AllowOrigin::any();
        }
        return AllowOrigin::list(origins.iter().map(|s| s.parse().unwrap()));
    }

    if cfg!(debug_assertions) {
        AllowOrigin::list([
            "http://localhost:5173".parse().unwrap(),
            "http://localhost:3000".parse().unwrap(),
        ])
    } else {
        // Production: default to denying all origins unless FRONTEND_URL is set.
        // Set CORS_ALLOWED_ORIGINS (comma-separated) for a real allowlist, or
        // set FRONTEND_URL as a single-origin fallback.
        if let Ok(frontend_url) = std::env::var("FRONTEND_URL") {
            if let Ok(origin) = frontend_url.parse() {
                return AllowOrigin::list([origin]);
            }
        }
        // No whitelist configured — deny all cross-origin requests in production.
        AllowOrigin::list([])
    }
}
