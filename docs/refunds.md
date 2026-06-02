# Admin Refunds

Magnetite supports admin-initiated refunds for wallet transactions (deposits and payouts).  The refund pipeline is intentionally kept simple in v1: refunds are admin-only, best-effort at the payment-provider level, and always write an audit-trail record regardless of provider outcome.

## Endpoint

```
POST /api/v1/admin/transactions/:id/refund
Authorization: Bearer <admin-jwt>
Content-Type: application/json

{
  "reason": "Customer request — duplicate deposit"
}
```

The `reason` field is optional.

### Response

```json
{
  "refund_id": "uuid",
  "transaction_id": "uuid",
  "user_id": "uuid",
  "amount": "49.99",
  "provider": "paystack",
  "provider_ref": "ps_refund_abc123",
  "status": "completed"
}
```

**Status values:**

| Status | Meaning |
|--------|---------|
| `completed` | Provider refund succeeded; balance restored. |
| `provider_unconfigured` | The required API key (`PAYSTACK_SECRET_KEY` or `WISE_API_TOKEN`) is not set in the environment.  The audit record is still written; the balance is still restored. |
| `failed` | Provider API returned a non-success response.  The audit record is still written; the balance is still restored. |

## Provider Routing

The refund handler determines the payment provider from the original transaction type:

| Transaction type | Provider |
|-----------------|---------|
| `deposit` | Paystack (`PAYSTACK_SECRET_KEY`) |
| `withdrawal` | Wise (`WISE_API_TOKEN`) |
| Any other | `none` — refund is recorded but no provider call is made. |

## Side Effects

1. The user's wallet balance is **credited** by the refund amount (regardless of provider outcome).
2. A `refund` wallet transaction row is inserted.
3. A `refund_records` row is inserted with the provider reference, status, and optional reason.

## Idempotency

The refund endpoint does **not** enforce idempotency in v1 — calling it twice on the same transaction ID will credit the wallet twice and create two audit rows.  Admins should verify the audit log before issuing a second refund.

## Legal Notes

Many jurisdictions (EU, UK, US card networks) require refund capability.  The current implementation satisfies the technical requirement; legal compliance (14-day cooling-off, chargeback dispute handling) is a process concern outside the platform's current scope.  See `docs/requirements.md` for the roadmap item.
