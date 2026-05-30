# Magnetite

**Magnetite (Fe₃O₄)** — Iron oxide, magnetic, grounded. The foundation upon which things are built.

*Build, distribute, and monetize Rust games — from a weekend jam to a COD-scale AAA title.*

---

## Vision

Magnetite is the open-source platform for Rust game development at any scale. Game logic is authored in Rust; clients compile Bevy → WASM for the browser and to native binaries. The platform is server-authoritative and sandboxed, providing hosting, matchmaking, real-time netcode, persistence, and payment rails — so developers only write game logic.

- **Rust-first.** Not HTML5. Not Unity. Bevy → WASM (browser) + native. Servers are sandboxed Rust.
- **Scales with the game.** A tiny arcade game and a large multiplayer title share the same SDK and platform.
- **Distribution built in.** Storefront/marketplace: players discover, play (in-browser via WASM or native), and pay.
- **Open source.** Platform MIT, SDK MIT, game template MIT.
- **Real money, no middlemen.** USDC payments (Circle), Paystack fiat on-ramp, 15% platform fee, playtime-based developer payouts.

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Platform backend | Rust — Axum 0.7, SQLx 0.7, Tokio |
| Database | PostgreSQL 16 |
| Cache / state | Redis 7 |
| Real-time | WebSocket (Axum WS), QUIC (planned) |
| Game engine (client) | Bevy → WASM (wasm-bindgen) |
| Payments | USDC via Circle; Paystack fiat on-ramp |
| Email | Resend / AWS SES (lettre) |
| Storage | AWS S3 (game artifacts, replays) |
| Frontend | React 19 + Vite, React Router 7, Recharts |
| Infrastructure | Fly.io (Firecracker VMs), Docker Compose (local) |

---

## Project Structure

```
magnetite/
├── backend/                  # Rust platform backend
│   ├── src/
│   │   ├── api/              # 27 HTTP route modules (Axum handlers)
│   │   ├── services/         # 18 business-logic modules
│   │   ├── middleware/       # CORS, rate limiting, request logging
│   │   ├── jobs/             # Background jobs (sessions, notifications, backups)
│   │   ├── db/               # Database pool
│   │   ├── ws/               # WebSocket game handler
│   │   ├── config.rs         # App configuration
│   │   └── error.rs          # Unified error type
│   ├── magnetite-sdk/        # Rust SDK for game developers
│   │   └── src/              # GameLogic trait, Input, State, Networking types
│   ├── migrations/           # 20 SQL migration files
│   ├── tests/                # Integration tests (auth, API, wallet)
│   └── tools/                # migrate.sh, backup.sh
├── game-template/            # Bevy + magnetite-sdk starter (WASM-ready)
│   └── src/lib.rs
├── src/                      # React frontend
│   ├── api/                  # API client (axios wrapper)
│   ├── components/           # 118 UI components
│   │   ├── common/           # Design-system primitives (Button, Input, Card, …)
│   │   ├── landing/          # HeroSection, FeaturesSection, Testimonials, …
│   │   ├── auth/             # AuthForm, OAuthButtons, PasswordInput, …
│   │   ├── admin/            # AdminRoute, AdminSidebar
│   │   ├── charts/           # Recharts wrappers (Line, Bar, Area, Pie, …)
│   │   ├── skeletons/        # Loading skeletons per entity type
│   │   └── empty/            # Empty-state illustrations
│   ├── context/              # AuthContext, WalletContext, GameContext,
│   │                         # ThemeContext, ToastContext, NotificationContext
│   ├── hooks/                # 30+ custom hooks (useAuth, useGames, useWallet, …)
│   ├── pages/                # 59 page components + admin/ + developers/ subdirs
│   ├── data/                 # Mock fallback data (used when API is unavailable)
│   ├── utils/                # Formatters, validation, feature flags, storage
│   └── styles/               # Supplemental CSS (animations, typography, layout)
├── e2e/                      # Playwright end-to-end tests
├── docs/                     # Platform documentation (getting-started, API ref, …)
├── .github/workflows/        # CI (ci.yml), deploy (deploy.yml), game-ci, release
├── docker-compose.yml        # Local dev: postgres + redis + backend + frontend
├── Dockerfile.backend
├── Dockerfile.frontend
├── nginx.conf
└── fly.toml
```

---

## Getting Started

### Prerequisites

| Tool | Minimum version |
|------|----------------|
| Rust | 1.75 |
| Node.js | 18 |
| PostgreSQL | 15 |
| Redis | 7 |

### Option A — Docker Compose (recommended)

```bash
cp .env.example .env.docker   # edit credentials
docker compose up -d
```

Services: `postgres:5432`, `redis:6379`, `backend:8080`, `frontend:5173`.

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

### Environment variables (minimum for local dev)

```
DATABASE_URL=postgres://postgres:postgres@localhost:5432/magnetite
REDIS_URL=redis://localhost:6379
JWT_SECRET=change-me
FRONTEND_URL=http://localhost:5173
```

Full list: see [`.env.example`](.env.example).

---

## API Routes

The backend exposes all routes under `/api`. Key modules:

| Module | Prefix | Notable endpoints |
|--------|--------|-------------------|
| Auth | `/api/auth` | `POST /register`, `POST /login`, `GET /me`, `POST /logout`, `POST /refresh` |
| OAuth | `/api/auth/{provider}` | Google, Discord, GitHub, GitLab; `/callback` |
| Games | `/api/games` | CRUD, search, categories, reviews, screenshots, versions |
| Developer | `/api/developer` | Dashboard stats, game management, build triggers |
| Marketplace | `/api/games` (public) | Browse, filter, wishlist, ratings |
| Wallet | `/api/wallet` | Balance, deposit (USDC/Paystack), withdraw, transaction history |
| Subscriptions | `/api/subscriptions` | Tiers (Free / Basic / Pro / Unlimited), activation |
| Matchmaking | `/api/matchmaking` | Join queue, leave queue, status |
| Leaderboard | `/api/leaderboard` | Global and per-game score boards |
| Achievements | `/api/achievements` | Definitions, progress, unlock |
| Social | `/api/social` | Friends, invites, activity feed |
| Notifications | `/api/notifications` | List, mark read, preferences |
| Comms | `/api/comms` | Communities, channels, messages, voice rooms, streams, presence |
| Profile | `/api/profile` | View/edit, avatar, public stats |
| Tournaments | `/api/tournaments` | Create, join, bracket |
| Admin | `/api/admin` | User management, game moderation, finance, settings |
| Health | `/api/health` | Liveness + readiness |
| Webhooks | `/api/webhooks` | GitHub push, payment events |
| WebSocket | `/ws/` | Real-time game state, chat, presence, voice signaling |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                       MAGNETITE PLATFORM                        │
├─────────────────────────────────────────────────────────────────┤
│  HTTP/WebSocket Gateway  (Rust / Axum)                          │
│  27 API modules · CORS · Rate limiting · JWT auth middleware    │
├─────────────────────────────────────────────────────────────────┤
│  Business Services (18 modules)                                 │
│  Auth · Games · Wallet · Payment · Payout · Matchmaking         │
│  Leaderboard · Achievements · Social · Anti-cheat · Email       │
│  Analytics · Cache · Friends · Invites · Verification           │
├─────────────────────────────────────────────────────────────────┤
│  Persistence                                                    │
│  PostgreSQL (20 migrations) · Redis (sessions, cache, queues)   │
│  AWS S3 (game builds, replays, assets)                          │
├─────────────────────────────────────────────────────────────────┤
│  Game Instances (Developer Code)                                │
│  Client: Rust + Bevy → WASM (browser) or native binary         │
│  Server: Rust, server-authoritative, sandboxed (planned gVisor) │
├─────────────────────────────────────────────────────────────────┤
│  magnetite-sdk (MIT)                                            │
│  GameLogic trait · Input/State types · Networking protocol      │
└─────────────────────────────────────────────────────────────────┘
```

---

## SDK Quick-Start

```rust
use magnetite_sdk::*;

struct MyGame { players: HashMap<PlayerId, PlayerState> }

impl GameLogic for MyGame {
    fn new() -> Self { ... }
    fn handle_input(&mut self, player: PlayerId, input: Input) { ... }
    fn tick(&mut self) { ... }
    fn state(&self) -> GameState { ... }
    fn players(&self) -> Vec<PlayerId> { ... }
}

// wasm-bindgen entry point generated by game-template
```

See [`backend/magnetite-sdk/`](backend/magnetite-sdk/) and
[`game-template/`](game-template/) for the full starter.

---

## Documentation

| Guide | File |
|-------|------|
| Developer Quickstart (clone → implement → build WASM → publish) | [`docs/for-developers/quickstart.md`](docs/for-developers/quickstart.md) |
| SDK Reference (`GameLogic`, `Input`, `GameState`, `Snapshot`) | [`docs/for-developers/sdk.md`](docs/for-developers/sdk.md) |
| Build & Distribution Pipeline | [`docs/for-developers/build-pipeline.md`](docs/for-developers/build-pipeline.md) |
| Architecture Overview (backend modules, services, data flow) | [`docs/architecture.md`](docs/architecture.md) |
| Comms Suite Overview (communities, chat, presence, voice, streaming) | [`docs/comms/index.md`](docs/comms/index.md) |
| Comms Realtime Protocol (WS chat/presence + WebRTC signaling) | [`docs/comms/realtime.md`](docs/comms/realtime.md) |
| Comms Data Model (communities/channels/messages/voice_rooms/streams) | [`docs/comms/data-model.md`](docs/comms/data-model.md) |
| In-Game Chat and Voice (`platform::comms` SDK usage) | [`docs/comms/in-game.md`](docs/comms/in-game.md) |
| REST API Reference (all real endpoints) | [`docs/api-reference/index.md`](docs/api-reference/index.md) |
| Self-Hosting Guide (Docker Compose + Fly.io) | [`docs/self-hosting/index.md`](docs/self-hosting/index.md) |
| Environment Variables | [`docs/self-hosting/environment-variables.md`](docs/self-hosting/environment-variables.md) |
| Security & Sandboxing | [`docs/security/index.md`](docs/security/index.md) |

---

## License

MIT — see [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

*Built with Rust. Powered by open source.*
