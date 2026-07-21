//! Seam §3.6 — a **real** `PaymentRail`: SPL USDC on Solana.
//!
//! Behind the `solana` cargo feature; the offline [`crate::payment::MockPaymentRail`]
//! stays the default so CI and self-hosting need no chain, no RPC and no money.
//!
//! # This is now a THIN ADAPTER, not the rail itself
//!
//! The actual transaction construction, Ed25519 signing, JSON-RPC client and
//! on-chain verification used to live HERE (`mod.rs` + `rpc.rs` + `tx.rs` +
//! `tests.rs`, ~1760 lines, 95 tests). That code has **moved** to the sibling
//! `patala` repo's `patala-solana` crate (see `../../patala/PATALA.md` §4 and
//! §7 — "magnetite switches from its in-crate `PaymentRail` seam to depending
//! on `patala`"). This module now only:
//!
//! 1. keeps magnetite's OWN `PaymentRail` seam
//!    (`checkout`/`checkout_for_item`/`open_channel`/`escrow`/`verify_receipt`/
//!    `verify_receipt_for_item`) so backend code, and the shape of existing
//!    `payment_receipts.binding` rows, do not change;
//! 2. computes magnetite's split-into-payouts arithmetic (developer + optional
//!    operator + optional protocol fee) — pure, local, no chain involved, same
//!    as before;
//! 3. maps a **single-recipient** split onto ONE `patala_core::PayRequest` /
//!    `charge` / `verify` call against [`patala_solana::SolanaRail`].
//!
//! # The split does not generalize — and this rail says so, loudly
//!
//! `patala_core`'s seam has no multi-party split concept (`PATALA.md` §3): one
//! `charge` moves money to exactly one destination. Magnetite's own
//! `PaymentSplit` can in principle carry a real operator cut and/or a nonzero
//! `protocol_fee_bps` — i.e. more than one non-zero payout. When that happens,
//! [`SolanaPaymentRail::checkout_item`] does **not** silently drop a leg and it
//! does **not** send several non-atomic charges pretending to be one purchase:
//! it refuses with [`PaymentError::Unsupported`]. Every real caller in this
//! codebase today (`backend/src/services/payment.rs`, `marketplace.rs`)
//! collapses to exactly one leg — the developer — because hosting fees are a
//! separate payment (§3.6b) and the protocol fee is `0` by default (governance
//! decides any real fee later, `DECENTRALIZATION.md` §3.6) — so this is not a
//! capability loss for anything actually wired up, only an honest refusal for
//! the shape nothing here produces.
//!
//! # What is genuinely dropped, not merely moved
//!
//! The old rail also exposed `build_message` — build an unsigned transaction so
//! an external wallet (a browser extension, a mobile signer) could sign it
//! itself, then hand the signature back via `receipt_for_signature`. Nothing in
//! this codebase ever called it (grep confirms it) and `patala_core::PaymentRail`
//! has no such split — its `charge` always signs and sends with the rail's OWN
//! configured signer (`PATALA.md` §6: the identity key doubles as the wallet
//! key). That capability is genuinely gone, not merely relocated; see
//! `docs/payments.md` for the client-wallet-signing path if it is ever built,
//! which would need a different seam method entirely.
//!
//! # Money math
//!
//! USDC has 6 decimals. Every amount in this module is an integer count of
//! smallest units (micro-USDC). There is no floating point anywhere in the
//! money path, and [`SolanaPaymentRail::plan`] guarantees the parts sum exactly
//! to the total — unchanged from before, this arithmetic never touched chain
//! code and did not need to move.
//!
//! # Keys
//!
//! The signing key is read from `SOLANA_KEYPAIR_PATH` / `SOLANA_KEYPAIR` by
//! [`patala_solana::keys::Keypair::from_env`] — magnetite no longer has its own
//! copy of that loader. It is never logged, never serialized and never written
//! anywhere.

use std::sync::Arc;

use crate::identity::{Identity, PubKey, RawKeypairAuth, Sig};
use crate::payment::{
    ChainBinding, Channel, Escrow, PayOut, PaymentError, PaymentRail, PaymentSplit, Receipt,
    WagerTerms,
};

use patala_core::PaymentRail as PatalaPaymentRail;
pub use patala_solana::{Cluster, Commitment};
use patala_solana::{keys::Keypair, rpc::SolanaRpc, tx, SolanaRail};

/// Everything that can go wrong on this rail. Every variant is a *refusal*:
/// none of them ever results in an entitlement being granted.
#[derive(Debug, thiserror::Error)]
pub enum SolanaError {
    /// Misconfiguration — missing mint, bad RPC URL, unusable keypair, a
    /// nonzero fee with no fee wallet, ...
    #[error("solana rail misconfigured: {0}")]
    Config(String),
    /// The rail holds no key for this buyer, so it cannot sign for them
    /// (non-custodial: this process does not custody arbitrary users' keys).
    #[error("this rail cannot sign for buyer {0} (non-custodial: it can only spend for its own configured signer)")]
    NotOurKey(String),
    /// Payment channels / escrow need on-chain programs that do not exist yet.
    #[error(
        "{0} is not supported on the Solana USDC rail (no on-chain program deployed); \
         see docs/payments.md"
    )]
    Unsupported(&'static str),
    /// A split has more than one non-zero payout (a real operator cut and/or a
    /// nonzero protocol fee). `patala_core::PaymentRail::charge` is
    /// single-destination (`PATALA.md` §3) — this rail refuses rather than
    /// dropping a leg or sending several non-atomic charges under one receipt.
    #[error(
        "split has {0} non-zero payouts (developer + operator + protocol fee); patala's Solana \
         rail is single-destination and cannot pay several parties atomically in one \
         transaction — collapse to one recipient (the common case: operator cuts are a \
         separate hosting payment, §3.6b, and protocol_fee_bps defaults to 0), or perform \
         separate sequential checkouts for each leg explicitly"
    )]
    MultiPartySplit(usize),
    /// The underlying `patala-solana` rail refused or failed the operation.
    #[error("patala solana rail: {0}")]
    Patala(String),
}

impl From<SolanaError> for PaymentError {
    fn from(e: SolanaError) -> Self {
        match e {
            SolanaError::Unsupported(w) => PaymentError::Unsupported(w),
            SolanaError::MultiPartySplit(_) => {
                PaymentError::Unsupported("multi-party split checkout")
            }
            other => PaymentError::Rail(other.to_string()),
        }
    }
}

impl From<patala_core::Error> for SolanaError {
    fn from(e: patala_core::Error) -> Self {
        SolanaError::Patala(e.to_string())
    }
}

/// Magnetite-level rail configuration: patala's own rail config, plus the one
/// thing `patala_core`'s seam does not model — where a protocol fee (if any)
/// goes. See [`SolanaError::MultiPartySplit`] for what happens if this is
/// actually used alongside a real payout.
#[derive(Clone, Debug)]
pub struct SolanaConfig {
    /// Everything patala's Solana rail itself needs (RPC URL, cluster,
    /// commitment, USDC mint).
    pub inner: patala_solana::SolanaConfig,
    /// Where the protocol fee goes. Required whenever `protocol_fee_bps > 0`.
    pub fee_wallet: Option<PubKey>,
}

/// A concrete, integer-exact plan for one checkout. Pure arithmetic — no
/// chain, no patala involved, unchanged from before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Ordered payouts: developer, [operator], [protocol fee].
    pub payouts: Vec<PayOut>,
    /// The fee component (0 when `protocol_fee_bps == 0`).
    pub protocol_fee: u64,
    /// `sum(payouts)` — checked, not assumed.
    pub total: u64,
}

/// Domain-separated binding reference: `blake3("magnetite-pay-v1" || buyer ||
/// item)`. Magnetite's OWN local item<->receipt consistency hash — distinct
/// from (and checked in addition to) patala's own domain-separated binding,
/// which uses a different tag so a receipt from one can never be mistaken for
/// a receipt from the other (see `patala_solana::binding_reference`).
pub fn binding_reference(buyer: &PubKey, item: &str) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"magnetite-pay-v1");
    h.update(&buyer.0);
    h.update(&(item.len() as u64).to_le_bytes());
    h.update(item.as_bytes());
    *h.finalize().as_bytes()
}

fn patala_pubkey(pk: &PubKey) -> patala_solana::keys::PubKey {
    patala_solana::keys::PubKey(pk.0)
}

/// Best-effort peek at the rail's own opaque proof blob for a human-readable
/// field (e.g. the on-chain tx signature), for storage/display only. **Never**
/// used for verification — that always goes through
/// [`patala_solana::SolanaRail::verify`] against the unmodified proof bytes.
/// If patala's internal proof shape ever changes this silently degrades to
/// `None` rather than breaking a charge.
fn peek_proof_str(proof: &[u8], key: &str) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(proof)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_string)
}

/// SPL-USDC-on-Solana payment rail — magnetite's seam, patala's crypto.
pub struct SolanaPaymentRail {
    cfg: SolanaConfig,
    /// The actual chain rail: tx construction, signing, RPC, verification.
    inner: SolanaRail,
    /// Key that signs magnetite's OWN receipt wrapper (a self-consistency
    /// marker, NOT the security boundary — chain state, checked by `inner`,
    /// is). Fixed seed, same as before the move to patala.
    rail: RawKeypairAuth,
}

impl SolanaPaymentRail {
    /// Build a rail over an arbitrary RPC implementation (unit tests pass a
    /// fake; production passes [`patala_solana::rpc::HttpRpc`]). No signer —
    /// verify-only until [`Self::with_signer`].
    pub fn new(cfg: SolanaConfig, rpc: Arc<dyn SolanaRpc>) -> Self {
        let inner = SolanaRail::new(cfg.inner.clone(), rpc);
        Self {
            cfg,
            inner,
            rail: RawKeypairAuth::from_seed(*blake3::hash(b"magnetite-solana-rail").as_bytes()),
        }
    }

    /// Attach a signing key so this rail can submit transactions itself.
    pub fn with_signer(mut self, signer: Keypair) -> Self {
        self.inner = self.inner.with_signer(signer);
        self
    }

    /// Build a rail whose signer (if any) is loaded from
    /// `SOLANA_KEYPAIR_PATH`/`SOLANA_KEYPAIR` (see
    /// [`patala_solana::keys::Keypair::from_env`]).
    pub fn from_env(cfg: SolanaConfig, rpc: Arc<dyn SolanaRpc>) -> Result<Self, SolanaError> {
        let inner = SolanaRail::from_env(cfg.inner.clone(), rpc).map_err(SolanaError::from)?;
        Ok(Self {
            cfg,
            inner,
            rail: RawKeypairAuth::from_seed(*blake3::hash(b"magnetite-solana-rail").as_bytes()),
        })
    }

    /// The rail's receipt-signing public key (magnetite's own bookkeeping key,
    /// see the `rail` field doc).
    pub fn rail_pubkey(&self) -> PubKey {
        self.rail.node_pubkey()
    }

    /// The configuration (read-only).
    pub fn config(&self) -> &SolanaConfig {
        &self.cfg
    }

    /// The wallet this rail can sign for, if any.
    pub fn signer_pubkey(&self) -> Option<PubKey> {
        self.inner.signer_pubkey().map(|pk| PubKey(pk.0))
    }

    /// Integer-exact split. The fee is taken **on top of** the subtotal,
    /// matching the mock rail, and the parts are asserted to sum to the
    /// total. Unchanged from before — pure arithmetic, no chain, no patala.
    pub fn plan(&self, split: &PaymentSplit) -> Result<Plan, SolanaError> {
        let dev = split.developer.amount;
        let op = split.operator.as_ref().map(|s| s.amount).unwrap_or(0);
        let subtotal = dev
            .checked_add(op)
            .ok_or_else(|| SolanaError::Config("split subtotal overflows u64".into()))?;
        // u128 intermediate, integer division — no floats, no rounding surprises.
        let fee = u64::try_from((subtotal as u128 * split.protocol_fee_bps as u128) / 10_000)
            .map_err(|_| SolanaError::Config("protocol fee overflows u64".into()))?;
        let total = subtotal
            .checked_add(fee)
            .ok_or_else(|| SolanaError::Config("split total overflows u64".into()))?;

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
            let fee_wallet = self.cfg.fee_wallet.ok_or_else(|| {
                SolanaError::Config(
                    "protocol_fee_bps > 0 but no fee wallet configured (SOLANA_FEE_WALLET)".into(),
                )
            })?;
            payouts.push(PayOut {
                wallet: fee_wallet,
                amount: fee,
            });
        }

        let sum: u64 = payouts.iter().map(|p| p.amount).sum();
        if sum != total {
            return Err(SolanaError::Config(format!(
                "split parts {sum} do not sum to total {total}"
            )));
        }
        Ok(Plan {
            payouts,
            protocol_fee: fee,
            total,
        })
    }

    /// Build → charge (via `patala_solana::SolanaRail::charge`) → return the
    /// bound receipt.
    ///
    /// Only possible when this process holds `buyer`'s key; otherwise
    /// [`SolanaError::NotOurKey`], because the rail is non-custodial and will
    /// not pretend to spend money it cannot move. Only possible when the plan
    /// collapses to exactly one non-zero payout; otherwise
    /// [`SolanaError::MultiPartySplit`] — see the module docs.
    pub async fn checkout_item(
        &self,
        buyer: &PubKey,
        item: &str,
        split: PaymentSplit,
    ) -> Result<Receipt, SolanaError> {
        let signer_pk = self
            .inner
            .signer_pubkey()
            .ok_or_else(|| SolanaError::NotOurKey(buyer.to_hex()))?;
        if signer_pk.0 != buyer.0 {
            return Err(SolanaError::NotOurKey(buyer.to_hex()));
        }

        let plan = self.plan(&split)?;
        let legs: Vec<&PayOut> = plan.payouts.iter().filter(|p| p.amount > 0).collect();
        let leg = match legs.as_slice() {
            [one] => *one,
            [] => return Err(SolanaError::Config("split has no non-zero payout".into())),
            many => return Err(SolanaError::MultiPartySplit(many.len())),
        };

        let req = patala_core::PayRequest {
            amount_minor: leg.amount,
            currency: "USDC".to_string(),
            destination: tx::pubkey_to_base58(&patala_pubkey(&leg.wallet)),
            reference: item.to_string(),
        };
        let patala_receipt = self.inner.charge(&req).await.map_err(SolanaError::from)?;
        Ok(self.wrap_receipt(buyer, item, &plan, &patala_receipt))
    }

    fn wrap_receipt(
        &self,
        buyer: &PubKey,
        item: &str,
        plan: &Plan,
        p: &patala_core::Receipt,
    ) -> Receipt {
        let tx_signature = peek_proof_str(&p.proof, "tx_signature").unwrap_or_default();
        let mut r = Receipt {
            buyer: *buyer,
            payouts: plan.payouts.clone(),
            protocol_fee: plan.protocol_fee,
            total: plan.total,
            nonce: binding_reference(buyer, item),
            rail_pubkey: self.rail.node_pubkey(),
            sig: Sig([0u8; 64]),
            binding: Some(ChainBinding {
                chain: "solana".to_string(),
                tx_signature,
                item: item.to_string(),
                mint: self.cfg.inner.mint_base58(),
                reference: hex::encode(binding_reference(buyer, item)),
                rail_proof: p.proof.clone(),
            }),
        };
        r.sig = self.rail.sign(&r.signing_bytes());
        r
    }

    // ── Verification ─────────────────────────────────────────────────────────

    /// The full check, async. **Every** error path means "do not grant".
    async fn verify_async(&self, r: &Receipt, expect_item: Option<&str>) -> Result<(), SolanaError> {
        let b = r
            .binding
            .as_ref()
            .ok_or_else(|| SolanaError::Config("receipt carries no chain binding".into()))?;

        // 1. Right chain, right mint (magnetite-local; patala's own verify
        //    re-checks mint against ITS configured mint from the proof too —
        //    this is a cheap, local, defense-in-depth duplicate of that same
        //    fact, not a new claim).
        if b.chain != "solana" {
            return Err(SolanaError::Config(format!("chain {:?}", b.chain)));
        }
        if b.mint != self.cfg.inner.mint_base58() {
            return Err(SolanaError::Config(
                "claimed mint is not the configured USDC mint".into(),
            ));
        }

        // 2. The binding must be the one derived from (buyer, item) — a
        //    receipt cannot be re-pointed at a different item by editing a
        //    field. Magnetite-local, no chain needed.
        let expected_ref = hex::encode(binding_reference(&r.buyer, &b.item));
        if b.reference != expected_ref {
            return Err(SolanaError::Config(
                "binding reference does not match (buyer, item)".into(),
            ));
        }
        // 3. ...and it must be the item the CALLER is asking about. This is
        //    what stops a real, fully-valid receipt for a cheap item
        //    unlocking an expensive one.
        if let Some(item) = expect_item {
            if b.item != item {
                return Err(SolanaError::Config(format!(
                    "receipt is bound to item {:?}, not {:?}",
                    b.item, item
                )));
            }
        }

        // 4. Internal arithmetic + magnetite's own rail signature (cheap,
        //    local — a self-consistency marker, NOT the security boundary;
        //    see the `rail` field doc and patala-solana's README).
        let sum: u64 = r
            .payouts
            .iter()
            .try_fold(0u64, |a, p| a.checked_add(p.amount))
            .ok_or_else(|| SolanaError::Config("payouts overflow".into()))?;
        if sum != r.total {
            return Err(SolanaError::Config("payouts do not sum to total".into()));
        }
        if !<RawKeypairAuth as Identity>::verify(&r.rail_pubkey, &r.signing_bytes(), &r.sig) {
            return Err(SolanaError::Config("receipt signature invalid".into()));
        }

        // 5. THE security boundary: delegate to patala for on-chain state —
        //    chain/mint match, transaction success, commitment, the buyer's
        //    real Ed25519 tx signature, the memo binding, and the exact
        //    token-balance deltas (`PATALA.md` §3, §7). The opaque proof
        //    bytes are carried through unmodified from charge time; patala
        //    re-derives everything from them plus current chain state.
        let patala_receipt = patala_core::Receipt {
            rail_id: "solana".to_string(),
            amount_minor: r.total,
            currency: "USDC".to_string(),
            reference: b.item.clone(),
            proof: b.rail_proof.clone(),
            settled_at_unix: 0,
        };
        let verified = self
            .inner
            .verify(&patala_receipt)
            .await
            .map_err(SolanaError::from)?;
        if !verified {
            return Err(SolanaError::Config(
                "patala rail could not verify the on-chain payment".into(),
            ));
        }
        Ok(())
    }

    /// Drive [`Self::verify_async`] from a synchronous caller
    /// (`PaymentRail::verify_receipt` is sync by seam contract). Builds a
    /// fresh current-thread runtime on its own OS thread so this never panics
    /// when called from inside an already-running async context (the backend
    /// calls `verify_receipt` from async handlers) — the same approach as
    /// before the move to patala; this bridging is generic, not solana-specific,
    /// and did not need to change.
    fn verify_blocking(&self, r: &Receipt, item: Option<String>) -> bool {
        let rpc_result = std::thread::scope(|s| {
            s.spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| SolanaError::Config(format!("runtime: {e}")))
                    .map(|rt| rt.block_on(self.verify_async(r, item.as_deref())))
            })
            .join()
        });
        match rpc_result {
            Ok(Ok(Ok(()))) => true,
            // Unreachable RPC, unconfirmed, mismatch, bad runtime, panic — all deny.
            Ok(Ok(Err(_))) | Ok(Err(_)) | Err(_) => false,
        }
    }
}

#[async_trait::async_trait]
impl PaymentRail for SolanaPaymentRail {
    /// Unbound checkout. The Solana rail REQUIRES an item binding, so this
    /// returns a receipt with no binding — which by construction fails
    /// verification. Use [`SolanaPaymentRail::checkout_item`] /
    /// [`Self::checkout_for_item`]. Unchanged from before.
    async fn checkout(&self, buyer: &PubKey, split: PaymentSplit) -> Receipt {
        let plan = self.plan(&split).unwrap_or(Plan {
            payouts: Vec::new(),
            protocol_fee: 0,
            total: 0,
        });
        let mut r = Receipt {
            buyer: *buyer,
            payouts: plan.payouts,
            protocol_fee: plan.protocol_fee,
            total: plan.total,
            nonce: [0u8; 32],
            rail_pubkey: self.rail.node_pubkey(),
            sig: Sig([0u8; 64]),
            binding: None,
        };
        r.sig = self.rail.sign(&r.signing_bytes());
        r
    }

    async fn checkout_for_item(
        &self,
        buyer: &PubKey,
        item: &str,
        split: PaymentSplit,
    ) -> Result<Receipt, PaymentError> {
        Ok(self.checkout_item(buyer, item, split).await?)
    }

    async fn open_channel(&self, _peer: &PubKey) -> Result<Channel, PaymentError> {
        Err(SolanaError::Unsupported("payment channels").into())
    }

    async fn escrow(&self, _terms: WagerTerms) -> Result<Escrow, PaymentError> {
        Err(SolanaError::Unsupported("wager escrow").into())
    }

    fn verify_receipt(&self, r: &Receipt) -> bool {
        self.verify_blocking(r, None)
    }

    fn verify_receipt_for_item(&self, r: &Receipt, item: &str) -> bool {
        self.verify_blocking(r, Some(item.to_string()))
    }
}

#[cfg(test)]
mod tests;
