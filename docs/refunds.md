# Refunds

**A refund is the *void* of a signed receipt, not the reversal of a balance.**

Magnetite holds no funds. There is no custodial wallet to credit, no developer
balance to claw back, and no payment provider to call. A purchase produced a
signed `Receipt` from the `PaymentRail` seam; that receipt is what granted the
entitlement. Refunding therefore means: mark the receipt voided, revoke the
entitlement it granted, and mark the purchase `refunded`.

Because entitlement checks **re-verify the rail signature and the voided flag
on every access**, voiding a receipt takes effect immediately and everywhere —
a stale database row on its own never grants anything.

## Endpoint

```
POST /api/v1/marketplace/purchases/:purchase_id/refund
Authorization: Bearer <jwt>
Content-Type: application/json

{ "reason": "Customer request" }
```

`reason` is optional.

The old admin endpoint `POST /api/v1/admin/transactions/:id/refund` and its
custodial balance-reversal logic were **deleted**, along with the Paystack
refund API call and the Wise provider routing. There is no provider-specific
refund path any more, because there is no provider.

## Authorization

| Actor | Allowed |
|-------|---------|
| Platform admin | Any purchase |
| The developer who owns the store | Purchases from their own store |
| Anyone else | `403 Forbidden` |

## Guards

Refunds fail closed:

| Condition | Result |
|-----------|--------|
| Purchase not found | `404` |
| Already refunded (`status = 'refunded'` or `refunded_at` set) | `400` — refunds are not repeatable |
| Purchase status is not `completed` | `400` |
| Caller is neither admin nor the store owner | `403` |

The already-refunded guard is what makes the operation idempotent in effect:
a second call is rejected rather than applied twice.

## Effects

**USD purchases** (one transaction):

1. `payment_receipts.voided = true` for the receipt backing the purchase.
2. The `entitlements` row granted by that receipt is revoked.
3. `store_purchases.status = 'refunded'`, with `refunded_at`, `refunded_by`, and `refund_reason` recorded.

**Points purchases:** the points are re-awarded through `PointsService` (which
manages its own transaction), then the same purchase/entitlement bookkeeping
runs.

## Not done

A refund today **does not move money**. Voiding the receipt removes the
buyer's access, but with the default `MockPaymentRail` there is nothing to
transfer back. The code carries an explicit `TODO(chain)` in
`backend/src/services/marketplace.rs`: with a real chain rail, a refund should
issue a compensating wallet→wallet transfer from the developer back to the
buyer (or settle it from an escrow/dispute window) and record that transfer's
receipt. Until a real rail exists, treat a refund as *entitlement revocation*,
not as a repayment.

## Legal note

Many jurisdictions require a refund capability. Revoking the entitlement is the
technical half. Because the platform never takes custody of the payment, the
commercial half (actually returning funds) is a settlement between buyer and
developer on whatever rail they used — not something Magnetite can perform on
their behalf.

## See also

- [Payments](./payments.md) — the non-custodial model
- [Economy & Marketplace](./economy-marketplace.md) — stores, items, entitlements
