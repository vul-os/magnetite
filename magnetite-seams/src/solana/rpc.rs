//! The JSON-RPC seam *inside* the Solana seam.
//!
//! Every network call the rail makes goes through [`SolanaRpc`], so the whole
//! verification path can be unit-tested offline against a fake. CI never opens a
//! socket; only [`HttpRpc`] does, and only when actually configured.

use async_trait::async_trait;

use super::SolanaError;

/// Minimal Solana JSON-RPC surface used by the payment rail.
#[async_trait]
pub trait SolanaRpc: Send + Sync {
    /// `getTransaction` with `jsonParsed` encoding at `commitment`.
    ///
    /// `Ok(None)` means "the cluster does not know this signature at this
    /// commitment" — i.e. unconfirmed. `Err` means the RPC could not be reached
    /// or answered garbage. **Both must fail closed at the call site.**
    async fn get_transaction(
        &self,
        signature: &str,
        commitment: &str,
    ) -> Result<Option<serde_json::Value>, SolanaError>;

    /// `getLatestBlockhash` — base58 blockhash for transaction construction.
    async fn get_latest_blockhash(&self, commitment: &str) -> Result<String, SolanaError>;

    /// `sendTransaction` of a base64 wire transaction; returns the signature.
    async fn send_transaction(&self, wire_base64: &str) -> Result<String, SolanaError>;
}

/// Real HTTP JSON-RPC client.
#[cfg(feature = "solana")]
pub struct HttpRpc {
    url: String,
    http: reqwest::Client,
}

#[cfg(feature = "solana")]
impl HttpRpc {
    /// Build a client for an RPC endpoint (e.g. `https://api.devnet.solana.com`).
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SolanaError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
        });
        let resp = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SolanaError::Rpc(format!("{method}: {e}")))?;
        if !resp.status().is_success() {
            return Err(SolanaError::Rpc(format!("{method}: HTTP {}", resp.status())));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SolanaError::Rpc(format!("{method}: bad JSON: {e}")))?;
        if let Some(err) = v.get("error") {
            return Err(SolanaError::Rpc(format!("{method}: {err}")));
        }
        v.get("result")
            .cloned()
            .ok_or_else(|| SolanaError::Rpc(format!("{method}: no result")))
    }
}

#[cfg(feature = "solana")]
#[async_trait]
impl SolanaRpc for HttpRpc {
    async fn get_transaction(
        &self,
        signature: &str,
        commitment: &str,
    ) -> Result<Option<serde_json::Value>, SolanaError> {
        let result = self
            .call(
                "getTransaction",
                serde_json::json!([
                    signature,
                    {
                        "encoding": "jsonParsed",
                        "commitment": commitment,
                        "maxSupportedTransactionVersion": 0,
                    }
                ]),
            )
            .await?;
        if result.is_null() {
            return Ok(None);
        }
        Ok(Some(result))
    }

    async fn get_latest_blockhash(&self, commitment: &str) -> Result<String, SolanaError> {
        let result = self
            .call(
                "getLatestBlockhash",
                serde_json::json!([{ "commitment": commitment }]),
            )
            .await?;
        result
            .get("value")
            .and_then(|v| v.get("blockhash"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| SolanaError::Rpc("getLatestBlockhash: no blockhash".into()))
    }

    async fn send_transaction(&self, wire_base64: &str) -> Result<String, SolanaError> {
        let result = self
            .call(
                "sendTransaction",
                serde_json::json!([wire_base64, { "encoding": "base64" }]),
            )
            .await?;
        result
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| SolanaError::Rpc("sendTransaction: no signature".into()))
    }
}
