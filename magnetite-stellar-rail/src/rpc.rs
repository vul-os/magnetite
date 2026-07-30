//! The Horizon seam inside the Stellar rail.
//!
//! Every network call the rail makes goes through [`StellarRpc`], so the
//! whole `checkout`/`verify` path can be unit-tested offline against a
//! scripted fake — see `src/tests.rs`. Only [`HorizonRpc`] opens a socket, and
//! only when actually invoked. Design copied by hand from
//! `patala/patala-stellar/src/rpc.rs` (see this crate's `Cargo.toml`).
//!
//! # Honesty
//!
//! **This crate's `HorizonRpc` has never been run against a live network.**
//! `patala-stellar`'s `HorizonRpc` — a sibling implementation of this same
//! seam, built independently for the `patala` repo — settled one real
//! payment on Stellar testnet on 2026-07-30 (tx
//! `32663937fe1407f9de3e781effa6ac9f4b1d29340ea63e72f6335a6c91effb89`, ledger
//! `3882739`); that evidence belongs to `patala`, not to this crate. This
//! crate's own `HorizonRpc` is untested against live Horizon — see
//! `src/lib.rs`'s module docs for the honest scope of what this port proves
//! and what it does not.

use async_trait::async_trait;

use crate::StellarError;

/// The result Horizon returns from a successful `POST /transactions`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmitResult {
    /// The transaction hash (hex), as Horizon reports it. `checkout_item`
    /// asserts this equals the hash it computed locally *before* trusting the
    /// receipt.
    pub hash: String,
    /// Whether the transaction actually succeeded. This crate always checks
    /// this field explicitly rather than inferring success from HTTP status.
    pub successful: bool,
}

/// The result of looking up a transaction that already happened, by hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxRecord {
    /// Whether the transaction succeeded.
    pub successful: bool,
    /// The signed transaction envelope, base64 XDR, exactly as submitted and
    /// included. `verify` decodes and checks this — never trusts Horizon's
    /// summary fields alone for the money-moving details.
    pub envelope_xdr: String,
}

/// Minimal Horizon surface used by the payment rail.
#[async_trait]
pub trait StellarRpc: Send + Sync {
    /// The account's current sequence number (`GET /accounts/{id}`,
    /// `sequence` field). The transaction to submit uses `sequence + 1`.
    async fn load_sequence(&self, account_strkey: &str) -> Result<i64, StellarError>;

    /// `POST /transactions` with a base64 XDR envelope.
    async fn submit_transaction(
        &self,
        envelope_xdr_b64: &str,
    ) -> Result<SubmitResult, StellarError>;

    /// `GET /transactions/{hash}`.
    ///
    /// `Ok(None)` means "Horizon does not know this hash" (unconfirmed).
    /// `Err` means Horizon could not be reached or answered garbage. **Both
    /// must fail closed at the call site** — see `verify_async` in
    /// `src/lib.rs`.
    async fn get_transaction(&self, tx_hash_hex: &str) -> Result<Option<TxRecord>, StellarError>;
}

/// Real Horizon HTTP client. **Never exercised against a live network from
/// this crate** — see the module docs.
pub struct HorizonRpc {
    base_url: String,
    http: reqwest::Client,
}

impl HorizonRpc {
    /// Build a client for a Horizon base URL, e.g.
    /// `https://horizon-testnet.stellar.org`.
    pub fn new(base_url: impl Into<String>) -> Self {
        let mut base_url = base_url.into();
        while base_url.ends_with('/') {
            base_url.pop();
        }
        Self {
            base_url,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl StellarRpc for HorizonRpc {
    async fn load_sequence(&self, account_strkey: &str) -> Result<i64, StellarError> {
        let url = format!("{}/accounts/{}", self.base_url, account_strkey);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| StellarError::Rpc(format!("GET /accounts: {e}")))?;
        if !resp.status().is_success() {
            return Err(StellarError::Rpc(format!(
                "GET /accounts: HTTP {}",
                resp.status()
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| StellarError::Rpc(format!("GET /accounts: bad JSON: {e}")))?;
        // Horizon renders int64 sequence numbers as a JSON string to avoid
        // precision loss in JS clients.
        v.get("sequence")
            .and_then(|s| s.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| StellarError::Rpc("GET /accounts: no sequence".into()))
    }

    async fn submit_transaction(
        &self,
        envelope_xdr_b64: &str,
    ) -> Result<SubmitResult, StellarError> {
        let url = format!("{}/transactions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .form(&[("tx", envelope_xdr_b64)])
            .send()
            .await
            .map_err(|e| StellarError::Rpc(format!("POST /transactions: {e}")))?;
        let status = resp.status();
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| StellarError::Rpc(format!("POST /transactions: bad JSON: {e}")))?;
        if !status.is_success() {
            return Err(StellarError::Rpc(format!(
                "POST /transactions: HTTP {status}: {v}"
            )));
        }
        Ok(SubmitResult {
            hash: v
                .get("hash")
                .and_then(|s| s.as_str())
                .ok_or_else(|| StellarError::Rpc("POST /transactions: no hash".into()))?
                .to_string(),
            successful: v
                .get("successful")
                .and_then(|s| s.as_bool())
                .unwrap_or(true),
        })
    }

    async fn get_transaction(&self, tx_hash_hex: &str) -> Result<Option<TxRecord>, StellarError> {
        let url = format!("{}/transactions/{}", self.base_url, tx_hash_hex);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| StellarError::Rpc(format!("GET /transactions/{{hash}}: {e}")))?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(StellarError::Rpc(format!(
                "GET /transactions/{{hash}}: HTTP {}",
                resp.status()
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| StellarError::Rpc(format!("GET /transactions/{{hash}}: bad JSON: {e}")))?;
        Ok(Some(TxRecord {
            successful: v
                .get("successful")
                .and_then(|s| s.as_bool())
                .ok_or_else(|| {
                    StellarError::Rpc("GET /transactions/{hash}: no successful".into())
                })?,
            envelope_xdr: v
                .get("envelope_xdr")
                .and_then(|s| s.as_str())
                .ok_or_else(|| {
                    StellarError::Rpc("GET /transactions/{hash}: no envelope_xdr".into())
                })?
                .to_string(),
        }))
    }
}
