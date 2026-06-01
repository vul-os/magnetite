# Wise Payouts — IBAN Support

Magnetite pays out developer earnings via the Wise (TransferWise) API. Three payout recipient types are supported:

| Type | When to use | Required fields |
|------|-------------|-----------------|
| `email` | PayPal-style / Wise-balance recipient | `email` |
| `iban` | SEPA (EUR), GBP, or any IBAN-routed transfer | `iban`; optionally `bic` |
| `aba` | US domestic ACH | `routing_number` + `account_number` |

## RecipientDetails fields

```rust
pub struct RecipientDetails {
    pub account_holder_name: String,
    pub country: String,         // ISO 3166-1 alpha-2
    pub currency: String,        // ISO 4217 (e.g. "EUR", "USD", "GBP")
    pub account_type: Option<String>,  // "checking" | "savings" — ACH only
    pub routing_number: Option<String>, // ACH: US ABA routing number
    pub account_number: Option<String>, // ACH: account number
    pub email: Option<String>,         // EMAIL type
    pub iban: Option<String>,          // IBAN type
    pub bic: Option<String>,           // BIC/SWIFT — required for non-SEPA IBAN routes
}
```

Selection priority: `email > iban > aba`. The caller must populate the appropriate fields.

## IBAN validation

Magnetite does **not** perform structural IBAN validation server-side (checksum + country format). Validation is delegated to Wise — an invalid IBAN will be rejected at the Wise API call with a descriptive error. Client-side validation is recommended before submission.

Minimum IBAN length: 15 characters. Maximum: 34 characters.

## Frontend (Earnings page)

The `Earnings.jsx` recipient form presents three tabs — ACH, IBAN, and Email — so the correct fields are collected and transmitted to the backend.

```js
// Example: IBAN recipient
await api.developer.updateWiseRecipient({
  account_holder_name: 'Alice Rust',
  country: 'DE',
  currency: 'EUR',
  iban: 'DE89370400440532013000',
  bic: 'COBADEFFXXX',  // optional for SEPA
});
```

## Sandbox mode

Set `WISE_SANDBOX=true` in `.env` to activate sandbox mode. All Wise API calls are simulated; recipient IDs are prefixed with `sandbox_recipient_`, quote IDs with `sandbox_quote_`, etc. No real transfers are made. The sandbox base URL is `https://api.sandbox.transferwise.tech`.

## Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `WISE_API_TOKEN` | yes (prod) | Wise personal access token |
| `WISE_PROFILE_ID` | yes (prod) | Wise account/profile numeric ID |
| `WISE_SANDBOX` | no | `"true"` for sandbox mode |

Without `WISE_API_TOKEN` (and not in sandbox mode), all payout calls return `HTTP 500 — payouts not configured`. Set `WISE_SANDBOX=true` for local development.

## Known Gaps

- BIC is not validated for non-SEPA routes — Wise will error if BIC is required but not supplied.
- W-9 / W-8 KYC collection is not yet built (see `docs/requirements.md` tax section).
