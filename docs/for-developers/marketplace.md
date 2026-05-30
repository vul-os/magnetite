# Developer Marketplace

Magnetite lets developers create **in-game stores** for their game. Players can browse
and purchase items (cosmetics, DLC, passes, consumable items) directly inside the game
or on the game's marketplace page. Revenue is split 70 % developer / 30 % platform for
USDC purchases; points purchases are a pure in-game transaction.

---

## Concepts

| Concept | Description |
|---------|-------------|
| **Store** | One store per game, owned by the developer. Created via the developer dashboard. |
| **Item** | A purchasable SKU: cosmetic, in-game item, DLC, or pass. |
| **Purchase** | A completed transaction; creates an entitlement for the buyer. |
| **Entitlement** | A record that a user owns a specific item. Checked at runtime via the SDK. |

---

## Item types

| `ItemType` | Description |
|------------|-------------|
| `Cosmetic` | Visual customisation (skin, emote, banner) — no gameplay effect |
| `Item` | Consumable or persistent in-game item (health pack, ammo, boost) |
| `Dlc` | Downloadable content — unlocks a new game mode, level, or feature |
| `Pass` | Season pass or battle pass — grants access to a set of content over a period |

---

## Currency

Items can be priced in **USDC** or **points** (see [Points Economy](./points-economy.md)):

| Currency | Developer share | Platform share |
|----------|----------------|----------------|
| `USDC` | 70 % | 30 % |
| `points` | Full points deducted from buyer's balance; developer receives no USDC | — |

---

## Data model

Tables from migration `20260531_economy.sql`:

### `dev_stores`

One per game; `game_id` is unique.

```sql
CREATE TABLE dev_stores (
    id           UUID PRIMARY KEY,
    game_id      UUID NOT NULL UNIQUE REFERENCES games(id),
    developer_id UUID NOT NULL REFERENCES users(id),
    name         TEXT NOT NULL,
    description  TEXT,
    active       BOOLEAN NOT NULL DEFAULT true
);
```

### `store_items`

```sql
CREATE TABLE store_items (
    id       UUID PRIMARY KEY,
    store_id UUID NOT NULL REFERENCES dev_stores(id),
    game_id  UUID NOT NULL REFERENCES games(id),
    sku      TEXT NOT NULL,           -- e.g. "skin_dragon_red"
    name     TEXT NOT NULL,
    price    NUMERIC(18, 6) NOT NULL,
    currency TEXT NOT NULL,           -- 'USDC' or 'points'
    kind     TEXT NOT NULL,           -- 'cosmetic' | 'item' | 'dlc' | 'pass'
    active   BOOLEAN NOT NULL DEFAULT true,
    metadata JSONB                    -- {icon_url, rarity, boost_multiplier, …}
);
```

### `store_purchases` and `entitlements`

`store_purchases` records the transaction (idempotency key, price, revenue split).
`entitlements` records ownership (survives item deactivation; optional `expires_at`).

---

## REST API

### Public (no auth required)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/marketplace/stores/:game_id` | Get store details for a game |
| `GET` | `/api/marketplace/stores/:id/items` | List active items (optionally filter by `?kind=cosmetic`) |

### Developer (requires auth + game ownership)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/marketplace/stores` | Create a store for your game |
| `PUT` | `/api/marketplace/stores/:id` | Update store name / description / active flag |
| `POST` | `/api/marketplace/stores/:id/items` | Add an item to the store |
| `PUT` | `/api/marketplace/stores/:id/items/:item_id` | Update item price, name, active flag |
| `GET` | `/api/marketplace/stores/:game_id/revenue` | Revenue breakdown (total, by item) |

### Player (requires auth)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/marketplace/items/:item_id/purchase` | Purchase an item |
| `GET` | `/api/marketplace/entitlements` | Your owned items |
| `GET` | `/api/marketplace/entitlements/:game_id` | Your owned items for a specific game |

### Purchase request

```http
POST /api/marketplace/items/:item_id/purchase
Authorization: Bearer <user-jwt>
Content-Type: application/json

{
  "idempotency_key": "client-generated-uuid"
}
```

The `idempotency_key` prevents double-charges if the client retries on network failure.

---

## SDK (`platform::marketplace`)

Game servers and in-game code can check entitlements and initiate purchases via the SDK:

```rust
use magnetite_sdk::platform::marketplace::{
    MarketplaceClient, MarketplaceConfig, PurchaseRequest, PaymentMethod,
};

let client = MarketplaceClient::new(MarketplaceConfig {
    api_base: "https://api.magnetite.gg".to_string(),
    game_id: MY_GAME_ID,
    server_jwt: env!("MAGNETITE_SERVER_JWT").to_string(),
});

// Check if a player owns an item.
let entitlements = client.get_entitlements(player.id).await?;
let has_dragon_skin = entitlements.iter()
    .any(|e| e.item_sku == "skin_dragon_red");

// Server-side: initiate a points-based purchase on behalf of a player.
let result = client.purchase(player.id, PurchaseRequest {
    item_id: DRAGON_SKIN_ITEM_ID,
    payment_method: PaymentMethod::Points,
    idempotency_key: Uuid::new_v4().to_string(),
}).await?;
```

---

## In-game store UI

The `InGameStore` component renders a purchasable item list as an overlay during play:

```jsx
import InGameStore from '../components/store/InGameStore';

// Inside Playground.jsx or a custom game HUD:
<InGameStore
  gameId={game.id}
  onPurchase={(item) => toast.success(`Bought ${item.name}!`)}
/>
```

The panel fetches items from `/api/marketplace/stores/:game_id/items`, checks
entitlements against the authenticated user, and calls the purchase endpoint on click.

---

## Developer store management

The `DevMarketplace` page at `/developers/marketplace` provides:

- Store creation and editing (name, description, active flag)
- Item CRUD (add / edit / deactivate items)
- Revenue overview: total revenue, per-item breakdown, purchase history

---

## See also

- [Points Economy](./points-economy.md) — points-based purchases
- [Economy & Marketplace Overview](../economy-marketplace.md) — data model + revenue split
- [SDK Reference](./sdk.md) — `platform::marketplace` types
