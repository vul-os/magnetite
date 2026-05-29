# API Reference

The Magnetite REST API is served at `/api/v1/` on port 8080.
All endpoints return JSON in one of two envelopes:

**Success:**
```json
{ "status": "success", "data": { … } }
```

**Paginated success:**
```json
{ "status": "success", "data": [ … ], "page": 1, "per_page": 20, "total": 142 }
```

**Error:**
```json
{ "status": "error", "message": "…", "code": 404 }
```

### Authentication

Protected routes require a JWT Bearer token:
```
Authorization: Bearer <access_token>
```

Obtain a token via `POST /api/v1/auth/login`. Tokens expire in 15 minutes (configurable
via `ACCESS_TOKEN_EXPIRY`). Use `POST /api/v1/auth/refresh` with your refresh token to
get a new pair without re-logging in.

---

## Auth — `/api/v1/auth`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/login` | — | Username + password login; returns `{ access_token, refresh_token, expires_at, user_id }` |
| `POST` | `/register` | — | Create account; returns same token pair |
| `POST` | `/refresh` | — | Exchange `refresh_token` for a new token pair |
| `DELETE` | `/logout` | required | Revoke current session |
| `DELETE` | `/logout-all` | required | Revoke all sessions for the user |
| `GET` | `/sessions` | required | List active sessions |
| `GET` | `/me` | required | Return current user `{ id, username, email, created_at }` |

---

## OAuth — `/api/v1/oauth`

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/google` | Redirect to Google OAuth |
| `GET` | `/google/callback` | Google OAuth callback |
| `GET` | `/discord` | Redirect to Discord OAuth |
| `GET` | `/discord/callback` | Discord OAuth callback |
| `GET` | `/github` | Redirect to GitHub OAuth |
| `GET` | `/github/callback` | GitHub OAuth callback |
| `GET` | `/gitlab` | Redirect to GitLab OAuth |
| `GET` | `/gitlab/callback` | GitLab OAuth callback |

---

## Games — `/api/v1/games`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | — | List live games (paginated) |
| `POST` | `/` | required | Create a game record |
| `GET` | `/:id` | — | Get game details |
| `PUT` | `/:id` | required | Update game (owner only) |
| `DELETE` | `/:id` | admin | Delete game |
| `GET` | `/:id/leaderboard` | — | Top scores for a game |

---

## Distribution — `/api/v1/distribution`

Manages versioned WASM artifacts and provides the play manifest consumed by the browser.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/:game_id/play` | — | Play manifest: WASM URL, server URL, version, hash |
| `GET` | `/:game_id/build-status` | — | Latest build status summary |
| `GET` | `/:game_id/artifacts` | — | List all artifacts for a game |
| `GET` | `/:game_id/artifacts/:artifact_id` | — | Single artifact details |
| `GET` | `/:game_id/versions` | — | List all registered versions |
| `POST` | `/:game_id/versions` | required | Register a new version `{ version, commit_sha, release_notes? }` |
| `PUT` | `/:game_id/versions/:version_id/promote` | required | Promote version to live |
| `PUT` | `/:game_id/artifacts/:artifact_id` | required | Update artifact build status / URL |

**Play manifest response:**
```json
{
  "game_id": "…",
  "version": "1.2.0",
  "commit_sha": "abc123",
  "wasm_url": "https://…/game_bg.wasm",
  "server_url": "wss://…/ws/game",
  "artifact_type": "wasm",
  "sha256_hash": "…",
  "file_size_bytes": 1048576
}
```

**Artifact build statuses:** `pending` | `building` | `success` | `failed`

---

## Leaderboard — `/api/v1/leaderboard`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/:game_id` | — | Global leaderboard for a game |
| `POST` | `/:game_id/scores` | required | Submit a score |
| `GET` | `/:game_id/me` | required | Authenticated user's rank |
| `GET` | `/:game_id/friends` | required | Friends-only leaderboard |

---

## Matchmaking — `/api/v1/matchmaking`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/join` | required | Join matchmaking queue |
| `DELETE` | `/leave` | required | Leave queue |
| `GET` | `/status` | required | Queue status + estimated wait seconds |

---

## Wallet — `/api/v1/wallet`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/balance` | required | USDC balance |
| `POST` | `/deposit` | required | Deposit funds |
| `POST` | `/withdraw` | required | Withdraw funds |
| `GET` | `/transactions` | required | Transaction history |

---

## Developer — `/api/v1/developer`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/register` | required | Register as a developer |
| `GET` | `/dashboard` | required | Dashboard stats (DAU, revenue, sessions) |
| `GET` | `/games` | required | Developer's own games |
| `PUT` | `/games/:id/status` | required | Update game status (e.g. submit for review) |
| `DELETE` | `/games/:id` | required | Delete a game |
| `GET` | `/earnings` | required | Earnings summary |
| `GET` | `/payouts` | required | Payout history |
| `POST` | `/payouts` | required | Request a payout |
| `GET` | `/games/:id/players` | required | Per-game player analytics |

---

## Achievements — `/api/v1/achievements`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/:user_id` | — | All achievements for a user |
| `GET` | `/:user_id/:id` | — | Single achievement |
| `POST` | `/:user_id/:id/progress` | required | Update progress |
| `GET` | `/leaderboard` | — | Achievement leaderboard |

---

## Social

### Friends — `/api/v1/friends`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | required | List friends |
| `POST` | `/request` | required | Send friend request |
| `POST` | `/accept/:id` | required | Accept request |
| `POST` | `/reject/:id` | required | Reject request |
| `DELETE` | `/:id` | required | Remove friend |
| `POST` | `/block/:id` | required | Block user |

### Invites — `/api/v1/invites`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | required | List pending invites |
| `POST` | `/:id/accept` | required | Accept game invite |
| `POST` | `/:id/decline` | required | Decline game invite |

### Users — `/api/v1/users`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/search` | — | Search users by username |
| `GET` | `/:id` | — | Public user profile |

---

## Subscriptions — `/api/v1/subscriptions`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | — | List subscription tiers |
| `POST` | `/subscribe` | required | Subscribe to a tier |
| `POST` | `/cancel` | required | Cancel subscription |
| `GET` | `/status` | required | Current subscription status |

---

## Notifications — `/api/v1/notifications`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | required | List notifications |
| `GET` | `/count` | required | Unread notification count |
| `PUT` | `/read-all` | required | Mark all as read |
| `PUT` | `/:id/read` | required | Mark one as read |
| `DELETE` | `/:id` | required | Delete notification |
| `POST` | `/` | required | Create notification (internal / admin) |

**WebSocket:** `GET /ws/notifications` — persistent push channel (JWT required via
`Authorization: Bearer` header or `?token=` query parameter).

---

## GitHub & CI — `/api/v1/github`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/webhooks/github` | HMAC-SHA256 | Receive GitHub push/check events |
| `GET` | `/installations` | required | List GitHub App installations |
| `GET` | `/repos` | required | List registered repositories |
| `POST` | `/repos/register` | required | Register a repository |
| `GET` | `/repos/:owner/:repo/build-status` | required | Build status for a repo |

The webhook signature is verified against `GITHUB_WEBHOOK_SECRET` using `X-Hub-Signature-256`.

---

## Webhooks — `/api/v1/webhooks`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/paystack` | HMAC | Paystack payment event |
| `POST` | `/circle` | HMAC | Circle USDC event |
| `POST` | `/game` | secret | Game-server event (session end, score) |
| `GET` | `/endpoints` | admin | List registered webhook endpoints |
| `POST` | `/endpoints` | admin | Register a webhook endpoint |
| `DELETE` | `/endpoints/:id` | admin | Remove a webhook endpoint |

---

## Admin — `/api/v1/admin`

All admin routes require an authenticated user with the `admin` role.

**Users**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/users` | List all users |
| `GET` | `/users/:id` | User detail |
| `PUT` | `/users/:id/role` | Change user role |
| `PUT` | `/users/:id/ban` | Ban / unban user |

**Games**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/games` | List all games |
| `PUT` | `/games/:id/review` | Submit review decision |
| `PUT` | `/games/:id/approve` | Approve game for marketplace |
| `PUT` | `/games/:id/feature` | Feature / unfeature game |

**Finance**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/revenue` | Platform revenue dashboard |
| `GET` | `/transactions` | All transactions |
| `POST` | `/payouts/process` | Process pending payouts |
| `POST` | `/payouts/:id/cancel` | Cancel a payout |

**Analytics**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/analytics/overview` | Platform overview stats |
| `GET` | `/analytics/revenue` | Revenue time series |
| `GET` | `/analytics/users` | User growth and activity |
| `GET` | `/analytics/games` | Per-game analytics |
| `GET` | `/analytics/performance` | System performance metrics |

**Misc**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Admin health check |
| `GET` | `/metrics` | Internal metrics |
| `POST` | `/seed` | Seed test data (non-production only) |

---

## Health & Metrics

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Simple `"ok"` response (no auth) |
| `GET` | `/health/ready` | Readiness probe: DB + Redis connectivity |
| `GET` | `/health/live` | Liveness probe |
| `GET` | `/metrics` | Prometheus-format metrics |

---

## Categories — `/api/v1/categories`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | — | List game categories |

---

## Rate limiting

Rate limiting is enforced by a Redis-backed sliding-window limiter. The default
configuration (`RateLimitConfig::default()`) applies globally. Configure via environment
variables or `RATE_LIMIT_*` settings. HTTP 429 is returned when the limit is exceeded.

---

## Error codes

| HTTP status | Meaning |
|-------------|---------|
| 400 | Validation error — check `message` for details |
| 401 | Missing or invalid JWT |
| 403 | Insufficient role |
| 404 | Resource not found |
| 409 | Conflict (e.g. duplicate username) |
| 422 | Unprocessable entity |
| 429 | Rate limited |
| 500 | Internal server error |
