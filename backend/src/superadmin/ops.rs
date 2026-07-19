// Super-admin operational management: platform settings.
//
// These are the policy-level surfaces a platform operator needs but that the
// read-only pages don't cover. All mutations validate CSRF, write an audit-log
// row, and redirect back with a flash.
//
// There are deliberately NO money-moving actions here. Settlement is
// non-custodial (§3.6): value moves wallet-to-wallet on the payment rail and we
// hold nothing, so there is no balance for an operator to adjust, no payout to
// approve, and no transfer an admin could trigger with a click. Refunds are the
// marketplace's receipt-void path, not an operator button.

use std::sync::Arc;

use axum::{
    extract::{Form, Path, Query, State},
    response::Html,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

use super::auth::Session;
use super::html::{self, esc, pill};
use super::pages::{audit, csrf_ok, dt, flash, nav, redirect, FlashQuery};
use super::SuperAdminState;

type Resp = axum::response::Response;


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

    // Second queue: reported game reviews (pending only).
    let review_reports = reported_reviews(&state.pool).await;
    let mut rtbody = String::new();
    for rr in &review_reports {
        rtbody.push_str(&format!(
            "<tr><td class=\"muted\">{when}</td>\
<td><a href=\"/superadmin/users/{aid}\">{author}</a></td>\
<td>{rating}★</td><td class=\"wrap\">{content}</td><td>{reason}</td>\
<td class=\"row\">\
<form class=\"inline\" method=\"post\" action=\"/superadmin/review-reports/{id}/act\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\"><input type=\"hidden\" name=\"action\" value=\"remove\">\
<button class=\"btn danger\" type=\"submit\">Remove review</button></form> \
<form class=\"inline\" method=\"post\" action=\"/superadmin/review-reports/{id}/act\">\
<input type=\"hidden\" name=\"csrf\" value=\"{csrf}\"><input type=\"hidden\" name=\"action\" value=\"dismiss\">\
<button class=\"btn\" type=\"submit\">Dismiss</button></form></td></tr>",
            when = dt(rr.created_at),
            aid = rr.author_id,
            author = esc(rr.author.as_deref().unwrap_or("—")),
            rating = rr.rating,
            content = esc(rr.content.as_deref().unwrap_or("")),
            reason = esc(&rr.reason),
            id = rr.id,
            csrf = csrf,
        ));
    }
    if review_reports.is_empty() {
        rtbody
            .push_str("<tr><td colspan=\"6\" class=\"muted\">No pending review reports.</td></tr>");
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
<table><tr><th>When</th><th>Author</th><th>Message</th><th>Reasons</th><th>Status</th><th>Actions</th></tr>{tbody}</table>\
<h2>Reported reviews</h2>\
<p class=\"sub\">User-reported game reviews. Remove deletes the review; dismiss keeps it and closes the report.</p>\
<table><tr><th>Reported</th><th>Author</th><th>Rating</th><th>Review</th><th>Reason</th><th>Actions</th></tr>{rtbody}</table>",
        flash = flash(&q.flash),
        pending = pending,
        filters = filters,
        tbody = tbody,
        rtbody = rtbody,
    );
    Html(html::page("Moderation", &nav("moderation"), &body))
}

#[derive(FromRow)]
struct ReviewReportRow {
    id: Uuid,
    author_id: Uuid,
    author: Option<String>,
    rating: i32,
    content: Option<String>,
    reason: String,
    created_at: DateTime<Utc>,
}

async fn reported_reviews(pool: &sqlx::PgPool) -> Vec<ReviewReportRow> {
    sqlx::query_as::<_, ReviewReportRow>(
        "SELECT rr.id, rv.user_id AS author_id, u.username AS author, rv.rating, rv.content,
                rr.reason, rr.created_at
         FROM review_reports rr
         JOIN reviews rv ON rv.id = rr.review_id
         LEFT JOIN users u ON u.id = rv.user_id
         WHERE rr.status = 'pending'
         ORDER BY rr.created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn review_report_act(
    State(state): State<Arc<SuperAdminState>>,
    Path(id): Path<Uuid>,
    sess: axum::Extension<Session>,
    Form(f): Form<ModActForm>,
) -> Resp {
    if !csrf_ok(&sess, &f.csrf) {
        return redirect("/superadmin/moderation?err=CSRF+check+failed");
    }
    let note = format!("by superadmin {}", sess.email);
    let (msg, outcome) = match f.action.as_str() {
        "dismiss" => {
            let res = sqlx::query(
                "UPDATE review_reports SET status='dismissed', resolved_at=NOW(), resolution_note=$2
                 WHERE id=$1 AND status='pending'",
            )
            .bind(id)
            .bind(&note)
            .execute(&state.pool)
            .await;
            match res {
                Ok(r) if r.rows_affected() > 0 => ("Report dismissed", "ok"),
                Ok(_) => ("Report was not pending", "denied"),
                Err(_) => ("Update failed", "error"),
            }
        }
        "remove" => {
            // Deleting the review cascades and removes its report rows too.
            let res = sqlx::query(
                "DELETE FROM reviews WHERE id = (SELECT review_id FROM review_reports WHERE id = $1)",
            )
            .bind(id)
            .execute(&state.pool)
            .await;
            match res {
                Ok(r) if r.rows_affected() > 0 => ("Review removed", "ok"),
                Ok(_) => ("Review not found", "denied"),
                Err(_) => ("Delete failed", "error"),
            }
        }
        _ => return redirect("/superadmin/moderation?err=Unknown+action"),
    };
    audit(
        &state.pool,
        &sess,
        &format!("review_report.{}", f.action),
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
    // Light type validation: numeric-typed keys must hold a parseable number, so a
    // malformed value can't break a downstream consumer that parses without a fallback.
    let numeric_suffixes = [
        "_percentage",
        "_rate",
        "_count",
        "_days",
        "_secs",
        "_seconds",
        "_limit",
        "_amount",
        "_max",
        "_min",
    ];
    let looks_numeric = numeric_suffixes.iter().any(|suf| f.key.ends_with(suf));
    if looks_numeric && f.value.trim().parse::<f64>().is_err() {
        return redirect("/superadmin/settings?err=Value+must+be+numeric+for+this+key");
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
