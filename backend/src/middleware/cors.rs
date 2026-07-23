use axum::http::Method;
use std::time::Duration;
use tower_http::cors::{AllowHeaders, AllowOrigin, Any, CorsLayer};

pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(resolved_allow_origin())
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

/// The three shapes an allow-origin decision can take. Kept separate from the
/// `AllowOrigin` mapping (which `tower_http` makes opaque) so the policy logic
/// is unit-testable.
#[derive(Debug, PartialEq, Eq)]
enum OriginPolicy {
    /// Allow any origin (`*`) — only when explicitly requested.
    Any,
    /// Allow exactly this allowlist.
    List(Vec<String>),
    /// Deny all cross-origin requests.
    Deny,
}

/// Decide the origin policy from configuration.
///
/// Security-critical defaults:
///   * An explicit `*` in `CORS_ALLOWED_ORIGINS` → allow-any (the operator's
///     deliberate choice).
///   * A non-empty allowlist → exactly that list.
///   * A BLANK or unset `CORS_ALLOWED_ORIGINS` → the safe fallback, NOT
///     allow-any: localhost in debug builds, else the single `FRONTEND_URL`
///     if set, else deny-all. A declared-but-empty env var (a common
///     deploy mistake) must never silently become an any-origin free-for-all
///     in production.
fn resolve_origin_policy(cors: Option<&str>, frontend: Option<&str>, is_debug: bool) -> OriginPolicy {
    if let Some(raw) = cors {
        let origins: Vec<String> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if origins.iter().any(|o| o == "*") {
            return OriginPolicy::Any;
        }
        if !origins.is_empty() {
            return OriginPolicy::List(origins);
        }
        // blank / whitespace-only value → fall through to the safe default.
    }

    if is_debug {
        OriginPolicy::List(vec![
            "http://localhost:5173".to_string(),
            "http://localhost:3000".to_string(),
        ])
    } else if let Some(f) = frontend.map(str::trim).filter(|s| !s.is_empty()) {
        OriginPolicy::List(vec![f.to_string()])
    } else {
        OriginPolicy::Deny
    }
}

fn resolved_allow_origin() -> AllowOrigin {
    let cors = std::env::var("CORS_ALLOWED_ORIGINS").ok();
    let frontend = std::env::var("FRONTEND_URL").ok();
    match resolve_origin_policy(cors.as_deref(), frontend.as_deref(), cfg!(debug_assertions)) {
        OriginPolicy::Any => AllowOrigin::any(),
        OriginPolicy::Deny => AllowOrigin::list([]),
        OriginPolicy::List(origins) => {
            // Skip malformed origins with a warning rather than panicking the
            // whole server on a single typo in the allowlist. A list that
            // parses to nothing then denies (empty allowlist), never falls
            // back to allow-any.
            let parsed: Vec<_> = origins
                .iter()
                .filter_map(|o| match o.parse() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        tracing::warn!(origin = %o, "ignoring malformed CORS origin");
                        None
                    }
                })
                .collect();
            AllowOrigin::list(parsed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_star_allows_any() {
        assert_eq!(resolve_origin_policy(Some("*"), None, false), OriginPolicy::Any);
        assert_eq!(
            resolve_origin_policy(Some("http://a.com, *"), None, false),
            OriginPolicy::Any
        );
    }

    #[test]
    fn nonempty_list_is_used_verbatim() {
        assert_eq!(
            resolve_origin_policy(Some("http://a.com, http://b.com"), None, false),
            OriginPolicy::List(vec!["http://a.com".into(), "http://b.com".into()]),
        );
    }

    #[test]
    fn blank_cors_var_never_allows_any_in_production() {
        // The footgun this guards: leaving CORS_ALLOWED_ORIGINS="" must give
        // deny-all in production, never allow-any.
        assert_eq!(resolve_origin_policy(Some(""), None, false), OriginPolicy::Deny);
        assert_eq!(resolve_origin_policy(Some("   "), None, false), OriginPolicy::Deny);
        assert_eq!(resolve_origin_policy(Some(", ,"), None, false), OriginPolicy::Deny);
    }

    #[test]
    fn unset_falls_back_to_frontend_then_deny_in_production() {
        assert_eq!(resolve_origin_policy(None, None, false), OriginPolicy::Deny);
        assert_eq!(
            resolve_origin_policy(None, Some("https://app.example"), false),
            OriginPolicy::List(vec!["https://app.example".into()]),
        );
        // A non-empty CORS allowlist still wins over FRONTEND_URL.
        assert_eq!(
            resolve_origin_policy(Some("http://a.com"), Some("https://app.example"), false),
            OriginPolicy::List(vec!["http://a.com".into()]),
        );
        // A blank FRONTEND_URL is treated as unset → deny.
        assert_eq!(resolve_origin_policy(None, Some("  "), false), OriginPolicy::Deny);
    }

    #[test]
    fn debug_builds_default_to_localhost() {
        let localhost = OriginPolicy::List(vec![
            "http://localhost:5173".into(),
            "http://localhost:3000".into(),
        ]);
        assert_eq!(resolve_origin_policy(None, None, true), localhost);
        // A blank var in debug also falls back to localhost, not any.
        assert_eq!(resolve_origin_policy(Some(""), None, true), localhost);
    }
}
