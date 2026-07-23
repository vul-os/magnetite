# Subscription Lifecycle

> **Status: the user-facing subscription system was removed.** There is no
> `/api/v1/subscriptions` REST API and no frontend subscription flow — the whole
> public surface was deleted with the off-thesis billing code (the
> [API Reference](./api-reference/index.md) now marks it removed). This page
> documents what actually remains: a dormant data model and the operator audit
> that reads it.

The platform charges nothing and runs no payment processor. Paid access is not a
recurring subscription — it is a **receipt-backed entitlement**: a signed,
non-voided `payment_receipts` row proving the buyer already paid the node
operator wallet-to-wallet through the `PaymentRail` seam. The node verifies
receipts; it never charges anyone. Paystack, ZAR pricing, and the auto-renewing
subscription charge were **deleted**. See [Payments](./payments.md).

## What remains

The tier data model is retained but **dormant** — no endpoint creates, changes,
renews, or cancels a subscription, and the frontend ships no subscription client:

| Table | Role now |
|-------|----------|
| `subscription_tiers` | Tier definitions (`name`, `slug`, `price_usdc`, `max_games`, `features`, `is_active`). A `price_zar` column also survives, defaulting to `0` and unused. |
| `user_subscriptions` | Historical tier assignments, if any. |
| `subscription_transactions` | Historical tier charges. |

The only code that still touches these tables is the **superadmin billing
audit** (`backend/src/superadmin/billing.rs`) — a strictly read-only consistency
check (it issues no `INSERT`/`UPDATE`/`DELETE` against them). It verifies
invariants such as "every recorded subscription charge matches its tier's
`price_usdc`" and "paid entitlements are receipt-backed". It reports; it manages
nothing.

## See also

- [Payments](./payments.md) — the `PaymentRail` seam and receipt verification
- [Refunds](./refunds.md) — voiding a receipt
