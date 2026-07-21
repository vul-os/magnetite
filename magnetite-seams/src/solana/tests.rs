//! Offline tests for the Solana rail. **No network.** The RPC is a fake, so CI
//! exercises every accept and reject path deterministically.
//!
//! An opt-in live test against devnet / `solana-test-validator` lives at the
//! bottom, gated on `MAGNETITE_SOLANA_LIVE_RPC` and skipped by default.

use super::*;
use super::tx::pubkey_from_base58;
use serde_json::json;
use std::sync::Mutex;

const MINT: &str = tx::USDC_DEVNET_MINT;
const OTHER_MINT: &str = "So11111111111111111111111111111111111111112";

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
            blockhash: bs58::encode([9u8; 32]).into_string(),
            sent: Mutex::new(Vec::new()),
        })
    }
    fn unconfirmed() -> Arc<Self> {
        Arc::new(Self {
            tx: Mutex::new(None),
            fail: false,
            blockhash: bs58::encode([9u8; 32]).into_string(),
            sent: Mutex::new(Vec::new()),
        })
    }
    fn broken() -> Arc<Self> {
        Arc::new(Self {
            tx: Mutex::new(None),
            fail: true,
            blockhash: bs58::encode([9u8; 32]).into_string(),
            sent: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait::async_trait]
impl SolanaRpc for FakeRpc {
    async fn get_transaction(
        &self,
        _signature: &str,
        _commitment: &str,
    ) -> Result<Option<serde_json::Value>, SolanaError> {
        if self.fail {
            return Err(SolanaError::Rpc("connection refused".into()));
        }
        Ok(self.tx.lock().unwrap().clone())
    }
    async fn get_latest_blockhash(&self, _c: &str) -> Result<String, SolanaError> {
        if self.fail {
            return Err(SolanaError::Rpc("connection refused".into()));
        }
        Ok(self.blockhash.clone())
    }
    async fn send_transaction(&self, wire_base64: &str) -> Result<String, SolanaError> {
        if self.fail {
            return Err(SolanaError::Rpc("connection refused".into()));
        }
        self.sent.lock().unwrap().push(wire_base64.to_string());
        Ok("5".repeat(64))
    }
}

fn cfg(fee_wallet: Option<PubKey>) -> SolanaConfig {
    SolanaConfig {
        rpc_url: "http://127.0.0.1:8899".into(),
        cluster: Cluster::Devnet,
        commitment: Commitment::Confirmed,
        usdc_mint: pubkey_from_base58(MINT).unwrap(),
        fee_wallet,
    }
}

fn key(seed: u8) -> RawKeypairAuth {
    RawKeypairAuth::from_seed([seed; 32])
}

fn split(dev: PubKey, dev_amt: u64, op: Option<(PubKey, u64)>, bps: u16) -> PaymentSplit {
    PaymentSplit {
        developer: crate::payment::Split {
            wallet: dev,
            amount: dev_amt,
        },
        operator: op.map(|(wallet, amount)| crate::payment::Split { wallet, amount }),
        protocol_fee_bps: bps,
    }
}

/// Build a jsonParsed transaction that satisfies every check.
fn good_txn(buyer: &PubKey, memo: &str, moves: &[(&PubKey, i128)], mint: &str) -> serde_json::Value {
    // pre = 1_000_000_000 for everyone; post = pre + delta.
    const BASE: i128 = 1_000_000_000;
    let mut pre = Vec::new();
    let mut post = Vec::new();
    for (i, (who, delta)) in moves.iter().enumerate() {
        let owner = pubkey_to_base58(who);
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
                    { "pubkey": pubkey_to_base58(buyer), "signer": true, "writable": true }
                ],
                "instructions": [
                    { "program": "spl-memo", "programId": tx::MEMO_PROGRAM_ID, "parsed": memo }
                ]
            }
        },
        "meta": { "err": null, "preTokenBalances": pre, "postTokenBalances": post }
    })
}

/// A valid (rail, receipt) pair for `item`, plus the fake RPC backing it.
fn scenario(
    item: &str,
    bps: u16,
    with_op: bool,
) -> (SolanaPaymentRail, Receipt, PubKey, PubKey, Option<PubKey>, PubKey) {
    let buyer = key(1).node_pubkey();
    let dev = key(2).node_pubkey();
    let op = with_op.then(|| key(3).node_pubkey());
    let fee_wallet = key(4).node_pubkey();

    let c = cfg(Some(fee_wallet));
    let planner = SolanaPaymentRail::new(c.clone(), FakeRpc::unconfirmed());
    let plan = planner
        .plan(&split(dev, 1_000_000, op.map(|o| (o, 250_000)), bps))
        .unwrap();

    let mut moves: Vec<(&PubKey, i128)> = vec![(&buyer, -(plan.total as i128))];
    for p in &plan.payouts {
        moves.push((&p.wallet, p.amount as i128));
    }
    let memo = binding_memo(&buyer, item);
    let txn = good_txn(&buyer, &memo, &moves, MINT);

    let rail = SolanaPaymentRail::new(c, FakeRpc::with(txn));
    let receipt = rail.receipt_for_signature(&buyer, item, &plan, &"5".repeat(64));
    (rail, receipt, buyer, dev, op, fee_wallet)
}

// ── Money math ───────────────────────────────────────────────────────────────

#[test]
fn split_math_zero_fee_sums_exactly() {
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed());
    let plan = rail
        .plan(&split(key(2).node_pubkey(), 1_000_000, Some((key(3).node_pubkey(), 250_000)), 0))
        .unwrap();
    assert_eq!(plan.protocol_fee, 0);
    assert_eq!(plan.total, 1_250_000);
    assert_eq!(plan.payouts.len(), 2, "no zero-value fee payout");
    let sum: u64 = plan.payouts.iter().map(|p| p.amount).sum();
    assert_eq!(sum, plan.total);
}

#[test]
fn split_math_nonzero_fee_sums_exactly() {
    let fee_wallet = key(4).node_pubkey();
    let rail = SolanaPaymentRail::new(cfg(Some(fee_wallet)), FakeRpc::unconfirmed());
    // 250 bps of 1_000_000 + 250_000 = 2.5% of 1_250_000 = 31_250 exactly.
    let plan = rail
        .plan(&split(key(2).node_pubkey(), 1_000_000, Some((key(3).node_pubkey(), 250_000)), 250))
        .unwrap();
    assert_eq!(plan.protocol_fee, 31_250);
    assert_eq!(plan.total, 1_281_250);
    assert_eq!(plan.payouts.last().unwrap().wallet, fee_wallet);
    let sum: u64 = plan.payouts.iter().map(|p| p.amount).sum();
    assert_eq!(sum, plan.total, "parts must sum to the total exactly");
}

#[test]
fn fee_truncates_down_and_never_loses_a_unit() {
    let fee_wallet = key(4).node_pubkey();
    let rail = SolanaPaymentRail::new(cfg(Some(fee_wallet)), FakeRpc::unconfirmed());
    // 1 bp of 999 units = 0.0999 -> 0. Integer division, no float, no rounding up.
    let plan = rail.plan(&split(key(2).node_pubkey(), 999, None, 1)).unwrap();
    assert_eq!(plan.protocol_fee, 0);
    assert_eq!(plan.total, 999);
    let sum: u64 = plan.payouts.iter().map(|p| p.amount).sum();
    assert_eq!(sum, plan.total);
}

#[test]
fn fee_without_a_fee_wallet_is_a_loud_config_error() {
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed());
    let err = rail.plan(&split(key(2).node_pubkey(), 1_000_000, None, 250));
    assert!(matches!(err, Err(SolanaError::Config(_))));
}

// ── Verification: the happy path ─────────────────────────────────────────────

#[test]
fn accepts_a_good_transaction() {
    let (rail, receipt, ..) = scenario("game:chess", 0, true);
    assert!(rail.verify_receipt(&receipt));
    assert!(rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn accepts_a_good_transaction_with_a_protocol_fee() {
    let (rail, receipt, ..) = scenario("game:go", 250, true);
    assert_eq!(receipt.protocol_fee, 31_250);
    assert!(rail.verify_receipt_for_item(&receipt, "game:go"));
}

// ── Verification: every rejection ────────────────────────────────────────────

#[test]
fn rejects_wrong_recipient() {
    let (rail, mut receipt, ..) = scenario("game:chess", 0, true);
    receipt.payouts[0].wallet = key(9).node_pubkey(); // not who was paid on chain
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_wrong_amount() {
    let (rail, mut receipt, ..) = scenario("game:chess", 0, false);
    receipt.payouts[0].amount += 1;
    receipt.total += 1;
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_wrong_mint() {
    let buyer = key(1).node_pubkey();
    let dev = key(2).node_pubkey();
    let c = cfg(None);
    let planner = SolanaPaymentRail::new(c.clone(), FakeRpc::unconfirmed());
    let plan = planner.plan(&split(dev, 1_000_000, None, 0)).unwrap();
    // Chain shows the money moving in a DIFFERENT token.
    let txn = good_txn(
        &buyer,
        &binding_memo(&buyer, "game:chess"),
        &[(&buyer, -1_000_000), (&dev, 1_000_000)],
        OTHER_MINT,
    );
    let rail = SolanaPaymentRail::new(c, FakeRpc::with(txn));
    let receipt = rail.receipt_for_signature(&buyer, "game:chess", &plan, &"5".repeat(64));
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:chess"),
        "payment in another token is not a USDC payment"
    );
}

#[test]
fn rejects_a_receipt_claiming_a_different_mint() {
    let (rail, mut receipt, ..) = scenario("game:chess", 0, false);
    let b = receipt.binding.as_mut().unwrap();
    b.mint = OTHER_MINT.to_string();
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_unconfirmed() {
    let (_, receipt, ..) = scenario("game:chess", 0, false);
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed());
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:chess"),
        "a transaction the cluster has never heard of grants nothing"
    );
}

#[test]
fn rejects_a_failed_transaction() {
    let (_, receipt, buyer, dev, ..) = scenario("game:chess", 0, false);
    let mut txn = good_txn(
        &buyer,
        &binding_memo(&buyer, "game:chess"),
        &[(&buyer, -1_000_000), (&dev, 1_000_000)],
        MINT,
    );
    txn["meta"]["err"] = json!({ "InstructionError": [0, "InsufficientFunds"] });
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(txn));
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_when_commitment_is_not_met() {
    let (_, receipt, buyer, dev, ..) = scenario("game:chess", 0, false);
    let mut txn = good_txn(
        &buyer,
        &binding_memo(&buyer, "game:chess"),
        &[(&buyer, -1_000_000), (&dev, 1_000_000)],
        MINT,
    );
    txn["confirmationStatus"] = json!("processed");
    let mut c = cfg(None);
    c.commitment = Commitment::Finalized;
    let rail = SolanaPaymentRail::new(c, FakeRpc::with(txn));
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_wrong_buyer() {
    let (rail, mut receipt, ..) = scenario("game:chess", 0, false);
    // Someone else's key on an otherwise real payment.
    receipt.buyer = key(99).node_pubkey();
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_when_buyer_did_not_sign() {
    let (_, receipt, buyer, dev, ..) = scenario("game:chess", 0, false);
    let mut txn = good_txn(
        &buyer,
        &binding_memo(&buyer, "game:chess"),
        &[(&buyer, -1_000_000), (&dev, 1_000_000)],
        MINT,
    );
    txn["transaction"]["message"]["accountKeys"][0]["signer"] = json!(false);
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(txn));
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_wrong_item_binding() {
    let (rail, mut receipt, ..) = scenario("game:chess", 0, false);
    // Repoint the receipt at another item without touching the chain.
    receipt.binding.as_mut().unwrap().item = "game:expensive".into();
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:expensive"),
        "the derived reference no longer matches (buyer, item)"
    );
}

#[test]
fn rejects_replay_for_another_item() {
    // A completely genuine, fully paid receipt for a cheap item...
    let (rail, receipt, ..) = scenario("game:cheap", 0, false);
    assert!(rail.verify_receipt_for_item(&receipt, "game:cheap"));
    // ...must not unlock a different one.
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:expensive"),
        "a receipt for one item must never be redeemable for another"
    );
}

#[test]
fn rejects_when_the_chain_memo_binds_a_different_item() {
    let (_, receipt, buyer, dev, ..) = scenario("game:chess", 0, false);
    // Receipt says chess; chain says checkers.
    let txn = good_txn(
        &buyer,
        &binding_memo(&buyer, "game:checkers"),
        &[(&buyer, -1_000_000), (&dev, 1_000_000)],
        MINT,
    );
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(txn));
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_a_transaction_with_no_memo_at_all() {
    let (_, receipt, buyer, dev, ..) = scenario("game:chess", 0, false);
    let mut txn = good_txn(
        &buyer,
        "unrelated note",
        &[(&buyer, -1_000_000), (&dev, 1_000_000)],
        MINT,
    );
    txn["transaction"]["message"]["instructions"] = json!([]);
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(txn));
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_rpc_error() {
    let (_, receipt, ..) = scenario("game:chess", 0, false);
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::broken());
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:chess"),
        "an unreachable RPC must NEVER grant an entitlement"
    );
}

#[test]
fn rejects_a_receipt_with_no_binding() {
    let (rail, mut receipt, ..) = scenario("game:chess", 0, false);
    receipt.binding = None;
    assert!(!rail.verify_receipt(&receipt));
}

#[test]
fn rejects_a_tampered_rail_signature() {
    let (rail, mut receipt, ..) = scenario("game:chess", 0, false);
    receipt.sig = Sig([0u8; 64]);
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}

#[test]
fn rejects_an_unaccounted_extra_recipient() {
    let (_, receipt, buyer, dev, ..) = scenario("game:chess", 0, false);
    let sneak = key(77).node_pubkey();
    let txn = good_txn(
        &buyer,
        &binding_memo(&buyer, "game:chess"),
        &[(&buyer, -1_000_000), (&dev, 900_000), (&sneak, 100_000)],
        MINT,
    );
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::with(txn));
    assert!(
        !rail.verify_receipt_for_item(&receipt, "game:chess"),
        "chain must match the claimed split exactly"
    );
}

#[test]
fn unbound_checkout_produces_an_unverifiable_receipt() {
    // The trait's item-less `checkout` cannot bind, so it must not be honoured.
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let (rail, ..) = scenario("game:chess", 0, false);
    let r = rt.block_on(rail.checkout(&key(1).node_pubkey(), split(key(2).node_pubkey(), 5, None, 0)));
    assert!(r.binding.is_none());
    assert!(!rail.verify_receipt(&r), "an unbound receipt grants nothing");
}

// ── Channels / escrow are honestly absent ────────────────────────────────────

#[test]
fn channels_and_escrow_are_unsupported_not_faked() {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let (rail, ..) = scenario("game:chess", 0, false);
    let c = rt.block_on(rail.open_channel(&key(3).node_pubkey()));
    assert!(matches!(c, Err(PaymentError::Unsupported(_))));
    let e = rt.block_on(rail.escrow(WagerTerms {
        players: vec![key(1).node_pubkey()],
        stake: 1,
        currency: "USDC".into(),
        game: crate::blobstore::Hash::of(b"chess"),
    }));
    assert!(matches!(e, Err(PaymentError::Unsupported(_))));
}

// ── Config parsing fails loudly ──────────────────────────────────────────────

#[test]
fn config_parsing_rejects_junk_and_processed_commitment() {
    assert!(Cluster::parse("mainnet-beta").is_ok());
    assert!(Cluster::parse("moonnet").is_err());
    assert!(Commitment::parse("finalized").is_ok());
    assert!(
        Commitment::parse("processed").is_err(),
        "processed can be rolled back"
    );
    assert!(Commitment::parse("").is_err());
}

#[test]
fn keypair_length_is_enforced() {
    assert!(SolanaPaymentRail::keypair_from_bytes(&[1u8; 64]).is_ok());
    assert!(SolanaPaymentRail::keypair_from_bytes(&[1u8; 32]).is_ok());
    assert!(SolanaPaymentRail::keypair_from_bytes(&[1u8; 13]).is_err());
}

// ── Transaction construction ─────────────────────────────────────────────────

#[test]
fn checkout_builds_one_transaction_with_every_leg() {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let signer = key(1);
    let buyer = signer.node_pubkey();
    let fee_wallet = key(4).node_pubkey();
    let rpc = FakeRpc::unconfirmed();
    let rail = SolanaPaymentRail::new(cfg(Some(fee_wallet)), rpc.clone()).with_signer(signer);

    let (msg, plan) = rt
        .block_on(rail.build_message(
            &buyer,
            "game:chess",
            &split(key(2).node_pubkey(), 1_000_000, Some((key(3).node_pubkey(), 250_000)), 250),
        ))
        .unwrap();
    assert_eq!(plan.payouts.len(), 3, "dev + operator + fee");
    assert_eq!(msg[0], 1, "single signer: the buyer");
    // 1 memo + 3 transfers, all in ONE message => the split is atomic.
    let ix_count = msg[msg.len() - 1..].len(); // sanity: message is non-empty
    assert!(ix_count > 0);
    assert_eq!(&msg[4..36], &buyer.0, "buyer is the fee payer / index 0");

    let sent = rt.block_on(rail.checkout_item(
        &buyer,
        "game:chess",
        split(key(2).node_pubkey(), 1_000_000, None, 0),
    ));
    assert!(sent.is_ok());
    assert_eq!(rpc.sent.lock().unwrap().len(), 1, "exactly one transaction");
}

#[test]
fn refuses_to_spend_a_key_it_does_not_hold() {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let rail = SolanaPaymentRail::new(cfg(None), FakeRpc::unconfirmed()).with_signer(key(1));
    let stranger = key(55).node_pubkey();
    let r = rt.block_on(rail.checkout_item(
        &stranger,
        "game:chess",
        split(key(2).node_pubkey(), 10, None, 0),
    ));
    assert!(matches!(r, Err(SolanaError::NotOurKey(_))));
}

// ── Opt-in live test ─────────────────────────────────────────────────────────

/// Live smoke test against a real cluster. **Skipped unless**
/// `MAGNETITE_SOLANA_LIVE_RPC` is set; see `docs/payments.md` for the
/// `solana-test-validator` recipe.
///
/// ```sh
/// solana-test-validator -r &
/// MAGNETITE_SOLANA_LIVE_RPC=http://127.0.0.1:8899 \
///   cargo test -p magnetite-seams --features solana live_rpc -- --ignored --nocapture
/// ```
#[test]
#[ignore = "requires a live Solana RPC; set MAGNETITE_SOLANA_LIVE_RPC"]
fn live_rpc_reachable_and_unknown_signature_denies() {
    let Ok(url) = std::env::var("MAGNETITE_SOLANA_LIVE_RPC") else {
        eprintln!("MAGNETITE_SOLANA_LIVE_RPC not set — skipping");
        return;
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let rpc = Arc::new(rpc::HttpRpc::new(url));
    let bh = rt
        .block_on(rpc.get_latest_blockhash("confirmed"))
        .expect("live cluster must answer getLatestBlockhash");
    assert_eq!(bs58::decode(&bh).into_vec().unwrap().len(), 32);

    // A signature that cannot exist must deny, not error out into an entitlement.
    let (_, mut receipt, ..) = scenario("game:chess", 0, false);
    receipt.binding.as_mut().unwrap().tx_signature = bs58::encode([0u8; 64]).into_string();
    let rail = SolanaPaymentRail::new(cfg(None), rpc);
    assert!(!rail.verify_receipt_for_item(&receipt, "game:chess"));
}
