# Economy & Marketplace

Magnetite ships two interlinked economic systems as platform services:

1. **Points economy** — a platform-wide XP / score ledger usable by all games
2. **Developer marketplace** — per-game stores where developers sell items to players

> **Non-custodial.** Real-money purchases go wallet-to-wallet through the
> `PaymentRail` seam and the developer receives the whole subtotal. There are
> no balances, deposits, withdrawals, payouts, or platform revenue share. The
> default rail (`MockPaymentRail`) signs receipts locally and runs offline; a
> real chain rail is `TODO(chain)` and is **not built**. Points are *not*
> money — they stay off-chain in a plain ledger.

---

## Points economy

### Design principles

- **Append-only ledger.** Every point change (award or spend) produces a new `points_ledger` row.
  Balances are never mutated in place; a separate `point_balances` table holds the materialised total
  for `O(1)` balance reads, updated atomically in the same transaction.
- **Season-aware.** Points exist within a named season. An admin can close the current season,
  reset balances, and start a new one without losing historical data.
- **Platform-wide.** Any game can award or spend points via the REST API or the SDK
  (`platform::points`); no per-game schema changes needed.

### Data model

```
seasons                     point_balances
  id                          user_id (PK)
  name                        balance
  starts_at                   season_id
  ends_at                     updated_at
  is_active
                            points_ledger
point_rewards                 id
  id                          user_id
  name                        delta          (+ award / - spend)
  kind (earn|redeem)          reason
  points                      game_id
  game_id                     season_id
  active                      balance_snapshot
                              metadata (JSONB)
                              created_at
```

### Revenue split — points

Points are an in-game currency, not real money. When a player spends points on a
store item, the platform records a `store_purchases` row with `currency = 'points'`
and creates an `entitlements` row. No real-money transfer occurs.

---

## Developer marketplace

### Design principles

- **One store per game.** A developer creates exactly one store per game via
  `POST /api/v1/marketplace/games/:game_id/store`. Multiple item SKUs can be sold from a single store.
- **Non-custodial checkout.** A USD purchase is an atomic wallet→wallet
  `PaymentRail::checkout` from the buyer to the developer. The platform holds
  no funds and is not in the payment path. Points purchases are pure ledger
  moves.
- **The receipt is the entitlement.** Checkout returns a signed `Receipt`,
  persisted in `payment_receipts` and linked from `entitlements.receipt_id`.
  Entitlement checks re-verify the rail signature, so a database row alone
  never grants access.
- **Idempotent purchases.** Every `POST /api/v1/marketplace/items/:item_id/purchase`
  requires a client-generated `idempotency_key` to prevent duplicate charges on retries.
- **Entitlements survive deactivation.** Items can be deactivated; entitlement records
  are never deleted.

### Data model

```
dev_stores                  store_items
  id                          id
  game_id (unique)            store_id
  developer_id                game_id
  name                        sku (unique per store)
  description                 name, description
  active                      price, currency
                              kind (cosmetic|item|dlc|pass)
store_purchases               active, metadata
  id
  user_id                   entitlements
  item_id                     id
  store_id, game_id           user_id, item_id
  price_paid, currency        purchase_id
  developer_share             receipt_id  ──┐
  platform_fee                granted_at    │
  status (completed|refunded) expires_at    │
  idempotency_key             revoked       │
  refunded_at/by/reason                     │
  metadata                                  │
                            payment_receipts │
                              id  ◄──────────┘
                              kind (item_purchase|subscription|hosting)
                              buyer_id, buyer_pubkey
                              purchase_id, item_id, game_id
                              total, protocol_fee   (smallest unit)
                              payouts (JSONB: [{wallet, amount}, …])
                              nonce, rail, rail_pubkey, sig
                              voided, voided_at
```

`entitlements.receipt_id` points at the signed receipt. Verification
re-computes the rail signature over the receipt and refuses voided receipts, so
the entitlement row is bookkeeping — the *signature* is the authority.

### Revenue split — USD

| Party | Share |
|-------|-------|
| Developer | **the entire subtotal** |
| Protocol | `PROTOCOL_FEE_BPS` basis points, **default 0**, added *on top of* the subtotal |

The 70/30 platform cut is gone. The developer is paid at the instant of sale by
the buyer's wallet — there is no platform balance to debit, no developer
balance to credit, and **no payout to request**. The legacy
`store_purchases.developer_share` column now records the full subtotal and
`platform_fee` records the protocol fee (0 by default).

`POST /api/developer/payouts` and the Wise disbursement pipeline were deleted.
Developers see settlement through `GET /api/v1/developer/earnings`, which sums
non-voided receipts; its `pending_payout` field is always zero because nothing
is ever held on a developer's behalf.

---

## API reference

### Points

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| `GET` | `/api/v1/points/balance` | User | Authenticated user's balance |
| `GET` | `/api/v1/points/balance/:user_id` | Public | Any user's balance |
| `POST` | `/api/v1/points/award` | Admin/Game | Award points; body: `{user_id, points, reason, game_id?, metadata?}` |
| `POST` | `/api/v1/points/spend` | User | Spend caller's points; body: `{points, reason, game_id?, metadata?}` |
| `GET` | `/api/v1/points/history` | User | Paginated ledger; `?limit=&offset=` |
| `GET` | `/api/v1/points/history/:user_id` | User | Another user's ledger |
| `GET` | `/api/v1/points/leaderboard` | Public | Top balances; `?limit=&offset=` |
| `GET` | `/api/v1/points/season` | Public | Active season |
| `POST` | `/api/v1/points/season/reset` | Admin | Close season; body: `{new_season_name}` |

### Marketplace

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| `GET` | `/api/v1/marketplace/stores/:game_id` | Public | Store details |
| `GET` | `/api/v1/marketplace/stores/:store_id/items` | Public | Active items; `?kind=` filter |
| `GET` | `/api/v1/marketplace/items/:item_id` | Public | Single item |
| `POST` | `/api/v1/marketplace/games/:game_id/store` | Developer | Create the game's store |
| `PUT` | `/api/v1/marketplace/stores/:store_id` | Developer | Update store |
| `GET` | `/api/v1/marketplace/my-stores` | Developer | Own stores |
| `POST` | `/api/v1/marketplace/stores/:store_id/items` | Developer | Add item; body: `{sku, name, price, currency, kind, metadata?}` |
| `PUT` | `/api/v1/marketplace/items/:item_id` | Developer | Update item |
| `GET` | `/api/v1/marketplace/stores/:store_id/revenue` | Developer | Revenue breakdown |
| `POST` | `/api/v1/marketplace/items/:item_id/purchase` | User | Purchase; body: `{idempotency_key}` |
| `GET` | `/api/v1/marketplace/purchases` | User | Purchase history |
| `GET` | `/api/v1/marketplace/purchases/:purchase_id` | User | Purchase receipt |
| `POST` | `/api/v1/marketplace/purchases/:purchase_id/refund` | Store owner / admin | Void the receipt, revoke the entitlement |
| `GET` | `/api/v1/marketplace/entitlements` | User | All owned items |
| `GET` | `/api/v1/marketplace/entitlements/:item_id/check` | User | Check one entitlement |

---

## SDK integration

### Points

```rust
use magnetite_sdk::platform::points::{AwardPointsRequest, PointsClient, PointsConfig};

let client = PointsClient::new(PointsConfig {
    api_base: "https://api.magnetite.gg".to_string(),
    game_id: MY_GAME_ID,
    server_jwt: env!("MAGNETITE_SERVER_JWT").to_string(),
});

// Award
client.award_points(AwardPointsRequest {
    user_id: player.id,
    points: 250,
    reason: "match_win".to_string(),
    game_id: Some(MY_GAME_ID),
    metadata: None,
}).await?;

// Balance
let balance = client.get_balance(player.id).await?;
```

### Marketplace

```rust
use magnetite_sdk::platform::marketplace::{
    MarketplaceClient, MarketplaceConfig, PurchaseRequest, PaymentMethod,
};

let client = MarketplaceClient::new(MarketplaceConfig { … });

// Check entitlement
let items = client.get_entitlements(player.id).await?;
let owns_skin = items.iter().any(|e| e.item_sku == "skin_gold_dragon");

// Server-initiated purchase (e.g. points reward)
client.purchase(player.id, PurchaseRequest {
    item_id: ITEM_ID,
    payment_method: PaymentMethod::Points,
    idempotency_key: uuid::Uuid::new_v4().to_string(),
}).await?;
```

---

## Frontend

| Page | Route | Description |
|------|-------|-------------|
| `Points.jsx` | `/points` | Player balance, ledger history, leaderboard |
| `DevMarketplace.jsx` | `/developers/marketplace` | Developer store + item management |
| `InGameStore` component | Overlay | Purchasable items during play |

---

## See also

- [Points Economy Guide](./for-developers/points-economy.md) — full developer guide
- [Marketplace Guide](./for-developers/marketplace.md) — store creation + items
- [SDK Reference](./for-developers/sdk.md)
- [Architecture Overview](./architecture.md)
