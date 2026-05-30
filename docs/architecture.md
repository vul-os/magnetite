# Architecture Overview

Magnetite is a server-authoritative Rust game platform. This page describes the real
backend modules as they exist in the codebase — updated through Wave 9 (full gaming suite).

---

## High-level diagram

```
Browser / Native Client
        │  HTTPS REST                WebSocket
        │  /api/v1/…                 /ws/comms  /ws/voice  /ws/game/{id}
        ▼                            ▼
┌────────────────────────────────────────────────────────────────┐
│               Axum HTTP Server  (0.0.0.0:8080)                 │
│                                                                │
│  Rate limiter (Redis)  ──►  Request logger                     │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                      API Layer (34 modules)             │   │
│  │  auth  games  wallet  developer  admin                  │   │
│  │  matchmaking  leaderboard  achievements  social         │   │
│  │  subscriptions  notifications  profile  tournaments     │   │
│  │  oauth  github  webhooks  distribution                  │   │
│  │  communities  channels  messages                        │   │
│  │  points  marketplace                                    │   │
│  │  categories  health  metrics  versioning  wishlist      │   │
│  └──────────────────────┬──────────────────────────────────┘   │
│                         │                                      │
│  ┌──────────────────────▼──────────────────────────────────┐   │
│  │               Services Layer (22 modules)               │   │
│  │  auth  games  wallet  payment  payout                   │   │
│  │  session  leaderboard  matchmaking                      │   │
│  │  achievements  friends  invites                         │   │
│  │  analytics  anticheat  cache  email                     │   │
│  │  health  verification  distribution                     │   │
│  │  communities  presence  points  marketplace             │   │
│  └──────────────────────┬──────────────────────────────────┘   │
│                         │                                      │
│  ┌──────────────────────▼──────────────────────────────────┐   │
│  │  Background Jobs   │  WebSocket Handlers                 │   │
│  │  session_cleanup   │  ws/comms.rs  (chat + presence)    │   │
│  │  notification_gc   │  ws/voice.rs  (WebRTC signaling)   │   │
│  │  backup            │  ws/game.rs   (game state sync)    │   │
│  └──────────────────────┬──────────────────────────────────┘   │
└────────────────────────┼───────────────────────────────────────┘
                         │
            ┌────────────┴──────────────┐
            │                           │
     PostgreSQL 16                   Redis 7
     (24 migrations)              (rate limiting,
                                   sessions, cache,
                                   pub/sub)
```

---

## Backend module reference

### `src/api/` — HTTP route modules (34 total)

Each module exports a `router(pool: PgPool) -> Router` function nested under
`/api/v1/<prefix>` in `main.rs`.

| Module | Base path | Key routes |
|--------|-----------|------------|
| `auth` | `/auth` | `POST /login`, `POST /register`, `POST /refresh`, `DELETE /logout`, `DELETE /logout-all`, `GET /sessions`, `GET /me` |
| `wallet` | `/wallet` | `GET /balance`, `POST /deposit`, `POST /withdraw`, `GET /transactions` |
| `games` | `/games` | `GET /`, `POST /`, `GET /:id`, `PUT /:id`, `DELETE /:id`, `GET /:id/leaderboard` |
| `distribution` | `/distribution` | `GET /:game_id/play`, `GET /:game_id/build-status`, `GET /:game_id/artifacts`, `POST /:game_id/versions`, `PUT /:game_id/versions/:version_id/promote` |
| `communities` | `/communities` | `GET /`, `POST /`, `GET /:id`, `PUT /:id`, `DELETE /:id`, `GET /:id/members`, `POST /:id/join`, `DELETE /:id/leave` |
| `channels` | `/channels` | `GET /communities/:id/channels`, `POST /communities/:id/channels`, `PUT /:id`, `DELETE /:id` |
| `messages` | `/messages` | Channel messages: `GET /channels/:id/messages`, `POST /channels/:id/messages`. DMs: `GET /dms`, `GET /dms/:thread_id/messages`, `POST /dms/:thread_id/messages` |
| `points` | `/points` | `GET /balance`, `GET /balance/:user_id`, `POST /award`, `POST /spend`, `GET /history`, `GET /leaderboard`, `POST /season-reset` |
| `marketplace` | `/marketplace` | `GET /stores/:game_id`, `POST /stores`, `PUT /stores/:id`, `GET /stores/:id/items`, `POST /stores/:id/items`, `PUT /stores/:id/items/:item_id`, `POST /items/:item_id/purchase`, `GET /entitlements`, `GET /stores/:game_id/revenue` |
| `categories` | `/categories` | game category listing |
| `leaderboard` | `/leaderboard` | `GET /:game_id`, `POST /:game_id/scores`, `GET /:game_id/me`, `GET /:game_id/friends` |
| `matchmaking` | `/matchmaking` | `POST /join`, `DELETE /leave`, `GET /status` |
| `developer` | `/developer` | `POST /register`, `GET /dashboard`, `GET /games`, `PUT /games/:id/status`, `DELETE /games/:id`, `GET /earnings`, `GET /payouts`, `POST /payouts`, `GET /games/:id/players` |
| `admin` | `/admin` | Users, games moderation, finance, analytics, health, metrics, seed |
| `oauth` | `/oauth` | Google, Discord, GitHub, GitLab; `/callback` per provider |
| `github` | `/github` | `POST /webhooks/github`, `GET /installations`, `GET /repos`, `POST /repos/register` |
| `webhooks` | `/webhooks` | `POST /paystack`, `POST /circle`, `POST /game`, endpoint CRUD |
| `achievements` | `/achievements` | `GET /:user_id`, `POST /:user_id/:id/progress`, `GET /leaderboard` |
| `social` | `/friends`, `/invites`, `/users` | Friends, invites, user search |
| `subscriptions` | `/subscriptions` | list tiers, subscribe, cancel, status |
| `notifications` | `/notifications` | list, mark read, WS push channel |
| `health` | `/health/ready`, `/health/live` | Readiness / liveness probes |
| `metrics` | `/metrics` | Prometheus-format metrics |
| `versioning` | middleware | API version header negotiation |
| `wishlist` | `/wishlist` | per-user game wishlists |
| (remaining) | | `profile`, `tournaments`, `reviews`, `sessions`, `search`, `platform` |

> **Note:** All routes requiring authentication pass through `middleware::auth_middleware`
> (JWT Bearer token validation). Admin routes additionally require the `admin` role
> via `middleware::admin_middleware`.

---

### `src/services/` — Business logic (22 modules)

Pure Rust functions that contain no HTTP concerns. Called by API handlers.

| Service | Responsibility |
|---------|---------------|
| `auth` | Password hashing (Argon2), credential verification |
| `session` | JWT issuance/validation, refresh tokens, session table management |
| `games` | Game record CRUD, status transitions |
| `wallet` | USDC balance management, deposit/withdrawal validation |
| `payment` | Circle (USDC) and Paystack integration — real HTTP clients gated on `CIRCLE_API_KEY`/`PAYSTACK_SECRET_KEY`; sandbox mode via `PAYMENTS_SANDBOX=true` |
| `payout` | Developer earnings calculation (70/30 split), payout request/process; dispatches real Circle `/v1/transfers` in production |
| `leaderboard` | Score submission, rank queries, friend scores |
| `matchmaking` | Queue management, player pairing logic; `start_game_session()` sets `server_endpoint` from `GAME_SERVER_WS_BASE` env var; estimated wait time derived from actual queue depth |
| `achievements` | Progress tracking, unlock conditions |
| `friends` | Friend request state machine, block list |
| `invites` | Game session invite creation and acceptance |
| `analytics` | Aggregated platform metrics for admin dashboard |
| `anticheat` | Input velocity checks, score anomaly detection, session integrity; wired into `ws/game.rs` (ban check on connect, velocity enforcement per tick, anomaly scan + replay store on disconnect) |
| `cache` | Redis wrapper — get/set/delete/invalidate |
| `email` | Multi-provider email dispatch — `ResendProvider` (HTTPS POST to Resend) or `SesProvider` (lettre SMTP to AWS SES endpoint); provider selected by `EMAIL_PROVIDER` env var; wired into registration, verification, password-reset, and payout-notification flows |
| `health` | Database and Redis connectivity checks |
| `verification` | Email verification token issuance and validation |
| `distribution` | Game artifact and version record management, play manifest resolution |
| `communities` | Community + channel + message service; DM thread management |
| `presence` | Presence upsert on WS connect/disconnect; offline sweep |
| `points` | Atomic ledger insert + balance update (single TX); season reset; leaderboard |
| `marketplace` | Store/item CRUD; USDC purchase (70/30 split); points purchase; entitlements |

---

### `src/jobs/` — Background workers

All jobs are `tokio::spawn`'d at startup in `main.rs`.

| Job | Interval | Function |
|-----|----------|----------|
| `notification_cleanup` | 1 h | Garbage-collects old read notifications |
| `session_cleanup` | 1 h | Expires stale auth sessions, old matchmaking entries, and expired password-reset tokens |
| `verification_cleanup` | 1 h | Purges expired and used email-verification tokens |
| `payout batch` | 1 h | Calls `PayoutService::process_pending_payouts()`; dispatches Circle USDC transfers for pending rows |
| `subscription renewal` | 1 h | Calls `SubscriptionService::process_renewals()`; handles expired/renewed subscriptions |
| `backup` | (not spawned) | `pg_dump` + S3/local storage — code exists but not yet scheduled; run manually via `backend/tools/backup.sh` |

---

### `src/ws/` — WebSocket handlers

| Module | Endpoint | Purpose |
|--------|----------|---------|
| `ws/game.rs` | `/ws/game/{id}` | Real-time game state sync — receives `Input` messages, calls `GameLogic::tick`, broadcasts `GameState` snapshots |
| `ws/comms.rs` | `/ws/comms?token=<jwt>` | Real-time chat + typing indicators + presence broadcast; per-channel `tokio::sync::broadcast` |
| `ws/voice.rs` | `/ws/voice?token=<jwt>&room=<token>` | WebRTC SDP/ICE signaling relay; mesh for ≤15 participants; SFU (LiveKit/mediasoup) documented as scale path |
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
and WASM alike.

| Module | Key types |
|--------|-----------|
| `game` | `GameLogic` trait, `GameMetadata`, `export_game!` macro |
| `state` | `GameState`, `Snapshot`, `PlayerId`, `PlayerState`, `Position`, `Rotation` |
| `input` | `Input`, `Action`, `KeyCode`, `MouseState`; `input::gamepad`: `InputMap`, `GameAction`, `GamepadButton`, `GamepadAxis` |
| `graphics` | `GraphicsTier` (Lite2D / Standard3D / Advanced3D), `RenderConfig`, `EngineCapability` |
| `protocol` | `Envelope`, `ClientMessage`, `ServerMessage`, `PROTOCOL_VERSION` |
| `networking` | `ServerConfig`, `TickLoop`, `PredictionBuffer`, `InterestManager`, `NetworkManager` |
| `platform::comms` | `CommsClient`, `ChatMessage`, `VoiceSignal`, `PresenceUpdate` |
| `platform::points` | `PointsClient`, `AwardPointsRequest`, `SpendPointsRequest`, `LedgerEntry` |
| `platform::marketplace` | `MarketplaceClient`, `StoreItem`, `PurchaseRequest`, `Entitlement` |
| `platform::cloud_save` | `CloudSaveClient`, `SaveSlot`, `SaveRequest` |
| `platform::streaming` | `StreamClient`, `GoLiveRequest`, `StreamInfo`, `StreamStatus`, `ExternalRtmpTarget` |

240 tests pass; 0 warnings. See the [SDK Reference](./for-developers/sdk.md).

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
