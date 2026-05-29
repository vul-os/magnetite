# API Reference

REST API documentation for the Magnetite platform.

## Base URL

```
Production: https://api.magnetite.dev
Staging:    https://api.staging.magnetite.dev
Local:      http://localhost:8080
```

## Authentication

All API requests require authentication via Bearer token.

```bash
curl -H "Authorization: Bearer <token>" https://api.magnetite.dev/v1/games
```

## Rate Limits

| Tier | Requests/minute | Burst |
|------|-----------------|-------|
| Free | 60 | 10 |
| Pro | 600 | 100 |
| Enterprise | 6000 | 1000 |

## Endpoints

| Section | Description |
|---------|-------------|
| [Auth](/api-reference/auth/) | Authentication endpoints |
| [Games](/api-reference/games/) | Game management |
| [Wallet](/api-reference/wallet/) | Wallet operations |
| [Matchmaking](/api-reference/matchmaking/) | Match queue management |

## Response Format

All responses follow this structure:

```json
{
  "success": true,
  "data": { ... },
  "meta": {
    "request_id": "req_abc123",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

### Error Response

```json
{
  "success": false,
  "error": {
    "code": "INVALID_INPUT",
    "message": "Entry fee must be positive",
    "details": {
      "field": "entry_fee",
      "value": -100
    }
  },
  "meta": {
    "request_id": "req_abc123"
  }
}
```

## Pagination

List endpoints support cursor-based pagination.

```bash
GET /v1/games?cursor=abc123&limit=20
```

```json
{
  "data": [...],
  "pagination": {
    "next_cursor": "xyz789",
    "has_more": true,
    "total": 150
  }
}
```

## Webhooks

Receive real-time updates via webhooks.

```json
{
  "event": "game.completed",
  "data": {
    "game_id": "game_abc123",
    "winner": "player_xyz",
    "prize": 180
  }
}
```

### Supported Events

| Event | Description |
|-------|-------------|
| `game.completed` | Game finished |
| `game.started` | Game began |
| `match.found` | Matchmaking complete |
| `wallet.debited` | Funds deducted |
| `wallet.credited` | Funds added |
