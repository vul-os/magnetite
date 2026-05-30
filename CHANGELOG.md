# Changelog

## [Unreleased] — Gaming Suite (Waves 6–9)

This entry covers the full gaming suite expansion on top of the completed
Waves 1–5 platform rebuild.

### Summary

Waves 6–9 extended Magnetite from a Rust game host into a **unified gaming suite**:
Discord-class communities, real-time text chat, WebRTC voice, streaming (go-live / watch),
controller / gamepad input, graphics tiers (Lite2D → Advanced3D), a platform-wide
points / XP economy, developer-run in-game marketplaces, FPS and motorsport starter
templates, and a full suite documentation refresh. All crates remain at 0 warnings;
frontend build, lint (0 errors), and tests (33/33) remain green.

---

### Wave 6 — Comms Core (Backend + SDK)

#### Backend
- New migration `20260530_communities.sql`: 11 tables —
  `communities`, `community_members`, `channels`, `channel_members`, `messages`,
  `dm_threads`, `dm_messages`, `presence`, `voice_rooms`, `voice_participants`, `streams`
- New API modules: `communities.rs`, `channels.rs`, `messages.rs`
  (communities CRUD, channel CRUD, message + DM thread endpoints)
- New services: `communities.rs` (community + channel + message business logic),
  `presence.rs` (upsert / offline sweep)
- `ws/comms.rs`: real-time chat + typing indicators + presence broadcast over
  per-channel `tokio::sync::broadcast` channels; supports JoinChannel / LeaveChannel /
  SendMessage / TypingStart / TypingStop / SetPresence / Ping frames
- `ws/voice.rs`: WebRTC SDP/ICE signaling relay for peer-to-peer voice;
  mesh architecture for small rooms (≤15); SFU (LiveKit / mediasoup) documented as
  the production scale path; supports Offer / Answer / IceCandidate / Mute / LeaveRoom frames
- **0 warnings**; `cargo fmt` clean; tests compile

#### SDK (`magnetite-sdk`)
- New `platform::comms` module: `CommsClient`, `CommsConfig`, `ChatMessage`, `VoiceSignal`,
  `PresenceStatus`, `PresenceUpdate`, `CommsEvent`, typed `ClientCommsMessage` /
  `ServerCommsMessage` enums — the in-game surface mirroring the WS protocol
- **101 tests pass, 0 warnings**

#### Frontend
- `src/api/client.js`: comms surface added (communities / channels / messages / presence / voice endpoints)
- New hooks: `useCommunities`, `useChannels`, `useMessages`, `usePresence`, `useVoice`,
  `useCommsSocket` (manages `RTCPeerConnection` mesh), `useVoiceClient`
- `CommsContext.jsx`: context provider mounted in `App.jsx`
- New components: `comms/ServerRail`, `comms/ChannelList`, `comms/MessageList`,
  `comms/MessageComposer`, `comms/VoicePanel`, `comms/MemberList`, `comms/PresenceDot`
- New page: `Communities.jsx` — Discord-like server rail / channel list / chat / voice panel / members
- New route: `/communities` (and nav link)
- Build green; lint **0 errors**; tests **33/33**

#### Docs
- `docs/comms/index.md` — comms overview: pillars, concept hierarchy, REST API surface
- `docs/comms/realtime.md` — WS chat/presence protocol + WebRTC voice signaling flow
- `docs/comms/data-model.md` — full schema for communities/channels/messages/voice_rooms/streams
- `docs/comms/in-game.md` — SDK `platform::comms` usage for lobby/match auto-provisioned rooms

---

### Wave 7 — Comms Frontend + In-Game Overlay + Streaming UI

#### Frontend
- `Communities.jsx` fully wired to live comms hooks: realtime chat, typing indicators, presence,
  load-more pagination, WebSocket connection status pill
- `CommsProvider` mounted at `App.jsx` root; all routes share the comms socket
- New page: `Messages.jsx` (`/messages`) — DM threads list + conversation + presence dots
- New page: `Streams.jsx` (`/streams`) — browse live streams grid; `StreamPlayer` (HLS/WebRTC watch);
  `GoLivePanel` (getDisplayMedia capture + RTMP key config)
- New streaming components: `streaming/StreamCard`, `streaming/StreamPlayer`, `streaming/GoLivePanel`
- `GameOverlay.jsx` — in-game chat + voice hotkey overlay rendered inside Playground / Lobby / Spectator
- `useVoiceClient` hook: `getUserMedia` + `RTCPeerConnection` mesh + Web Audio analyser for speaking ring;
  mute / deafen state managed client-side and synced to backend via Mute frame
- Presence dots added to Navbar, Friends list, ProfileCard
- Build green; lint **0 errors**; tests **33/33**

---

### Wave 8 — Game-Dev Capabilities + Economy + Marketplace

#### Backend
- New migration `20260531_economy.sql`: 8 tables —
  `seasons`, `point_balances`, `points_ledger`, `point_rewards`,
  `dev_stores`, `store_items`, `store_purchases`, `entitlements`;
  seed: Season 1 — Launch inserted on migration
- `backend/src/api/points.rs`: `GET /points/balance`, `POST /points/award` (admin/game),
  `POST /points/spend`, `GET /points/history`, `GET /points/leaderboard`,
  `POST /points/season-reset` (admin)
- `backend/src/api/marketplace.rs`: `GET /marketplace/stores/:game_id`,
  `POST /marketplace/stores` (developer), `PUT /marketplace/stores/:id`,
  `GET /marketplace/stores/:id/items`, `POST /marketplace/stores/:id/items`,
  `PUT /marketplace/stores/:id/items/:item_id`, `POST /marketplace/items/:item_id/purchase`,
  `GET /marketplace/entitlements`, `GET /marketplace/stores/:game_id/revenue`
- `backend/src/services/points.rs`: atomic ledger insert + balance update (single transaction),
  season reset (soft-wipes balances, creates new season), leaderboard query
- `backend/src/services/marketplace.rs`: store/item CRUD, purchase via USDC (70/30 split) or
  points (full debit to ledger), entitlement creation, revenue aggregation
- **0 warnings**; `cargo fmt` clean

#### SDK (`magnetite-sdk`)
- `input/gamepad.rs`: `GamepadState`, `GamepadButton`, `GamepadAxis`, `GamepadEvent`,
  `InputMap`, `GameAction` (Move, Jump, Dash, Shoot, Reload, Interact, Throttle, Brake, Steer,
  MenuConfirm, MenuBack, Pause), `InputBinding`, `InputSource` — unified gamepad + keyboard binding
- `graphics.rs`: `GraphicsTier` (Lite2D / Standard3D / Advanced3D), `RenderConfig`,
  `RenderConfigBuilder` (builder with `.tier()`, `.hdr()`, `.physics_substeps()`, `.shadows()`),
  `EngineCapability`
- `platform::points.rs`: `PointsClient`, `AwardPointsRequest`, `SpendPointsRequest`,
  `PointsBalance`, `LedgerEntry`, `LedgerEntryKind`, typed message enums
- `platform::marketplace.rs`: `MarketplaceClient`, `StoreItem`, `ItemType`,
  `PurchaseRequest`, `PurchaseResult`, `Entitlement`, `PaymentMethod`, typed message enums
- `platform::cloud_save.rs`: `CloudSaveClient`, `SaveSlot`, `SaveSlotMeta`, `SaveRequest`, typed enums
- **240 tests pass, 0 warnings**; `cargo fmt` clean

#### New crates
- `game-template-fps/` (`magnetite-fps-starter`): Bevy + rapier3d FPS starter;
  hitscan (`hitscan.rs`), level layout (`level.rs`), Bevy ECS plugin (`bevy_client.rs`),
  gamepad look/move/shoot (`input_map.rs`), Advanced3D tier;
  `cargo check --no-default-features` 0/0, **38 tests**
- `game-template-motorsport/` (`magnetite-game-motorsport`): Bevy + rapier3d vehicle physics;
  analog throttle/brake/steer via `GamepadAxis`, lap timing → points award via `platform::points`;
  `cargo check --no-default-features` 0/0, **26 tests**

#### Frontend
- New page: `Points.jsx` (`/points`) — player balance, ledger history, leaderboard, season info
- New page: `DevMarketplace.jsx` (`/developers/marketplace`) — store creation/edit, item CRUD,
  revenue overview (developer-only)
- New page: `ControllerSettings.jsx` (`/settings/controller`) — live Gamepad API display,
  button/axis binding editor, save bindings
- New component: `store/InGameStore` — in-game overlay panel listing purchasable items during play
- New hooks: `usePoints`, `useMarketplace`, `useGamepad`
- New routes: `/points`, `/developers/marketplace`, `/settings/controller` wired in `App.jsx`
- Build green; lint **0 errors**; tests **33/33**

---

### Wave 9 — Suite Docs Close-Out

#### Documentation
- `README.md` — complete rewrite: gaming suite features table (comms, controllers, graphics tiers,
  points economy, dev marketplaces, game templates), updated project structure (new crates + modules),
  updated API routes table (communities / channels / messages / points / marketplace / WS handlers),
  updated architecture diagram with new services and WS layer, updated documentation index
- `roadmap.md` — Phase 5 (Gaming Suite) added and marked COMPLETE; Phase 6 (scaling to large titles)
  and Phase 7 (distribution flywheel) added as future work; all completed items checked
- `TASKS.md` — checked off all Wave 6–9 items: 34 API modules, 22 services, 3 WS handlers,
  24 migrations, full SDK surface, new templates, new pages/components/hooks, new docs
- `docs/comms/streaming.md` — RTMP egress, HLS/WebRTC in-platform watch, GoLivePanel, scale path
- `docs/for-developers/controllers.md` — Gamepad API + gilrs, InputMap, GameAction bindings,
  ControllerSettings page, integration example
- `docs/for-developers/graphics-tiers.md` — Lite2D / Standard3D / Advanced3D tiers,
  RenderConfig builder, platform provisioning
- `docs/for-developers/points-economy.md` — ledger design, seasons, award/spend endpoints,
  `platform::points` SDK integration, game-template examples
- `docs/for-developers/marketplace.md` — dev store creation, item types, purchase flow,
  USDC vs points, entitlements, revenue split, SDK integration
- `docs/for-developers/fps-starter.md` — `game-template-fps` usage: clone, implement, cargo check,
  hitscan, gamepad, Advanced3D, publish
- `docs/for-developers/motorsport-starter.md` — `game-template-motorsport` usage: vehicle physics,
  analog input, lap → points, cargo check, publish
- `docs/economy-marketplace.md` — data model (seasons / ledger / stores / items / purchases /
  entitlements), revenue split (70/30 USDC, full points), API reference, SDK integration
- `docs/architecture.md` — updated: 34 API modules, 22 services, 3 WS handlers, 24 migrations,
  SDK platform modules, new data flow examples
- `docs/for-developers/index.md` — updated guide listing to include all new suite docs

---

## [Unreleased] — Autonomous Rebuild (Waves 1–5)

### Summary

A coordinated 5-wave rebuild hardened the backend to 0 warnings, wired the frontend to real
API endpoints, shipped an "Industrial Magnetite" design system across all 67 pages, and
delivered elevated typography, atmosphere, and per-route UX polish.

---

### Wave 1 — Foundation

#### Backend
- Upgraded `sqlx` 0.7.4 → 0.8.6; cleared all future-incompat warnings
- Applied `cargo fix` across all backend crates; result: **0 compiler warnings** (baseline: 341)
- `cargo fmt --check` passes; `cargo test --no-run` compiles clean
- Integration tests: auth, API, wallet — all pass

#### Frontend design system — "Industrial Magnetite"
- New `src/styles/tokens.css`: complete CSS custom-property token set
  - Colors: `--color-bg-primary` (#07070b) through accent (`--color-accent` #38e1c8 electric teal)
  - Typography: `--font-display` (Archivo), `--font-sans` (Hanken Grotesk), `--font-mono` (JetBrains Mono)
  - Motion: `--t-fast` (140 ms), `--t` (240 ms), `--t-slow` (420 ms)
  - Radius: `--radius-sm` (6 px), `--radius` (10 px), `--radius-lg` (16 px)
- `src/index.css` rewritten: light-theme overrides, global resets, shared utility classes
  (`.bg-atmosphere`, `.glow-accent`, `.kicker`, `.reveal-N` stagger system)
- All 17 `src/components/common/` components and `Navbar` + `Toast` restyled to new tokens
- `eslint.config.js`: excluded `**/target/**`, added vitest/node globals for test/e2e/config files;
  lint errors reduced from 712 → 46 (the 712 were rustdoc-generated JS in `backend/target/`)

#### Docs
- `README.md`, `roadmap.md`, `TASKS.md` rewritten to reflect actual codebase state
  and the Rust-games-at-any-scale vision (retired "HTML5 games" framing)

---

### Wave 2 — Page Restyles

- All 67 frontend pages restyled to Industrial Magnetite design system
- HTML5 copy pivoted to Rust vision across all marketing pages (Home, About, Careers, Pricing,
  Marketplace, DeveloperDashboard, Onboarding, FAQ, etc.)
- `vitest.config.js`: excluded `e2e/**` so Playwright specs are not run by Vitest
- Unit tests: PasswordInput strength test fixed; **all 33 unit tests pass**
- Lint: 46 → 18 errors (partitioned page agents cleared their files)
- 128 files changed in this wave

---

### Wave 3 — Wiring, SDK, WASM Pipeline, Docs

#### Frontend wiring (mock → real API with graceful fallback)
- `Marketplace`, `GameDetail` → `GET /api/games` / `GET /api/games/:id`
- `Wallet` → `GET /api/wallet/balance` + transaction history
- `Leaderboard` → `GET /api/leaderboard`
- `Achievements` → `GET /api/achievements`
- `Friends`, `Profile`, `DeveloperDashboard`, `Wishlist`, `Notifications` → corresponding API endpoints
- All pages retain mock-data fallback for graceful degradation when backend is unavailable
- Lint driven to **0 errors** (45 warnings — all experimental `react-hooks`, set to `warn` by design)

#### magnetite-sdk rewrite
- Versioned wire protocol (framed binary messages, protocol version negotiation)
- Netcode module: client-side prediction buffer, interest management, fixed-timestep tick loop
- **55 tests pass; 0 warnings**; `cargo fmt` clean

#### game-template
- Bevy client: `GamePlugin` with `handle_input_system` and `tick_system`
- `build.sh`: cargo → wasm-bindgen → wasm-opt pipeline
- `cargo check` passes (full Bevy WASM compile not run in CI — intentional; takes several minutes)

#### Backend distribution module
- New `backend/src/api/distribution.rs`: artifact registration, version management, play-manifest
  endpoint, build-webhook receiver
- Migration `20260530_game_distribution.sql`: `game_artifacts`, `game_versions`, `build_jobs` tables
- **0 warnings**, `cargo fmt` clean, tests compile

#### Docs (6 new files)
- `docs/for-developers/quickstart.md` — clone → implement → WASM build → publish workflow
- `docs/for-developers/build-pipeline.md` — CI/CD pipeline for WASM game builds
- `docs/for-developers/sdk.md` — `GameLogic` trait, `Input`, `GameState`, `Snapshot` API reference
- `docs/architecture.md` — backend modules, services, data flow, infrastructure
- `docs/security/index.md` — threat model, sandboxing, auth, anti-cheat layers
- `docs/self-hosting/` — expanded to 8 guides (docker, fly-io, environment-variables, database,
  monitoring, ssl, updating, troubleshooting)

---

### Wave 4 — UI/UX Polish

#### Typography upgrade
- Google Fonts loaded in `index.html`: Archivo (variable, 400–800), Hanken Grotesk (300–700),
  JetBrains Mono (400–600)
- All remaining `Inter` references removed from CSS; `--font-display` / `--font-sans` / `--font-mono`
  tokens applied consistently across all pages and components

#### Atmosphere & depth
- Grain overlay + faint magnetic grid on dark surfaces — implemented as shared utility classes
- Layered radial accent glows behind hero and CTA sections
- Dramatic multi-stop card shadows on elevated surfaces

#### Per-route UX polish
- **Home / HeroSection**: showstopper layered glows, field-line backdrop, Rust terminal card,
  orchestrated stagger reveals (`reveal-1` through `reveal-6`)
- **Marketplace**: cinematic header with dual glows, sticky buy-bar on GameDetail, category pills
- **Auth (Login / Register)**: split-panel layout — rich pitch/stats left panel, clean form right panel;
  headings "Welcome back" / "Join Magnetite"; buttons "Sign In" / "Create Account"
- **DeveloperDashboard**: polished stats grid, chart sections, earnings timeline
- **Wallet / Subscription / Pricing**: transaction list, tier cards, usage meters
- **Leaderboard / Achievements**: rank rows, progress rings, trophy cards
- **Admin / Settings / Profile**: sidebar nav, section layouts, form fields
- **Error pages (404 / 403 / 500)**: themed with magnetic motifs, clear recovery CTAs
- All pages: `prefers-reduced-motion` honored; visible focus rings; `aria-label` on interactive elements

#### Test fixes (auth copy change)
- `Login.test.jsx` / `Register.test.jsx` updated to new accessible names
- **33/33 unit tests pass**; lint **0 errors**; build clean

---

### Wave 5 — Close-out (E2E, Performance, Design Polish)

#### Performance
- Vite `manualChunks` code-split: index bundle 344 → **101 kB**; DeveloperDashboard chunk **10 kB**;
  recharts only loaded on `/developers`; react/router/recharts split into cached vendor chunks

#### E2E specs — aligned with redesigned UI
- `e2e/auth.spec.js`: replaced stale `data-testid` selectors; added Register suite; OAuth buttons via aria-labels
- `e2e/marketplace.spec.js`: heading asserts "Discover Rust Games"; category-pills nav check; search visibility
- `e2e/navigation.spec.js`: logo visibility, marketplace navigation, hero heading checks
- `e2e/page-objects/`: login, marketplace, navigation — all selectors coherent with redesigned UI

#### CSS consolidation
- Removed duplicate CSS contract between `tokens.css` and `index.css`
- Navbar + Footer received final Industrial Magnetite polish (mono nav, magnetic hover, atmospheric footer)

#### Documentation
- `TASKS.md`: checked off all items completed across Waves 1–4
- `roadmap.md`: Phase 2 marked COMPLETE; Phases 3 and 4 updated with completed items
- `CHANGELOG.md`: this entry

---

## [0.1.0] - 2025-01-19

### Added
- Initial release
- Platform API with auth, wallet, games, matchmaking endpoints
- React frontend with marketplace, wallet, developer dashboard
- PostgreSQL database schema
- Migration system with reset/up commands
- WebSocket support for game connections
