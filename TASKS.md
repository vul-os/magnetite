# Magnetite — Implementation Tasks

Tasks are derived from the actual files present in the repository.
Check marks mean the code exists and compiles; unchecked items are genuine gaps.

---

## Backend API modules (`backend/src/api/`)

- [x] `auth.rs` — register, login, logout, refresh, me
- [x] `oauth.rs` — Google, Discord, GitHub, GitLab OAuth flows + callbacks
- [x] `games.rs` — game CRUD, search, categories, screenshots, versions
- [x] `developer.rs` — developer dashboard stats, game management
- [x] `wallet.rs` — balance, deposit, withdraw, transaction history
- [x] `subscriptions.rs` — subscription tiers, activation, status
- [x] `matchmaking.rs` — join queue, leave queue, status
- [x] `leaderboard.rs` — global and per-game leaderboards, score submission
- [x] `achievements.rs` — definitions, progress, unlock
- [x] `social.rs` — friends, invites, activity feed
- [x] `notifications.rs` — list, mark read, preferences
- [x] `profile.rs` — view, edit, avatar, public stats
- [x] `tournaments.rs` — create, join, bracket
- [x] `admin.rs` — user management, game moderation, finance, platform settings
- [x] `categories.rs` — game category taxonomy
- [x] `reviews.rs` — game reviews, ratings (inside games or separate module)
- [x] `sessions.rs` — game session records
- [x] `search.rs` — full-text / indexed search
- [x] `platform.rs` — platform-wide settings and info
- [x] `metrics.rs` — platform metrics (Prometheus-style)
- [x] `versioning.rs` — API versioning helpers
- [x] `webhooks.rs` — GitHub push events, payment webhooks
- [x] `github.rs` — GitHub App installation, repo registration
- [x] `health.rs` — liveness + readiness probes
- [x] `middleware.rs` — JWT auth extractor used by protected routes
- [x] `response.rs` — unified response/error helpers
- [x] `wishlist.rs` — per-user game wishlists
- [ ] All 27 modules verified as wired into Axum router in `main.rs` / `lib.rs`

---

## Backend services (`backend/src/services/`)

- [x] `auth.rs` — JWT issuance, argon2 hashing, token validation
- [x] `games.rs` — game business logic, status transitions
- [x] `wallet.rs` — USDC balance management, transfer logic
- [x] `payment.rs` — Circle / Paystack payment processing stubs
- [x] `payout.rs` — playtime-based developer payout calculation
- [x] `matchmaking.rs` — queue management, match formation
- [x] `leaderboard.rs` — score storage, ranking (Redis-backed)
- [x] `achievements.rs` — achievement definition, progress tracking, unlock
- [x] `analytics.rs` — event ingestion, aggregation
- [x] `anticheat.rs` — velocity detection, anomaly detection, ban list (not yet wired to game sessions)
- [x] `cache.rs` — Redis cache operations
- [x] `email.rs` — email dispatch via lettre (SMTP / Resend / SES)
- [x] `friends.rs` — friend request / accept / reject / list
- [x] `health.rs` — dependency health checks
- [x] `invites.rs` — game invite generation and redemption
- [x] `session.rs` — game session lifecycle
- [x] `verification.rs` — email / identity verification tokens
- [x] `wallet.rs` (service) — see above
- [ ] Circle SDK: real wallet creation, deposit, withdrawal (stubs exist; live integration missing)
- [ ] Paystack: ZAR → USDC on-ramp + webhook verification (stubs exist; live integration missing)
- [ ] Email templates rendered (Handlebars; lettre wired; templates for welcome, verification, reset, payout, anti-cheat needed)
- [ ] Anti-cheat wired to WebSocket game sessions (service exists; hookup missing)

---

## Backend middleware (`backend/src/middleware/`)

- [x] `cors.rs` — CORS layer (tower-http)
- [x] `logging.rs` — request/response logging
- [x] `rate_limit.rs` — per-IP rate limiting

---

## Backend WebSocket (`backend/src/ws/`)

- [x] `game.rs` — WebSocket game handler, message routing
- [ ] Authoritative game state broadcast to all session players
- [ ] Player input validation and tick integration
- [ ] Latency compensation / rollback support

---

## Background jobs (`backend/src/jobs/`)

- [x] `session_cleanup.rs` — purge expired game sessions
- [x] `notification_cleanup.rs` — prune old notifications
- [x] `backup.rs` — scheduled database backup

---

## Database migrations (`backend/migrations/`)

- [x] `20250119_*` — initial schema (users, games, transactions, scores)
- [x] `20250120_*` — admin fields, sessions table
- [x] `20250121_*` — achievements, social features
- [x] `20250122_*` — anti-cheat table, friendships
- [x] `20250123_*` — user profile fields, wishlists
- [x] `20250125_*` — basic indexes
- [x] `20250126_*` — search indexes, categories, game sessions, notifications, platform settings
- [x] `20250127_*` — reviews, verification tokens
- [x] `20250128_*` — wallet service tables
- [x] `20250130_*` — subscription model, tournaments
- [ ] Partial indexes for active/live records
- [ ] Composite indexes for common query patterns (e.g., game+user+score)

---

## magnetite-sdk (`backend/magnetite-sdk/src/`)

- [x] `GameLogic` trait (`fn new`, `handle_input`, `tick`, `state`, `players`, `metadata`)
- [x] `GameMetadata` struct (name, max_players, tick_rate)
- [x] `Input` / `Action` / `KeyCode` / `KeyState` / `MouseState` types (`input.rs`)
- [x] `GameState` / `PlayerId` / `PlayerState` / `Position` / `Rotation` types (`state.rs`)
- [x] `Connection` / `Message` / `NetworkManager` / `ServerNetworkManager` / `StateSyncProtocol` (`networking.rs`)
- [ ] Stable semver-committed API (1.0)
- [ ] Deterministic fixed-timestep tick (rollback-ready)
- [ ] QUIC / WebTransport transport (quinn integration)
- [ ] Client-side prediction + server reconciliation helpers
- [ ] Lobby / spectator hooks in SDK surface
- [ ] Published to crates.io

---

## game-template (`game-template/src/`)

- [x] Bevy plugin (`GamePlugin`) with `handle_input_system` and `tick_system`
- [x] Implements `GameLogic` for `GamePluginState`
- [x] wasm-bindgen `#[wasm_bindgen(start)]` entry point
- [x] `build.sh` WASM build script
- [ ] `wasm-opt` size optimization step in build script
- [ ] CI smoke test (headless WASM load + tick check)
- [ ] Published as GitHub template repo

---

## Backend infrastructure

- [x] `Dockerfile.backend`
- [x] `Dockerfile.frontend`
- [x] `Dockerfile.fly`
- [x] `docker-compose.yml` (postgres 16, redis 7, backend, frontend, nginx)
- [x] `docker-compose.override.yml` (local dev overrides)
- [x] `nginx.conf`
- [x] `fly.toml`
- [x] `backend/tools/migrate.sh` (up, status, reset)
- [x] `backend/tools/backup.sh`
- [x] `.env.example` (full variable list)
- [x] `.github/workflows/ci.yml`
- [x] `.github/workflows/deploy.yml`
- [x] `.github/workflows/game-ci.yml`
- [x] `.github/workflows/game-deploy.yml`
- [x] `.github/workflows/release.yml`

---

## Backend tests (`backend/tests/`)

- [x] `auth_tests.rs`
- [x] `api_tests.rs`
- [x] `wallet_tests.rs`
- [x] `common/mod.rs` (test helpers)
- [ ] Service unit tests (individual service modules)
- [ ] WebSocket integration tests
- [ ] Anti-cheat unit tests

---

## Frontend — pages (`src/pages/`)

- [x] `Home.jsx` + `Home.css` — landing page
- [x] `Login.jsx` + `Login.test.jsx`
- [x] `Register.jsx` + `Register.test.jsx`
- [x] `ForgotPassword.jsx`
- [x] `ResetPassword.jsx`
- [x] `UpdatePassword.jsx`
- [x] `VerifyEmail.jsx`
- [x] `AuthCallback.jsx`
- [x] `Marketplace.jsx` + `Marketplace.css`
- [x] `GameDetail.jsx` + `GameDetail.css`
- [x] `DeveloperDashboard.jsx` + `DeveloperDashboard.css`
- [x] `GameStudio.jsx`
- [x] `Earnings.jsx`
- [x] `Settings.jsx`
- [x] `Wallet.jsx` + `Wallet.css`
- [x] `Subscription.jsx` + `Subscription.css`
- [x] `Pricing.jsx` + `Pricing.css`
- [x] `Matchmaking.jsx`
- [x] `Leaderboard.jsx`
- [x] `Achievements.jsx`
- [x] `Profile.jsx`
- [x] `EditProfile.jsx`
- [x] `Friends.jsx`
- [x] `Wishlist.jsx`
- [x] `Onboarding.jsx` + `Onboarding.css`
- [x] `Welcome.jsx` + `Welcome.css`
- [x] `GameLobby.jsx`
- [x] `GameAccess.jsx` + `GameAccess.css`
- [x] `Spectator.jsx`
- [x] `Playground.jsx`
- [x] `LinkAccount.jsx`
- [x] `ConnectedAccounts.jsx`
- [x] `PrivacySettings.jsx`
- [x] `Security.jsx`
- [x] `About.jsx` + `About.css`
- [x] `Contact.jsx` + `Contact.css`
- [x] `Careers.jsx` + `Careers.css`
- [x] `Terms.jsx`
- [x] `Privacy.jsx`
- [x] `Cookies.jsx`
- [x] `FAQ.jsx`
- [x] `NotFound.jsx` / `403.jsx` / `500.jsx` / `Error.jsx` / `Forbidden.jsx` / `ServerError.jsx`
- [x] `LoadingPage.jsx` + `PageTransition.jsx`
- [x] `admin/AdminDashboard.jsx`, `admin/Users.jsx`, `admin/Games.jsx`, `admin/Finance.jsx`, `admin/Settings.jsx`
- [x] `developers/GameDeploy.jsx`, `developers/DeploymentStatus.jsx`, `developers/BuildLogs.jsx`

---

## Frontend — components (`src/components/`)

### common/ (design-system primitives)
- [x] Button, Input, Card, Badge, Modal, Table, Tabs
- [x] Pagination, Progress, Checkbox, Radio, Select, Switch
- [x] Tooltip, Spinner, Avatar, Breadcrumb, ConfirmDialog / ConfirmDialogContext / useConfirm

### landing/
- [x] LandingPage, HeroSection, FeaturesSection, HowItWorksSection
- [x] TestimonialsSection, DeveloperCTA, FinalCTA

### auth/
- [x] AuthForm, OAuthButtons, EmailInput, PasswordInput, SocialProof, TermsCheckbox

### admin/
- [x] AdminRoute, AdminSidebar

### charts/ (Recharts wrappers)
- [x] LineChart, BarChart, AreaChart, PieChart, EarningsChart, RevenueChart, PlayersChart

### skeletons/
- [x] Skeleton, GameCardSkeleton, GameGridSkeleton, LeaderboardSkeleton, ProfileSkeleton, TransactionSkeleton

### empty/
- [x] EmptyState, NoGames, NoFriends, NoTransactions, NoSearchResults

### Top-level components
- [x] Navbar, Footer, Layout, LegalLayout
- [x] GameCard, GameGallery, GameGridSkeleton, GameHUD, GameScreenshot
- [x] LeaderboardRow, ProfileCard, FriendCard, ReviewCard, ReviewList, CreateReview
- [x] StatsCard, PricingCard, SubscriptionCard, SubscriptionBadge, UsageMeter
- [x] FilterBar, CategoryFilter, SortDropdown, SearchBar, SearchModal, ActiveFilters
- [x] PriceRangeSlider, InfiniteScroll, Pagination
- [x] Modal, AlertModal, ConfirmModal, SelectModal, PaymentModal
- [x] Toast, NotificationBell, NotificationDropdown, NotificationItem
- [x] AnnouncementBanner, ChatWidget, HelpWidget, KeyboardShortcuts
- [x] CookieConsent, ErrorBoundary, AccessibilityProvider
- [x] OnboardingProgress, OnboardingTour, TourStep
- [x] LoadingOverlay, LoadingSpinner, PageLoader
- [x] DepositForm, WithdrawForm, WishlistButton
- [x] PlayerList, ReadyButton, StartGameButton, LobbyChat, GameHUD
- [x] Breadcrumb, ThemeToggle, WidgetWrapper

---

## Frontend — contexts (`src/context/`)

- [x] `AuthContext.jsx`
- [x] `WalletContext.jsx`
- [x] `GameContext.jsx`
- [x] `ThemeContext.jsx` + `themeConstants.js`
- [x] `ToastContext.jsx`
- [x] `NotificationContext.jsx`
- [x] `AnnouncementContext.jsx`
- [ ] `SocketContext.jsx` — WebSocket connection state (missing)

---

## Frontend — hooks (`src/hooks/`)

- [x] `useAuth.js` + `useAuth.test.js`
- [x] `useWallet.js`
- [x] `useGames.js`
- [x] `useMatchmaking.js`
- [x] `useToast.js`
- [x] `useWebSocket.js`
- [x] `useGameSession.js`
- [x] `useGameLobby.js`
- [x] `useLeaderboard.js`
- [x] `useNotifications.js`
- [x] `useSearch.js`
- [x] `usePagination.js`
- [x] `useInfiniteScroll.js`
- [x] `useAnimation.js`
- [x] `useCountUp.js`
- [x] `useKeyboardShortcuts.js`
- [x] `useFeatureFlag.js`
- [x] `useTour.js`
- [x] `useTypingEffect.js`
- [x] `useParallax.js`
- [x] `useMediaQuery.js`
- [x] `useWindowSize.js`
- [x] `useIsMobile.js`
- [x] `useDebounce.js`
- [x] `useClickOutside.js`
- [x] `useFocusTrap.js`
- [x] `useMemoOne.js`
- [x] `useUser.js`
- [x] `useTheme.js`
- [x] `useAnnouncement.js`
- [x] `useIntersectionObserver.js`
- [ ] `useLocalStorage.js` (not present; `src/utils/storage.js` fills some of this)

---

## Frontend — utilities (`src/utils/`)

- [x] `formatters.js`
- [x] `validation.js`
- [x] `formValidation.js`
- [x] `featureFlags.js`
- [x] `storage.js`
- [x] `currency.js`
- [x] `date.js`
- [x] `helpers.js`

---

## Frontend — mock data (`src/data/`)

- [x] `mockGames.js`
- [x] `mockAchievements.js`
- [x] `mockFriends.js`
- [x] `mockLeaderboard.js`
- [x] `mockNotifications.js`
- [x] `mockProfile.js`
- [x] `mockUser.js`
- [x] `faqData.js`

---

## Frontend — API wiring (mock → real)

- [ ] Marketplace: replace `mockGames` with `GET /api/games`
- [ ] GameDetail: replace mock with `GET /api/games/:id`
- [ ] Wallet: replace mock with `GET /api/wallet/balance` + transaction history
- [ ] Leaderboard: replace `mockLeaderboard` with `GET /api/leaderboard`
- [ ] Achievements: replace `mockAchievements` with `GET /api/achievements`
- [ ] Friends: replace `mockFriends` with `GET /api/social/friends`
- [ ] Notifications: replace `mockNotifications` with `GET /api/notifications`
- [ ] Profile: replace `mockProfile` with `GET /api/profile`
- [ ] DeveloperDashboard: wire to `GET /api/developer/stats`
- [ ] Subscriptions / Pricing: wire to `GET /api/subscriptions`
- [ ] Matchmaking: full WebSocket integration
- [ ] OAuth login/connect (Google, Discord, GitHub, GitLab)
- [ ] Email verification + password-reset flows

---

## Frontend — tests

- [x] `src/pages/Login.test.jsx`
- [x] `src/pages/Register.test.jsx`
- [x] `src/components/common/Button.test.jsx`
- [x] `src/components/common/Input.test.jsx`
- [x] `src/hooks/useAuth.test.js`
- [x] Playwright config (`playwright.config.js`)
- [x] `e2e/auth.spec.js`
- [x] `e2e/marketplace.spec.js`
- [x] `e2e/navigation.spec.js`
- [x] `e2e/page-objects/`: base, login, marketplace, navigation page objects
- [ ] Component tests for common/ design system
- [ ] Hook tests beyond useAuth
- [ ] E2E: wallet flow, matchmaking, developer dashboard

---

## Design system — "Industrial Magnetite"

- [ ] Design tokens in `src/index.css` (`--color-*`, `--radius-*`, `--t-*`, font stacks)
- [ ] Restyle all `src/components/common/` to new tokens
- [ ] Restyle all pages (67 total) to Industrial Magnetite
- [ ] Light theme under `[data-theme="light"]`
- [ ] Magnetic ring / field-line hero backdrop (HeroSection)
- [ ] Entrance fade/slide animations; card magnetic hover; stat count-up
- [ ] `prefers-reduced-motion` respected throughout
- [ ] WCAG AA contrast audit

---

## WASM build & hosting pipeline (vision gap)

- [ ] Platform CI: GitHub webhook → pull source → `cargo build --target wasm32-unknown-unknown` → wasm-opt → security scan → sandboxed smoke test → store artifact in S3
- [ ] WASM artifact hosting: CDN-backed per-version URLs
- [ ] In-browser WASM game runner (iframe / web-worker sandbox)
- [ ] Native binary distribution (signed builds for desktop)
- [ ] Game version management: multiple live versions, developer rollout control
- [ ] Developer webhook notifications (build pass/fail)
- [ ] Replay storage in S3 (anti-cheat review)
- [ ] `game-ci.yml` / `game-deploy.yml` wired to actual WASM build steps

---

## SDK maturity & multiplayer (vision gap)

- [ ] Stable `magnetite-sdk` 1.0 API
- [ ] Deterministic fixed-timestep tick (rollback-ready)
- [ ] QUIC / WebTransport transport layer (quinn)
- [ ] Client-side prediction + server reconciliation helpers
- [ ] Lobby creation, spectator hooks in SDK surface
- [ ] Anti-cheat wired to game sessions (velocity + anomaly detection)
- [ ] Wasmtime WASM sandbox for server-side game code
- [ ] gVisor container isolation for native game server processes
- [ ] Resource limits + timeout enforcement per game instance
- [ ] Skill-based matchmaking (ELO / Glicko-2)
- [ ] Region-aware matchmaking (Fly.io multi-region)
- [ ] Party / group matchmaking

---

## Scaling & operations (vision gap)

- [ ] Horizontal game-server scaling (Fly Machines API, auto-scale on player count)
- [ ] Dedicated server support: developer-supplied binaries + managed lifecycle
- [ ] Multi-region PostgreSQL read replicas + Redis Cluster
- [ ] Spectator scaling: thousands of concurrent spectators per match
- [ ] Tournament engine: brackets, scheduling, prize pools, live standings
- [ ] Developer analytics: DAU, retention, session length, revenue breakdown
- [ ] Platform Prometheus metrics export + Grafana dashboards
- [ ] CDN / edge caching for game assets
- [ ] Load testing suite (k6 / wrk) targeting 10 k concurrent players
- [ ] Refresh-token rotation + force-logout endpoint

---

## Docs (`docs/`)

- [x] `docs/index.md`
- [x] `docs/requirements.md`
- [x] `docs/getting-started/` directory
- [x] `docs/for-developers/` directory
- [x] `docs/api-reference/` directory
- [x] `docs/security/` directory
- [x] `docs/self-hosting/` directory
- [x] `docs/troubleshooting.md`
- [x] `docs/color-palette.md`
- [ ] SDK API reference (auto-generated from rustdoc)
- [ ] WASM build pipeline guide
- [ ] Self-hosting guide verified against current docker-compose
- [ ] Contributing guide updated for Rust-games vision
