use axum::{
    extract::{Request, State},
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

pub const HEADER_API_DEPRECATED: &str = "X-API-Deprecated";
pub const HEADER_API_SUNSET: &str = "X-API-Sunset";
pub const HEADER_API_VERSION: &str = "X-API-Version";
pub const CURRENT_VERSION: &str = "1";
pub const SUNSET_DATE: &str = "2025-12-31";

pub const ACCEPT_HEADER_PREFIX: &str = "application/vnd.magnetite.v";
pub const ACCEPT_HEADER_SUFFIX: &str = "+json";

#[derive(Clone)]
pub struct ApiVersion {
    pub version: String,
    pub deprecated: bool,
}

impl Default for ApiVersion {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION.to_string(),
            deprecated: false,
        }
    }
}

pub fn parse_version(accept_header: Option<&str>, query_version: Option<&str>) -> ApiVersion {
    if let Some(v) = query_version {
        if let Ok(version) = v.parse::<i32>() {
            if version == 1 {
                return ApiVersion {
                    version: "1".to_string(),
                    deprecated: false,
                };
            }
        }
    }

    if let Some(accept) = accept_header {
        if let Some(version_str) = extract_version_from_accept(accept) {
            if let Ok(version) = version_str.parse::<i32>() {
                if version == 1 {
                    return ApiVersion {
                        version: "1".to_string(),
                        deprecated: false,
                    };
                }
            }
        }
    }

    ApiVersion::default()
}

fn extract_version_from_accept(accept: &str) -> Option<&str> {
    if !accept.starts_with(ACCEPT_HEADER_PREFIX) {
        return None;
    }

    let without_prefix = &accept[ACCEPT_HEADER_PREFIX.len()..];
    if let Some(suffix_pos) = without_prefix.find(ACCEPT_HEADER_SUFFIX) {
        return Some(&without_prefix[..suffix_pos]);
    }

    None
}

pub async fn versioning_middleware(
    State(_state): State<Arc<()>>,
    mut request: Request,
    next: Next,
) -> Response {
    let accept_header = request
        .headers()
        .get("Accept")
        .and_then(|v| v.to_str().ok());

    let query_version = request.uri().query().and_then(|q| {
        q.split('&')
            .find(|p| p.starts_with("version="))
            .map(|p| p.trim_start_matches("version="))
    });

    let api_version = parse_version(accept_header, query_version);
    request.extensions_mut().insert(api_version.clone());

    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert(
        HEADER_API_VERSION,
        HeaderValue::try_from(api_version.version.as_str())
            .unwrap_or_else(|_| HeaderValue::from_static("1")),
    );

    if api_version.deprecated {
        headers.insert(HEADER_API_DEPRECATED, HeaderValue::from_static("true"));
        headers.insert(HEADER_API_SUNSET, HeaderValue::from_static(SUNSET_DATE));
    }

    response
}
