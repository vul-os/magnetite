//! Seam §3.6 — a **real** `PaymentRail`: SPL USDC on Solana.
//!
//! Behind the `solana` cargo feature; the offline [`MockPaymentRail`] stays the
//! default so CI and self-hosting need no chain, no RPC and no money.
//!
//! # Shape
//!
//! * **checkout** builds ONE transaction containing one SPL `TransferChecked`
//!   per party (developer, optional operator, optional protocol fee) plus a Memo
//!   instruction carrying the `(buyer, item)` binding. Because it is a single
//!   transaction, the split is atomic *by construction* — Solana either lands
//!   every leg or none. No custom on-chain program is involved.
//! * **verify_receipt** re-derives the binding and re-reads the transaction from
//!   the cluster. It trusts nothing in the receipt except as a *claim* to be
//!   checked against chain state.
//! * **open_channel / escrow** are NOT implemented — they need real on-chain
//!   programs. They return [`SolanaError::Unsupported`] rather than a
//!   convincing-looking stub. See `docs/payments.md`.
//!
//! # Money math
//!
//! USDC has 6 decimals. Every amount in this module is an integer count of
//! smallest units (micro-USDC). There is no floating point anywhere in the money
//! path, and [`SolanaPaymentRail::plan`] guarantees the parts sum exactly to the
//! total.
//!
//! # Keys
//!
//! The signing key is read from `SOLANA_KEYPAIR_PATH` (a solana-CLI JSON array
//! of 64 bytes, which MUST be `chmod 600`) or `SOLANA_KEYPAIR` (base58 secret
//! key). It is never logged, never serialized and never written anywhere.

pub mod rpc;
pub mod tx;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::identity::{Identity, PubKey, RawKeypairAuth, Sig};
use crate::payment::{
    ChainBinding, Channel, Escrow, PayOut, PaymentError, PaymentRail, PaymentSplit, Receipt,
    WagerTerms,
};

use rpc::SolanaRpc;
use tx::{pubkey_from_base58, pubkey_to_base58, USDC_DECIMALS};

/// Everything that can go wrong on this rail. Every variant is a *refusal*:
/// none of them ever results in an entitlement being granted.
#[derive(Debug, thiserror::Error)]
pub enum SolanaError {
    /// The RPC endpoint was unreachable, slow, or answered with an error.
    #[error("solana rpc: {0}")]
    Rpc(String),
    /// A base58 address failed to decode into 32 bytes.
    #[error("not a valid solana address: {0}")]
    BadAddress(String),
    /// Program-derived-address search exhausted every bump (astronomically unlikely).
    #[error("could not derive associated token address")]
    Derivation,
    /// Misconfiguration — missing mint, bad RPC URL, unusable keypair, ...
    #[error("solana rail misconfigured: {0}")]
    Config(String),
    /// The transaction is not on chain at the configured commitment.
    #[error("transaction not confirmed at commitment {0}")]
    Unconfirmed(String),
    /// Chain state contradicts the receipt.
    #[error("receipt does not match chain state: {0}")]
    Mismatch(String),
    /// The rail holds no key for this buyer, so it cannot sign for them.
    #[error("this rail cannot sign for buyer {0} (non-custodial: build an unsigned tx instead)")]
    NotOurKey(String),
    /// Payment channels / escrow need on-chain programs that do not exist yet.
    #[error("{0} is not supported on the Solana USDC rail (no on-chain program deployed); \
             see docs/payments.md")]
    Unsupported(&'static str),
}

impl From<SolanaError> for PaymentError {
    fn from(e: SolanaError) -> Self {
        match e {
            SolanaError::Unsupported(what) => PaymentError::Unsupported(what),
            other => PaymentError::Rail(other.to_string()),
        }
    }
}

/// Which cluster the rail is pointed at. `MainnetBeta` moves **real money**.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cluster {
    /// Real funds. Real losses.
    MainnetBeta,
    /// Free test USDC.
    Devnet,
    /// Free test USDC.
    Testnet,
    /// `solana-test-validator` on localhost.
    Localnet,
}

impl Cluster {
    /// Parse a cluster name; unknown names are a hard error (never a default).
    pub fn parse(s: &str) -> Result<Self, SolanaError> {
        match s {
            "mainnet-beta" | "mainnet" => Ok(Cluster::MainnetBeta),
            "devnet" => Ok(Cluster::Devnet),
            "testnet" => Ok(Cluster::Testnet),
            "localnet" | "local" => Ok(Cluster::Localnet),
            other => Err(SolanaError::Config(format!("unknown cluster {other:?}"))),
        }
    }
    /// Does this cluster move real money?
    pub fn is_mainnet(&self) -> bool {
        matches!(self, Cluster::MainnetBeta)
    }
}

/// Confirmation level required before a receipt may be honoured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Commitment {
    /// Supermajority-voted. Reasonable for low-value goods.
    Confirmed,
    /// Rooted; cannot be rolled back. Correct for anything valuable.
    Finalized,
}

impl Commitment {
    /// Parse a commitment level. `processed` is deliberately REJECTED: it can be
    /// rolled back, so honouring it would hand out goods for reverted payments.
    pub fn parse(s: &str) -> Result<Self, SolanaError> {
        match s {
            "confirmed" => Ok(Commitment::Confirmed),
            "finalized" => Ok(Commitment::Finalized),
            "processed" => Err(SolanaError::Config(
                "commitment 'processed' can be rolled back and is not accepted".into(),
            )),
            other => Err(SolanaError::Config(format!("unknown commitment {other:?}"))),
        }
    }
    /// The wire string for JSON-RPC.
    pub fn as_str(&self) -> &'static str {
        match self {
            Commitment::Confirmed => "confirmed",
            Commitment::Finalized => "finalized",
        }
    }
}

/// Static configuration for the rail. Built by the caller (the backend reads it
/// from env and fails loudly if it is wrong).
#[derive(Clone, Debug)]
pub struct SolanaConfig {
    /// JSON-RPC endpoint.
    pub rpc_url: String,
    /// Cluster the endpoint belongs to.
    pub cluster: Cluster,
    /// Commitment required for a receipt to verify.
    pub commitment: Commitment,
    /// The USDC mint. Anything paid in another mint is not a payment.
    pub usdc_mint: PubKey,
    /// Where the protocol fee goes. Required whenever `protocol_fee_bps > 0`.
    pub fee_wallet: Option<PubKey>,
}

impl SolanaConfig {
    /// The mint as base58.
    pub fn mint_base58(&self) -> String {
        pubkey_to_base58(&self.usdc_mint)
    }
}

/// A concrete, integer-exact plan for one checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Ordered payouts: developer, [operator], [protocol fee].
    pub payouts: Vec<PayOut>,
    /// The fee component (0 when `protocol_fee_bps == 0`).
    pub protocol_fee: u64,
    /// `sum(payouts)` — checked, not assumed.
    pub total: u64,
}

/// Domain-separated binding reference: `blake3("magnetite-pay-v1" || buyer || item)`.
pub fn binding_reference(buyer: &PubKey, item: &str) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"magnetite-pay-v1");
    h.update(&buyer.0);
    h.update(&(item.len() as u64).to_le_bytes());
    h.update(item.as_bytes());
    *h.finalize().as_bytes()
}

/// The exact memo string a bound transaction must carry.
pub fn binding_memo(buyer: &PubKey, item: &str) -> String {
    format!("magnetite:v1:{}", hex::encode(binding_reference(buyer, item)))
}

/// SPL-USDC-on-Solana payment rail.
pub struct SolanaPaymentRail {
    cfg: SolanaConfig,
    rpc: Arc<dyn SolanaRpc>,
    /// Optional signing key. Present only for wallets this process custodies
    /// (e.g. a treasury). Never logged, never serialized.
    signer: Option<RawKeypairAuth>,
    /// Key that signs receipts (a self-consistency marker, NOT the security
    /// boundary — chain state is).
    rail: RawKeypairAuth,
    runtime: Option<Arc<tokio::runtime::Runtime>>,
}

impl SolanaPaymentRail {
    /// Build a rail over an arbitrary RPC implementation (the unit tests pass a
    /// fake; production passes [`rpc::HttpRpc`]).
    pub fn new(cfg: SolanaConfig, rpc: Arc<dyn SolanaRpc>) -> Self {
        Self {
            cfg,
            rpc,
            signer: None,
            rail: RawKeypairAuth::from_seed(*blake3::hash(b"magnetite-solana-rail").as_bytes()),
            runtime: None,
        }
    }

    /// Attach a signing key so this rail can submit transactions itself.
    pub fn with_signer(mut self, signer: RawKeypairAuth) -> Self {
        self.signer = Some(signer);
        self
    }

    /// Attach a runtime used to drive RPC from the *synchronous* `verify_receipt`.
    /// Without one, a fresh current-thread runtime is created per verification.
    pub fn with_runtime(mut self, rt: Arc<tokio::runtime::Runtime>) -> Self {
        self.runtime = Some(rt);
        self
    }

    /// The rail's receipt-signing public key.
    pub fn rail_pubkey(&self) -> PubKey {
        self.rail.node_pubkey()
    }

    /// The configuration (read-only).
    pub fn config(&self) -> &SolanaConfig {
        &self.cfg
    }

    /// The wallet this rail can sign for, if any.
    pub fn signer_pubkey(&self) -> Option<PubKey> {
        self.signer.as_ref().map(|s| s.node_pubkey())
    }

    /// Load a signing key from `SOLANA_KEYPAIR_PATH` (solana-CLI JSON byte
    /// array; `chmod 600`) or `SOLANA_KEYPAIR` (base58 secret key).
    ///
    /// Returns `Ok(None)` when neither is set — a verify-only rail, which is the
    /// right posture for a server that never spends. The key material is never
    /// logged and the error messages never quote it.
    pub fn signer_from_env() -> Result<Option<RawKeypairAuth>, SolanaError> {
        if let Ok(path) = std::env::var("SOLANA_KEYPAIR_PATH") {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| SolanaError::Config(format!("SOLANA_KEYPAIR_PATH {path}: {e}")))?;
            let bytes: Vec<u8> = serde_json::from_str(&raw).map_err(|_| {
                SolanaError::Config(format!("SOLANA_KEYPAIR_PATH {path}: not a JSON byte array"))
            })?;
            return Ok(Some(Self::keypair_from_bytes(&bytes)?));
        }
        if let Ok(b58) = std::env::var("SOLANA_KEYPAIR") {
            let bytes = bs58::decode(b58.trim())
                .into_vec()
                .map_err(|_| SolanaError::Config("SOLANA_KEYPAIR: not base58".into()))?;
            return Ok(Some(Self::keypair_from_bytes(&bytes)?));
        }
        Ok(None)
    }

    fn keypair_from_bytes(bytes: &[u8]) -> Result<RawKeypairAuth, SolanaError> {
        // Solana keypairs are 64 bytes: 32-byte seed followed by the public key.
        let seed: [u8; 32] = match bytes.len() {
            64 => bytes[..32].try_into().unwrap(),
            32 => bytes.try_into().unwrap(),
            n => {
                return Err(SolanaError::Config(format!(
                    "keypair must be 32 or 64 bytes, got {n}"
                )))
            }
        };
        Ok(RawKeypairAuth::from_seed(seed))
    }

    /// Integer-exact split. The fee is taken **on top of** the subtotal, matching
    /// the mock rail, and the parts are asserted to sum to the total.
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

    /// Build the unsigned transaction message for a bound checkout.
    ///
    /// This is the non-custodial path: hand the bytes to the buyer's wallet, let
    /// *them* sign and submit, then call [`Self::receipt_for_signature`].
    pub async fn build_message(
        &self,
        buyer: &PubKey,
        item: &str,
        split: &PaymentSplit,
    ) -> Result<(Vec<u8>, Plan), SolanaError> {
        let plan = self.plan(split)?;
        let blockhash = self
            .rpc
            .get_latest_blockhash(self.cfg.commitment.as_str())
            .await?;
        let bh: [u8; 32] = bs58::decode(&blockhash)
            .into_vec()
            .ok()
            .and_then(|v| <[u8; 32]>::try_from(v).ok())
            .ok_or_else(|| SolanaError::Rpc("blockhash is not 32 bytes".into()))?;

        let source = tx::associated_token_address(buyer, &self.cfg.usdc_mint)?;
        let mut ixs = vec![tx::memo(*buyer, &binding_memo(buyer, item))];
        for p in &plan.payouts {
            if p.amount == 0 {
                continue;
            }
            let dest = tx::associated_token_address(&p.wallet, &self.cfg.usdc_mint)?;
            ixs.push(tx::transfer_checked(
                source,
                self.cfg.usdc_mint,
                dest,
                *buyer,
                p.amount,
                USDC_DECIMALS,
            ));
        }
        Ok((tx::serialize_message(buyer, &ixs, &bh), plan))
    }

    /// Build → sign → submit, then return the bound receipt.
    ///
    /// Only possible when this process holds `buyer`'s key; otherwise
    /// [`SolanaError::NotOurKey`], because the rail is non-custodial and will not
    /// pretend to spend money it cannot move.
    pub async fn checkout_item(
        &self,
        buyer: &PubKey,
        item: &str,
        split: PaymentSplit,
    ) -> Result<Receipt, SolanaError> {
        let signer = self
            .signer
            .as_ref()
            .filter(|s| s.node_pubkey() == *buyer)
            .ok_or_else(|| SolanaError::NotOurKey(pubkey_to_base58(buyer)))?;

        let (message, plan) = self.build_message(buyer, item, &split).await?;
        let sig = signer.sign(&message);
        let wire = tx::wire_transaction(&sig.0, &message);
        let signature = self.rpc.send_transaction(&b64(&wire)).await?;

        Ok(self.receipt_for_signature(buyer, item, &plan, &signature))
    }

    /// Assemble the receipt for an already-submitted transaction. The receipt is
    /// only a *claim*; [`Self::verify_receipt`] is what makes it worth anything.
    pub fn receipt_for_signature(
        &self,
        buyer: &PubKey,
        item: &str,
        plan: &Plan,
        signature: &str,
    ) -> Receipt {
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
                tx_signature: signature.to_string(),
                item: item.to_string(),
                mint: self.cfg.mint_base58(),
                reference: hex::encode(binding_reference(buyer, item)),
            }),
        };
        r.sig = self.rail.sign(&r.signing_bytes());
        r
    }

    // ── Verification ─────────────────────────────────────────────────────────

    /// The full check, async. **Every** error path means "do not grant".
    pub async fn verify_receipt_async(
        &self,
        r: &Receipt,
        expect_item: Option<&str>,
    ) -> Result<(), SolanaError> {
        let b = r
            .binding
            .as_ref()
            .ok_or_else(|| SolanaError::Mismatch("receipt carries no chain binding".into()))?;

        // 1. Right chain, right mint. A USDC receipt is not a receipt in some
        //    other token the buyer happened to have.
        if b.chain != "solana" {
            return Err(SolanaError::Mismatch(format!("chain {:?}", b.chain)));
        }
        if b.mint != self.cfg.mint_base58() {
            return Err(SolanaError::Mismatch("claimed mint is not the configured USDC mint".into()));
        }

        // 2. The binding must be the one derived from (buyer, item) — a receipt
        //    cannot be re-pointed at a different item by editing a field.
        let expected_ref = hex::encode(binding_reference(&r.buyer, &b.item));
        if b.reference != expected_ref {
            return Err(SolanaError::Mismatch("binding reference does not match (buyer, item)".into()));
        }
        // 3. ...and it must be the item the CALLER is asking about. This is what
        //    stops a real, fully-valid receipt for a cheap item unlocking an
        //    expensive one.
        if let Some(item) = expect_item {
            if b.item != item {
                return Err(SolanaError::Mismatch(format!(
                    "receipt is bound to item {:?}, not {:?}",
                    b.item, item
                )));
            }
        }

        // 4. Internal arithmetic + rail signature (cheap, local).
        let sum: u64 = r.payouts.iter().try_fold(0u64, |a, p| a.checked_add(p.amount))
            .ok_or_else(|| SolanaError::Mismatch("payouts overflow".into()))?;
        if sum != r.total {
            return Err(SolanaError::Mismatch("payouts do not sum to total".into()));
        }
        if !<RawKeypairAuth as Identity>::verify(&r.rail_pubkey, &r.signing_bytes(), &r.sig) {
            return Err(SolanaError::Mismatch("receipt signature invalid".into()));
        }

        // 5. Chain state. Unreachable RPC propagates as Err → caller denies.
        let txn = self
            .rpc
            .get_transaction(&b.tx_signature, self.cfg.commitment.as_str())
            .await?
            .ok_or_else(|| SolanaError::Unconfirmed(self.cfg.commitment.as_str().to_string()))?;

        // 6. The transaction must have SUCCEEDED. A landed-but-failed tx moved
        //    nothing.
        match txn.get("meta").and_then(|m| m.get("err")) {
            None => return Err(SolanaError::Mismatch("no meta.err field".into())),
            Some(e) if !e.is_null() => {
                return Err(SolanaError::Mismatch(format!("transaction failed: {e}")))
            }
            _ => {}
        }
        // Some RPCs echo a confirmationStatus; if present it must be good enough.
        if let Some(status) = txn.get("confirmationStatus").and_then(|v| v.as_str()) {
            let ok = match self.cfg.commitment {
                Commitment::Confirmed => status == "confirmed" || status == "finalized",
                Commitment::Finalized => status == "finalized",
            };
            if !ok {
                return Err(SolanaError::Unconfirmed(status.to_string()));
            }
        }

        // 7. The buyer must have signed. Otherwise anyone could point a receipt
        //    at someone else's payment.
        let signed = txn
            .get("transaction")
            .and_then(|t| t.get("message"))
            .and_then(|m| m.get("accountKeys"))
            .and_then(|k| k.as_array())
            .map(|keys| {
                let want = pubkey_to_base58(&r.buyer);
                keys.iter().any(|k| {
                    k.get("pubkey").and_then(|v| v.as_str()) == Some(want.as_str())
                        && k.get("signer").and_then(|v| v.as_bool()) == Some(true)
                })
            })
            .unwrap_or(false);
        if !signed {
            return Err(SolanaError::Mismatch("buyer did not sign the transaction".into()));
        }

        // 8. The on-chain memo must be exactly the derived binding, so a
        //    transaction is redeemable for one (buyer, item) and nothing else.
        let want_memo = binding_memo(&r.buyer, &b.item);
        if !memo_matches(&txn, &want_memo) {
            return Err(SolanaError::Mismatch(
                "transaction carries no memo binding it to this (buyer, item)".into(),
            ));
        }

        // 9. The money. Net token-balance deltas for the configured mint must be
        //    EXACTLY: buyer -total, and +amount for each claimed recipient. Using
        //    balance deltas (rather than reading instructions) means a transfer
        //    that is cancelled out by a hidden reverse transfer in the same
        //    transaction cannot pass.
        let deltas = mint_deltas(&txn, &self.cfg.mint_base58())?;
        let buyer_b58 = pubkey_to_base58(&r.buyer);
        let mut expected: Vec<(String, i128)> = Vec::new();
        for p in &r.payouts {
            if p.amount == 0 {
                continue;
            }
            let who = pubkey_to_base58(&p.wallet);
            if let Some(e) = expected.iter_mut().find(|e| e.0 == who) {
                e.1 += p.amount as i128;
            } else {
                expected.push((who, p.amount as i128));
            }
        }
        expected.push((buyer_b58.clone(), -(r.total as i128)));

        for (who, want) in &expected {
            let got = deltas
                .iter()
                .find(|(o, _)| o == who)
                .map(|(_, d)| *d)
                .unwrap_or(0);
            if got != *want {
                return Err(SolanaError::Mismatch(format!(
                    "{who} received {got} micro-USDC, receipt claims {want}"
                )));
            }
        }
        // No unaccounted party may have gained or lost this mint in this tx.
        for (who, d) in &deltas {
            if *d != 0 && !expected.iter().any(|(e, _)| e == who) {
                return Err(SolanaError::Mismatch(format!(
                    "unaccounted balance change for {who}"
                )));
            }
        }
        Ok(())
    }

    fn block_on<F>(&self, fut: F) -> Result<F::Output, SolanaError>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // `verify_receipt` is synchronous by seam contract but this rail must do
        // I/O. Drive the future on a runtime and wait on a channel: unlike
        // `Handle::block_on` this does not panic when called from inside an
        // async context.
        match &self.runtime {
            Some(rt) => {
                let (tx, rx) = std::sync::mpsc::channel();
                rt.spawn(async move {
                    let _ = tx.send(fut.await);
                });
                rx.recv()
                    .map_err(|_| SolanaError::Rpc("verification task dropped".into()))
            }
            None => std::thread::scope(|s| {
                s.spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| SolanaError::Rpc(format!("runtime: {e}")))
                        .map(|rt| rt.block_on(fut))
                })
                .join()
                .map_err(|_| SolanaError::Rpc("verification thread panicked".into()))?
            }),
        }
    }

    fn verify_blocking(&self, r: &Receipt, item: Option<String>) -> bool {
        // Everything the async check needs, owned, so the future is 'static.
        let rpc = self.rpc.clone();
        let cfg = self.cfg.clone();
        let rail_pubkey = self.rail.node_pubkey();
        let receipt = r.clone();
        let fut = async move {
            let probe = SolanaPaymentRail {
                cfg,
                rpc,
                signer: None,
                rail: RawKeypairAuth::from_seed(
                    *blake3::hash(b"magnetite-solana-rail").as_bytes(),
                ),
                runtime: None,
            };
            debug_assert_eq!(probe.rail.node_pubkey(), rail_pubkey);
            probe.verify_receipt_async(&receipt, item.as_deref()).await
        };
        match self.block_on(fut) {
            Ok(Ok(())) => true,
            // Unreachable RPC, unconfirmed, mismatch, panic — all deny.
            Ok(Err(_)) | Err(_) => false,
        }
    }
}

fn b64(bytes: &[u8]) -> String {
    // Tiny, dependency-free base64 (standard alphabet, padded).
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
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
        .map(|groups| {
            groups
                .iter()
                .any(|g| scan(g.get("instructions"), want))
        })
        .unwrap_or(false)
}

/// Net per-owner balance change for `mint`, in integer smallest units.
fn mint_deltas(
    txn: &serde_json::Value,
    mint: &str,
) -> Result<Vec<(String, i128)>, SolanaError> {
    let meta = txn
        .get("meta")
        .ok_or_else(|| SolanaError::Mismatch("transaction has no meta".into()))?;
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
                .ok_or_else(|| SolanaError::Mismatch("token balance without owner".into()))?;
            let amount: i128 = e
                .get("uiTokenAmount")
                .and_then(|u| u.get("amount"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| SolanaError::Mismatch("token balance without amount".into()))?
                .parse()
                .map_err(|_| SolanaError::Mismatch("token amount is not an integer".into()))?;
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
    /// verification. Use [`SolanaPaymentRail::checkout_item`].
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
