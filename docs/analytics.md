# Developer Analytics

The developer analytics endpoint provides per-game metrics covering player activity, session data, and revenue over a rolling 30-day window.

## Endpoint

```
GET /api/v1/developer/games/:gameId/players
Authorization: Bearer <developer-jwt>
```

Also mounted as `GET /api/v1/developer/games/:gameId/analytics` (same handler).

## Response Shape

```json
{
  "game_id": "uuid",
  "daily_active_players": [
    { "date": "2026-06-01", "active_players": 58, "new_players": 8 }
  ],
  "session_duration_stats": {
    "avg_duration_secs": 240.0,
    "total_sessions": 100,
    "avg_score": 1500.0
  },
  "revenue_breakdown": {
    "total_revenue": "1000.00",
    "platform_fee": "300.00",
    "developer_earnings": "700.00",
    "session_count": 100
  },
  "daily_revenue": [
    { "date": "2026-06-01", "revenue_usd": "42.00" }
  ],
  "daily_playtime": [
    { "date": "2026-06-01", "total_minutes": 3120 }
  ]
}
```

### daily_active_players

An array of up to 30 data points (one per day), covering daily active players and new-player acquisition:

| Field | Type | Description |
|-------|------|-------------|
| `date` | `string` | ISO 8601 date (`YYYY-MM-DD`). |
| `active_players` | `integer` | Players with at least one completed session on this day. |
| `new_players` | `integer` | First-time players on this day. |

### session_duration_stats

Aggregate session statistics over the 30-day window:

| Field | Type | Description |
|-------|------|-------------|
| `avg_duration_secs` | `float` | Mean session length in seconds. |
| `total_sessions` | `integer` | Total completed sessions. |
| `avg_score` | `float` | Mean final score across sessions. |

### revenue_breakdown

| Field | Type | Description |
|-------|------|-------------|
| `total_revenue` | `decimal string` | Sum of the legacy `game_revenue` table for this game (USD). |
| `platform_fee` | `decimal string` | See the warning below. |
| `developer_earnings` | `decimal string` | `total_revenue − platform_fee`. |
| `session_count` | `integer` | Sessions for this game. |

> ⚠️ **Known inconsistency — do not treat these three fields as authoritative.**
> `platform_fee` is still computed as a hardcoded `total_revenue × 0.30` in
> `backend/src/api/developer.rs`, reading the legacy `game_revenue` table. That
> is a leftover from the deleted 70/30 model and **contradicts the shipped
> non-custodial system**, where the developer receives the entire subtotal and
> the only deduction is `PROTOCOL_FEE_BPS` (default 0). Settlement is
> authoritatively reported by `GET /api/v1/developer/earnings`, which sums
> signed, non-voided `payment_receipts`. Use that endpoint for money; treat
> this block as a stale analytics artifact pending cleanup.

### daily_revenue / daily_playtime

Both are 30-day daily buckets.

| Field | Type | Description |
|-------|------|-------------|
| `daily_revenue[].date` | `string` | ISO 8601 date. |
| `daily_revenue[].revenue_usd` | `decimal string` | Revenue recorded that day (`game_revenue`). |
| `daily_playtime[].date` | `string` | ISO 8601 date. |
| `daily_playtime[].total_minutes` | `integer` | Total minutes played by all users that day. |

## Frontend Usage

The `DeveloperDashboard` page fetches analytics via `api.developer.analytics(gameId)` and displays the data in Recharts-based charts.  The response shape mirrors the backend `GameAnalytics` struct (`backend/src/api/developer.rs`).
