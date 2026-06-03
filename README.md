# Magnetite

**Magnetite (Fe₃O₄)** — Iron oxide, magnetic, grounded. The foundation upon which things are built.

*Build, distribute, and monetize Rust games — from a weekend jam to a COD-scale AAA title.*

---

## The Magnetite Moat — one Rust game, jam to AAA

> Nobody gives an open, Rust-native "same code, jam-to-AAA, authoritative + sandboxed + anti-cheat +
> one-command-deploy" primitive. That's what this is.

Three interlocking differentiators form one system that competitors (Nakama, PlayFab, Roblox) do not offer together:

### Scale primitive — identical game code, topology auto-selected

Write your game once against `magnetite-sdk::authority::AuthoritativeGame`. The platform runs it at any scale:

| Topology | Player count | How |
|----------|-------------|-----|
| `SingleRoom` | up to ~16 | 1 process, broadcast-all |
| `Dedicated` | up to ~256 | authoritative server, interest-filtered snapshots |
| `Sharded` | AAA / unbounded | spatial shards + cross-shard handoff (N1 local seam; multi-node is Bucket D) |

`MatchConfig::auto(n)` escalates topology by player count. Your game code is identical across all three.

Perf numbers (debug build, single-threaded in-proc, `magnetite-e2e` scale bench):

| Scenario | ticks/sec | μs/tick |
|----------|-----------|---------|
| SingleRoom (4 players) | 203,116 | 4.92 |
| SingleRoom (16 players) | 185,399 | 5.39 |
| Dedicated (32 players) | 151,388 | 6.61 |
| Dedicated (64 players) | 114,215 | 8.76 |
| Dedicated (128 players) | 78,950 | 12.67 |
| Dedicated (256 players) | 50,591 | 19.77 |

A release build is ~3–5× faster. The smoke-check assertion `ticks/sec ≥ 1,000` is met with large margin.

### Wasmtime sandbox — untrusted game logic, deterministic by construction

Game logic compiles to `wasm32-wasip1` and runs inside a `WasmExecutor` with hard guarantees:

- **Fuel budget** per tick (`fuel_per_step`) — runaway loops cannot stall the server.
- **Memory cap** (`max_memory_bytes`) — guest cannot exhaust host RAM.
- **Epoch interrupt** (`epoch_tick_ms × max_epochs_per_step`) — wall-clock timeout per step.
- **No OS randomness, no wall clock** — `random_get` and `clock_time_get` return `ENOSYS`. The only
  randomness source is `StepCtx.rng` (seeded `DeterministicRng`, xoshiro256**).

Result: same `(state, ordered commands, tick, seed)` always produces the same result, on any host.

### Anti-cheat by construction — server-authoritative + deterministic replay verification

Anti-cheat is not a bolt-on; it is the architecture:

1. Clients send *inputs*; the server runs `AuthoritativeGame::validate` to reject illegal actions, then
   `AuthoritativeGame::step` to advance state. Clients never send state.
2. The runtime records a `ReplayLog` (every tick's inputs + `state_hash`). `verify_replay` re-simulates
   from scratch; any divergence is tamper evidence or a determinism bug.
3. `magnetite-anticheat` adds composable `Validator`s (aimbot snap, position teleport, fire-rate flood)
   and a `TrustScoreMap` (Warn → Kick → Ban escalation with decay).

### One-command pipeline

```bash
cargo install magnetite-cli        # install once

magnetite new my-game              # scaffold a crate implementing AuthoritativeGame
cd my-game
magnetite build                    # cargo build --release --target wasm32-wasip1 → game.wasm
magnetite dev                      # build → WasmExecutor → SingleRoom server → ws://127.0.0.1:<port>
magnetite deploy                   # build → register artifact with backend → live instance
```

### Crate map

| Crate | Role |
|-------|------|
| `backend/magnetite-sdk` (`::authority`) | Frozen traits: `AuthoritativeGame`, `GameExecutor`, `NativeExecutor`, `Validator`, `ReplayLog`, `verify_replay`, `Topology`, `MatchConfig`, `DeterministicRng` |
| `magnetite-runtime` | Authoritative game-server host: tick loop, WebSocket connection mgmt, interest-filtered delta/snapshot fan-out, `ShardManager` seam; `magnetite-serve` binary |
| `magnetite-sandbox` | `WasmExecutor` — Wasmtime host implementing `GameExecutor`; fuel/memory/epoch limits; WASI stubs (no clock, no rng) |
| `magnetite-anticheat` | Composable validators, `TrustScoreMap`, `ReplayVerifier` |
| `magnetite-cli` | `magnetite new|build|dev|deploy` binary |
| `magnetite-web-client` | JS web client speaking `ClientNet`/`ServerNet`; prediction buffer; canvas renderer; in-browser replay playback |
| `game-template-authoritative` | Reference game (top-down arena shooter) implementing `AuthoritativeGame`; canonical wasm ABI exports behind `--features wasm` |
| `game-client-bevy` | Bevy client with prediction/reconciliation (`PredictionBuffer` + `ClientPredictor`) wired to `ServerNet` |
| `magnetite-e2e` | Integration tests: convergence + `verify_replay` clean + anti-cheat WS rejection + wasm parity vs native + full-stack WS + scale bench |

### Proved end-to-end (magnetite-e2e)

The `magnetite-e2e` suite (9 passing tests) proves the full pipeline:

- `WasmExecutor` and `NativeExecutor` produce **identical `state_hash`** on every tick (seed `0xDEADCAFE1337BABE`, 30 ticks).
- `verify_replay` returns `ReplayVerdict::Clean` over that run.
- The live WebSocket server delivers `Snapshot`/`Delta` frames to all connected clients.
- Cheating inputs (position delta 9999, threshold 100) receive `ServerNet::Reject`; the `TrustScoreMap` escalates.
- Full-stack test (`fullstack_ws_welcome_snapshot_delta_ack_and_replay_clean`): 3 real `tokio-tungstenite`
  clients, 10 rounds of inputs, both `Snapshot` and `Delta` frames confirmed, convergence proven, replay clean.

See [`docs/MOAT-ARCHITECTURE.md`](docs/MOAT-ARCHITECTURE.md) and [`docs/moat/`](docs/moat/) for the frozen
interface contracts, replay/spectator protocol, and the full wave plan.

---

## Vision

Magnetite is the open-source **unified gaming suite** for Rust game development at any scale. Game logic is authored in Rust; clients compile Bevy to WASM for the browser and to native binaries. The platform is server-authoritative and sandboxed, providing the heavy lifting — hosting, matchmaking, real-time netcode, persistence, comms, economy, and payment rails — so developers only write game logic.

- **Rust-first.** Not HTML5. Not Unity. Bevy → WASM (browser) + native. Servers are sandboxed Rust.
- **Scales with the game.** A tiny 2D arcade game and a large FPS or motorsport title share the same SDK and platform.
- **Distribution built in.** Storefront/marketplace: players discover, play (in-browser via WASM or native), and pay.
- **Communities & comms.** Discord-class servers, channels, real-time text chat, voice (WebRTC), presence, and streaming — all plugged into every game automatically.
- **Points economy.** Platform-wide XP/score system with seasons, ledger, leaderboard, and rewards.
- **Dev-run marketplaces.** Developers create in-game stores (cosmetics, items, DLC, passes) with a shared checkout and 70/30 revenue split.
- **Controllers.** First-class gamepad support (Gamepad API + gilrs) with a unified binding layer.
- **Graphics tiers.** `Lite2D` → `Standard3D` → `Advanced3D` — simple games stay lightweight, AAA games scale up.
- **Open source.** Platform MIT, SDK MIT, game templates MIT.
- **Real money, fiat only.** USD-denominated balances, Paystack fiat on-ramp for deposits and subscriptions, Wise payouts for developers, **30% platform fee / 70% developer split**.
- **PWA + mobile.** Installable progressive web app; responsive at 360/768/1280; bottom navigation on mobile.
- **i18n.** English, Spanish, French locales; browser-language auto-detection via `I18nProvider`.

---

## Gaming Suite Features

### Communities & Comms (Discord-class)

| Feature | Detail |
|---------|--------|
| Servers / guilds | Create communities with channels, roles, and members |
| Text channels | Real-time chat over WebSocket; persisted to PostgreSQL |
| Voice | WebRTC mesh (backend as signaling server); SFU (LiveKit/mediasoup) as documented scale path |
| Presence | Online / idle / DND / in-game / offline; updated by the WS heartbeat |
| Direct messages | 1-to-1 DM threads with the same message store |
| Streaming | Go live with `getDisplayMedia` + RTMP egress to Twitch/YouTube via MediaMTX; in-platform HLS/WebRTC watch |
| In-game overlay | Auto-provisioned voice + text room for every lobby/match; accessible via `platform::comms` SDK module |

### Controllers & Input

- Unified `InputMap` with `GameAction` bindings (Jump, Dash, Shoot, Throttle, etc.)
- Web: Gamepad API via `useGamepad` hook; native: gilrs integration in the SDK
- Live controller remapping UI at `/settings/controller`
- Analog axes (throttle, brake, steer) for motorsport-style input

### Graphics Tiers

| Tier | Target | Runtime |
|------|--------|---------|
| `Lite2D` | Simple 2D arcade games | Canvas 2D / WebGL |
| `Standard3D` | General 3D games | WebGL2 / Vulkan |
| `Advanced3D` | FPS / motorsport / AAA | WebGPU / Vulkan, HDR, physics substeps |

### Points Economy

- Platform-wide XP / score ledger (append-only; atomic balance table)
- Season system with configurable resets
- Award / spend endpoints callable from games and the SDK (`platform::points`)
- Points leaderboard per game and global (Redis sorted sets; Postgres fallback)
- Point rewards catalog (earn + redeem definitions)

### Developer Marketplace

- Developers create in-game stores for their game (cosmetics, items, DLC, passes)
- Shared checkout: fiat USD via Paystack (70% developer / 30% platform) or points; developer payouts via Wise
- Entitlement tracking per user/item; purchase hardening (idempotency, content-rating age gate)
- Store management UI at `/developers/marketplace`; in-game store panel component

### Tournaments

- Create, join, and bracket management via `api/tournaments.rs`
- Seven REST endpoints: create, list, join, start, get, list-matches, submit-result
- Bracket generation (single-elimination skeleton); lifecycle state machine
- `prize_pool` stored; prize disbursement is a documented future item (see GAPS.md)

### Replay & Spectator

- `ReplayLog` recorded by `TickScheduler` on every tick; stored via `store_replay()` in `ws/game.rs`
- `verify_replay` re-simulates from scratch; any `state_hash` divergence is tamper evidence
- `magnetite-web-client` JS client supports in-browser replay playback via `replay-player.js`
- Spectator path: web client connects to live runtime session and receives `Delta` frames
- Replay retrieval REST endpoint (`GET /api/v1/matches/:id/replay`) is a documented open item

### Game Templates

| Template | Crate | Features |
|----------|-------|---------|
| `game-template/` | `magnetite-game-template` | 2D arcade; Bevy + `GameLogic`; WASM-ready |
| `game-template-fps/` | `magnetite-fps-starter` | Advanced 3D FPS; Bevy + rapier3d; hitscan; controller-ready |
| `game-template-motorsport/` | `magnetite-game-motorsport` | Vehicle physics; Bevy + rapier3d; analog throttle/brake/steer; lap → points |
| `game-template-authoritative/` | `magnetite-game-authoritative` | Top-down arena shooter; canonical `AuthoritativeGame` reference; WASM ABI exports |

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Platform backend | Rust — Axum 0.8, SQLx 0.8.6, Tokio |
| Database | PostgreSQL 16 (41 migration files) |
| Cache / state | Redis 7 |
| Real-time | WebSocket (Axum WS), WebRTC (signaling), RTMP egress (MediaMTX) |
| Game engine (client) | Bevy → WASM (wasm-bindgen) |
| Physics | rapier3d (FPS + motorsport templates) |
| Payments | Paystack fiat on-ramp (deposits + subscriptions); Wise (developer payouts); USD-denominated balances |
| Email | Resend / AWS SES (lettre); both real, fail-safe without credentials |
| Storage | AWS S3 (game artifacts, replays) |
| Observability | `tracing` + `tracing-subscriber` (structured request logging); `metrics-exporter-prometheus` (Cargo dep; `/metrics` endpoint currently returns DB pool stats only — full Prometheus tower layer is the CONSOLIDATE+OBSERVE target) |
| Frontend | React 19 + Vite, React Router 7, Recharts |
| i18n | `src/i18n/` — `I18nProvider`, `useTranslation`, EN/ES/FR JSON locales |
| PWA | `public/manifest.json` + `public/sw.js` (installable, offline shell) |
| Infrastructure | Fly.io (Firecracker VMs), Docker Compose (local + MediaMTX), Kubernetes (k8s manifests), Nomad (job files) |

---

## Project Structure

```
magnetite/
├── backend/                        # Rust platform backend
│   ├── src/
│   │   ├── api/                    # 38 HTTP route modules (Axum handlers)
│   │   │   ├── auth.rs             # Login, register, refresh, logout, sessions, me, 2FA, API keys
│   │   │   ├── communities.rs      # Communities (servers / guilds)
│   │   │   ├── channels.rs         # Channels (text + voice)
│   │   │   ├── messages.rs         # Channel messages + DM threads/messages
│   │   │   ├── points.rs           # Points balance, award, spend, history, leaderboard, season reset
│   │   │   ├── marketplace.rs      # Dev stores, items, purchases, entitlements
│   │   │   ├── distribution.rs     # Game artifact registration, play-manifest, build webhooks
│   │   │   ├── tournaments.rs      # Bracket management, lifecycle
│   │   │   ├── notifications.rs    # List, mark read, preferences; /ws/notifications handler
│   │   │   ├── replays.rs          # Replay record endpoints
│   │   │   ├── admin.rs            # User mgmt, game moderation, finance, review-reports, settings
│   │   │   └── …                   # games, wallet, developer, social, leaderboard, matchmaking, …
│   │   ├── services/               # 26 business-logic modules
│   │   │   ├── communities.rs      # Community + channel + message service
│   │   │   ├── presence.rs         # Presence upsert / sweep
│   │   │   ├── points.rs           # Atomic ledger, balance, season reset
│   │   │   ├── marketplace.rs      # Store CRUD, purchase, entitlement
│   │   │   ├── wise.rs             # Wise payout client (ACH + email; IBAN is a known gap)
│   │   │   └── …                   # auth, games, wallet, payment, payout, matchmaking, …
│   │   ├── middleware/             # CORS, rate limiting, request logging, JWT auth
│   │   ├── jobs/                   # Background jobs (sessions, notifications, backups, payouts)
│   │   ├── db/                     # Database pool
│   │   ├── ws/                     # WebSocket handlers
│   │   │   ├── comms.rs            # Real-time chat + presence broadcast
│   │   │   ├── voice.rs            # WebRTC SDP/ICE signaling relay
│   │   │   └── game.rs             # Game-state sync (ClientNet / ServerNet)
│   │   ├── config.rs               # App configuration
│   │   └── error.rs                # Unified error type
│   ├── magnetite-sdk/              # Rust SDK for game developers
│   │   └── src/
│   │       ├── authority.rs        # AuthoritativeGame, Topology, MatchConfig, ReplayLog, verify_replay, DeterministicRng
│   │       ├── game.rs             # GameLogic trait, GameMetadata
│   │       ├── graphics.rs         # GraphicsTier, RenderConfig (Lite2D / Standard3D / Advanced3D)
│   │       ├── input/              # Input, KeyCode, MouseState; gamepad/ (InputMap, GameAction)
│   │       ├── state.rs            # GameState, Snapshot, PlayerId, PlayerState
│   │       ├── protocol.rs         # Versioned wire protocol (Envelope, ClientMessage, ServerMessage)
│   │       ├── networking.rs       # ServerConfig, TickLoop, PredictionBuffer, InterestManager
│   │       └── platform/           # comms, points, marketplace, cloud_save
│   ├── migrations/                 # 41 SQL migration files
│   └── tests/                      # Integration tests (auth, API, wallet)
├── magnetite-runtime/              # Authoritative server host; magnetite-serve binary; Dockerfile
├── magnetite-sandbox/              # WasmExecutor (Wasmtime); fuel/memory/epoch limits
├── magnetite-anticheat/            # Validators, TrustScoreMap, ReplayVerifier
├── magnetite-cli/                  # magnetite new|build|dev|deploy binary
├── magnetite-web-client/           # JS web client (ClientNet/ServerNet); prediction; replay player
├── magnetite-e2e/                  # Integration + full-stack + scale bench tests
├── game-template/                  # Arcade 2D starter (Bevy + GameLogic + WASM-ready)
├── game-template-authoritative/    # Arena shooter reference (AuthoritativeGame + wasm ABI)
├── game-template-fps/              # FPS starter (Bevy + rapier3d; hitscan; gamepad; Advanced3D)
├── game-template-motorsport/       # Motorsport starter (Bevy + rapier3d vehicle; lap → points)
├── game-client-bevy/               # Bevy client (prediction/reconciliation wired to ServerNet)
├── src/                            # React frontend
│   ├── api/                        # API client (axios wrapper + all platform surfaces)
│   ├── i18n/                       # I18nProvider, useTranslation, en/es/fr JSON locales
│   ├── components/
│   │   ├── common/                 # Design-system primitives (Button, Input, Card, …)
│   │   ├── comms/                  # ServerRail, ChannelList, MessageList, MessageComposer,
│   │   │                           #   VoicePanel, MemberList, PresenceDot
│   │   ├── store/                  # InGameStore panel
│   │   ├── streaming/              # StreamCard, StreamPlayer, GoLivePanel
│   │   ├── NotificationPreferences.jsx  # 4-category × 3-channel a11y toggle grid
│   │   ├── GameOverlay.jsx         # In-game chat+voice overlay (hotkey toggle)
│   │   ├── landing/                # HeroSection, FeaturesSection, …
│   │   ├── auth/                   # AuthForm, OAuthButtons, PasswordInput, …
│   │   ├── charts/                 # Recharts wrappers
│   │   ├── skeletons/              # Loading skeletons
│   │   └── empty/                  # Empty-state illustrations
│   ├── context/                    # AuthContext, WalletContext, GameContext, ThemeContext,
│   │                               #   ToastContext, NotificationContext (WS to /ws/notifications),
│   │                               #   CommsContext, AnnouncementContext
│   ├── hooks/                      # 41 custom hooks; comms (useCommunities, useChannels,
│   │                               #   useMessages, usePresence, useVoice, useCommsSocket,
│   │                               #   useVoiceClient); useGamepad; usePoints; useMarketplace
│   ├── pages/                      # 97 page components across all platform areas
│   ├── data/                       # Mock fallback data (gated behind VITE_USE_MOCKS=true)
│   ├── utils/                      # Formatters, validation, feature flags, storage
│   └── styles/                     # tokens.css, animations, typography
├── e2e/                            # Playwright end-to-end tests
├── docs/                           # Platform documentation (see docs/index.md)
├── deploy/
│   ├── k8s/                        # 11 Kubernetes manifests (namespace → ingress + HPA)
│   └── nomad/                      # 6 Nomad job files (postgres, redis, mediamtx, backend, frontend, runtime)
├── config/
│   └── mediamtx.yml                # MediaMTX config (HLS + RTMP egress)
├── .github/workflows/              # CI (ci.yml), deploy (deploy.yml), game-ci, game-deploy, release
├── docker-compose.yml              # postgres + redis + mediamtx + backend + frontend
├── docker-compose.override.yml
├── Dockerfile.backend / Dockerfile.frontend / Dockerfile.fly
├── magnetite-runtime/Dockerfile    # Multi-stage; exposes port 9000
├── nginx.conf
└── fly.toml
```

---

## Getting Started

### Prerequisites

| Tool | Minimum version |
|------|----------------|
| Rust | 1.82 |
| Node.js | 18 |
| PostgreSQL | 16 |
| Redis | 7 |
| Docker + Compose | 24 |

### Option A — Docker Compose (recommended)

```bash
cp .env.example .env.docker   # edit credentials
docker compose up -d
```

Services: `postgres:5432`, `redis:6379`, `mediamtx:8888/1935`, `backend:8080`, `frontend:5173`.

### Option B — Local development

**Frontend:**

```bash
npm install
npm run dev         # http://localhost:5173
```

**Backend:**

```bash
cp .env.example backend/.env    # fill DATABASE_URL, JWT_SECRET, etc.
cd backend
cargo run                       # http://localhost:8080
```

**Run migrations:**

```bash
cd backend/tools
./migrate.sh up        # apply pending migrations
./migrate.sh status    # list applied / pending
./migrate.sh reset     # drop + re-create (dev only)
```

**Run the authoritative runtime standalone:**

```bash
# Smoke-test mode (no wasm required):
cargo run --package magnetite-runtime --bin serve

# With a compiled wasm artifact:
cargo run --package magnetite-runtime --bin serve -- --wasm path/to/game.wasm --port 9000
```

### Environment variables (minimum for local dev)

```
DATABASE_URL=postgres://postgres:postgres@localhost:5432/magnetite
REDIS_URL=redis://localhost:6379
JWT_SECRET=change-me
FRONTEND_URL=http://localhost:5173
```

Full list: see [`.env.example`](.env.example) and [`docs/self-hosting/environment-variables.md`](docs/self-hosting/environment-variables.md).

Payment/payout providers (absent → HTTP 502, never silent success):
- `PAYSTACK_SECRET_KEY` — fiat deposits + subscriptions
- `WISE_API_TOKEN` + `WISE_PROFILE_ID` — developer payouts (set `WISE_SANDBOX=true` for dev)
- `RESEND_API_KEY` or `AWS_SES_SMTP_USER/PASSWORD` — transactional email

---

## API Routes

The backend exposes all routes under `/api/v1`. Key modules:

| Module | Prefix | Notable endpoints |
|--------|--------|-------------------|
| Auth | `/api/v1/auth` | `POST /register`, `POST /login`, `GET /me`, `POST /logout`, `POST /refresh`, `POST /2fa/{setup,verify,disable}`, `GET/POST/DELETE /api-keys` |
| OAuth | `/api/v1/oauth/{provider}` | Google, Discord, GitHub, GitLab; `/callback` |
| Games | `/api/v1/games` | CRUD, search, categories, reviews, screenshots, versions, content-rating, helpful/report |
| Developer | `/api/v1/developer` | Dashboard stats, game management, build triggers, earnings, daily revenue chart |
| Marketplace | `/api/v1/games` (public) | Browse, filter, wishlist, ratings |
| In-game stores | `/api/v1/marketplace` | Dev stores, items, purchase, entitlements |
| Wallet | `/api/v1/wallet` | Balance, deposit (Paystack), withdraw, transaction history |
| Subscriptions | `/api/v1/subscriptions` | Tiers (Free / Basic / Pro / Unlimited), `GET /me`, subscribe, cancel |
| Matchmaking | `/api/v1/matchmaking` | Join queue, leave queue, status |
| Leaderboard | `/api/v1/leaderboard` | Global and per-game score boards |
| Achievements | `/api/v1/achievements` | Definitions, progress, unlock |
| Social | `/api/v1/social`, `/api/v1/friends` | Friends, invites, activity feed, block/unblock, pending requests |
| Notifications | `/api/v1/notifications` | List, mark read, preferences (`GET/PUT /preferences`) |
| Communities | `/api/v1/communities` | Community CRUD, membership, roles |
| Channels | `/api/v1/channels` | Channel CRUD within communities |
| Messages | `/api/v1/messages` | Channel messages + DM threads/messages |
| Points | `/api/v1/points` | Balance, award (admin), spend, history, leaderboard, season reset |
| Distribution | `/api/v1/distribution` | Play-manifest, artifact registration, build status |
| Profile | `/api/v1/profile` | View/edit, avatar, public stats |
| Platform settings | `/api/v1/platform` | `GET/PUT /settings` (admin) |
| Tournaments | `/api/v1/tournaments` | Create, list, join, start, bracket, match results |
| Replays | `/api/v1/replays` | Record endpoints (retrieval GET is a known open item) |
| Admin | `/api/v1/admin` | User mgmt, game moderation, finance, review-reports moderation, settings, refunds |
| Contact | `/api/v1/contact` | `POST /` persisted + optional email |
| Search | `/api/v1/search` | Full search across games, users, communities |
| Wishlist | `/api/v1/wishlist` | `GET/POST/DELETE /:game_id` |
| Health | `/api/v1/health` | Liveness + readiness |
| Webhooks | `/api/v1/webhooks` | GitHub push, payment events |
| WebSocket (comms) | `/ws/comms` | Real-time chat, presence, typing indicators |
| WebSocket (voice) | `/ws/voice` | WebRTC SDP/ICE signaling relay |
| WebSocket (game) | `/ws/game/{id}` | Real-time game state (`ClientNet`/`ServerNet`) |
| WebSocket (notifs) | `/ws/notifications` | Push notification delivery (JWT query-param auth) |

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                        MAGNETITE PLATFORM                            │
├──────────────────────────────────────────────────────────────────────┤
│  HTTP / WebSocket Gateway  (Rust / Axum 0.8)                         │
│  38 API modules · CORS · Rate limiting · JWT auth middleware         │
├──────────────────────────────────────────────────────────────────────┤
│  Business Services (26 modules)                                      │
│  Auth · Games · Wallet · Payment · Payout · Matchmaking              │
│  Leaderboard · Achievements · Social · Anti-cheat · Email            │
│  Analytics · Cache · Friends · Invites · Verification                │
│  Communities · Presence · Points · Marketplace · Distribution        │
│  Wise (payouts) · Streaming · Health · Provisioning                  │
├──────────────────────────────────────────────────────────────────────┤
│  WebSocket Handlers                                                  │
│  ws/comms.rs  (chat + presence)  ·  ws/voice.rs  (WebRTC signaling) │
│  ws/game.rs   (ClientNet/ServerNet) · api/notifications.rs (/ws/notifs) │
├──────────────────────────────────────────────────────────────────────┤
│  Persistence                                                         │
│  PostgreSQL 16 (41 migrations) · Redis 7 (sessions, cache, queues)  │
│  AWS S3 (game builds, replays, assets)                               │
├──────────────────────────────────────────────────────────────────────┤
│  Game Instances (Developer Code)                                     │
│  Client: Rust + Bevy → WASM (browser) or native binary              │
│  Server: magnetite-runtime (NativeExecutor or WasmExecutor)          │
│           sandboxed — fuel/memory/epoch limits via Wasmtime          │
│  JS web client: magnetite-web-client (ClientNet adapter, prediction) │
├──────────────────────────────────────────────────────────────────────┤
│  magnetite-sdk (MIT)                                                 │
│  AuthoritativeGame · Topology · MatchConfig · ReplayLog              │
│  GameLogic trait · Input / State · Networking protocol               │
│  platform: comms · points · marketplace · cloud_save                │
│  graphics: Lite2D / Standard3D / Advanced3D tiers                   │
│  input: keyboard + mouse + gamepad (unified InputMap)                │
└──────────────────────────────────────────────────────────────────────┘
```

---

## SDK Quick-Start

```rust
use magnetite_sdk::{
    export_game,
    game::{GameLogic, GameMetadata},
    input::{Action, Input},
    state::{GameState, PlayerId, Snapshot},
};

struct MyGame { state: GameState }

impl GameLogic for MyGame {
    fn new() -> Self { MyGame { state: GameState::default() } }
    fn handle_input(&mut self, _pid: PlayerId, _input: Input) -> Action { Action::None }
    fn tick(&mut self) { self.state.tick += 1; }
    fn state(&self) -> &GameState { &self.state }
    fn players(&self) -> Vec<PlayerId> { vec![] }
    fn metadata(&self) -> GameMetadata { GameMetadata::default() }
    fn snapshot(&self) -> Snapshot { Snapshot::new(self.state.tick, self.state.clone()) }
    fn restore(&mut self, snap: Snapshot) { self.state = snap.state; }
}

export_game!(MyGame);
```

For the server-authoritative path:

```rust
use magnetite_sdk::authority::{AuthoritativeGame, Topology, MatchConfig};

// Implement AuthoritativeGame, then:
let cfg = MatchConfig::auto(player_count);  // SingleRoom / Dedicated / Sharded
```

See [`backend/magnetite-sdk/`](backend/magnetite-sdk/) and [`game-template/`](game-template/) for the full starter.

Use `game-template-fps/` for a ready-made advanced FPS with Bevy + rapier3d + controller support.
Use `game-template-motorsport/` for vehicle physics, lap timing, and analog gamepad input.
Use `game-template-authoritative/` for the canonical `AuthoritativeGame` reference implementation.

---

## Observability

Current state:
- **Structured logging**: `tracing` + `tracing-subscriber` (env-filter); request ID middleware.
- **`/metrics` endpoint**: returns DB pool size + idle connections (`backend/src/api/metrics.rs`).
- **Target (CONSOLIDATE+OBSERVE wave)**: full Prometheus tower layer — request count, latency histogram, error rate, active-WS gauges, game-session gauges, DB pool; request-ID correlation across logs.

---

## Documentation

| Guide | File |
|-------|------|
| Developer Quickstart | [`docs/for-developers/quickstart.md`](docs/for-developers/quickstart.md) |
| SDK Reference | [`docs/for-developers/sdk.md`](docs/for-developers/sdk.md) |
| Controllers & Gamepad Input | [`docs/for-developers/controllers.md`](docs/for-developers/controllers.md) |
| Graphics Tiers | [`docs/for-developers/graphics-tiers.md`](docs/for-developers/graphics-tiers.md) |
| Points & Score Economy | [`docs/for-developers/points-economy.md`](docs/for-developers/points-economy.md) |
| Dev Marketplace | [`docs/for-developers/marketplace.md`](docs/for-developers/marketplace.md) |
| FPS Starter Template | [`docs/for-developers/fps-starter.md`](docs/for-developers/fps-starter.md) |
| Motorsport Starter Template | [`docs/for-developers/motorsport-starter.md`](docs/for-developers/motorsport-starter.md) |
| Build & Distribution Pipeline | [`docs/for-developers/build-pipeline.md`](docs/for-developers/build-pipeline.md) |
| MOAT Architecture | [`docs/MOAT-ARCHITECTURE.md`](docs/MOAT-ARCHITECTURE.md) |
| MOAT Scaling (topology + bench) | [`docs/moat/scaling.md`](docs/moat/scaling.md) |
| Replay & Spectator | [`docs/moat/replay-spectator.md`](docs/moat/replay-spectator.md) |
| Architecture Overview | [`docs/architecture.md`](docs/architecture.md) |
| Comms Overview | [`docs/comms/index.md`](docs/comms/index.md) |
| Comms Realtime Protocol | [`docs/comms/realtime.md`](docs/comms/realtime.md) |
| Comms Data Model | [`docs/comms/data-model.md`](docs/comms/data-model.md) |
| In-Game Chat and Voice | [`docs/comms/in-game.md`](docs/comms/in-game.md) |
| Streaming (go live / watch) | [`docs/comms/streaming.md`](docs/comms/streaming.md) |
| Economy & Marketplace | [`docs/economy-marketplace.md`](docs/economy-marketplace.md) |
| REST API Reference | [`docs/api-reference/index.md`](docs/api-reference/index.md) |
| Self-Hosting Guide | [`docs/self-hosting/index.md`](docs/self-hosting/index.md) |
| Environment Variables | [`docs/self-hosting/environment-variables.md`](docs/self-hosting/environment-variables.md) |
| External Dependencies | [`docs/self-hosting/external-dependencies.md`](docs/self-hosting/external-dependencies.md) |
| Streaming Infrastructure | [`docs/self-hosting/streaming.md`](docs/self-hosting/streaming.md) |
| Security & Sandboxing | [`docs/security/index.md`](docs/security/index.md) |
| i18n | [`docs/i18n.md`](docs/i18n.md) |
| Subscriptions Lifecycle | [`docs/subscriptions-lifecycle.md`](docs/subscriptions-lifecycle.md) |
| Moderation | [`docs/moderation.md`](docs/moderation.md) |
| Analytics | [`docs/analytics.md`](docs/analytics.md) |

---

## License

MIT — see [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

*Built with Rust. Powered by open source.*
