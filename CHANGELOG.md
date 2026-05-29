# Changelog

## [Unreleased] — Autonomous Rebuild (Waves 1–4)

This entry covers the complete autonomous multi-wave rebuild of the Magnetite platform.
All changes are on the `feat/redesign-and-harden` branch.

### Summary

A coordinated 4-wave rebuild hardened the backend to 0 warnings, wired the frontend to real
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
- Lint: 46 → 18 errors (partitioned page agents cleared their files); 18 residual in
  non-partition files (utils/contexts/hooks) — cleared in subsequent cleanup
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
- **Skeleton loaders**: shimmer animation, correct shape for each entity type
- All pages: `prefers-reduced-motion` honored; visible focus rings; `aria-label` on interactive elements

#### Test fixes (auth copy change)
- `Login.test.jsx` / `Register.test.jsx` updated to new accessible names:
  `getByRole('heading', { name: /welcome back/i })`, `getByRole('button', { name: /sign in/i })`,
  `getByRole('heading', { name: /join magnetite/i })`, `getByRole('button', { name: /create account/i })`
- **33/33 unit tests pass**; lint **0 errors**; build clean

---

### Wave 5 — Close-out (Docs, E2E, CHANGELOG)

#### E2E specs — aligned with redesigned UI
- `e2e/auth.spec.js`: replaced stale `data-testid` selectors with class/role selectors that match
  the split-panel auth layout; added Register suite asserting "Join Magnetite" heading and
  "Create Account" button; OAuth buttons tested via `aria-label="Continue with <Provider>"`
- `e2e/marketplace.spec.js`: heading asserts "Discover Rust Games"; added category-pills nav check
  (`nav[aria-label="Game categories"]`); added search-input visibility test
- `e2e/navigation.spec.js`: added logo visibility, marketplace link navigation, hero heading checks
- `e2e/page-objects/login.page.js`: replaced `data-testid` assumptions with `.auth-submit-btn`,
  `.auth-error[role="alert"]`, `button.oauth-btn[aria-label*="<Provider>"]`
- `e2e/page-objects/marketplace.page.js`: updated heading selector to `h1#marketplace-heading`,
  added `selectCategory()` helper, clarified search selector
- `e2e/page-objects/navigation.page.js`: updated `clickNavbarLink`/`clickFooterLink` to scoped
  `nav.navbar` / `footer.footer` selectors; documented layout in JSDoc

#### Documentation
- `TASKS.md`: checked off all items completed across Waves 1–4 (design system, all 67 pages,
  API wiring, SDK rewrite, distribution module, docs expansion, e2e alignment)
- `roadmap.md`: Phase 2 marked COMPLETE; Phases 3 and 4 updated with completed items checked
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
