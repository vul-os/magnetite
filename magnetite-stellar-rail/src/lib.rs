//! Seam §3.6 — a **real** `PaymentRail`: native USDC on Stellar.
//!
//! Backlog A12: replaces `magnetite-solana-rail` (deleted in the same change
//! that added this crate — see `magnetite-solana-rail`'s absence from
//! `ci/rust-crates.json`/this repo, and `docs/cross-repo-backlog.md`). A12
//! required the precondition "Stellar rail lands, and a payment has actually
//! settled" to be met **before** the port — met by the sibling `patala` repo's
//! `patala-stellar` crate, which settled one single-leg USDC-shaped payment on
//! Stellar testnet 2026-07-30 (tx
//! `32663937fe1407f9de3e781effa6ac9f4b1d29340ea63e72f6335a6c91effb89`, ledger
//! `3882739`). **That settlement is patala's, not this crate's — see "HONEST
//! STATUS" below.**
//!
//! # STANDALONE (owner directive #1, `FANOUT-LOOP-STATE.md` §1)
//!
//! This crate has **no dependency, path or git, on the sibling `patala`
//! repo** — a stricter standalone-ness than `magnetite-solana-rail`, which
//! depended on `patala-core`/`patala-solana` via a pinned git rev. Every
//! module here (`keys`, `tx`, `rpc`, and this file) is written fresh, copying
//! `patala/patala-stellar`'s *design* (documented in its `README.md`/`src/
//! lib.rs`) by hand — exactly as `rotation.rs` (A8) and `chunktree.rs` (A24)
//! copied evermesh's semantics without a dependency. What IS shared is a
//! dependency on the same third-party, Apache-2.0 upstream crates
//! (`stellar-xdr`/`stellar-strkey`, published by the Stellar Development
//! Foundation) that `patala-stellar` also depends on — that is not "depending
//! on patala" any more than both crates depending on `tokio` would be.
//!
//! # HONEST STATUS — READ FIRST
//!
//! **No payment has ever settled through this crate.** Its transaction
//! construction path has never run against a real Horizon instance — not
//! testnet, not mainnet. Everything below is exercised offline against a
//! scripted fake Horizon only (`src/tests.rs`). The testnet settlement that
//! satisfied A12's precondition belongs to `patala-stellar`, a sibling
//! implementation of the same wire format built independently in a different
//! repo — its evidence does not transfer to this crate's own code, which has
//! never been run live. Do not point this crate at mainnet, or claim testnet
//! capability for it specifically, before someone completes a real Horizon
//! round trip through *this* crate's `HorizonRpc`.
//!
//! # What this crate owns
//!
//! Everything: unlike `magnetite-solana-rail` (which delegated Ed25519
//! signing, StrKey/base58, and XDR/message construction to patala's own
//! crates), this crate owns the Ed25519 keypair ([`keys`]), the XDR
//! transaction construction/signing/hashing ([`tx`]), and the Horizon REST
//! client ([`rpc`]) itself, built on the Stellar Development Foundation's own
//! `stellar-xdr`/`stellar-strkey` codec crates (see `Cargo.toml`).
//!
//! # The ten fail-closed checks, ported from `magnetite-solana-rail`
//!
//! [`StellarPaymentRail::verify_async`] carries the same ten numbered checks
//! `magnetite-solana-rail`'s `verify_async` did (ported here as `1.`
//! through `10.` in its own doc comments), re-expressed over Stellar's wire
//! shape:
//!
//! 1. the receipt carries a chain binding, and the chain is `"stellar"`;
//! 2. the claimed USDC issuer equals the configured one (plus the rail proof's
//!    own internal consistency: it parses, names the same chain/issuer/buyer,
//!    and its legs agree with the receipt's payouts — including the stewards
//!    destination re-check the Solana rail also did);
//! 3. the binding reference is the one derived from `(buyer, item, legs)`;
//! 4. ...and it is the item the caller is asking about;
//! 5. payouts sum EXACTLY to the total, the rail's own self-consistency
//!    signature is intact, **and** — strengthened relative to the Solana
//!    port, see below — the transaction is rebuilt from the proof's own
//!    scalar fields and its Ed25519 signature is verified **offline**,
//!    cryptographically, against the claimed source;
//! 6. the transaction is known to Horizon;
//! 7. it landed successfully (no `meta.err` equivalent — Stellar's
//!    `successful` field);
//! 8. the buyer is the transaction's own source account (decoded from the
//!    envelope Horizon actually returns, not trusted from a summary field);
//! 9. the on-chain `MEMO_HASH` is EXACTLY the derived binding reference;
//! 10. the money: every decoded `PAYMENT` operation, in order, pays exactly
//!     the wallet and amount a leg claims, in the configured USDC asset, with
//!     no extra/missing operation.
//!
//! **Check 5 is strengthened, not weakened**, relative to the Solana port:
//! `magnetite-solana-rail` trusted the RPC's `"signer": true` JSON field for
//! "the buyer signed this" (folded into its own check 8) and never
//! cryptographically re-verified a signature offline at all. This crate does
//! — following `patala-stellar`'s design, which does the same — because
//! Stellar's `MEMO_HASH`-carrying, single-signature transaction shape makes an
//! offline rebuild-and-verify cheap and exact. Nothing was removed to make
//! room for it.
//!
//! What was **not** ported: `magnetite-solana-rail`'s wire-byte-size refusal
//! (`MAX_TX_WIRE_BYTES`) has no Stellar analogue — Stellar's hard limit is an
//! **operation count** ([`tx::MAX_OPERATIONS`]), which this crate checks
//! directly and more precisely (no serialized-byte estimate needed).
//!
//! # Money math
//!
//! USDC on Stellar has 7 decimals (`tx::USDC_DECIMALS`) — classic Stellar XDR
//! amounts are always this fixed-point `int64` scale. Every amount here is an
//! integer count of ten-millionths. **No floating point anywhere in the money
//! path.**
//!
//! # Keys
//!
//! The signing key is read from `STELLAR_SECRET_KEY` by
//! [`keys::Keypair::from_env`]. Never logged, never serialized, never written
//! anywhere.

pub mod keys;
pub mod rpc;
pub mod tx;

use std::sync::Arc;

use magnetite_seams::identity::{Identity, IdentityVerifier, PubKey, RawKeypairAuth, Sig};
use magnetite_seams::payment::{
    split_digest, ChainBinding, Channel, Escrow, Leg, PayOut, PaymentError, PaymentRail,
    PaymentSplit, Receipt, Role, WagerTerms,
};

use rpc::StellarRpc;

/// Smallest leg this rail will pay, in stroops (USDC has 7 decimals: `1_000`
/// stroops = `0.0001` USDC).
///
/// A leg must be worth at least what it costs to include: Stellar's fee bid
/// scales with operation count (`tx::total_fee`), so every additional leg in
/// an atomic bundle adds real cost. `1_000` stroops sits three orders of
/// magnitude above Stellar's typical per-operation fee (~100 stroops), so
/// ordinary prices are nowhere near this floor. Chosen as a fixed integer, not
/// derived from a live price oracle — see `magnetite_solana_rail`'s identical
/// reasoning for its own `DUST_FLOOR_MICRO_USDC` (that crate is gone; the
/// reasoning is not).
pub const DUST_FLOOR_STROOPS: u64 = 1_000;

/// USDC issuer as StrKey (`G...`). Anything paid in an asset with a different
/// issuer, or a different code, is not a USDC payment.
pub type UsdcIssuer = String;

/// Circle's publicly-documented USDC issuing account on the Stellar public
/// network — stated as a public fact, not independently re-verified against a
/// live ledger from this crate (mirrors `patala_stellar::CIRCLE_USDC_ISSUER_PUBLIC`,
/// which is where this address is drawn from; both crates state the same
/// public fact independently).
pub const CIRCLE_USDC_ISSUER_PUBLIC: &str =
    "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";

/// Which Stellar network the rail is pointed at. `Public` moves real money.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    /// Real funds, real losses. **This crate has never touched this
    /// network.**
    Public,
    /// Free test USDC / test assets.
    Testnet,
}

impl Network {
    /// The network passphrase Stellar defines for this network.
    pub fn passphrase(&self) -> &'static str {
        match self {
            Network::Public => "Public Global Stellar Network ; September 2015",
            Network::Testnet => "Test SDF Network ; September 2015",
        }
    }

    /// Does this network move real money?
    pub fn is_mainnet(&self) -> bool {
        matches!(self, Network::Public)
    }
}

// ── The stewards destination ─────────────────────────────────────────────────

/// Where the voluntary contribution to magnetite's maintainers goes on this
/// rail. Same non-forgeable design as `magnetite_solana_rail::stewards` (that
/// crate is gone; the reasoning is not — see [`resolve_stewards`]).
pub mod stewards {
    /// The stewards wallet baked in at **build** time from
    /// `MAGNETITE_STEWARDS_WALLET_STELLAR` (StrKey `G...`).
    ///
    /// **`None` in this tree, and that is the honest state**: no release
    /// sets this. While it is `None` a [`super::Role::Stewards`] leg cannot be
    /// paid at all.
    pub const COMPILED_IN: Option<&str> = option_env!("MAGNETITE_STEWARDS_WALLET_STELLAR");

    /// Testnet-only runtime override. On `Network::Public` its mere presence
    /// is a **fatal misconfiguration** — see [`super::resolve_stewards`].
    pub const TESTNET_OVERRIDE_ENV: &str = "MAGNETITE_STEWARDS_WALLET_STELLAR_TESTNET";
}

/// See [`stewards`]. Separate so tests can drive it directly. Same
/// three-state shape `magnetite_solana_rail::resolve_stewards` documented:
/// nothing configured (`Ok(None)`, legitimate), configured and honourable
/// (`Ok(Some(pk))`), or configured but this build/network cannot honour it
/// (`Err`, loud, never silently ignored).
pub fn resolve_stewards(network: Network) -> Result<Option<PubKey>, StellarError> {
    let override_env = std::env::var(stewards::TESTNET_OVERRIDE_ENV).ok();
    if let Some(raw) = override_env {
        if network.is_mainnet() {
            return Err(StellarError::Stewards(format!(
                "{} is set, but the network is Public (mainnet). The stewards destination on \
                 mainnet comes from the signed release ONLY (compiled in) — a node operator \
                 must not be able to redirect other people's voluntary contribution. Unset the \
                 override, or use testnet. Refusing to start.",
                stewards::TESTNET_OVERRIDE_ENV
            )));
        }
        let pk = keys::from_strkey(raw.trim()).map_err(|e| {
            StellarError::Stewards(format!(
                "{} is not a valid stellar address: {e}",
                stewards::TESTNET_OVERRIDE_ENV
            ))
        })?;
        return Ok(Some(pk));
    }
    match stewards::COMPILED_IN {
        None => Ok(None),
        Some(raw) => {
            let pk = keys::from_strkey(raw.trim()).map_err(|e| {
                StellarError::Stewards(format!(
                    "the compiled-in stewards address ({raw:?}, baked in from \
                     MAGNETITE_STEWARDS_WALLET_STELLAR at build time) is not a valid stellar \
                     address: {e}. This is a build defect, not a runtime one."
                ))
            })?;
            Ok(Some(pk))
        }
    }
}

/// Everything that can go wrong on this rail. Every variant is a refusal:
/// none of them ever results in an entitlement being granted.
#[derive(Debug, thiserror::Error)]
pub enum StellarError {
    /// Misconfiguration — bad issuer address, unusable keypair, bad asset
    /// code, ...
    #[error("stellar rail misconfigured: {0}")]
    Config(String),
    /// A StrKey address failed to decode.
    #[error("not a valid stellar address: {0}")]
    BadAddress(String),
    /// Building, encoding, or decoding an XDR value failed.
    #[error("stellar xdr: {0}")]
    Xdr(String),
    /// Horizon was unreachable, slow, or answered with an error.
    #[error("stellar horizon: {0}")]
    Rpc(String),
    /// The rail holds no key for this buyer (non-custodial).
    #[error("this rail cannot sign for buyer {0} (non-custodial: it can only spend for its own configured signer)")]
    NotOurKey(String),
    /// Payment channels / escrow need on-chain programs that do not exist.
    #[error("{0} is not supported on the Stellar USDC rail (no on-chain program deployed)")]
    Unsupported(&'static str),
    /// Every leg was below [`DUST_FLOOR_STROOPS`].
    #[error(
        "split has no payable leg: all {0} legs are below the {DUST_FLOOR_STROOPS} stroop dust \
         floor, so this purchase would move no money"
    )]
    NoPayableLeg(usize),
    /// A `Role::Stewards` leg named a wallet other than the compiled-in
    /// stewards address, or there is no compiled-in address to name.
    #[error("stewards leg refused: {0}")]
    Stewards(String),
    /// A split with more legs than Stellar's operation limit.
    #[error(
        "a split with {legs} legs exceeds Stellar's {max}-operation-per-transaction limit; it \
         cannot be paid atomically in ONE transaction and this rail will not split it into \
         several non-atomic ones"
    )]
    TooManyLegs {
        /// How many legs were payable.
        legs: usize,
        /// The protocol limit that was exceeded.
        max: usize,
    },
}

impl From<StellarError> for PaymentError {
    fn from(e: StellarError) -> Self {
        match e {
            StellarError::Unsupported(w) => PaymentError::Unsupported(w),
            other => PaymentError::Rail(other.to_string()),
        }
    }
}

/// Rail configuration.
#[derive(Clone, Debug)]
pub struct StellarConfig {
    /// Which network this rail talks to.
    pub network: Network,
    /// The USDC issuer account (StrKey `G...`).
    pub usdc_issuer: UsdcIssuer,
    /// Per-operation fee bid, in stroops.
    pub base_fee_stroops: u32,
    /// The resolved stewards wallet, or `None` when this build has no
    /// compiled-in address. Set through [`StellarConfig::resolve`] so the
    /// mainnet rule cannot be bypassed by construction; `pub` only because
    /// tests in this crate build configs directly.
    pub stewards: Option<PubKey>,
}

impl StellarConfig {
    /// Testnet config with an explicit issuer (the testnet issuer rotates and
    /// is not a fixed well-known constant, unlike mainnet's).
    pub fn testnet(usdc_issuer: impl Into<String>) -> Self {
        Self {
            network: Network::Testnet,
            usdc_issuer: usdc_issuer.into(),
            base_fee_stroops: 100,
            stewards: None,
        }
    }

    /// Resolve the stewards destination for `network`, failing closed —
    /// see [`resolve_stewards`].
    pub fn resolve(
        network: Network,
        usdc_issuer: impl Into<String>,
        base_fee_stroops: u32,
    ) -> Result<Self, StellarError> {
        let stewards = resolve_stewards(network)?;
        Ok(Self {
            network,
            usdc_issuer: usdc_issuer.into(),
            base_fee_stroops,
            stewards,
        })
    }
}

/// A concrete, integer-exact plan for one checkout. Pure arithmetic — no
/// chain, no RPC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The legs that will actually be paid, in order — one Stellar `PAYMENT`
    /// operation each. Sums EXACTLY to [`Plan::total`].
    pub legs: Vec<Leg>,
    /// Legs skipped for being below [`DUST_FLOOR_STROOPS`].
    pub skipped: Vec<Leg>,
    /// The voluntary [`Role::Stewards`] component of [`Plan::legs`].
    pub stewards_amount: u64,
    /// `sum(legs)` — checked, not assumed.
    pub total: u64,
}

impl Plan {
    /// The paid legs as receipt payouts.
    pub fn payouts(&self) -> Vec<PayOut> {
        self.legs.iter().map(PayOut::from).collect()
    }
}

/// Domain-separated binding reference:
/// `blake3("magnetite-pay-v2" ‖ buyer ‖ len(item) ‖ item ‖ split_digest(legs))`.
///
/// **Identical derivation to `magnetite_solana_rail::binding_reference`** —
/// this is magnetite's OWN item-and-distribution↔receipt consistency hash
/// (`magnetite_seams::payment::ChainBinding`'s own doc), not a per-chain
/// scheme, so the same buyer/item/split produces the same reference
/// regardless of which rail settles it; the `chain` field is what
/// discriminates rails, checked separately (check 1). Carried directly as the
/// 32-byte Stellar `MEMO_HASH` — unlike the Solana rail, which had to
/// hex-encode this into an SPL Memo *string* instruction, Stellar's memo
/// field already is exactly 32 bytes.
pub fn binding_reference(buyer: &PubKey, item: &str, legs: &[Leg]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"magnetite-pay-v2");
    h.update(&buyer.0);
    h.update(&(item.len() as u64).to_le_bytes());
    h.update(item.as_bytes());
    h.update(&split_digest(legs));
    *h.finalize().as_bytes()
}

/// The rail-specific data carried in [`ChainBinding::rail_proof`] (JSON).
/// Opaque to `magnetite-seams`; re-read here at verification time and checked
/// against current chain state and re-derived offline, never trusted on its
/// own.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RailProof {
    /// Always `"stellar"` (checked, not assumed).
    chain: String,
    /// On-chain transaction hash (hex).
    tx_hash: String,
    /// The USDC issuer (StrKey `G...`).
    asset_issuer: String,
    /// The paying wallet (StrKey `G...`) — also the signing identity and the
    /// transaction's source account.
    buyer: String,
    /// Every paid leg, in order.
    legs: Vec<ProofLeg>,
    /// Hex [`binding_reference`], also carried on-chain as `MEMO_HASH`.
    reference: String,
    /// The exact sequence number consumed — needed to rebuild and re-hash the
    /// identical transaction offline (check 5).
    seq_num: i64,
    /// The exact total fee bid used — needed for the same offline rebuild.
    fee: u32,
    /// Hex Ed25519 signature over the transaction hash, by `buyer`. Verified
    /// offline against the rebuilt hash (check 5) — a genuine cryptographic
    /// guarantee, not a JSON field trusted from Horizon.
    signature: String,
}

/// One leg as recorded in [`RailProof`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ProofLeg {
    /// Destination wallet, StrKey `G...`.
    wallet: String,
    /// Amount in stroops.
    amount: u64,
    /// [`Role::tag`] of the leg.
    role: String,
}

/// Map a [`Role::tag`] back to a [`Role`]. Same labelling-collision caveat as
/// `magnetite_solana_rail::role_from_tag`: `Other("developer")` and
/// `Developer` share a tag; what moves money is the wallet and amount, both
/// checked against the chain independently.
fn role_from_tag(tag: &str) -> Role {
    match tag {
        "developer" => Role::Developer,
        "operator" => Role::Operator,
        "stewards" => Role::Stewards,
        other => Role::Other(other.to_string()),
    }
}

/// Native-USDC-on-Stellar payment rail — magnetite's split logic, this
/// crate's own Stellar primitives.
pub struct StellarPaymentRail {
    cfg: StellarConfig,
    rpc: Arc<dyn StellarRpc>,
    /// The wallet this rail can spend from — absent for a verify-only rail.
    signer: Option<keys::Keypair>,
    /// Key that signs magnetite's OWN receipt wrapper (self-consistency
    /// marker, NOT the security boundary — chain state is).
    rail: RawKeypairAuth,
}

impl StellarPaymentRail {
    /// Build a rail over an arbitrary RPC implementation (unit tests pass a
    /// fake; production passes [`rpc::HorizonRpc`], never live-exercised from
    /// this crate — see the module docs). No signer — verify-only until
    /// [`Self::with_signer`].
    pub fn new(cfg: StellarConfig, rpc: Arc<dyn StellarRpc>) -> Self {
        Self {
            cfg,
            rpc,
            signer: None,
            rail: RawKeypairAuth::from_seed(*blake3::hash(b"magnetite-stellar-rail").as_bytes()),
        }
    }

    /// Attach a signing key so this rail can submit transactions itself.
    pub fn with_signer(mut self, signer: keys::Keypair) -> Self {
        self.signer = Some(signer);
        self
    }

    /// Build a rail whose signer (if any) is loaded from `STELLAR_SECRET_KEY`.
    pub fn from_env(cfg: StellarConfig, rpc: Arc<dyn StellarRpc>) -> Result<Self, StellarError> {
        let rail = Self::new(cfg, rpc);
        Ok(match keys::Keypair::from_env()? {
            Some(k) => rail.with_signer(k),
            None => rail,
        })
    }

    /// The rail's receipt-signing public key (magnetite's own bookkeeping key).
    pub fn rail_pubkey(&self) -> PubKey {
        self.rail.node_pubkey()
    }

    /// The configuration (read-only).
    pub fn config(&self) -> &StellarConfig {
        &self.cfg
    }

    /// The wallet this rail can sign for, if any.
    pub fn signer_pubkey(&self) -> Option<PubKey> {
        self.signer.as_ref().map(|s| s.pubkey())
    }

    /// The stewards destination this rail will pay, if this build has one.
    pub fn stewards_wallet(&self) -> Option<PubKey> {
        self.cfg.stewards
    }

    /// Integer-exact plan. Pure arithmetic, no chain, no floats, no rates.
    /// Same refusal rules as `magnetite_solana_rail::SolanaPaymentRail::plan`
    /// (overflow, stewards destination not caller-choosable, dust-only
    /// refused).
    pub fn plan(&self, split: &PaymentSplit) -> Result<Plan, StellarError> {
        split.checked_total().ok_or_else(|| {
            StellarError::Config("split legs overflow u64; refusing to state a total".into())
        })?;

        for leg in split.legs.iter().filter(|l| l.role.is_stewards()) {
            match self.cfg.stewards {
                None => {
                    return Err(StellarError::Stewards(format!(
                        "this build has no compiled-in stewards address \
                         (MAGNETITE_STEWARDS_WALLET_STELLAR was not set when it was built), so a \
                         stewards leg cannot be paid. Refusing rather than paying the \
                         caller-supplied wallet {} or silently dropping the leg.",
                        keys::to_strkey(&leg.wallet)
                    )))
                }
                Some(want) if want != leg.wallet => {
                    return Err(StellarError::Stewards(format!(
                        "a stewards leg names {}, but this build's stewards address is {}. The \
                         destination comes from the signed release, never from the caller or the \
                         node's environment.",
                        keys::to_strkey(&leg.wallet),
                        keys::to_strkey(&want)
                    )))
                }
                Some(_) => {}
            }
        }

        let (legs, skipped) = split.partition_at(DUST_FLOOR_STROOPS);
        if legs.is_empty() {
            return Err(StellarError::NoPayableLeg(split.legs.len()));
        }
        if legs.len() > tx::MAX_OPERATIONS {
            return Err(StellarError::TooManyLegs {
                legs: legs.len(),
                max: tx::MAX_OPERATIONS,
            });
        }

        let paid = PaymentSplit::new(legs);
        let total = paid.checked_total().ok_or_else(|| {
            StellarError::Config("payable legs overflow u64; refusing to state a total".into())
        })?;
        let sum = paid
            .legs
            .iter()
            .try_fold(0u64, |a, l| a.checked_add(l.amount))
            .ok_or_else(|| StellarError::Config("payable legs overflow u64".into()))?;
        if sum != total {
            return Err(StellarError::Config(format!(
                "split legs {sum} do not sum exactly to total {total}"
            )));
        }

        Ok(Plan {
            stewards_amount: paid.stewards_total(),
            legs: paid.legs,
            skipped,
            total,
        })
    }

    /// Build ONE transaction paying every leg of `plan`, sign it, submit it,
    /// and return the bound receipt. Atomic by construction: Stellar lands
    /// every leg or none.
    pub async fn checkout_item(
        &self,
        buyer: &PubKey,
        item: &str,
        split: PaymentSplit,
    ) -> Result<Receipt, StellarError> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| StellarError::NotOurKey(buyer.to_hex()))?;
        if signer.pubkey().0 != buyer.0 {
            return Err(StellarError::NotOurKey(buyer.to_hex()));
        }

        let plan = self.plan(&split)?;
        let source = signer.pubkey();
        let issuer = keys::from_strkey(&self.cfg.usdc_issuer)?;
        let asset = tx::usdc_asset("USDC", issuer.0)?;

        let seq = self.rpc.load_sequence(&keys::to_strkey(&source)).await?;
        let next_seq = seq
            .checked_add(1)
            .ok_or_else(|| StellarError::Config("account sequence number overflow".into()))?;

        let memo = binding_reference(buyer, item, &plan.legs);
        let mut payment_legs = Vec::with_capacity(plan.legs.len());
        for leg in &plan.legs {
            let amount = i64::try_from(leg.amount).map_err(|_| {
                StellarError::Config("leg amount exceeds Stellar's i64 range".into())
            })?;
            payment_legs.push(tx::PaymentLeg::new(leg.wallet.0, asset.clone(), amount));
        }

        let fee = tx::total_fee(self.cfg.base_fee_stroops, payment_legs.len())?;
        let unsigned = tx::build_payment_transaction(
            source.0,
            &payment_legs,
            next_seq,
            self.cfg.base_fee_stroops,
            memo,
        )?;
        let net_id = tx::network_id(self.cfg.network.passphrase());
        let hash = tx::tx_hash(net_id, &unsigned)?;
        let sig = signer.sign(&hash);
        let env = tx::envelope(unsigned, source.0, sig.0)?;
        let env_b64 = tx::envelope_to_xdr_base64(&env)?;

        let submitted = self.rpc.submit_transaction(&env_b64).await?;
        if !submitted.successful {
            return Err(StellarError::Config(
                "stellar transaction failed on submission".into(),
            ));
        }
        if submitted.hash != hex::encode(hash) {
            return Err(StellarError::Config(
                "horizon-reported transaction hash does not match the locally computed hash".into(),
            ));
        }

        Ok(self.wrap_receipt(buyer, item, &plan, &submitted.hash, next_seq, fee, &sig))
    }

    #[allow(clippy::too_many_arguments)]
    fn wrap_receipt(
        &self,
        buyer: &PubKey,
        item: &str,
        plan: &Plan,
        tx_hash: &str,
        seq_num: i64,
        fee: u32,
        sig: &keys::Sig,
    ) -> Receipt {
        let reference = hex::encode(binding_reference(buyer, item, &plan.legs));
        let proof = RailProof {
            chain: "stellar".to_string(),
            tx_hash: tx_hash.to_string(),
            asset_issuer: self.cfg.usdc_issuer.clone(),
            buyer: keys::to_strkey(buyer),
            legs: plan
                .legs
                .iter()
                .map(|l| ProofLeg {
                    wallet: keys::to_strkey(&l.wallet),
                    amount: l.amount,
                    role: l.role.tag().to_string(),
                })
                .collect(),
            reference: reference.clone(),
            seq_num,
            fee,
            signature: hex::encode(sig.0),
        };
        let mut r = Receipt {
            buyer: *buyer,
            payouts: plan.payouts(),
            stewards_amount: plan.stewards_amount,
            total: plan.total,
            nonce: binding_reference(buyer, item, &plan.legs),
            rail_pubkey: self.rail.node_pubkey(),
            sig: Sig([0u8; 64]),
            binding: Some(ChainBinding {
                chain: "stellar".to_string(),
                tx_signature: tx_hash.to_string(),
                item: item.to_string(),
                mint: self.cfg.usdc_issuer.clone(),
                reference,
                rail_proof: serde_json::to_vec(&proof).unwrap_or_default(),
            }),
        };
        r.sig = self.rail.sign(&r.signing_bytes());
        r
    }

    // ── Verification ─────────────────────────────────────────────────────────

    /// The full ten-check verification, async, ported from
    /// `magnetite_solana_rail::SolanaPaymentRail::verify_async` — see this
    /// crate's module docs for the numbered mapping. **Every** error path
    /// means "do not grant". There is no fail-open branch anywhere below.
    async fn verify_async(
        &self,
        r: &Receipt,
        expect_item: Option<&str>,
    ) -> Result<(), StellarError> {
        let deny = |m: &str| StellarError::Config(m.to_string());

        // ── 1. the receipt carries a chain binding, and the chain is stellar ─
        let b = r
            .binding
            .as_ref()
            .ok_or_else(|| deny("receipt carries no chain binding"))?;
        if b.chain != "stellar" {
            return Err(StellarError::Config(format!("chain {:?}", b.chain)));
        }

        // ── 2. the claimed issuer equals the configured USDC issuer ─────────
        if b.mint != self.cfg.usdc_issuer {
            return Err(deny("claimed issuer is not the configured USDC issuer"));
        }

        let proof: RailProof =
            serde_json::from_slice(&b.rail_proof).map_err(|_| deny("rail proof does not parse"))?;
        if proof.chain != "stellar" || proof.asset_issuer != self.cfg.usdc_issuer {
            return Err(deny("rail proof names another chain or issuer"));
        }
        if proof.buyer != keys::to_strkey(&r.buyer) {
            return Err(deny("rail proof names another buyer"));
        }
        if proof.legs.len() != r.payouts.len()
            || proof
                .legs
                .iter()
                .zip(r.payouts.iter())
                .any(|(l, p)| l.wallet != keys::to_strkey(&p.wallet) || l.amount != p.amount)
        {
            return Err(deny("rail proof legs disagree with the receipt payouts"));
        }
        let legs: Vec<Leg> = proof
            .legs
            .iter()
            .zip(r.payouts.iter())
            .map(|(l, p)| Leg::new(p.wallet, p.amount, role_from_tag(&l.role)))
            .collect();

        let mut stewards_sum = 0u64;
        for l in legs.iter().filter(|l| l.role.is_stewards()) {
            if self.cfg.stewards != Some(l.wallet) {
                return Err(StellarError::Stewards(format!(
                    "receipt claims a stewards leg to {}, which is not this build's stewards \
                     address",
                    keys::to_strkey(&l.wallet)
                )));
            }
            stewards_sum = stewards_sum
                .checked_add(l.amount)
                .ok_or_else(|| deny("stewards legs overflow"))?;
        }
        if stewards_sum != r.stewards_amount {
            return Err(deny(
                "receipt's stewards amount is not the sum of its stewards legs",
            ));
        }

        // ── 3. the binding reference is the one derived from (buyer, item,
        //       legs) ──────────────────────────────────────────────────────
        let expected_ref = hex::encode(binding_reference(&r.buyer, &b.item, &legs));
        if b.reference != expected_ref || proof.reference != expected_ref {
            return Err(deny("binding reference does not match (buyer, item, legs)"));
        }
        if hex::encode(r.nonce) != expected_ref {
            return Err(deny("receipt nonce is not the binding reference"));
        }

        // ── 4. ...and it is the item the CALLER is asking about ─────────────
        if let Some(item) = expect_item {
            if b.item != item {
                return Err(StellarError::Config(format!(
                    "receipt is bound to item {:?}, not {:?}",
                    b.item, item
                )));
            }
        }

        // ── 5. payouts sum EXACTLY to the total, the rail's own
        //       self-consistency signature is intact, AND — strengthened
        //       relative to the Solana port — the transaction is rebuilt
        //       from the proof's own scalar fields and its Ed25519 signature
        //       is verified OFFLINE, cryptographically, against the claimed
        //       source. See this crate's module docs.
        let sum = r
            .payouts
            .iter()
            .try_fold(0u64, |a, p| a.checked_add(p.amount))
            .ok_or_else(|| deny("payouts overflow"))?;
        if sum != r.total {
            return Err(deny("payouts do not sum exactly to total"));
        }
        if r.stewards_amount > r.total {
            return Err(deny("stewards component exceeds the total"));
        }
        if !self.rail.verify(&r.rail_pubkey, &r.signing_bytes(), &r.sig) {
            return Err(deny("receipt signature invalid"));
        }
        let source_pk = keys::from_strkey(&proof.buyer)?;
        if source_pk.0 != r.buyer.0 {
            return Err(deny(
                "rail proof's buyer strkey does not decode to the receipt buyer",
            ));
        }
        let issuer_pk = issuer_pk_from_config(self)?;
        let asset = tx::usdc_asset("USDC", issuer_pk.0)?;
        let mut rebuilt_legs = Vec::with_capacity(legs.len());
        for leg in &legs {
            let amount = i64::try_from(leg.amount)
                .map_err(|_| deny("leg amount exceeds Stellar's i64 range"))?;
            rebuilt_legs.push(tx::PaymentLeg::new(leg.wallet.0, asset.clone(), amount));
        }
        let memo_bytes = hex::decode(&proof.reference).map_err(|_| deny("reference is not hex"))?;
        let memo: [u8; 32] = memo_bytes
            .try_into()
            .map_err(|_| deny("reference is not 32 bytes"))?;
        let rebuilt = tx::build_payment_transaction(
            source_pk.0,
            &rebuilt_legs,
            proof.seq_num,
            proof.fee / u32::try_from(rebuilt_legs.len().max(1)).unwrap_or(1),
            memo,
        )
        .map_err(|_| deny("proof does not reconstruct into a valid transaction"))?;
        let net_id = tx::network_id(self.cfg.network.passphrase());
        let recomputed_hash = tx::tx_hash(net_id, &rebuilt)?;
        if hex::encode(recomputed_hash) != proof.tx_hash {
            return Err(deny(
                "proof's own scalar fields do not re-hash to its claimed tx_hash \
                 (self-inconsistent proof)",
            ));
        }
        let sig_bytes: [u8; 64] = hex::decode(&proof.signature)
            .map_err(|_| deny("signature is not hex"))?
            .try_into()
            .map_err(|_| deny("signature is not 64 bytes"))?;
        if !keys::Keypair::verify(&source_pk, &recomputed_hash, &keys::Sig(sig_bytes)) {
            return Err(deny(
                "claimed signature is not a genuine Ed25519 signature by the claimed source over \
                 the transaction hash",
            ));
        }

        // ── 6. the transaction is known to Horizon ──────────────────────────
        let record = match self.rpc.get_transaction(&proof.tx_hash).await {
            Ok(Some(t)) => t,
            Ok(None) => return Err(deny("horizon has never heard of this transaction hash")),
            Err(e) => return Err(e),
        };

        // ── 7. it landed successfully ────────────────────────────────────
        if !record.successful {
            return Err(deny("transaction failed on chain"));
        }

        let env = tx::envelope_from_xdr_base64(&record.envelope_xdr)
            .map_err(|_| deny("horizon returned an envelope that does not decode"))?;
        let decoded = tx::decode_payments(&env)
            .map_err(|_| deny("horizon's envelope is not the payment shape this rail builds"))?;

        // ── 8. the buyer is the transaction's own source account ───────────
        if decoded.source_pk != r.buyer.0 {
            return Err(deny("buyer is not the source account of this transaction"));
        }

        // ── 9. the on-chain memo is EXACTLY the derived binding ─────────────
        if decoded.memo != memo {
            return Err(deny("on-chain memo is not the derived binding"));
        }

        // ── 10. the money: every decoded PAYMENT operation, in order, pays
        //        exactly the wallet and amount a leg claims, in the
        //        configured USDC asset, with no extra/missing operation.
        if decoded.legs.len() != legs.len() {
            return Err(StellarLegCountMismatch(decoded.legs.len(), legs.len()).into());
        }
        for (i, (d, want)) in decoded.legs.iter().zip(legs.iter()).enumerate() {
            let want_amount = i64::try_from(want.amount)
                .map_err(|_| deny("leg amount exceeds Stellar's i64 range"))?;
            if d.dest_pk != want.wallet.0 {
                return Err(StellarError::Config(format!(
                    "operation {i}: on-chain destination does not match the claimed leg"
                )));
            }
            if d.amount != want_amount {
                return Err(StellarError::Config(format!(
                    "operation {i}: on-chain amount does not match the claimed leg"
                )));
            }
            if !tx::asset_is(&d.asset, "USDC", issuer_pk.0) {
                return Err(StellarError::Config(format!(
                    "operation {i}: on-chain asset is not the configured USDC issuer"
                )));
            }
        }

        Ok(())
    }

    /// Drive [`Self::verify_async`] from a synchronous caller.
    /// (`PaymentRail::verify_receipt` is sync by seam contract). Builds a
    /// fresh current-thread runtime on its own OS thread so this never
    /// panics when called from inside an already-running async context.
    fn verify_blocking(&self, r: &Receipt, item: Option<String>) -> bool {
        let rpc_result = std::thread::scope(|s| {
            s.spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| StellarError::Config(format!("runtime: {e}")))
                    .map(|rt| rt.block_on(self.verify_async(r, item.as_deref())))
            })
            .join()
        });
        match rpc_result {
            Ok(Ok(Ok(()))) => true,
            Ok(Ok(Err(_))) | Ok(Err(_)) | Err(_) => false,
        }
    }
}

/// Decode the configured USDC issuer's raw bytes — a small helper so
/// `verify_async` does not repeat the StrKey decode inline.
fn issuer_pk_from_config(rail: &StellarPaymentRail) -> Result<PubKey, StellarError> {
    keys::from_strkey(&rail.cfg.usdc_issuer)
}

/// Named so `verify_async`'s "leg count mismatch" `.into()` reads clearly at
/// the call site — an unaccounted extra recipient OR a missing one, either
/// direction, is the same refusal: the chain did not pay exactly the claimed
/// set of legs.
struct StellarLegCountMismatch(usize, usize);
impl From<StellarLegCountMismatch> for StellarError {
    fn from(m: StellarLegCountMismatch) -> Self {
        StellarError::Config(format!(
            "on-chain operation count ({}) does not match the claimed leg count ({}): an \
             unaccounted party either gained or is missing",
            m.0, m.1
        ))
    }
}

#[async_trait::async_trait]
impl PaymentRail for StellarPaymentRail {
    /// Unbound checkout. This rail REQUIRES an item binding, so this returns
    /// a receipt with no binding — which by construction fails verification.
    /// Use [`StellarPaymentRail::checkout_item`] / [`Self::checkout_for_item`].
    async fn checkout(&self, buyer: &PubKey, split: PaymentSplit) -> Receipt {
        let (payouts, stewards_amount, total) = match self.plan(&split) {
            Ok(p) => (p.payouts(), p.stewards_amount, p.total),
            Err(_) => (Vec::new(), 0, 0),
        };
        let mut r = Receipt {
            buyer: *buyer,
            payouts,
            stewards_amount,
            total,
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
        Err(StellarError::Unsupported("payment channels").into())
    }

    async fn escrow(&self, _terms: WagerTerms) -> Result<Escrow, PaymentError> {
        Err(StellarError::Unsupported("wager escrow").into())
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
