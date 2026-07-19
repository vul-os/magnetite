// Receipt-gated join (§3.5 + §3.6).
//
// A paid room hands out no join credential until the payer can show a verified,
// non-voided payment receipt for it. The check is deliberately cryptographic
// rather than a boolean column: we reconstruct the signed `Receipt` from its
// stored fields and re-verify it against the active rail. A row someone edited
// straight in the database does not open a paid room.
//
// This module only READS the payments tables — minting, splitting and storing
// receipts belong to `services/payment.rs` and are not touched here.

use magnetite_seams::identity::{PubKey, Sig};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::services::payment::{verify_receipt, PayOut, Receipt};

/// Why a join was refused. Rendered as a 402 by the API layer.
pub const PAYMENT_REQUIRED: &str =
    "this room requires payment — complete checkout, then join with the receipt";

/// Whether `user_id` holds a verified receipt admitting them to `room_id`.
///
/// Free rooms (`price_units == 0`) short-circuit to `true`. Paid rooms require
/// a `payment_receipts` row that (a) belongs to this buyer, (b) references this
/// room via `item_id`, (c) is not voided, (d) covers the price, and (e) still
/// verifies against the rail's signing key.
pub async fn has_paid(pool: &PgPool, user_id: Uuid, room_id: Uuid, price_units: i64) -> bool {
    if price_units <= 0 {
        return true;
    }
    match load_room_receipt(pool, user_id, room_id).await {
        Ok(Some(r)) => r.total >= price_units as u64 && verify_receipt(&r),
        Ok(None) => false,
        Err(e) => {
            // Fail CLOSED: an unreadable payments table must never mint a free
            // credential for a paid room.
            tracing::error!("comms: receipt lookup failed for room {room_id}: {e}");
            false
        }
    }
}

/// `has_paid`, as a `Result` for handler use.
pub async fn require_paid(
    pool: &PgPool,
    user_id: Uuid,
    room_id: Uuid,
    price_units: i64,
) -> Result<()> {
    if has_paid(pool, user_id, room_id, price_units).await {
        Ok(())
    } else {
        Err(AppError::Validation(PAYMENT_REQUIRED.to_string()))
    }
}

/// Reconstruct the newest matching signed receipt from its stored columns.
async fn load_room_receipt(
    pool: &PgPool,
    user_id: Uuid,
    room_id: Uuid,
) -> Result<Option<Receipt>> {
    let row: Option<(String, i64, i64, serde_json::Value, String, String, String)> =
        sqlx::query_as(
            r#"
            SELECT buyer_pubkey, total, protocol_fee, payouts, nonce, rail_pubkey, sig
              FROM payment_receipts
             WHERE buyer_id = $1
               AND item_id  = $2
               AND voided   = false
             ORDER BY created_at DESC
             LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(room_id)
        .fetch_optional(pool)
        .await?;

    let Some((buyer, total, fee, payouts, nonce, rail_pk, sig)) = row else {
        return Ok(None);
    };

    // Any malformed field means the row cannot be the receipt it claims to be.
    let Ok(buyer) = PubKey::from_hex(&buyer) else {
        return Ok(None);
    };
    let Ok(rail_pubkey) = PubKey::from_hex(&rail_pk) else {
        return Ok(None);
    };
    let Some(nonce) = hex::decode(&nonce).ok().and_then(|b| <[u8; 32]>::try_from(b).ok()) else {
        return Ok(None);
    };
    let Some(sig) = hex::decode(&sig).ok().and_then(|b| <[u8; 64]>::try_from(b).ok()) else {
        return Ok(None);
    };

    let empty = Vec::new();
    let mut parsed = Vec::new();
    for p in payouts.as_array().unwrap_or(&empty) {
        let (Some(w), Some(a)) = (p.get("wallet").and_then(|v| v.as_str()), p.get("amount").and_then(|v| v.as_u64()))
        else {
            return Ok(None);
        };
        let Ok(wallet) = PubKey::from_hex(w) else {
            return Ok(None);
        };
        parsed.push(PayOut { wallet, amount: a });
    }

    Ok(Some(Receipt {
        buyer,
        payouts: parsed,
        protocol_fee: fee.max(0) as u64,
        total: total.max(0) as u64,
        nonce,
        rail_pubkey,
        sig: Sig(sig),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn free_rooms_need_no_receipt_and_no_database() {
        // price 0 short-circuits before any query, so a bogus pool is never used.
        let pool = PgPool::connect_lazy("postgres://invalid/invalid").unwrap();
        assert!(has_paid(&pool, Uuid::new_v4(), Uuid::new_v4(), 0).await);
    }

    #[tokio::test]
    async fn paid_rooms_fail_closed_when_payments_are_unreadable() {
        let pool = PgPool::connect_lazy("postgres://127.0.0.1:1/nope").unwrap();
        assert!(
            !has_paid(&pool, Uuid::new_v4(), Uuid::new_v4(), 500).await,
            "an unreachable payments table must never admit a paying room"
        );
    }
}
