# Magnetite

**Magnetite (Fe₃O₄)** — Iron oxide, magnetic, grounded. The foundation upon which things are built.

*Build, distribute, and monetize Rust games — from a weekend jam to a COD-scale AAA title.*

---

## Vision

Magnetite is the open-source **unified gaming suite** for Rust game development at any scale. Game logic is authored in Rust; clients compile Bevy → WASM for the browser and to native binaries. The platform is server-authoritative and sandboxed, providing the heavy lifting — hosting, matchmaking, real-time netcode, persistence, comms, economy, and payment rails — so developers only write game logic.

- **Rust-first.** Not HTML5. Not Unity. Bevy → WASM (browser) + native. Servers are sandboxed Rust.
- **Scales with the game.** A tiny 2D arcade game and a large FPS or motorsport title share the same SDK and platform.
- **Distribution built in.** Storefront/marketplace: players discover, play (in-browser via WASM or native), and pay.
- **Communities & comms.** Discord-class servers, channels, real-time text chat, voice (WebRTC), presence, and streaming — all plugged into every game automatically.
- **Points economy.** Platform-wide XP/score system with seasons, ledger, leaderboard, and rewards.
- **Dev-run marketplaces.** Developers create in-game stores (cosmetics, items, DLC, passes) with a shared checkout and 70/30 revenue split.
- **Controllers.** First-class gamepad support (Gamepad API + gilrs) with a unified binding layer.
- **Graphics tiers.** `Lite2D` → `Standard3D` → `Advanced3D` — simple games stay lightweight, AAA games scale up.
- **Open source.** Platform MIT, SDK MIT, game templates MIT.
- **Real money, no middlemen.** USDC payments (Circle), Paystack fiat on-ramp, 15% platform fee, playtime-based developer payouts.

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
| Streaming | Go live with `getDisplayMedia` + RTMP egress to Twitch/YouTube; in-platform HLS/WebRTC watch |
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
- Points leaderboard per game and global
- Point rewards catalog (earn + redeem definitions)

### Developer Marketplace

- Developers create in-game stores for their game (cosmetics, items, DLC, passes)
- Shared checkout: USDC (70 % developer / 30 % platform) or points
- Entitlement tracking per user/item
- Store management UI at `/developers/marketplace`; in-game store panel component

### Game Templates

| Template | Crate | Features |
|----------|-------|---------|
| `game-template/` | `magnetite-game-template` | 2D arcade; Bevy + `GameLogic`; WASM-ready |
| `game-template-fps/` | `magnetite-fps-starter` | Advanced 3D FPS; Bevy + rapier3d; hitscan; controller-ready |
| `game-template-motorsport/` | `magnetite-game-motorsport` | Vehicle physics; Bevy + rapier3d; analog throttle/brake/steer; lap → points |

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Platform backend | Rust — Axum 0.8, SQLx 0.8.6, Tokio |
| Database | PostgreSQL 16 |
| Cache / state | Redis 7 |
| Real-time | WebSocket (Axum WS), WebRTC (signaling), RTMP egress |
| Game engine (client) | Bevy → WASM (wasm-bindgen) |
| Physics | rapier3d (FPS + motorsport templates) |
| Payments | USDC via Circle; Paystack fiat on-ramp |
| Email | Resend / AWS SES (lettre) |
| Storage | AWS S3 (game artifacts, replays) |
| Frontend | React 19 + Vite, React Router 7, Recharts |
| Infrastructure | Fly.io (Firecracker VMs), Docker Compose (local) |

---

## Project Structure

```
magnetite/
├── backend/                   # Rust platform backend
│   ├── src/
│   │   ├── api/               # 34 HTTP route modules (Axum handlers)
│   │   │   ├── auth.rs        # Login, register, refresh, logout, sessions, me
│   │   │   ├── communities.rs # Communities (servers / guilds)
│   │   │   ├── channels.rs    # Channels (text + voice)
│   │   │   ├── messages.rs    # Channel messages + DM threads/messages
│   │   │   ├── points.rs      # Points balance, award, spend, history, leaderboard
│   │   │   ├── marketplace.rs # Dev stores, items, purchases, entitlements
│   │   │   ├── distribution.rs# Game artifact registration, play-manifest, build webhooks
│   │   │   └── …              # games, wallet, developer, admin, leaderboard, matchmaking, …
│   │   ├── services/          # 22 business-logic modules
│   │   │   ├── communities.rs # Community + channel + message service
│   │   │   ├── presence.rs    # Presence upsert / sweep
│   │   │   ├── points.rs      # Atomic ledger, balance, season reset
│   │   │   ├── marketplace.rs # Store CRUD, purchase, entitlement
│   │   │   └── …              # auth, games, wallet, payment, payout, matchmaking, …
│   │   ├── middleware/        # CORS, rate limiting, request logging, JWT auth
│   │   ├── jobs/              # Background jobs (sessions, notifications, backups)
│   │   ├── db/                # Database pool
│   │   ├── ws/                # WebSocket handlers
│   │   │   ├── comms.rs       # Real-time chat + presence broadcast
│   │   │   ├── voice.rs       # WebRTC SDP/ICE signaling relay
│   │   │   └── game.rs        # Game-state sync
│   │   ├── config.rs          # App configuration
│   │   └── error.rs           # Unified error type
│   ├── magnetite-sdk/         # Rust SDK for game developers
│   │   └── src/
│   │       ├── game.rs        # GameLogic trait, GameMetadata
│   │       ├── graphics.rs    # GraphicsTier, RenderConfig (Lite2D / Standard3D / Advanced3D)
│   │       ├── input/         # Input, KeyCode, MouseState; gamepad/ (InputMap, GameAction)
│   │       ├── state.rs       # GameState, Snapshot, PlayerId, PlayerState
│   │       ├── protocol.rs    # Versioned wire protocol (Envelope, ClientMessage, ServerMessage)
│   │       ├── networking.rs  # ServerConfig, TickLoop, PredictionBuffer, InterestManager
│   │       └── platform/      # comms, points, marketplace, cloud_save
│   ├── migrations/            # 24 SQL migration files
│   └── tests/                 # Integration tests (auth, API, wallet)
├── game-template/             # Arcade 2D starter (Bevy + GameLogic + WASM-ready)
├── game-template-fps/         # FPS starter (Bevy + rapier3d; hitscan; gamepad; Advanced3D)
├── game-template-motorsport/  # Motorsport starter (Bevy + rapier3d vehicle; lap → points)
├── src/                       # React frontend
│   ├── api/                   # API client (axios wrapper + comms/points/marketplace surfaces)
│   ├── components/
│   │   ├── common/            # Design-system primitives (Button, Input, Card, …)
│   │   ├── comms/             # ServerRail, ChannelList, MessageList, MessageComposer,
│   │   │                      #   VoicePanel, MemberList, PresenceDot
│   │   ├── store/             # InGameStore panel
│   │   ├── streaming/         # StreamCard, StreamPlayer, GoLivePanel
│   │   ├── GameOverlay.jsx    # In-game chat+voice overlay (hotkey toggle)
│   │   ├── landing/           # HeroSection, FeaturesSection, …
│   │   ├── auth/              # AuthForm, OAuthButtons, PasswordInput, …
│   │   ├── charts/            # Recharts wrappers
│   │   ├── skeletons/         # Loading skeletons
│   │   └── empty/             # Empty-state illustrations
│   ├── context/               # AuthContext, WalletContext, GameContext, ThemeContext,
│   │                          #   ToastContext, NotificationContext, CommsContext
│   ├── hooks/                 # 42+ custom hooks; comms (useCommunities, useChannels,
│   │                          #   useMessages, usePresence, useVoice, useCommsSocket,
│   │                          #   useVoiceClient); useGamepad; usePoints; useMarketplace
│   ├── pages/                 # 75+ page components
│   │   ├── Communities.jsx    # Discord-like server/channel/chat/voice UI
│   │   ├── Messages.jsx       # Direct Messages (threads + conversation)
│   │   ├── Streams.jsx        # Browse live streams
│   │   ├── Points.jsx         # Player points dashboard
│   │   ├── DevMarketplace.jsx # Developer store management
│   │   ├── ControllerSettings.jsx # Live gamepad + key binding editor
│   │   └── …                  # Marketplace, GameDetail, Wallet, Matchmaking, …
│   ├── data/                  # Mock fallback data
│   ├── utils/                 # Formatters, validation, feature flags, storage
│   └── styles/                # tokens.css, animations, typography
├── e2e/                       # Playwright end-to-end tests
├── docs/                      # Platform documentation
├── .github/workflows/         # CI, deploy, game-ci, release
├── docker-compose.yml
├── Dockerfile.backend / Dockerfile.frontend / Dockerfile.fly
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
| OAuth | `/api/oauth/{provider}` | Google, Discord, GitHub, GitLab; `/callback` |
| Games | `/api/games` | CRUD, search, categories, reviews, screenshots, versions |
| Developer | `/api/developer` | Dashboard stats, game management, build triggers, earnings |
| Marketplace | `/api/games` (public) | Browse, filter, wishlist, ratings |
| Wallet | `/api/wallet` | Balance, deposit (USDC/Paystack), withdraw, transaction history |
| Subscriptions | `/api/subscriptions` | Tiers (Free / Basic / Pro / Unlimited), activation |
| Matchmaking | `/api/matchmaking` | Join queue, leave queue, status |
| Leaderboard | `/api/leaderboard` | Global and per-game score boards |
| Achievements | `/api/achievements` | Definitions, progress, unlock |
| Social | `/api/social` | Friends, invites, activity feed |
| Notifications | `/api/notifications` | List, mark read, preferences |
| Communities | `/api/communities` | Community CRUD, membership, roles |
| Channels | `/api/channels` | Channel CRUD within communities |
| Messages | `/api/messages` | Channel messages + DM threads/messages |
| Points | `/api/points` | Balance, award, spend, history, leaderboard, season reset |
| Marketplace (in-game) | `/api/marketplace` | Dev stores, items, purchase, entitlements |
| Distribution | `/api/distribution` | Play-manifest, artifact registration, build status |
| Profile | `/api/profile` | View/edit, avatar, public stats |
| Tournaments | `/api/tournaments` | Create, join, bracket |
| Admin | `/api/admin` | User management, game moderation, finance, settings |
| Health | `/api/health` | Liveness + readiness |
| Webhooks | `/api/webhooks` | GitHub push, payment events |
| WebSocket (comms) | `/ws/comms` | Real-time chat, presence, typing indicators |
| WebSocket (voice) | `/ws/voice` | WebRTC SDP/ICE signaling relay |
| WebSocket (game) | `/ws/game/{id}` | Real-time game state, input |

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                        MAGNETITE PLATFORM                            │
├──────────────────────────────────────────────────────────────────────┤
│  HTTP / WebSocket Gateway  (Rust / Axum 0.8)                         │
│  34 API modules · CORS · Rate limiting · JWT auth middleware         │
├──────────────────────────────────────────────────────────────────────┤
│  Business Services (22 modules)                                      │
│  Auth · Games · Wallet · Payment · Payout · Matchmaking              │
│  Leaderboard · Achievements · Social · Anti-cheat · Email            │
│  Analytics · Cache · Friends · Invites · Verification                │
│  Communities · Presence · Points · Marketplace · Distribution        │
├──────────────────────────────────────────────────────────────────────┤
│  WebSocket Handlers                                                  │
│  ws/comms.rs  (chat + presence)  ·  ws/voice.rs  (WebRTC signaling) │
│  ws/game.rs   (game state sync)                                      │
├──────────────────────────────────────────────────────────────────────┤
│  Persistence                                                         │
│  PostgreSQL 16 (24 migrations) · Redis 7 (sessions, cache, queues)  │
│  AWS S3 (game builds, replays, assets)                               │
├──────────────────────────────────────────────────────────────────────┤
│  Game Instances (Developer Code)                                     │
│  Client: Rust + Bevy → WASM (browser) or native binary              │
│  Server: Rust, server-authoritative, sandboxed (planned gVisor)      │
├──────────────────────────────────────────────────────────────────────┤
│  magnetite-sdk (MIT)                                                 │
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

See [`backend/magnetite-sdk/`](backend/magnetite-sdk/) and
[`game-template/`](game-template/) for the full starter.

Use `game-template-fps/` for a ready-made advanced FPS with Bevy + rapier3d + controller support.
Use `game-template-motorsport/` for vehicle physics, lap timing, and analog gamepad input.

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
| Security & Sandboxing | [`docs/security/index.md`](docs/security/index.md) |

---

## License

MIT — see [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

*Built with Rust. Powered by open source.*
