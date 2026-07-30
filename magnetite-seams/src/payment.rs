//! Seam §3.6 — `PaymentRail` (non-custodial crypto — no balances, no payouts, no custody).
//!
//! Money flows wallet→wallet. There is no balance table and no payout queue. An
//! entitlement is a **signed receipt**; hosting fees ride a payment channel;
//! wagers settle from escrow (in production, gated by `verify_replay`).
//!
//! Default [`MockPaymentRail`] produces **deterministic** signed receipts using
//! a fixed rail keypair and no network — so CI runs fully offline.
//!
//! # A split is a list of legs that sums exactly to the total
//!
//! [`PaymentSplit`] is a `Vec<Leg>`, not a fixed `{developer, operator, fee}`
//! shape (`ALIGNMENT.md` §4). Co-developers, publishers, asset-pack authors,
//! operator revenue shares, charity splits and the voluntary contribution to
//! magnetite's own maintainers ([`Role::Stewards`]) are all the same code path
//! and the same arithmetic:
//!
//! * the buyer's total is `sum(legs)` — **derived, never asserted separately**,
//!   so there is no field a caller can set to disagree with the legs;
//! * every amount is an integer count of the currency's smallest unit
//!   (micro-USDC for the chain rails: 6 decimals). **No floats anywhere.**
//!   Overflow is `checked_add`, never saturating — a split that cannot be
//!   summed is refused, not silently clamped;
//! * a rail MUST refuse any plan whose paid legs do not sum exactly to the
//!   receipt total; and
//! * the receipt's binding seed commits to **every leg** (see
//!   [`Receipt::nonce`] and [`split_digest`]), so a receipt cannot be
//!   re-pointed at a different distribution of the same total.
//!
//! # There is no protocol fee, and no rate lives here
//!
//! There is no enforceable mandatory fee in a system with no central server.
//! [`Role::Stewards`] is a **voluntary** leg, and this seam only ever sees an
//! already-decided absolute `amount`: no basis-points rate, no percentage, and
//! nothing read from the node's environment. Whoever's money it is decides the
//! rate (developer in the signed manifest, player at checkout) and hands this
//! seam integers. See `site/docs/payments.md`.
//!
//! **This layer does no proportional allocation.** A signed manifest carries
//! *proportions* (basis points summing to `10_000`), because it is signed at
//! publish time when the total is not yet known — a pay-what-you-want or tipped
//! purchase only has a total at checkout. Turning those proportions into
//! sum-exact absolute amounts is one job in one place (the manifest side's
//! `SplitPlan::resolve`, largest-remainder allocation). Everything here takes
//! amounts as given and only ever checks that they sum exactly. Do not add a
//! second bps → amount path: two implementations of that arithmetic differ at
//! the first total that does not divide evenly.
//!
//! # Dust floor
//!
//! A leg below the paying rail's dust floor is **skipped**, never a hard
//! failure: a rounding-dust voluntary contribution must not break a purchase.
//! The buyer is not charged for a skipped leg — the total shrinks by exactly
//! that amount, so sum-exactness still holds over what was actually paid.
//!
//! The floor is **the rail's**, not this seam's, and that is deliberate. A
//! value-based floor requires knowing the currency's scale, and this seam does
//! not: its callers denominate in different units today (the backend in USD
//! cents, `magnetite_solana_rail` in micro-USDC). A single constant here would
//! be a floor of 0.001 USDC on one rail and $10 on the other — and on the
//! second it would silently skip the *developer's* leg on any ordinary
//! purchase, which is a fail-open dressed as a safety feature. So the
//! mechanism lives here ([`PaymentSplit::partition_at`]) and the number lives
//! with whoever knows the unit ([`MockPaymentRail::with_dust_floor`],
//! `magnetite_solana_rail::DUST_FLOOR_MICRO_USDC`).
//!
//! A plan with **no** payable leg is a different matter and is refused
//! outright: a purchase that moves no money is not a purchase.

use serde::{Deserialize, Serialize};

use crate::blobstore::Hash;
use crate::identity::{Identity, PubKey, RawKeypairAuth, Sig};

/// The floor a currency-agnostic rail uses: `1` unit, i.e. only a
/// zero-amount leg is dust.
///
/// This is the *degenerate* dust case and the only one that can be judged
/// without knowing what a unit is worth — a zero transfer moves nothing, so
/// emitting an instruction for it is pure cost. Any rail that does know its
/// currency's scale must state a real, value-based floor instead; see
/// `magnetite_solana_rail::DUST_FLOOR_MICRO_USDC` and the module docs above
/// for why this seam refuses to guess one.
pub const ZERO_ONLY_DUST_FLOOR: u64 = 1;

/// Who a [`Leg`] pays. Carried in the receipt binding, so the role a leg
/// claimed at checkout is part of what the receipt is tamper-evident over.
///
/// [`Role::Other`] exists so a publisher, co-developer, asset-pack author or
/// charity is not a special case in this seam — it is a label, and it has
/// exactly the same code path as every other leg.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The game's developer.
    Developer,
    /// The hosting operator (a revenue share, or a per-session entry charge).
    Operator,
    /// **Voluntary** contribution to magnetite's maintainers. The destination
    /// is NOT operator-configurable: chain rails take it from a compiled-in,
    /// signed-release constant, never from node environment (see
    /// `magnetite_solana_rail::stewards`).
    Stewards,
    /// Anyone else, labelled: publisher, co-developer, asset-pack author,
    /// charity, translator…
    Other(String),
}

impl Role {
    /// Stable, canonical byte tag for signing and binding seeds.
    ///
    /// Length-prefixed by the caller (see [`split_digest`]) so
    /// `Other("a") + Other("bc")` can never hash the same as
    /// `Other("ab") + Other("c")`.
    pub fn tag(&self) -> &str {
        match self {
            Role::Developer => "developer",
            Role::Operator => "operator",
            Role::Stewards => "stewards",
            Role::Other(s) => s,
        }
    }

    /// Whether this is the voluntary maintainers' leg.
    pub fn is_stewards(&self) -> bool {
        matches!(self, Role::Stewards)
    }
}

/// One destination in a purchase: an absolute amount to a wallet, and who it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leg {
    /// Destination wallet (an Ed25519 key doubles as a wallet key).
    pub wallet: PubKey,
    /// Amount in the smallest unit of the currency. Absolute, never a rate.
    pub amount: u64,
    /// Who this leg pays.
    pub role: Role,
}

impl Leg {
    /// A leg with an explicit role.
    pub fn new(wallet: PubKey, amount: u64, role: Role) -> Self {
        Self {
            wallet,
            amount,
            role,
        }
    }
    /// Convenience: a developer leg.
    pub fn developer(wallet: PubKey, amount: u64) -> Self {
        Self::new(wallet, amount, Role::Developer)
    }
    /// Convenience: an operator leg.
    pub fn operator(wallet: PubKey, amount: u64) -> Self {
        Self::new(wallet, amount, Role::Operator)
    }
    /// Convenience: a voluntary stewards leg.
    pub fn stewards(wallet: PubKey, amount: u64) -> Self {
        Self::new(wallet, amount, Role::Stewards)
    }
    /// Is this leg below `dust_floor` (and therefore skipped, not paid)?
    pub fn is_dust_below(&self, dust_floor: u64) -> bool {
        self.amount < dust_floor
    }
}

/// How a checkout divides: an ordered list of legs that sums to the total.
///
/// There is no separate total field — see the module docs. Construct with
/// [`PaymentSplit::new`], read the money with [`PaymentSplit::checked_total`]
/// (which refuses to overflow) and split payable-from-dust with
/// [`PaymentSplit::partition`].
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PaymentSplit {
    /// The legs, in the order a rail should emit them.
    pub legs: Vec<Leg>,
}

impl PaymentSplit {
    /// A split from an explicit leg list.
    pub fn new(legs: Vec<Leg>) -> Self {
        Self { legs }
    }

    /// A single-developer split — the overwhelmingly common case.
    pub fn to_developer(wallet: PubKey, amount: u64) -> Self {
        Self::new(vec![Leg::developer(wallet, amount)])
    }

    /// Sum of every leg, or `None` on overflow.
    ///
    /// Deliberately not saturating: `u64::MAX` would be a *lie* about what the
    /// buyer owes, and a rail that cannot state the total exactly must refuse.
    pub fn checked_total(&self) -> Option<u64> {
        self.legs
            .iter()
            .try_fold(0u64, |a, l| a.checked_add(l.amount))
    }

    /// Do the legs sum **exactly** to `total`? (`false` on overflow.)
    pub fn sums_to(&self, total: u64) -> bool {
        self.checked_total() == Some(total)
    }

    /// Split into `(payable, dust)` at `dust_floor`, preserving order in both.
    ///
    /// A dust leg (amount `< dust_floor`, which always includes a zero-amount
    /// leg) is skipped rather than failing the whole purchase. Nothing here
    /// mutates or redistributes amounts: a skipped leg's money is simply never
    /// charged, so the buyer pays exactly `sum(payable)`.
    pub fn partition_at(&self, dust_floor: u64) -> (Vec<Leg>, Vec<Leg>) {
        let (dust, payable): (Vec<Leg>, Vec<Leg>) = self
            .legs
            .iter()
            .cloned()
            .partition(|l| l.is_dust_below(dust_floor));
        (payable, dust)
    }

    /// Total of the voluntary stewards legs only (`0` when there are none).
    /// Saturating here is safe and intentional: this is a reporting figure,
    /// never what a buyer is charged.
    pub fn stewards_total(&self) -> u64 {
        self.legs
            .iter()
            .filter(|l| l.role.is_stewards())
            .fold(0u64, |a, l| a.saturating_add(l.amount))
    }
}

/// Domain-separated digest over an ordered leg list:
/// `blake3("magnetite-split-v2" ‖ n ‖ (wallet ‖ amount ‖ len(tag) ‖ tag)*)`.
///
/// This is what makes a receipt tamper-evident over its **distribution** and
/// not merely its total. The previous shape folded `protocol_fee_bps` into the
/// receipt seed, which committed to a *rate* and to nothing about where the
/// money went; two different distributions of the same total produced the same
/// seed. They now cannot: every wallet, every amount, every role label and the
/// order are inside the hash, and each variable-length field is
/// length-prefixed so no two distinct leg lists can encode to the same bytes.
pub fn split_digest(legs: &[Leg]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"magnetite-split-v2");
    h.update(&(legs.len() as u64).to_le_bytes());
    for l in legs {
        h.update(&l.wallet.0);
        h.update(&l.amount.to_le_bytes());
        let tag = l.role.tag().as_bytes();
        h.update(&(tag.len() as u64).to_le_bytes());
        h.update(tag);
    }
    *h.finalize().as_bytes()
}

/// A concrete wallet→amount transfer captured in a [`Receipt`].
///
/// Deliberately still `{wallet, amount}` and not a [`Leg`]: this is the shape
/// persisted in `payment_receipts.payouts` rows that already exist, and a
/// receipt's job is to record what moved, which is a wallet and an amount. The
/// roles live in the binding digest, which is what verification checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayOut {
    /// Destination wallet.
    pub wallet: PubKey,
    /// Amount paid.
    pub amount: u64,
}

impl From<&Leg> for PayOut {
    fn from(l: &Leg) -> Self {
        PayOut {
            wallet: l.wallet,
            amount: l.amount,
        }
    }
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
    /// Hex `blake3("magnetite-pay-v2" || buyer || item || split_digest(legs))`;
    /// magnetite's OWN local item+distribution↔receipt consistency hash
    /// (distinct from, and checked in addition to, whatever domain-separated
    /// binding the rail itself uses on chain). Because the legs are inside it,
    /// a receipt is redeemable for one item under one distribution and nothing
    /// else.
    pub reference: String,
    /// The chain rail's own opaque proof blob, carried through unmodified from
    /// charge time so verification can hand it back to the rail exactly as
    /// issued. Opaque to everything except the rail that produced it. Absent
    /// (empty) for rows written before this field existed or for rails that do
    /// not use it.
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
    /// Where the money actually went — one entry per **paid** leg, in order.
    /// Dust legs are absent (they were skipped, not paid).
    pub payouts: Vec<PayOut>,
    /// The voluntary [`Role::Stewards`] component of `payouts`, in smallest
    /// units. `0` unless a stewards leg was both requested and paid.
    ///
    /// Kept under its historical column name (`payment_receipts.protocol_fee`)
    /// so existing rows keep their meaning; it never was a protocol fee that
    /// anyone could enforce, and now it is not called one anywhere it is not
    /// forced to be.
    #[serde(alias = "protocol_fee")]
    pub stewards_amount: u64,
    /// Total the buyer paid (`sum(payouts)`).
    pub total: u64,
    /// Binding seed. Commits to the buyer AND to every leg of the distribution
    /// (see [`split_digest`]), so a receipt cannot be re-pointed at a
    /// different split of the same total.
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
        b.extend_from_slice(&self.stewards_amount.to_le_bytes());
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
///
/// It holds **no** rate and **no** stewards address: a split arrives with every
/// amount already decided, and this rail's whole job is to pay the payable legs
/// and sign what it paid.
pub struct MockPaymentRail {
    rail: RawKeypairAuth,
    /// Legs below this are skipped. Defaults to [`ZERO_ONLY_DUST_FLOOR`],
    /// because this rail is currency-agnostic — see the module docs.
    dust_floor: u64,
}

impl Default for MockPaymentRail {
    fn default() -> Self {
        // Fixed seed => deterministic rail pubkey => reproducible receipts.
        Self {
            rail: RawKeypairAuth::from_seed([7u8; 32]),
            dust_floor: ZERO_ONLY_DUST_FLOOR,
        }
    }
}

impl MockPaymentRail {
    /// A rail with the default fixed key and the zero-only dust floor.
    pub fn new() -> Self {
        Self::default()
    }

    /// A rail with an explicit dust floor, for a caller that knows what its
    /// units are worth (still deterministic).
    pub fn with_dust_floor(dust_floor: u64) -> Self {
        Self {
            dust_floor,
            ..Self::default()
        }
    }

    /// The floor below which this rail skips a leg.
    pub fn dust_floor(&self) -> u64 {
        self.dust_floor
    }

    /// The rail's public (verifying) key.
    pub fn rail_pubkey(&self) -> PubKey {
        self.rail.node_pubkey()
    }

    /// Pure, integer-exact plan: `(payouts, stewards_amount, total, nonce)`.
    ///
    /// Dust legs are dropped and are NOT charged; `total` is the sum of what
    /// remains, and the sum is re-derived and checked rather than assumed.
    /// `None` means refuse — the only cause is a leg list that overflows `u64`,
    /// which cannot be charged honestly.
    fn compute(
        &self,
        buyer: &PubKey,
        split: &PaymentSplit,
    ) -> Option<(Vec<PayOut>, u64, u64, [u8; 32])> {
        // The REQUESTED total must be statable, even if part of it is dust: a
        // caller whose own arithmetic overflowed is not one to charge on the
        // strength of whichever legs happened to survive the floor.
        split.checked_total()?;

        let (payable, _dust) = split.partition_at(self.dust_floor);
        let paid = PaymentSplit::new(payable);
        let total = paid.checked_total()?;

        let payouts: Vec<PayOut> = paid.legs.iter().map(PayOut::from).collect();
        // Re-derive independently: a plan whose parts do not sum exactly to the
        // total it claims is refused, never rounded into agreement.
        let sum = payouts
            .iter()
            .try_fold(0u64, |a, p| a.checked_add(p.amount))?;
        if sum != total {
            return None;
        }

        // Binding seed commits to the buyer AND the full requested split —
        // including the dust legs, so "what was asked for" is recoverable and
        // a receipt for one distribution is not a receipt for another.
        let mut seed = Vec::new();
        seed.extend_from_slice(b"magnetite-mock-receipt-v2");
        seed.extend_from_slice(&buyer.0);
        seed.extend_from_slice(&split_digest(&split.legs));
        seed.extend_from_slice(&split_digest(&paid.legs));
        let nonce = *blake3::hash(&seed).as_bytes();

        Some((payouts, paid.stewards_total(), total, nonce))
    }
}

#[async_trait::async_trait]
impl PaymentRail for MockPaymentRail {
    /// A split this rail cannot sum honestly yields a receipt with no payouts,
    /// a zero total and a **blank** signature — i.e. one that
    /// [`Self::verify_receipt`] rejects. The seam's `checkout` is infallible by
    /// signature, so "refuse" has to be expressed as an unverifiable receipt;
    /// [`PaymentRail::checkout_for_item`] is the fallible form.
    async fn checkout(&self, buyer: &PubKey, split: PaymentSplit) -> Receipt {
        let Some((payouts, stewards_amount, total, nonce)) = self.compute(buyer, &split) else {
            return Receipt {
                buyer: *buyer,
                payouts: Vec::new(),
                stewards_amount: 0,
                total: u64::MAX, // cannot equal sum(payouts) == 0 ⇒ never verifies
                nonce: [0u8; 32],
                rail_pubkey: self.rail_pubkey(),
                sig: Sig([0u8; 64]),
                binding: None,
            };
        };
        let mut r = Receipt {
            buyer: *buyer,
            payouts,
            stewards_amount,
            total,
            nonce,
            rail_pubkey: self.rail_pubkey(),
            sig: Sig([0u8; 64]),
            binding: None,
        };
        r.sig = self.rail.sign(&r.signing_bytes());
        r
    }

    async fn checkout_for_item(
        &self,
        buyer: &PubKey,
        _item: &str,
        split: PaymentSplit,
    ) -> Result<Receipt, PaymentError> {
        if split.checked_total().is_none() {
            return Err(PaymentError::Rail(
                "split legs overflow u64 — refusing to state a total this rail cannot pay".into(),
            ));
        }
        let (payable, _dust) = split.partition_at(self.dust_floor);
        if payable.is_empty() {
            return Err(PaymentError::Rail(format!(
                "split has no payable leg (all {} legs are below the {} unit dust floor)",
                split.legs.len(),
                self.dust_floor
            )));
        }
        Ok(self.checkout(buyer, split).await)
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
        // 1. Arithmetic must be internally consistent — sum EXACTLY, no
        //    overflow. Any doubt is a refusal.
        let Some(sum) = r
            .payouts
            .iter()
            .try_fold(0u64, |a, p| a.checked_add(p.amount))
        else {
            return false;
        };
        if sum != r.total {
            return false;
        }
        // 2. The stewards component cannot exceed what was paid.
        if r.stewards_amount > r.total {
            return false;
        }
        // 3. Signature must be by the claimed rail key. Because
        //    `signing_bytes` covers the nonce, and the nonce commits to every
        //    leg, this also pins the distribution.
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

    fn dev(amount: u64) -> Leg {
        Leg::developer(PubKey([0xD0; 32]), amount)
    }
    fn op(amount: u64) -> Leg {
        Leg::operator(PubKey([0x0B; 32]), amount)
    }
    fn stew(amount: u64) -> Leg {
        Leg::stewards(PubKey([0x57; 32]), amount)
    }

    #[tokio::test]
    async fn legs_sum_exactly_to_the_total() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB0; 32]);
        let r = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(1_000_000), op(250_000)]))
            .await;

        assert_eq!(r.stewards_amount, 0);
        assert_eq!(r.total, 1_250_000);
        assert_eq!(r.payouts.len(), 2);
        assert_eq!(r.payouts[0].amount, 1_000_000);
        assert_eq!(r.payouts[1].amount, 250_000);
        let sum: u64 = r.payouts.iter().map(|p| p.amount).sum();
        assert_eq!(sum, r.total, "sum-exact, always");
        assert!(rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn a_stewards_leg_is_an_absolute_amount_and_is_reported_as_such() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB1; 32]);
        // The developer decided 2.5% of 1_000_000 = 25_000 BEFORE this seam saw
        // it. No rate, no bps, no env var reaches the rail.
        let r = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(975_000), stew(25_000)]))
            .await;
        assert_eq!(r.stewards_amount, 25_000);
        assert_eq!(r.total, 1_000_000);
        assert_eq!(r.payouts.len(), 2);
        assert_eq!(r.payouts[1].wallet, PubKey([0x57; 32]));
        assert!(rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn a_receipt_whose_payouts_do_not_sum_is_refused() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB2; 32]);
        let mut r = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(600), op(400)]))
            .await;
        assert!(rail.verify_receipt(&r));

        // Drop a leg but keep the total: the legs no longer sum to it.
        r.payouts.pop();
        assert!(
            !rail.verify_receipt(&r),
            "a receipt whose legs do not sum exactly to its total must be refused"
        );
    }

    #[tokio::test]
    async fn a_split_that_overflows_u64_is_refused_not_clamped() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB3; 32]);
        let s = PaymentSplit::new(vec![dev(u64::MAX), op(1)]);
        assert!(s.checked_total().is_none(), "and it must not saturate");

        let err = rail.checkout_for_item(&buyer, "game:x", s.clone()).await;
        assert!(matches!(err, Err(PaymentError::Rail(_))));
        // Even the infallible form must not produce something that verifies.
        let r = rail.checkout(&buyer, s).await;
        assert!(!rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn zero_leg_and_single_leg_splits() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB4; 32]);

        // Zero legs: nothing to pay, so the item-bound form refuses.
        let none = rail
            .checkout_for_item(&buyer, "game:x", PaymentSplit::new(vec![]))
            .await;
        assert!(matches!(none, Err(PaymentError::Rail(_))));

        // One leg: the common case.
        let one = rail
            .checkout(
                &buyer,
                PaymentSplit::to_developer(PubKey([0xD0; 32]), 1_999),
            )
            .await;
        assert_eq!(one.total, 1_999);
        assert_eq!(one.payouts.len(), 1);
        assert!(rail.verify_receipt(&one));
    }

    #[tokio::test]
    async fn many_legs_still_sum_exactly() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB5; 32]);
        let legs: Vec<Leg> = (0u8..24)
            .map(|i| {
                Leg::new(
                    PubKey([i; 32]),
                    1_000 + i as u64,
                    Role::Other(format!("coauthor-{i}")),
                )
            })
            .collect();
        let expected: u64 = legs.iter().map(|l| l.amount).sum();
        let r = rail.checkout(&buyer, PaymentSplit::new(legs)).await;
        assert_eq!(r.payouts.len(), 24);
        assert_eq!(r.total, expected);
        assert!(rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn a_dust_leg_is_skipped_and_never_breaks_the_purchase() {
        // A rail that knows its units are worth 1e-6 USDC each.
        let rail = MockPaymentRail::with_dust_floor(1_000);
        let buyer = PubKey([0xB6; 32]);
        // 1 unit of voluntary contribution: below the floor, so skipped.
        let r = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(1_000_000), stew(1)]))
            .await;

        assert!(
            rail.verify_receipt(&r),
            "the purchase must still go through"
        );
        assert_eq!(r.payouts.len(), 1, "the dust leg is not paid");
        assert_eq!(r.stewards_amount, 0);
        assert_eq!(
            r.total, 1_000_000,
            "and the buyer is not charged for what was not paid"
        );
    }

    #[tokio::test]
    async fn a_leg_exactly_at_the_floor_is_paid() {
        let rail = MockPaymentRail::with_dust_floor(1_000);
        let buyer = PubKey([0xB7; 32]);
        let r = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(1_000_000), stew(1_000)]))
            .await;
        assert_eq!(r.payouts.len(), 2, "the floor is inclusive");
        assert_eq!(r.stewards_amount, 1_000);
        assert_eq!(r.total, 1_001_000);
        assert!(rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn the_default_floor_skips_only_a_zero_leg() {
        let rail = MockPaymentRail::new();
        assert_eq!(rail.dust_floor(), ZERO_ONLY_DUST_FLOOR);
        let buyer = PubKey([0xC0; 32]);
        // A 1-unit leg is REAL money to a currency-agnostic rail and is paid;
        // only the zero leg is dropped. This is what stops a
        // cents-denominated caller having its whole price silently skipped.
        let r = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(1), stew(0), op(199)]))
            .await;
        assert_eq!(r.payouts.len(), 2);
        assert_eq!(r.total, 200);
        assert!(rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn a_dust_only_split_is_refused_not_silently_free() {
        let rail = MockPaymentRail::with_dust_floor(1_000);
        let buyer = PubKey([0xC1; 32]);
        // Every leg is dust — including the developer's. This must NOT become
        // a free purchase.
        let s = PaymentSplit::new(vec![dev(10), stew(1)]);
        assert!(matches!(
            rail.checkout_for_item(&buyer, "game:x", s.clone()).await,
            Err(PaymentError::Rail(_))
        ));
        let r = rail.checkout(&buyer, s).await;
        assert_eq!(r.total, 0);
        assert!(
            !receipt_admits(&rail, &r, &buyer, 11),
            "a zero-total receipt must never admit a paid item"
        );
    }

    #[tokio::test]
    async fn a_receipt_for_one_distribution_is_not_a_receipt_for_another() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB8; 32]);
        // Same total, same buyer, different distribution.
        let a = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(900_000), op(100_000)]))
            .await;
        let b = rail
            .checkout(&buyer, PaymentSplit::new(vec![dev(500_000), op(500_000)]))
            .await;
        assert_eq!(a.total, b.total);
        assert_ne!(
            a.nonce, b.nonce,
            "the binding seed must commit to the legs, not just the total"
        );

        // Re-point A's receipt at B's distribution: the seed no longer matches
        // the signature.
        let mut forged = a.clone();
        forged.payouts = b.payouts.clone();
        assert!(
            !rail.verify_receipt(&forged),
            "a receipt bound to one distribution must not verify against another"
        );

        // ...and the role labels are inside the digest too, so relabelling a
        // leg changes the seed even at identical wallets and amounts.
        let relabelled = PaymentSplit::new(vec![
            dev(900_000),
            Leg::new(PubKey([0x0B; 32]), 100_000, Role::Stewards),
        ]);
        let c = rail.checkout(&buyer, relabelled).await;
        assert_ne!(a.nonce, c.nonce, "role is part of the binding");
    }

    #[tokio::test]
    async fn receipts_are_deterministic() {
        let buyer = PubKey([0xB9; 32]);
        let s = || PaymentSplit::new(vec![dev(500_000), op(100_000)]);
        let a = MockPaymentRail::new().checkout(&buyer, s()).await;
        let b = MockPaymentRail::new().checkout(&buyer, s()).await;
        assert_eq!(a.nonce, b.nonce, "deterministic nonce");
        assert_eq!(a.sig.0, b.sig.0, "deterministic signature");
        assert_eq!(a.rail_pubkey, b.rail_pubkey, "fixed rail key");
    }

    #[tokio::test]
    async fn tampered_receipt_fails_verify() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xBA; 32]);
        let mut r = rail
            .checkout(
                &buyer,
                PaymentSplit::to_developer(PubKey([0xD0; 32]), 1_000),
            )
            .await;
        assert!(rail.verify_receipt(&r));

        // Inflate a payout -> both arithmetic and signature break.
        r.payouts[0].amount = 9999;
        assert!(!rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn an_inflated_stewards_claim_is_refused() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xBB; 32]);
        let mut r = rail
            .checkout(
                &buyer,
                PaymentSplit::to_developer(PubKey([0xD0; 32]), 1_000),
            )
            .await;
        r.stewards_amount = 5_000;
        assert!(!rail.verify_receipt(&r));
    }

    #[tokio::test]
    async fn receipt_admits_is_fail_closed() {
        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xBC; 32]);
        let stranger = PubKey([0xBD; 32]);
        let r = rail
            .checkout(&buyer, PaymentSplit::to_developer(PubKey([0xD0; 32]), 500))
            .await;

        assert!(receipt_admits(&rail, &r, &buyer, 500), "exact price admits");
        assert!(receipt_admits(&rail, &r, &buyer, 100), "overpay admits");
        assert!(
            receipt_admits(&rail, &r, &stranger, 0),
            "free needs nothing"
        );

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

    #[test]
    fn split_digest_is_order_and_boundary_sensitive() {
        let a = split_digest(&[dev(1), op(2)]);
        let b = split_digest(&[op(2), dev(1)]);
        assert_ne!(a, b, "order is part of the distribution");

        // Length-prefixed role tags: no two distinct leg lists collide.
        let w = PubKey([1; 32]);
        let x = split_digest(&[
            Leg::new(w, 1, Role::Other("a".into())),
            Leg::new(w, 1, Role::Other("bc".into())),
        ]);
        let y = split_digest(&[
            Leg::new(w, 1, Role::Other("ab".into())),
            Leg::new(w, 1, Role::Other("c".into())),
        ]);
        assert_ne!(x, y);
    }

    #[test]
    fn partition_preserves_order_and_never_moves_money() {
        let s = PaymentSplit::new(vec![dev(5_000), stew(1), op(2_000), stew(999)]);
        let (payable, dust) = s.partition_at(1_000);
        assert_eq!(payable.len(), 2);
        assert_eq!(payable[0].amount, 5_000);
        assert_eq!(payable[1].amount, 2_000);
        assert_eq!(dust.len(), 2);
        assert_eq!(dust[0].amount, 1);
        assert_eq!(dust[1].amount, 999);
        // Dust is dropped, never folded into another leg.
        assert_eq!(
            PaymentSplit::new(payable).checked_total(),
            Some(7_000),
            "the buyer pays exactly the payable legs"
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
