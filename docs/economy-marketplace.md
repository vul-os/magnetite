# Economy & Marketplace

Magnetite ships two interlinked economic systems as platform services:

1. **Points economy** — a platform-wide XP / score ledger usable by all games
2. **Developer marketplace** — per-game stores where developers sell items to players

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
  `POST /api/marketplace/stores`. Multiple item SKUs can be sold from a single store.
- **Shared checkout.** The platform handles payment processing (fiat USD via Paystack) and
  point deduction — developers don't build their own checkout.
- **Idempotent purchases.** Every `POST /api/marketplace/items/:item_id/purchase`
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
  developer_share             granted_at, expires_at
  platform_fee                revoked
  status (completed|refunded)
  idempotency_key
  metadata
```

### Revenue split — USD

| Party | Share |
|-------|-------|
| Developer | **70 %** of `price_paid` |
| Platform | **30 %** of `price_paid` |

The `developer_share` and `platform_fee` columns in `store_purchases` record the
exact amounts at purchase time. Developers request payouts via `POST /api/developer/payouts`;
the platform processes disbursements through **Wise** (TransferWise).

---

## API reference

### Points

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| `GET` | `/api/points/balance` | User | Authenticated user's balance |
| `GET` | `/api/points/balance/:user_id` | Public | Any user's balance |
| `POST` | `/api/points/award` | Admin/Game | Award points; body: `{user_id, points, reason, game_id?, metadata?}` |
| `POST` | `/api/points/spend` | User | Spend caller's points; body: `{points, reason, game_id?, metadata?}` |
| `GET` | `/api/points/history` | User | Paginated ledger; `?limit=&offset=` |
| `GET` | `/api/points/leaderboard` | Public | Top balances; `?limit=&offset=` |
| `POST` | `/api/points/season-reset` | Admin | Close season; body: `{new_season_name}` |

### Marketplace

| Method | Path | Auth | Notes |
|--------|------|------|-------|
| `GET` | `/api/marketplace/stores/:game_id` | Public | Store details |
| `GET` | `/api/marketplace/stores/:id/items` | Public | Active items; `?kind=` filter |
| `POST` | `/api/marketplace/stores` | Developer | Create store; body: `{game_id, name, description?}` |
| `PUT` | `/api/marketplace/stores/:id` | Developer | Update store |
| `POST` | `/api/marketplace/stores/:id/items` | Developer | Add item; body: `{sku, name, price, currency, kind, metadata?}` |
| `PUT` | `/api/marketplace/stores/:id/items/:item_id` | Developer | Update item |
| `POST` | `/api/marketplace/items/:item_id/purchase` | User | Purchase; body: `{idempotency_key}` |
| `GET` | `/api/marketplace/entitlements` | User | All owned items |
| `GET` | `/api/marketplace/entitlements/:game_id` | User | Owned items for a game |
| `GET` | `/api/marketplace/stores/:game_id/revenue` | Developer | Revenue breakdown |

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
