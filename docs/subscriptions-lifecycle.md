# Subscription Lifecycle

Magnetite uses a tiered subscription model. Paystack is the payment on-ramp (fiat) for all paid tiers; platform and free tiers require no payment.

## Tiers

| Slug | Price (ZAR/month) | Max Games |
|------|-------------------|-----------|
| `free` | 0 | 1 |
| `basic` | ~150 | 3 |
| `pro` | ~350 | 10 |
| `unlimited` | ~800 | unlimited |

Prices in `price_usdc` and `price_zar` columns are stored in `subscription_tiers` and returned by `GET /api/v1/subscriptions`.

## REST Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/v1/subscriptions` | no | List all active tiers (aliased from `/plans`) |
| `GET` | `/api/v1/subscriptions/me` | yes | Current user's active subscription |
| `POST` | `/api/v1/subscriptions` | yes | Subscribe to a tier (free = no payment; paid = Paystack ref required) |
| `DELETE` | `/api/v1/subscriptions` | yes | Cancel — sets `cancel_at_period_end = true`, expires at period end |
| `POST` | `/api/v1/subscriptions/cancel` | yes | Named alias for `DELETE /` (same handler) |
| `POST` | `/api/v1/subscriptions/upgrade` | yes | Upgrade or downgrade tier (see proration below) |
| `GET` | `/api/v1/subscriptions/hours` | yes | Included compute hours for the current tier |
| `GET` | `/api/v1/subscriptions/usage` | yes | Game-slot usage this period |

## Upgrade / Downgrade (Proration)

`POST /api/v1/subscriptions/upgrade` — body: `{ tier_id: UUID, payment_id?: string }`

- The current active subscription is immediately cancelled.
- A new subscription is created at the target tier for a 30-day period starting now.
- For paid tier upgrades, a `payment_id` (Paystack reference) covering the upgrade cost must be supplied. The Paystack charge is verified server-side before the subscription is activated.
- For downgrades or free-tier moves, `payment_id` is omitted.

### Proration math

The recommended client-side proration display formula:

```
factor = remaining_seconds / total_seconds  (clamped to [0, 1])
upgrade_charge = (new_price - old_price) * factor
```

The backend does not currently issue a partial refund for downgrade credit; the unused days are forfeited. Future work (see TASKS.md) will add a credit mechanism.

## Cancel Behaviour

Cancellation sets `status = 'cancel_pending'` and `cancel_at_period_end = true`. The subscription remains active until `current_period_end`. The renewal job then marks it `cancelled` at expiry.

This differs from the legacy behaviour (immediate `status = 'cancelled'`), which is the standard SaaS cancel-at-period-end expectation.

## Frontend API client

```js
import { api } from 'src/api/client';

// List tiers
const { data: tiers } = await api.subscriptions.plans();

// Current subscription (includes cancel_at_period_end)
const { data: sub } = await api.subscriptions.current();

// Upgrade
await api.subscriptions.upgrade(tierUuid, paystackPaymentRef);

// Downgrade to free (no payment_id)
await api.subscriptions.upgrade(freeTierUuid);

// Cancel
await api.subscriptions.cancel();

// Usage
const { data: usage } = await api.subscriptions.usage();
// { used_games: 2, max_games: 5, remaining_days: 14 }
```
