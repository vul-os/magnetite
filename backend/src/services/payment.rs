// Payment service — NON-CUSTODIAL crypto only (seam §3.6).
//
// All fiat is gone: no Paystack on-ramp, no Wise payouts, no platform-held
// balances. Money moves buyer-wallet → seller-wallet through the `PaymentRail`
// seam and the signed `Receipt` is the entitlement. Subscriptions/tiers were
// removed entirely (the platform charges nothing); the one real checkout path
// is dev→player through `magnetite-seams` → `patala-solana`. The payment rail
// itself lives below.
#![allow(dead_code)]

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

// ─── Non-custodial payment rail (seam §3.6) ───────────────────────────────────
//
// There is no custody here: no balances, no deposits, no withdrawals, no payouts.
// A purchase is an atomic wallet→wallet transfer produced by a `PaymentRail`
// implementation; the resulting signed `Receipt` IS the entitlement.
//
// The default rail is `MockPaymentRail` — deterministic, offline, zero external
// services — selected by `PAYMENT_RAIL=mock` (the default).
//
// `PAYMENT_RAIL=solana` (requires `--features solana`) selects the real SPL-USDC
// rail. Selection happens HERE and nowhere else: no module outside this one
// names a chain type. Misconfiguration PANICS at startup rather than falling
// back to the mock — a silent fallback in production would hand out every paid
// item for free.

use std::sync::OnceLock;

pub use magnetite_seams::identity::{PubKey, Sig};
pub use magnetite_seams::payment::{
    ChainBinding, Channel, MockPaymentRail, PayOut, PaymentRail, PaymentSplit,
    Receipt, Split,
};

/// Protocol fee in basis points. Default `0` (governance decides any real fee).
pub fn protocol_fee_bps() -> u16 {
    std::env::var("PROTOCOL_FEE_BPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Which rail `PAYMENT_RAIL` selects. Unknown values are FATAL.
fn rail_kind() -> String {
    std::env::var("PAYMENT_RAIL").unwrap_or_else(|_| "mock".to_string())
}

/// The process-wide payment rail. Default `mock` — fully offline.
///
/// # Fail loud, never fall back
///
/// If `PAYMENT_RAIL` names a rail that is unknown, not compiled in, or
/// misconfigured, this **panics**. It must not degrade to the mock: the mock
/// signs receipts for free, so a production process that quietly fell back to it
/// would give every paid item, paid room and hosted server away for nothing.
pub fn rail() -> &'static dyn PaymentRail {
    static RAIL: OnceLock<Box<dyn PaymentRail + Send + Sync>> = OnceLock::new();
    RAIL.get_or_init(|| match rail_kind().as_str() {
        "mock" => {
            Box::new(MockPaymentRail::with_fee_bps(protocol_fee_bps())) as Box<dyn PaymentRail + Send + Sync>
        }
        #[cfg(feature = "solana")]
        "solana" => Box::new(solana_rail_from_env().unwrap_or_else(|e| {
            panic!(
                "PAYMENT_RAIL=solana is misconfigured: {e}. Refusing to start — falling \
                 back to the mock rail would hand out paid items for free."
            )
        })) as Box<dyn PaymentRail + Send + Sync>,
        #[cfg(not(feature = "solana"))]
        "solana" => panic!(
            "PAYMENT_RAIL=solana but this binary was built WITHOUT `--features solana`. \
             Refusing to start rather than silently using the mock rail."
        ),
        other => panic!(
            "PAYMENT_RAIL={other:?} is not a known payment rail (expected \"mock\" or \
             \"solana\"). Refusing to start."
        ),
    })
    .as_ref()
}

/// Build the Solana rail from the environment, validating every field.
///
/// | env | meaning |
/// |---|---|
/// | `SOLANA_RPC_URL` | JSON-RPC endpoint (required) |
/// | `SOLANA_CLUSTER` | `mainnet-beta` \| `devnet` \| `testnet` \| `localnet` (required) |
/// | `SOLANA_COMMITMENT` | `confirmed` \| `finalized` (default `finalized`) |
/// | `SOLANA_USDC_MINT` | base58 mint; defaults to the canonical mint for the cluster |
/// | `SOLANA_FEE_WALLET` | base58; REQUIRED when `PROTOCOL_FEE_BPS > 0` |
/// | `SOLANA_KEYPAIR_PATH` / `SOLANA_KEYPAIR` | optional signer (`chmod 600`); absent ⇒ verify-only |
///
/// The actual chain rail (tx construction, signing, RPC, on-chain
/// verification) is `patala_solana::SolanaRail`; this only builds the config
/// and hands it to `magnetite_seams::solana::SolanaPaymentRail`, the thin
/// adapter that keeps magnetite's own `PaymentRail` seam on top of it (see
/// that module's docs and `patala/PATALA.md` §7).
#[cfg(feature = "solana")]
fn solana_rail_from_env(
) -> std::result::Result<magnetite_seams::solana::SolanaPaymentRail, String> {
    use magnetite_seams::solana::{Cluster, Commitment, SolanaConfig, SolanaPaymentRail};
    use patala_solana::{
        rpc::HttpRpc,
        tx::{pubkey_from_base58, USDC_DEVNET_MINT, USDC_MAINNET_MINT},
    };
    use std::sync::Arc;

    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .map_err(|_| "SOLANA_RPC_URL is not set".to_string())?;
    if !rpc_url.starts_with("http://") && !rpc_url.starts_with("https://") {
        return Err(format!("SOLANA_RPC_URL {rpc_url:?} is not an http(s) URL"));
    }
    let cluster = Cluster::parse(
        &std::env::var("SOLANA_CLUSTER").map_err(|_| "SOLANA_CLUSTER is not set".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let commitment = Commitment::parse(
        &std::env::var("SOLANA_COMMITMENT").unwrap_or_else(|_| "finalized".to_string()),
    )
    .map_err(|e| e.to_string())?;

    let default_mint = match cluster {
        Cluster::MainnetBeta => USDC_MAINNET_MINT,
        _ => USDC_DEVNET_MINT,
    };
    let usdc_mint = pubkey_from_base58(
        &std::env::var("SOLANA_USDC_MINT").unwrap_or_else(|_| default_mint.to_string()),
    )
    .map_err(|e| format!("SOLANA_USDC_MINT: {e}"))?;

    // `fee_wallet` stays a magnetite-level concept: `patala_core`'s seam has
    // no multi-party split, so this is only consulted by `SolanaPaymentRail`
    // to decide whether a split collapses to one payable recipient — see
    // `magnetite_seams::solana::SolanaError::MultiPartySplit`.
    let fee_wallet = match std::env::var("SOLANA_FEE_WALLET") {
        Ok(w) => Some(
            pubkey_from_base58(&w)
                .map(|pk| PubKey(pk.0))
                .map_err(|e| format!("SOLANA_FEE_WALLET: {e}"))?,
        ),
        Err(_) => None,
    };
    if protocol_fee_bps() > 0 && fee_wallet.is_none() {
        return Err("PROTOCOL_FEE_BPS > 0 but SOLANA_FEE_WALLET is not set".into());
    }

    if cluster.is_mainnet() {
        tracing::warn!(
            "PAYMENT_RAIL=solana on MAINNET-BETA: this process moves REAL money. \
             commitment={}",
            commitment.as_str(),
        );
    }

    let cfg = SolanaConfig {
        inner: patala_solana::SolanaConfig {
            rpc_url: rpc_url.clone(),
            cluster,
            commitment,
            usdc_mint,
        },
        fee_wallet,
    };
    // Signer (if any) is loaded from SOLANA_KEYPAIR_PATH/SOLANA_KEYPAIR by
    // `patala_solana::keys::Keypair::from_env` inside `from_env` below — never
    // logged, never persisted.
    SolanaPaymentRail::from_env(cfg, Arc::new(HttpRpc::new(rpc_url))).map_err(|e| e.to_string())
}

/// Verify a receipt against the active rail (signature + internal arithmetic).
pub fn verify_receipt(r: &Receipt) -> bool {
    rail().verify_receipt(r)
}

/// Convert a USD-denominated `Decimal` price to the rail's smallest unit (cents).
pub fn units_from_usd(price: Decimal) -> u64 {
    use rust_decimal::prelude::ToPrimitive;
    (price * Decimal::new(100, 0))
        .round()
        .to_u64()
        .unwrap_or(u64::MAX)
}

/// The wallet (Ed25519 pubkey) a user has linked, if any. Non-custodial: we only
/// ever record an address, never hold funds.
pub async fn wallet_of(pool: &PgPool, user_id: Uuid) -> Result<Option<PubKey>> {
    let row = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT wallet_address FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .and_then(|r| r.0)
        .and_then(|h| PubKey::from_hex(h.trim_start_matches("0x")).ok()))
}

/// Require a linked wallet, with a role label for the error message.
pub async fn require_wallet(pool: &PgPool, user_id: Uuid, role: &str) -> Result<PubKey> {
    wallet_of(pool, user_id).await?.ok_or_else(|| {
        AppError::Validation(format!(
            "{role} has no linked wallet address — payments are non-custodial, \
             link a wallet before transacting"
        ))
    })
}

/// The operator wallet that receives hosting / subscription fees, if configured.
pub fn operator_wallet() -> Option<PubKey> {
    std::env::var("OPERATOR_WALLET_PUBKEY")
        .ok()
        .and_then(|h| PubKey::from_hex(h.trim_start_matches("0x")).ok())
}

/// Persist a signed receipt. This row is the durable entitlement proof.
#[allow(clippy::too_many_arguments)]
pub async fn store_receipt(
    pool: &PgPool,
    receipt: &Receipt,
    kind: &str,
    buyer_id: Uuid,
    purchase_id: Option<Uuid>,
    item_id: Option<Uuid>,
    game_id: Option<Uuid>,
) -> Result<Uuid> {
    if !verify_receipt(receipt) {
        return Err(AppError::Internal(
            "refusing to store an unverifiable receipt".to_string(),
        ));
    }
    let id = Uuid::new_v4();
    let payouts = serde_json::json!(receipt
        .payouts
        .iter()
        .map(|p| serde_json::json!({ "wallet": p.wallet.to_hex(), "amount": p.amount }))
        .collect::<Vec<_>>());

    sqlx::query(
        r#"
        INSERT INTO payment_receipts
            (id, kind, buyer_id, buyer_pubkey, purchase_id, item_id, game_id,
             total, protocol_fee, payouts, nonce, rail_pubkey, sig, rail, binding, voided, created_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,false,NOW())
        "#,
    )
    .bind(id)
    .bind(kind)
    .bind(buyer_id)
    .bind(receipt.buyer.to_hex())
    .bind(purchase_id)
    .bind(item_id)
    .bind(game_id)
    .bind(receipt.total as i64)
    .bind(receipt.protocol_fee as i64)
    .bind(payouts)
    .bind(hex::encode(receipt.nonce))
    .bind(receipt.rail_pubkey.to_hex())
    .bind(hex::encode(receipt.sig.0))
    .bind(rail_kind())
    .bind(
        receipt
            .binding
            .as_ref()
            .map(|b| serde_json::json!(b)),
    )
    .execute(pool)
    .await?;

    Ok(id)
}

/// Void a receipt (refund path — there is no money to claw back, only proof to revoke).
pub async fn void_receipt_for_purchase(pool: &PgPool, purchase_id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE payment_receipts SET voided = true, voided_at = NOW() WHERE purchase_id = $1",
    )
    .bind(purchase_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Open a hosting-fee payment channel to an operator (per-seat / per-hour).
///
/// TODO(chain): with a real rail this anchors an on-chain channel and the
/// per-join debits are off-chain signed channel updates. The mock rail returns a
/// deterministic channel id so the flow is testable offline.
pub async fn open_hosting_channel(
    pool: &PgPool,
    payer_id: Uuid,
    operator: &PubKey,
    server_id: Option<Uuid>,
) -> Result<Channel> {
    let channel = rail()
        .open_channel(operator)
        .await
        .map_err(|e| AppError::BadRequest(format!("cannot open a hosting channel: {e}")))?;
    sqlx::query(
        r#"
        INSERT INTO hosting_channels
            (id, channel_id, payer_id, operator_pubkey, server_id, rail_pubkey, open, created_at)
        VALUES ($1,$2,$3,$4,$5,$6,true,NOW())
        ON CONFLICT (channel_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(hex::encode(channel.id))
    .bind(payer_id)
    .bind(operator.to_hex())
    .bind(server_id)
    .bind(channel.rail_pubkey.to_hex())
    .execute(pool)
    .await?;
    Ok(channel)
}

/// Charge a hosting fee (per-seat / per-hour) to an operator and record the receipt.
///
/// Scaffold: with the mock rail this is a deterministic offline checkout, so the
/// join-gate below is fully testable without a chain.
/// TODO(chain): debit the open channel with a signed channel update instead of a
/// full checkout, so a join costs no gas.
pub async fn charge_hosting_fee(
    pool: &PgPool,
    payer_id: Uuid,
    operator: &PubKey,
    amount: u64,
    server_id: Option<Uuid>,
) -> Result<Receipt> {
    let payer = require_wallet(pool, payer_id, "player").await?;
    // Ensure a channel exists (idempotent, deterministic id).
    open_hosting_channel(pool, payer_id, operator, server_id).await?;

    let split = sale_split(*operator, amount, None);
    let receipt = rail().checkout(&payer, split).await;
    if !verify_receipt(&receipt) {
        return Err(AppError::Internal(
            "hosting fee receipt failed verification".to_string(),
        ));
    }
    store_receipt(pool, &receipt, "hosting", payer_id, None, None, None).await?;
    Ok(receipt)
}

/// Join-gate for a PAID server: the player must hold a non-voided hosting receipt.
///
/// A server with no hosting fee configured is free to join and returns `true`.
pub async fn has_hosting_access(pool: &PgPool, user_id: Uuid, server_id: Uuid) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payment_receipts r
         JOIN hosting_channels c ON c.payer_id = r.buyer_id
         WHERE r.kind = 'hosting' AND r.voided = false
           AND r.buyer_id = $1 AND c.server_id = $2 AND c.open = true",
    )
    .bind(user_id)
    .bind(server_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

// ── Receipt-backed entitlement checks ───────────────────────────────────────
//
// An entitlement is a SIGNATURE, never a row. Every check below reconstructs the
// stored receipt and re-verifies it against the active rail, so editing the
// database directly grants nothing. All paths fail CLOSED.

/// Reconstruct the newest non-voided receipt for `(buyer_id, item_id)`.
///
/// Returns `Ok(None)` when there is no such receipt OR when the stored row is
/// malformed — a row that cannot be parsed back into the receipt it claims to be
/// is not a receipt.
pub async fn load_receipt(
    pool: &PgPool,
    buyer_id: Uuid,
    item_id: Uuid,
) -> Result<Option<Receipt>> {
    let row: Option<(
        String,
        i64,
        i64,
        serde_json::Value,
        String,
        String,
        String,
        Option<serde_json::Value>,
    )> = sqlx::query_as(
            r#"
            SELECT buyer_pubkey, total, protocol_fee, payouts, nonce, rail_pubkey, sig, binding
              FROM payment_receipts
             WHERE buyer_id = $1
               AND item_id  = $2
               AND voided   = false
             ORDER BY created_at DESC
             LIMIT 1
            "#,
        )
        .bind(buyer_id)
        .bind(item_id)
        .fetch_optional(pool)
        .await?;

    let Some((buyer, total, fee, payouts, nonce, rail_pk, sig, binding)) = row else {
        return Ok(None);
    };

    let Ok(buyer) = PubKey::from_hex(&buyer) else {
        return Ok(None);
    };
    let Ok(rail_pubkey) = PubKey::from_hex(&rail_pk) else {
        return Ok(None);
    };
    let Some(nonce) = hex::decode(&nonce)
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
    else {
        return Ok(None);
    };
    let Some(sig) = hex::decode(&sig)
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
    else {
        return Ok(None);
    };

    let empty = Vec::new();
    let mut parsed = Vec::new();
    for p in payouts.as_array().unwrap_or(&empty) {
        let (Some(w), Some(a)) = (
            p.get("wallet").and_then(|v| v.as_str()),
            p.get("amount").and_then(|v| v.as_u64()),
        ) else {
            return Ok(None);
        };
        let Ok(wallet) = PubKey::from_hex(w) else {
            return Ok(None);
        };
        parsed.push(PayOut { wallet, amount: a });
    }

    // A stored binding that will not parse back is a malformed row, not a
    // receipt — drop it rather than returning a receipt missing its anchor.
    let binding = match binding {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => match serde_json::from_value::<ChainBinding>(v) {
            Ok(b) => Some(b),
            Err(_) => return Ok(None),
        },
    };

    Ok(Some(Receipt {
        buyer,
        payouts: parsed,
        protocol_fee: fee.max(0) as u64,
        total: total.max(0) as u64,
        nonce,
        rail_pubkey,
        sig: Sig(sig),
        binding,
    }))
}

/// Whether `buyer_id` holds a verified, non-voided receipt for `item_id`
/// covering at least `min_units`.
///
/// Fails CLOSED on any error. Also refuses receipts bound to the account's
/// naming-only derived key (see `comms::AccountKey`) — such a receipt could only
/// come from a forged or back-filled row, because checkout always binds a
/// receipt to a LINKED wallet.
pub async fn has_verified_receipt(
    pool: &PgPool,
    buyer_id: Uuid,
    item_id: Uuid,
    min_units: u64,
) -> bool {
    if min_units == 0 {
        return true;
    }
    match load_receipt(pool, buyer_id, item_id).await {
        Ok(Some(r)) => {
            if r.buyer == crate::comms::derived_key(buyer_id) {
                tracing::error!(
                    "payments: receipt for item {item_id} is bound to a DERIVED account key — refusing"
                );
                return false;
            }
            // The receipt-only half of the gate lives in the seam
            // (`receipt_admits`: buyer binding, amount cover, rail signature)
            // so this backend and the offline node path cannot drift apart.
            // The DB-only facts — item binding, not-voided, proven key — are
            // checked here and in `load_receipt`, which is where they live.
            let buyer = r.buyer;
            magnetite_seams::payment::receipt_admits(rail(), &r, &buyer, min_units)
        }
        Ok(None) => false,
        Err(e) => {
            tracing::error!("payments: receipt lookup failed for item {item_id}: {e}");
            false
        }
    }
}

/// Build the split for a single-seller sale: the developer takes the whole
/// subtotal, an optional operator takes a hosting cut, protocol fee rides on top.
pub fn sale_split(developer: PubKey, amount: u64, operator: Option<(PubKey, u64)>) -> PaymentSplit {
    PaymentSplit {
        developer: Split {
            wallet: developer,
            amount,
        },
        operator: operator.map(|(wallet, amount)| Split { wallet, amount }),
        protocol_fee_bps: protocol_fee_bps(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(b: u8) -> PubKey {
        PubKey([b; 32])
    }

    #[test]
    fn usd_converts_to_cents() {
        assert_eq!(units_from_usd(Decimal::new(1999, 2)), 1999);
        assert_eq!(units_from_usd(Decimal::new(5, 0)), 500);
        assert_eq!(units_from_usd(Decimal::ZERO), 0);
    }

    #[test]
    fn default_protocol_fee_is_zero() {
        // No PROTOCOL_FEE_BPS in the test env.
        assert_eq!(
            std::env::var("PROTOCOL_FEE_BPS").ok().is_none(),
            true,
            "test env must not set PROTOCOL_FEE_BPS"
        );
        assert_eq!(protocol_fee_bps(), 0);
    }

    #[tokio::test]
    async fn checkout_produces_verifiable_receipt_offline() {
        let buyer = pk(0xB0);
        let split = sale_split(pk(0xD0), 1999, None);
        let r = rail().checkout(&buyer, split).await;

        assert_eq!(r.total, 1999);
        assert_eq!(r.protocol_fee, 0);
        assert_eq!(r.payouts.len(), 1);
        assert_eq!(r.payouts[0].wallet, pk(0xD0));
        assert!(verify_receipt(&r), "receipt must verify against the rail");
    }

    #[tokio::test]
    async fn tampered_receipt_does_not_grant_entitlement() {
        let buyer = pk(0xB1);
        let mut r = rail().checkout(&buyer, sale_split(pk(0xD1), 500, None)).await;
        assert!(verify_receipt(&r));
        r.payouts[0].amount = 5_000_000;
        assert!(
            !verify_receipt(&r),
            "a forged receipt must never gate an entitlement"
        );
    }

    #[tokio::test]
    async fn operator_cut_is_split_atomically() {
        let buyer = pk(0xB2);
        let r = rail()
            .checkout(&buyer, sale_split(pk(0xD2), 900, Some((pk(0x0B), 100))))
            .await;
        assert_eq!(r.total, 1000);
        assert_eq!(r.payouts.len(), 2);
        assert_eq!(r.payouts[1].amount, 100);
        assert!(verify_receipt(&r));
    }

    #[tokio::test]
    async fn hosting_channel_id_is_deterministic() {
        let op = pk(0x0C);
        let a = rail().open_channel(&op).await.unwrap();
        let b = rail().open_channel(&op).await.unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.peer, op);
    }
}
