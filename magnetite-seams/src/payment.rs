//! Seam §3.6 — `PaymentRail` (non-custodial crypto — no balances, no payouts, no custody).
//!
//! Money flows wallet→wallet. There is no balance table and no payout queue. An
//! entitlement is a **signed receipt**; hosting fees ride a payment channel;
//! wagers settle from escrow (in production, gated by `verify_replay`).
//!
//! Default [`MockPaymentRail`] produces **deterministic** signed receipts using
//! a fixed rail keypair and no network — so CI runs fully offline. `protocol_fee_bps`
//! defaults to `0` (governance decides any real fee later).

use serde::{Deserialize, Serialize};

use crate::blobstore::Hash;
use crate::identity::{Identity, PubKey, RawKeypairAuth, Sig};

/// One party's share of a purchase: an absolute payout amount to a wallet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Split {
    /// Destination wallet (an Ed25519 key doubles as a wallet key).
    pub wallet: PubKey,
    /// Amount in the smallest unit of the currency.
    pub amount: u64,
}

/// How a checkout divides among developer, optional operator, and protocol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentSplit {
    /// Game developer's cut.
    pub developer: Split,
    /// Optional hosting operator's cut.
    pub operator: Option<Split>,
    /// Protocol fee in basis points (default 0), taken on top of the subtotal.
    pub protocol_fee_bps: u16,
}

/// A concrete wallet→amount transfer captured in a [`Receipt`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayOut {
    /// Destination wallet.
    pub wallet: PubKey,
    /// Amount paid.
    pub amount: u64,
}

/// Where a receipt is anchored on a real chain, and to what.
///
/// Present only for chain rails (see the separate `magnetite-solana-rail`
/// crate). For the offline mock this is `None` and the receipt is worth
/// exactly what the rail signature says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainBinding {
    /// Chain name, e.g. `"solana"`.
    pub chain: String,
    /// The on-chain transaction signature (base58 on Solana). Informational —
    /// a best-effort peek at `rail_proof`, never used for verification.
    pub tx_signature: String,
    /// The item this payment is redeemable for — and ONLY this item.
    pub item: String,
    /// The token mint that was transferred (base58 on Solana). Informational,
    /// same caveat as `tx_signature`.
    pub mint: String,
    /// Hex `blake3("magnetite-pay-v1" || buyer || item)`; magnetite's OWN
    /// local item<->receipt consistency hash (distinct from, and checked in
    /// addition to, whatever domain-separated binding the rail itself uses
    /// on chain).
    pub reference: String,
    /// The chain rail's own opaque proof blob (e.g.
    /// `patala_core::Receipt::proof` for the Solana rail) — carried through
    /// unmodified from charge time so verification can hand it back to the
    /// rail exactly as issued. Opaque to everything except the rail that
    /// produced it. Absent (empty) for rows written before this field existed
    /// or for rails that do not use it.
    #[serde(default)]
    pub rail_proof: Vec<u8>,
}

/// A payment operation that a given rail cannot perform.
#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    /// The rail has no implementation of this operation and will not fake one.
    #[error("{0} is not supported on this payment rail")]
    Unsupported(&'static str),
    /// The rail tried and failed (RPC down, insufficient funds, ...).
    #[error("payment rail error: {0}")]
    Rail(String),
}

/// Signed proof of an atomic wallet→wallet purchase. This IS the entitlement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    /// Who paid.
    pub buyer: PubKey,
    /// Where the money went (developer, [operator], [protocol fee]).
    pub payouts: Vec<PayOut>,
    /// Protocol fee component (in smallest unit).
    pub protocol_fee: u64,
    /// Total the buyer paid (`sum(payouts)`).
    pub total: u64,
    /// Deterministic nonce derived from the split (no wall-clock → reproducible).
    pub nonce: [u8; 32],
    /// The rail key that signed this receipt.
    pub rail_pubkey: PubKey,
    /// Rail signature over `signing_bytes`.
    pub sig: Sig,
    /// On-chain anchor, for rails that have one.
    #[serde(default)]
    pub binding: Option<ChainBinding>,
}

impl Receipt {
    /// Public (not `pub(crate)`): the out-of-tree `magnetite-solana-rail`
    /// crate needs this to compute/verify the rail signature over a receipt
    /// it builds — see that crate's `Cargo.toml` for why the real Solana rail
    /// is a separate crate rather than an in-tree module.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.buyer.0);
        b.extend_from_slice(&(self.payouts.len() as u32).to_le_bytes());
        for p in &self.payouts {
            b.extend_from_slice(&p.wallet.0);
            b.extend_from_slice(&p.amount.to_le_bytes());
        }
        b.extend_from_slice(&self.protocol_fee.to_le_bytes());
        b.extend_from_slice(&self.total.to_le_bytes());
        b.extend_from_slice(&self.nonce);
        if let Some(c) = &self.binding {
            b.extend_from_slice(c.chain.as_bytes());
            b.extend_from_slice(c.tx_signature.as_bytes());
            b.extend_from_slice(c.item.as_bytes());
            b.extend_from_slice(c.mint.as_bytes());
            b.extend_from_slice(c.reference.as_bytes());
            b.extend_from_slice(&c.rail_proof);
        }
        b
    }
}

/// A micro-payment channel handle (hosting fees, per-seat/per-hour).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    /// Deterministic channel id.
    pub id: [u8; 32],
    /// The counterparty (operator).
    pub peer: PubKey,
    /// Rail that opened it.
    pub rail_pubkey: PubKey,
}

/// Terms of an optional wager / tournament.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WagerTerms {
    /// Participating player keys.
    pub players: Vec<PubKey>,
    /// Stake per player, in smallest unit.
    pub stake: u64,
    /// Currency label, e.g. `"USDC"`.
    pub currency: String,
    /// The game being wagered on (settled by replay verification in production).
    pub game: Hash,
}

/// An escrow holding staked funds until settlement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Escrow {
    /// Deterministic escrow id.
    pub id: [u8; 32],
    /// The wager terms.
    pub terms: WagerTerms,
    /// Total locked (`stake * players`).
    pub locked: u64,
    /// Rail that opened it.
    pub rail_pubkey: PubKey,
}

/// Non-custodial crypto payment rail (§3.6).
#[async_trait::async_trait]
pub trait PaymentRail {
    /// Atomic wallet→wallet purchase; returns a signed entitlement receipt.
    async fn checkout(&self, buyer: &PubKey, split: PaymentSplit) -> Receipt;
    /// Checkout bound to a specific item, so the resulting receipt is
    /// redeemable for that item and nothing else.
    ///
    /// The default implementation ignores `item` — correct for rails whose
    /// receipts are bound by the caller's database (the mock). Chain rails
    /// override it and put the binding on chain.
    async fn checkout_for_item(
        &self,
        buyer: &PubKey,
        _item: &str,
        split: PaymentSplit,
    ) -> Result<Receipt, PaymentError> {
        Ok(self.checkout(buyer, split).await)
    }
    /// Open a micro-payment channel to a peer (hosting fees).
    ///
    /// Rails without an on-chain channel program MUST return
    /// [`PaymentError::Unsupported`] rather than a stub that appears to work.
    async fn open_channel(&self, peer: &PubKey) -> Result<Channel, PaymentError>;
    /// Lock a wager into escrow. Same rule as [`Self::open_channel`].
    async fn escrow(&self, terms: WagerTerms) -> Result<Escrow, PaymentError>;
    /// Verify a receipt. **Must fail closed**: any doubt returns `false`.
    fn verify_receipt(&self, r: &Receipt) -> bool;
    /// Verify a receipt AND that it is bound to `item`.
    ///
    /// Defaults to [`Self::verify_receipt`] for rails whose item binding lives
    /// in the caller's database.
    fn verify_receipt_for_item(&self, r: &Receipt, _item: &str) -> bool {
        self.verify_receipt(r)
    }
}

/// Deterministic, offline mock rail. Signs receipts with a fixed rail keypair.
pub struct MockPaymentRail {
    rail: RawKeypairAuth,
    /// Basis-points protocol fee. Default 0.
    pub protocol_fee_bps: u16,
}

impl Default for MockPaymentRail {
    fn default() -> Self {
        // Fixed seed => deterministic rail pubkey => reproducible receipts.
        Self {
            rail: RawKeypairAuth::from_seed([7u8; 32]),
            protocol_fee_bps: 0,
        }
    }
}

impl MockPaymentRail {
    /// A rail with the default fixed key and `protocol_fee_bps = 0`.
    pub fn new() -> Self {
        Self::default()
    }
    /// A rail with an explicit protocol fee (still deterministic).
    pub fn with_fee_bps(bps: u16) -> Self {
        Self {
            protocol_fee_bps: bps,
            ..Self::default()
        }
    }
    /// The rail's public (verifying) key.
    pub fn rail_pubkey(&self) -> PubKey {
        self.rail.node_pubkey()
    }

    fn compute(&self, buyer: &PubKey, split: &PaymentSplit) -> (Vec<PayOut>, u64, u64, [u8; 32]) {
        let dev = split.developer.amount;
        let op = split.operator.as_ref().map(|s| s.amount).unwrap_or(0);
        let subtotal = dev.saturating_add(op);
        // Fee taken on top of the subtotal.
        let fee = ((subtotal as u128 * split.protocol_fee_bps as u128) / 10_000) as u64;
        let total = subtotal.saturating_add(fee);

        let mut payouts = vec![PayOut {
            wallet: split.developer.wallet,
            amount: dev,
        }];
        if let Some(o) = &split.operator {
            payouts.push(PayOut {
                wallet: o.wallet,
                amount: o.amount,
            });
        }
        if fee > 0 {
            payouts.push(PayOut {
                wallet: self.rail_pubkey(),
                amount: fee,
            });
        }

        // Deterministic nonce = BLAKE3 over the canonical split inputs.
        let mut seed = Vec::new();
        seed.extend_from_slice(&buyer.0);
        seed.extend_from_slice(&split.developer.wallet.0);
        seed.extend_from_slice(&dev.to_le_bytes());
        if let Some(o) = &split.operator {
            seed.extend_from_slice(&o.wallet.0);
            seed.extend_from_slice(&o.amount.to_le_bytes());
        }
        seed.extend_from_slice(&split.protocol_fee_bps.to_le_bytes());
        let nonce = *blake3::hash(&seed).as_bytes();

        (payouts, fee, total, nonce)
    }
}

#[async_trait::async_trait]
impl PaymentRail for MockPaymentRail {
    async fn checkout(&self, buyer: &PubKey, split: PaymentSplit) -> Receipt {
        let (payouts, fee, total, nonce) = self.compute(buyer, &split);
        let mut r = Receipt {
            buyer: *buyer,
            payouts,
            protocol_fee: fee,
            total,
            nonce,
            rail_pubkey: self.rail_pubkey(),
            sig: Sig([0u8; 64]),
            binding: None,
        };
        r.sig = self.rail.sign(&r.signing_bytes());
        r
    }

    async fn open_channel(&self, peer: &PubKey) -> Result<Channel, PaymentError> {
        let mut seed = Vec::new();
        seed.extend_from_slice(b"channel");
        seed.extend_from_slice(&self.rail_pubkey().0);
        seed.extend_from_slice(&peer.0);
        Ok(Channel {
            id: *blake3::hash(&seed).as_bytes(),
            peer: *peer,
            rail_pubkey: self.rail_pubkey(),
        })
    }

    async fn escrow(&self, terms: WagerTerms) -> Result<Escrow, PaymentError> {
        let locked = terms.stake.saturating_mul(terms.players.len() as u64);
        let mut seed = Vec::new();
        seed.extend_from_slice(b"escrow");
        seed.extend_from_slice(terms.game.to_hex().as_bytes());
        for p in &terms.players {
            seed.extend_from_slice(&p.0);
        }
        seed.extend_from_slice(&terms.stake.to_le_bytes());
        Ok(Escrow {
            id: *blake3::hash(&seed).as_bytes(),
            terms,
            locked,
            rail_pubkey: self.rail_pubkey(),
        })
    }

    fn verify_receipt(&self, r: &Receipt) -> bool {
        // 1. Arithmetic must be internally consistent.
        let sum: u64 = r.payouts.iter().map(|p| p.amount).sum();
        if sum != r.total {
            return false;
        }
        // 2. Signature must be by the claimed rail key.
        <RawKeypairAuth as Identity>::verify(&r.rail_pubkey, &r.signing_bytes(), &r.sig)
    }
}

/// The **pure** half of the paid-access gate: does this receipt admit `buyer`
/// to something costing `min_units`?
///
/// Callers that own a database (the backend's comms/session gates) still have to
/// bind the receipt to a specific item, check it is not voided, and refuse a
/// merely *derived* account key — those facts live in their storage, not in the
/// receipt. Everything that can be decided from the receipt alone is decided
/// here, once, so no caller re-implements it and drifts:
///
/// 1. the receipt is bound to this buyer,
/// 2. it covers at least `min_units`,
/// 3. its arithmetic and rail signature verify.
///
/// **Fails closed.** `min_units == 0` means "free" and short-circuits to `true`
/// before any receipt is consulted.
pub fn receipt_admits<R: PaymentRail + ?Sized>(
    rail: &R,
    receipt: &Receipt,
    buyer: &PubKey,
    min_units: u64,
) -> bool {
    if min_units == 0 {
        return true;
    }
    receipt.buyer == *buyer && receipt.total >= min_units && rail.verify_receipt(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(dev_amt: u64, op: Option<u64>, bps: u16) -> PaymentSplit {
        PaymentSplit {
            developer: Split {
                wallet: PubKey([0xD0; 32]),
                amount: dev_amt,
            },
            operator: op.map(|amount| Split {
                wallet: PubKey([0x0B; 32]),
                amount,
            }),
            protocol_fee_bps: bps,
        }
    }

    #[tokio::test]
    async fn checkout_split_math_no_fee() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB0; 32]);
        let r = rail.checkout(&buyer, split(1000, Some(250), 0)).await;

        assert_eq!(r.protocol_fee, 0);
        assert_eq!(r.total, 1250);
        assert_eq!(r.payouts.len(), 2); // dev + operator, no fee payout
        assert_eq!(r.payouts[0].amount, 1000);
        assert_eq!(r.payouts[1].amount, 250);
        let sum: u64 = r.payouts.iter().map(|p| p.amount).sum();
        assert_eq!(sum, r.total);
        assert!(rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn checkout_split_math_with_fee() {
        let rail = MockPaymentRail::with_fee_bps(500); // 5%
        let buyer = PubKey([0xB1; 32]);
        let r = rail.checkout(&buyer, split(1000, None, 500)).await;

        assert_eq!(r.protocol_fee, 50); // 5% of 1000
        assert_eq!(r.total, 1050);
        assert_eq!(r.payouts.len(), 2); // dev + fee payout to rail
        assert_eq!(r.payouts[1].wallet, rail.rail_pubkey());
        assert_eq!(r.payouts[1].amount, 50);
        let sum: u64 = r.payouts.iter().map(|p| p.amount).sum();
        assert_eq!(sum, r.total);
        assert!(rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn receipts_are_deterministic() {
        let buyer = PubKey([0xB2; 32]);
        let a = MockPaymentRail::new()
            .checkout(&buyer, split(500, Some(100), 0))
            .await;
        let b = MockPaymentRail::new()
            .checkout(&buyer, split(500, Some(100), 0))
            .await;
        assert_eq!(a.nonce, b.nonce, "deterministic nonce");
        assert_eq!(a.sig.0, b.sig.0, "deterministic signature");
        assert_eq!(a.rail_pubkey, b.rail_pubkey, "fixed rail key");
    }

    #[tokio::test]
    async fn tampered_receipt_fails_verify() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB3; 32]);
        let mut r = rail.checkout(&buyer, split(1000, None, 0)).await;
        assert!(rail.verify_receipt(&r));

        // Inflate a payout -> both arithmetic and signature break.
        r.payouts[0].amount = 9999;
        assert!(!rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn receipt_admits_is_fail_closed() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB4; 32]);
        let stranger = PubKey([0xB5; 32]);
        let r = rail.checkout(&buyer, split(500, None, 0)).await;

        assert!(receipt_admits(&rail, &r, &buyer, 500), "exact price admits");
        assert!(receipt_admits(&rail, &r, &buyer, 100), "overpay admits");
        assert!(receipt_admits(&rail, &r, &stranger, 0), "free needs nothing");

        assert!(
            !receipt_admits(&rail, &r, &buyer, 501),
            "underpaying must never admit"
        );
        assert!(
            !receipt_admits(&rail, &r, &stranger, 500),
            "another buyer's receipt must never admit"
        );

        let mut forged = r.clone();
        forged.total = 100_000;
        forged.payouts[0].amount = 100_000;
        assert!(
            !receipt_admits(&rail, &forged, &buyer, 100_000),
            "a re-signed-by-nobody receipt must never admit"
        );
    }

    #[tokio::test]
    async fn channel_and_escrow_are_deterministic_offline() {
        let rail = MockPaymentRail::new();
        let peer = PubKey([0x0B; 32]);
        let c1 = rail.open_channel(&peer).await.unwrap();
        let c2 = rail.open_channel(&peer).await.unwrap();
        assert_eq!(c1.id, c2.id);
        assert_eq!(c1.rail_pubkey, rail.rail_pubkey());

        let terms = WagerTerms {
            players: vec![PubKey([1; 32]), PubKey([2; 32])],
            stake: 100,
            currency: "USDC".into(),
            game: Hash::of(b"chess"),
        };
        let e = rail.escrow(terms).await.unwrap();
        assert_eq!(e.locked, 200);
    }
}
