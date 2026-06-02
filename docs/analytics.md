# Developer Analytics

The developer analytics endpoint provides per-game metrics covering player activity, session data, and revenue over a rolling 30-day window.

## Endpoint

```
GET /api/v1/developer/games/:gameId/players
Authorization: Bearer <developer-jwt>
```

(Also accessible as `GET /api/developer/analytics/:gameId` via the legacy path.)

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
  }
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

Revenue split using the platform's 70/30 model:

| Field | Type | Description |
|-------|------|-------------|
| `total_revenue` | `decimal string` | Gross revenue from session fees (USD). |
| `platform_fee` | `decimal string` | Platform's 30% share. |
| `developer_earnings` | `decimal string` | Developer's 70% share. |
| `session_count` | `integer` | Sessions that generated revenue. |

## Revenue Time-Series (Roadmap)

The current analytics response does not include a daily revenue chart (time-series of earnings per day).  This is a planned enhancement — see AUDIT.md medium-severity finding "Developer analytics: no revenue time-series per game".  When implemented, the response will include a `daily_revenue` array with the same date format as `daily_active_players`.

## Frontend Usage

The `DeveloperDashboard` page fetches analytics via `api.developer.analytics(gameId)` and displays the data in Recharts-based charts.  The response shape mirrors the backend `GameAnalytics` struct (`backend/src/api/developer.rs`).
