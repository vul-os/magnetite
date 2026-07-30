//! Integration tests for the Solana rail — **fully offline**. The only
//! "network" is `FakeRpc`, a scripted implementation of
//! `patala_solana::rpc::SolanaRpc`.
//!
//! What is covered here:
//!
//!  * the split arithmetic (`plan`): sum-exact, dust skipped, overflow refused,
//!    dust-only refused, stewards destination not caller-choosable;
//!  * multi-leg `checkout_for_item` → ONE transaction with one
//!    `TransferChecked` per leg → `verify_receipt(_for_item)` round trip, at
//!    one, two, three and many legs;
//!  * every fail-closed rejection: wrong recipient, wrong amount, wrong mint
//!    (claimed and on-chain), unconfirmed, failed transaction, insufficient
//!    commitment, wrong buyer, buyer not a signer, wrong item binding, replay
//!    against another item, chain memo binding a different item, missing memo,
//!    unaccounted extra recipient, missing binding, tampered signature, RPC
//!    error — plus the new ones: a receipt bound to one DISTRIBUTION verified
//!    against another, a forged role tag, and a forged stewards destination.
//!
//! What is NOT covered, and cannot be here: whether any of this is accepted by
//! a real validator. **No payment has ever settled through this rail.**

use super::*;
use serde_json::json;
use std::sync::Mutex;

const MINT: &str = patala_solana::tx::USDC_DEVNET_MINT;

/// Scripted RPC: returns whatever the test put in it, or an error.
struct FakeRpc {
    tx: Mutex<Option<serde_json::Value>>,
    fail: bool,
    blockhash: String,
    sent: Mutex<Vec<String>>,
}

impl FakeRpc {
    fn with(txn: serde_json::Value) -> Arc<Self> {
        Arc::new(Self {
            tx: Mutex::new(Some(txn)),
            fail: false,
            blockhash: bs58_encode_fixed(),
            sent: Mutex::new(Vec::new()),
        })
    }
    fn unconfirmed() -> Arc<Self> {
        Arc::new(Self {
            tx: Mutex::new(None),
            fail: false,
            blockhash: bs58_encode_fixed(),
            sent: Mutex::new(Vec::new()),
        })
    }
    fn broken() -> Arc<Self> {
        Arc::new(Self {
            tx: Mutex::new(None),
            fail: true,
            blockhash: bs58_encode_fixed(),
            sent: Mutex::new(Vec::new()),
        })
    }
}

/// A fixed, valid-looking base58 32-byte "blockhash".
fn bs58_encode_fixed() -> String {
    patala_solana::tx::pubkey_to_base58(&patala_solana::keys::PubKey([9u8; 32]))
}

#[async_trait::async_trait]
impl SolanaRpc for FakeRpc {
    async fn get_transaction(
        &self,
        _signature: &str,
        _commitment: &str,
    ) -> Result<Option<serde_json::Value>, patala_solana::SolanaError> {
        if self.fail {
            return Err(patala_solana::SolanaError::Rpc("connection refused".into()));
        }
        Ok(self.tx.lock().unwrap().clone())
    }
    async fn get_latest_blockhash(&self, _c: &str) -> Result<String, patala_solana::SolanaError> {
        if self.fail {
            return Err(patala_solana::SolanaError::Rpc("connection refused".into()));
        }
        Ok(self.blockhash.clone())
    }
    async fn send_transaction(
        &self,
        wire_base64: &str,
    ) -> Result<String, patala_solana::SolanaError> {
        if self.fail {
            return Err(patala_solana::SolanaError::Rpc("connection refused".into()));
        }
        self.sent.lock().unwrap().push(wire_base64.to_string());
        Ok("5".repeat(64))
    }
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Devnet config with no stewards address (the honest default: this tree has no
/// compiled-in one).
fn cfg() -> SolanaConfig {
    SolanaConfig {
        inner: patala_solana::SolanaConfig::devnet("http://127.0.0.1:8899"),
        stewards: None,
    }
}

/// Devnet config with an explicit stewards address, standing in for one that a
/// signed release baked in at build time.
fn cfg_stewards(w: PubKey) -> SolanaConfig {
    SolanaConfig {
        inner: patala_solana::SolanaConfig::devnet("http://127.0.0.1:8899"),
        stewards: Some(w),
    }
}

fn signer() -> Keypair {
    Keypair::from_seed([1u8; 32])
}
fn buyer() -> PubKey {
    PubKey(signer().pubkey().0)
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

/// Build a jsonParsed transaction satisfying every check: memo, buyer signer,
/// success, confirmed, and exact per-owner balance deltas.
fn good_txn(buyer: &PubKey, memo: &str, moves: &[(PubKey, i128)], mint: &str) -> serde_json::Value {
    const BASE: i128 = 1_000_000_000;
    let mut pre = Vec::new();
    let mut post = Vec::new();
    for (i, (who, delta)) in moves.iter().enumerate() {
        let owner = b58(who);
        pre.push(json!({
            "accountIndex": i, "mint": mint, "owner": owner,
            "uiTokenAmount": { "amount": BASE.to_string(), "decimals": 6 }
        }));
        post.push(json!({
            "accountIndex": i, "mint": mint, "owner": owner,
            "uiTokenAmount": { "amount": (BASE + delta).to_string(), "decimals": 6 }
        }));
    }
    json!({
        "slot": 1234,
        "confirmationStatus": "confirmed",
        "transaction": {
            "message": {
                "accountKeys": [
                    { "pubkey": b58(buyer), "signer": true, "writable": true }
                ],
                "instructions": [
                    { "program": "spl-memo", "programId": patala_solana::tx::MEMO_PROGRAM_ID, "parsed": memo }
                ]
            }
        },
        "meta": { "err": null, "preTokenBalances": pre, "postTokenBalances": post }
    })
}

/// The honest chain state for `split` bought as `item` by the rail's signer:
/// buyer −total, each leg +amount.
fn txn_for(cfg: SolanaConfig, item: &str, split: &PaymentSplit) -> (serde_json::Value, Plan) {
    let rail = SolanaPaymentRail::new(cfg, FakeRpc::unconfirmed());
    let plan = rail.plan(split).expect("planable");
    let b = buyer();
    let memo = binding_memo(&b, item, &plan.legs);
    let mut moves: Vec<(PubKey, i128)> = vec![(b, -(plan.total as i128))];
    for l in &plan.legs {
        moves.push((l.wallet, l.amount as i128));
    }
    (good_txn(&b, &memo, &moves, MINT), plan)
}

/// Charge `split` for `item` against a rail whose RPC already reports the
/// honest resulting transaction, and return `(rail, receipt)`.
fn charge(cfg: SolanaConfig, item: &str, split: PaymentSplit) -> (SolanaPaymentRail, Receipt) {
    let (txn, _) = txn_for(cfg.clone(), item, &split);
    let rpc = FakeRpc::with(txn);
    let rail = SolanaPaymentRail::new(cfg, rpc).with_signer(signer());
    let receipt = rt()
        .block_on(rail.checkout_for_item(&buyer(), item, split))
        .expect("charge must succeed");
    (rail, receipt)
}

/// A verify-only rail over an arbitrary scripted transaction.
fn verifier(cfg: SolanaConfig, txn: serde_json::Value) -> SolanaPaymentRail {
    SolanaPaymentRail::new(cfg, FakeRpc::with(txn))
}

// ── Money math: sum-exact, integer-only, no rates ───────────────────────────

#[test]
fn legs_sum_exactly_to_the_total() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
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
    assert_eq!(plan.stewards_amount, 0);
}

#[test]
fn many_legs_sum_exactly() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    let legs: Vec<Leg> = (0u8..8)
        .map(|i| {
            Leg::new(
                pk(0x20 + i),
                DUST_FLOOR_MICRO_USDC * (i as u64 + 1),
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
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    let s = PaymentSplit::new(vec![
        Leg::developer(pk(2), u64::MAX),
        Leg::operator(pk(3), DUST_FLOOR_MICRO_USDC),
    ]);
    assert!(matches!(rail.plan(&s), Err(SolanaError::Config(_))));
}

#[test]
fn a_zero_leg_split_is_refused() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    assert!(matches!(
        rail.plan(&PaymentSplit::new(vec![])),
        Err(SolanaError::NoPayableLeg(0))
    ));
}

#[test]
fn a_single_leg_split_is_the_common_case() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    let plan = rail
        .plan(&PaymentSplit::to_developer(pk(2), 1_990_000))
        .unwrap();
    assert_eq!(plan.legs.len(), 1);
    assert_eq!(plan.total, 1_990_000);
}

// ── Dust floor: skipped, never fatal — and never fail-open ──────────────────

#[test]
fn a_dust_leg_is_skipped_and_the_buyer_is_not_charged_for_it() {
    let stewards = pk(0x57);
    let rail = SolanaPaymentRail::new(cfg_stewards(stewards), FakeRpc::unconfirmed());
    // 1 micro-USDC of voluntary contribution: a millionth of a dollar.
    let plan = rail
        .plan(&PaymentSplit::new(vec![
            Leg::developer(pk(2), 1_000_000),
            Leg::stewards(stewards, 1),
        ]))
        .expect("a dust voluntary leg must never break a purchase");
    assert_eq!(plan.legs.len(), 1, "the dust leg is not paid");
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.skipped[0].amount, 1);
    assert_eq!(plan.stewards_amount, 0);
    assert_eq!(
        plan.total, 1_000_000,
        "and the total shrinks by exactly the skipped amount"
    );
}

#[test]
fn a_leg_exactly_at_the_floor_is_paid() {
    let stewards = pk(0x57);
    let rail = SolanaPaymentRail::new(cfg_stewards(stewards), FakeRpc::unconfirmed());
    let plan = rail
        .plan(&PaymentSplit::new(vec![
            Leg::developer(pk(2), 1_000_000),
            Leg::stewards(stewards, DUST_FLOOR_MICRO_USDC),
        ]))
        .unwrap();
    assert_eq!(plan.legs.len(), 2, "the floor is inclusive");
    assert_eq!(plan.stewards_amount, DUST_FLOOR_MICRO_USDC);
    assert_eq!(plan.total, 1_000_000 + DUST_FLOOR_MICRO_USDC);
}

#[test]
fn a_dust_only_split_is_refused_never_settled_as_free() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    let s = PaymentSplit::new(vec![Leg::developer(pk(2), 10), Leg::operator(pk(3), 20)]);
    assert!(
        matches!(rail.plan(&s), Err(SolanaError::NoPayableLeg(2))),
        "skipping every leg must refuse, not produce a free entitlement"
    );
}

#[test]
fn a_dust_leg_is_skipped_end_to_end_and_the_receipt_still_verifies() {
    let stewards = pk(0x57);
    let item = "game:chess";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 1_000_000),
        Leg::stewards(stewards, 1),
    ]);
    let (rail, receipt) = charge(cfg_stewards(stewards), item, split);
    assert_eq!(receipt.payouts.len(), 1);
    assert_eq!(receipt.total, 1_000_000);
    assert_eq!(receipt.stewards_amount, 0);
    assert!(rail.verify_receipt_for_item(&receipt, item));
}

// ── The stewards destination is not the caller's, and not the node's ────────

#[test]
fn a_stewards_leg_to_any_other_wallet_is_refused() {
    let real = pk(0x57);
    let rail = SolanaPaymentRail::new(cfg_stewards(real), FakeRpc::unconfirmed());
    // The classic attack the old `SOLANA_FEE_WALLET` allowed: point the
    // contribution at yourself.
    let operators_own_wallet = pk(0x0B);
    let s = PaymentSplit::new(vec![
        Leg::developer(pk(2), 1_000_000),
        Leg::stewards(operators_own_wallet, 25_000),
    ]);
    assert!(matches!(rail.plan(&s), Err(SolanaError::Stewards(_))));
}

#[test]
fn a_stewards_leg_with_no_compiled_in_address_is_refused_not_dropped() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    let s = PaymentSplit::new(vec![
        Leg::developer(pk(2), 1_000_000),
        Leg::stewards(pk(0x57), 25_000),
    ]);
    // Refused. NOT silently dropped (which would quietly cancel the
    // contribution) and NOT paid to whatever the caller named.
    assert!(matches!(rail.plan(&s), Err(SolanaError::Stewards(_))));
}

#[test]
fn a_stewards_leg_to_the_release_address_is_paid_and_verifies() {
    let stewards = pk(0x57);
    let item = "game:go";
    // The developer decided 2.5% of 1_000_000 = 25_000 before the rail saw it.
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stewards, 25_000),
    ]);
    let (rail, receipt) = charge(cfg_stewards(stewards), item, split);
    assert_eq!(receipt.total, 1_000_000);
    assert_eq!(receipt.stewards_amount, 25_000);
    assert_eq!(receipt.payouts.len(), 2);
    assert_eq!(receipt.payouts[1].wallet, stewards);
    assert!(rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn a_receipt_claiming_stewards_to_another_wallet_never_verifies() {
    let stewards = pk(0x57);
    let item = "game:go";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stewards, 25_000),
    ]);
    let (_, receipt) = charge(cfg_stewards(stewards), item, split.clone());

    // Same receipt, checked by a node whose build's stewards address differs:
    // the claimed stewards leg is not to ITS stewards address, so it denies.
    let (txn, _) = txn_for(cfg_stewards(stewards), item, &split);
    let other = verifier(cfg_stewards(pk(0x58)), txn);
    assert!(!other.verify_receipt_for_item(&receipt, item));
}

#[test]
fn the_mainnet_override_is_fatal_never_ignored() {
    // Serialized with the other env-touching test via one lock, because env is
    // process-global.
    let _g = env_lock();
    std::env::set_var(stewards::DEVNET_OVERRIDE_ENV, b58(&pk(0x57)));

    let mainnet = resolve_stewards(Cluster::MainnetBeta);
    assert!(
        matches!(mainnet, Err(SolanaError::Stewards(_))),
        "an override on mainnet must be FATAL, not ignored: {mainnet:?}"
    );
    // ...and the config constructor the backend uses must fail with it too, so
    // this is fatal at startup and not at first checkout.
    let cfg = SolanaConfig::resolve(patala_solana::SolanaConfig::mainnet(
        "https://api.mainnet-beta.solana.com",
    ));
    assert!(cfg.is_err(), "startup must refuse this configuration");

    // On devnet the override is honoured — that is the only place it works.
    let dev = resolve_stewards(Cluster::Devnet).unwrap();
    assert_eq!(dev, Some(pk(0x57)));

    std::env::remove_var(stewards::DEVNET_OVERRIDE_ENV);
}

#[test]
fn with_no_override_and_no_compiled_in_address_there_is_no_stewards_wallet() {
    let _g = env_lock();
    std::env::remove_var(stewards::DEVNET_OVERRIDE_ENV);
    // This tree publishes no stewards address, so nothing is baked in.
    assert!(
        stewards::COMPILED_IN.is_none(),
        "if this fires, a build set MAGNETITE_STEWARDS_WALLET"
    );
    assert_eq!(resolve_stewards(Cluster::MainnetBeta).unwrap(), None);
    assert_eq!(resolve_stewards(Cluster::Devnet).unwrap(), None);
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ── One transaction, one TransferChecked per leg ────────────────────────────

#[test]
fn a_three_leg_split_is_one_transaction_and_round_trips() {
    let stewards = pk(0x57);
    let item = "game:chess";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 800_000),
        Leg::operator(pk(0x0B), 175_000),
        Leg::stewards(stewards, 25_000),
    ]);
    let (txn, plan) = txn_for(cfg_stewards(stewards), item, &split);
    assert_eq!(plan.legs.len(), 3);

    let rpc = FakeRpc::with(txn);
    let rail = SolanaPaymentRail::new(cfg_stewards(stewards), rpc.clone()).with_signer(signer());
    let receipt = rt()
        .block_on(rail.checkout_for_item(&buyer(), item, split))
        .expect("a three-leg split must be payable atomically");

    assert_eq!(
        rpc.sent.lock().unwrap().len(),
        1,
        "ONE transaction — atomicity by construction, not by retry logic"
    );
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
    let split = PaymentSplit::new(legs);
    let (txn, plan) = txn_for(cfg(), item, &split);
    assert_eq!(plan.legs.len(), 8);

    let rpc = FakeRpc::with(txn);
    let rail = SolanaPaymentRail::new(cfg(), rpc.clone()).with_signer(signer());
    let receipt = rt()
        .block_on(rail.checkout_for_item(&buyer(), item, split))
        .expect("many legs must still be one atomic transaction");
    assert_eq!(rpc.sent.lock().unwrap().len(), 1);
    assert_eq!(receipt.payouts.len(), 8);
    assert!(rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn too_many_legs_is_refused_never_truncated() {
    // Enough legs that the serialized transaction cannot fit in one packet.
    let legs: Vec<Leg> = (0u8..40)
        .map(|i| Leg::developer(pk(0x40 + i), 100_000))
        .collect();
    let rpc = FakeRpc::unconfirmed();
    let rail = SolanaPaymentRail::new(cfg(), rpc.clone()).with_signer(signer());
    let r = rt().block_on(rail.checkout_item(&buyer(), "game:huge", PaymentSplit::new(legs)));
    assert!(
        matches!(r, Err(SolanaError::TooManyLegs { .. })),
        "got {r:?}"
    );
    assert_eq!(
        rpc.sent.lock().unwrap().len(),
        0,
        "nothing may be submitted for a split that cannot be paid atomically"
    );
}

#[test]
fn one_wallet_appearing_in_two_legs_verifies() {
    // A co-developer who is also the operator: the chain reports ONE net delta
    // per owner, so check 10 must aggregate rather than reject this.
    let both = pk(0xAB);
    let item = "game:dual";
    let split = PaymentSplit::new(vec![
        Leg::developer(both, 600_000),
        Leg::operator(both, 400_000),
    ]);
    let (rail, receipt) = charge(cfg(), item, split);
    assert_eq!(receipt.payouts.len(), 2);
    assert_eq!(receipt.total, 1_000_000);
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

    let (_, receipt_a) = charge(cfg(), item, a.clone());
    let (txn_b, _) = txn_for(cfg(), item, &b);

    // Same total, same buyer, same item, same wallets — only the DIVISION
    // differs. A's receipt must not verify against B's chain state.
    let rail_b = verifier(cfg(), txn_b);
    assert!(
        !rail_b.verify_receipt_for_item(&receipt_a, item),
        "a receipt bound to one distribution must never verify against another"
    );

    // And the binding references themselves differ.
    let ra = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    assert_ne!(
        binding_reference(&buyer(), item, &ra.plan(&a).unwrap().legs),
        binding_reference(&buyer(), item, &ra.plan(&b).unwrap().legs),
    );
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

    // Move money between legs, keeping the total: the proof legs, the derived
    // reference and the on-chain memo all stop agreeing.
    let mut forged = receipt.clone();
    forged.payouts[0].amount = 500_000;
    forged.payouts[1].amount = 500_000;
    assert!(!rail.verify_receipt_for_item(&forged, item));
}

#[test]
fn forging_a_role_tag_denies() {
    let stewards = pk(0x57);
    let item = "game:go";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stewards, 25_000),
    ]);
    let (rail, receipt) = charge(cfg_stewards(stewards), item, split);
    assert!(rail.verify_receipt_for_item(&receipt, item));

    // Relabel the stewards leg as the operator's in the rail proof. The derived
    // reference changes; the memo already on chain does not.
    let mut forged = receipt.clone();
    let b = forged.binding.as_mut().unwrap();
    let mut proof: serde_json::Value = serde_json::from_slice(&b.rail_proof).unwrap();
    proof["legs"][1]["role"] = json!("operator");
    b.rail_proof = serde_json::to_vec(&proof).unwrap();
    assert!(
        !rail.verify_receipt_for_item(&forged, item),
        "role tags are inside the binding digest and the on-chain memo"
    );
}

#[test]
fn a_proof_disagreeing_with_the_payouts_denies() {
    let item = "game:chess";
    let (rail, receipt) = charge(cfg(), item, PaymentSplit::to_developer(pk(0xD0), 1_000_000));

    let mut forged = receipt.clone();
    let b = forged.binding.as_mut().unwrap();
    let mut proof: serde_json::Value = serde_json::from_slice(&b.rail_proof).unwrap();
    proof["legs"][0]["wallet"] = json!(b58(&pk(0xEE)));
    b.rail_proof = serde_json::to_vec(&proof).unwrap();
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
    let (_, receipt) = charge(cfg(), item, split.clone());
    let plan = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed())
        .plan(&split)
        .unwrap();
    let memo = binding_memo(&buyer(), item, &plan.legs);
    // The operator's money went somewhere else.
    let txn = good_txn(
        &buyer(),
        &memo,
        &[
            (buyer(), -1_000_000),
            (pk(0xD0), 800_000),
            (pk(0xEE), 200_000),
        ],
        MINT,
    );
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_amount_on_chain() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split.clone());
    let plan = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed())
        .plan(&split)
        .unwrap();
    let memo = binding_memo(&buyer(), item, &plan.legs);
    let txn = good_txn(
        &buyer(),
        &memo,
        &[
            (buyer(), -900_000),
            (pk(0xD0), 800_000),
            (pk(0x0B), 100_000),
        ],
        MINT,
    );
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_mint_on_chain() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split.clone());
    let plan = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed())
        .plan(&split)
        .unwrap();
    let memo = binding_memo(&buyer(), item, &plan.legs);
    // Balances moved, but in some other token.
    let txn = good_txn(
        &buyer(),
        &memo,
        &[
            (buyer(), -1_000_000),
            (pk(0xD0), 800_000),
            (pk(0x0B), 200_000),
        ],
        patala_solana::tx::USDC_MAINNET_MINT,
    );
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_claimed_mint() {
    let (item, split) = base_case();
    let (txn, _) = txn_for(cfg(), item, &split);
    let (_, mut receipt) = charge(cfg(), item, split);
    receipt.binding.as_mut().unwrap().mint = patala_solana::tx::USDC_MAINNET_MINT.to_string();
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_unconfirmed_transaction() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_failed_transaction() {
    let (item, split) = base_case();
    let (txn, _) = txn_for(cfg(), item, &split);
    let (_, receipt) = charge(cfg(), item, split);
    let mut failed = txn;
    failed["meta"]["err"] = json!({ "InstructionError": [0, "InsufficientFunds"] });
    assert!(!verifier(cfg(), failed).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_missing_meta_err_field() {
    let (item, split) = base_case();
    let (txn, _) = txn_for(cfg(), item, &split);
    let (_, receipt) = charge(cfg(), item, split);
    let mut malformed = txn;
    malformed["meta"].as_object_mut().unwrap().remove("err");
    assert!(!verifier(cfg(), malformed).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_insufficient_commitment() {
    let (item, split) = base_case();
    let (txn, _) = txn_for(cfg(), item, &split);
    let (_, receipt) = charge(cfg(), item, split);
    // A `finalized` rail must not accept a merely `confirmed` transaction.
    let mut inner = patala_solana::SolanaConfig::devnet("http://127.0.0.1:8899");
    inner.commitment = Commitment::Finalized;
    let strict = SolanaConfig {
        inner,
        stewards: None,
    };
    assert!(!verifier(strict, txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_buyer() {
    let (item, split) = base_case();
    let (txn, _) = txn_for(cfg(), item, &split);
    let (_, mut receipt) = charge(cfg(), item, split);
    receipt.buyer = pk(0x99);
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_buyer_not_a_signer() {
    let (item, split) = base_case();
    let (mut txn, _) = txn_for(cfg(), item, &split);
    let (_, receipt) = charge(cfg(), item, split);
    txn["transaction"]["message"]["accountKeys"][0]["signer"] = json!(false);
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_wrong_item_binding() {
    let (item, split) = base_case();
    let (txn, _) = txn_for(cfg(), item, &split);
    let (_, mut receipt) = charge(cfg(), item, split);
    // Re-point the binding at another item without redoing the reference.
    receipt.binding.as_mut().unwrap().item = "game:expensive".to_string();
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, "game:expensive"));
}

#[test]
fn rejects_replay_against_another_item() {
    let (item, split) = base_case();
    let (rail, receipt) = charge(cfg(), item, split);
    assert!(rail.verify_receipt_for_item(&receipt, item));
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:expensive"),
        "a fully-paid receipt for one item must never unlock a different one"
    );
}

#[test]
fn rejects_chain_memo_binding_a_different_item() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split.clone());
    let plan = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed())
        .plan(&split)
        .unwrap();
    let other_memo = binding_memo(&buyer(), "game:other", &plan.legs);
    let txn = good_txn(
        &buyer(),
        &other_memo,
        &[
            (buyer(), -1_000_000),
            (pk(0xD0), 800_000),
            (pk(0x0B), 200_000),
        ],
        MINT,
    );
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_missing_memo() {
    let (item, split) = base_case();
    let (mut txn, _) = txn_for(cfg(), item, &split);
    let (_, receipt) = charge(cfg(), item, split);
    txn["transaction"]["message"]["instructions"] = json!([]);
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_unaccounted_extra_recipient() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split.clone());
    let plan = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed())
        .plan(&split)
        .unwrap();
    let memo = binding_memo(&buyer(), item, &plan.legs);
    // Check #10 with an arbitrary leg count: a party nobody accounted for
    // gaining the mint denies, however many legitimate legs there are.
    let txn = good_txn(
        &buyer(),
        &memo,
        &[
            (buyer(), -1_000_000),
            (pk(0xD0), 800_000),
            (pk(0x0B), 200_000),
            (pk(0xEE), 7),
        ],
        MINT,
    );
    assert!(
        !verifier(cfg(), txn).verify_receipt_for_item(&receipt, item),
        "no unaccounted party may gain or lose this mint"
    );
}

#[test]
fn rejects_unaccounted_party_losing_the_mint() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split.clone());
    let plan = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed())
        .plan(&split)
        .unwrap();
    let memo = binding_memo(&buyer(), item, &plan.legs);
    let txn = good_txn(
        &buyer(),
        &memo,
        &[
            (buyer(), -1_000_000),
            (pk(0xD0), 800_000),
            (pk(0x0B), 200_000),
            (pk(0xEE), -5),
        ],
        MINT,
    );
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_missing_binding() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    let s = PaymentSplit::to_developer(pk(2), 1_000_000);
    let r = rt().block_on(rail.checkout(&pk(1), s));
    assert!(r.binding.is_none());
    assert!(
        !rail.verify_receipt(&r),
        "an unbound receipt grants nothing (the item-less form cannot bind)"
    );
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
    let stewards = pk(0x57);
    let item = "game:go";
    let split = PaymentSplit::new(vec![
        Leg::developer(pk(0xD0), 975_000),
        Leg::stewards(stewards, 25_000),
    ]);
    let (rail, mut receipt) = charge(cfg_stewards(stewards), item, split);
    receipt.stewards_amount = 900_000;
    assert!(!rail.verify_receipt_for_item(&receipt, item));
}

#[test]
fn rejects_rpc_error() {
    let (item, split) = base_case();
    let (_, receipt) = charge(cfg(), item, split);
    let broken = SolanaPaymentRail::new(cfg(), FakeRpc::broken());
    assert!(
        !broken.verify_receipt_for_item(&receipt, item),
        "cannot verify NEVER grants — there is no fail-open path"
    );
}

#[test]
fn rejects_unparseable_rail_proof() {
    let (item, split) = base_case();
    let (txn, _) = txn_for(cfg(), item, &split);
    let (_, mut receipt) = charge(cfg(), item, split);
    receipt.binding.as_mut().unwrap().rail_proof = b"not json".to_vec();
    assert!(!verifier(cfg(), txn).verify_receipt_for_item(&receipt, item));
}

// ── Non-custodial refusal + honestly-absent capabilities ────────────────────

#[test]
fn refuses_to_spend_a_key_it_does_not_hold() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed()).with_signer(signer());
    let stranger = pk(0x55);
    let s = PaymentSplit::to_developer(pk(0xD0), 1_000_000);
    let r = rt().block_on(rail.checkout_item(&stranger, "game:chess", s));
    assert!(matches!(r, Err(SolanaError::NotOurKey(_))));
}

#[test]
fn a_verify_only_rail_cannot_charge() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
    let s = PaymentSplit::to_developer(pk(0xD0), 1_000_000);
    let r = rt().block_on(rail.checkout_item(&buyer(), "game:chess", s));
    assert!(matches!(r, Err(SolanaError::NotOurKey(_))));
}

#[test]
fn channels_and_escrow_are_unsupported_not_faked() {
    let rail = SolanaPaymentRail::new(cfg(), FakeRpc::unconfirmed());
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
