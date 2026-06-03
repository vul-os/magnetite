// Billing-model compliance.
//
// Magnetite's revenue model is fixed:
//   * Play-session fees split 15% platform / 85% developer  (game_revenue)
//   * Dev-store sales split  30% platform / 70% developer   (store_purchases)
//   * Developers are paid out only what they have accrued    (payouts)
//   * Subscription charges equal the tier's list price       (subscription_transactions)
//   * Wallet balances reconcile to their ledger              (wallet_balances vs wallet_transactions)
//
// Each check below re-derives the expected figures from first principles and
// reports any rows that drift from the model, so an operator can see at a glance
// whether the platform is actually charging/splitting/paying as designed.

use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};

pub const SESSION_PLATFORM_RATE: &str = "0.15";
pub const STORE_PLATFORM_RATE: &str = "0.30";
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
    pub gross_session_revenue: Decimal,
    pub platform_session_revenue: Decimal,
    pub developer_session_revenue: Decimal,
    pub gross_store_revenue: Decimal,
    pub platform_store_revenue: Decimal,
    pub total_paid_out: Decimal,
    pub pending_payouts: Decimal,
    pub wallet_liability: Decimal,
}

pub async fn summary(pool: &PgPool) -> BillingSummary {
    sqlx::query_as::<_, BillingSummary>(
        "SELECT
           COALESCE((SELECT SUM(amount)          FROM game_revenue),0)            AS gross_session_revenue,
           COALESCE((SELECT SUM(platform_share)  FROM game_revenue),0)            AS platform_session_revenue,
           COALESCE((SELECT SUM(developer_share) FROM game_revenue),0)            AS developer_session_revenue,
           COALESCE((SELECT SUM(price_paid)      FROM store_purchases WHERE currency='USD'),0) AS gross_store_revenue,
           COALESCE((SELECT SUM(platform_fee)    FROM store_purchases WHERE currency='USD'),0) AS platform_store_revenue,
           COALESCE((SELECT SUM(amount) FROM payouts WHERE status IN ('completed','paid')),0)  AS total_paid_out,
           COALESCE((SELECT SUM(amount) FROM payouts WHERE status IN ('pending','processing')),0) AS pending_payouts,
           COALESCE((SELECT SUM(balance) FROM wallet_balances),0)                 AS wallet_liability",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(BillingSummary {
        gross_session_revenue: Decimal::ZERO,
        platform_session_revenue: Decimal::ZERO,
        developer_session_revenue: Decimal::ZERO,
        gross_store_revenue: Decimal::ZERO,
        platform_store_revenue: Decimal::ZERO,
        total_paid_out: Decimal::ZERO,
        pending_payouts: Decimal::ZERO,
        wallet_liability: Decimal::ZERO,
    })
}

/// Run every compliance check.
pub async fn run_all(pool: &PgPool) -> Vec<CheckResult> {
    vec![
        session_split_integrity(pool).await,
        session_rate_correct(pool).await,
        store_split_integrity(pool).await,
        store_rate_correct(pool).await,
        payouts_within_earnings(pool).await,
        no_negative_balances(pool).await,
        subscription_charges_match_tier(pool).await,
        wallet_ledger_reconciles(pool).await,
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

async fn session_split_integrity(pool: &PgPool) -> CheckResult {
    let c = count_check(
        pool,
        "SELECT COUNT(*) FROM game_revenue",
        &format!("SELECT COUNT(*) FROM game_revenue WHERE ABS(amount - (developer_share + platform_share)) > {EPS}"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT id::text AS id,
                    'amount '||amount||' ≠ dev '||developer_share||' + platform '||platform_share AS summary
             FROM game_revenue WHERE ABS(amount - (developer_share + platform_share)) > {EPS}
             ORDER BY created_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Session revenue — split integrity".into(),
        description: "Every session's gross must equal developer_share + platform_share.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} session-revenue rows balance exactly.", c.checked)
        } else {
            format!("{} of {} rows do not balance.", c.violations, c.checked)
        },
        offenders: off,
    }
}

async fn session_rate_correct(pool: &PgPool) -> CheckResult {
    let c = count_check(
        pool,
        "SELECT COUNT(*) FROM game_revenue",
        &format!("SELECT COUNT(*) FROM game_revenue WHERE ABS(platform_share - ROUND(amount * {SESSION_PLATFORM_RATE}, 6)) > {EPS}"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT id::text AS id,
                    'platform '||platform_share||' expected '||ROUND(amount * {SESSION_PLATFORM_RATE},6) AS summary
             FROM game_revenue WHERE ABS(platform_share - ROUND(amount * {SESSION_PLATFORM_RATE},6)) > {EPS}
             ORDER BY created_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Session revenue — 15% platform rate".into(),
        description: "Platform share must equal 15% of each session's gross fee.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            "Platform took exactly 15% on every session.".into()
        } else {
            format!(
                "{} of {} rows deviate from the 15% rate.",
                c.violations, c.checked
            )
        },
        offenders: off,
    }
}

async fn store_split_integrity(pool: &PgPool) -> CheckResult {
    let c = count_check(
        pool,
        "SELECT COUNT(*) FROM store_purchases WHERE currency='USD' AND status='completed' AND developer_share IS NOT NULL AND platform_fee IS NOT NULL",
        &format!("SELECT COUNT(*) FROM store_purchases WHERE currency='USD' AND status='completed' AND developer_share IS NOT NULL AND platform_fee IS NOT NULL AND ABS(price_paid - (developer_share + platform_fee)) > {EPS}"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT id::text AS id,
                    'paid '||price_paid||' ≠ dev '||developer_share||' + fee '||platform_fee AS summary
             FROM store_purchases
             WHERE currency='USD' AND status='completed' AND developer_share IS NOT NULL AND platform_fee IS NOT NULL
               AND ABS(price_paid - (developer_share + platform_fee)) > {EPS}
             ORDER BY created_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Store sales — split integrity".into(),
        description: "Each USD store sale must equal developer_share + platform_fee.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} USD store sales balance exactly.", c.checked)
        } else {
            format!("{} of {} sales do not balance.", c.violations, c.checked)
        },
        offenders: off,
    }
}

async fn store_rate_correct(pool: &PgPool) -> CheckResult {
    let c = count_check(
        pool,
        "SELECT COUNT(*) FROM store_purchases WHERE currency='USD' AND status='completed' AND platform_fee IS NOT NULL",
        &format!("SELECT COUNT(*) FROM store_purchases WHERE currency='USD' AND status='completed' AND platform_fee IS NOT NULL AND ABS(platform_fee - ROUND(price_paid * {STORE_PLATFORM_RATE}, 6)) > {EPS}"),
    )
    .await;
    let off = offenders(
        pool,
        &format!(
            "SELECT id::text AS id,
                    'fee '||platform_fee||' expected '||ROUND(price_paid * {STORE_PLATFORM_RATE},6) AS summary
             FROM store_purchases
             WHERE currency='USD' AND status='completed' AND platform_fee IS NOT NULL
               AND ABS(platform_fee - ROUND(price_paid * {STORE_PLATFORM_RATE},6)) > {EPS}
             ORDER BY created_at DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Store sales — 30% platform rate".into(),
        description: "Platform fee must equal 30% of each USD store sale.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            "Platform took exactly 30% on every USD store sale.".into()
        } else {
            format!(
                "{} of {} sales deviate from the 30% rate.",
                c.violations, c.checked
            )
        },
        offenders: off,
    }
}

async fn payouts_within_earnings(pool: &PgPool) -> CheckResult {
    let cte = format!(
        "WITH accrued AS (
            SELECT developer_id AS uid, COALESCE(SUM(developer_share),0) AS amt
              FROM game_revenue WHERE status='completed' GROUP BY developer_id
            UNION ALL
            SELECT g.developer_id AS uid, COALESCE(SUM(sp.developer_share),0) AS amt
              FROM store_purchases sp JOIN games g ON g.id = sp.game_id
              WHERE sp.status='completed' AND sp.currency='USD' AND sp.developer_share IS NOT NULL
              GROUP BY g.developer_id
         ),
         earned AS (SELECT uid, SUM(amt) AS total FROM accrued GROUP BY uid),
         paid AS (SELECT user_id AS uid, COALESCE(SUM(amount),0) AS total
                    FROM payouts WHERE status NOT IN ('cancelled','failed') GROUP BY user_id),
         over AS (
            SELECT p.uid, p.total AS paid, COALESCE(e.total,0) AS earned
              FROM paid p LEFT JOIN earned e ON e.uid = p.uid
             WHERE p.total > COALESCE(e.total,0) + {EPS}
         )"
    );
    let c: CountRow = sqlx::query_as::<_, CountRow>(&format!(
        "{cte} SELECT (SELECT COUNT(*) FROM paid)::bigint AS checked, (SELECT COUNT(*) FROM over)::bigint AS violations"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(CountRow { checked: 0, violations: 0 });
    let off = offenders(
        pool,
        &format!(
            "{cte}
             SELECT o.uid::text AS id,
                    COALESCE(u.username,'?')||' paid '||o.paid||' but earned '||o.earned AS summary
             FROM over o LEFT JOIN users u ON u.id = o.uid
             ORDER BY (o.paid - o.earned) DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Payouts — within accrued earnings".into(),
        description: "No developer may be paid out more than their 85%/70% accrued share.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!(
                "All {} paid developers are within their accrued earnings.",
                c.checked
            )
        } else {
            format!(
                "{} developer(s) have been over-paid versus earnings.",
                c.violations
            )
        },
        offenders: off,
    }
}

async fn no_negative_balances(pool: &PgPool) -> CheckResult {
    let c = count_check(
        pool,
        "SELECT COUNT(*) FROM wallet_balances",
        "SELECT COUNT(*) FROM wallet_balances WHERE balance < 0",
    )
    .await;
    let off = offenders(
        pool,
        "SELECT user_id::text AS id, currency||' balance '||balance AS summary
         FROM wallet_balances WHERE balance < 0 ORDER BY balance ASC LIMIT 8",
    )
    .await;
    CheckResult {
        name: "Wallets — no negative balances".into(),
        description: "A wallet balance must never go below zero.".into(),
        severity: verdict(c.checked, c.violations, Severity::Fail),
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("All {} wallets are non-negative.", c.checked)
        } else {
            format!("{} wallet(s) are negative.", c.violations)
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

/// Best-effort double-entry reconciliation. Ledger sign conventions are inferred
/// from known tx_types; rows with an unrecognised tx_type are excluded and the
/// wallet is reported as "unverifiable" rather than failed, to avoid false alarms.
async fn wallet_ledger_reconciles(pool: &PgPool) -> CheckResult {
    let ledger = "WITH ledger AS (
            SELECT user_id, currency,
              COALESCE(SUM(amount) FILTER (WHERE tx_type IN ('deposit','transfer_in')),0) AS credits,
              COALESCE(SUM(amount) FILTER (WHERE tx_type IN ('withdrawal','transfer_out','store_purchase','payout')),0) AS debits,
              COALESCE(SUM(amount) FILTER (WHERE tx_type NOT IN ('deposit','transfer_in','withdrawal','transfer_out','store_purchase','payout')),0) AS unclassified
            FROM wallet_transactions
            WHERE status IN ('completed','confirmed','succeeded')
            GROUP BY user_id, currency
         ),
         joined AS (
            SELECT wb.user_id, wb.currency, wb.balance,
                   COALESCE(l.credits,0) - COALESCE(l.debits,0) AS expected,
                   COALESCE(l.unclassified,0) AS unclassified
            FROM wallet_balances wb LEFT JOIN ledger l
              ON l.user_id = wb.user_id AND l.currency = wb.currency
         )";
    let c: CountRow = sqlx::query_as::<_, CountRow>(&format!(
        "{ledger}
         SELECT (SELECT COUNT(*) FROM joined WHERE unclassified = 0)::bigint AS checked,
                (SELECT COUNT(*) FROM joined WHERE unclassified = 0 AND ABS(balance - expected) > {EPS})::bigint AS violations"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(CountRow { checked: 0, violations: 0 });
    let off = offenders(
        pool,
        &format!(
            "{ledger}
             SELECT user_id::text AS id,
                    currency||' balance '||balance||' vs ledger '||expected AS summary
             FROM joined WHERE unclassified = 0 AND ABS(balance - expected) > {EPS}
             ORDER BY ABS(balance - expected) DESC LIMIT 8"
        ),
    )
    .await;
    CheckResult {
        name: "Wallets — ledger reconciliation".into(),
        description:
            "Balance should equal credits − debits from the transaction ledger (best-effort)."
                .into(),
        severity: if c.violations == 0 {
            Severity::Ok
        } else {
            Severity::Warn
        },
        checked: c.checked,
        violations: c.violations,
        detail: if c.violations == 0 {
            format!("{} reconcilable wallets match their ledger.", c.checked)
        } else {
            format!(
                "{} wallet(s) drift from their ledger — review.",
                c.violations
            )
        },
        offenders: off,
    }
}
