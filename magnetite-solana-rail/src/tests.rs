//! Integration tests for the Solana rail ADAPTER — not a re-test of the chain
//! logic itself. That logic (tx construction, signing, JSON-RPC, on-chain
//! verification: chain/mint match, memo binding, exact balance deltas,
//! commitment, buyer-signed) moved to the sibling `patala` repo's
//! `patala-solana` crate and is exhaustively tested THERE, offline, against a
//! scripted fake RPC of its own.
//!
//! What these tests cover instead — the things that are actually this
//! adapter's own responsibility:
//!  * magnetite's split arithmetic (`plan`) still sums exactly (pure, no chain);
//!  * a single-recipient split really does round-trip: `checkout_for_item` →
//!    `patala_solana::SolanaRail::charge` → `verify_receipt(_for_item)` →
//!    `patala_solana::SolanaRail::verify`, proving the wiring is real, not a
//!    stub (`on_chain_mismatch_denies_via_patala_delegation` proves this by
//!    flipping only the chain-level fact and nothing local);
//!  * a split that does NOT collapse to one recipient (a real operator cut
//!    and/or nonzero protocol fee) is REFUSED rather than silently dropping a
//!    leg or firing off non-atomic charges (`multi_party_split_is_refused_*`);
//!  * the non-custodial `NotOurKey` refusal, and the honestly-absent
//!    `open_channel`/`escrow`, are unchanged.
//!
//! Still fully offline: the only "network" here is `FakeRpc`, a scripted
//! implementation of `patala_solana::rpc::SolanaRpc`.

use super::*;
use patala_solana::binding_memo;
use serde_json::json;
use std::sync::Mutex;

const MINT: &str = patala_solana::tx::USDC_DEVNET_MINT;

/// Scripted RPC: returns whatever the test put in it, or an error. Mirrors
/// `patala-solana`'s own test fake (same shape, since it answers the same
/// trait) — the queried signature is ignored, exactly like there.
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
}

/// A fixed, valid-looking base58 32-byte "blockhash" — content does not
/// matter to the fake, only that it decodes to 32 bytes.
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
    async fn get_latest_blockhash(
        &self,
        _c: &str,
    ) -> Result<String, patala_solana::SolanaError> {
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

fn cfg(fee_wallet: Option<PubKey>) -> SolanaConfig {
    SolanaConfig {
        inner: patala_solana::SolanaConfig::devnet("http://127.0.0.1:8899"),
        fee_wallet,
    }
}

fn split(dev: PubKey, dev_amt: u64, op: Option<(PubKey, u64)>, bps: u16) -> PaymentSplit {
    PaymentSplit {
        developer: magnetite_seams::payment::Split {
            wallet: dev,
            amount: dev_amt,
        },
        operator: op.map(|(wallet, amount)| magnetite_seams::payment::Split { wallet, amount }),
        protocol_fee_bps: bps,
    }
}

/// Build a jsonParsed transaction that satisfies every patala-side check.
fn good_txn(
    buyer: &patala_solana::keys::PubKey,
    memo: &str,
    moves: &[(&patala_solana::keys::PubKey, i128)],
    mint: &str,
) -> serde_json::Value {
    const BASE: i128 = 1_000_000_000;
    let mut pre = Vec::new();
    let mut post = Vec::new();
    for (i, (who, delta)) in moves.iter().enumerate() {
        let owner = patala_solana::tx::pubkey_to_base58(who);
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
                    { "pubkey": patala_solana::tx::pubkey_to_base58(buyer), "signer": true, "writable": true }
                ],
                "instructions": [
                    { "program": "spl-memo", "programId": patala_solana::tx::MEMO_PROGRAM_ID, "parsed": memo }
                ]
            }
        },
        "meta": { "err": null, "preTokenBalances": pre, "postTokenBalances": post }
    })
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

// ── Money math (pure — no chain, no patala) ─────────────────────────────────

#[test]
fn split_math_zero_fee_sums_exactly() {
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed());
    let plan = rail
        .plan(&split(PubKey([2; 32]), 1_000_000, Some((PubKey([3; 32]), 250_000)), 0))
        .unwrap();
    assert_eq!(plan.protocol_fee, 0);
    assert_eq!(plan.total, 1_250_000);
    assert_eq!(plan.payouts.len(), 2, "no zero-value fee payout");
    let sum: u64 = plan.payouts.iter().map(|p| p.amount).sum();
    assert_eq!(sum, plan.total);
}

#[test]
fn split_math_nonzero_fee_sums_exactly() {
    let fee_wallet = PubKey([4; 32]);
    let rail = SolanaPaymentRail::new(cfg(Some(fee_wallet)), FakeRpc::unconfirmed());
    // 250 bps of 1_250_000 subtotal = 31_250 exactly.
    let plan = rail
        .plan(&split(PubKey([2; 32]), 1_000_000, Some((PubKey([3; 32]), 250_000)), 250))
        .unwrap();
    assert_eq!(plan.protocol_fee, 31_250);
    assert_eq!(plan.total, 1_281_250);
    assert_eq!(plan.payouts.last().unwrap().wallet, fee_wallet);
    let sum: u64 = plan.payouts.iter().map(|p| p.amount).sum();
    assert_eq!(sum, plan.total, "parts must sum to the total exactly");
}

#[test]
fn fee_truncates_down_and_never_loses_a_unit() {
    let fee_wallet = PubKey([4; 32]);
    let rail = SolanaPaymentRail::new(cfg(Some(fee_wallet)), FakeRpc::unconfirmed());
    // 1 bp of 999 units = 0.0999 -> 0. Integer division, no float, no rounding up.
    let plan = rail.plan(&split(PubKey([2; 32]), 999, None, 1)).unwrap();
    assert_eq!(plan.protocol_fee, 0);
    assert_eq!(plan.total, 999);
    let sum: u64 = plan.payouts.iter().map(|p| p.amount).sum();
    assert_eq!(sum, plan.total);
}

#[test]
fn fee_without_a_fee_wallet_is_a_loud_config_error() {
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed());
    let err = rail.plan(&split(PubKey([2; 32]), 1_000_000, None, 250));
    assert!(matches!(err, Err(SolanaError::Config(_))));
}

// ── The split does not generalize — refuse, never drop or split silently ───

#[test]
fn multi_party_split_is_refused_not_dropped_or_sent_non_atomically() {
    let signer = Keypair::from_seed([1u8; 32]);
    let buyer = PubKey(signer.pubkey().0);
    let fee_wallet = PubKey([0xFE; 32]);
    let rpc = FakeRpc::unconfirmed();
    let rail = SolanaPaymentRail::new(cfg(Some(fee_wallet)), rpc.clone())
        .with_signer(Keypair::from_seed([1u8; 32]));

    // developer + operator + a real protocol fee: three non-zero legs.
    let s = split(PubKey([0xD0; 32]), 1_000_000, Some((PubKey([0x0B; 32]), 250_000)), 250);
    let result = rt().block_on(rail.checkout_for_item(&buyer, "game:chess", s));
    assert!(
        matches!(result, Err(PaymentError::Unsupported(_))),
        "must refuse a split that cannot collapse to one recipient, got {result:?}"
    );
    assert_eq!(
        rpc.sent.lock().unwrap().len(),
        0,
        "no transaction may be sent for a split this rail cannot pay atomically"
    );
}

#[test]
fn operator_only_split_also_refuses() {
    // Even with protocol_fee_bps == 0, a real (nonzero) operator leg alongside
    // the developer leg is still two non-zero payouts.
    let signer = Keypair::from_seed([1u8; 32]);
    let buyer = PubKey(signer.pubkey().0);
    let rpc = FakeRpc::unconfirmed();
    let rail =
        SolanaPaymentRail::new(cfg(None), rpc.clone()).with_signer(Keypair::from_seed([1u8; 32]));

    let s = split(PubKey([0xD1; 32]), 900, Some((PubKey([0x0B; 32]), 100)), 0);
    let result = rt().block_on(rail.checkout_for_item(&buyer, "game:go", s));
    assert!(matches!(result, Err(PaymentError::Unsupported(_))));
    assert_eq!(rpc.sent.lock().unwrap().len(), 0);
}

// ── Single-recipient checkout really goes through patala ────────────────────

#[test]
fn single_recipient_checkout_round_trips_through_patala() {
    let signer = Keypair::from_seed([1u8; 32]);
    let buyer = PubKey(signer.pubkey().0);
    let dev = PubKey([0xD2; 32]);
    let dev_patala = patala_solana::keys::PubKey(dev.0);
    let amount = 1_000_000u64;
    let item = "game:chess";

    let memo = binding_memo(&signer.pubkey(), item);
    let txn = good_txn(
        &signer.pubkey(),
        &memo,
        &[(&signer.pubkey(), -(amount as i128)), (&dev_patala, amount as i128)],
        MINT,
    );
    let rpc = FakeRpc::with(txn);
    let rail = SolanaPaymentRail::new(cfg(None), rpc.clone())
        .with_signer(Keypair::from_seed([1u8; 32]));

    let s = split(dev, amount, None, 0);
    let receipt = rt()
        .block_on(rail.checkout_for_item(&buyer, item, s))
        .expect("a single-recipient split must be payable");

    assert_eq!(receipt.total, amount);
    assert_eq!(receipt.protocol_fee, 0);
    assert_eq!(receipt.payouts.len(), 1);
    assert_eq!(receipt.payouts[0].wallet, dev);
    assert_eq!(rpc.sent.lock().unwrap().len(), 1, "exactly one transaction");

    assert!(
        rail.verify_receipt(&receipt),
        "must verify — proves patala_solana::SolanaRail::verify is really wired in"
    );
    assert!(rail.verify_receipt_for_item(&receipt, item));
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:other"),
        "a receipt for one item must never unlock another"
    );
}

#[test]
fn on_chain_mismatch_denies_via_patala_delegation() {
    // Every magnetite-LOCAL check (rail self-signature, arithmetic, item
    // binding hash) is identical between the two rails built below — only
    // the delegated, patala-side chain fact changes. If this test passed
    // anyway it would mean the "crypto guts" had NOT actually moved to
    // patala and verification was rubber-stamping local checks alone.
    let signer = Keypair::from_seed([1u8; 32]);
    let buyer = PubKey(signer.pubkey().0);
    let dev = PubKey([0xD3; 32]);
    let dev_patala = patala_solana::keys::PubKey(dev.0);
    let amount = 500_000u64;
    let item = "game:go";
    let memo = binding_memo(&signer.pubkey(), item);

    let good = good_txn(
        &signer.pubkey(),
        &memo,
        &[(&signer.pubkey(), -(amount as i128)), (&dev_patala, amount as i128)],
        MINT,
    );
    let charging_rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(good.clone()))
        .with_signer(Keypair::from_seed([1u8; 32]));
    let s = split(dev, amount, None, 0);
    let receipt = rt()
        .block_on(charging_rail.checkout_for_item(&buyer, item, s))
        .unwrap();
    assert!(charging_rail.verify_receipt(&receipt), "sanity: the honest chain state verifies");

    let mut failed = good;
    failed["meta"]["err"] = json!({ "InstructionError": [0, "InsufficientFunds"] });
    let verify_only_rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(failed));
    assert!(
        !verify_only_rail.verify_receipt(&receipt),
        "a transaction the chain says failed must never verify, regardless of local checks"
    );
}

// ── Non-custodial refusal + honestly-absent capabilities ────────────────────

#[test]
fn refuses_to_spend_a_key_it_does_not_hold() {
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed())
        .with_signer(Keypair::from_seed([1u8; 32]));
    let stranger = PubKey([0x55; 32]);
    let s = split(PubKey([0xD0; 32]), 10, None, 0);
    let r = rt().block_on(rail.checkout_item(&stranger, "game:chess", s));
    assert!(matches!(r, Err(SolanaError::NotOurKey(_))));
}

#[test]
fn channels_and_escrow_are_unsupported_not_faked() {
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed());
    let c = rt().block_on(rail.open_channel(&PubKey([3; 32])));
    assert!(matches!(c, Err(PaymentError::Unsupported(_))));
    let e = rt().block_on(rail.escrow(WagerTerms {
        players: vec![PubKey([1; 32])],
        stake: 1,
        currency: "USDC".into(),
        game: magnetite_seams::blobstore::Hash::of(b"chess"),
    }));
    assert!(matches!(e, Err(PaymentError::Unsupported(_))));
}

#[test]
fn unbound_checkout_produces_an_unverifiable_receipt() {
    // The trait's item-less `checkout` cannot bind, so it must not be honoured.
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed());
    let s = split(PubKey([2; 32]), 5, None, 0);
    let r = rt().block_on(rail.checkout(&PubKey([1; 32]), s));
    assert!(r.binding.is_none());
    assert!(!rail.verify_receipt(&r), "an unbound receipt grants nothing");
}

#[test]
fn tampered_rail_signature_fails_closed() {
    let signer = Keypair::from_seed([1u8; 32]);
    let buyer = PubKey(signer.pubkey().0);
    let dev = PubKey([0xD4; 32]);
    let dev_patala = patala_solana::keys::PubKey(dev.0);
    let amount = 42_000u64;
    let item = "game:checkers";
    let memo = binding_memo(&signer.pubkey(), item);
    let txn = good_txn(
        &signer.pubkey(),
        &memo,
        &[(&signer.pubkey(), -(amount as i128)), (&dev_patala, amount as i128)],
        MINT,
    );
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(txn))
        .with_signer(Keypair::from_seed([1u8; 32]));
    let s = split(dev, amount, None, 0);
    let mut receipt = rt().block_on(rail.checkout_for_item(&buyer, item, s)).unwrap();
    assert!(rail.verify_receipt(&receipt));

    receipt.sig = Sig([0u8; 64]);
    assert!(
        !rail.verify_receipt(&receipt),
        "a forged/blank magnetite self-signature must never verify"
    );
}

#[test]
fn rejects_replay_for_another_item() {
    let signer = Keypair::from_seed([1u8; 32]);
    let buyer = PubKey(signer.pubkey().0);
    let dev = PubKey([0xD5; 32]);
    let dev_patala = patala_solana::keys::PubKey(dev.0);
    let amount = 777u64;
    let item = "game:cheap";
    let memo = binding_memo(&signer.pubkey(), item);
    let txn = good_txn(
        &signer.pubkey(),
        &memo,
        &[(&signer.pubkey(), -(amount as i128)), (&dev_patala, amount as i128)],
        MINT,
    );
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(txn))
        .with_signer(Keypair::from_seed([1u8; 32]));
    let s = split(dev, amount, None, 0);
    let receipt = rt().block_on(rail.checkout_for_item(&buyer, item, s)).unwrap();

    assert!(rail.verify_receipt_for_item(&receipt, "game:cheap"));
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:expensive"),
        "a fully-paid receipt for one item must never unlock a different one"
    );
}
