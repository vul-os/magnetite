# Points & Score Economy

Magnetite provides a **platform-wide XP / score ledger** that all games share. Players
accumulate points by playing games; games can also let players spend points on in-game
rewards. The system supports seasons, leaderboards, and developer-defined reward catalogs.

---

## Concepts

| Concept | Description |
|---------|-------------|
| **Points** | A platform-wide currency (not real money). Also used as XP / score. |
| **Ledger** | Append-only record of every award and spend event |
| **Balance** | Materialised running total — `O(1)` read, maintained atomically with every ledger insert |
| **Season** | Named time period; admin can reset balances to start a new competitive season |
| **Point reward** | A named rule: earn N points for doing X, or spend M points to get Y |

---

## Data model

Tables from migration `20260531_economy.sql`:

### `seasons`

```sql
CREATE TABLE seasons (
    id          UUID PRIMARY KEY,
    name        TEXT NOT NULL,        -- "Season 1 — Launch"
    starts_at   TIMESTAMPTZ NOT NULL,
    ends_at     TIMESTAMPTZ,          -- NULL = ongoing
    is_active   BOOLEAN NOT NULL DEFAULT false
);
-- Only one active season at a time (partial unique index).
```

### `points_ledger`

```sql
CREATE TABLE points_ledger (
    id               UUID PRIMARY KEY,
    user_id          UUID REFERENCES users(id),
    delta            BIGINT NOT NULL,          -- positive = award, negative = spend
    reason           TEXT NOT NULL,            -- e.g. 'game_complete', 'item_purchase'
    game_id          UUID,                     -- optional context
    season_id        UUID,                     -- season at time of entry
    balance_snapshot BIGINT NOT NULL,          -- running balance after this entry
    metadata         JSONB,                    -- arbitrary context (match_id, item_id, …)
    created_at       TIMESTAMPTZ
);
```

### `point_balances`

```sql
CREATE TABLE point_balances (
    user_id   UUID PRIMARY KEY REFERENCES users(id),
    balance   BIGINT NOT NULL DEFAULT 0,
    season_id UUID,                            -- season of last reset
    updated_at TIMESTAMPTZ
);
```

---

## REST API

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/points/balance` | User | Your current balance |
| `GET` | `/api/points/balance/:user_id` | Public | Any user's balance |
| `POST` | `/api/points/award` | Admin / Game server | Award points to a user |
| `POST` | `/api/points/spend` | User | Spend your own points |
| `GET` | `/api/points/history` | User | Paginated ledger history |
| `GET` | `/api/points/leaderboard` | Public | Top balances (platform or per-game) |
| `POST` | `/api/points/season-reset` | Admin | Close season, reset balances, start new season |

### Award points (game server)

```http
POST /api/points/award
Authorization: Bearer <server-jwt>
Content-Type: application/json

{
  "user_id": "uuid",
  "points": 250,
  "reason": "match_win",
  "game_id": "uuid",
  "metadata": { "match_id": "uuid", "placement": 1 }
}
```

### Spend points (user)

```http
POST /api/points/spend
Authorization: Bearer <user-jwt>
Content-Type: application/json

{
  "points": 500,
  "reason": "item_purchase",
  "game_id": "uuid",
  "metadata": { "item_id": "uuid" }
}
```

---

## SDK (`platform::points`)

In-game code can award and spend points directly through the SDK without making raw HTTP calls:

```rust
use magnetite_sdk::platform::points::{
    AwardPointsRequest, PointsClient, PointsConfig, SpendPointsRequest,
};

// Configure the client (once, typically at game startup).
let client = PointsClient::new(PointsConfig {
    api_base: "https://api.magnetite.gg".to_string(),
    game_id: MY_GAME_ID,
    server_jwt: env!("MAGNETITE_SERVER_JWT").to_string(),
});

// Award 100 points to a player who completed the level.
let req = AwardPointsRequest {
    user_id: player.id,
    points: 100,
    reason: "level_complete".to_string(),
    game_id: Some(MY_GAME_ID),
    metadata: None,
};
client.award_points(req).await?;

// Read a player's balance.
let balance = client.get_balance(player.id).await?;
println!("Balance: {} pts", balance.balance);
```

---

## Game template integration

The motorsport template awards points when a player completes a lap:

```rust
// game-template-motorsport/src/lib.rs
use magnetite_sdk::platform::points::{AwardPointsRequest, PointsClient};

fn on_lap_complete(&self, player_id: PlayerId, lap_time_ms: u64) {
    let pts = if lap_time_ms < GOLD_LAP_MS { 500 }
              else if lap_time_ms < SILVER_LAP_MS { 250 }
              else { 100 };
    // Async award — fire-and-forget in a tokio::spawn.
    let client = self.points_client.clone();
    tokio::spawn(async move {
        let _ = client.award_points(AwardPointsRequest {
            user_id: player_id.into(),
            points: pts,
            reason: "lap_complete".to_string(),
            game_id: Some(GAME_ID),
            metadata: None,
        }).await;
    });
}
```

---

## Points leaderboard

The `/api/points/leaderboard` endpoint returns top balances for the current season.
Pass `?game_id=<uuid>` to filter to points earned in a specific game.

The Points dashboard page (`/points`) in the frontend shows the player's own balance,
recent history, and their rank in the global leaderboard.

---

## Season lifecycle

1. A season is seeded automatically (`Season 1 — Launch`) on first migration.
2. At the end of a competitive period, an admin calls `POST /api/points/season-reset`
   with `new_season_name`. The service:
   - Closes the current season (`ended_at = NOW()`).
   - Zeroes all `point_balances` rows (records the old balance in the final ledger entry).
   - Creates and activates the new season.
3. Old ledger history is retained; queries can filter by `season_id` for historical reporting.

---

## See also

- [Dev Marketplace](./marketplace.md) — spending points on store items
- [SDK Reference](./sdk.md) — `platform::points` types
- [Economy & Marketplace Overview](../economy-marketplace.md)
