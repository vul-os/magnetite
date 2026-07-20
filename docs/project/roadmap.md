# Magnetite — Platform Roadmap

> **[2026-07-20] STALE — historical record.**
> Moved here from the repository root. This roadmap is from the centralized era
> and its phases no longer map to how Magnetite is built. Superseded by
> [`DECENTRALIZATION.md`](../../DECENTRALIZATION.md) and
> [DECENTRALIZATION_PROGRESS.md](./DECENTRALIZATION_PROGRESS.md).

---

**Magnetite (Fe₃O₄)** — Iron oxide, magnetic, grounded.

*Build, distribute, and monetize Rust games — from a weekend jam to a COD-scale AAA title.*

---

## Vision

Magnetite is the open-source **unified gaming suite** for Rust games at any scale. Game logic is authored in Rust; clients compile Bevy → WASM for browsers and to native binaries. The platform provides the heavy infrastructure — hosting, matchmaking, real-time netcode, Discord-class comms, points economy, developer marketplaces, persistence, distribution, and payments — so developers write only game logic.

**The "HTML5 games" framing is retired.** Magnetite is a Rust-native platform for games at any scale, from a simple 2D arcade jam to an advanced FPS or motorsport title.

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
│  PostgreSQL 16 (24 migrations) · Redis 7 (sessions/cache/queues)    │
│  AWS S3 (game builds, replays, assets)                               │
├──────────────────────────────────────────────────────────────────────┤
│  Game Instances (Developer Code)                                     │
│  Client: Rust + Bevy → WASM (browser) or native binary              │
│  Server: Rust, server-authoritative, sandboxed (gVisor planned)      │
├──────────────────────────────────────────────────────────────────────┤
│  magnetite-sdk (MIT)                                                 │
│  GameLogic trait · Input/State · Networking protocol                 │
│  platform: comms · points · marketplace · cloud_save                │
│  graphics: Lite2D / Standard3D / Advanced3D tiers                   │
│  input: keyboard + mouse + gamepad (unified InputMap)                │
└──────────────────────────────────────────────────────────────────────┘
```

### Payment & distribution model

| Tier | Price | Access |
|------|-------|--------|
| Free | $0/mo | Free games only |
| Basic | $4.99/mo | 10 h/month |
| Pro | $9.99/mo | 50 h/month |
| Unlimited | $19.99/mo | Unlimited hours |

Platform takes a 15% fee; developers receive payouts proportional to playtime.
USDC (Circle) for on-chain payments; Paystack for fiat on-ramp.
In-game store purchases split 70 % developer / 30 % platform.

### Developer workflow (SDK → platform)

1. Clone `game-template` (Bevy + magnetite-sdk) — or use `game-template-fps` / `game-template-motorsport` for advanced games
2. Implement `GameLogic` trait
3. Push to GitHub
4. Register repo on Magnetite developer dashboard
5. Platform CI: `cargo build --target wasm32-unknown-unknown`, security scan, sandboxed smoke test, manual review, live

---

## Phase 1 — Foundation (COMPLETE)

The backend API, database schema, and core frontend scaffold are built and compile.

### Backend
- [x] Axum HTTP server with config, unified error type, structured logging
- [x] 27 initial API modules: auth, oauth, games, developer, wallet, subscriptions, matchmaking, leaderboard, achievements, social, notifications, profile, tournaments, admin, categories, reviews, sessions, search, platform, metrics, versioning, webhooks, github, health, middleware, response helpers, wishlist
- [x] 18 initial service modules: auth, games, wallet, payment, payout, matchmaking, leaderboard, achievements, analytics, anticheat, cache, email, friends, health, invites, session, verification
- [x] Middleware: CORS (tower-http), rate limiting, request logging
- [x] Background jobs: session cleanup, notification cleanup, database backup
- [x] WebSocket game handler (`/ws/game/{id}`)
- [x] PostgreSQL connection pool (SQLx 0.8.6)
- [x] Redis integration (sessions, cache, pub/sub)
- [x] AWS S3 integration (game artifacts)
- [x] Email sending (lettre; Resend + AWS SES)
- [x] JWT auth (jsonwebtoken) + Argon2 password hashing
- [x] 22 SQL migrations (users, games, sessions, wallets, scores, achievements, social, tournaments, notifications, subscriptions, anti-cheat, …)
- [x] Integration tests: auth, API, wallet (backend/tests/)

### Frontend
- [x] React 19 + Vite + React Router 7 scaffold
- [x] 67+ top-level page components + admin/ (5) + developers/ (3) subdirs
- [x] 118 component files across common/, landing/, auth/, admin/, charts/, skeletons/, empty/
- [x] Common design-system primitives: Button, Input, Card, Badge, Modal, Table, Tabs, Pagination, Progress, Checkbox, Radio, Select, Switch, Tooltip, Spinner, Avatar, Breadcrumb, ConfirmDialog
- [x] Contexts: AuthContext, WalletContext, GameContext, ThemeContext, ToastContext, NotificationContext, AnnouncementContext
- [x] 30+ hooks: useAuth, useGames, useWallet, useMatchmaking, useLeaderboard, useNotifications, useWebSocket, useGameSession, useGameLobby, useSearch, usePagination, useInfiniteScroll, useAnimation, useCountUp, useKeyboardShortcuts, useFeatureFlag, useToast, useTour, useTypingEffect, useParallax, useMediaQuery, useWindowSize, useIsMobile, useDebounce, useClickOutside, useFocusTrap, useMemoOne, useUser, useTheme, useAnnouncement, useIntersectionObserver
- [x] Mock data (graceful fallback): games, achievements, friends, leaderboard, notifications, profile, user

### Infrastructure
- [x] Docker: Dockerfile.backend, Dockerfile.frontend, Dockerfile.fly
- [x] docker-compose.yml (postgres, redis, backend, frontend, nginx)
- [x] nginx.conf (reverse proxy + static serving)
- [x] fly.toml (Fly.io deployment)
- [x] GitHub Actions: ci.yml, deploy.yml, game-ci.yml, game-deploy.yml, release.yml
- [x] backend/tools: migrate.sh, backup.sh

### SDK & game template
- [x] `magnetite-sdk` Rust crate: `GameLogic` trait, `Input`/`Action`/`State` types, `PlayerId`/`PlayerState`/`Position`/`Rotation`, `NetworkManager` / `StateSyncProtocol`
- [x] `game-template`: Bevy plugin implementing `GameLogic`, wasm-bindgen entry point, input / tick systems, WASM build script

### Docs
- [x] docs/ directory: getting-started, API reference, for-developers (quickstart, sdk, build-pipeline, submission), security, self-hosting (docker, fly-io, env-vars, database, monitoring, ssl, updating, troubleshooting), color-palette, requirements, index, architecture

---

## Phase 2 — Hardening & Vision Alignment (COMPLETE)

Wire the complete platform to the "Rust games at any scale" vision; replace stale copy; close gaps between service code and live endpoints.

### Backend hardening
- [x] Upgraded sqlx 0.7.4 → 0.8.6 (cleared future-incompat warnings)
- [x] `cargo fix` + 0 compiler warnings (baseline was 341)
- [x] All API modules verified as wired into the Axum router
- [x] Backend distribution module: artifact registration, build webhooks, play-manifest (`distribution.rs` + migration)
- [ ] Circle SDK integration: real wallet creation, deposit, withdrawal flows (stubs wired; live integration future)
- [ ] Paystack integration: ZAR → USDC on-ramp, webhook verification (stubs wired; live integration future)
- [ ] Email templates: welcome, verification, password-reset, payout, anti-cheat alert (lettre wired; Handlebars templates future)
- [ ] Session refresh-token rotation (endpoint + service)
- [ ] Force-logout / token revocation
- [ ] Partial + composite DB indexes for hot queries

### Frontend wiring (mock → real API)
- [x] Marketplace, GameDetail wired to `GET /api/games` (mock fallback retained)
- [x] Wallet wired to `GET /api/wallet/balance` + transaction history (mock fallback retained)
- [x] Leaderboard wired to `GET /api/leaderboard` (mock fallback retained)
- [x] Achievements wired to `GET /api/achievements` (mock fallback retained)
- [x] Friends / social wired to `/api/social` + `/api/friends` (mock fallback retained)
- [x] Notifications wired to `GET /api/notifications` (mock fallback retained)
- [x] Profile / EditProfile wired to `/api/profile` (mock fallback retained)
- [x] DeveloperDashboard wired to `/api/developer` stats (mock fallback retained)
- [x] Wishlist wired to `/api/wishlist` endpoints
- [ ] Subscriptions / Pricing wire to `/api/subscriptions`
- [ ] Matchmaking: full WebSocket integration end-to-end
- [ ] Email verification + password-reset flows (pages exist; token dispatch pending email templates)

### Design system — "Industrial Magnetite"
- [x] Design tokens (`--color-*`, `--radius-*`, `--t-*`, `--font-*`) in `src/styles/tokens.css` + `src/index.css`
- [x] All common/ components restyled to Industrial Magnetite tokens
- [x] All 67+ pages restyled to new design system
- [x] Light theme implementation under `[data-theme="light"]`
- [x] Magnetic ring / field-line hero backdrop (HeroSection)
- [x] Entrance fade/slide animations; card magnetic hover; stat count-up
- [x] `prefers-reduced-motion` respected throughout
- [x] Elevated typography: Archivo display / Hanken Grotesk body / JetBrains Mono; Google Fonts loaded
- [ ] WCAG AA contrast audit (automated audit pending; color tokens selected for AA compliance)

---

## Phase 3 — WASM Build & Hosting Pipeline (COMPLETE)

Ship the end-to-end path from developer Rust source to player-facing WASM game.

- [x] Backend distribution API: artifact/version registration, play-manifest endpoint, build-webhook receiver
- [x] `game-templates/arcade/build.sh`: cargo → wasm-bindgen → wasm-opt pipeline defined
- [x] `game-ci.yml` / `game-deploy.yml`: WASM build steps + S3 upload placeholders
- [x] Developer portal pages: GameDeploy, DeploymentStatus, BuildLogs implemented
- [ ] Platform CI: live end-to-end (GitHub webhook → pull source → `cargo build --target wasm32-unknown-unknown` → wasm-opt → security scan → sandboxed smoke test → store artifact in S3)
- [ ] WASM artifact hosting: CDN-backed URLs served per game version
- [ ] In-browser WASM game runner: iframe or web-worker sandboxed loader
- [ ] Game versioning: multiple live versions, developer-controlled rollout
- [ ] WASM size budget enforcement (wasm-opt budget gate in CI)
- [ ] Native binary distribution: signed native builds for desktop platforms
- [ ] Developer webhook: notify on build pass/fail (email delivery; endpoint exists)
- [ ] Replay storage: server-side game replays in S3 for anti-cheat review

---

## Phase 4 — SDK Maturity & Multiplayer (COMPLETE)

Harden the SDK and deliver a first-class real-time multiplayer experience.

- [x] `magnetite-sdk` rewritten: versioned wire protocol, netcode prediction buffer, interest management, fixed-timestep tick loop; 240 tests; 0 warnings
- [x] Anti-cheat service: velocity + anomaly detection, global ban list (`anticheat.rs`)
- [x] `export_game!` macro: FFI glue for runtime discovery + instantiation
- [ ] Stable `magnetite-sdk` API (1.0 semver commitment + crates.io publish)
- [ ] SDK: fully deterministic game tick with rollback support
- [ ] SDK: QUIC / WebTransport transport layer (quinn) for low-latency netcode
- [ ] SDK: full client-side prediction + server reconciliation helpers
- [ ] SDK: lobby creation, matchmaking integration, spectator hooks
- [ ] Anti-cheat layer 2: replay analysis, per-game custom rules; wire to game sessions
- [ ] Game isolation: Wasmtime WASM sandbox for untrusted server-side code
- [ ] gVisor container isolation for native game server processes
- [ ] Resource limits + timeout enforcement per game instance
- [ ] Skill-based matchmaking (ELO / Glicko-2)
- [ ] Region-aware matchmaking (Fly.io multi-region)
- [ ] Party / group matchmaking

---

## Phase 5 — Gaming Suite: Comms, Controllers, Economy & Marketplace (COMPLETE)

Extend Magnetite from a game host into a full gaming suite with Discord-class communications, controller support, a points/score economy, developer-run marketplaces, and advanced game templates.

### Backend
- [x] Migration `20260530_communities.sql`: 11 tables — communities, community_members, channels, channel_members, messages, dm_threads, dm_messages, presence, voice_rooms, voice_participants, streams
- [x] Migration `20260531_economy.sql`: 8 tables — seasons, points_ledger, point_balances, point_rewards, dev_stores, store_items, store_purchases, entitlements
- [x] API modules: communities, channels, messages, points, marketplace (34 total, wired in router)
- [x] Services: communities, presence, points (atomic ledger + season reset), marketplace (store/item CRUD, purchase, entitlements) — **0 warnings**
- [x] WebSocket handlers: `ws/comms.rs` (chat + presence broadcast over broadcast channels), `ws/voice.rs` (WebRTC SDP/ICE signaling relay; mesh for small rooms; SFU as documented scale path)

### SDK (`magnetite-sdk`)
- [x] `input/gamepad`: `GamepadState`, `GamepadButton`, `GamepadAxis`, `GamepadEvent`, `InputMap`, `GameAction`, `InputBinding`, `InputSource` — unified binding layer (gamepad + keyboard)
- [x] `graphics`: `GraphicsTier` (Lite2D / Standard3D / Advanced3D), `RenderConfig`, `RenderConfigBuilder`, `EngineCapability`
- [x] `platform::comms`: `CommsClient`, `ChatMessage`, `VoiceSignal`, `PresenceUpdate` — typed SDK surface for in-game chat/voice
- [x] `platform::points`: `PointsClient`, `AwardPointsRequest`, `SpendPointsRequest`, `LedgerEntry`, `PointsBalance`
- [x] `platform::marketplace`: `MarketplaceClient`, `StoreItem`, `PurchaseRequest`, `PurchaseResult`, `Entitlement`, `ItemType`
- [x] `platform::cloud_save`: `CloudSaveClient`, `SaveSlot`, `SaveRequest`
- [x] **240 tests pass, 0 warnings**

### Game templates
- [x] `game-templates/fps/` (`magnetite-fps-starter`): Bevy + rapier3d, hitscan, gamepad look/move/shoot, Advanced3D tier; `cargo check --no-default-features` 0/0, 38 tests
- [x] `game-templates/motorsport/` (`magnetite-game-motorsport`): vehicle physics, analog throttle/brake/steer, lap → points, rapier3d; `cargo check --no-default-features` 0/0, 26 tests

### Frontend
- [x] Communities page (`/communities`): server rail, channel list, real-time chat, typing indicators, voice panel, member list, presence dots, CommsProvider context
- [x] Messages page (`/messages`): DM threads + conversation, presence, real-time updates
- [x] Streams page (`/streams`): browse live streams grid; StreamPlayer (HLS/WebRTC); GoLivePanel (getDisplayMedia + RTMP key)
- [x] Points dashboard (`/points`): balance, history, leaderboard, season info
- [x] Dev Marketplace (`/developers/marketplace`): store creation/edit, item CRUD, revenue overview
- [x] Controller Settings (`/settings/controller`): live Gamepad API display + binding editor
- [x] In-game store panel: `InGameStore` component (overlay purchasable items during play)
- [x] GameOverlay: in-game chat + voice hotkey overlay used by Playground / Lobby / Spectator
- [x] `useGamepad` hook: live Gamepad API polling with button/axis callbacks
- [x] `usePoints` hook: balance, history, award, spend
- [x] `useMarketplace` hook: store, items, purchase, entitlements
- [x] `useVoiceClient` hook: getUserMedia + RTCPeerConnection mesh + Web Audio speaking ring
- [x] `useCommunities`, `useChannels`, `useMessages`, `usePresence`, `useVoice`, `useCommsSocket` hooks
- [x] CommsContext provider (mounted at App root); CommsSocket WebSocket management

### Docs
- [x] `docs/comms/`: index.md, realtime.md, data-model.md, in-game.md
- [x] `docs/comms/streaming.md`: RTMP egress, HLS/WebRTC watch, GoLivePanel usage
- [x] `docs/for-developers/controllers.md`: gamepad input, InputMap, binding editor
- [x] `docs/for-developers/graphics-tiers.md`: Lite2D / Standard3D / Advanced3D
- [x] `docs/for-developers/points-economy.md`: ledger, seasons, SDK integration
- [x] `docs/for-developers/marketplace.md`: dev stores, items, purchase, entitlements
- [x] `docs/for-developers/fps-starter.md`: game-template-fps usage guide
- [x] `docs/for-developers/motorsport-starter.md`: game-template-motorsport usage guide
- [x] `docs/economy-marketplace.md`: data model, revenue split, API reference
- [x] `docs/architecture.md`: updated with new modules, services, WS handlers, and SDK surface

---

## Phase 6 — Scaling to Large Titles

Prove Magnetite scales from game-jam to AAA.

- [ ] Horizontal game server scaling (Fly Machines API, auto-scale on player count)
- [ ] Dedicated server support: developer-supplied server binaries, managed lifecycle
- [ ] Multi-region PostgreSQL (read replicas) + Redis Cluster
- [ ] Spectator capacity: thousands of concurrent spectators per match
- [ ] Tournament engine: brackets, scheduling, prize pools, live standings
- [ ] Developer analytics: DAU, retention, session length, revenue breakdown
- [ ] Platform metrics: Prometheus export + Grafana dashboards
- [ ] CDN / edge caching for game assets
- [ ] Load testing suite (k6 / wrk) targeting 10 k concurrent players
- [ ] Voice SFU upgrade: replace WebRTC mesh with LiveKit or mediasoup at 8+ participants
- [ ] RTMP ingest relay: full CDN-backed transcoding pipeline for platform streaming

---

## Phase 7 — Distribution & Community Flywheel

Complete the marketplace flywheel.

- [ ] Storefront: featured games, editorial picks, new releases, top charts
- [ ] Game discoverability: tags, categories, advanced search, recommendations
- [ ] Player reviews + verified-purchase gating
- [ ] Developer profiles: portfolio pages, follower count, game catalog
- [ ] Mobile-optimized game browser (responsive marketplace)
- [ ] Native mobile app (React Native or PWA) for discovery + social
- [ ] Community moderation tools: report, warn, kick, temporary ban per-community

---

## Open Source

| Component | License |
|-----------|---------|
| Platform API (`backend/`) | MIT |
| SDK (`backend/magnetite-sdk/`) | MIT |
| Game Templates (`game-template*/`) | MIT |
| Frontend (`src/`) | MIT |
| Docs (`docs/`) | CC0 |

---

*Built with Rust. Powered by open source.*
