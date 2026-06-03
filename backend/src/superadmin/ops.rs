// Super-admin operational management: developer payouts + platform settings.
//
// These are the two money-/policy-level surfaces a platform operator needs but
// that the read-only pages don't cover. All mutations validate CSRF, write an
// audit-log row, and redirect back with a flash.
//
// Safety note: the payout actions here only change DB state (cancel a pending
// request, or mark one processed for manual reconciliation). They deliberately
// do NOT trigger an outbound Wise transfer — real disbursement is the hourly
// PayoutService batch job, so an admin can't accidentally send money with a click.

use std::sync::Arc;

use axum::{
    extract::{Form, Path, Query, State},
    response::Html,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

use super::auth::Session;
use super::html::{self, esc, pill};
use super::pages::{audit, csrf_ok, dt, flash, money, nav, payout_pill, redirect, FlashQuery};
use super::SuperAdminState;

type Resp = axum::response::Response;

// ── Payouts ─────────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct PayoutRow {
    id: Uuid,
    username: Option<String>,
    amount: Decimal,
    destination: String,
    status: String,
    created_at: DateTime<Utc>,
    processed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct PayoutQuery {
    pub status: Option<String>,
    #[serde(flatten)]
    pub flash: FlashQuery,
}

pub async fn payouts_list(
    State(state): State<Arc<SuperAdminState>>,
    Query(q): Query<PayoutQuery>,
    sess: axum::Extension<Session>,
) -> Html<String> {
    let pool = &state.pool;
    let filter = q.status.clone().unwrap_or_default();

    let rows = sqlx::query_as::<_, PayoutRow>(
        "SELECT p.id, u.username, p.amount, p.destination, p.status, p.created_at, p.processed_at
         FROM payouts p LEFT JOIN users u ON u.id = p.user_id
         WHERE ($1 = '' OR p.status = $1)
         ORDER BY (p.status = 'pending') DESC, p.created_at DESC
         LIMIT 200",
    )
    .bind(&filter)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    #[derive(FromRow)]
    struct Totals {
        pending_count: i64,
        pending_amount: Decimal,
        paid_amount: Decimal,
    }
    let totals = sqlx::query_as::<_, Totals>(
        "SELECT
           COUNT(*) FILTER (WHERE status='pending')::bigint AS pending_count,
           COALESCE(SUM(amount) FILTER (WHERE status='pending'),0) AS pending_amount,
           COALESCE(SUM(amount) FILTER (WHERE status IN ('completed','paid')),0) AS paid_amount
         FROM payouts",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(Totals {
        pending_count: 0,
        pending_amount: Decimal::ZERO,
        paid_amount: Decimal::ZERO,
    });

    let csrf = esc(&sess.csrf);
    let mut tbody = String::new();
    for r in &rows {
        let actions = if r.status == "pending" {
            format!(
                "<form class=\"inline\" method=\"post\" action=\"/superadmin/payouts/{id}/process\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\">\
<button class=\"btn go\" type=\"submit\">Mark processed</button></form> \
<form class=\"inline\" method=\"post\" action=\"/superadmin/payouts/{id}/cancel\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\">\
<button class=\"btn danger\" type=\"submit\">Cancel</button></form>",
                id = r.id,
                csrf = csrf
            )
        } else {
            "<span class=\"muted\">—</span>".to_string()
        };
        tbody.push_str(&format!(
            "<tr><td class=\"muted\">{when}</td><td>{user}</td><td>{amt}</td>\
<td class=\"muted wrap\">{dest}</td><td>{st}</td><td class=\"muted\">{proc}</td><td class=\"row\">{actions}</td></tr>",
            when = dt(r.created_at),
            user = esc(r.username.as_deref().unwrap_or("—")),
            amt = money(r.amount),
            dest = esc(&r.destination),
            st = pill(payout_pill(&r.status), &r.status),
            proc = r.processed_at.map(dt).unwrap_or_else(|| "—".into()),
            actions = actions,
        ));
    }
    if rows.is_empty() {
        tbody.push_str("<tr><td colspan=\"7\" class=\"muted\">No payouts.</td></tr>");
    }

    let filters = ["", "pending", "completed", "cancelled", "failed"]
        .iter()
        .map(|s| {
            let label = if s.is_empty() { "all" } else { s };
            let on = if *s == filter {
                " style=\"border-color:var(--accent);color:var(--accent)\""
            } else {
                ""
            };
            format!(
                "<a class=\"btn\" href=\"/superadmin/payouts?status={s}\"{on}>{label}</a>",
                s = s,
                on = on,
                label = label
            )
        })
        .collect::<String>();

    let body = format!(
        "<h1>Payouts</h1>{flash}\
<p class=\"sub\">Developer disbursements. Cancel a pending request, or mark one processed for manual \
reconciliation. Automatic sending runs hourly via the payout batch (requires Wise credentials).</p>\
<div class=\"grid\">\
<div class=\"card\"><div class=\"k\">Pending</div><div class=\"v sm amber\">{pc}</div></div>\
<div class=\"card\"><div class=\"k\">Pending amount</div><div class=\"v sm\">{pa}</div></div>\
<div class=\"card\"><div class=\"k\">Paid out</div><div class=\"v sm\">{paid}</div></div>\
</div>\
<div class=\"row\" style=\"margin:16px 0\">{filters}</div>\
<table><tr><th>Requested</th><th>Developer</th><th>Amount</th><th>Destination</th><th>Status</th><th>Processed</th><th>Actions</th></tr>{tbody}</table>",
        flash = flash(&q.flash),
        pc = totals.pending_count,
        pa = money(totals.pending_amount),
        paid = money(totals.paid_amount),
        filters = filters,
        tbody = tbody,
    );
    Html(html::page("Payouts", &nav("payouts"), &body))
}

#[derive(Debug, Deserialize)]
pub struct CsrfForm {
    pub csrf: String,
}

pub async fn payout_cancel(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<CsrfForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect("/superadmin/payouts?err=CSRF+check+failed");
    }
    // Only a still-pending payout can be cancelled.
    let res = sqlx::query(
        "UPDATE payouts SET status='cancelled', cancelled_at=NOW() WHERE id=$1 AND status='pending'",
    )
    .bind(id)
    .execute(&state.pool)
    .await;
    let (msg, outcome) = match res {
        Ok(r) if r.rows_affected() > 0 => ("Payout cancelled", "ok"),
        Ok(_) => ("Payout was not pending", "denied"),
        Err(_) => ("Update failed", "error"),
    };
    audit(
        &state.pool,
        &sess,
        "payout.cancel",
        &id.to_string(),
        "",
        outcome,
    )
    .await;
    redirect(&format!(
        "/superadmin/payouts?msg={}",
        msg.replace(' ', "+")
    ))
}

pub async fn payout_process(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<CsrfForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect("/superadmin/payouts?err=CSRF+check+failed");
    }
    let res = sqlx::query(
        "UPDATE payouts SET status='completed', processed_at=NOW() WHERE id=$1 AND status='pending'",
    )
    .bind(id)
    .execute(&state.pool)
    .await;
    let (msg, outcome) = match res {
        Ok(r) if r.rows_affected() > 0 => ("Payout marked processed", "ok"),
        Ok(_) => ("Payout was not pending", "denied"),
        Err(_) => ("Update failed", "error"),
    };
    audit(
        &state.pool,
        &sess,
        "payout.mark_processed",
        &id.to_string(),
        "manual reconciliation",
        outcome,
    )
    .await;
    redirect(&format!(
        "/superadmin/payouts?msg={}",
        msg.replace(' ', "+")
    ))
}

// ── Moderation queue (flagged chat messages) ────────────────────────────────

#[derive(FromRow)]
struct FlagRow {
    id: Uuid,
    author_id: Uuid,
    author: Option<String>,
    content: String,
    flag_reasons: String,
    status: String,
    created_at: DateTime<Utc>,
    resolution_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModQuery {
    pub status: Option<String>,
    #[serde(flatten)]
    pub flash: FlashQuery,
}

pub async fn moderation_list(
    State(state): State<Arc<SuperAdminState>>,
    Query(q): Query<ModQuery>,
    sess: axum::Extension<Session>,
) -> Html<String> {
    // Default to the pending queue; allow viewing resolved/dismissed/all.
    let filter = q.status.clone().unwrap_or_else(|| "pending".to_string());

    let rows = sqlx::query_as::<_, FlagRow>(
        "SELECT cf.id, cf.author_id, u.username AS author, cf.content, cf.flag_reasons,
                cf.status, cf.created_at, cf.resolution_note
         FROM chat_flags cf LEFT JOIN users u ON u.id = cf.author_id
         WHERE ($1 = 'all' OR cf.status = $1)
         ORDER BY (cf.status='pending') DESC, cf.created_at DESC
         LIMIT 200",
    )
    .bind(&filter)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_flags WHERE status='pending'")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let csrf = esc(&sess.csrf);
    let mut tbody = String::new();
    for r in &rows {
        let st = match r.status.as_str() {
            "resolved" => pill("ok", "resolved"),
            "dismissed" => pill("mute", "dismissed"),
            _ => pill("warn", "pending"),
        };
        let actions = if r.status == "pending" {
            format!(
                "<form class=\"inline\" method=\"post\" action=\"/superadmin/moderation/{id}/act\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\"><input type=\"hidden\" name=\"action\" value=\"resolve\">\
<button class=\"btn danger\" type=\"submit\">Resolve</button></form> \
<form class=\"inline\" method=\"post\" action=\"/superadmin/moderation/{id}/act\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\"><input type=\"hidden\" name=\"action\" value=\"dismiss\">\
<button class=\"btn\" type=\"submit\">Dismiss</button></form>",
                id = r.id,
                csrf = csrf
            )
        } else {
            format!(
                "<span class=\"muted\">{}</span>",
                esc(r.resolution_note.as_deref().unwrap_or("—"))
            )
        };
        tbody.push_str(&format!(
            "<tr><td class=\"muted\">{when}</td>\
<td><a href=\"/superadmin/users/{aid}\">{author}</a></td>\
<td class=\"wrap\">{content}</td><td>{reasons}</td><td>{st}</td><td class=\"row\">{actions}</td></tr>",
            when = dt(r.created_at),
            aid = r.author_id,
            author = esc(r.author.as_deref().unwrap_or("—")),
            content = esc(&r.content),
            reasons = esc(&r.flag_reasons),
            st = st,
            actions = actions,
        ));
    }
    if rows.is_empty() {
        tbody.push_str("<tr><td colspan=\"6\" class=\"muted\">Nothing in this queue.</td></tr>");
    }

    let filters = ["pending", "resolved", "dismissed", "all"]
        .iter()
        .map(|s| {
            let on = if **s == filter {
                " style=\"border-color:var(--accent);color:var(--accent)\""
            } else {
                ""
            };
            format!(
                "<a class=\"btn\" href=\"/superadmin/moderation?status={s}\"{on}>{s}</a>",
                s = s,
                on = on
            )
        })
        .collect::<String>();

    let body = format!(
        "<h1>Moderation</h1>{flash}\
<p class=\"sub\">Auto-flagged chat messages. Resolve (uphold the flag) or dismiss (false positive). \
The message itself is retained; use the author link to ban a repeat offender. \
<span class=\"amber\">{pending} pending</span>.</p>\
<div class=\"row\" style=\"margin:16px 0\">{filters}</div>\
<table><tr><th>When</th><th>Author</th><th>Message</th><th>Reasons</th><th>Status</th><th>Actions</th></tr>{tbody}</table>",
        flash = flash(&q.flash),
        pending = pending,
        filters = filters,
        tbody = tbody,
    );
    Html(html::page("Moderation", &nav("moderation"), &body))
}

#[derive(Debug, Deserialize)]
pub struct ModActForm {
    pub csrf: String,
    pub action: String,
}

pub async fn moderation_act(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<ModActForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect("/superadmin/moderation?err=CSRF+check+failed");
    }
    let status = match f.action.as_str() {
        "resolve" => "resolved",
        "dismiss" => "dismissed",
        _ => return redirect("/superadmin/moderation?err=Unknown+action"),
    };
    // The super-admin is an env credential, not a users row, so resolved_by stays
    // NULL and the actor is recorded in the note + the audit log.
    let note = format!("by superadmin {}", sess.email);
    let res = sqlx::query(
        "UPDATE chat_flags SET status=$2, resolved_at=NOW(), resolution_note=$3
         WHERE id=$1 AND status='pending'",
    )
    .bind(id)
    .bind(status)
    .bind(&note)
    .execute(&state.pool)
    .await;
    let (msg, outcome) = match res {
        Ok(r) if r.rows_affected() > 0 => ("Flag updated", "ok"),
        Ok(_) => ("Flag was not pending", "denied"),
        Err(_) => ("Update failed", "error"),
    };
    audit(
        &state.pool,
        &sess,
        &format!("moderation.{}", f.action),
        &id.to_string(),
        "",
        outcome,
    )
    .await;
    redirect(&format!(
        "/superadmin/moderation?msg={}",
        msg.replace(' ', "+")
    ))
}

// ── Platform settings ───────────────────────────────────────────────────────

#[derive(FromRow)]
struct SettingRow {
    key: String,
    value: String,
    updated_at: Option<DateTime<Utc>>,
}

pub async fn settings_list(
    State(state): State<Arc<SuperAdminState>>,
    Query(q): Query<FlashQuery>,
    sess: axum::Extension<Session>,
) -> Html<String> {
    let rows = sqlx::query_as::<_, SettingRow>(
        "SELECT key, value, updated_at FROM platform_settings ORDER BY key",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let csrf = esc(&sess.csrf);
    let mut tbody = String::new();
    for r in &rows {
        tbody.push_str(&format!(
            "<tr><td><code>{key}</code></td>\
<td><form class=\"row\" method=\"post\" action=\"/superadmin/settings/update\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\">\
<input type=\"hidden\" name=\"key\" value=\"{key}\">\
<input type=\"text\" name=\"value\" value=\"{val}\" style=\"max-width:320px\">\
<button class=\"btn go\" type=\"submit\">Save</button></form></td>\
<td class=\"muted\">{updated}</td></tr>",
            key = esc(&r.key),
            csrf = csrf,
            val = esc(&r.value),
            updated = r.updated_at.map(dt).unwrap_or_else(|| "—".into()),
        ));
    }
    if rows.is_empty() {
        tbody.push_str("<tr><td colspan=\"3\" class=\"muted\">No platform settings.</td></tr>");
    }

    let body = format!(
        "<h1>Platform settings</h1>{flash}\
<p class=\"sub\">Key/value configuration read by platform features. \
Note: the per-session platform fee (15%) and store fee (30%) are fixed in code — \
the billing-compliance report verifies the platform actually follows them.</p>\
<table><tr><th>Key</th><th>Value</th><th>Updated</th></tr>{tbody}</table>",
        flash = flash(&q),
        tbody = tbody,
    );
    Html(html::page("Platform settings", &nav("settings"), &body))
}

#[derive(Debug, Deserialize)]
pub struct SettingForm {
    pub csrf: String,
    pub key: String,
    pub value: String,
}

pub async fn setting_update(
    State(state): State<Arc<SuperAdminState>>,
    sess: axum::Extension<Session>,
    Form(f): Form<SettingForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect("/superadmin/settings?err=CSRF+check+failed");
    }
    // Update an existing key only (UPSERT would let the form invent arbitrary keys).
    let res = sqlx::query("UPDATE platform_settings SET value=$2, updated_at=NOW() WHERE key=$1")
        .bind(&f.key)
        .bind(&f.value)
        .execute(&state.pool)
        .await;
    let (msg, outcome) = match res {
        Ok(r) if r.rows_affected() > 0 => ("Setting saved", "ok"),
        Ok(_) => ("Unknown setting key", "denied"),
        Err(_) => ("Update failed", "error"),
    };
    audit(
        &state.pool,
        &sess,
        "setting.update",
        &f.key,
        &f.value,
        outcome,
    )
    .await;
    redirect(&format!(
        "/superadmin/settings?msg={}",
        msg.replace(' ', "+")
    ))
}
