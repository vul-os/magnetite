// Super-admin page + action handlers (server-rendered HTML).
//
// Every handler takes the shared `SuperAdminState` and the authenticated
// `Session` (injected by the session-guard middleware). Mutations validate a
// CSRF token, write an audit-log row, then 303-redirect back with a flash.

use std::sync::Arc;

use axum::{
    extract::{Form, Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::auth::{ct_eq, Session};
use super::billing::{self, Severity};
use super::html::{self, esc, pill, Nav};
use super::{analytics, SuperAdminState};

type Resp = axum::response::Response;

// ── shared helpers ──────────────────────────────────────────────────────────

fn nav(active: &str) -> Vec<Nav<'static>> {
    let items = [
        ("/superadmin", "Overview", "overview"),
        ("/superadmin/users", "Users", "users"),
        ("/superadmin/games", "Games", "games"),
        ("/superadmin/transactions", "Money", "money"),
        ("/superadmin/billing", "Billing compliance", "billing"),
        ("", "", "sep"),
        ("/superadmin/analytics", "Analytics", "analytics"),
        ("/superadmin/audit", "Audit log", "audit"),
        ("", "", "sep2"),
        ("/superadmin/logout", "Sign out", "logout"),
    ];
    items
        .iter()
        .map(|(href, label, key)| Nav {
            href,
            label,
            active: *key == active,
        })
        .collect()
}

fn money(d: Decimal) -> String {
    format!("${}", d.round_dp(2))
}

fn dt(d: DateTime<Utc>) -> String {
    d.format("%Y-%m-%d %H:%M").to_string()
}

fn flash(q: &FlashQuery) -> String {
    if let Some(m) = &q.msg {
        format!("<div class=\"flash ok\">{}</div>", esc(m))
    } else if let Some(e) = &q.err {
        format!("<div class=\"flash err\">{}</div>", esc(e))
    } else {
        String::new()
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct FlashQuery {
    pub msg: Option<String>,
    pub err: Option<String>,
}

async fn audit(
    pool: &PgPool,
    sess: &Session,
    action: &str,
    target: &str,
    detail: &str,
    outcome: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO superadmin_audit_log (actor_email, actor_ip, action, target, detail, outcome)
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(&sess.email)
    .bind(&sess.ip)
    .bind(action)
    .bind(target)
    .bind(detail)
    .bind(outcome)
    .execute(pool)
    .await;
}

/// Validate a submitted CSRF token against the session. On mismatch returns the
/// redirect target the caller should short-circuit to.
fn csrf_ok(sess: &Session, supplied: &str) -> bool {
    ct_eq(sess.csrf.as_bytes(), supplied.as_bytes())
}

fn redirect(path: &str) -> Resp {
    Redirect::to(path).into_response()
}

// ── Overview ────────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct Kpis {
    users: i64,
    banned: i64,
    developers: i64,
    games: i64,
    pending_games: i64,
    active_games: i64,
}

pub async fn overview(
    State(state): State<Arc<SuperAdminState>>,
    sess: axum::Extension<Session>,
) -> Html<String> {
    let pool = &state.pool;
    let k = sqlx::query_as::<_, Kpis>(
        "SELECT
           (SELECT COUNT(*) FROM users)::bigint AS users,
           (SELECT COUNT(*) FROM users WHERE is_banned)::bigint AS banned,
           (SELECT COUNT(*) FROM users WHERE is_developer)::bigint AS developers,
           (SELECT COUNT(*) FROM games)::bigint AS games,
           (SELECT COUNT(*) FROM games WHERE status = 'pending')::bigint AS pending_games,
           (SELECT COUNT(*) FROM games WHERE active)::bigint AS active_games",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(Kpis {
        users: 0,
        banned: 0,
        developers: 0,
        games: 0,
        pending_games: 0,
        active_games: 0,
    });

    let sum = billing::summary(pool).await;
    let checks = billing::run_all(pool).await;
    let failing = checks
        .iter()
        .filter(|c| c.severity == Severity::Fail)
        .count();
    let warning = checks
        .iter()
        .filter(|c| c.severity == Severity::Warn)
        .count();
    let an = analytics::overview(pool).await;

    let platform_rev = sum.platform_session_revenue + sum.platform_store_revenue;

    let compliance_pill = if failing > 0 {
        pill("bad", &format!("{failing} failing"))
    } else if warning > 0 {
        pill("warn", &format!("{warning} to review"))
    } else {
        pill("ok", "all passing")
    };

    let body = format!(
        "<h1>Platform overview</h1>\
<p class=\"sub\">Signed in as <span class=\"accent\">{email}</span> · since {since} · \
{sessions} active control session(s) · geo enrichment {geo}</p>\
<div class=\"grid\">\
{users}{devs}{banned}{games}{pending}{active}\
</div>\
<h2>Money</h2><div class=\"grid\">\
<div class=\"card\"><div class=\"k\">Platform revenue</div><div class=\"v accent\">{prev}</div></div>\
<div class=\"card\"><div class=\"k\">Developer earnings</div><div class=\"v\">{dev_earn}</div></div>\
<div class=\"card\"><div class=\"k\">Paid out</div><div class=\"v sm\">{paid}</div></div>\
<div class=\"card\"><div class=\"k\">Pending payouts</div><div class=\"v sm amber\">{pending_pay}</div></div>\
<div class=\"card\"><div class=\"k\">Wallet liability</div><div class=\"v sm\">{liab}</div></div>\
</div>\
<h2>Health</h2><div class=\"grid\">\
<div class=\"card\"><div class=\"k\">Billing compliance</div><div class=\"v sm\">{cpill}</div>\
<div class=\"muted\" style=\"margin-top:8px\"><a href=\"/superadmin/billing\">View report →</a></div></div>\
<div class=\"card\"><div class=\"k\">Requests (24h)</div><div class=\"v sm\">{req24}</div>\
<div class=\"muted\" style=\"margin-top:8px\"><a href=\"/superadmin/analytics\">Analytics →</a></div></div>\
<div class=\"card\"><div class=\"k\">Error rate</div><div class=\"v sm {errc}\">{err:.2}%</div></div>\
</div>",
        email = esc(&sess.email),
        since = dt(sess.created),
        sessions = state.sessions.active_count().await,
        geo = if state.geo.enabled() { "<span class=\"ok\">on</span>" } else { "<span class=\"muted\">off</span>" },
        users = kpi("Users", &k.users.to_string(), ""),
        devs = kpi("Developers", &k.developers.to_string(), ""),
        banned = kpi("Banned", &k.banned.to_string(), if k.banned > 0 { "bad" } else { "" }),
        games = kpi("Games", &k.games.to_string(), ""),
        pending = kpi("Pending review", &k.pending_games.to_string(), if k.pending_games > 0 { "amber" } else { "" }),
        active = kpi("Active games", &k.active_games.to_string(), ""),
        prev = money(platform_rev),
        dev_earn = money(sum.developer_session_revenue),
        paid = money(sum.total_paid_out),
        pending_pay = money(sum.pending_payouts),
        liab = money(sum.wallet_liability),
        cpill = compliance_pill,
        req24 = an.last_24h,
        err = an.error_rate_pct,
        errc = if an.error_rate_pct > 2.0 { "bad" } else { "ok" },
    );
    Html(html::page("Overview", &nav("overview"), &body))
}

fn kpi(label: &str, value: &str, cls: &str) -> String {
    format!(
        "<div class=\"card\"><div class=\"k\">{}</div><div class=\"v {}\">{}</div></div>",
        esc(label),
        cls,
        esc(value)
    )
}

// ── Users ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    #[serde(flatten)]
    pub flash: FlashQuery,
}

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    email: String,
    is_admin: bool,
    is_developer: bool,
    is_banned: bool,
    created_at: DateTime<Utc>,
    balance: Decimal,
}

pub async fn users_list(
    State(state): State<Arc<SuperAdminState>>,
    Query(q): Query<UserListQuery>,
) -> Html<String> {
    let page = q.page.unwrap_or(0).max(0);
    let limit = 50i64;
    let search = q.q.clone().unwrap_or_default();
    let like = format!("%{}%", search);

    let rows = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.email, u.is_admin, u.is_developer, u.is_banned, u.created_at,
                COALESCE((SELECT balance FROM wallet_balances w WHERE w.user_id=u.id AND w.currency='USD'),0) AS balance
         FROM users u
         WHERE ($1 = '' OR u.username ILIKE $2 OR u.email ILIKE $2)
         ORDER BY u.created_at DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(&search)
    .bind(&like)
    .bind(limit)
    .bind(page * limit)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut tbody = String::new();
    for r in &rows {
        let roles = {
            let mut v = Vec::new();
            if r.is_admin {
                v.push(pill("warn", "admin"));
            }
            if r.is_developer {
                v.push(pill("mute", "dev"));
            }
            v.join(" ")
        };
        let status = if r.is_banned {
            pill("bad", "banned")
        } else {
            pill("ok", "active")
        };
        tbody.push_str(&format!(
            "<tr><td><a href=\"/superadmin/users/{id}\">{user}</a></td>\
<td class=\"muted\">{email}</td><td>{roles}</td><td>{status}</td>\
<td>{bal}</td><td class=\"muted\">{created}</td></tr>",
            id = r.id,
            user = esc(&r.username),
            email = esc(&r.email),
            roles = roles,
            status = status,
            bal = money(r.balance),
            created = dt(r.created_at),
        ));
    }
    if rows.is_empty() {
        tbody.push_str("<tr><td colspan=\"6\" class=\"muted\">No users found.</td></tr>");
    }

    let prev = if page > 0 {
        format!(
            "<a class=\"btn\" href=\"/superadmin/users?page={}&q={}\">← Prev</a>",
            page - 1,
            esc(&search)
        )
    } else {
        String::new()
    };
    let next = if rows.len() as i64 == limit {
        format!(
            "<a class=\"btn\" href=\"/superadmin/users?page={}&q={}\">Next →</a>",
            page + 1,
            esc(&search)
        )
    } else {
        String::new()
    };

    let body = format!(
        "<h1>Users</h1>{flash}\
<form method=\"get\" action=\"/superadmin/users\" class=\"row\" style=\"max-width:480px;margin-bottom:18px\">\
<input type=\"text\" name=\"q\" value=\"{q}\" placeholder=\"search username or email…\">\
<button class=\"btn\" type=\"submit\">Search</button></form>\
<table><tr><th>User</th><th>Email</th><th>Roles</th><th>Status</th><th>Balance</th><th>Joined</th></tr>{tbody}</table>\
<div class=\"row\" style=\"margin-top:14px\">{prev}{next}</div>",
        flash = flash(&q.flash),
        q = esc(&search),
        tbody = tbody,
        prev = prev,
        next = next,
    );
    Html(html::page("Users", &nav("users"), &body))
}

#[derive(FromRow)]
struct UserDetail {
    id: Uuid,
    username: String,
    email: String,
    is_admin: bool,
    is_developer: bool,
    is_banned: bool,
    ban_reason: Option<String>,
    email_verified: bool,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PayoutRow {
    amount: Decimal,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct WalletRow {
    currency: String,
    balance: Decimal,
}

pub async fn user_detail(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    Query(fq): Query<FlashQuery>,
    sess: axum::Extension<Session>,
) -> Resp {
    let pool = &state.pool;
    let u = match sqlx::query_as::<_, UserDetail>(
        "SELECT id, username, email, is_admin, is_developer, is_banned, ban_reason,
                email_verified, created_at FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    {
        Some(u) => u,
        None => {
            return Html(html::page(
                "Not found",
                &nav("users"),
                "<h1>User not found</h1>",
            ))
            .into_response()
        }
    };

    let wallets = sqlx::query_as::<_, WalletRow>(
        "SELECT currency, balance FROM wallet_balances WHERE user_id = $1 ORDER BY currency",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let payouts = sqlx::query_as::<_, PayoutRow>(
        "SELECT amount, status, created_at FROM payouts WHERE user_id = $1 ORDER BY created_at DESC LIMIT 8",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let earned = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(developer_share),0) FROM game_revenue WHERE developer_id = $1 AND status='completed'",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap_or(Decimal::ZERO);

    let wallet_html = if wallets.is_empty() {
        "<span class=\"muted\">no wallet</span>".to_string()
    } else {
        wallets
            .iter()
            .map(|w| format!("{} <b>{}</b>", esc(&w.currency), money(w.balance)))
            .collect::<Vec<_>>()
            .join(" · ")
    };

    let mut payout_rows = String::new();
    for p in &payouts {
        payout_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"muted\">{}</td></tr>",
            money(p.amount),
            pill(payout_pill(&p.status), &p.status),
            dt(p.created_at)
        ));
    }
    if payouts.is_empty() {
        payout_rows.push_str("<tr><td colspan=\"3\" class=\"muted\">No payouts.</td></tr>");
    }

    let csrf = esc(&sess.csrf);
    let ban_label = if u.is_banned {
        "Unban user"
    } else {
        "Ban user"
    };
    let ban_class = if u.is_banned { "btn go" } else { "btn danger" };
    let ban_value = if u.is_banned { "false" } else { "true" };

    let body = format!(
        "<h1>{user}</h1>{flash}\
<p class=\"sub\">{email} · joined {created} · {verified}</p>\
<div class=\"grid\">\
<div class=\"card\"><div class=\"k\">Status</div><div class=\"v sm\">{status}</div></div>\
<div class=\"card\"><div class=\"k\">Roles</div><div class=\"v sm\">{roles}</div></div>\
<div class=\"card\"><div class=\"k\">Wallet</div><div class=\"v sm\">{wallet}</div></div>\
<div class=\"card\"><div class=\"k\">Accrued earnings</div><div class=\"v sm\">{earned}</div></div>\
</div>\
{ban_note}\
<h2>Actions</h2>\
<div class=\"row\">\
<form class=\"inline\" method=\"post\" action=\"/superadmin/users/{id}/ban\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\">\
<input type=\"hidden\" name=\"banned\" value=\"{ban_value}\">\
<input type=\"hidden\" name=\"reason\" value=\"super-admin action\">\
<button class=\"{ban_class}\" type=\"submit\">{ban_label}</button></form>\
<form class=\"inline\" method=\"post\" action=\"/superadmin/users/{id}/role\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\">\
<input type=\"hidden\" name=\"field\" value=\"is_admin\">\
<input type=\"hidden\" name=\"value\" value=\"{toggle_admin}\">\
<button class=\"btn\" type=\"submit\">{admin_label}</button></form>\
<form class=\"inline\" method=\"post\" action=\"/superadmin/users/{id}/role\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\">\
<input type=\"hidden\" name=\"field\" value=\"is_developer\">\
<input type=\"hidden\" name=\"value\" value=\"{toggle_dev}\">\
<button class=\"btn\" type=\"submit\">{dev_label}</button></form>\
</div>\
<h2>Recent payouts</h2>\
<table><tr><th>Amount</th><th>Status</th><th>When</th></tr>{payout_rows}</table>",
        user = esc(&u.username),
        flash = flash(&fq),
        email = esc(&u.email),
        created = dt(u.created_at),
        verified = if u.email_verified {
            "<span class=\"ok\">verified</span>"
        } else {
            "<span class=\"amber\">unverified</span>"
        },
        status = if u.is_banned {
            pill("bad", "banned")
        } else {
            pill("ok", "active")
        },
        roles = {
            let mut v = Vec::new();
            if u.is_admin {
                v.push(pill("warn", "admin"));
            }
            if u.is_developer {
                v.push(pill("mute", "developer"));
            }
            if v.is_empty() {
                v.push(pill("mute", "user"));
            }
            v.join(" ")
        },
        wallet = wallet_html,
        earned = money(earned),
        ban_note = match &u.ban_reason {
            Some(r) if u.is_banned => format!("<div class=\"flash err\">Banned: {}</div>", esc(r)),
            _ => String::new(),
        },
        id = u.id,
        csrf = csrf,
        ban_value = ban_value,
        ban_class = ban_class,
        ban_label = ban_label,
        toggle_admin = if u.is_admin { "false" } else { "true" },
        admin_label = if u.is_admin {
            "Revoke admin"
        } else {
            "Grant admin"
        },
        toggle_dev = if u.is_developer { "false" } else { "true" },
        dev_label = if u.is_developer {
            "Revoke developer"
        } else {
            "Grant developer"
        },
        payout_rows = payout_rows,
    );
    Html(html::page(&u.username, &nav("users"), &body)).into_response()
}

fn payout_pill(status: &str) -> &'static str {
    match status {
        "completed" | "paid" => "ok",
        "failed" | "cancelled" => "bad",
        _ => "warn",
    }
}

#[derive(Debug, Deserialize)]
pub struct BanForm {
    pub csrf: String,
    pub banned: String,
    pub reason: Option<String>,
}

pub async fn user_ban(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<BanForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect(&format!("/superadmin/users/{id}?err=CSRF+check+failed"));
    }
    let banning = f.banned == "true";
    let reason = f.reason.unwrap_or_default();
    let res = if banning {
        sqlx::query(
            "UPDATE users SET is_banned = true, banned_at = NOW(), ban_reason = $2 WHERE id = $1",
        )
        .bind(id)
        .bind(&reason)
        .execute(&state.pool)
        .await
    } else {
        sqlx::query(
            "UPDATE users SET is_banned = false, banned_at = NULL, ban_reason = NULL WHERE id = $1",
        )
        .bind(id)
        .execute(&state.pool)
        .await
    };
    let (msg, outcome) = match res {
        Ok(_) => (
            if banning {
                "User banned"
            } else {
                "User unbanned"
            },
            "ok",
        ),
        Err(_) => ("Update failed", "error"),
    };
    audit(
        &state.pool,
        &sess,
        if banning { "user.ban" } else { "user.unban" },
        &id.to_string(),
        &reason,
        outcome,
    )
    .await;
    redirect(&format!(
        "/superadmin/users/{id}?msg={}",
        msg.replace(' ', "+")
    ))
}

#[derive(Debug, Deserialize)]
pub struct RoleForm {
    pub csrf: String,
    pub field: String,
    pub value: String,
}

pub async fn user_role(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<RoleForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect(&format!("/superadmin/users/{id}?err=CSRF+check+failed"));
    }
    // Whitelist the column to prevent injection; only two boolean role flags.
    let column = match f.field.as_str() {
        "is_admin" => "is_admin",
        "is_developer" => "is_developer",
        _ => return redirect(&format!("/superadmin/users/{id}?err=Unknown+field")),
    };
    let value = f.value == "true";
    let sql = format!("UPDATE users SET {column} = $2 WHERE id = $1");
    let res = sqlx::query(&sql)
        .bind(id)
        .bind(value)
        .execute(&state.pool)
        .await;
    let outcome = if res.is_ok() { "ok" } else { "error" };
    audit(
        &state.pool,
        &sess,
        "user.role",
        &id.to_string(),
        &format!("{column}={value}"),
        outcome,
    )
    .await;
    redirect(&format!("/superadmin/users/{id}?msg=Role+updated"))
}

// ── Games ───────────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct GameRow {
    id: Uuid,
    title: String,
    status: String,
    featured_at: Option<DateTime<Utc>>,
    fee_per_session: Decimal,
    developer: Option<String>,
    created_at: DateTime<Utc>,
}

pub async fn games_list(
    State(state): State<Arc<SuperAdminState>>,
    Query(fq): Query<FlashQuery>,
    sess: axum::Extension<Session>,
) -> Html<String> {
    let rows = sqlx::query_as::<_, GameRow>(
        "SELECT g.id, g.title, g.status, g.featured_at, g.fee_per_session,
                u.username AS developer, g.created_at
         FROM games g LEFT JOIN users u ON u.id = g.developer_id
         ORDER BY (g.status='pending') DESC, g.created_at DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let csrf = esc(&sess.csrf);
    let mut tbody = String::new();
    for g in &rows {
        let st = match g.status.as_str() {
            "approved" | "published" => pill("ok", &g.status),
            "rejected" => pill("bad", &g.status),
            _ => pill("warn", &g.status),
        };
        let featured = if g.featured_at.is_some() {
            pill("warn", "featured")
        } else {
            String::new()
        };
        let approve_btn = if g.status != "approved" && g.status != "published" {
            format!(
                "<form class=\"inline\" method=\"post\" action=\"/superadmin/games/{id}/approve\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\"><input type=\"hidden\" name=\"approved\" value=\"true\">\
<button class=\"btn go\" type=\"submit\">Approve</button></form>",
                id = g.id,
                csrf = csrf
            )
        } else {
            format!(
                "<form class=\"inline\" method=\"post\" action=\"/superadmin/games/{id}/approve\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\"><input type=\"hidden\" name=\"approved\" value=\"false\">\
<button class=\"btn danger\" type=\"submit\">Unpublish</button></form>",
                id = g.id,
                csrf = csrf
            )
        };
        let feature_btn = format!(
            "<form class=\"inline\" method=\"post\" action=\"/superadmin/games/{id}/feature\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\"><input type=\"hidden\" name=\"featured\" value=\"{val}\">\
<button class=\"btn\" type=\"submit\">{lbl}</button></form>",
            id = g.id,
            csrf = csrf,
            val = if g.featured_at.is_some() { "false" } else { "true" },
            lbl = if g.featured_at.is_some() { "Unfeature" } else { "Feature" },
        );
        tbody.push_str(&format!(
            "<tr><td class=\"wrap\">{title} {featured}</td><td class=\"muted\">{dev}</td>\
<td>{st}</td><td>{fee}</td><td class=\"muted\">{created}</td>\
<td class=\"row\">{approve}{feature}</td></tr>",
            title = esc(&g.title),
            featured = featured,
            dev = esc(g.developer.as_deref().unwrap_or("—")),
            st = st,
            fee = money(g.fee_per_session),
            created = dt(g.created_at),
            approve = approve_btn,
            feature = feature_btn,
        ));
    }
    if rows.is_empty() {
        tbody.push_str("<tr><td colspan=\"6\" class=\"muted\">No games.</td></tr>");
    }

    let body = format!(
        "<h1>Games</h1>{flash}\
<p class=\"sub\">Pending submissions are listed first. Approve, unpublish, or feature any title.</p>\
<table><tr><th>Title</th><th>Developer</th><th>Status</th><th>Fee/session</th><th>Created</th><th>Actions</th></tr>{tbody}</table>",
        flash = flash(&fq),
        tbody = tbody,
    );
    Html(html::page("Games", &nav("games"), &body))
}

#[derive(Debug, Deserialize)]
pub struct ApproveForm {
    pub csrf: String,
    pub approved: String,
}

pub async fn game_approve(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<ApproveForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect("/superadmin/games?err=CSRF+check+failed");
    }
    let approved = f.approved == "true";
    let (status, active) = if approved {
        ("approved", true)
    } else {
        ("rejected", false)
    };
    let res =
        sqlx::query("UPDATE games SET status = $2, active = $3, reviewed_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(status)
            .bind(active)
            .execute(&state.pool)
            .await;
    let outcome = if res.is_ok() { "ok" } else { "error" };
    audit(
        &state.pool,
        &sess,
        if approved {
            "game.approve"
        } else {
            "game.reject"
        },
        &id.to_string(),
        "",
        outcome,
    )
    .await;
    redirect(&format!(
        "/superadmin/games?msg=Game+{}",
        if approved { "approved" } else { "unpublished" }
    ))
}

#[derive(Debug, Deserialize)]
pub struct FeatureForm {
    pub csrf: String,
    pub featured: String,
}

pub async fn game_feature(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<FeatureForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect("/superadmin/games?err=CSRF+check+failed");
    }
    let featured = f.featured == "true";
    let res = if featured {
        sqlx::query("UPDATE games SET featured_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await
    } else {
        sqlx::query("UPDATE games SET featured_at = NULL WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await
    };
    let outcome = if res.is_ok() { "ok" } else { "error" };
    audit(
        &state.pool,
        &sess,
        "game.feature",
        &id.to_string(),
        &featured.to_string(),
        outcome,
    )
    .await;
    redirect(&format!(
        "/superadmin/games?msg=Game+{}",
        if featured { "featured" } else { "unfeatured" }
    ))
}

// ── Money / transactions ────────────────────────────────────────────────────

#[derive(FromRow)]
struct TxRow {
    username: Option<String>,
    game_title: Option<String>,
    tx_type: String,
    amount: Decimal,
    created_at: DateTime<Utc>,
}

pub async fn transactions(State(state): State<Arc<SuperAdminState>>) -> Html<String> {
    let pool = &state.pool;
    let sum = billing::summary(pool).await;
    let txs = sqlx::query_as::<_, TxRow>(
        "SELECT u.username, g.title AS game_title, t.type AS tx_type, t.amount, t.created_at
         FROM transactions t
         LEFT JOIN users u ON u.id = t.user_id
         LEFT JOIN games g ON g.id = t.game_id
         ORDER BY t.created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut rows = String::new();
    for t in &txs {
        rows.push_str(&format!(
            "<tr><td class=\"muted\">{when}</td><td>{user}</td><td>{game}</td>\
<td>{ty}</td><td>{amt}</td></tr>",
            when = dt(t.created_at),
            user = esc(t.username.as_deref().unwrap_or("—")),
            game = esc(t.game_title.as_deref().unwrap_or("—")),
            ty = esc(&t.tx_type),
            amt = money(t.amount),
        ));
    }
    if txs.is_empty() {
        rows.push_str("<tr><td colspan=\"5\" class=\"muted\">No transactions.</td></tr>");
    }

    let body = format!(
        "<h1>Money flow</h1>\
<div class=\"grid\">\
<div class=\"card\"><div class=\"k\">Gross session revenue</div><div class=\"v sm\">{gsr}</div></div>\
<div class=\"card\"><div class=\"k\">Platform (15%)</div><div class=\"v sm accent\">{psr}</div></div>\
<div class=\"card\"><div class=\"k\">Developer (85%)</div><div class=\"v sm\">{dsr}</div></div>\
<div class=\"card\"><div class=\"k\">Store revenue</div><div class=\"v sm\">{gst}</div></div>\
<div class=\"card\"><div class=\"k\">Store platform (30%)</div><div class=\"v sm accent\">{pst}</div></div>\
</div>\
<h2>Recent transactions</h2>\
<table><tr><th>When</th><th>User</th><th>Game</th><th>Type</th><th>Amount</th></tr>{rows}</table>",
        gsr = money(sum.gross_session_revenue),
        psr = money(sum.platform_session_revenue),
        dsr = money(sum.developer_session_revenue),
        gst = money(sum.gross_store_revenue),
        pst = money(sum.platform_store_revenue),
        rows = rows,
    );
    Html(html::page("Money", &nav("money"), &body))
}

// ── Billing compliance ──────────────────────────────────────────────────────

pub async fn billing_page(State(state): State<Arc<SuperAdminState>>) -> Html<String> {
    let pool = &state.pool;
    let checks = billing::run_all(pool).await;
    let failing = checks
        .iter()
        .filter(|c| c.severity == Severity::Fail)
        .count();
    let warning = checks
        .iter()
        .filter(|c| c.severity == Severity::Warn)
        .count();

    let headline = if failing > 0 {
        format!(
            "<div class=\"flash err\">{failing} check(s) FAILING — the platform is not following its billing model. Investigate below.</div>"
        )
    } else if warning > 0 {
        format!("<div class=\"flash err\" style=\"border-color:#5a4420;background:#1c1408;color:#f5a524\">{warning} check(s) need review.</div>")
    } else {
        "<div class=\"flash ok\">All billing checks pass — the platform is charging, splitting, and paying exactly as designed.</div>".to_string()
    };

    let mut cards = String::new();
    for c in &checks {
        let (pk, badge) = match c.severity {
            Severity::Ok => ("ok", pill("ok", "pass")),
            Severity::Warn => ("warn", pill("warn", "review")),
            Severity::Fail => ("bad", pill("bad", "fail")),
        };
        let mut offenders = String::new();
        if !c.offenders.is_empty() {
            offenders
                .push_str("<table style=\"margin-top:10px\"><tr><th>Ref</th><th>Detail</th></tr>");
            for o in &c.offenders {
                offenders.push_str(&format!(
                    "<tr><td class=\"muted\">{}</td><td class=\"wrap\">{}</td></tr>",
                    esc(&o.id),
                    esc(&o.summary)
                ));
            }
            offenders.push_str("</table>");
        }
        cards.push_str(&format!(
            "<div class=\"card\" style=\"margin-bottom:14px\">\
<div class=\"row\" style=\"justify-content:space-between\"><b>{name}</b>{badge}</div>\
<div class=\"muted\" style=\"margin:6px 0\">{desc}</div>\
<div class=\"{pk}\">{detail}</div>\
<div class=\"muted\" style=\"margin-top:6px;font-size:11px\">{checked} checked · {violations} flagged</div>\
{offenders}</div>",
            name = esc(&c.name),
            badge = badge,
            desc = esc(&c.description),
            pk = pk,
            detail = esc(&c.detail),
            checked = c.checked,
            violations = c.violations,
            offenders = offenders,
        ));
    }

    let body = format!(
        "<h1>Billing-model compliance</h1>\
<p class=\"sub\">Sessions split 15/85 · store sales split 30/70 · payouts ≤ accrued · subscriptions = list price · wallets reconcile.</p>\
{headline}{cards}",
        headline = headline,
        cards = cards,
    );
    Html(html::page("Billing compliance", &nav("billing"), &body))
}

// ── Analytics ───────────────────────────────────────────────────────────────

pub async fn analytics_page(State(state): State<Arc<SuperAdminState>>) -> Html<String> {
    let pool = &state.pool;
    let ov = analytics::overview(pool).await;
    let countries = analytics::top_countries(pool, 12).await;
    let paths = analytics::top_paths(pool, 12).await;
    let users = analytics::top_users(pool, 12).await;
    let by_day = analytics::requests_by_day(pool, 14).await;
    let recent = analytics::recent_events(pool, 60).await;

    let max_day = by_day.iter().map(|d| d.count).max().unwrap_or(1).max(1);
    let mut day_bars = String::new();
    for d in &by_day {
        let pct = (d.count as f64 / max_day as f64 * 100.0).round() as i64;
        day_bars.push_str(&format!(
            "<div class=\"row\" style=\"margin:5px 0\"><div style=\"width:110px\" class=\"muted\">{}</div>\
<div class=\"bar\"><span style=\"width:{}%\"></span></div><div style=\"width:60px;text-align:right\">{}</div></div>",
            d.day.format("%b %d"),
            pct,
            d.count
        ));
    }
    if by_day.is_empty() {
        day_bars.push_str("<div class=\"muted\">No data yet.</div>");
    }

    let geo_note = if state.geo.enabled() {
        "<span class=\"ok\">offline GeoIP active</span>"
    } else {
        "<span class=\"amber\">no GeoIP database — set GEOIP_DB_PATH to enable city/country</span>"
    };

    let bars = |items: &[analytics::LabelCount]| -> String {
        let max = items.iter().map(|i| i.count).max().unwrap_or(1).max(1);
        let mut s = String::new();
        for i in items {
            let pct = (i.count as f64 / max as f64 * 100.0).round() as i64;
            s.push_str(&format!(
                "<div class=\"row\" style=\"margin:5px 0\"><div style=\"width:160px\" class=\"wrap\">{}</div>\
<div class=\"bar\"><span style=\"width:{}%\"></span></div><div style=\"width:60px;text-align:right\">{}</div></div>",
                esc(i.label.as_deref().unwrap_or("—")),
                pct,
                i.count
            ));
        }
        if items.is_empty() {
            s.push_str("<div class=\"muted\">No data yet.</div>");
        }
        s
    };

    let mut user_rows = String::new();
    for u in &users {
        user_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"muted\">{}</td></tr>",
            esc(u.username.as_deref().unwrap_or("—")),
            u.count,
            dt(u.last_seen)
        ));
    }
    if users.is_empty() {
        user_rows.push_str(
            "<tr><td colspan=\"3\" class=\"muted\">No authenticated traffic yet.</td></tr>",
        );
    }

    let mut recent_rows = String::new();
    for e in &recent {
        let loc = match (&e.country, &e.city) {
            (Some(c), Some(city)) => format!("{}, {}", esc(city), esc(c)),
            (Some(c), None) => esc(c),
            _ => "—".to_string(),
        };
        let sc = if e.status >= 500 {
            "bad"
        } else if e.status >= 400 {
            "amber"
        } else {
            "ok"
        };
        recent_rows.push_str(&format!(
            "<tr><td class=\"muted\">{when}</td><td class=\"muted\">{ip}</td><td>{loc}</td>\
<td>{method}</td><td class=\"wrap\">{path}</td><td class=\"{sc}\">{status}</td>\
<td>{dur}ms</td><td>{user}</td></tr>",
            when = dt(e.occurred_at),
            ip = esc(e.ip.as_deref().unwrap_or("—")),
            loc = loc,
            method = esc(&e.method),
            path = esc(&e.path),
            sc = sc,
            status = e.status,
            dur = e.duration_ms.unwrap_or(0),
            user = esc(e.username.as_deref().unwrap_or("—")),
        ));
    }
    if recent.is_empty() {
        recent_rows
            .push_str("<tr><td colspan=\"8\" class=\"muted\">No requests recorded yet.</td></tr>");
    }

    let body = format!(
        "<h1>Analytics</h1><p class=\"sub\">In-house request analytics · {geo}</p>\
<div class=\"grid\">\
<div class=\"card\"><div class=\"k\">Total requests</div><div class=\"v sm\">{total}</div></div>\
<div class=\"card\"><div class=\"k\">Last 24h</div><div class=\"v sm\">{l24}</div></div>\
<div class=\"card\"><div class=\"k\">Unique IPs</div><div class=\"v sm\">{ips}</div></div>\
<div class=\"card\"><div class=\"k\">Signed-in users</div><div class=\"v sm\">{uu}</div></div>\
<div class=\"card\"><div class=\"k\">Avg latency</div><div class=\"v sm\">{lat:.0}ms</div></div>\
<div class=\"card\"><div class=\"k\">Error rate</div><div class=\"v sm {errc}\">{err:.2}%</div></div>\
</div>\
<h2>Requests (last 14 days)</h2>{day_bars}\
<h2>Top countries</h2>{countries}\
<h2>Top endpoints</h2>{paths}\
<h2>Most active users</h2>\
<table><tr><th>User</th><th>Requests</th><th>Last seen</th></tr>{user_rows}</table>\
<h2>Recent requests</h2>\
<table><tr><th>When</th><th>IP</th><th>Location</th><th>Method</th><th>Path</th><th>Status</th><th>Latency</th><th>User</th></tr>{recent_rows}</table>",
        geo = geo_note,
        total = ov.total,
        l24 = ov.last_24h,
        ips = ov.unique_ips,
        uu = ov.unique_users,
        lat = ov.avg_duration_ms,
        err = ov.error_rate_pct,
        errc = if ov.error_rate_pct > 2.0 { "bad" } else { "ok" },
        day_bars = day_bars,
        countries = bars(&countries),
        paths = bars(&paths),
        user_rows = user_rows,
        recent_rows = recent_rows,
    );
    Html(html::page("Analytics", &nav("analytics"), &body))
}

// ── Audit log ───────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct AuditRow {
    occurred_at: DateTime<Utc>,
    actor_email: String,
    actor_ip: Option<String>,
    action: String,
    target: Option<String>,
    detail: Option<String>,
    outcome: String,
}

pub async fn audit_page(State(state): State<Arc<SuperAdminState>>) -> Html<String> {
    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT occurred_at, actor_email, actor_ip, action, target, detail, outcome
         FROM superadmin_audit_log ORDER BY occurred_at DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut tbody = String::new();
    for r in &rows {
        let oc = match r.outcome.as_str() {
            "ok" => pill("ok", "ok"),
            "denied" => pill("warn", "denied"),
            _ => pill("bad", &r.outcome),
        };
        tbody.push_str(&format!(
            "<tr><td class=\"muted\">{when}</td><td>{actor}</td><td class=\"muted\">{ip}</td>\
<td>{action}</td><td class=\"muted\">{target}</td><td class=\"wrap\">{detail}</td><td>{oc}</td></tr>",
            when = dt(r.occurred_at),
            actor = esc(&r.actor_email),
            ip = esc(r.actor_ip.as_deref().unwrap_or("—")),
            action = esc(&r.action),
            target = esc(r.target.as_deref().unwrap_or("—")),
            detail = esc(r.detail.as_deref().unwrap_or("")),
            oc = oc,
        ));
    }
    if rows.is_empty() {
        tbody.push_str("<tr><td colspan=\"7\" class=\"muted\">No audited actions yet.</td></tr>");
    }

    let body = format!(
        "<h1>Audit log</h1><p class=\"sub\">Append-only record of every super-admin action. Newest first.</p>\
<table><tr><th>When</th><th>Actor</th><th>IP</th><th>Action</th><th>Target</th><th>Detail</th><th>Outcome</th></tr>{tbody}</table>",
        tbody = tbody,
    );
    Html(html::page("Audit log", &nav("audit"), &body))
}
