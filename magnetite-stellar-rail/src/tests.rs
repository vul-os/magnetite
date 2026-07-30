//! Integration tests for the Stellar rail — **fully offline**. The only
//! "network" is [`FakeRpc`], a scripted implementation of
//! [`crate::rpc::StellarRpc`].
//!
//! # The fake-RPC pattern, and why it is stronger than the Solana port's
//!
//! `magnetite-solana-rail`'s `FakeRpc` served whatever JSON a test hand-built
//! (`good_txn`/`txn_for`) — for the happy-path round-trip tests, that JSON was
//! constructed independently of the actual wire bytes `checkout_item`
//! produced and submitted, so those tests never proved the real
//! serialize/sign/wire-format path was self-consistent with what `verify`
//! expects (only that verify's *logic* accepts a hand-matching fixture).
//!
//! This fake, modelled on `patala/patala-stellar/src/tests.rs`'s (see this
//! crate's `Cargo.toml` for why that is a design copy, not a dependency):
//! `submit_transaction` **decodes and rebuilds** whatever `checkout_item`
//! actually handed it, using this crate's own pure functions
//! (`tx::decode_payments`, `tx::build_payment_transaction`, `tx::tx_hash`) —
//! not a hand-typed fixture — and `get_transaction` serves that back by hash.
//! So the happy-path tests below exercise the REAL wire encoding end to end:
//! sign → serialize → decode → re-hash → compare, all through this crate's
//! own code, offline. [`FakeRpc::seed`] is the escape hatch used only where a
//! test needs to model an *adversarial* chain state (Horizon disagreeing with
//! what a receipt claims) — mirroring both `patala-stellar`'s `.seed()` and
//! `magnetite-solana-rail`'s hand-built fixtures for that one purpose.
//!
//! What is NOT covered, and cannot be here: whether any of this is accepted
//! by real Horizon. **No payment has ever settled through this rail** (that
//! evidence belongs to the sibling `patala-stellar` crate, a different
//! implementation of the same wire format — see `src/lib.rs`).

use std::sync::Mutex;

use super::*;
use magnetite_seams::payment::{PaymentError, PaymentSplit, Role};

/// Scripted Horizon: `submit_transaction` independently decodes + rebuilds +
/// re-hashes exactly like a real Horizon/stellar-core would derive the
/// canonical transaction hash from the envelope it was handed — see the
/// module docs.
struct FakeRpc {
    passphrase: String,
    seq: i64,
    fail_load: bool,
    fail_submit: bool,
    fail_get: bool,
    lie_about_hash: bool,
    force_unsuccessful_on_get: bool,
    stored: Mutex<Option<(String, String)>>, // (hash_hex, envelope_b64)
    sent: Mutex<Vec<String>>,
}

impl FakeRpc {
    fn plain(passphrase: &str, seq: i64) -> Self {
        Self {
            passphrase: passphrase.to_string(),
            seq,
            fail_load: false,
            fail_submit: false,
            fail_get: false,
            lie_about_hash: false,
            force_unsuccessful_on_get: false,
            stored: Mutex::new(None),
            sent: Mutex::new(Vec::new()),
        }
    }
    fn new(passphrase: &str, seq: i64) -> Arc<Self> {
        Arc::new(Self::plain(passphrase, seq))
    }
    fn unconfirmed(passphrase: &str, seq: i64) -> Arc<Self> {
        Self::new(passphrase, seq)
    }
    fn failing_load(passphrase: &str) -> Arc<Self> {
        Arc::new(Self {
            fail_load: true,
            ..Self::plain(passphrase, 0)
        })
    }
    fn failing_submit(passphrase: &str, seq: i64) -> Arc<Self> {
        Arc::new(Self {
            fail_submit: true,
            ..Self::plain(passphrase, seq)
        })
    }
    fn failing_get(passphrase: &str, seq: i64) -> Arc<Self> {
        Arc::new(Self {
            fail_get: true,
            ..Self::plain(passphrase, seq)
        })
    }
    fn lying_about_hash(passphrase: &str, seq: i64) -> Arc<Self> {
        Arc::new(Self {
            lie_about_hash: true,
            ..Self::plain(passphrase, seq)
        })
    }
    fn unsuccessful_on_get(passphrase: &str, seq: i64) -> Arc<Self> {
        Arc::new(Self {
            force_unsuccessful_on_get: true,
            ..Self::plain(passphrase, seq)
        })
    }

    /// Directly seed the "chain" state, bypassing `submit_transaction` — used
    /// only to model an adversarial/dishonest Horizon reporting something
    /// other than what a receipt's own proof claims (checks 6-10). See the
    /// module docs.
    fn seed(&self, hash_hex: &str, envelope_b64: &str) {
        *self.stored.lock().unwrap() = Some((hash_hex.to_string(), envelope_b64.to_string()));
    }
}

#[async_trait::async_trait]
impl rpc::StellarRpc for FakeRpc {
    async fn load_sequence(&self, _account_strkey: &str) -> Result<i64, StellarError> {
        if self.fail_load {
            return Err(StellarError::Rpc("connection refused".into()));
        }
        Ok(self.seq)
    }

    async fn submit_transaction(
        &self,
        envelope_xdr_b64: &str,
    ) -> Result<rpc::SubmitResult, StellarError> {
        if self.fail_submit {
            return Err(StellarError::Rpc("connection refused".into()));
        }
        self.sent.lock().unwrap().push(envelope_xdr_b64.to_string());
        // Independently decode + rebuild + re-hash exactly like real Horizon
        // would derive the canonical hash from the envelope handed to it —
        // NOT just echoing back what `checkout_item` computed.
        let env = tx::envelope_from_xdr_base64(envelope_xdr_b64)?;
        let decoded = tx::decode_payments(&env)?;
        let legs: Vec<tx::PaymentLeg> = decoded
            .legs
            .iter()
            .map(|l| tx::PaymentLeg::new(l.dest_pk, l.asset.clone(), l.amount))
            .collect();
        let per_op_fee = decoded.fee
            / u32::try_from(legs.len())
                .map_err(|_| StellarError::Xdr("absurd leg count".into()))?;
        let rebuilt = tx::build_payment_transaction(
            decoded.source_pk,
            &legs,
            decoded.seq_num,
            per_op_fee,
            decoded.memo,
        )?;
        let net_id = tx::network_id(&self.passphrase);
        let hash = tx::tx_hash(net_id, &rebuilt)?;
        let hash_hex = if self.lie_about_hash {
            hex::encode([0xEEu8; 32])
        } else {
            hex::encode(hash)
        };
        *self.stored.lock().unwrap() = Some((hex::encode(hash), envelope_xdr_b64.to_string()));
        Ok(rpc::SubmitResult {
            hash: hash_hex,
            successful: true,
        })
    }

    async fn get_transaction(
        &self,
        tx_hash_hex: &str,
    ) -> Result<Option<rpc::TxRecord>, StellarError> {
        if self.fail_get {
            return Err(StellarError::Rpc("connection refused".into()));
        }
        let stored = self.stored.lock().unwrap().clone();
        let Some((hash, envelope_b64)) = stored else {
            return Ok(None);
        };
        if hash != tx_hash_hex {
            return Ok(None);
        }
        Ok(Some(rpc::TxRecord {
            successful: !self.force_unsuccessful_on_get,
            envelope_xdr: envelope_b64,
        }))
    }
}

// ── Fixtures ─────────────────────────────────────────────────────────────

const PASSPHRASE: &str = "Test SDF Network ; September 2015";

fn issuer_pk() -> PubKey {
    keys::Keypair::from_seed([2u8; 32]).pubkey()
}
fn cfg() -> StellarConfig {
    StellarConfig {
        network: Network::Testnet,
        usdc_issuer: keys::to_strkey(&issuer_pk()),
        base_fee_stroops: 100,
        stewards: None,
    }
}
fn cfg_stewards(w: PubKey) -> StellarConfig {
    StellarConfig {
        stewards: Some(w),
        ..cfg()
    }
}
fn signer() -> keys::Keypair {
    keys::Keypair::from_seed([1u8; 32])
}
fn buyer() -> PubKey {
    signer().pubkey()
}
fn pk(b: u8) -> PubKey {
    PubKey([b; 32])
}
fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Charge `split` for `item` against a rail whose FakeRpc will honestly
/// decode+re-hash+serve back whatever is actually submitted (see module
/// docs). Returns `(rail, receipt)`; the rail's own FakeRpc still holds the
/// state, so `rail.verify_receipt*` reads back the real submitted wire tx.
fn charge(cfg: StellarConfig, item: &str, split: PaymentSplit) -> (StellarPaymentRail, Receipt) {
    let fake = FakeRpc::new(PASSPHRASE, 41);
    let rail = StellarPaymentRail::new(cfg, fake).with_signer(signer());
    let receipt = rt()
        .block_on(rail.checkout_for_item(&buyer(), item, split))
        .expect("charge must succeed");
    (rail, receipt)
}

fn proof_of(r: &Receipt) -> RailProof {
    serde_json::from_slice(&r.binding.as_ref().unwrap().rail_proof).unwrap()
}

fn with_proof(r: &Receipt, f: impl FnOnce(&mut RailProof)) -> Receipt {
    let mut p = proof_of(r);
    f(&mut p);
    let mut r2 = r.clone();
    let b = r2.binding.as_mut().unwrap();
    b.rail_proof = serde_json::to_vec(&p).unwrap();
    r2
}

/// Re-sign a receipt with THIS crate's own fixed, publicly-reproducible rail
/// key (`RawKeypairAuth::from_seed(blake3::hash(b"magnetite-stellar-rail"))`,
/// same seed `StellarPaymentRail::new` uses). Per the module docs this key is
/// a **self-consistency marker, not the security boundary** — anyone can
/// reproduce it, which is the point. Used ONLY so a test can isolate a check
/// that sits textually AFTER check 5's outer signature verify (checks 5's
/// offline-reconstruction sub-part, and checks 6-10 do not need this since
/// they read from the seeded fake chain, not the receipt) from that outer
/// signature's blanket backstop: without resigning, ANY proof/receipt tamper
/// also invalidates the outer signature, so a test could pass for the wrong
/// reason (the outer signature catching it) even with the check under test
/// disabled. Resigning removes that confound.
fn resign(r: &Receipt) -> Receipt {
    let rail = RawKeypairAuth::from_seed(*blake3::hash(b"magnetite-stellar-rail").as_bytes());
    let mut r2 = r.clone();
    r2.sig = rail.sign(&r2.signing_bytes());
    r2
}

/// A verify-only rail whose FakeRpc has been seeded directly (an adversarial
/// / dishonest chain state, bypassing the honest submit path).
fn verifier_with_seed(
    cfg: StellarConfig,
    hash_hex: &str,
    envelope_b64: &str,
) -> StellarPaymentRail {
    let fake = FakeRpc::new(PASSPHRASE, 1);
    fake.seed(hash_hex, envelope_b64);
    StellarPaymentRail::new(cfg, fake)
}

/// Build a signed envelope directly (bypassing `checkout_item`) so tests can
/// construct a chain state that disagrees with a receipt's own claims.
fn build_envelope(
    source_pk: [u8; 32],
    legs: &[(PubKey, u64)],
    seq: i64,
    fee_per_op: u32,
    memo: [u8; 32],
    signer: &keys::Keypair,
) -> (String, String) {
    let asset = tx::usdc_asset("USDC", issuer_pk().0).unwrap();
    let payment_legs: Vec<tx::PaymentLeg> = legs
        .iter()
        .map(|(w, a)| tx::PaymentLeg::new(w.0, asset.clone(), *a as i64))
        .collect();
    let fee = tx::total_fee(fee_per_op, payment_legs.len()).unwrap();
    let unsigned = tx::build_payment_transaction(source_pk, &payment_legs, seq, fee, memo).unwrap();
    let net_id = tx::network_id(PASSPHRASE);
    let hash = tx::tx_hash(net_id, &unsigned).unwrap();
    let sig = signer.sign(&hash);
    let env = tx::envelope(unsigned, source_pk, sig.0).unwrap();
    (hex::encode(hash), tx::envelope_to_xdr_base64(&env).unwrap())
}

// ── Money math: sum-exact, integer-only, no rates ───────────────────────────

#[test]
fn legs_sum_exactly_to_the_total() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let plan = rail
        .plan(&PaymentSplit::new(vec![
            Leg::developer(pk(2), 1_000_000),
            Leg::operator(pk(3), 250_000),
        ]))
        .unwrap();
    assert_eq!(plan.total, 1_250_000);
    assert_eq!(plan.legs.len(), 2);
    assert!(plan.skipped.is_empty());
    let sum: u64 = plan.legs.iter().map(|l| l.amount).sum();
    assert_eq!(sum, plan.total, "parts must sum to the total EXACTLY");
}

#[test]
fn many_legs_sum_exactly() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let legs: Vec<Leg> = (0u8..8)
        .map(|i| {
            Leg::new(
                pk(0x20 + i),
                DUST_FLOOR_STROOPS * (i as u64 + 1),
                Role::Other(format!("coauthor-{i}")),
            )
        })
        .collect();
    let want: u64 = legs.iter().map(|l| l.amount).sum();
    let plan = rail.plan(&PaymentSplit::new(legs)).unwrap();
    assert_eq!(plan.legs.len(), 8);
    assert_eq!(plan.total, want);
}

#[test]
fn a_split_that_overflows_u64_is_refused_not_clamped() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let s = PaymentSplit::new(vec![
        Leg::developer(pk(2), u64::MAX),
        Leg::operator(pk(3), DUST_FLOOR_STROOPS),
    ]);
    assert!(matches!(rail.plan(&s), Err(StellarError::Config(_))));
}

#[test]
fn a_zero_leg_split_is_refused() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert!(matches!(
        rail.plan(&PaymentSplit::new(vec![])),
        Err(StellarError::NoPayableLeg(0))
    ));
}

#[test]
fn a_single_leg_split_is_the_common_case() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let plan = rail
        .plan(&PaymentSplit::to_developer(pk(2), 1_990_000))
        .unwrap();
    assert_eq!(plan.legs.len(), 1);
    assert_eq!(plan.total, 1_990_000);
}

// ── Dust floor: skipped, never fatal ────────────────────────────────────────

#[test]
fn a_dust_leg_is_skipped_and_the_buyer_is_not_charged_for_it() {
    let stew = pk(0x57);
    let rail = StellarPaymentRail::new(cfg_stewards(stew), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let plan = rail
        .plan(&PaymentSplit::new(vec![
            Leg::developer(pk(2), 1_000_000),
            Leg::stewards(stew, 1),
        ]))
        .expect("a dust voluntary leg must never break a purchase");
    assert_eq!(plan.legs.len(), 1);
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.stewards_amount, 0);
    assert_eq!(plan.total, 1_000_000);
}

#[test]
fn a_leg_exactly_at_the_floor_is_paid() {
    let stew = pk(0x57);
    let rail = StellarPaymentRail::new(cfg_stewards(stew), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let plan = rail
        .plan(&PaymentSplit::new(vec![
            Leg::developer(pk(2), 1_000_000),
            Leg::stewards(stew, DUST_FLOOR_STROOPS),
        ]))
        .unwrap();
    assert_eq!(plan.legs.len(), 2, "the floor is inclusive");
    assert_eq!(plan.total, 1_000_000 + DUST_FLOOR_STROOPS);
}

#[test]
fn a_dust_only_split_is_refused_never_settled_as_free() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let s = PaymentSplit::new(vec![Leg::developer(pk(2), 10), Leg::operator(pk(3), 20)]);
    assert!(matches!(rail.plan(&s), Err(StellarError::NoPayableLeg(2))));
}

// ── The stewards destination is not the caller's, and not the node's ───────

#[test]
fn a_stewards_leg_to_any_other_wallet_is_refused() {
    let real = pk(0x57);
    let rail = StellarPaymentRail::new(cfg_stewards(real), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let operators_own_wallet = pk(0x0B);
    let s = PaymentSplit::new(vec![
        Leg::developer(pk(2), 1_000_000),
        Leg::stewards(operators_own_wallet, 25_000),
    ]);
    assert!(matches!(rail.plan(&s), Err(StellarError::Stewards(_))));
}

#[test]
fn a_stewards_leg_with_no_compiled_in_address_is_refused_not_dropped() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let s = PaymentSplit::new(vec![
        Leg::developer(pk(2), 1_000_000),
        Leg::stewards(pk(0x57), 25_000),
    ]);
    assert!(matches!(rail.plan(&s), Err(StellarError::Stewards(_))));
}

#[test]
fn a_stewards_leg_to_the_release_address_is_paid_and_verifies() {
    let stew = pk(0x57);
    let item = "game:go";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stew, 25_000),
    ]);
    let (rail, receipt) = charge(cfg_stewards(stew), item, split);
    assert_eq!(receipt.total, 1_000_000);
    assert_eq!(receipt.stewards_amount, 25_000);
    assert!(rail.verify_receipt_for_item(&receipt, item));
}

/// Verify-time re-check of the stewards destination (check 2's stewards
/// sub-check), distinct from `plan()`'s charge-time refusal above: a receipt
/// minted by some OTHER build (a different compiled-in stewards address)
/// must not verify here just because it verified there.
#[test]
fn a_stewards_leg_never_verifies_against_a_rail_with_a_different_stewards_address() {
    let stew = pk(0x57);
    let item = "game:go";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stew, 25_000),
    ]);
    let (_, receipt) = charge(cfg_stewards(stew), item, split);
    // A different node, built with a different stewards address, must refuse
    // even a genuine, chain-verified receipt.
    let other =
        StellarPaymentRail::new(cfg_stewards(pk(0x58)), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert!(!other.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_chain_name() {
    let (item, split) = base_case();
    let (rail, mut receipt) = charge(cfg(), item, split);
    receipt.binding.as_mut().unwrap().chain = "not-stellar".to_string();
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn the_mainnet_override_is_fatal_never_ignored() {
    let _g = env_lock();
    std::env::set_var(stewards::TESTNET_OVERRIDE_ENV, keys::to_strkey(&pk(0x57)));

    let mainnet = resolve_stewards(Network::Public);
    assert!(
        matches!(mainnet, Err(StellarError::Stewards(_))),
        "an override on mainnet must be FATAL, not ignored: {mainnet:?}"
    );
    let cfg = StellarConfig::resolve(Network::Public, keys::to_strkey(&issuer_pk()), 100);
    assert!(cfg.is_err(), "startup must refuse this configuration");

    let dev = resolve_stewards(Network::Testnet).unwrap();
    assert_eq!(dev, Some(pk(0x57)));

    std::env::remove_var(stewards::TESTNET_OVERRIDE_ENV);
}

#[test]
fn with_no_override_and_no_compiled_in_address_there_is_no_stewards_wallet() {
    let _g = env_lock();
    std::env::remove_var(stewards::TESTNET_OVERRIDE_ENV);
    assert!(
        stewards::COMPILED_IN.is_none(),
        "if this fires, a build set MAGNETITE_STEWARDS_WALLET_STELLAR"
    );
    assert_eq!(resolve_stewards(Network::Public).unwrap(), None);
    assert_eq!(resolve_stewards(Network::Testnet).unwrap(), None);
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ── One transaction, one PAYMENT operation per leg ──────────────────────────

#[test]
fn a_three_leg_split_is_one_transaction_and_round_trips() {
    let stew = pk(0x57);
    let item = "game:chess";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 800_000),
        Leg::operator(pk(0x0B), 175_000),
        Leg::stewards(stew, 25_000),
    ]);
    let (rail, receipt) = charge(cfg_stewards(stew), item, split);
    assert_eq!(receipt.total, 1_000_000);
    assert_eq!(receipt.payouts.len(), 3);
    assert!(rail.verify_receipt(&receipt));
    assert!(rail.verify_receipt_for_item(&receipt, item));
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:other"),
        "a receipt for one item must never unlock another"
    );
}

#[test]
fn eight_legs_are_one_transaction() {
    let item = "game:many";
    let legs: Vec<Leg> = (0u8..8)
        .map(|i| {
            Leg::new(
                pk(0x30 + i),
                100_000 + i as u64 * 1_000,
                Role::Other(format!("c{i}")),
            )
        })
        .collect();
    let (rail, receipt) = charge(cfg(), item, PaymentSplit::new(legs));
    assert_eq!(receipt.payouts.len(), 8);
    assert!(rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn too_many_legs_is_refused_never_truncated() {
    let legs: Vec<Leg> = (0..(tx::MAX_OPERATIONS + 1))
        .map(|i| Leg::developer(pk((i % 250) as u8), 100_000))
        .collect();
    let fake = FakeRpc::unconfirmed(PASSPHRASE, 1);
    let rail = StellarPaymentRail::new(cfg(), fake.clone()).with_signer(signer());
    let r = rt().block_on(rail.checkout_item(&buyer(), "game:huge", PaymentSplit::new(legs)));
    assert!(
        matches!(r, Err(StellarError::TooManyLegs { .. })),
        "got {r:?}"
    );
    assert_eq!(
        fake.sent.lock().unwrap().len(),
        0,
        "nothing may be submitted for a split that cannot be paid atomically"
    );
}

#[test]
fn one_wallet_appearing_in_two_legs_verifies() {
    let both = pk(0xAB);
    let item = "game:dual";
    let split = PaymentSplit::new(vec![
        Leg::developer(both, 600_000),
        Leg::operator(both, 400_000),
    ]);
    let (rail, receipt) = charge(cfg(), item, split);
    assert_eq!(receipt.payouts.len(), 2);
    assert!(rail.verify_receipt_for_item(&receipt, item));
}

// ── A receipt is bound to ONE distribution ──────────────────────────────────

#[test]
fn a_receipt_bound_to_one_distribution_fails_against_another() {
    let item = "game:chess";
    let a = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 900_000),
        Leg::operator(pk(0x0B), 100_000),
    ]);
    let b = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 500_000),
        Leg::operator(pk(0x0B), 500_000),
    ]);
    let ra = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert_ne!(
        binding_reference(&buyer(), item, &ra.plan(&a).unwrap().legs),
        binding_reference(&buyer(), item, &ra.plan(&b).unwrap().legs),
    );

    let (_, receipt_a) = charge(cfg(), item, a);
    // Re-point A's receipt at B's plan/legs by editing the proof: reference,
    // memo and signature all stop agreeing.
    let plan_b = ra.plan(&b).unwrap();
    let forged = with_proof(&receipt_a, |p| {
        p.legs = plan_b
            .legs
            .iter()
            .map(|l| ProofLeg {
                wallet: keys::to_strkey(&l.wallet),
                amount: l.amount,
                role: l.role.tag().to_string(),
            })
            .collect();
    });
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert!(!rail.verify_receipt_for_item(&forged, item));
}

#[test]
fn editing_the_amounts_of_a_receipt_denies() {
    let item = "game:chess";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 900_000),
        Leg::operator(pk(0x0B), 100_000),
    ]);
    let (rail, receipt) = charge(cfg(), item, split);
    assert!(rail.verify_receipt_for_item(&receipt, item));

    let mut forged = receipt.clone();
    forged.payouts[0].amount = 500_000;
    forged.payouts[1].amount = 500_000;
    assert!(!rail.verify_receipt_for_item(&forged, item));
}

#[test]
fn forging_a_role_tag_denies() {
    let stew = pk(0x57);
    let item = "game:go";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stew, 25_000),
    ]);
    let (rail, receipt) = charge(cfg_stewards(stew), item, split);
    assert!(rail.verify_receipt_for_item(&receipt, item));

    let forged = with_proof(&receipt, |p| p.legs[1].role = "operator".to_string());
    assert!(
        !rail.verify_receipt_for_item(&forged, item),
        "role tags are inside the binding digest and the on-chain memo"
    );
}

#[test]
fn a_proof_disagreeing_with_the_payouts_denies() {
    let item = "game:chess";
    let (rail, receipt) = charge(cfg(), item, PaymentSplit::to_developer(pk(0xD0), 1_000_000));
    let forged = with_proof(&receipt, |p| p.legs[0].wallet = keys::to_strkey(&pk(0xEE)));
    assert!(!rail.verify_receipt_for_item(&forged, item));
}

/// Isolates check 3 specifically: payouts and the rail proof's legs stay
/// perfectly consistent with each other (so check 2's proof-vs-payouts
/// sub-check does not fire) — only the outer binding's `reference` is wrong.
#[test]
fn rejects_binding_reference_that_disagrees_with_buyer_item_and_legs() {
    let item = "game:chess";
    let (rail, mut receipt) = charge(cfg(), item, PaymentSplit::to_developer(pk(0xD0), 1_000_000));
    receipt.binding.as_mut().unwrap().reference = hex::encode([0x11u8; 32]);
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

// ── The new, stronger check: offline signature reconstruction ─────────────

// These three RE-SIGN after tampering (see `resign`'s docs): without that,
// the tamper would also invalidate the OUTER rail signature (check 5's
// self-consistency signature, which covers the whole `rail_proof` blob), so
// a naive test would pass for the wrong reason — proving the outer signature
// fires, not proving the offline reconstruction/verify actually does its own
// independent work. Resigning isolates the sub-check under test.

#[test]
fn a_tampered_proof_signature_denies_even_though_everything_else_is_genuine() {
    let item = "game:chess";
    let (rail, receipt) = charge(cfg(), item, PaymentSplit::to_developer(pk(0xD0), 1_000_000));
    assert!(rail.verify_receipt_for_item(&receipt, item));
    let forged = resign(&with_proof(&receipt, |p| {
        p.signature = hex::encode([0u8; 64])
    }));
    assert!(
        !rail.verify_receipt_for_item(&forged, item),
        "a forged signature must never verify, even against the real chain state"
    );
}

#[test]
fn a_tampered_proof_sequence_number_denies_self_consistency() {
    let item = "game:chess";
    let (rail, receipt) = charge(cfg(), item, PaymentSplit::to_developer(pk(0xD0), 1_000_000));
    let forged = resign(&with_proof(&receipt, |p| p.seq_num += 1));
    assert!(
        !rail.verify_receipt_for_item(&forged, item),
        "a proof whose own fields do not re-hash to its claimed tx_hash must deny"
    );
}

#[test]
fn a_tampered_proof_fee_denies_self_consistency() {
    let item = "game:chess";
    let (rail, receipt) = charge(cfg(), item, PaymentSplit::to_developer(pk(0xD0), 1_000_000));
    let forged = resign(&with_proof(&receipt, |p| p.fee += 1));
    assert!(!rail.verify_receipt_for_item(&forged, item));
}

// ── Every existing rejection still rejects ─────────────────────────────────

fn base_case() -> (&'static str, PaymentSplit) {
    (
        "game:chess",
        PaymentSplit::new(vec![
            Leg::developer(pk(0xD0), 800_000),
            Leg::operator(pk(0x0B), 200_000),
        ]),
    )
}

#[test]
fn rejects_wrong_recipient_on_chain() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    // The operator's money went somewhere else.
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 800_000), (pk(0xEE), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        memo,
        &signer(),
    );
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert!(!verifier.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_amount_on_chain() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 700_000), (pk(0x0B), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        memo,
        &signer(),
    );
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert!(!verifier.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_issuer_on_chain() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    let other_issuer = keys::Keypair::from_seed([99u8; 32]).pubkey();
    let asset = tx::usdc_asset("USDC", other_issuer.0).unwrap();
    let legs = vec![
        tx::PaymentLeg::new(pk(0xD0).0, asset.clone(), 800_000),
        tx::PaymentLeg::new(pk(0x0B).0, asset, 200_000),
    ];
    let fee = tx::total_fee(cfg().base_fee_stroops, legs.len()).unwrap();
    let unsigned =
        tx::build_payment_transaction(buyer().0, &legs, proof.seq_num, fee, memo).unwrap();
    let hash = tx::tx_hash(tx::network_id(PASSPHRASE), &unsigned).unwrap();
    let sig = signer().sign(&hash);
    let env = tx::envelope(unsigned, buyer().0, sig.0).unwrap();
    let env_b64 = tx::envelope_to_xdr_base64(&env).unwrap();
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert!(!verifier.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_claimed_issuer() {
    let (item, split) = base_case();
    let (rail, mut receipt) = charge(cfg(), item, split);
    receipt.binding.as_mut().unwrap().mint = keys::to_strkey(&pk(0x99));
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_unconfirmed_transaction() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_failed_transaction() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let fake = FakeRpc::unsuccessful_on_get(PASSPHRASE, 1);
    // Seed with the real chain state so only `successful` differs.
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 800_000), (pk(0x0B), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        memo,
        &signer(),
    );
    fake.seed(&proof.tx_hash, &env_b64);
    let verifier = StellarPaymentRail::new(cfg(), fake);
    assert!(!verifier.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_buyer() {
    let (item, split) = base_case();
    let (_, mut receipt) = charge(cfg(), item, split);
    receipt.buyer = pk(0x99);
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_buyer_not_the_transaction_source() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    // Someone else's account submitted the (otherwise identical) legs.
    let stranger = keys::Keypair::from_seed([77u8; 32]);
    let (_, env_b64) = build_envelope(
        stranger.pubkey().0,
        &[(pk(0xD0), 800_000), (pk(0x0B), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        memo,
        &stranger,
    );
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert!(!verifier.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_item_binding() {
    let (item, split) = base_case();
    let (rail, mut receipt) = charge(cfg(), item, split);
    receipt.binding.as_mut().unwrap().item = "game:expensive".to_string();
    assert!(!rail.verify_receipt_for_item(&receipt, "game:expensive"));
}

#[test]
fn rejects_replay_against_another_item() {
    let (item, split) = base_case();
    let (rail, receipt) = charge(cfg(), item, split);
    assert!(rail.verify_receipt_for_item(&receipt, item));
    assert!(!rail.verify_receipt_for_item(&receipt, "game:expensive"));
}

#[test]
fn rejects_chain_memo_binding_a_different_item() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    // A transaction whose memo binds a DIFFERENT item, same legs.
    let other_memo = binding_reference(
        &buyer(),
        "game:other",
        &[
            Leg::developer(pk(0xD0), 800_000),
            Leg::operator(pk(0x0B), 200_000),
        ],
    );
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 800_000), (pk(0x0B), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        other_memo,
        &signer(),
    );
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert!(!verifier.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_altered_memo() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 800_000), (pk(0x0B), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        [0xAAu8; 32],
        &signer(),
    );
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert!(!verifier.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_unaccounted_extra_recipient() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 800_000), (pk(0x0B), 200_000), (pk(0xEE), 7)],
        proof.seq_num,
        cfg().base_fee_stroops,
        memo,
        &signer(),
    );
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert!(
        !verifier.verify_receipt_for_item(&receipt, item),
        "no unaccounted party may appear as an extra operation"
    );
}

#[test]
fn rejects_missing_binding() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let s = PaymentSplit::to_developer(pk(2), 1_000_000);
    let r = rt().block_on(rail.checkout(&pk(1), s));
    assert!(r.binding.is_none());
    assert!(!rail.verify_receipt(&r));
}

#[test]
fn rejects_tampered_rail_signature() {
    let (item, split) = base_case();
    let (rail, mut receipt) = charge(cfg(), item, split);
    assert!(rail.verify_receipt(&receipt));
    receipt.sig = Sig([0u8; 64]);
    assert!(!rail.verify_receipt(&receipt));
}

#[test]
fn rejects_tampered_stewards_amount() {
    let stew = pk(0x57);
    let item = "game:go";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stew, 25_000),
    ]);
    let (rail, mut receipt) = charge(cfg_stewards(stew), item, split);
    receipt.stewards_amount = 900_000;
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

/// Isolates check 5's sum-exactness sub-check: `total` alone is wrong while
/// every payout, and the proof, stay consistent with each other.
#[test]
fn rejects_a_total_that_does_not_match_the_sum_of_payouts() {
    let item = "game:chess";
    let (rail, mut receipt) = charge(cfg(), item, PaymentSplit::to_developer(pk(0xD0), 1_000_000));
    receipt.total = 2_000_000;
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_rpc_error() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let broken = StellarPaymentRail::new(cfg(), FakeRpc::failing_get(PASSPHRASE, 1));
    assert!(
        !broken.verify_receipt_for_item(&receipt, item),
        "cannot verify NEVER grants — there is no fail-open path"
    );
}

#[test]
fn rejects_unparseable_rail_proof() {
    let (item, split) = base_case();
    let (rail, mut receipt) = charge(cfg(), item, split);
    receipt.binding.as_mut().unwrap().rail_proof = b"not json".to_vec();
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn refuses_to_issue_a_receipt_when_horizon_lies_about_the_hash() {
    let fake = FakeRpc::lying_about_hash(PASSPHRASE, 1);
    let rail = StellarPaymentRail::new(cfg(), fake).with_signer(signer());
    let err = rt().block_on(rail.checkout_item(
        &buyer(),
        "game:chess",
        PaymentSplit::to_developer(pk(0xD0), 1_000_000),
    ));
    assert!(
        err.is_err(),
        "a horizon-reported hash that disagrees with the locally computed one must never yield a receipt"
    );
}

#[test]
fn charge_propagates_sequence_load_failure() {
    let fake = FakeRpc::failing_load(PASSPHRASE);
    let rail = StellarPaymentRail::new(cfg(), fake).with_signer(signer());
    let err = rt().block_on(rail.checkout_item(
        &buyer(),
        "game:chess",
        PaymentSplit::to_developer(pk(0xD0), 1_000_000),
    ));
    assert!(err.is_err());
}

#[test]
fn charge_propagates_submit_failure() {
    let fake = FakeRpc::failing_submit(PASSPHRASE, 1);
    let rail = StellarPaymentRail::new(cfg(), fake).with_signer(signer());
    let err = rt().block_on(rail.checkout_item(
        &buyer(),
        "game:chess",
        PaymentSplit::to_developer(pk(0xD0), 1_000_000),
    ));
    assert!(err.is_err());
}

// ── Non-custodial refusal + honestly-absent capabilities ────────────────────

#[test]
fn refuses_to_spend_a_key_it_does_not_hold() {
    let rail =
        StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1)).with_signer(signer());
    let stranger = pk(0x55);
    let s = PaymentSplit::to_developer(pk(0xD0), 1_000_000);
    let r = rt().block_on(rail.checkout_item(&stranger, "game:chess", s));
    assert!(matches!(r, Err(StellarError::NotOurKey(_))));
}

#[test]
fn a_verify_only_rail_cannot_charge() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let s = PaymentSplit::to_developer(pk(0xD0), 1_000_000);
    let r = rt().block_on(rail.checkout_item(&buyer(), "game:chess", s));
    assert!(matches!(r, Err(StellarError::NotOurKey(_))));
}

#[test]
fn channels_and_escrow_are_unsupported_not_faked() {
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    let c = rt().block_on(rail.open_channel(&pk(3)));
    assert!(matches!(c, Err(PaymentError::Unsupported(_))));
    let e = rt().block_on(rail.escrow(WagerTerms {
        players: vec![pk(1)],
        stake: 1,
        currency: "USDC".into(),
        game: magnetite_seams::blobstore::Hash::of(b"chess"),
    }));
    assert!(matches!(e, Err(PaymentError::Unsupported(_))));
}

// ── A26: verify_receipt_for_item_tiered — confirmed / could-not-check / refused ─

/// Confirmed: offline checks pass AND Horizon confirms → `Settled`.
#[test]
fn tiered_reports_settled_when_horizon_confirms() {
    let (item, split) = base_case();
    let (rail, receipt) = charge(cfg(), item, split);
    assert_eq!(
        rail.verify_receipt_for_item_tiered(&receipt, item),
        Some(Settlement::Settled)
    );
}

/// Could-not-check, case 1: Horizon has never heard of the hash
/// (`Ok(None)`) — the exact shape of a testnet reset/pruned history per
/// `docs/stellar-history-retention.md`. Offline checks (1-5) all still
/// passed, so this degrades to `SignedUnsettled`, NOT a refusal.
#[test]
fn tiered_reports_signed_unsettled_when_horizon_has_never_heard_of_the_hash() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    // A fresh rail whose FakeRpc was never told about this transaction —
    // exactly `rejects_unconfirmed_transaction`'s setup, but read through the
    // tiered method instead of the boolean one.
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert_eq!(
        rail.verify_receipt_for_item_tiered(&receipt, item),
        Some(Settlement::SignedUnsettled)
    );
    assert_ne!(
        rail.verify_receipt_for_item_tiered(&receipt, item),
        Some(Settlement::Settled),
        "a miss is not a confirmation"
    );
}

/// Could-not-check, case 2: Horizon could not even be asked (`Err` — no
/// network, timeout, garbage response). An OPERATIONAL failure to check is
/// the same fact as a miss from this rail's point of view — also
/// `SignedUnsettled`, never a refusal and never silently treated as
/// confirmed.
#[test]
fn tiered_reports_signed_unsettled_when_horizon_is_unreachable() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::failing_get(PASSPHRASE, 1));
    assert_eq!(
        rail.verify_receipt_for_item_tiered(&receipt, item),
        Some(Settlement::SignedUnsettled),
        "an RPC error is 'could not check', not 'refused' — see the crux this backlog item names"
    );
}

/// Refused, case 1: Horizon answered and the transaction failed on chain
/// (`successful: false`). This must NEVER read as `SignedUnsettled` — Horizon
/// did not fail to answer, it said "no".
#[test]
fn tiered_refuses_when_horizon_reports_the_transaction_failed() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let fake = FakeRpc::unsuccessful_on_get(PASSPHRASE, 1);
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 800_000), (pk(0x0B), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        memo,
        &signer(),
    );
    fake.seed(&proof.tx_hash, &env_b64);
    let verifier = StellarPaymentRail::new(cfg(), fake);
    assert_eq!(
        verifier.verify_receipt_for_item_tiered(&receipt, item),
        None,
        "'Horizon said no' must never become the softer 'Horizon didn't answer' tier"
    );
}

/// Refused, case 2: Horizon answered, but the on-chain content disagrees
/// (wrong recipient) — one of checks 7-10, exercised through the tiered
/// path. Must refuse, not degrade.
#[test]
fn tiered_refuses_when_on_chain_content_disagrees() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let proof = proof_of(&receipt);
    let memo: [u8; 32] = hex::decode(&proof.reference).unwrap().try_into().unwrap();
    // The operator's money went somewhere else — same tamper as
    // `rejects_wrong_recipient_on_chain`, read through the tiered method.
    let (_, env_b64) = build_envelope(
        buyer().0,
        &[(pk(0xD0), 800_000), (pk(0xEE), 200_000)],
        proof.seq_num,
        cfg().base_fee_stroops,
        memo,
        &signer(),
    );
    let verifier = verifier_with_seed(cfg(), &proof.tx_hash, &env_b64);
    assert_eq!(
        verifier.verify_receipt_for_item_tiered(&receipt, item),
        None
    );
}

/// The crux this backlog item names: an offline check failing (checks 1-5,
/// a tampered/forged receipt) must refuse at EVERY reachability setting —
/// unconfirmed and unreachable included. `SignedUnsettled` is never a softer
/// failure mode for a receipt that never verified locally in the first
/// place.
#[test]
fn tiered_never_reports_signed_unsettled_for_a_receipt_that_fails_offline() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let mut tampered = receipt.clone();
    tampered.total += 1; // signature no longer covers the bytes — fails check 5

    let fakes: Vec<Arc<dyn rpc::StellarRpc>> = vec![
        FakeRpc::unconfirmed(PASSPHRASE, 1),
        FakeRpc::failing_get(PASSPHRASE, 1),
    ];
    for fake in fakes {
        let rail = StellarPaymentRail::new(cfg(), fake);
        assert_eq!(
            rail.verify_receipt_for_item_tiered(&tampered, item),
            None,
            "an offline-invalid receipt must refuse regardless of chain reachability"
        );
    }
}

/// The tiered and boolean paths must never disagree about what checks 1-5
/// decide: `verify_receipt_for_item` (boolean) already refuses this receipt
/// (it is unconfirmed), and the tiered method must not quietly grant
/// something the boolean method refuses outright — it is allowed to grant
/// something WEAKER (`SignedUnsettled`), never to disagree about whether
/// checks 1-5 passed.
#[test]
fn tiered_and_boolean_paths_agree_on_offline_validity() {
    let (item, split) = base_case();
    let (_, mut receipt) = charge(cfg(), item, split);
    receipt.total += 1;
    let rail = StellarPaymentRail::new(cfg(), FakeRpc::unconfirmed(PASSPHRASE, 1));
    assert!(!rail.verify_receipt_for_item(&receipt, item));
    assert_eq!(rail.verify_receipt_for_item_tiered(&receipt, item), None);
}

// ── Coverage-count assertion (FANOUT-LOOP-STATE.md §2): the ten checks ─────

/// Asserts `verify_async` in `lib.rs` still carries exactly ten numbered
/// checks (`── 1.` through `── 10.`), each present exactly once, and no
/// eleventh has crept in silently. A "check that exists but cannot fire" is
/// not a check; this at least proves the count of DOCUMENTED check sites
/// hasn't silently drifted, complementing the mutation tests above (which
/// prove each one actually fires).
#[test]
fn verify_async_carries_exactly_ten_numbered_checks() {
    let src = include_str!("lib.rs");
    for n in 1..=10 {
        let marker = format!("── {n}.");
        assert_eq!(
            src.matches(&marker).count(),
            1,
            "check {n} marker missing or duplicated in lib.rs"
        );
    }
    assert_eq!(
        src.matches("── 11.").count(),
        0,
        "an eleventh check crept in silently"
    );
}
