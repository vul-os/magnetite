// Wise (TransferWise) API client — fiat developer payouts.
//
// Env vars:
//   WISE_API_TOKEN  — required in production; if absent and not sandbox → explicit HTTP 502.
//   WISE_PROFILE_ID — required in production (numeric Wise profile/account id).
//   WISE_SANDBOX    — set to "true" to use sandbox.transferwise.com and skip real API calls.
//
// Sandbox mode returns clearly-labelled fake ids prefixed with "sandbox_" so callers can tell
// they are not interacting with the real Wise API.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WiseClient {
    api_token: Option<String>,
    profile_id: Option<String>,
    sandbox: bool,
    http: reqwest::Client,
}

impl WiseClient {
    /// Build from environment variables.
    pub fn from_env() -> Self {
        let api_token = std::env::var("WISE_API_TOKEN")
            .ok()
            .filter(|v| !v.is_empty());
        let profile_id = std::env::var("WISE_PROFILE_ID")
            .ok()
            .filter(|v| !v.is_empty());
        let sandbox = std::env::var("WISE_SANDBOX")
            .map(|v| v == "true")
            .unwrap_or(false);
        Self {
            api_token,
            profile_id,
            sandbox,
            http: reqwest::Client::new(),
        }
    }

    fn base_url(&self) -> &'static str {
        if self.sandbox {
            "https://api.sandbox.transferwise.tech"
        } else {
            "https://api.transferwise.com"
        }
    }

    /// Return the API token or an error if not configured (and not in sandbox mode).
    fn token(&self) -> Result<&str> {
        if self.sandbox {
            // In sandbox mode we might still have a token (optional) — proceed either way.
            return Ok(self.api_token.as_deref().unwrap_or("sandbox_token"));
        }
        self.api_token.as_deref().ok_or_else(|| {
            AppError::Internal(
                "payouts not configured: WISE_API_TOKEN is unset (set WISE_SANDBOX=true for local dev)"
                    .to_string(),
            )
        })
    }

    /// Return the numeric profile id or an error.
    fn profile_id(&self) -> Result<&str> {
        if self.sandbox {
            return Ok(self.profile_id.as_deref().unwrap_or("sandbox_profile"));
        }
        self.profile_id.as_deref().ok_or_else(|| {
            AppError::Internal("payouts not configured: WISE_PROFILE_ID is unset".to_string())
        })
    }
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Input describing the recipient's bank or email details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipientDetails {
    /// Wise accountHolderName
    pub account_holder_name: String,
    /// ISO 3166-1 alpha-2 country of the bank account
    pub country: String,
    /// ISO 4217 currency code (e.g. "USD", "EUR", "GBP")
    pub currency: String,
    /// Bank account type for ACH: "checking" or "savings"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    /// US routing number (ABA) — for ACH/USD transfers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_number: Option<String>,
    /// Bank account number — for ACH/USD transfers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number: Option<String>,
    /// PayPal / email payout address (for EMAIL type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// IBAN — for SEPA/international transfers (EUR, GBP sort-code IBAN, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iban: Option<String>,
    /// BIC/SWIFT code — required for most IBAN transfers outside SEPA
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bic: Option<String>,
}

/// A Wise recipient that has been created and stored.
/// Public surface type — used externally by callers that store recipient metadata.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiseRecipient {
    pub wise_recipient_id: String,
    pub account_holder_name: String,
    pub currency: String,
}

/// A Wise quote.
#[derive(Debug, Clone)]
pub struct WiseQuote {
    pub quote_id: String,
}

/// A Wise transfer.
#[derive(Debug, Clone)]
pub struct WiseTransfer {
    pub transfer_id: String,
}

// ---------------------------------------------------------------------------
// API calls
// ---------------------------------------------------------------------------

impl WiseClient {
    /// Create a Wise recipient account for a developer.
    /// Returns the Wise-assigned recipient id string.
    pub async fn create_recipient(&self, details: &RecipientDetails) -> Result<String> {
        let token = self.token()?;

        if self.sandbox {
            let fake_id = format!("sandbox_recipient_{}", uuid::Uuid::new_v4());
            tracing::info!(
                "[WISE SANDBOX] create_recipient for '{}' → {}",
                details.account_holder_name,
                fake_id
            );
            return Ok(fake_id);
        }

        let profile_id = self.profile_id()?;

        // Determine Wise account type — EMAIL, IBAN (SEPA/international), or ABA (US ACH).
        // Priority: email > iban > aba.
        let (type_str, type_details) = if details.email.is_some() {
            (
                "email",
                serde_json::json!({
                    "email": details.email
                }),
            )
        } else if details.iban.is_some() {
            // IBAN type: used for SEPA EUR transfers and GBP/other SWIFT-IBAN routes.
            // Wise requires legalType + IBAN; BIC is optional for SEPA but required for
            // non-SEPA routes (e.g., USD SWIFT via IBAN). Include it when present.
            let mut iban_details = serde_json::json!({
                "legalType": "PRIVATE",
                "IBAN": details.iban,
            });
            if let Some(ref bic) = details.bic {
                iban_details["BIC"] = serde_json::Value::String(bic.clone());
            }
            ("iban", iban_details)
        } else {
            (
                "aba",
                serde_json::json!({
                    "legalType": "PRIVATE",
                    "accountType": details.account_type.as_deref().unwrap_or("checking"),
                    "abartn": details.routing_number,
                    "accountNumber": details.account_number,
                }),
            )
        };

        let body = serde_json::json!({
            "currency": details.currency,
            "type": type_str,
            "profile": profile_id.parse::<u64>().unwrap_or(0),
            "accountHolderName": details.account_holder_name,
            "details": type_details,
        });

        let resp = self
            .http
            .post(format!("{}/v1/accounts", self.base_url()))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AppError::Internal(format!("Wise create_recipient request failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Wise create_recipient failed (HTTP {status}): {text}"
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Wise create_recipient parse error: {e}")))?;

        let id = json["id"]
            .as_u64()
            .map(|n| n.to_string())
            .or_else(|| json["id"].as_str().map(|s| s.to_string()))
            .ok_or_else(|| {
                AppError::Internal("Wise create_recipient: missing id in response".to_string())
            })?;

        Ok(id)
    }

    /// Create a quote for sending `amount` USD to the recipient's currency.
    pub async fn create_quote(
        &self,
        source_currency: &str,
        target_currency: &str,
        amount: rust_decimal::Decimal,
    ) -> Result<WiseQuote> {
        let token = self.token()?;

        if self.sandbox {
            let fake_id = format!("sandbox_quote_{}", uuid::Uuid::new_v4());
            tracing::info!(
                "[WISE SANDBOX] create_quote {} {} {} → {}",
                amount,
                source_currency,
                target_currency,
                fake_id
            );
            return Ok(WiseQuote { quote_id: fake_id });
        }

        let profile_id = self.profile_id()?;

        let body = serde_json::json!({
            "sourceCurrency": source_currency,
            "targetCurrency": target_currency,
            "sourceAmount": amount,
            "profile": profile_id.parse::<u64>().unwrap_or(0),
            "payOut": "BANK_TRANSFER",
        });

        let resp = self
            .http
            .post(format!(
                "{}/v3/profiles/{}/quotes",
                self.base_url(),
                profile_id
            ))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Wise create_quote request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Wise create_quote failed (HTTP {status}): {text}"
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Wise create_quote parse error: {e}")))?;

        let id = json["id"].as_str().map(|s| s.to_string()).ok_or_else(|| {
            AppError::Internal("Wise create_quote: missing id in response".to_string())
        })?;

        Ok(WiseQuote { quote_id: id })
    }

    /// Create a transfer linking a quote to a recipient.
    pub async fn create_transfer(
        &self,
        quote: &WiseQuote,
        recipient_id: &str,
        reference: &str,
    ) -> Result<WiseTransfer> {
        let token = self.token()?;

        if self.sandbox {
            let fake_id = format!("sandbox_transfer_{}", uuid::Uuid::new_v4());
            tracing::info!(
                "[WISE SANDBOX] create_transfer quote={} recipient={} → {}",
                quote.quote_id,
                recipient_id,
                fake_id
            );
            return Ok(WiseTransfer {
                transfer_id: fake_id,
            });
        }

        let idempotency_uuid = uuid::Uuid::new_v4().to_string();

        let body = serde_json::json!({
            "targetAccount": recipient_id.parse::<u64>().unwrap_or(0),
            "quoteUuid": quote.quote_id,
            "customerTransactionId": idempotency_uuid,
            "details": {
                "reference": reference,
            },
        });

        let resp = self
            .http
            .post(format!("{}/v1/transfers", self.base_url()))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Wise create_transfer request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Wise create_transfer failed (HTTP {status}): {text}"
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Wise create_transfer parse error: {e}")))?;

        let id = json["id"]
            .as_u64()
            .map(|n| n.to_string())
            .or_else(|| json["id"].as_str().map(|s| s.to_string()))
            .ok_or_else(|| {
                AppError::Internal("Wise create_transfer: missing id in response".to_string())
            })?;

        Ok(WiseTransfer { transfer_id: id })
    }

    /// Fund (execute) a transfer. This debits the Wise balance and initiates the bank transfer.
    pub async fn fund_transfer(&self, transfer: &WiseTransfer) -> Result<()> {
        let token = self.token()?;

        if self.sandbox {
            tracing::info!(
                "[WISE SANDBOX] fund_transfer {} — simulated success",
                transfer.transfer_id
            );
            return Ok(());
        }

        let profile_id = self.profile_id()?;

        let body = serde_json::json!({ "type": "BALANCE" });

        let resp = self
            .http
            .post(format!(
                "{}/v3/profiles/{}/transfers/{}/payments",
                self.base_url(),
                profile_id,
                transfer.transfer_id
            ))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Wise fund_transfer request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Wise fund_transfer failed (HTTP {status}): {text}"
            )));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Unit tests (pure logic, no network)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox_client() -> WiseClient {
        WiseClient {
            api_token: None,
            profile_id: None,
            sandbox: true,
            http: reqwest::Client::new(),
        }
    }

    fn unconfigured_client() -> WiseClient {
        WiseClient {
            api_token: None,
            profile_id: None,
            sandbox: false,
            http: reqwest::Client::new(),
        }
    }

    #[test]
    fn sandbox_base_url() {
        let c = sandbox_client();
        assert_eq!(c.base_url(), "https://api.sandbox.transferwise.tech");
    }

    #[test]
    fn prod_base_url() {
        let c = WiseClient {
            api_token: Some("tok".into()),
            profile_id: Some("123".into()),
            sandbox: false,
            http: reqwest::Client::new(),
        };
        assert_eq!(c.base_url(), "https://api.transferwise.com");
    }

    #[test]
    fn unconfigured_token_returns_error() {
        let c = unconfigured_client();
        let err = c.token().unwrap_err();
        assert!(
            err.to_string().contains("payouts not configured"),
            "expected 'payouts not configured' in: {}",
            err
        );
    }

    #[test]
    fn sandbox_token_ok_without_env() {
        let c = sandbox_client();
        assert!(c.token().is_ok());
    }

    #[tokio::test]
    async fn sandbox_create_recipient_returns_sandbox_prefix() {
        let c = sandbox_client();
        let details = RecipientDetails {
            account_holder_name: "Alice Dev".into(),
            country: "US".into(),
            currency: "USD".into(),
            account_type: Some("checking".into()),
            routing_number: Some("110000000".into()),
            account_number: Some("000123456789".into()),
            email: None,
            iban: None,
            bic: None,
        };
        let id = c.create_recipient(&details).await.unwrap();
        assert!(id.starts_with("sandbox_recipient_"), "got: {id}");
    }

    #[tokio::test]
    async fn sandbox_create_recipient_iban_returns_sandbox_prefix() {
        let c = sandbox_client();
        let details = RecipientDetails {
            account_holder_name: "Klaus Müller".into(),
            country: "DE".into(),
            currency: "EUR".into(),
            account_type: None,
            routing_number: None,
            account_number: None,
            email: None,
            iban: Some("DE89370400440532013000".into()),
            bic: Some("COBADEFFXXX".into()),
        };
        let id = c.create_recipient(&details).await.unwrap();
        assert!(id.starts_with("sandbox_recipient_"), "got: {id}");
    }

    #[tokio::test]
    async fn sandbox_create_quote_returns_sandbox_prefix() {
        let c = sandbox_client();
        let q = c
            .create_quote("USD", "USD", rust_decimal::Decimal::new(5000, 2))
            .await
            .unwrap();
        assert!(
            q.quote_id.starts_with("sandbox_quote_"),
            "got: {}",
            q.quote_id
        );
    }

    #[tokio::test]
    async fn sandbox_create_transfer_returns_sandbox_prefix() {
        let c = sandbox_client();
        let quote = WiseQuote {
            quote_id: "sandbox_quote_abc".into(),
        };
        let t = c
            .create_transfer(&quote, "sandbox_recipient_xyz", "payout-ref-001")
            .await
            .unwrap();
        assert!(
            t.transfer_id.starts_with("sandbox_transfer_"),
            "got: {}",
            t.transfer_id
        );
    }

    #[tokio::test]
    async fn sandbox_fund_transfer_ok() {
        let c = sandbox_client();
        let transfer = WiseTransfer {
            transfer_id: "sandbox_transfer_abc".into(),
        };
        assert!(c.fund_transfer(&transfer).await.is_ok());
    }

    #[tokio::test]
    async fn unconfigured_create_recipient_fails_with_clear_error() {
        let c = unconfigured_client();
        let details = RecipientDetails {
            account_holder_name: "Bob".into(),
            country: "US".into(),
            currency: "USD".into(),
            account_type: None,
            routing_number: None,
            account_number: None,
            email: Some("bob@example.com".into()),
            iban: None,
            bic: None,
        };
        let err = c.create_recipient(&details).await.unwrap_err();
        assert!(
            err.to_string().contains("payouts not configured"),
            "expected 'payouts not configured' in: {}",
            err
        );
    }
}
