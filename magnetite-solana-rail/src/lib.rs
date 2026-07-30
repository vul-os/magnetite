//! Seam §3.6 — a **real** `PaymentRail`: SPL USDC on Solana.
//!
//! A standalone crate (not a `magnetite-seams` feature — see this crate's
//! `Cargo.toml` for why) implementing [`magnetite_seams::payment::PaymentRail`]
//! on top of the sibling `patala` repo's `patala-solana` crate. The offline
//! [`magnetite_seams::payment::MockPaymentRail`] stays the default rail
//! everywhere else; nothing in `magnetite-seams` or `backend`'s default build
//! depends on this crate existing.
//!
//! # HONEST STATUS — READ FIRST
//!
//! **No payment has ever settled through this rail.** Its transaction
//! *construction* path (message serialization, associated-token-account
//! derivation, instruction encoding) has **never run against a validator** —
//! not devnet, not a local `solana-test-validator`, not mainnet. Everything
//! below is exercised offline against a scripted fake RPC only. Nothing here is
//! chain-verified. Do not point it at mainnet before someone completes a devnet
//! round-trip (`ALIGNMENT.md` §7, Phase 3 item 12).
//!
//! # What this crate owns, and what patala owns
//!
//! patala owns the **cryptography and chain primitives**: Ed25519 keys and
//! signing (`patala_solana::keys`), base58, program-derived and
//! associated-token-account derivation, SPL `TransferChecked` and Memo
//! instruction encoding, legacy message serialization, the wire transaction
//! format, the JSON-RPC client and the `SolanaRpc` trait
//! (`patala_solana::tx`, `::rpc`), and the cluster / commitment / mint config.
//! None of that is reimplemented here.
//!
//! This crate owns **the multi-leg split**, because patala cannot express it:
//! `patala_core::PaymentRail::charge` moves money to exactly ONE destination
//! and `::verify` checks exactly one destination's balance delta plus "no
//! unaccounted party" (`PATALA.md` §3). A magnetite
//! [`PaymentSplit`](magnetite_seams::payment::PaymentSplit) is a list of legs
//! that must all land or none. So `checkout_item` here composes patala's
//! instruction primitives into ONE transaction with one `TransferChecked` per
//! payable leg — atomic by construction, Solana lands every leg or none — and
//! `verify_async` performs the ten fail-closed checks over an arbitrary number
//! of recipients.
//!
//! The earlier version of this crate delegated whole `charge`/`verify` calls to
//! `patala_solana::SolanaRail` and **refused** any split with more than one
//! non-zero leg (`MultiPartySplit`). That refusal is gone because the
//! capability is now real, not because the constraint was wished away: patala's
//! own single-destination `charge`/`verify` are no longer on this path, and
//! when patala grows a multi-destination seam this crate should delegate again.
//!
//! # Money math
//!
//! USDC has 6 decimals. Every amount here is an integer count of smallest
//! units (micro-USDC). **There is no floating point anywhere in the money
//! path**, no percentage and no basis-points rate: a split arrives with every
//! leg's absolute amount already decided by whoever's money it is.
//! [`SolanaPaymentRail::plan`] refuses any split it cannot sum exactly, and
//! the legs it emits sum exactly to the total the buyer is charged.
//!
//! # Keys
//!
//! The signing key is read from `SOLANA_KEYPAIR_PATH` / `SOLANA_KEYPAIR` by
//! [`patala_solana::keys::Keypair::from_env`] — magnetite has no copy of that
//! loader. It is never logged, never serialized and never written anywhere.

use std::sync::Arc;

use magnetite_seams::identity::{Identity, PubKey, RawKeypairAuth, Sig};
use magnetite_seams::payment::{
    split_digest, ChainBinding, Channel, Escrow, Leg, PayOut, PaymentError, PaymentRail,
    PaymentSplit, Receipt, Role, WagerTerms,
};

/// Re-exported so callers (e.g. `backend`) that need lower-level
/// `patala_solana`/`patala_core` types (RPC client, key loading, mint
/// constants, ...) can reach them through this ONE crate, rather than taking
/// their own direct dependency on the sibling `patala` repo.
pub use patala_core;
pub use patala_solana;
use patala_solana::{keys::Keypair, rpc::SolanaRpc, tx};
pub use patala_solana::{Cluster, Commitment};

/// Smallest leg this rail will pay, in micro-USDC.
///
/// **1_000 micro-USDC = 0.001 USDC = one tenth of a cent** (USDC has 6
/// decimals).
///
/// # Why this number
///
/// A leg must be worth at least what it costs to move. On Solana the payer
/// covers two costs that a leg does not shrink below:
///
/// * the transaction's base fee — 5_000 lamports per signature, on the order of
///   a tenth of a cent at any SOL price this project has been priced against;
/// * one-off per recipient, the rent to create the destination's associated
///   token account — ~0.00204 SOL, two orders of magnitude more than that.
///
/// A leg under a tenth of a cent therefore costs the buyer more to send than
/// the recipient receives. It is negative-sum, so it is not sent.
///
/// It is a **fixed integer chosen once**, deliberately not a live
/// currency conversion: converting would need a price oracle (a party who can
/// lie) and floating point (which has no place in the money path). It sits
/// three orders of magnitude above the smallest representable unit, so ordinary
/// prices are nowhere near it.
///
/// # Skipped, never fatal
///
/// A leg below this is **skipped**: the instruction is not emitted, the buyer
/// is not charged for it, and the total shrinks by exactly that amount. A tiny
/// voluntary [`Role::Stewards`] contribution can therefore never break a
/// purchase. What is NOT tolerated is a plan with no payable leg at all — that
/// is refused ([`SolanaError::NoPayableLeg`]), so a dust-only split can never
/// become a free entitlement.
pub const DUST_FLOOR_MICRO_USDC: u64 = 1_000;

/// Hard ceiling on a legacy Solana transaction's wire size, in bytes (the
/// network's IPv6 MTU-derived packet limit). Checked against the ACTUAL
/// serialized bytes rather than a guessed leg count, so an over-large split is
/// refused here instead of being rejected by the cluster after the buyer
/// thought they had paid.
pub const MAX_TX_WIRE_BYTES: usize = 1232;

// ── The stewards destination ─────────────────────────────────────────────────

/// Where the voluntary contribution to magnetite's maintainers goes.
///
/// # Why this is not an environment variable
///
/// It used to be: `SOLANA_FEE_WALLET` → `SolanaConfig.fee_wallet`. That put the
/// destination of the "protocol fee" under the **node operator's** control, and
/// verification could not catch it: the ten checks below prove that the wallets
/// a receipt *claims* are the wallets the chain actually paid — not that they
/// are the right parties. An operator who set `SOLANA_FEE_WALLET` to their own
/// address produced receipts that verified perfectly while redirecting other
/// people's voluntary contribution to themselves.
///
/// So the address is **compiled in**, via [`COMPILED_IN`] — `option_env!` is
/// evaluated by the compiler, which puts the value in the binary's bytes. A
/// release publishes those bytes under a `SHA256SUMS` manifest plus a
/// provenance attestation (`scripts/release-checksums.sh`, `scripts/verify.sh`,
/// commit 363a26a), and `verify.sh` fails closed on any mismatch. Changing the
/// address therefore requires producing a *different binary*, whose digest does
/// not match the published manifest.
///
/// That is the intended difference in kind: **a fork changing it is expected
/// and visible; a host silently redirecting it is not possible.** Anyone may
/// build their own magnetite pointing the stewards leg wherever they like —
/// what they cannot do is do it while claiming to run the published release.
pub mod stewards {
    /// The stewards wallet baked in at **build** time from
    /// `MAGNETITE_STEWARDS_WALLET` (base58).
    ///
    /// `option_env!` is a compile-time macro: this is a constant in the
    /// compiled artifact and no runtime environment can change it.
    ///
    /// **`None` in this tree, and that is the honest state**: magnetite has no
    /// published stewards address yet, so no release sets this. While it is
    /// `None` a [`super::Role::Stewards`] leg cannot be paid at all — the rail
    /// refuses the plan rather than inventing a destination or quietly dropping
    /// the leg.
    pub const COMPILED_IN: Option<&str> = option_env!("MAGNETITE_STEWARDS_WALLET");

    /// Devnet/testnet/localnet-only runtime override.
    ///
    /// Exists so a developer can exercise the stewards path against a local
    /// validator without rebuilding. On `mainnet-beta` its mere presence is a
    /// **fatal misconfiguration** — see [`resolve`]. It is never ignored,
    /// because "ignored" is how an override becomes a silent fallback.
    pub const DEVNET_OVERRIDE_ENV: &str = "MAGNETITE_STEWARDS_WALLET_DEVNET";
}

/// Everything that can go wrong on this rail. Every variant is a *refusal*:
/// none of them ever results in an entitlement being granted.
#[derive(Debug, thiserror::Error)]
pub enum SolanaError {
    /// Misconfiguration — missing mint, bad RPC URL, unusable keypair, ...
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
    /// Every leg was below [`DUST_FLOOR_MICRO_USDC`], so there is nothing to
    /// pay. Refused rather than settled as a free purchase.
    #[error(
        "split has no payable leg: all {0} legs are below the {DUST_FLOOR_MICRO_USDC} micro-USDC \
         dust floor, so this purchase would move no money"
    )]
    NoPayableLeg(usize),
    /// A `Role::Stewards` leg named a wallet other than the compiled-in
    /// stewards address, or there is no compiled-in address to name.
    #[error("stewards leg refused: {0}")]
    Stewards(String),
    /// The transaction needed to pay every leg does not fit in one Solana
    /// transaction. Refused rather than truncated: dropping a leg would break
    /// the sum-exactness the whole design rests on.
    #[error(
        "a split with {legs} legs serializes to {bytes} bytes, over Solana's \
         {MAX_TX_WIRE_BYTES}-byte transaction limit; it cannot be paid atomically in ONE \
         transaction and this rail will not split it into several non-atomic ones"
    )]
    TooManyLegs {
        /// How many legs were payable.
        legs: usize,
        /// The serialized wire size that exceeded the limit.
        bytes: usize,
    },
    /// The underlying `patala-solana` primitives or RPC refused or failed.
    #[error("patala solana: {0}")]
    Patala(String),
}

impl From<SolanaError> for PaymentError {
    fn from(e: SolanaError) -> Self {
        match e {
            SolanaError::Unsupported(w) => PaymentError::Unsupported(w),
            other => PaymentError::Rail(other.to_string()),
        }
    }
}

impl From<patala_core::Error> for SolanaError {
    fn from(e: patala_core::Error) -> Self {
        SolanaError::Patala(e.to_string())
    }
}

impl From<patala_solana::SolanaError> for SolanaError {
    fn from(e: patala_solana::SolanaError) -> Self {
        SolanaError::Patala(e.to_string())
    }
}

/// Magnetite-level rail configuration: patala's own rail config, plus the one
/// thing `patala_core`'s seam does not model — the stewards destination.
///
/// Build it with [`SolanaConfig::resolve`], which is **fallible on purpose**:
/// resolving the stewards address is where a misconfiguration must become
/// fatal, at startup, rather than at the first checkout.
#[derive(Clone, Debug)]
pub struct SolanaConfig {
    /// Everything patala's Solana primitives need (RPC URL, cluster,
    /// commitment, USDC mint).
    pub inner: patala_solana::SolanaConfig,
    /// The resolved stewards wallet, or `None` when this build has no
    /// compiled-in address (in which case a stewards leg is refused).
    ///
    /// Private-by-convention: set it through [`SolanaConfig::resolve`] so the
    /// mainnet rule below cannot be bypassed by construction. It is `pub` only
    /// because tests in this crate build configs directly.
    pub stewards: Option<PubKey>,
}

impl SolanaConfig {
    /// Resolve the stewards destination for `inner`'s cluster, failing closed.
    ///
    /// Precedence, and the rule that makes it non-forgeable:
    ///
    /// 1. `MAGNETITE_STEWARDS_WALLET_DEVNET` is set **and the cluster is
    ///    mainnet** → **hard error.** Not ignored, not warned about: a
    ///    process that tolerated it would be one env var away from
    ///    operator-controlled redirection on real money.
    /// 2. `MAGNETITE_STEWARDS_WALLET_DEVNET` is set on devnet / testnet /
    ///    localnet → use it (unparseable ⇒ error).
    /// 3. otherwise [`stewards::COMPILED_IN`] (unparseable ⇒ error).
    /// 4. neither ⇒ `Ok(None)`: this build has no stewards address, and a
    ///    stewards leg will be refused. Absence is never a silent default to
    ///    some other wallet.
    pub fn resolve(inner: patala_solana::SolanaConfig) -> Result<Self, SolanaError> {
        let stewards = resolve_stewards(inner.cluster)?;
        Ok(Self { inner, stewards })
    }
}

/// See [`SolanaConfig::resolve`]. Separate so tests can drive it directly.
///
/// # Three states, not two
///
/// Modelled on the same shape as beepbite's
/// `backend/internal/payments/gateway.go`, which distinguishes:
///
/// | state | here | outcome |
/// |---|---|---|
/// | nothing configured | no override, no [`stewards::COMPILED_IN`] | `Ok(None)` — **not an error.** No stewards leg can be paid, and that is a legitimate configuration, not a failure. |
/// | configured and honourable | compiled-in, or an override on a non-mainnet cluster | `Ok(Some(pk))` |
/// | configured but this build/cluster cannot honour it | an override on mainnet, or an unparseable address | `Err` — **loud, never silently ignored.** |
///
/// The third row is the one that matters: "set but quietly disregarded" is how
/// an override becomes a fallback nobody notices.
pub fn resolve_stewards(cluster: Cluster) -> Result<Option<PubKey>, SolanaError> {
    let override_env = std::env::var(stewards::DEVNET_OVERRIDE_ENV).ok();
    if let Some(raw) = override_env {
        if cluster.is_mainnet() {
            return Err(SolanaError::Stewards(format!(
                "{} is set, but SOLANA_CLUSTER is mainnet-beta. The stewards destination on \
                 mainnet comes from the signed release ONLY (compiled in) — a node operator \
                 must not be able to redirect other people's voluntary contribution. Unset \
                 the override, or use devnet/testnet/localnet. Refusing to start.",
                stewards::DEVNET_OVERRIDE_ENV
            )));
        }
        let pk = tx::pubkey_from_base58(raw.trim()).map_err(|e| {
            SolanaError::Stewards(format!(
                "{} is not a valid solana address: {e}",
                stewards::DEVNET_OVERRIDE_ENV
            ))
        })?;
        return Ok(Some(PubKey(pk.0)));
    }
    match stewards::COMPILED_IN {
        None => Ok(None),
        Some(raw) => {
            let pk = tx::pubkey_from_base58(raw.trim()).map_err(|e| {
                SolanaError::Stewards(format!(
                    "the compiled-in stewards address ({raw:?}, baked in from \
                     MAGNETITE_STEWARDS_WALLET at build time) is not a valid solana address: \
                     {e}. This is a build defect, not a runtime one."
                ))
            })?;
            Ok(Some(PubKey(pk.0)))
        }
    }
}

/// A concrete, integer-exact plan for one checkout. Pure arithmetic — no chain,
/// no RPC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The legs that will actually be paid, in order — one SPL
    /// `TransferChecked` each. Sums EXACTLY to [`Plan::total`].
    pub legs: Vec<Leg>,
    /// Legs skipped for being below [`DUST_FLOOR_MICRO_USDC`]. Retained so the
    /// caller can see what was not charged; the buyer pays none of it.
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
/// Magnetite's OWN item **and distribution** ↔ receipt consistency hash. It is
/// carried in the receipt AND as the on-chain memo, so the chain record itself
/// pins which item was bought and how the money was divided.
///
/// The `v1` form committed to `(buyer, item)` only, which left the distribution
/// unbound: two different splits of the same total produced the same reference,
/// and a receipt could be re-pointed from one to the other. `v2` folds
/// [`split_digest`] in, so it cannot.
pub fn binding_reference(buyer: &PubKey, item: &str, legs: &[Leg]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"magnetite-pay-v2");
    h.update(&buyer.0);
    h.update(&(item.len() as u64).to_le_bytes());
    h.update(item.as_bytes());
    h.update(&split_digest(legs));
    *h.finalize().as_bytes()
}

/// The exact memo string a bound transaction must carry.
///
/// Magnetite's own domain tag, deliberately not patala's `patala:v1:` one:
/// patala's memo commits to `(payer, reference-string)` and has nowhere to put
/// a leg list, and a receipt from one system must never be mistakable for a
/// receipt from the other.
pub fn binding_memo(buyer: &PubKey, item: &str, legs: &[Leg]) -> String {
    format!(
        "magnetite:v2:{}",
        hex::encode(binding_reference(buyer, item, legs))
    )
}

fn patala_pubkey(pk: &PubKey) -> patala_solana::keys::PubKey {
    patala_solana::keys::PubKey(pk.0)
}

fn b58(pk: &PubKey) -> String {
    tx::pubkey_to_base58(&patala_pubkey(pk))
}

/// The rail-specific data carried in [`ChainBinding::rail_proof`] (JSON).
/// Opaque to `magnetite-seams`; re-read here at verification time and checked
/// against current chain state, never trusted on its own.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RailProof {
    /// Always `"solana"` (checked, not assumed).
    chain: String,
    /// On-chain transaction signature (base58).
    tx_signature: String,
    /// The token mint transferred (base58).
    mint: String,
    /// The paying wallet (base58) — also the signing identity.
    buyer: String,
    /// Every paid leg, in order: `(base58 wallet, amount, role tag)`.
    ///
    /// The role tags live here rather than in [`Receipt::payouts`] (which is
    /// `{wallet, amount}` and matches existing `payment_receipts.payouts` rows)
    /// and they are **not** trusted: they are folded into
    /// [`binding_reference`], which must equal the receipt's reference, the
    /// receipt's nonce, and — the part no editor can forge — the memo already
    /// on chain. Editing a role tag changes the derived reference and the
    /// receipt stops verifying.
    legs: Vec<ProofLeg>,
    /// Hex [`binding_reference`], also carried on-chain as the memo.
    reference: String,
}

/// One leg as recorded in [`RailProof`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ProofLeg {
    /// Destination wallet, base58.
    wallet: String,
    /// Amount in micro-USDC.
    amount: u64,
    /// [`Role::tag`] of the leg.
    role: String,
}

/// Map a [`Role::tag`] back to a [`Role`].
///
/// The three canonical tags map to their variants; anything else is
/// [`Role::Other`]. Note that `Other("developer")` and `Developer` therefore
/// share a tag and hash identically — a labelling collision, not a security
/// one: what moves money is the wallet and the amount, both of which are
/// checked against the chain independently.
fn role_from_tag(tag: &str) -> Role {
    match tag {
        "developer" => Role::Developer,
        "operator" => Role::Operator,
        "stewards" => Role::Stewards,
        other => Role::Other(other.to_string()),
    }
}

/// SPL-USDC-on-Solana payment rail — magnetite's split, patala's crypto.
pub struct SolanaPaymentRail {
    cfg: SolanaConfig,
    rpc: Arc<dyn SolanaRpc>,
    /// The wallet this rail can spend from — absent for a verify-only rail (a
    /// server that only ever checks receipts, never pays).
    signer: Option<Keypair>,
    /// Key that signs magnetite's OWN receipt wrapper (a self-consistency
    /// marker, NOT the security boundary — chain state is).
    rail: RawKeypairAuth,
}

impl SolanaPaymentRail {
    /// Build a rail over an arbitrary RPC implementation (unit tests pass a
    /// fake; production passes [`patala_solana::rpc::HttpRpc`]). No signer —
    /// verify-only until [`Self::with_signer`].
    pub fn new(cfg: SolanaConfig, rpc: Arc<dyn SolanaRpc>) -> Self {
        Self {
            cfg,
            rpc,
            signer: None,
            rail: RawKeypairAuth::from_seed(*blake3::hash(b"magnetite-solana-rail").as_bytes()),
        }
    }

    /// Attach a signing key so this rail can submit transactions itself. The
    /// same key is the rail's signing identity and, since Solana addresses are
    /// Ed25519 public keys, the wallet the funds move from.
    pub fn with_signer(mut self, signer: Keypair) -> Self {
        self.signer = Some(signer);
        self
    }

    /// Build a rail whose signer (if any) is loaded from
    /// `SOLANA_KEYPAIR_PATH`/`SOLANA_KEYPAIR` (see
    /// [`patala_solana::keys::Keypair::from_env`]).
    pub fn from_env(cfg: SolanaConfig, rpc: Arc<dyn SolanaRpc>) -> Result<Self, SolanaError> {
        let rail = Self::new(cfg, rpc);
        Ok(match Keypair::from_env()? {
            Some(k) => rail.with_signer(k),
            None => rail,
        })
    }

    /// The rail's receipt-signing public key (magnetite's own bookkeeping key).
    pub fn rail_pubkey(&self) -> PubKey {
        self.rail.node_pubkey()
    }

    /// The configuration (read-only).
    pub fn config(&self) -> &SolanaConfig {
        &self.cfg
    }

    /// The wallet this rail can sign for, if any.
    pub fn signer_pubkey(&self) -> Option<PubKey> {
        self.signer.as_ref().map(|s| PubKey(s.pubkey().0))
    }

    /// The stewards destination this rail will pay, if this build has one.
    pub fn stewards_wallet(&self) -> Option<PubKey> {
        self.cfg.stewards
    }

    /// Integer-exact plan. Pure arithmetic, no chain, no floats, no rates.
    ///
    /// Refuses — never silently adjusts — when:
    ///
    /// * the requested legs overflow `u64` (a total that cannot be stated
    ///   honestly cannot be charged);
    /// * a [`Role::Stewards`] leg names a wallet that is not this build's
    ///   compiled-in stewards address, or there is no such address. **This is
    ///   the check that takes the destination away from the node operator**:
    ///   labelling a leg `Stewards` does not let a caller choose where it goes;
    /// * every leg is dust, so nothing would move.
    ///
    /// Dust legs below [`DUST_FLOOR_MICRO_USDC`] are skipped (recorded in
    /// [`Plan::skipped`]) and the total shrinks by exactly their amount, so the
    /// paid legs still sum exactly to what the buyer is charged.
    pub fn plan(&self, split: &PaymentSplit) -> Result<Plan, SolanaError> {
        // A requested total that overflows is refused outright: not "whatever
        // survives the dust floor summed fine, so charge that".
        split.checked_total().ok_or_else(|| {
            SolanaError::Config("split legs overflow u64; refusing to state a total".into())
        })?;

        // The stewards destination is not the caller's to choose.
        for leg in split.legs.iter().filter(|l| l.role.is_stewards()) {
            match self.cfg.stewards {
                None => {
                    return Err(SolanaError::Stewards(format!(
                        "this build has no compiled-in stewards address (MAGNETITE_STEWARDS_WALLET \
                         was not set when it was built), so a stewards leg cannot be paid. \
                         Refusing rather than paying the caller-supplied wallet {} or silently \
                         dropping the leg.",
                        b58(&leg.wallet)
                    )))
                }
                Some(want) if want != leg.wallet => {
                    return Err(SolanaError::Stewards(format!(
                        "a stewards leg names {}, but this build's stewards address is {}. The \
                         destination comes from the signed release, never from the caller or the \
                         node's environment.",
                        b58(&leg.wallet),
                        b58(&want)
                    )))
                }
                Some(_) => {}
            }
        }

        let (legs, skipped) = split.partition_at(DUST_FLOOR_MICRO_USDC);
        if legs.is_empty() {
            return Err(SolanaError::NoPayableLeg(split.legs.len()));
        }

        let paid = PaymentSplit::new(legs);
        let total = paid.checked_total().ok_or_else(|| {
            SolanaError::Config("payable legs overflow u64; refusing to state a total".into())
        })?;
        // Re-derive independently. A plan whose parts do not sum EXACTLY to the
        // total it claims is refused, never rounded into agreement.
        let sum = paid
            .legs
            .iter()
            .try_fold(0u64, |a, l| a.checked_add(l.amount))
            .ok_or_else(|| SolanaError::Config("payable legs overflow u64".into()))?;
        if sum != total {
            return Err(SolanaError::Config(format!(
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
    /// and return the bound receipt.
    ///
    /// The transaction carries an SPL Memo with [`binding_memo`] plus one SPL
    /// `TransferChecked` per leg. Because it is a single transaction, the split
    /// is atomic **by construction**: Solana lands every leg or none. No custom
    /// on-chain program is involved.
    ///
    /// Only possible when this process holds `buyer`'s key; otherwise
    /// [`SolanaError::NotOurKey`], because the rail is non-custodial and will
    /// not pretend to spend money it cannot move.
    pub async fn checkout_item(
        &self,
        buyer: &PubKey,
        item: &str,
        split: PaymentSplit,
    ) -> Result<Receipt, SolanaError> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| SolanaError::NotOurKey(buyer.to_hex()))?;
        if signer.pubkey().0 != buyer.0 {
            return Err(SolanaError::NotOurKey(buyer.to_hex()));
        }

        let plan = self.plan(&split)?;
        let payer = signer.pubkey();
        let mint = self.cfg.inner.usdc_mint;

        let blockhash = self
            .rpc
            .get_latest_blockhash(self.cfg.inner.commitment.as_str())
            .await?;
        let bh: [u8; 32] = bs58_32(&blockhash)
            .ok_or_else(|| SolanaError::Patala("blockhash is not 32 bytes".into()))?;

        let source = tx::associated_token_address(&payer, &mint)?;
        // The memo commits to the FULL paid distribution, so the on-chain
        // record — not just magnetite's own signature over the receipt — pins
        // which legs this payment was for.
        let memo_str = binding_memo(buyer, item, &plan.legs);
        let mut ixs = vec![tx::memo(payer, &memo_str)];
        for leg in &plan.legs {
            let dest_ata = tx::associated_token_address(&patala_pubkey(&leg.wallet), &mint)?;
            ixs.push(tx::transfer_checked(
                source,
                mint,
                dest_ata,
                payer,
                leg.amount,
                tx::USDC_DECIMALS,
            ));
        }

        let message = tx::serialize_message(&payer, &ixs, &bh);
        let wire = tx::wire_transaction(&signer.sign(&message).0, &message);
        // Refuse an over-large transaction HERE. Truncating legs would break
        // sum-exactness; letting the cluster reject it would leave a buyer who
        // believes they paid.
        if wire.len() > MAX_TX_WIRE_BYTES {
            return Err(SolanaError::TooManyLegs {
                legs: plan.legs.len(),
                bytes: wire.len(),
            });
        }
        let tx_signature = self.rpc.send_transaction(&tx::b64(&wire)).await?;

        Ok(self.wrap_receipt(buyer, item, &plan, tx_signature))
    }

    fn wrap_receipt(
        &self,
        buyer: &PubKey,
        item: &str,
        plan: &Plan,
        tx_signature: String,
    ) -> Receipt {
        let reference = hex::encode(binding_reference(buyer, item, &plan.legs));
        let proof = RailProof {
            chain: "solana".to_string(),
            tx_signature: tx_signature.clone(),
            mint: self.cfg.inner.mint_base58(),
            buyer: b58(buyer),
            legs: plan
                .legs
                .iter()
                .map(|l| ProofLeg {
                    wallet: b58(&l.wallet),
                    amount: l.amount,
                    role: l.role.tag().to_string(),
                })
                .collect(),
            reference: reference.clone(),
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
                chain: "solana".to_string(),
                tx_signature,
                item: item.to_string(),
                mint: self.cfg.inner.mint_base58(),
                reference,
                // `serde_json::to_vec` on this shape cannot fail; an empty
                // proof would fail verification anyway, which is the safe way
                // for it to be wrong.
                rail_proof: serde_json::to_vec(&proof).unwrap_or_default(),
            }),
        };
        r.sig = self.rail.sign(&r.signing_bytes());
        r
    }

    // ── Verification ─────────────────────────────────────────────────────────

    /// The full ten-check verification, async. **Every** error path means "do
    /// not grant". There is no fail-open branch anywhere below.
    async fn verify_async(
        &self,
        r: &Receipt,
        expect_item: Option<&str>,
    ) -> Result<(), SolanaError> {
        let deny = |m: &str| SolanaError::Config(m.to_string());

        // ── 1. the receipt carries a chain binding, and the chain is solana ──
        let b = r
            .binding
            .as_ref()
            .ok_or_else(|| deny("receipt carries no chain binding"))?;
        if b.chain != "solana" {
            return Err(SolanaError::Config(format!("chain {:?}", b.chain)));
        }

        // ── 2. the claimed mint equals the configured USDC mint ─────────────
        if b.mint != self.cfg.inner.mint_base58() {
            return Err(deny("claimed mint is not the configured USDC mint"));
        }

        // The rail proof is a CLAIM. Parse it, then check every field of it
        // against the receipt and the chain.
        let proof: RailProof =
            serde_json::from_slice(&b.rail_proof).map_err(|_| deny("rail proof does not parse"))?;
        if proof.chain != "solana" || proof.mint != self.cfg.inner.mint_base58() {
            return Err(deny("rail proof names another chain or mint"));
        }
        if proof.buyer != b58(&r.buyer) {
            return Err(deny("rail proof names another buyer"));
        }
        // The proof's leg list must be exactly the receipt's payouts, in order:
        // one claim, not two that can disagree.
        if proof.legs.len() != r.payouts.len()
            || proof
                .legs
                .iter()
                .zip(r.payouts.iter())
                .any(|(l, p)| l.wallet != b58(&p.wallet) || l.amount != p.amount)
        {
            return Err(deny("rail proof legs disagree with the receipt payouts"));
        }
        let legs: Vec<Leg> = proof
            .legs
            .iter()
            .zip(r.payouts.iter())
            .map(|(l, p)| Leg::new(p.wallet, p.amount, role_from_tag(&l.role)))
            .collect();

        // A leg CLAIMING to be a stewards contribution must pay this build's
        // compiled-in stewards address — the same rule `plan` enforces when
        // charging, re-checked here so a receipt minted by some other build
        // cannot claim a stewards leg to an arbitrary wallet. And the receipt's
        // stewards bookkeeping must equal what those legs actually paid.
        let mut stewards_sum = 0u64;
        for l in legs.iter().filter(|l| l.role.is_stewards()) {
            if self.cfg.stewards != Some(l.wallet) {
                return Err(SolanaError::Stewards(format!(
                    "receipt claims a stewards leg to {}, which is not this build's stewards \
                     address",
                    b58(&l.wallet)
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
        //       legs) — so a receipt cannot be re-pointed at another item OR
        //       at a different distribution of the same total by editing a
        //       field. Magnetite-local, no chain needed. Note that the role
        //       tags are inside this digest, so they are as tamper-evident as
        //       the wallets and amounts (and check 9 anchors the whole thing to
        //       the memo already on chain).
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
                return Err(SolanaError::Config(format!(
                    "receipt is bound to item {:?}, not {:?}",
                    b.item, item
                )));
            }
        }

        // ── 5. payouts sum EXACTLY to the total, and the rail signature is
        //       intact (cheap, local — a self-consistency marker, NOT the
        //       security boundary; chain state below is).
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
        if !<RawKeypairAuth as Identity>::verify(&r.rail_pubkey, &r.signing_bytes(), &r.sig) {
            return Err(deny("receipt signature invalid"));
        }

        // ── 6. the transaction is known to the cluster AT the configured
        //       commitment. RPC unreachable / error propagates as Err — an
        //       operational failure to even check, never "verified".
        let txn = match self
            .rpc
            .get_transaction(&proof.tx_signature, self.cfg.inner.commitment.as_str())
            .await
        {
            Ok(Some(t)) => t,
            Ok(None) => return Err(deny("cluster has never heard of this signature")),
            Err(e) => return Err(e.into()),
        };
        // `processed` can be rolled back and is rejected outright: patala's
        // `Commitment` only models `confirmed`/`finalized`, and anything the
        // RPC reports that is not good enough for the configured level denies.
        if let Some(status) = txn.get("confirmationStatus").and_then(|v| v.as_str()) {
            let ok = match self.cfg.inner.commitment {
                Commitment::Confirmed => status == "confirmed" || status == "finalized",
                Commitment::Finalized => status == "finalized",
            };
            if !ok {
                return Err(SolanaError::Config(format!(
                    "confirmationStatus {status:?} is below the configured commitment {:?}",
                    self.cfg.inner.commitment
                )));
            }
        }

        // ── 7. meta.err is null — a landed-but-failed transaction moved
        //       nothing. A MISSING meta.err field is malformed, and denies.
        match txn.get("meta").and_then(|m| m.get("err")) {
            None => return Err(deny("transaction has no meta.err field (malformed)")),
            Some(e) if !e.is_null() => return Err(deny("transaction failed on chain")),
            _ => {}
        }

        // ── 8. the buyer appears in accountKeys as a SIGNER ─────────────────
        let buyer_b58 = b58(&r.buyer);
        let signed = txn
            .get("transaction")
            .and_then(|t| t.get("message"))
            .and_then(|m| m.get("accountKeys"))
            .and_then(|k| k.as_array())
            .map(|keys| {
                keys.iter().any(|k| {
                    k.get("pubkey").and_then(|v| v.as_str()) == Some(buyer_b58.as_str())
                        && k.get("signer").and_then(|v| v.as_bool()) == Some(true)
                })
            })
            .unwrap_or(false);
        if !signed {
            return Err(deny("buyer is not a signer of this transaction"));
        }

        // ── 9. the on-chain memo is EXACTLY the derived binding ─────────────
        let want_memo = binding_memo(&r.buyer, &b.item, &legs);
        if !memo_matches(&txn, &want_memo) {
            return Err(deny("on-chain memo is not the derived binding"));
        }

        // ── 10. the money. Net token-balance deltas for the configured mint
        //        must be EXACTLY -total for the buyer and +amount for EACH
        //        claimed recipient, with NO unaccounted party gaining or
        //        losing that mint. Balance deltas (rather than reading
        //        instructions) mean a transfer cancelled out by a hidden
        //        reverse transfer in the same transaction cannot pass.
        //
        //        This is the check that has to survive an arbitrary leg count:
        //        recipients are aggregated by wallet first (the same wallet may
        //        legitimately appear in more than one leg — a co-developer who
        //        is also the operator), because the chain reports ONE net delta
        //        per owner and comparing per-leg would reject that honest case.
        let deltas = mint_deltas(&txn, &self.cfg.inner.mint_base58())?;
        let mut expected: Vec<(String, i128)> = Vec::new();
        let push = |who: String, amt: i128, into: &mut Vec<(String, i128)>| match into
            .iter_mut()
            .find(|(o, _)| *o == who)
        {
            Some(e) => e.1 += amt,
            None => into.push((who, amt)),
        };
        for p in &r.payouts {
            push(b58(&p.wallet), p.amount as i128, &mut expected);
        }
        push(buyer_b58.clone(), -(r.total as i128), &mut expected);

        for (who, want) in &expected {
            let got = deltas
                .iter()
                .find(|(o, _)| o == who)
                .map(|(_, d)| *d)
                .unwrap_or(0);
            if got != *want {
                return Err(SolanaError::Config(format!(
                    "on-chain delta for {who} is {got}, expected {want}"
                )));
            }
        }
        for (who, d) in &deltas {
            if *d != 0 && !expected.iter().any(|(e, _)| e == who) {
                return Err(SolanaError::Config(format!(
                    "unaccounted party {who} moved {d} of this mint in the transaction"
                )));
            }
        }

        Ok(())
    }

    /// Drive [`Self::verify_async`] from a synchronous caller
    /// (`PaymentRail::verify_receipt` is sync by seam contract). Builds a fresh
    /// current-thread runtime on its own OS thread so this never panics when
    /// called from inside an already-running async context (the backend calls
    /// `verify_receipt` from async handlers).
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

/// Decode a base58 32-byte value (a blockhash) without pulling in a second
/// base58 implementation: patala's `pubkey_from_base58` is exactly that check.
fn bs58_32(s: &str) -> Option<[u8; 32]> {
    tx::pubkey_from_base58(s).ok().map(|pk| pk.0)
}

/// Does any (top-level or inner) memo instruction carry exactly `want`?
fn memo_matches(txn: &serde_json::Value, want: &str) -> bool {
    fn scan(ixs: Option<&serde_json::Value>, want: &str) -> bool {
        ixs.and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().any(|ix| {
                    ix.get("program").and_then(|v| v.as_str()) == Some("spl-memo")
                        && ix.get("parsed").and_then(|v| v.as_str()) == Some(want)
                })
            })
            .unwrap_or(false)
    }
    let msg = txn.get("transaction").and_then(|t| t.get("message"));
    if scan(msg.and_then(|m| m.get("instructions")), want) {
        return true;
    }
    txn.get("meta")
        .and_then(|m| m.get("innerInstructions"))
        .and_then(|v| v.as_array())
        .map(|groups| groups.iter().any(|g| scan(g.get("instructions"), want)))
        .unwrap_or(false)
}

/// Net per-owner balance change for `mint`, in integer smallest units.
/// Malformed metadata is an error, never an assumed zero.
fn mint_deltas(txn: &serde_json::Value, mint: &str) -> Result<Vec<(String, i128)>, SolanaError> {
    let meta = txn
        .get("meta")
        .ok_or_else(|| SolanaError::Config("transaction has no meta".into()))?;
    let mut deltas: Vec<(String, i128)> = Vec::new();

    let mut apply = |list: Option<&serde_json::Value>, sign: i128| -> Result<(), SolanaError> {
        let empty: Vec<serde_json::Value> = Vec::new();
        for e in list.and_then(|v| v.as_array()).unwrap_or(&empty) {
            if e.get("mint").and_then(|v| v.as_str()) != Some(mint) {
                continue;
            }
            let owner = e
                .get("owner")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SolanaError::Config("token balance without owner".into()))?;
            let amount: i128 = e
                .get("uiTokenAmount")
                .and_then(|u| u.get("amount"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| SolanaError::Config("token balance without amount".into()))?
                .parse()
                .map_err(|_| SolanaError::Config("token amount is not an integer".into()))?;
            match deltas.iter_mut().find(|(o, _)| o == owner) {
                Some(d) => d.1 += sign * amount,
                None => deltas.push((owner.to_string(), sign * amount)),
            }
        }
        Ok(())
    };
    apply(meta.get("preTokenBalances"), -1)?;
    apply(meta.get("postTokenBalances"), 1)?;
    Ok(deltas)
}

#[async_trait::async_trait]
impl PaymentRail for SolanaPaymentRail {
    /// Unbound checkout. The Solana rail REQUIRES an item binding, so this
    /// returns a receipt with no binding — which by construction fails
    /// verification. Use [`SolanaPaymentRail::checkout_item`] /
    /// [`Self::checkout_for_item`].
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
