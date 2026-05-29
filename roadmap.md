# Magnetite — Platform Roadmap

**Magnetite (Fe₃O₄)** — Iron oxide, magnetic, grounded.

*Build, distribute, and monetize Rust games — from a weekend jam to a COD-scale AAA title.*

---

## Vision

Magnetite is the open-source platform for Rust games at any scale. Game logic is authored in Rust; clients compile Bevy → WASM for browsers and to native binaries. The platform is server-authoritative and sandboxed, providing the heavy infrastructure (hosting, matchmaking, real-time netcode, persistence, distribution, payments) so developers write only game logic.

**The "HTML5 games" framing is retired.** Magnetite is a Rust-native platform.

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
│  Analytics · Cache · Friends · Invites · Session · Verification │
├─────────────────────────────────────────────────────────────────┤
│  Persistence                                                    │
│  PostgreSQL 16 (20 migrations) · Redis 7 (sessions/cache)       │
│  AWS S3 (game builds, replays, assets)                          │
├─────────────────────────────────────────────────────────────────┤
│  Game Instances (Developer Code)                                │
│  Client: Rust + Bevy → WASM (browser) or native binary         │
│  Server: Rust, server-authoritative, sandboxed (gVisor planned) │
├─────────────────────────────────────────────────────────────────┤
│  magnetite-sdk (MIT Rust crate)                                 │
│  GameLogic trait · Input/State types · Networking protocol      │
└─────────────────────────────────────────────────────────────────┘
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

### Developer workflow (SDK → platform)

1. Clone `game-template` (Bevy + magnetite-sdk)
2. Implement `GameLogic` trait
3. Push to GitHub
4. Register repo on Magnetite developer dashboard
5. Platform CI: `cargo build --target wasm32-unknown-unknown`, security scan, sandboxed smoke test, manual review, live

---

## Phase 1 — Foundation (COMPLETE)

The backend API, database schema, and core frontend scaffold are built and compile.

### Backend
- [x] Axum HTTP server with config, unified error type, structured logging
- [x] 27 API modules: auth, oauth, games, developer, wallet, subscriptions, matchmaking, leaderboard, achievements, social, notifications, profile, tournaments, admin, categories, reviews, sessions, search, platform, metrics, versioning, webhooks, github, health, middleware, response helpers, wishlist
- [x] 18 service modules: auth, games, wallet, payment, payout, matchmaking, leaderboard, achievements, analytics, anticheat, cache, email, friends, health, invites, session, verification
- [x] Middleware: CORS (tower-http), rate limiting, request logging
- [x] Background jobs: session cleanup, notification cleanup, database backup
- [x] WebSocket game handler (`/ws/game/{id}`)
- [x] PostgreSQL connection pool (SQLx)
- [x] Redis integration (sessions, cache, pub/sub)
- [x] AWS S3 integration (game artifacts)
- [x] Email sending (lettre; Resend + AWS SES)
- [x] JWT auth (jsonwebtoken) + Argon2 password hashing
- [x] 20 SQL migrations (users, games, sessions, wallets, scores, achievements, social, tournaments, notifications, subscriptions, anti-cheat, …)
- [x] Integration tests: auth, API, wallet (backend/tests/)

### Frontend
- [x] React 19 + Vite + React Router 7 scaffold
- [x] 59 top-level page components + admin/ (5) + developers/ (3) subdirs = 67 pages total
- [x] 118 component files across common/, landing/, auth/, admin/, charts/, skeletons/, empty/
- [x] Common design-system primitives: Button, Input, Card, Badge, Modal, Table, Tabs, Pagination, Progress, Checkbox, Radio, Select, Switch, Tooltip, Spinner, Avatar, Breadcrumb, ConfirmDialog
- [x] Landing page sections: HeroSection, FeaturesSection, HowItWorksSection, TestimonialsSection, DeveloperCTA, FinalCTA
- [x] Auth components: AuthForm, OAuthButtons, EmailInput, PasswordInput, SocialProof, TermsCheckbox
- [x] Auth pages: Login, Register, ForgotPassword, ResetPassword, VerifyEmail, AuthCallback
- [x] Core pages: Home (landing), Marketplace, GameDetail, DeveloperDashboard, GameStudio, Earnings, Settings, Wallet, Matchmaking, Pricing, Subscription, Profile, EditProfile, Leaderboard, Achievements, Friends, Wishlist, Onboarding
- [x] Admin pages: AdminDashboard, Users, Games, Finance, Settings
- [x] Developer sub-pages: GameDeploy, DeploymentStatus, BuildLogs
- [x] Social/misc pages: About, Contact, Careers, Spectator, GameLobby, Playground, GameAccess, LinkAccount, ConnectedAccounts, PrivacySettings, Security, Welcome
- [x] Legal pages: Terms, Privacy, Cookies, FAQ
- [x] Error pages: NotFound (404), Forbidden (403), ServerError (500), Error
- [x] Chart components: LineChart, BarChart, AreaChart, PieChart, EarningsChart, RevenueChart, PlayersChart (Recharts)
- [x] Skeleton loaders: Game, GameGrid, Leaderboard, Profile, Transaction
- [x] Empty states: NoGames, NoFriends, NoTransactions, NoSearchResults
- [x] Contexts: AuthContext, WalletContext, GameContext, ThemeContext, ToastContext, NotificationContext, AnnouncementContext
- [x] 30+ hooks: useAuth, useGames, useWallet, useMatchmaking, useLeaderboard, useNotifications, useWebSocket, useGameSession, useGameLobby, useSearch, usePagination, useInfiniteScroll, useAnimation, useCountUp, useKeyboardShortcuts, useFeatureFlag, useToast, useTour, useTypingEffect, useParallax, useMediaQuery, useWindowSize, useIsMobile, useDebounce, useClickOutside, useFocusTrap, useMemoOne, useUser, useTheme, useAnnouncement, useIntersectionObserver
- [x] Utilities: formatters, validation, formValidation, featureFlags, storage, currency, date helpers
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
- [x] docs/ directory: getting-started, API reference, for-developers, security, self-hosting, troubleshooting, color-palette, requirements, index
- [x] Playwright e2e tests: auth.spec.js, marketplace.spec.js, navigation.spec.js

---

## Phase 2 — Hardening & Vision Alignment (In Progress)

Wire the complete platform to the "Rust games at any scale" vision; replace stale copy; close gaps between service code and live endpoints.

### Backend hardening
- [ ] Upgrade sqlx 0.7 → 0.8.x (clears future-incompat warnings)
- [ ] `cargo fix` + eliminate all compiler warnings (341 at baseline)
- [ ] Verify all 27 API modules are wired into the Axum router
- [ ] Wire OAuth providers (Google, Discord, GitHub, GitLab) end-to-end with real callback URIs
- [ ] Circle SDK integration: real wallet creation, deposit, withdrawal flows
- [ ] Paystack integration: ZAR → USDC on-ramp, webhook verification
- [ ] Email templates: welcome, verification, password-reset, payout, anti-cheat alert (Handlebars + lettre)
- [ ] Session refresh-token rotation (endpoint + service)
- [ ] Force-logout / token revocation
- [ ] Partial + composite DB indexes for hot queries

### Frontend wiring (mock → real API)
- [ ] Wire Marketplace, GameDetail to `GET /api/games` (replace mockGames)
- [ ] Wire Wallet page to `GET /api/wallet/balance` + transaction history
- [ ] Wire Leaderboard to `GET /api/leaderboard`
- [ ] Wire Achievements to `GET /api/achievements`
- [ ] Wire Friends / social pages to `/api/social` + `/api/friends`
- [ ] Wire Notifications to `GET /api/notifications`
- [ ] Wire Profile / EditProfile to `/api/profile`
- [ ] Wire DeveloperDashboard to `/api/developer` stats
- [ ] Wire Subscriptions / Pricing to `/api/subscriptions`
- [ ] Wire Matchmaking flow end-to-end with WebSocket
- [ ] OAuth login/connect flows (Google, Discord, GitHub, GitLab)
- [ ] Email verification + password-reset flows

### Design system — "Industrial Magnetite"
- [ ] Apply design tokens (`--color-*`, `--radius-*`, `--t-*`) to `src/index.css`
- [ ] Restyle all common/ components to Industrial Magnetite tokens
- [ ] Restyle all 67 pages to new design system
- [ ] Light theme implementation under `[data-theme="light"]`
- [ ] Magnetic ring / field-line hero backdrop (HeroSection)
- [ ] Entrance fade/slide animations; card magnetic hover; stat count-up
- [ ] Respect `prefers-reduced-motion`
- [ ] WCAG AA contrast audit

---

## Phase 3 — WASM Build & Hosting Pipeline

Ship the end-to-end path from developer Rust source to player-facing WASM game.

- [ ] Platform CI: GitHub webhook → pull source → `cargo build --target wasm32-unknown-unknown` → security scan → sandboxed smoke test → store artifact in S3
- [ ] WASM artifact hosting: CDN-backed URLs served per game version
- [ ] In-browser WASM game runner: iframe or web-worker sandboxed loader
- [ ] Game versioning: multiple live versions, developer-controlled rollout
- [ ] WASM size budget enforcement + optimization (wasm-opt)
- [ ] Native binary distribution: signed native builds for desktop platforms
- [ ] Developer webhook: notify on build pass/fail (email + dashboard)
- [ ] Replay storage: server-side game replays in S3 for anti-cheat review

---

## Phase 4 — SDK Maturity & Multiplayer

Harden the SDK and deliver a first-class real-time multiplayer experience.

- [ ] Stable `magnetite-sdk` API (1.0 semver commitment)
- [ ] SDK: deterministic game tick (fixed timestep, rollback-ready)
- [ ] SDK: QUIC / WebTransport transport layer (quinn) for low-latency netcode
- [ ] SDK: client-side prediction + server reconciliation helpers
- [ ] SDK: lobby creation, matchmaking integration, spectator hooks
- [ ] Anti-cheat layer 1: server-authoritative state, velocity + anomaly detection, global ban list
- [ ] Anti-cheat layer 2: replay analysis, per-game custom rules
- [ ] Game isolation: Wasmtime WASM sandbox for untrusted server-side code
- [ ] gVisor container isolation for native game server processes
- [ ] Resource limits + timeout enforcement per game instance
- [ ] Skill-based matchmaking (ELO / Glicko-2)
- [ ] Region-aware matchmaking (Fly.io multi-region)
- [ ] Party / group matchmaking

---

## Phase 5 — Scaling to Large Titles

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

---

## Phase 6 — Distribution & Community

Complete the marketplace flywheel.

- [ ] Storefront: featured games, editorial picks, new releases, top charts
- [ ] Game discoverability: tags, categories, advanced search, recommendations
- [ ] Player reviews + verified-purchase gating
- [ ] Wishlists (backend: exists; frontend: wire to API)
- [ ] Developer profiles: portfolio pages, follower count, game catalog
- [ ] Community features: game-specific forums or Discord integration
- [ ] Mobile-optimized game browser (responsive marketplace)
- [ ] Native mobile app (React Native or PWA) for discovery + social

---

## Open Source

| Component | License |
|-----------|---------|
| Platform API (`backend/`) | MIT |
| SDK (`backend/magnetite-sdk/`) | MIT |
| Game Template (`game-template/`) | MIT |
| Frontend (`src/`) | MIT |
| Docs (`docs/`) | CC0 |

---

*Built with Rust. Powered by open source.*
