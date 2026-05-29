# Architecture Overview

Magnetite is a server-authoritative Rust game platform. This page describes the real
backend modules as they exist in the codebase.

---

## High-level diagram

```
Browser / Native Client
        │  HTTPS REST          WebSocket
        │  /api/v1/…           /ws/notifications
        ▼                      ▼
┌───────────────────────────────────────────────────────┐
│            Axum HTTP Server  (0.0.0.0:8080)           │
│                                                       │
│  Rate limiter (Redis)  ──►  Request logger            │
│                                                       │
│  ┌──────────────────────────────────────────────┐     │
│  │                  API Layer                   │     │
│  │  auth  games  wallet  developer  admin       │     │
│  │  matchmaking  leaderboard  achievements      │     │
│  │  social  subscriptions  notifications        │     │
│  │  oauth  github  webhooks  distribution       │     │
│  │  categories  health  metrics  versioning     │     │
│  └──────────────────────┬───────────────────────┘     │
│                         │                             │
│  ┌──────────────────────▼───────────────────────┐     │
│  │               Services Layer                 │     │
│  │  auth  games  wallet  payment  payout        │     │
│  │  session  leaderboard  matchmaking           │     │
│  │  achievements  friends  invites              │     │
│  │  analytics  anticheat  cache  email          │     │
│  │  health  verification  distribution          │     │
│  └──────────────────────┬───────────────────────┘     │
│                         │                             │
│  ┌──────────────────────▼───────────────────────┐     │
│  │  Background Jobs  │  WebSocket Handler        │     │
│  │  session_cleanup  │  ws/game.rs               │     │
│  │  notification_gc  │  ws/mod.rs                │     │
│  │  backup           │                           │     │
│  └──────────────────────┬───────────────────────┘     │
└────────────────────────┼──────────────────────────────┘
                         │
            ┌────────────┴──────────────┐
            │                           │
     PostgreSQL 16                   Redis 7
     (primary store)              (rate limiting,
                                   sessions, cache)
```

---

## Backend module reference

### `src/api/` — HTTP route modules

Each module exports a `router(pool: PgPool) -> Router` function that is nested under
`/api/v1/<prefix>` in `main.rs`.

| Module | Base path | Key routes |
|--------|-----------|------------|
| `auth` | `/auth` | `POST /login`, `POST /register`, `POST /refresh`, `DELETE /logout`, `DELETE /logout-all`, `GET /sessions`, `GET /me` |
| `wallet` | `/wallet` | `GET /balance`, `POST /deposit`, `POST /withdraw`, `GET /transactions` |
| `games` | `/games` | `GET /`, `POST /`, `GET /:id`, `PUT /:id`, `DELETE /:id`, `GET /:id/leaderboard` |
| `distribution` | `/distribution` | `GET /:game_id/play`, `GET /:game_id/build-status`, `GET /:game_id/artifacts`, `GET /:game_id/artifacts/:artifact_id`, `GET /:game_id/versions`, `POST /:game_id/versions`, `PUT /:game_id/versions/:version_id/promote`, `PUT /:game_id/artifacts/:artifact_id` |
| `categories` | `/categories` | game category listing |
| `leaderboard` | `/leaderboard` | `GET /:game_id`, `POST /:game_id/scores`, `GET /:game_id/me`, `GET /:game_id/friends` |
| `matchmaking` | `/matchmaking` | `POST /join`, `DELETE /leave`, `GET /status` |
| `developer` | `/developer` | `POST /register`, `GET /dashboard`, `GET /games`, `PUT /games/:id/status`, `DELETE /games/:id`, `GET /earnings`, `GET /payouts`, `POST /payouts`, `GET /games/:id/players` |
| `admin` | `/admin` | Users: `GET /users`, `GET /users/:id`, `PUT /users/:id/role`, `PUT /users/:id/ban`. Games: `GET /games`, `PUT /games/:id/review`, `PUT /games/:id/approve`, `PUT /games/:id/feature`. Finance: `GET /revenue`, `GET /transactions`, `POST /payouts/process`, `POST /payouts/:id/cancel`. Analytics: `GET /analytics/overview`, `GET /analytics/revenue`, `GET /analytics/users`, `GET /analytics/games`, `GET /analytics/performance`. Misc: `GET /health`, `GET /metrics`, `POST /seed` |
| `oauth` | `/oauth` | `GET /google`, `GET /google/callback`, `GET /discord`, `GET /discord/callback`, `GET /github`, `GET /github/callback`, `GET /gitlab`, `GET /gitlab/callback` |
| `github` | `/github` | `POST /webhooks/github`, `GET /installations`, `GET /repos`, `POST /repos/register`, `GET /repos/:owner/:repo/build-status` |
| `webhooks` | `/webhooks` | `POST /paystack`, `POST /circle`, `POST /game`, `GET /endpoints`, `POST /endpoints`, `DELETE /endpoints/:id` |
| `achievements` | `/achievements` | `GET /:user_id`, `GET /:user_id/:id`, `POST /:user_id/:id/progress`, `GET /leaderboard` |
| `social` (friends) | `/friends` | `GET /`, `POST /request`, `POST /accept/:id`, `POST /reject/:id`, `DELETE /:id`, `POST /block/:id` |
| `social` (invites) | `/invites` | `GET /`, `POST /:id/accept`, `POST /:id/decline` |
| `social` (users) | `/users` | `GET /search`, `GET /:id` |
| `subscriptions` | `/subscriptions` | `GET /` (list tiers), subscribe, cancel, status |
| `notifications` | `/notifications` | `GET /`, `GET /count`, `PUT /read-all`, `PUT /:id/read`, `DELETE /:id`, `POST /` + `GET /ws/notifications` (WebSocket) |
| `health` | `/health/ready`, `/health/live` | Kubernetes-style readiness / liveness probes |
| `metrics` | `/metrics` | Prometheus-format metrics |
| `versioning` | middleware | API version header negotiation |

> **Note:** All routes requiring authentication pass through `middleware::auth_middleware`
> (JWT Bearer token validation). Admin routes additionally require the `admin` role
> via `middleware::admin_middleware`.

---

### `src/services/` — Business logic

Pure Rust functions that contain no HTTP concerns. Called by API handlers.

| Service | Responsibility |
|---------|---------------|
| `auth` | Password hashing (bcrypt), credential verification |
| `session` | JWT issuance/validation, refresh tokens, session table management |
| `games` | Game record CRUD, status transitions |
| `wallet` | USDC balance management, deposit/withdrawal validation |
| `payment` | Circle (USDC) and Paystack webhook event processing |
| `payout` | Developer earnings calculation (85/15 split), payout request/process |
| `leaderboard` | Score submission, rank queries, friend scores |
| `matchmaking` | Queue management, player pairing logic |
| `achievements` | Progress tracking, unlock conditions |
| `friends` | Friend request state machine, block list |
| `invites` | Game session invite creation and acceptance |
| `analytics` | Aggregated platform metrics for admin dashboard |
| `anticheat` | Input velocity checks, score anomaly detection, session integrity |
| `cache` | Redis wrapper — get/set/delete/invalidate |
| `email` | Multi-provider email dispatch (Resend, SMTP, AWS SES) |
| `health` | Database and Redis connectivity checks |
| `verification` | Email verification token issuance and validation |
| `distribution` | Game artifact and version record management, play manifest resolution |

---

### `src/jobs/` — Background workers

Launched as `tokio::spawn` tasks at startup.

| Job | Function |
|-----|----------|
| `session_cleanup` | Periodically deletes expired refresh-token records |
| `notification_cleanup` | Garbage-collects old read notifications |
| `backup` | Schedules periodic database backup exports |

---

### `src/ws/` — WebSocket handlers

| Module | Endpoint | Purpose |
|--------|----------|---------|
| `ws/game.rs` | (internal) | Real-time game state sync — receives `Input` messages, calls `GameLogic::tick`, broadcasts `GameState` snapshots |
| `ws/mod.rs` | `/ws/notifications` | Per-user notification push channel |

---

### `src/middleware/`

| Middleware | Role |
|------------|------|
| `cors_layer` | CORS headers, configurable allowed origins |
| `rate_limit` | Redis-backed sliding-window rate limiter |
| `logging::log_request` | Structured request/response logging |
| `auth_middleware` | Validates `Authorization: Bearer <token>` JWT |
| `admin_middleware` | Requires authenticated user with `admin` role |

---

### `src/db/`

| Module | Role |
|--------|------|
| `pool.rs` | Creates the `PgPool` from `DATABASE_URL`; runs `sqlx::migrate!()` on startup |
| `mod.rs` | Re-exports `get_db_pool`, `init_db` |

---

### `magnetite-sdk/` — Rust SDK

A pure Rust library with no async runtime or HTTP dependencies — compiles to native
and WASM alike. See the [SDK Reference](./for-developers/sdk.md).

---

## Frontend

React 19 + Vite SPA (`src/`). Communicates with the backend only through the REST API
(`src/api/` axios wrappers) and the WebSocket notification channel. The frontend has
no direct database access.

Pages call real API endpoints when they exist; a mock-data fallback is used during
development when the backend is unavailable. The frontend is served by the Nginx
container in production (`nginx.conf`).

---

## Data flow: player plays a game

```
1. GET /api/v1/distribution/:game_id/play
   → PlayManifest { wasm_url, server_url, version, sha256_hash }

2. Browser fetches wasm_url, instantiates WASM module

3. WebSocket connect to server_url (game session)
   → Message::PlayerJoin(player_id)

4. Game loop:
   Client sends Message::Input(Input { keys, mouse, timestamp })
   Server calls GameLogic::handle_input → tick → state()
   Server broadcasts Message::StateSync(GameState)

5. Session ends:
   Message::PlayerLeave | game_over event
   Score recorded → leaderboard updated → earnings calculated
```

---

## See also

- [API Reference](./api-reference/index.md)
- [Security & Sandboxing](./security/index.md)
- [Self-Hosting Guide](./self-hosting/index.md)
