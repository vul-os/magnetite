//! Stellar XDR transaction construction, signing base, and hashing —
//! multi-leg only (magnetite's `PaymentSplit` is always a leg list; there is
//! no separate single-recipient shape to also support, unlike
//! `patala-stellar`, which has to serve `patala_core`'s single-recipient seam
//! *and* its own N-leg `charge_split`).
//!
//! Design copied by hand from `patala/patala-stellar/src/tx.rs` (see this
//! crate's `Cargo.toml`): built on [`stellar_xdr`], the Stellar Development
//! Foundation's own crate generated from the same `.x` definitions
//! `stellar-core` uses (Apache-2.0) — using the official codec for
//! struct/union layout removes an entire class of hand-XDR bugs. This module
//! still does the actual transaction construction, the signature base, the
//! hash and the envelope assembly itself.
//!
//! # Wire shape (classic Stellar, `ENVELOPE_TYPE_TX` / v1)
//!
//! One [`Transaction`] with 1 to [`MAX_OPERATIONS`] `PAYMENT` [`Operation`]s,
//! all paid by the transaction's own `source_account`, each moving `amount` of
//! [`Asset::CreditAlphanum4`] ("USDC") to one `destination`. Plus a
//! [`Memo::Hash`] carrying magnetite's own 32-byte `binding_reference`
//! (`lib.rs`) directly — Stellar's memo field IS 32 bytes, so unlike the
//! Solana rail (which had to hex-encode the reference into an SPL Memo
//! *string* instruction), no extra encoding step is needed here.
//! [`Preconditions::None`] (no time bounds), sequence number = the account's
//! current sequence + 1, fee bid = `base_fee_stroops × operation_count`.
//!
//! # Atomicity (`ALIGNMENT.md` §7 / A12)
//!
//! A Stellar transaction is atomic: every operation applies, or none does.
//! That is what lets an N-way split be one settlement event, exactly as the
//! Solana rail's one-transaction-per-checkout design did.
//!
//! # Signing
//!
//! What gets signed is not the transaction's own XDR — it is
//! `SHA256(networkId || XDR(TransactionSignaturePayload{ network_id,
//! tagged_transaction: Tx(tx) }))`, where `networkId = SHA256(network
//! passphrase)`. That 32-byte hash is also the transaction hash Horizon
//! indexes transactions by.
//!
//! # Money math
//!
//! Every asset amount in classic Stellar XDR is a fixed-point `int64` scaled
//! by 10^7 ([`USDC_DECIMALS`]) — there is no per-asset decimals field the way
//! an SPL mint carries one. Nothing here ever divides, rounds, or touches a
//! float.

use sha2::{Digest, Sha256};
use stellar_xdr::curr::{
    AccountId, AlphaNum4, Asset, AssetCode4, DecoratedSignature, Hash, Limits, Memo, MuxedAccount,
    Operation, OperationBody, PaymentOp, Preconditions, PublicKey as XdrPublicKey, ReadXdr,
    SequenceNumber, Signature as XdrSignature, SignatureHint, Transaction, TransactionEnvelope,
    TransactionExt, TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
    TransactionV1Envelope, Uint256, VecM, WriteXdr,
};

use crate::StellarError;

/// USDC on Stellar has no per-asset decimals field; classic Stellar XDR
/// amounts are always this fixed-point scale (10^7).
pub const USDC_DECIMALS: u8 = 7;

/// The most operations one classic Stellar transaction may carry — a protocol
/// limit (`Transaction::operations` is `VecM<Operation, 100>` in the XDR
/// definition itself), not a choice made here.
pub const MAX_OPERATIONS: usize = 100;

fn xdr_pubkey(pk: [u8; 32]) -> XdrPublicKey {
    XdrPublicKey::PublicKeyTypeEd25519(Uint256(pk))
}

fn muxed(pk: [u8; 32]) -> MuxedAccount {
    MuxedAccount::Ed25519(Uint256(pk))
}

/// Pad an asset code (1-4 ASCII alphanumerics, e.g. `"USDC"`) to the 4-byte
/// `AssetCode4` layout. Anything else is rejected, never truncated or coerced.
pub fn asset_code4(code: &str) -> Result<AssetCode4, StellarError> {
    if code.is_empty() || code.len() > 4 || !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(StellarError::Config(format!(
            "asset code {code:?} must be 1-4 ASCII alphanumerics"
        )));
    }
    let mut bytes = [0u8; 4];
    bytes[..code.len()].copy_from_slice(code.as_bytes());
    Ok(AssetCode4(bytes))
}

/// Build the native USDC [`Asset`]: a `CreditAlphanum4` credit asset,
/// `issuer_pk` being the issuing account.
pub fn usdc_asset(code: &str, issuer_pk: [u8; 32]) -> Result<Asset, StellarError> {
    Ok(Asset::CreditAlphanum4(AlphaNum4 {
        asset_code: asset_code4(code)?,
        issuer: AccountId(xdr_pubkey(issuer_pk)),
    }))
}

/// `networkId = SHA256(network passphrase)` — the Stellar-protocol-defined
/// domain separator, so a signature over a testnet transaction can never be
/// replayed as a mainnet one.
pub fn network_id(passphrase: &str) -> [u8; 32] {
    Sha256::digest(passphrase.as_bytes()).into()
}

/// One leg of a payment transaction: one payee, one asset, one integer amount.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentLeg {
    /// The payee's raw Ed25519 public key (a `G…` StrKey address decoded).
    pub dest_pk: [u8; 32],
    /// The asset this leg moves. Always USDC here, but the type is general.
    pub asset: Asset,
    /// The amount, in the asset's fixed-point `int64` units (10^-7). Must be
    /// strictly positive.
    pub amount: i64,
}

impl PaymentLeg {
    /// A leg paying `amount` of `asset` to `dest_pk`.
    pub fn new(dest_pk: [u8; 32], asset: Asset, amount: i64) -> Self {
        Self {
            dest_pk,
            asset,
            amount,
        }
    }
}

/// The `PAYMENT` [`Operation`] for one leg, with no operation-level source
/// override — the transaction's own source account pays every leg.
///
/// Unvalidated on purpose: rebuilding a transaction byte-for-byte from an
/// untrusted receipt — what [`crate::StellarPaymentRail::verify_receipt`]
/// does, offline, to re-derive its hash — has to be able to encode whatever a
/// tampered proof claimed, so that the *hash* is what disagrees.
fn payment_op(leg: &PaymentLeg) -> Operation {
    Operation {
        source_account: None,
        body: OperationBody::Payment(PaymentOp {
            destination: muxed(leg.dest_pk),
            asset: leg.asset.clone(),
            amount: leg.amount,
        }),
    }
}

/// The exact sum of every leg's amount, in stroops. `checked_add` at every
/// step so an overflowing bundle is a refusal instead of a wrapped amount.
pub fn total_amount(legs: &[PaymentLeg]) -> Result<i64, StellarError> {
    let mut total: i64 = 0;
    for (i, leg) in legs.iter().enumerate() {
        total = total.checked_add(leg.amount).ok_or_else(|| {
            StellarError::Config(format!(
                "leg {i}: summing payment amounts overflows Stellar's i64 amount range"
            ))
        })?;
    }
    Ok(total)
}

/// The fee bid for `op_count` operations at `base_fee_stroops` each.
/// stellar-core charges `base_fee × operation_count` and rejects a
/// transaction bidding less as `txINSUFFICIENT_FEE`.
pub fn total_fee(base_fee_stroops: u32, op_count: usize) -> Result<u32, StellarError> {
    let ops = u32::try_from(op_count)
        .map_err(|_| StellarError::Config(format!("absurd operation count {op_count}")))?;
    base_fee_stroops.checked_mul(ops).ok_or_else(|| {
        StellarError::Config(format!(
            "fee bid overflows u32: {base_fee_stroops} stroops x {ops} operations"
        ))
    })
}

/// Build the (unsigned) N-leg payment [`Transaction`], atomically:
/// [`Preconditions::None`], the given sequence number, a fee bid of
/// `base_fee_stroops × legs.len()`, and a [`Memo::Hash`] carrying `memo`.
///
/// Refused, rather than built and left to fail on-chain:
///
/// * **no legs** — a transaction that moves nothing still burns a sequence
///   number and a fee;
/// * **more than [`MAX_OPERATIONS`] legs** — unencodable;
/// * **a non-positive amount on any leg** — stellar-core rejects a `PAYMENT`
///   of `amount <= 0`, taking the whole atomic bundle down with it;
/// * **a fee bid that overflows `u32`**.
pub fn build_payment_transaction(
    source_pk: [u8; 32],
    legs: &[PaymentLeg],
    seq_num: i64,
    base_fee_stroops: u32,
    memo: [u8; 32],
) -> Result<Transaction, StellarError> {
    if legs.is_empty() {
        return Err(StellarError::Config(
            "a payment transaction needs at least one leg".into(),
        ));
    }
    if legs.len() > MAX_OPERATIONS {
        return Err(StellarError::Config(format!(
            "{} legs exceeds Stellar's limit of {MAX_OPERATIONS} operations per transaction; \
             this rail will not split a bundle across several non-atomic transactions",
            legs.len()
        )));
    }
    for (i, leg) in legs.iter().enumerate() {
        if leg.amount <= 0 {
            return Err(StellarError::Config(format!(
                "leg {i}: payment amount must be strictly positive, got {}",
                leg.amount
            )));
        }
    }
    total_amount(legs)?;

    let fee = total_fee(base_fee_stroops, legs.len())?;
    let ops: Vec<Operation> = legs.iter().map(payment_op).collect();
    let operations = VecM::try_from(ops).map_err(|_| {
        StellarError::Xdr(format!(
            "a stellar transaction carries at most {MAX_OPERATIONS} operations"
        ))
    })?;
    Ok(Transaction {
        source_account: muxed(source_pk),
        fee,
        seq_num: SequenceNumber(seq_num),
        cond: Preconditions::None,
        memo: Memo::Hash(Hash(memo)),
        operations,
        ext: TransactionExt::V0,
    })
}

/// The exact bytes that get hashed and signed:
/// `XDR(TransactionSignaturePayload{ network_id, tagged_transaction: Tx(tx) })`.
pub fn signing_payload(net_id: [u8; 32], tx: &Transaction) -> Result<Vec<u8>, StellarError> {
    let payload = TransactionSignaturePayload {
        network_id: Hash(net_id),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    payload
        .to_xdr(Limits::none())
        .map_err(|e| StellarError::Xdr(format!("encode signature payload: {e}")))
}

/// `tx_hash = SHA256(signing_payload)` — also the transaction hash Horizon
/// indexes by, and what an Ed25519 signature over this transaction is over.
pub fn tx_hash(net_id: [u8; 32], tx: &Transaction) -> Result<[u8; 32], StellarError> {
    Ok(Sha256::digest(signing_payload(net_id, tx)?).into())
}

/// Wrap a signed [`Transaction`] into a `ENVELOPE_TYPE_TX`
/// [`TransactionEnvelope`], with a single [`DecoratedSignature`] — hint = last
/// 4 bytes of the signer's public key, per the Stellar spec.
pub fn envelope(
    tx: Transaction,
    signer_pk: [u8; 32],
    signature: [u8; 64],
) -> Result<TransactionEnvelope, StellarError> {
    let mut hint = [0u8; 4];
    hint.copy_from_slice(&signer_pk[28..32]);
    let sig = DecoratedSignature {
        hint: SignatureHint(hint),
        signature: XdrSignature(
            signature
                .to_vec()
                .try_into()
                .expect("a 64-byte Ed25519 signature always fits Signature's BytesM<64>"),
        ),
    };
    Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: VecM::try_from(vec![sig])
            .expect("one signature is well under the 20-signature limit"),
    }))
}

/// Base64-encode a [`TransactionEnvelope`] for Horizon's `POST /transactions`
/// `tx` form field.
pub fn envelope_to_xdr_base64(env: &TransactionEnvelope) -> Result<String, StellarError> {
    env.to_xdr_base64(Limits::none())
        .map_err(|e| StellarError::Xdr(format!("encode envelope: {e}")))
}

/// Decode a base64 [`TransactionEnvelope`] — used by `verify` to check what
/// Horizon actually reports against what a receipt claims.
pub fn envelope_from_xdr_base64(b64: &str) -> Result<TransactionEnvelope, StellarError> {
    TransactionEnvelope::from_xdr_base64(b64, Limits::none())
        .map_err(|e| StellarError::Xdr(format!("decode envelope: {e}")))
}

/// One payment leg pulled back out of a decoded envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedLeg {
    /// The payee's raw Ed25519 public key.
    pub dest_pk: [u8; 32],
    /// The asset this leg moves.
    pub asset: Asset,
    /// The amount, in the asset's fixed-point `int64` units.
    pub amount: i64,
}

/// Every payment leg plus the transaction-level fields, decoded out of a v1
/// envelope for `verify` to compare against a receipt's binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedPayments {
    /// The payer — the transaction's own source account.
    pub source_pk: [u8; 32],
    /// The sequence number consumed.
    pub seq_num: i64,
    /// The total fee bid carried on the transaction (all legs together).
    pub fee: u32,
    /// The `MEMO_HASH` bytes — magnetite's `binding_reference` directly.
    pub memo: [u8; 32],
    /// The legs, in transaction order — load-bearing, because the binding
    /// reference commits to that order.
    pub legs: Vec<DecodedLeg>,
}

/// Pull every payment leg + the memo hash out of a decoded v1 envelope.
///
/// Rejects (with a descriptive [`StellarError::Xdr`]) anything that is not
/// the 1-to-[`MAX_OPERATIONS`]-operation, memo-hash shape this crate itself
/// only ever builds: non-v1/fee-bump envelopes, zero operations, more than
/// [`MAX_OPERATIONS`], a non-`Payment` operation anywhere, an
/// operation-level source override on any leg, a non-Ed25519 destination on
/// any leg, a non-positive amount on any leg, or a non-`Hash` memo.
///
/// **Every leg is checked, and one bad leg rejects the whole envelope** — the
/// only correct reading of an atomic transaction. Errors name the leg index.
pub fn decode_payments(env: &TransactionEnvelope) -> Result<DecodedPayments, StellarError> {
    let TransactionEnvelope::Tx(v1) = env else {
        return Err(StellarError::Xdr(
            "not a v1 (ENVELOPE_TYPE_TX) envelope".into(),
        ));
    };
    let tx = &v1.tx;
    let MuxedAccount::Ed25519(Uint256(source_pk)) = tx.source_account else {
        return Err(StellarError::Xdr(
            "source account is not a plain Ed25519 key".into(),
        ));
    };
    let Memo::Hash(Hash(memo)) = &tx.memo else {
        return Err(StellarError::Xdr("memo is not MEMO_HASH".into()));
    };
    let ops: &[Operation] = tx.operations.as_ref();
    if ops.is_empty() {
        return Err(StellarError::Xdr(
            "transaction carries no operations at all".into(),
        ));
    }
    if ops.len() > MAX_OPERATIONS {
        return Err(StellarError::Xdr(format!(
            "{} operations exceeds the protocol limit of {MAX_OPERATIONS}",
            ops.len()
        )));
    }

    let mut legs = Vec::with_capacity(ops.len());
    for (i, op) in ops.iter().enumerate() {
        if op.source_account.is_some() {
            return Err(StellarError::Xdr(format!(
                "operation {i} carries its own source override"
            )));
        }
        let OperationBody::Payment(pay) = &op.body else {
            return Err(StellarError::Xdr(format!("operation {i} is not PAYMENT")));
        };
        let MuxedAccount::Ed25519(Uint256(dest_pk)) = pay.destination else {
            return Err(StellarError::Xdr(format!(
                "operation {i}: destination is not a plain Ed25519 key"
            )));
        };
        if pay.amount <= 0 {
            return Err(StellarError::Xdr(format!(
                "operation {i}: payment amount {} is not strictly positive",
                pay.amount
            )));
        }
        legs.push(DecodedLeg {
            dest_pk,
            asset: pay.asset.clone(),
            amount: pay.amount,
        });
    }

    Ok(DecodedPayments {
        source_pk,
        seq_num: tx.seq_num.0,
        fee: tx.fee,
        memo: *memo,
        legs,
    })
}

/// Does `asset` equal the given `CreditAlphanum4(code, issuer_pk)`? (Never
/// `Native` XLM or `CreditAlphanum12` — USDC on Stellar is a 4-char code.)
pub fn asset_is(asset: &Asset, code: &str, issuer_pk: [u8; 32]) -> bool {
    let Asset::CreditAlphanum4(a) = asset else {
        return false;
    };
    let Ok(want_code) = asset_code4(code) else {
        return false;
    };
    a.asset_code == want_code && a.issuer.0 == xdr_pubkey(issuer_pk)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(dest: u8, amount: i64) -> PaymentLeg {
        PaymentLeg::new(
            [dest; 32],
            usdc_asset("USDC", [42u8; 32]).expect("a fixed valid asset"),
            amount,
        )
    }

    #[test]
    fn asset_code4_pads_short_codes_and_rejects_bad_ones() {
        assert_eq!(asset_code4("USDC").unwrap().0, *b"USDC");
        assert_eq!(asset_code4("A").unwrap().0, [b'A', 0, 0, 0]);
        assert!(asset_code4("").is_err());
        assert!(asset_code4("TOOLONG").is_err());
        assert!(asset_code4("US-C").is_err());
    }

    #[test]
    fn network_id_is_sha256_of_the_passphrase() {
        let want: [u8; 32] = Sha256::digest(b"Test SDF Network ; September 2015").into();
        assert_eq!(network_id("Test SDF Network ; September 2015"), want);
    }

    #[test]
    fn the_fee_bid_scales_with_the_operation_count() {
        let legs: Vec<PaymentLeg> = (0..7).map(|i| leg(i, 10)).collect();
        let tx = build_payment_transaction([1u8; 32], &legs, 5, 100, [0u8; 32]).unwrap();
        assert_eq!(tx.fee, 700);
        assert_eq!(tx.operations.len(), 7);
        assert!(
            total_fee(u32::MAX, 2).is_err(),
            "overflow refused, not wrapped"
        );
    }

    #[test]
    fn the_builder_accepts_exactly_the_protocol_limit_and_refuses_one_more() {
        let ok: Vec<PaymentLeg> = (0..MAX_OPERATIONS).map(|i| leg(i as u8, 1)).collect();
        let tx = build_payment_transaction([1u8; 32], &ok, 1, 100, [0u8; 32]).unwrap();
        assert_eq!(tx.operations.len(), MAX_OPERATIONS);

        let too_many: Vec<PaymentLeg> = (0..=MAX_OPERATIONS).map(|i| leg(i as u8, 1)).collect();
        let err = build_payment_transaction([1u8; 32], &too_many, 1, 100, [0u8; 32]).unwrap_err();
        assert!(format!("{err}").contains("101 legs exceeds"), "{err}");
    }

    #[test]
    fn the_builder_refuses_an_empty_bundle_and_a_non_positive_leg() {
        let err = build_payment_transaction([1u8; 32], &[], 1, 100, [0u8; 32]).unwrap_err();
        assert!(format!("{err}").contains("at least one leg"), "{err}");
        for bad in [0i64, -1, i64::MIN] {
            let legs = [leg(1, 10), leg(2, bad), leg(3, 10)];
            let err = build_payment_transaction([1u8; 32], &legs, 1, 100, [0u8; 32]).unwrap_err();
            assert!(format!("{err}").contains("leg 1:"), "{err}");
        }
    }

    #[test]
    fn envelope_round_trips_through_the_official_xdr_decoder() {
        // Strong, network-independent correctness check (per README.md's
        // discussion of the same test in patala-stellar): encode with this
        // module's own builder, decode with stellar-xdr's own spec-generated
        // decoder, and every field must survive byte-for-byte.
        let legs = [leg(9, 700), leg(8, 300)];
        let memo = [7u8; 32];
        let tx = build_payment_transaction([1u8; 32], &legs, 42, 100, memo).unwrap();
        let env = envelope(tx, [1u8; 32], [5u8; 64]).unwrap();
        let b64 = envelope_to_xdr_base64(&env).unwrap();
        let back = envelope_from_xdr_base64(&b64).unwrap();
        let decoded = decode_payments(&back).unwrap();
        assert_eq!(decoded.source_pk, [1u8; 32]);
        assert_eq!(decoded.seq_num, 42);
        assert_eq!(decoded.fee, 200);
        assert_eq!(decoded.memo, memo);
        assert_eq!(decoded.legs.len(), 2);
        assert_eq!(decoded.legs[0].dest_pk, [9u8; 32]);
        assert_eq!(decoded.legs[0].amount, 700);
        assert_eq!(decoded.legs[1].dest_pk, [8u8; 32]);
        assert_eq!(decoded.legs[1].amount, 300);
        assert!(asset_is(&decoded.legs[0].asset, "USDC", [42u8; 32]));
    }

    #[test]
    fn decoding_rejects_a_bundle_with_a_bad_leg_and_names_it() {
        let asset = usdc_asset("USDC", [42u8; 32]).unwrap();
        let good = payment_op(&PaymentLeg::new([9u8; 32], asset.clone(), 10));
        let mut overridden = good.clone();
        overridden.source_account = Some(muxed([5u8; 32]));
        let operations = VecM::try_from(vec![good, overridden]).unwrap();
        let tx = Transaction {
            source_account: muxed([1u8; 32]),
            fee: 200,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::Hash(Hash([0u8; 32])),
            operations,
            ext: TransactionExt::V0,
        };
        let env = envelope(tx, [1u8; 32], [0u8; 64]).unwrap();
        let err = decode_payments(&env).unwrap_err();
        assert!(format!("{err}").contains("operation 1"), "{err}");
    }
}
