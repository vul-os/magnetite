# Magnetite — Implementation Tasks

Tasks are derived from the actual files present in the repository.
Check marks mean the code exists and compiles; unchecked items are genuine gaps.

---

## Backend API modules (`backend/src/api/`)

- [x] `auth.rs` — register, login, logout, refresh, me, sessions
- [x] `oauth.rs` — Google, Discord, GitHub, GitLab OAuth flows + callbacks
- [x] `games.rs` — game CRUD, search, categories, screenshots, versions
- [x] `developer.rs` — developer dashboard stats, game management, earnings, payouts
- [x] `wallet.rs` — balance, deposit, withdraw, transaction history
- [x] `subscriptions.rs` — subscription tiers, activation, status
- [x] `matchmaking.rs` — join queue, leave queue, status
- [x] `leaderboard.rs` — global and per-game leaderboards, score submission
- [x] `achievements.rs` — definitions, progress, unlock
- [x] `social.rs` — friends, invites, activity feed
- [x] `notifications.rs` — list, mark read, preferences
- [x] `profile.rs` — view, edit, avatar, public stats
- [x] `tournaments.rs` — create, join, bracket
- [x] `admin.rs` — user management, game moderation, finance, platform settings, analytics
- [x] `categories.rs` — game category taxonomy
- [x] `reviews.rs` — game reviews, ratings
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
- [x] `distribution.rs` — artifact/version registration, play-manifest, build-webhook (Wave 3)
- [x] `communities.rs` — community CRUD, membership, roles (Wave 6)
- [x] `channels.rs` — channel CRUD within communities (Wave 6)
- [x] `messages.rs` — channel messages + DM threads/messages (Wave 6)
- [x] `points.rs` — balance, award, spend, history, leaderboard, season reset (Wave 8)
- [x] `marketplace.rs` — dev stores, items, purchase, entitlements (Wave 8)
- [x] All 34 modules verified as wired into Axum router in `main.rs` / `lib.rs`

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
- [x] `anticheat.rs` — velocity detection, anomaly detection, ban list
- [x] `cache.rs` — Redis cache operations
- [x] `email.rs` — email dispatch via lettre (SMTP / Resend / SES)
- [x] `friends.rs` — friend request / accept / reject / list
- [x] `health.rs` — dependency health checks
- [x] `invites.rs` — game invite generation and redemption
- [x] `session.rs` — game session lifecycle
- [x] `verification.rs` — email / identity verification tokens
- [x] `distribution.rs` — game artifact/version registration, build webhooks, play manifest (Wave 3)
- [x] `communities.rs` — community + channel + message service (Wave 6)
- [x] `presence.rs` — presence upsert / sweep (Wave 6)
- [x] `points.rs` — atomic ledger inserts, balance management, season reset (Wave 8)
- [x] `marketplace.rs` — store/item CRUD, purchase via USDC or points, entitlements (Wave 8)
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
- [x] `comms.rs` — Real-time chat, typing indicators, presence broadcast over broadcast channels (Wave 6)
- [x] `voice.rs` — WebRTC SDP/ICE signaling relay; mesh architecture; SFU documented as scale path (Wave 6)
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
- [x] `20260530_communities.sql` — 11 tables: communities/members/channels/channel_members/messages/dm_threads/dm_messages/presence/voice_rooms/voice_participants/streams (Wave 6)
- [x] `20260530_game_distribution.sql` — game_artifacts, game_versions, build_jobs (Wave 3)
- [x] `20260531_economy.sql` — seasons/point_balances/points_ledger/point_rewards/dev_stores/store_items/store_purchases/entitlements (Wave 8)
- [ ] Partial indexes for active/live records
- [ ] Composite indexes for common query patterns (e.g., game+user+score)

---

## magnetite-sdk (`backend/magnetite-sdk/src/`)

### Core modules
- [x] `game.rs` — `GameLogic` trait (`new`, `handle_input`, `tick`, `state`, `players`, `metadata`, `snapshot`, `restore`), `GameMetadata`, `export_game!` macro
- [x] `state.rs` — `GameState`, `PlayerId`, `PlayerState`, `Position`, `Rotation`, `Snapshot`
- [x] `input/mod.rs` — `Input`, `Action`, `Direction`, `InputEvent`, `KeyCode`, `KeyState`, `MouseState`
- [x] `input/gamepad.rs` — `GamepadState`, `GamepadButton`, `GamepadAxis`, `GamepadEvent`, `InputMap`, `GameAction`, `InputBinding`, `InputSource` (Wave 8)
- [x] `graphics.rs` — `GraphicsTier` (Lite2D / Standard3D / Advanced3D), `RenderConfig`, `RenderConfigBuilder`, `EngineCapability` (Wave 8)
- [x] `protocol.rs` — `Envelope`, `ClientMessage`, `ServerMessage`, `ErrorCode`, `PROTOCOL_VERSION`
- [x] `networking.rs` — `ServerConfig`, `TickLoop`, `PredictionBuffer`, `InterestManager`, `RadiusInterest`, `FullInterest`, `NetworkManager`, `ServerNetworkManager`, `StateSyncProtocol`

### Platform services
- [x] `platform/comms.rs` — `CommsClient`, `CommsConfig`, `CommsConnectionState`, `ChatMessage`, `VoiceSignal`, `PresenceStatus`, `PresenceUpdate`, `CommsEvent`, `CommsErrorCode`, `ClientCommsMessage`, `ServerCommsMessage` (Wave 6)
- [x] `platform/points.rs` — `PointsClient`, `PointsConfig`, `AwardPointsRequest`, `SpendPointsRequest`, `PointsBalance`, `LedgerEntry`, `LedgerEntryKind`, `PointsErrorCode`, `ClientPointsMessage`, `ServerPointsMessage` (Wave 8)
- [x] `platform/marketplace.rs` — `MarketplaceClient`, `MarketplaceConfig`, `StoreItem`, `ItemType`, `PurchaseRequest`, `PurchaseResult`, `PurchaseErrorCode`, `PaymentMethod`, `Entitlement`, `MarketplaceErrorCode`, `ClientMarketplaceMessage`, `ServerMarketplaceMessage` (Wave 8)
- [x] `platform/cloud_save.rs` — `CloudSaveClient`, `CloudSaveConfig`, `SaveSlot`, `SaveSlotMeta`, `SaveRequest`, `CloudSaveErrorCode`, `ClientCloudSaveMessage`, `ServerCloudSaveMessage` (Wave 8)
- [x] **240 tests pass, 0 warnings**
- [ ] Stable semver-committed API (1.0)
- [ ] Deterministic fixed-timestep tick with full rollback support
- [ ] QUIC / WebTransport transport (quinn integration)
- [ ] Full client-side prediction + server reconciliation helpers
- [ ] Lobby / spectator hooks in SDK surface
- [ ] Published to crates.io

---

## game-template (`game-template/src/`)

- [x] Bevy plugin (`GamePlugin`) with `handle_input_system` and `tick_system`
- [x] Implements `GameLogic` for `GamePluginState`
- [x] wasm-bindgen `#[wasm_bindgen(start)]` entry point
- [x] `build.sh` WASM build script (cargo → wasm-bindgen → wasm-opt; `cargo check` passes)
- [ ] `wasm-opt` fully automated in build script (stub present; requires wasm-opt binary in CI)
- [ ] CI smoke test (headless WASM load + tick check)
- [ ] Published as GitHub template repo

---

## game-template-fps (`game-template-fps/src/`)

- [x] Crate: `magnetite-fps-starter` (Bevy + rapier3d + magnetite-sdk)
- [x] `GameLogic` implemented for `FpsGame`
- [x] `hitscan.rs` — raycast hitscan with hit registration
- [x] `input_map.rs` — gamepad look/move/shoot binding (InputMap + GameAction)
- [x] `level.rs` — procedural level layout
- [x] `bevy_client.rs` — Bevy ECS plugin (native + WASM feature flags)
- [x] `cargo check --no-default-features` passes, 0 warnings, 38 tests
- [ ] Fully playable in-browser WASM build (requires full Bevy WASM compile; not run in CI)
- [ ] Integrated with platform points: kill → award points via `platform::points`

---

## game-template-motorsport (`game-template-motorsport/src/`)

- [x] Crate: `magnetite-game-motorsport` (Bevy + rapier3d + magnetite-sdk)
- [x] `GameLogic` implemented for `MotorsportGame`
- [x] Vehicle physics (rapier3d rigid body + wheel joints)
- [x] Analog throttle / brake / steer via `GamepadAxis`
- [x] Lap timing → points award via `platform::points`
- [x] `cargo check --no-default-features` passes, 0 warnings, 26 tests
- [ ] Fully playable in-browser WASM build (requires full Bevy WASM compile; not run in CI)
- [ ] Multiple track layouts

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
- [x] `ForgotPassword.jsx`, `ResetPassword.jsx`, `UpdatePassword.jsx`, `VerifyEmail.jsx`, `AuthCallback.jsx`
- [x] `Marketplace.jsx` + `Marketplace.css`
- [x] `GameDetail.jsx` + `GameDetail.css`
- [x] `DeveloperDashboard.jsx` + `DeveloperDashboard.css`
- [x] `GameStudio.jsx` + `GameStudio.css`
- [x] `Earnings.jsx` + `Earnings.css`
- [x] `Settings.jsx`, `PrivacySettings.jsx`, `Security.jsx`
- [x] `ControllerSettings.jsx` + `ControllerSettings.css` — live gamepad display + binding editor (Wave 8)
- [x] `Wallet.jsx` + `Wallet.css`
- [x] `Subscription.jsx` + `Subscription.css`
- [x] `Pricing.jsx` + `Pricing.css`
- [x] `Matchmaking.jsx` + `Matchmaking.css`
- [x] `Leaderboard.jsx`
- [x] `Achievements.jsx`
- [x] `Profile.jsx`, `EditProfile.jsx`
- [x] `Friends.jsx`
- [x] `Wishlist.jsx`
- [x] `Onboarding.jsx` + `Onboarding.css`
- [x] `Welcome.jsx` + `Welcome.css`
- [x] `GameLobby.jsx` + `GameLobby.css`
- [x] `GameAccess.jsx` + `GameAccess.css`
- [x] `Spectator.jsx` + `Spectator.css`
- [x] `Playground.jsx` + `Playground.css`
- [x] `LinkAccount.jsx`, `ConnectedAccounts.jsx`
- [x] `About.jsx`, `Contact.jsx`, `Careers.jsx`
- [x] `Terms.jsx`, `Privacy.jsx`, `Cookies.jsx`, `FAQ.jsx`
- [x] `NotFound.jsx`, `403.jsx`, `500.jsx`, `Error.jsx`, `Forbidden.jsx`, `ServerError.jsx`
- [x] `LoadingPage.jsx`, `PageTransition.jsx`
- [x] `admin/` — AdminDashboard, Users, Games, Finance, Settings
- [x] `developers/` — GameDeploy, DeploymentStatus, BuildLogs
- [x] `Communities.jsx` + `Communities.css` — Discord-like server/channel/chat/voice UI (Wave 6–7)
- [x] `Messages.jsx` + `Messages.css` — Direct Messages (Wave 7)
- [x] `Streams.jsx` + `Streams.css` — Browse live streams + StreamPlayer + GoLivePanel (Wave 7)
- [x] `Points.jsx` + `Points.css` — Player points dashboard (Wave 8)
- [x] `DevMarketplace.jsx` + `DevMarketplace.css` — Developer store management (Wave 8)

---

## Frontend — components (`src/components/`)

### common/ (design-system primitives)
- [x] Button, Input, Card, Badge, Modal, Table, Tabs
- [x] Pagination, Progress, Checkbox, Radio, Select, Switch
- [x] Tooltip, Spinner, Avatar, Breadcrumb, ConfirmDialog / ConfirmDialogContext / useConfirm

### comms/ (Wave 6–7)
- [x] ServerRail, ChannelList, MessageList, MessageComposer
- [x] VoicePanel, MemberList, PresenceDot

### store/ (Wave 8)
- [x] InGameStore — in-game purchasable items overlay

### streaming/ (Wave 7)
- [x] StreamCard, StreamPlayer, GoLivePanel

### GameOverlay.jsx (Wave 7)
- [x] In-game chat + voice hotkey overlay (used by Playground / Lobby / Spectator)

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
- [x] CommsContext (in `CommsContext.jsx` or equivalent; mounted via CommsProvider in App.jsx) (Wave 6–7)
- [ ] `SocketContext.jsx` — WebSocket connection state (hook `useWebSocket.js` exists; context wrapper pending)

---

## Frontend — hooks (`src/hooks/`)

- [x] `useAuth.js` + `useAuth.test.js`
- [x] `useWallet.js`, `useGames.js`, `useMatchmaking.js`, `useToast.js`
- [x] `useWebSocket.js`, `useGameSession.js`, `useGameLobby.js`
- [x] `useLeaderboard.js`, `useNotifications.js`, `useSearch.js`
- [x] `usePagination.js`, `useInfiniteScroll.js`, `useAnimation.js`
- [x] `useCountUp.js`, `useKeyboardShortcuts.js`, `useFeatureFlag.js`
- [x] `useTour.js`, `useTypingEffect.js`, `useParallax.js`
- [x] `useMediaQuery.js`, `useWindowSize.js`, `useIsMobile.js`
- [x] `useDebounce.js`, `useClickOutside.js`, `useFocusTrap.js`
- [x] `useMemoOne.js`, `useUser.js`, `useTheme.js`, `useAnnouncement.js`
- [x] `useIntersectionObserver.js`
- [x] `useCommunities.js`, `useChannels.js`, `useMessages.js` (Wave 6)
- [x] `usePresence.js`, `useVoice.js`, `useCommsSocket.js`, `useVoiceClient.js` (Wave 6–7)
- [x] `useGamepad.js` — live Gamepad API polling (Wave 8)
- [x] `usePoints.js` — balance, history, award, spend (Wave 8)
- [x] `useMarketplace.js` — store, items, purchase, entitlements (Wave 8)
- [ ] `useLocalStorage.js` (not present; `src/utils/storage.js` fills some of this)

---

## Frontend — utilities (`src/utils/`)

- [x] `formatters.js`, `validation.js`, `formValidation.js`
- [x] `featureFlags.js`, `storage.js`, `currency.js`, `date.js`, `helpers.js`

---

## Frontend — mock data (`src/data/`)

- [x] `mockGames.js`, `mockAchievements.js`, `mockFriends.js`
- [x] `mockLeaderboard.js`, `mockNotifications.js`, `mockProfile.js`, `mockUser.js`, `faqData.js`

---

## Frontend — API wiring (mock → real)

- [x] Marketplace: wired to `GET /api/games`; mock fallback retained
- [x] GameDetail: wired to `GET /api/games/:id`; mock fallback retained
- [x] Wallet: wired to `GET /api/wallet/balance` + transaction history; mock fallback retained
- [x] Leaderboard: wired to `GET /api/leaderboard`; mock fallback retained
- [x] Achievements: wired to `GET /api/achievements`; mock fallback retained
- [x] Friends: wired to `GET /api/social/friends`; mock fallback retained
- [x] Notifications: wired to `GET /api/notifications`; mock fallback retained
- [x] Profile: wired to `GET /api/profile`; mock fallback retained
- [x] DeveloperDashboard: wired to `GET /api/developer/stats`; mock fallback retained
- [x] Wishlist: wired to `/api/wishlist` endpoints
- [x] Communities: wired to comms hooks (useCommunities / useChannels / useMessages)
- [x] Points: wired to `usePoints` hook → `/api/points`
- [x] DevMarketplace: wired to `useMarketplace` hook → `/api/marketplace`
- [ ] Subscriptions / Pricing: wire to `GET /api/subscriptions`
- [ ] Matchmaking: full WebSocket integration
- [ ] OAuth login/connect (Google, Discord, GitHub, GitLab) — OAuth flows wired server-side; client redirects in place
- [ ] Email verification + password-reset flows

---

## Frontend — tests

- [x] `src/pages/Login.test.jsx` (updated for "Welcome back" / "Sign In" accessible names)
- [x] `src/pages/Register.test.jsx` (updated for "Join Magnetite" / "Create Account" accessible names)
- [x] `src/components/common/Button.test.jsx`
- [x] `src/components/common/Input.test.jsx`
- [x] `src/hooks/useAuth.test.js`
- [x] Playwright config (`playwright.config.js`)
- [x] `e2e/auth.spec.js` (updated selectors: `.auth-submit-btn`, `.auth-error[role="alert"]`, provider aria-labels)
- [x] `e2e/marketplace.spec.js` (updated selectors: `.game-card`, `nav[aria-label="Game categories"]`)
- [x] `e2e/navigation.spec.js` (updated selectors: `nav.navbar a`, `footer.footer a`)
- [x] `e2e/page-objects/`: base, login, marketplace, navigation — all coherent with redesigned UI
- [x] vitest.config.js excludes `e2e/**` (Playwright specs not run by Vitest)
- [ ] Component tests for comms/ and gaming-suite components
- [ ] Hook tests: usePoints, useMarketplace, useGamepad, useCommsSocket
- [ ] E2E: wallet flow, matchmaking, developer dashboard, communities, streams

---

## Design system — "Industrial Magnetite"

- [x] Design tokens in `src/styles/tokens.css` + `src/index.css` (`--color-*`, `--radius-*`, `--t-*`, font stacks)
- [x] Restyle all `src/components/common/` to new tokens
- [x] Restyle all pages (75+ total) to Industrial Magnetite
- [x] Light theme under `[data-theme="light"]`
- [x] Magnetic ring / field-line hero backdrop (HeroSection)
- [x] Entrance fade/slide animations; card magnetic hover; stat count-up
- [x] `prefers-reduced-motion` respected throughout
- [ ] WCAG AA contrast audit (automated audit pending; tokens selected for AA)

---

## WASM build & hosting pipeline

- [x] Backend distribution module: artifact/version registration, play-manifest endpoint, build-webhook receiver (`backend/src/api/distribution.rs` + migration `20260530_game_distribution.sql`)
- [x] `game-template/build.sh` WASM build script (cargo + wasm-bindgen + wasm-opt stub)
- [x] `game-ci.yml` / `game-deploy.yml` wired with WASM build steps and S3 upload placeholders
- [x] Developer dashboard: GameDeploy / DeploymentStatus / BuildLogs pages implemented
- [ ] Platform CI: GitHub webhook → pull source → `cargo build --target wasm32-unknown-unknown` → wasm-opt → security scan → sandboxed smoke test → store artifact in S3 (wasm-opt step live)
- [ ] WASM artifact hosting: CDN-backed per-version URLs
- [ ] In-browser WASM game runner (iframe / web-worker sandbox)
- [ ] Native binary distribution (signed builds for desktop)
- [ ] Game version management: multiple live versions, developer rollout control
- [ ] Developer webhook notifications (build pass/fail — endpoint exists; email dispatch pending)
- [ ] Replay storage in S3 (anti-cheat review)

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

## Comms & streaming (future scale)

- [ ] Replace WebRTC mesh with LiveKit / mediasoup SFU at 8+ voice participants
- [ ] Full CDN-backed RTMP ingest + HLS transcoding pipeline
- [ ] Redis Pub/Sub backend for multi-replica comms broadcast
- [ ] Screen-share stream (getDisplayMedia) as HLS ingest
- [ ] Community moderation: report/warn/kick per-community
- [ ] Community roles: granular permissions per channel

---

## Economy & marketplace (future)

- [ ] Point rewards redeemable for real currency (developer-settable exchange)
- [ ] Seasonal leaderboard with prize pool distribution
- [ ] Subscription-gated store items (pass-based access)
- [ ] Refund flow for store purchases
- [ ] Developer analytics: store revenue breakdown, top items, conversion funnel

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
- [x] `docs/for-developers/` directory (quickstart.md, sdk.md, build-pipeline.md, submission.md)
- [x] `docs/for-developers/controllers.md` — gamepad input, InputMap, binding editor (Wave 8–9)
- [x] `docs/for-developers/graphics-tiers.md` — Lite2D / Standard3D / Advanced3D (Wave 8–9)
- [x] `docs/for-developers/points-economy.md` — ledger, seasons, SDK integration (Wave 8–9)
- [x] `docs/for-developers/marketplace.md` — dev stores, items, purchase, entitlements (Wave 8–9)
- [x] `docs/for-developers/fps-starter.md` — game-template-fps usage guide (Wave 8–9)
- [x] `docs/for-developers/motorsport-starter.md` — game-template-motorsport usage guide (Wave 8–9)
- [x] `docs/api-reference/` directory (index.md, auth.md)
- [x] `docs/security/index.md` — threat model, sandboxing, auth, anti-cheat
- [x] `docs/self-hosting/` directory (docker.md, fly-io.md, environment-variables.md, database.md, monitoring.md, ssl.md, updating.md, troubleshooting.md, quickstart.md)
- [x] `docs/troubleshooting.md`
- [x] `docs/color-palette.md`
- [x] `docs/architecture.md` — backend modules, services, data flow (updated Wave 8–9)
- [x] `docs/comms/index.md` — comms overview (Wave 6)
- [x] `docs/comms/realtime.md` — WS protocol + WebRTC signaling (Wave 6)
- [x] `docs/comms/data-model.md` — communities/channels/messages/voice_rooms/streams (Wave 6)
- [x] `docs/comms/in-game.md` — SDK `platform::comms` for lobby/match (Wave 6)
- [x] `docs/comms/streaming.md` — RTMP egress, HLS/WebRTC watch, GoLivePanel (Wave 8–9)
- [x] `docs/economy-marketplace.md` — economy data model, revenue split, API reference (Wave 8–9)
- [x] WASM build pipeline guide (`docs/for-developers/build-pipeline.md`)
- [x] Self-hosting guide verified against current docker-compose (`docs/self-hosting/docker.md`)
- [ ] SDK API reference auto-generated from rustdoc
- [ ] Contributing guide updated for Rust-games + gaming-suite vision
