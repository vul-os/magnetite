// Hardened super-admin control surface.
//
// A separate, server-rendered surface mounted at `/superadmin` — distinct from
// the JSON `/api/v1/admin` API and the React SPA. It authenticates a single
// env-provisioned super credential, manages users/games/money, and exposes the
// in-house analytics and billing-compliance reports.
//
// The whole surface is disabled (returns 404) unless `SUPERADMIN_EMAIL` and a
// password/hash are configured, so it adds no attack surface when unused.
//
// Security layering (outermost first):
//   1. IP allowlist + strict security headers (every route, incl. login)
//   2. session guard (protected routes only) → injects the `Session`
//   3. CSRF validation inside each mutating handler

pub mod analytics;
mod auth;
mod billing;
mod geo;
mod html;
mod pages;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, Request, Response, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::IntoResponse,
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use sqlx::PgPool;

pub use auth::{client_ip, SuperAdminConfig};
pub use geo::GeoResolver;

const CSRF_COOKIE: &str = "mag_sa_csrf";

pub struct SuperAdminState {
    pub pool: PgPool,
    pub config: SuperAdminConfig,
    pub sessions: auth::SessionStore,
    pub guard: auth::LoginGuard,
    pub geo: Arc<GeoResolver>,
}

/// Build the super-admin router, or `None` if no super credential is configured.
/// Shares the `GeoResolver` with the analytics recorder. Sessions + lockout use
/// Redis when reachable (multi-replica safe, survives restart), else in-memory.
pub async fn router(pool: PgPool, geo: Arc<GeoResolver>) -> Option<Router> {
    let config = SuperAdminConfig::from_env()?;

    let redis = build_session_redis().await;
    let sessions = auth::SessionStore::new(redis.clone());
    let guard = auth::LoginGuard::new(redis);

    tracing::info!(
        "Super-admin panel enabled at /superadmin (ip-allowlist: {}, secure-cookie: {}, session-backend: {})",
        if std::env::var("SUPERADMIN_IP_ALLOWLIST")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
        {
            "on"
        } else {
            "off"
        },
        config.secure_cookie,
        sessions.backend_name(),
    );

    let state = Arc::new(SuperAdminState {
        pool,
        config,
        sessions,
        guard,
        geo,
    });

    // Periodic sweep of expired sessions (TTL also handles this in Redis).
    let sweep_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            ticker.tick().await;
            sweep_state.sessions.sweep().await;
        }
    });

    let protected = Router::new()
        .route("/", get(pages::overview))
        .route("/users", get(pages::users_list))
        .route("/users/:id", get(pages::user_detail))
        .route("/users/:id/ban", post(pages::user_ban))
        .route("/users/:id/role", post(pages::user_role))
        .route("/games", get(pages::games_list))
        .route("/games/:id/approve", post(pages::game_approve))
        .route("/games/:id/feature", post(pages::game_feature))
        .route("/transactions", get(pages::transactions))
        .route("/billing", get(pages::billing_page))
        .route("/analytics", get(pages::analytics_page))
        .route("/audit", get(pages::audit_page))
        .route("/logout", get(logout))
        .route_layer(from_fn_with_state(Arc::clone(&state), require_session));

    let public = Router::new().route("/login", get(login_form).post(login_submit));

    let router = protected
        .merge(public)
        .with_state(Arc::clone(&state))
        .layer(from_fn_with_state(
            Arc::clone(&state),
            security_and_allowlist,
        ));

    Some(router)
}

// ── Security + allowlist layer (all routes) ─────────────────────────────────

fn apply_security_headers(resp: &mut Response<Body>) {
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        header::HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; img-src data:; \
             form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
    h.insert(
        header::X_FRAME_OPTIONS,
        header::HeaderValue::from_static("DENY"),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    h.insert(
        header::HeaderName::from_static("permissions-policy"),
        header::HeaderValue::from_static("geolocation=(), camera=(), microphone=()"),
    );
    h.insert(
        header::HeaderName::from_static("cross-origin-opener-policy"),
        header::HeaderValue::from_static("same-origin"),
    );
}

async fn security_and_allowlist(
    State(state): State<Arc<SuperAdminState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let ip = client_ip(request.headers(), Some(peer.ip()), state.config.trust_proxy);
    if !state.config.ip_allowed(&ip) {
        let mut resp = (StatusCode::FORBIDDEN, "Forbidden").into_response();
        apply_security_headers(&mut resp);
        return resp;
    }
    let mut resp = next.run(request).await;
    apply_security_headers(&mut resp);
    resp
}

// ── Session guard (protected routes) ────────────────────────────────────────

async fn require_session(
    State(state): State<Arc<SuperAdminState>>,
    mut request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let session = match auth::token_from_cookies(request.headers()) {
        Some(token) => state.sessions.get(&token).await,
        None => None,
    };
    match session {
        Some(session) => {
            request.extensions_mut().insert(session);
            next.run(request).await
        }
        None => redirect_to("/superadmin/login"),
    }
}

// ── Login / logout ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub err: Option<String>,
}

async fn login_form(
    State(state): State<Arc<SuperAdminState>>,
    axum::extract::Query(q): axum::extract::Query<LoginQuery>,
) -> Response<Body> {
    let csrf = auth::random_token();
    let secure = if state.config.secure_cookie {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!(
        "{CSRF_COOKIE}={csrf}; HttpOnly; SameSite=Strict; Path=/superadmin; Max-Age=600{secure}"
    );
    let body = html::login_page(q.err.as_deref(), &csrf);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::SET_COOKIE, cookie)
        .body(Body::from(body))
        .unwrap()
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    pub csrf: String,
}

async fn login_submit(
    State(state): State<Arc<SuperAdminState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response<Body> {
    let ip = client_ip(&headers, Some(peer.ip()), state.config.trust_proxy);

    // Brute-force lockout.
    if let Err(secs) = state.guard.check(&ip).await {
        audit_event(
            &state.pool,
            &state.config.email,
            &ip,
            "login",
            "",
            &format!("locked {secs}s"),
            "denied",
        )
        .await;
        return redirect_to("/superadmin/login?err=Too+many+attempts.+Try+again+later.");
    }

    // CSRF double-submit: cookie value must match the submitted field.
    let cookie_csrf = auth::cookie_value(&headers, CSRF_COOKIE).unwrap_or_default();
    if cookie_csrf.is_empty() || !auth::ct_eq(cookie_csrf.as_bytes(), form.csrf.as_bytes()) {
        return redirect_to("/superadmin/login?err=Session+expired,+please+retry.");
    }

    if state.config.verify_credentials(&form.email, &form.password) {
        state.guard.record_success(&ip).await;
        let (token, _session) = state
            .sessions
            .create(&state.config.email, &ip, state.config.session_ttl)
            .await;
        audit_event(
            &state.pool,
            &state.config.email,
            &ip,
            "login",
            "",
            "success",
            "ok",
        )
        .await;
        let mut resp = redirect_to("/superadmin");
        let h = resp.headers_mut();
        h.append(
            header::SET_COOKIE,
            state.config.set_cookie_header(&token).parse().unwrap(),
        );
        // Clear the one-shot CSRF cookie.
        let secure = if state.config.secure_cookie {
            "; Secure"
        } else {
            ""
        };
        h.append(
            header::SET_COOKIE,
            format!(
                "{CSRF_COOKIE}=; HttpOnly; SameSite=Strict; Path=/superadmin; Max-Age=0{secure}"
            )
            .parse()
            .unwrap(),
        );
        resp
    } else {
        state.guard.record_failure(&ip).await;
        audit_event(
            &state.pool,
            &form.email,
            &ip,
            "login",
            "",
            "bad credentials",
            "denied",
        )
        .await;
        redirect_to("/superadmin/login?err=Invalid+credentials.")
    }
}

async fn logout(State(state): State<Arc<SuperAdminState>>, headers: HeaderMap) -> Response<Body> {
    if let Some(token) = auth::token_from_cookies(&headers) {
        state.sessions.remove(&token).await;
    }
    let mut resp = redirect_to("/superadmin/login?err=Signed+out.");
    resp.headers_mut().append(
        header::SET_COOKIE,
        state.config.clear_cookie_header().parse().unwrap(),
    );
    resp
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn redirect_to(location: &str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, location)
        .body(Body::empty())
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
async fn audit_event(
    pool: &PgPool,
    actor_email: &str,
    actor_ip: &str,
    action: &str,
    target: &str,
    detail: &str,
    outcome: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO superadmin_audit_log (actor_email, actor_ip, action, target, detail, outcome)
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(actor_email)
    .bind(actor_ip)
    .bind(action)
    .bind(target)
    .bind(detail)
    .bind(outcome)
    .execute(pool)
    .await;
}

#[cfg(test)]
mod route_tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::Request;
    use std::net::SocketAddr;
    use tower::ServiceExt;

    // Lazy pool pointed at a closed port so any DB call fails fast (handlers that
    // touch the DB only run after auth; the routes we test never reach a query, or
    // ignore its result as with the audit insert).
    fn test_pool() -> PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(150))
            .connect_lazy("postgres://127.0.0.1:1/none")
            .unwrap()
    }

    async fn build(extra: &[(&str, Option<&str>)]) -> Router {
        let mut vars: Vec<(&str, Option<&str>)> = vec![
            ("SUPERADMIN_EMAIL", Some("admin@x.com")),
            ("SUPERADMIN_PASSWORD", Some("secret")),
            ("SUPERADMIN_PASSWORD_HASH", None),
            ("SUPERADMIN_IP_ALLOWLIST", None),
            ("SUPERADMIN_SESSION_BACKEND", Some("memory")),
            ("GEOIP_DB_PATH", None),
        ];
        vars.extend_from_slice(extra);
        temp_env::async_with_vars(vars, async {
            router(test_pool(), Arc::new(GeoResolver::from_env()))
                .await
                .expect("panel should be enabled")
        })
        .await
    }

    fn request(method: &str, uri: &str) -> Request<Body> {
        let mut r = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        r.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        r
    }

    fn header(resp: &Response<Body>, name: &str) -> String {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    }

    fn set_cookies(resp: &Response<Body>) -> String {
        resp.headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
            .join("; ")
    }

    #[tokio::test]
    async fn login_form_renders_and_sets_csrf_cookie() {
        let app = build(&[]).await;
        let resp = app.oneshot(request("GET", "/login")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(set_cookies(&resp).contains(CSRF_COOKIE));
        // Strict security headers are applied to every response.
        assert_eq!(header(&resp, "x-frame-options"), "DENY");
        assert!(header(&resp, "content-security-policy").contains("default-src 'none'"));
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&body).contains("Magnetite Control"));
    }

    #[tokio::test]
    async fn protected_route_without_session_redirects_to_login() {
        let app = build(&[]).await;
        let resp = app.oneshot(request("GET", "/")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert_eq!(header(&resp, "location"), "/superadmin/login");
    }

    #[tokio::test]
    async fn ip_allowlist_blocks_disallowed_source() {
        let app = build(&[("SUPERADMIN_IP_ALLOWLIST", Some("10.0.0.0/8"))]).await;
        // Request originates from 127.0.0.1 (set in `request`), outside 10/8.
        let resp = app.oneshot(request("GET", "/login")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn ip_allowlist_permits_listed_source() {
        let app = build(&[("SUPERADMIN_IP_ALLOWLIST", Some("127.0.0.0/8"))]).await;
        let resp = app.oneshot(request("GET", "/login")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn login_rejects_csrf_mismatch() {
        let app = build(&[]).await;
        let mut req = Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", "mag_sa_csrf=aaa")
            .body(Body::from("email=admin%40x.com&password=secret&csrf=bbb"))
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert!(header(&resp, "location").contains("err="));
    }

    #[tokio::test]
    async fn login_succeeds_and_sets_session_cookie() {
        let app = build(&[]).await;
        let mut req = Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", "mag_sa_csrf=tok123")
            .body(Body::from(
                "email=admin%40x.com&password=secret&csrf=tok123",
            ))
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert_eq!(header(&resp, "location"), "/superadmin");
        assert!(set_cookies(&resp).contains(auth::COOKIE));
    }

    #[tokio::test]
    async fn login_fails_with_wrong_password() {
        let app = build(&[]).await;
        let mut req = Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", "mag_sa_csrf=tok123")
            .body(Body::from("email=admin%40x.com&password=wrong&csrf=tok123"))
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert!(header(&resp, "location").contains("Invalid+credentials"));
    }
}

/// Build a Redis connection for session + lockout storage when reachable.
/// Honoured opt-out: `SUPERADMIN_SESSION_BACKEND=memory` forces in-memory.
/// Otherwise uses `REDIS_URL` if a connection succeeds; falls back to in-memory.
async fn build_session_redis() -> Option<redis::aio::ConnectionManager> {
    if std::env::var("SUPERADMIN_SESSION_BACKEND").as_deref() == Ok("memory") {
        return None;
    }
    let url = std::env::var("REDIS_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())?;
    let client = redis::Client::open(url).ok()?;
    match redis::aio::ConnectionManager::new(client).await {
        Ok(cm) => Some(cm),
        Err(e) => {
            tracing::warn!(
                "super-admin: Redis session backend unavailable ({e}); using in-memory sessions"
            );
            None
        }
    }
}
