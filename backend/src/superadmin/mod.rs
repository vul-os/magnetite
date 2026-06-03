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
/// Shares the `GeoResolver` with the analytics recorder.
pub fn router(pool: PgPool, geo: Arc<GeoResolver>) -> Option<Router> {
    let config = SuperAdminConfig::from_env()?;
    tracing::info!(
        "Super-admin panel enabled at /superadmin (ip-allowlist: {}, secure-cookie: {})",
        if std::env::var("SUPERADMIN_IP_ALLOWLIST")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
        {
            "on"
        } else {
            "off"
        },
        config.secure_cookie,
    );

    let state = Arc::new(SuperAdminState {
        pool,
        config,
        sessions: auth::SessionStore::new(),
        guard: auth::LoginGuard::new(),
        geo,
    });

    // Periodic sweep of expired in-memory sessions.
    let sweep_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            ticker.tick().await;
            sweep_state.sessions.sweep();
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
    let token = auth::token_from_cookies(request.headers());
    match token.and_then(|t| state.sessions.get(&t)) {
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
    if let Err(secs) = state.guard.check(&ip) {
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
        state.guard.record_success(&ip);
        let (token, _session) =
            state
                .sessions
                .create(&state.config.email, &ip, state.config.session_ttl);
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
        state.guard.record_failure(&ip);
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
        state.sessions.remove(&token);
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
