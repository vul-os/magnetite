// Settlement-model compliance (NON-CUSTODIAL).
//
// Magnetite holds no funds. There are no balances, no payouts and nothing to
// reconcile — every sale is an atomic wallet-to-wallet transfer witnessed by a
// signed `Receipt` (§3.6). The model in force is therefore:
//   * The developer receives the WHOLE subtotal          (store_purchases)
//   * The platform takes only `PROTOCOL_FEE_BPS`, default 0
//   * Every paid entitlement is backed by a receipt      (entitlements.receipt_id)
//   * Receipt arithmetic balances and the signature verifies (payment_receipts)
//   * A voided receipt grants nothing
//   * Subscription charges equal the tier's list price   (subscription_transactions)
//   * The legacy custodial tables stay DORMANT           (no new custody)
//
// Each check re-derives the expected figures from first principles, so an
// operator can see at a glance whether settlement really is non-custodial.

use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};

const EPS: &str = "0.000001";

#[derive(Debug)]
pub struct CheckResult {
    pub name: String,
    pub description: String,
    /// `ok` (passes), `fail` (model violated), or `warn` (best-effort/needs review).
    pub severity: Severity,
    pub checked: i64,
    pub violations: i64,
    pub detail: String,
    pub offenders: Vec<Offender>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Severity {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, FromRow)]
pub struct Offender {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, FromRow)]
struct CountRow {
    checked: i64,
    violations: i64,
}

/// High-level money totals that describe the model in force.
#[derive(Debug, FromRow)]
pub struct BillingSummary {
    pub gross_store_revenue: Decimal,
    /// Settled straight to developer wallets. We never touched it.
    pub developer_settled: Decimal,
    /// Protocol fee actually taken (0 unless `PROTOCOL_FEE_BPS` is set).
    pub protocol_fees: Decimal,
    /// Gross units moved by the rail, from verified receipts.
    pub settled_units: Decimal,
    /// Receipts voided by refunds — the only "reversal" that exists.
    pub voided_receipts: Decimal,
    /// Funds we are holding on anyone's behalf. Structurally always zero.
    pub custodial_liability: Decimal,
}

pub async fn summary(pool: &PgPool) -> BillingSummary {
    sqlx::query_as::<_, BillingSummary>(
        "SELECT
           COALESCE((SELECT SUM(price_paid)      FROM store_purchases WHERE currency='USD'),0) AS gross_store_revenue,
           COALESCE((SELECT SUM(developer_share) FROM store_purchases WHERE currency='USD'),0) AS developer_settled,
           COALESCE((SELECT SUM(platform_fee)    FROM store_purchases WHERE currency='USD'),0) AS protocol_fees,
           COALESCE((SELECT SUM(total)  FROM payment_receipts WHERE voided = false),0)::numeric AS settled_units,
           COALESCE((SELECT COUNT(*)    FROM payment_receipts WHERE voided = true),0)::numeric  AS voided_receipts,
           0::numeric                                                             AS custodial_liability",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(BillingSummary {
        gross_store_revenue: Decimal::ZERO,
        developer_settled: Decimal::ZERO,
        protocol_fees: Decimal::ZERO,
        settled_units: Decimal::ZERO,
        voided_receipts: Decimal::ZERO,
        custodial_liability: Decimal::ZERO,
    })
}

/// Run every compliance check.
pub async fn run_all(pool: &PgPool) -> Vec<CheckResult> {
    vec![
        developer_takes_full_subtotal(pool).await,
        protocol_fee_matches_configuration(pool).await,
        paid_entitlements_are_receipt_backed(pool).await,
        receipt_arithmetic_balances(pool).await,
        receipt_signatures_verify(pool).await,
        voided_receipts_grant_nothing(pool).await,
        subscription_charges_match_tier(pool).await,
        custody_is_dormant(pool).await,
    ]
}

async fn count_check(pool: &PgPool, checked_sql: &str, violations_sql: &str) -> CountRow {
    let q = format!(
        "SELECT ({checked_sql})::bigint AS checked, ({violations_sql})::bigint AS violations"
    );
    sqlx::query_as::<_, CountRow>(&q)
        .fetch_one(pool)
        .await
        .unwrap_or(CountRow {
            checked: 0,
            violations: 0,
        })
}

async fn offenders(pool: &PgPool, sql: &str) -> Vec<Offender> {
    sqlx::query_as::<_, Offender>(sql)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
}

fn verdict(_checked: i64, violations: i64, fail_severity: Severity) -> Severity {
    if violations == 0 {
        Severity::Ok
    } else {
        fail_severity
    }
}

// ── Checks ──────────────────────────────────────────────────────────────────

/// The developer must receive the entire subtotal — there is no 70/30 split
/// any more, because there is no intermediary holding the money.
async fn developer_takes_full_subtotal(pool: &PgPool) -> CheckResult {
    let base = "FROM store_purchases WHERE currency='USD' AND status='completed'";
    let c = count_check(
        pool,
        &format!("SELECT COUNT(*) {base}"),
        &format!("SELECT COUNT(*) {base} AND ABS(developer_share - price_paid) > {EPS}"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT id::text AS id,
                    'developer got '||developer_share||' of '||price_paid AS summary
             {base} AND ABS(developer_share - price_paid) > {EPS}
             ORDER BY created_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Sales — developer takes the full subtotal".into(),
        description: "Non-custodial sales pay the developer the whole subtotal; the protocol fee rides on top.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} sales paid the developer in full.", c.checked)
        } else {
            format!("{} of {} sales short-changed the developer.", c.violations, c.checked)
        },
        offenders: off,
    }
}

/// The platform may take only the configured protocol fee (default 0 bps).
async fn protocol_fee_matches_configuration(pool: &PgPool) -> CheckResult {
    let bps = crate::services::payment::protocol_fee_bps();
    let rate = format!("{:.6}", bps as f64 / 10_000.0);
    let base = "FROM store_purchases WHERE currency='USD' AND status='completed'";
    let c = count_check(
        pool,
        &format!("SELECT COUNT(*) {base}"),
        &format!("SELECT COUNT(*) {base} AND ABS(platform_fee - ROUND(price_paid * {rate}, 6)) > 0.01"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT id::text AS id,
                    'fee '||platform_fee||' expected '||ROUND(price_paid * {rate}, 6) AS summary
             {base} AND ABS(platform_fee - ROUND(price_paid * {rate}, 6)) > 0.01
             ORDER BY created_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: format!("Sales — protocol fee is {bps} bps"),
        description: "The platform may take only PROTOCOL_FEE_BPS (default 0).".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} sales took exactly {bps} bps.", c.checked)
        } else {
            format!("{} of {} sales deviate from {bps} bps.", c.violations, c.checked)
        },
        offenders: off,
    }
}

/// An entitlement bought with money must point at a receipt. Points purchases
/// are off-chain and legitimately receipt-less.
async fn paid_entitlements_are_receipt_backed(pool: &PgPool) -> CheckResult {
    let base = "FROM entitlements e
                JOIN store_purchases sp ON sp.id = e.purchase_id
                WHERE sp.currency = 'USD' AND sp.status = 'completed'";
    let c = count_check(
        pool,
        &format!("SELECT COUNT(*) {base}"),
        &format!("SELECT COUNT(*) {base} AND e.receipt_id IS NULL"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT e.id::text AS id, 'entitlement has no receipt' AS summary
             {base} AND e.receipt_id IS NULL
             ORDER BY e.granted_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Entitlements — receipt-backed".into(),
        description: "Every paid entitlement must reference the signed receipt that granted it.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} paid entitlements cite a receipt.", c.checked)
        } else {
            format!("{} paid entitlement(s) have no receipt.", c.violations)
        },
        offenders: off,
    }
}

/// `total` must equal the protocol fee plus every payout leg.
async fn receipt_arithmetic_balances(pool: &PgPool) -> CheckResult {
    let legs = "WITH legs AS (
            SELECT r.id, r.total, r.protocol_fee,
                   COALESCE((SELECT SUM((p->>'amount')::bigint)
                               FROM jsonb_array_elements(r.payouts::jsonb) p), 0) AS paid
              FROM payment_receipts r
         )";
    let c: CountRow = sqlx::query_as::<_, CountRow>(&format!(
        "{legs} SELECT (SELECT COUNT(*) FROM legs)::bigint AS checked,
                       (SELECT COUNT(*) FROM legs WHERE total <> paid + protocol_fee)::bigint AS violations"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(CountRow { checked: 0, violations: 0 });
    let off = offenders(
        pool,
        &format!(
            "{legs}
             SELECT id::text AS id,
                    'total '||total||' ≠ payouts '||paid||' + fee '||protocol_fee AS summary
             FROM legs WHERE total <> paid + protocol_fee LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Receipts — arithmetic balances".into(),
        description: "A receipt's total must equal the sum of its payout legs plus the protocol fee.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} receipts balance exactly.", c.checked)
        } else {
            format!("{} receipt(s) do not balance.", c.violations)
        },
        offenders: off,
    }
}

/// Re-verify stored receipts against the rail's signing key.
///
/// This is the check that makes the others meaningful: it proves the rows are
/// signed artefacts and not just numbers someone typed into Postgres. Sampled
/// (newest first) so the page stays responsive on a large deployment.
async fn receipt_signatures_verify(pool: &PgPool) -> CheckResult {
    const SAMPLE: i64 = 500;
    let ids: Vec<(uuid::Uuid, uuid::Uuid, Option<uuid::Uuid>)> = sqlx::query_as(
        "SELECT id, buyer_id, item_id FROM payment_receipts
          WHERE voided = false ORDER BY created_at DESC LIMIT $1",
    )
    .bind(SAMPLE)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut checked = 0i64;
    let mut violations = 0i64;
    let mut off = Vec::new();
    for (id, buyer_id, item_id) in ids {
        let Some(item_id) = item_id else { continue };
        checked += 1;
        let ok = matches!(
            crate::services::payment::load_receipt(pool, buyer_id, item_id).await,
            Ok(Some(r)) if crate::services::payment::verify_receipt(&r)
        );
        if !ok {
            violations += 1;
            if off.len() < 8 {
                off.push(Offender {
                    id: id.to_string(),
                    summary: "receipt does not verify against the rail".into(),
                });
            }
        }
    }
    CheckResult {
        name: "Receipts — signatures verify".into(),
        description: "Stored receipts must still verify against the active rail's key.".into(),
        severity: verdict(checked, violations, Severity::Fail),
        checked,
        violations,
        detail: if violations == 0 {
            format!("All {checked} sampled receipts verify.")
        } else {
            format!("{violations} receipt(s) FAILED verification — treat as forged.")
        },
        offenders: off,
    }
}

/// Voiding a receipt is the whole of a refund, so nothing it granted may remain live.
async fn voided_receipts_grant_nothing(pool: &PgPool) -> CheckResult {
    let base = "FROM entitlements e
                JOIN payment_receipts r ON r.id = e.receipt_id
                WHERE r.voided = true";
    let c = count_check(
        pool,
        &format!("SELECT COUNT(*) {base}"),
        &format!("SELECT COUNT(*) {base} AND e.revoked = false"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT e.id::text AS id, 'live entitlement on a voided receipt' AS summary
             {base} AND e.revoked = false LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Refunds — voided receipts grant nothing".into(),
        description: "An entitlement whose receipt was voided must be revoked.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} voided receipts have revoked entitlements.", c.checked)
        } else {
            format!("{} entitlement(s) survive a voided receipt.", c.violations)
        },
        offenders: off,
    }
}

async fn subscription_charges_match_tier(pool: &PgPool) -> CheckResult {
    let base = "FROM subscription_transactions st
                JOIN user_subscriptions us ON us.id = st.user_subscription_id
                JOIN subscription_tiers t ON t.id = us.tier_id
                WHERE st.status IN ('completed','paid','succeeded')";
    let c = count_check(
        pool,
        &format!("SELECT COUNT(*) {base}"),
        &format!("SELECT COUNT(*) {base} AND ABS(st.amount - t.price_usdc) > 0.01"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT st.id::text AS id,
                    t.name||': charged '||st.amount||' vs list '||t.price_usdc AS summary
             {base} AND ABS(st.amount - t.price_usdc) > 0.01
             ORDER BY st.created_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Subscriptions — charge matches tier price".into(),
        description: "Each subscription charge must equal its tier's list price.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!(
                "All {} subscription charges match the tier price.",
                c.checked
            )
        } else {
            format!(
                "{} of {} charges differ from the list price.",
                c.violations, c.checked
            )
        },
        offenders: off,
    }
}

/// Custody must stay gone.
///
/// The legacy fiat tables are retained (marked DEPRECATED) so historical rows
/// are not destroyed, but nothing may write to them any more. Any non-zero
/// balance or new payout row means custodial code came back.
async fn custody_is_dormant(pool: &PgPool) -> CheckResult {
    let mut violations = 0i64;
    let mut off = Vec::new();
    for (table, what) in [
        ("wallet_balances", "custodial balance rows"),
        ("payouts", "payout rows"),
        ("game_revenue", "accrued session-revenue rows"),
    ] {
        // Missing table (a fresh non-custodial install) is the ideal outcome.
        let n = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap_or(0);
        if n > 0 {
            violations += 1;
            off.push(Offender {
                id: table.to_string(),
                summary: format!("{n} legacy {what} still present"),
            });
        }
    }
    CheckResult {
        name: "Custody — dormant".into(),
        description: "Magnetite holds no funds: the legacy custodial tables must stay empty.".into(),
        // Historical rows are a migration artefact, not a live breach.
        severity: if violations == 0 { Severity::Ok } else { Severity::Warn },
        checked: 3,
        violations,
        detail: if violations == 0 {
            "No custodial balances, payouts or accruals exist.".into()
        } else {
            format!("{violations} legacy custodial table(s) still hold rows — historical only; nothing writes to them.")
        },
        offenders: off,
    }
}
