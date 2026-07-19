# API Reference

The Magnetite REST API is served at `/api/v1/` on port 8080.

> **This is the *node's* API, not a cloud API.** Anyone runs the node binary.
> Nothing here is a central authority: the discovery tracker is a phonebook,
> and payments are non-custodial — no balances, deposits, withdrawals, or
> payouts exist. Endpoints removed with the fiat on-ramp are called out
> explicitly below so stale client code fails loudly rather than silently.

All endpoints return JSON in one of two envelopes:

**Success:**
```json
{ "success": true, "data": { … }, "error": null }
```

**Paginated success:**
```json
{
  "success": true,
  "data": [ … ],
  "pagination": { "page": 1, "per_page": 20, "total": 142, "total_pages": 8 }
}
```

**Error:**
```json
{ "success": false, "data": null, "error": { "code": "…", "message": "…", "details": null } }
```

(A few older handlers return their payload unwrapped; the envelope above is
`backend/src/api/response.rs`, which the majority use.)

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

## Discovery — `/api/v1/discovery`

The tracker: a **phonebook, not an authority**. It stores signed, leased
self-advertisements from nodes and serves them back. It verifies signatures and
leases; it certifies nothing about what a node claims.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/announce` | Ed25519 signature | Publish or renew a `SignedAd`. Heartbeat by re-announcing |
| `DELETE` | `/announce` | Ed25519 signature | Deregister via a `SignedWithdraw` |
| `GET` | `/sessions` | — | Query live sessions |

`GET /sessions` filters: `game` (plain BLAKE3 hex), `max_ping`,
`free_slots_only`, `free_only`, `max_price`, `limit`. Only unexpired leases are
ever served.

**Announce body** — the seam's `SessionAd` shape, flattened, plus the
signature envelope:

```json
{
  "game": "<blake3 hex>",
  "node": "<node address>",
  "capacity": { "cpu_cores": 8, "ram_mb": 16384, "bandwidth_mbps": 1000,
                "free_slots": 120, "max_shards": 8 },
  "ping_hint": 24,
  "price": { "amount": 0, "currency": "USDC", "unit": "per_hour" },
  "chat_room": null,
  "voice_room": null,
  "operator": "self-declared, optional",
  "region": "self-declared, optional"
}
```

Rejected fail-closed (before any DB query, so these return 4xx without
touching storage): unsigned ads, forged or price-tampered signatures,
relabelled node keys, relabelled operator/region labels, expired,
future-dated, or over-long leases (`MAX_AD_TTL_SECS = 600`), non-hex game
filters, and forged withdrawals. An upsert is bound to the announcing key, so
one node cannot take over another's `(game, node)` slot — that returns 403.

Responses additionally carry tracker bookkeeping (`id`, `node_key`, `players`,
`max_players`, `expires_at`) and best-effort catalog resolution
(`game_title`, `game_version`), which are **null for any hash this tracker has
never indexed** — the normal case in a decentralized network, not an error.

> **Not done:** there is no multi-tracker gossip. A client queries the trackers
> it is statically configured with (`FanoutDiscovery`); trackers do not
> discover each other.

---

## Wallet — `/api/v1/wallet`

**Address-only. No balances, no deposits, no withdrawals, no payouts** — the
platform holds no funds. See [Payments](../payments.md).

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | required | Linked wallet address + custody posture (`custodial: false`) + rail |
| `POST` | `/link` | required | Link an Ed25519 wallet address (32-byte hex) to the account |
| `GET` | `/receipts` | required | Signed `payment_receipts` for this user |
| `POST` | `/hosting/pay` | required | Open a hosting-fee payment channel (`PaymentRail::open_channel`) |
| `GET` | `/hosting/:server_id` | required | Join-gate check for a paid server |

The removed endpoints — `GET /balance`, `POST /deposit`, `POST /withdraw`,
`GET /transactions` — no longer exist.

> **Not done:** `POST /wallet/link` does not yet demand a signed challenge
> proving key ownership, and hosting channels are a scaffold — the mock rail
> returns a deterministic channel id and there are no off-chain signed channel
> updates per join.

---

## Developer — `/api/v1/developer`

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/register` | required | Register as a developer |
| `GET` | `/dashboard` | required | Dashboard stats (DAU, revenue, sessions) |
| `GET` | `/games` | required | Developer's own games |
| `PUT` | `/games/:id/status` | required | Update game status (e.g. submit for review) |
| `DELETE` | `/games/:id` | required | Delete a game |
| `GET` | `/earnings` | required | Receipt-backed earnings summary (`pending_payout` is always `0`) |
| `GET` | `/wallet` | required | The developer's linked payee address |
| `GET` | `/games/:id/players` | required | Per-game analytics |
| `GET` | `/games/:id/analytics` | required | Alias of the above |
| `POST` | `/games/scaffold` | required | Scaffold a game from a template |
| `POST` | `/games/:id/build` | required | Trigger a build |
| `GET` | `/games/:id/build-status` | required | Build status |
| `GET` | `/games/:id/versions` | required | List versions |
| `PUT` | `/games/:game_id/versions/:version_id/promote` | required | Promote a version |
| `PUT` | `/games/:game_id/versions/:version_id/rollback` | required | Roll a version back |

`GET /payouts` and `POST /payouts` were **deleted** — nothing is held on a
developer's behalf, so there is nothing to request.

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

Tiers are **receipt-backed feature flags**, not a recurring charge — see
[Subscription Lifecycle](../subscriptions-lifecycle.md).

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/` | — | List active tiers |
| `GET` | `/plans` | — | Alias of `/` |
| `GET` | `/me` | required | Current subscription |
| `GET` | `/current` | required | Alias of `/me` |
| `POST` | `/` | required | Subscribe. Paid tiers require `payment_id` = a `payment_receipts` id owned by the caller |
| `DELETE` | `/` | required | Cancel at period end |
| `POST` | `/cancel` | required | Alias of `DELETE /` |
| `POST` | `/upgrade` | required | Move to a higher tier (prorated delta must be receipt-covered) |
| `POST` | `/downgrade` | required | Move to a lower tier |
| `GET` | `/hours` | required | Included compute hours |
| `GET` | `/usage` | required | Game-slot usage this period |

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
| `POST` | `/game` | secret | Game-server event (session end, score) |
| `GET` | `/endpoints` | admin | List registered webhook endpoints |
| `POST` | `/endpoints` | admin | Register a webhook endpoint |
| `DELETE` | `/endpoints/:id` | admin | Remove a webhook endpoint |

**There are no payment webhooks.** `POST /paystack` and `POST /circle` were
deleted with the fiat on-ramp. Payment truth arrives as a signed `Receipt`, not
as a provider callback.

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

**Settlement** (read-only — there is no money-moving admin action)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/revenue` | Settlement dashboard: `total_settled_units`, `settled_receipts`, `voided_receipts` |
| `GET` | `/transactions` | Transaction log |

`POST /payouts/process` and `POST /payouts/:id/cancel` were **deleted**, along
with the custodial `POST /admin/transactions/:id/refund`. Refunds now go
through `POST /api/v1/marketplace/purchases/:purchase_id/refund`, which voids
the signed receipt and revokes the entitlement — see [Refunds](../refunds.md).

**Moderation**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/review-reports` | Flagged reviews |
| `POST` | `/review-reports/:id/action` | Act on a review report |
| `GET` | `/chat-flags` | Flagged chat messages |
| `POST` | `/chat-flags/:id/action` | Act on a chat flag |

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

## Marketplace — `/api/v1/marketplace`

Stores, items, non-custodial purchase, receipts, entitlements, and refunds.
Documented in full in [Developer Marketplace](../for-developers/marketplace.md)
and [Economy & Marketplace](../economy-marketplace.md).

---

## Points — `/api/v1/points`

Off-chain XP/score ledger. **Points are not money and are not tokenized.**
See [Economy & Marketplace](../economy-marketplace.md).

---

## Comms — `/api/v1/comms`

Room creation and join-credential minting through the `CommsProvider` seam
(`builtin` by default; Matrix / Jitsi / LiveKit / Owncast if configured). A
paid room only issues a credential after the payment receipt re-verifies.
See [Comms](../comms.md).

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
