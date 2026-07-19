# Subscription Lifecycle

Subscriptions are **receipt-backed feature flags**, not a recurring charge.

There is no payment processor. A paid tier is activated by presenting a signed,
non-voided `payment_receipts` row that the subscriber owns — proof that they
already paid the node operator wallet-to-wallet through the `PaymentRail` seam.
The node verifies the receipt; it never charges anyone. Paystack, ZAR pricing,
and the auto-renewing subscription charge were **deleted**.

Consequently a tier **expires** at the end of its period rather than
auto-renewing: nothing on the node is capable of taking money again.

## Tiers

| Slug | Price (`price_usdc` / month) | Max Games |
|------|------------------------------|-----------|
| `free` | 0 | 1 |
| `basic` | see `subscription_tiers` | 3 |
| `pro` | see `subscription_tiers` | 10 |
| `unlimited` | see `subscription_tiers` | unlimited |

Prices live in `subscription_tiers.price_usdc` and are returned by
`GET /api/v1/subscriptions`. The `price_zar` column is gone. Tiers with
`price_usdc = 0` require no payment at all.

Paid tiers pay the operator identified by `OPERATOR_WALLET_PUBKEY`; if that is
unset, paid tiers cannot be activated (fails closed).

## REST Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/v1/subscriptions` | no | List active tiers |
| `GET` | `/api/v1/subscriptions/plans` | no | Alias of the above |
| `GET` | `/api/v1/subscriptions/me` | yes | Current user's active subscription |
| `GET` | `/api/v1/subscriptions/current` | yes | Alias of `/me` |
| `POST` | `/api/v1/subscriptions` | yes | Subscribe to a tier |
| `DELETE` | `/api/v1/subscriptions` | yes | Cancel — sets `cancel_at_period_end = true` |
| `POST` | `/api/v1/subscriptions/cancel` | yes | Named alias for `DELETE /` (same handler) |
| `POST` | `/api/v1/subscriptions/upgrade` | yes | Move to a higher tier |
| `POST` | `/api/v1/subscriptions/downgrade` | yes | Move to a lower tier |
| `GET` | `/api/v1/subscriptions/hours` | yes | Included compute hours for the current tier |
| `GET` | `/api/v1/subscriptions/usage` | yes | Game-slot usage this period |

## Subscribing

`POST /api/v1/subscriptions` — body: `{ tier_id, payment_id?, payment_provider? }`

- **Free / zero-price tiers:** no `payment_id` required.
- **Paid tiers:** `payment_id` must be the UUID of a `payment_receipts` row.
  The only accepted `payment_provider` values are `receipt` (default),
  `crypto`, `platform`, or `free`. Anything else is rejected.

The receipt is checked to be **found, not voided, and owned by the calling
user**. Any of those failing rejects the subscription — a row alone is never
proof.

## Upgrade / Downgrade (Proration)

`POST /api/v1/subscriptions/upgrade` and `/downgrade` share one handler.

- The current active subscription is cancelled immediately.
- A new subscription is created at the target tier for a 30-day period starting now.
- The prorated delta is computed as:

```
factor           = remaining_seconds / total_seconds   (clamped to [0, 1])
prorated_delta   = (new_price_usdc − old_price_usdc) × factor
```

- If the delta is positive, a `payment_id` covering it is **required** — but
  the node only *verifies* that receipt. It never moves money itself.
- Downgrades and moves to free omit `payment_id`. Unused days are forfeited;
  there is no downgrade credit.

## Cancel Behaviour

Cancelling sets `status = 'cancel_pending'` and `cancel_at_period_end = true`.
The subscription stays active until `current_period_end`, then expires. Since
nothing renews automatically, "cancel" is really "do not extend".

## Frontend API client

```js
import { api } from 'src/api/client';

const { data: tiers } = await api.subscriptions.plans();
const { data: sub }   = await api.subscriptions.current();

// Upgrade — the second argument is a payment_receipts id, not a processor ref
await api.subscriptions.upgrade(tierUuid, receiptId);

// Downgrade to free (no receipt needed)
await api.subscriptions.upgrade(freeTierUuid);

await api.subscriptions.cancel();

const { data: usage } = await api.subscriptions.usage();
```

## See also

- [Payments](./payments.md) — the `PaymentRail` seam and receipt verification
- [Refunds](./refunds.md) — voiding a receipt
