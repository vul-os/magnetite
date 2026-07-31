# Changelog

## [Unreleased] — Brand, docs restructure, Stellar rail landed

### Added

- **`magnetite-stellar-rail`** (backlog A12): a real, standalone native-USDC-on-
  Stellar `PaymentRail` — the ten fail-closed verification checks ported from
  `magnetite-solana-rail`, a compiled-in stewards address
  (`MAGNETITE_STEWARDS_WALLET_STELLAR`) with the same devnet-only-override
  pattern, and `DUST_FLOOR_STROOPS`. No dependency, path or git, on the
  sibling `patala` repo. **Not wired into `backend`'s `PAYMENT_RAIL` selector
  yet** (blocked on a `backend/` territory conflict, see
  `docs/cross-repo-backlog.md` A12) and has never settled a payment itself —
  the one verified Stellar testnet settlement to date belongs to the sibling
  `patala-stellar` crate, not this one. See `docs/payments.md`.
- **A26 — tiered settlement wired into the Stellar rail.** A Horizon miss or
  RPC failure now degrades to `Settlement::SignedUnsettled` rather than a hard
  refusal; a chain-confirmed failure still refuses outright.
  `magnetite-web-host` keeps a conservative refuse-by-default policy for the
  unsettled tier by deliberate choice, documented in `entitlement.rs`.
- **Full PNG icon set rendered from `brand/logo.svg`** and wired into the
  site/app manifests, favicons and docs assets — no icon in this repo is
  hand-drawn or re-derived from anything but the one approved source file.
- **Research notes**: `docs/a25-storefront-tract-assessment.md` (A25 — the
  TRACT-shaped storefront gap; recommendation is a disclosure, not a
  migration), `docs/folder-transport-assessment.md` (A23 — FlowStock's
  folder-transport pattern), `docs/zero-third-party-path.md` (A22 — what
  needs nobody else, and where a third party is genuinely unavoidable),
  `docs/stellar-history-retention.md` (A26 — what degrades if Horizon prunes
  history, and why it fails closed), and a node-deploy recipe distinct from
  the legacy backend's (A21, `97fa148`/`9f00917`).
- **`site/docs.html` nav rebuilt user-first** from a single `DOCS`
  source-of-truth array (slug/title/group), promoting 16 chapters that
  previously existed only under `docs/` (self-hosting, troubleshooting,
  notification preferences, content rating, moderation, blocking,
  economy/marketplace, security, API reference, the moat, cross-repo
  backlog, roadmap, decentralization) into the interactive site.
- Three hand-authored inline-SVG illustrations added to the landing page.

### Changed

- **README.md restructured to the VulOS product-repo standard**: added the
  "Part of VulOS" banner and a dedicated section, renamed `## Overview` to
  `## What is magnetite?`, reordered Contributing ahead of License, and added
  a `## Configuration` section distinguishing the CLI-flag-only node binary
  from the legacy `.env`-driven backend/frontend. Its "What is magnetite?"
  opening now leads with the reproducibility claim this product is actually
  sharpest about — `verify_replay` and its tamper-detection test — cited by
  file:line rather than asserted.
- Site meta tags, manifest, robots.txt and sitemap added/corrected; a stale
  app-manifest claim fixed; the OG card fixed (the description was being
  clipped mid-sentence); label crowding and sub-12px type fixed across the
  landing and docs after actually rendering and measuring the pages, not just
  reading the source.
- `magnetite-seams`'s `PaymentSplit` moved from a fixed
  `{developer, operator, protocol_fee_bps}` shape to `Vec<Leg>` (A13/A14) —
  co-developers, publishers, operator shares and the voluntary stewards
  contribution are now the same code path and the same sum-exact arithmetic,
  with a per-rail dust floor (skipped, never fatal) rather than a seam-wide
  constant.
- `magnetite-seams`: Identity verification now follows the live provider
  (A7) instead of a hard-coded check; a rotated key keeps the same identity
  (A8); `BlobStore::get_range` added so range reads are expressible (A9, not
  yet wired into the tamper-checked serving path — deliberately, see
  `magnetite-web-host`'s A9 commit); adopted evermesh's EM-1 chunk-tree
  primitive with no dependency (A24).

### Fixed

- **A live 10,000× scale bug in payment-analytics reporting (B5)** — and the
  test that would have hidden it, in the same pattern this suite has now
  found more than once: a test asserting the wrong unit rather than the
  right one.
- `docs/self-hosting/index.md`, `run-it-all.md`, `local-infra.md` still
  cloned `github.com/magnetite-platform/magnetite` — the wrong org, missed
  when `site/docs/self-hosting.md` was corrected to `vul-os` earlier.
  `CONTRIBUTING.md` had the same defect twice.
- **README.md documented `cargo build/test/clippy --workspace` and
  `cargo run --package … `, none of which run** — there is no root
  `Cargo.toml` (14 standalone crates, by design; see the comment atop
  `magnetite-seams/Cargo.toml`), confirmed by running each command against a
  clean checkout. Replaced with the per-crate commands `.github/workflows/
  ci.yml` actually uses.
- **`docs/self-hosting/quickstart.md`, `docker.md`, `updating.md`** referenced
  container images (`magnetite/app`, `magnetite/backend:latest`,
  `magnetite/api`, `magnetite/game-host`) that are not published anywhere —
  no workflow in this repo pushes to Docker Hub or GHCR; `deploy.yml` deploys
  straight to Fly.io. Rewrote the compose examples to build from the tracked
  `Dockerfile.backend`/`Dockerfile.frontend`, matching the real
  `docker-compose.yml`; added the same gap as an explicit caveat in
  `deploy.md`'s k8s/Nomad image table.
- **Fabricated commands removed**: `docker-compose exec backend migrate` /
  `create-admin` (the runtime image contains only the compiled binary —
  migrations run automatically via `sqlx::migrate!` at startup, and there is
  no admin-creation CLI); `run-it-all.md`'s `sqlx migrate run`,
  `/api/v1/admin/seed`, and `magnetite register --repo …` (the real CLI has
  no `register`/`token create` subcommand — replaced with the actual
  `POST /api/v1/developer/games/scaffold` flow).
- `docs/payments.md` predated `magnetite-stellar-rail` — added a full section
  with the same "read this before promising anything" honesty as the
  existing Solana section, and corrected the two-rail table to three,
  including the fact that the Stellar rail is not yet reachable from
  `PAYMENT_RAIL` at all.
- `docs/self-hosting/deploy.md` had two `hosting-a-server.md` links missing
  the `../` needed to resolve from `docs/self-hosting/` — found by a
  coverage-counted link checker (346 relative links/images across 83 files).
- `docs/index.md` was missing 16 real, current chapters with no inbound link
  from anywhere in the tree (7 of 8 `docs/moat/*.md` files, and 14 standalone
  feature/assessment pages) — added three new index sections linking all of
  them. `docs/HANDOVER.md`, a one-time session note whose own claims were
  already overtaken same-day by later commits, marked historical rather than
  left to be mistaken for current status.
- `ci/rust-crates.json` now the enforced single source of truth for which
  Rust crates CI gates — `scripts/ci-crate-coverage.sh` fails the build if a
  crate on disk is missing from it, closing the hole where 13 of 14 crates
  compiled, tested and linted nowhere until this wave.
- `app-screenshots.mjs` no longer rewrites its own tracked subject when run.
- Local `.env` files had no `.gitignore` rule at all in this repo; added.

### Security

- Fly.io deploy workflow reviewed; `deploy/k8s`/`deploy/nomad` manifests'
  plaintext `ws://` shipped as-is is now a loud, documented gap rather than a
  silent one (`b647328`), and a verified `wss://` reverse-proxy recipe for
  the standalone node is documented (A19, `97fa148`).

## [Unreleased] — Repository structure

### Security

- **Releases carry a sigstore build-provenance attestation.**
  `actions/attest-build-provenance` signs every staged asset — including
  `SHA256SUMS` itself, so the attestation on the manifest transitively covers
  every asset it names — with a short-lived certificate minted from the release
  job's OIDC token. No long-lived key, no repository secret, nothing to rotate.
  `SHA256SUMS` alone proves only internal consistency, since whoever can serve
  you the assets can serve you a matching manifest; the attestation is what
  binds the bytes to this repository's release workflow. It is **not** OS
  code-signing, and it is not load-bearing for integrity.

- **Added `scripts/verify.sh`** — the fail-closed check a *user* runs before
  executing downloaded bytes, as opposed to `release-checksums.sh`, which is
  the producer's own re-derivation. Distinct exit code and diagnostic per
  failure mode (missing manifest 3, HTML page served as the manifest 4,
  empty/malformed manifest 5, no entry 6, unfetchable artifact 7, truncated
  download 8, digest mismatch 9, missing tool 10, failed attestation 11,
  plaintext origin 12); no skip flag and no path where an absent `SHA256SUMS`
  means "nothing to check". `--selftest` runs 24 synthetic-origin cases
  asserting exit code and that a diagnostic was printed; CI runs it on every
  push, and the release job runs it plus a `--dir` verification of its own
  output before publishing.

### Fixed

- **`release-checksums.sh emit` could write a short manifest and exit 0.**
  A digest tool that fails for one file while exiting 0 (an unreadable asset, a
  full disk) produced a manifest covering fewer assets than the release ships;
  only `verify` caught it, and only as the differently-worded "present but not
  listed". `emit` now asserts one manifest line per asset at the point of
  production.

- **`release-checksums.sh verify` skipped the last manifest entry when the
  manifest had no trailing newline.** `while read` returns non-zero with the
  final line already in the variables, so the digest loop dropped it; the build
  still failed via the present-but-unlisted cross-check, pointing the reader at
  the wrong bug. The loop now ends in `|| [ -n "$want_digest" ]`, and the
  summary line counts records with `awk NR` instead of `wc -l`, which would
  otherwise report one asset fewer than it just checked.


### Changed
- **Game templates consolidated.** The four root-level template crates moved
  under a single `game-templates/` directory, and their directory names now
  match the template-catalog `id` values served by `GET /api/v1/templates`:
  - `game-template/` → `game-templates/arcade/`
  - `game-template-authoritative/` → `game-templates/authoritative/`
  - `game-template-fps/` → `game-templates/fps/`
  - `game-template-motorsport/` → `game-templates/motorsport/`

  Cargo **package names are unchanged** (`magnetite-game-template`,
  `game-template-authoritative`, `magnetite-fps-starter`,
  `magnetite-game-motorsport`) — only paths moved, so `cargo` dependency
  entries that refer to crates by name keep working. Path dependencies
  (`game-client-bevy`, `magnetite-e2e`, and each template's `magnetite-sdk`
  dep) were re-pointed.

  The `template_path` / `template_repo` fields in the templates API now report
  the new locations — **API response values changed**.

- **Root markdown folded into `docs/`.** The repository root now carries only
  the files tooling and GitHub expect (`README.md`, `CONTRIBUTING.md`,
  `CHANGELOG.md`, `LICENSE`) plus `DECENTRALIZATION.md`, which stays at the
  root because it is the active anchor spec referenced by path from outside
  this repository. `AUDIT.md`, `DECISIONS.md`, `GAPS.md`, `TASKS.md`,
  `roadmap.md`, and `DECENTRALIZATION_PROGRESS.md` moved to `docs/project/`
  with a new [`docs/project/index.md`](docs/project/index.md) index. Documents
  that are superseded or predate the decentralization redesign now carry a
  dated header saying so; none were deleted.

  **Note for agents:** the decentralization progress log is now at
  `docs/project/DECENTRALIZATION_PROGRESS.md`.

---

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
