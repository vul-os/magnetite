# Magnetite Documentation

**Magnetite** is the open-source platform for building, distributing, and monetizing Rust games —
that scale from a weekend game jam to a COD-size AAA title.

Game logic is authored in Rust. Clients compile Bevy to WASM (browser) and to native. The platform
is server-authoritative and sandboxed, providing hosting, matchmaking, real-time netcode, persistence,
comms, economy, and payment rails — so developers only write game logic.

---

## Quick Links

| Section | Description |
|---------|-------------|
| [Developer Quickstart](./for-developers/quickstart.md) | Clone template → implement → build WASM → publish |
| [SDK Reference](./for-developers/sdk.md) | `magnetite-sdk` crate reference |
| [Build & Distribution Pipeline](./for-developers/build-pipeline.md) | How games go from source to players |
| [Architecture Overview](./architecture.md) | Backend modules, services, and data flow |
| [MOAT Architecture](./MOAT-ARCHITECTURE.md) | Frozen interface contracts: AuthoritativeGame, Topology, ReplayLog |
| [Replay & Spectator](./moat/replay-spectator.md) | ReplayLog, verify_replay, web-client protocol, tournament system |
| [Comms Suite](./comms/index.md) | Communities, channels, chat, presence, voice, streaming |
| [Self-Hosting Guide](./self-hosting/index.md) | Docker Compose, Fly.io, k8s, Nomad deployments |
| [Security & Sandboxing](./security/index.md) | Auth, anti-cheat, and deployment hardening |
| [API Reference](./api-reference/index.md) | REST API endpoints |

---

## What Magnetite Provides

| Concern | What the platform handles |
|---------|--------------------------|
| **Distribution** | Storefront/marketplace; players discover, play (browser WASM or native), and pay |
| **Hosting** | Server-authoritative Rust game servers; WASM artifacts served to browsers |
| **Matchmaking** | Queue join/leave/status; player pairing by skill and region |
| **Real-time netcode** | WebSocket state-sync (`ClientNet`/`ServerNet`); client SDK connection types; prediction + delta compression |
| **Persistence** | Leaderboards (Redis + Postgres), achievements, session history, replays |
| **Payments** | Paystack fiat on-ramp (deposits + subscriptions); Wise developer payouts; 70/30 split (USD) |
| **Social** | Friends, invites, pending requests, block/unblock, activity feed, notifications (real-time WS) |
| **Comms** | Communities (servers/guilds), text channels, real-time chat, presence, voice rooms (WebRTC), streaming (MediaMTX HLS + RTMP egress) |
| **Analytics** | Developer dashboard, revenue breakdown, daily revenue chart, session stats |
| **i18n** | EN, ES, FR locales; browser-language auto-detection; `I18nProvider` + `useTranslation` hook |
| **PWA / mobile** | Installable progressive web app; responsive at 360/768/1280; bottom navigation on mobile |
| **Orchestration** | Docker Compose (local), Fly.io (Firecracker), Kubernetes (`deploy/k8s/`), Nomad (`deploy/nomad/`) |

---

## The MOAT

Three interlocking differentiators ship as real, tested Rust crates:

| Differentiator | Crates | Status |
|---------------|--------|--------|
| **Scale primitive** | `magnetite-sdk` (AuthoritativeGame, Topology, MatchConfig), `magnetite-runtime` (TickScheduler, ShardManager, magnetite-serve binary) | Proved: 9 e2e tests passing; scale bench 50k–203k ticks/sec (debug) |
| **Wasmtime sandbox** | `magnetite-sandbox` (WasmExecutor, LimitsConfig, WASI stubs) | Proved: `wasm_sandbox_parity_with_native` — identical `state_hash` vs `NativeExecutor` over 30 ticks |
| **Anti-cheat by construction** | `magnetite-anticheat` (validators, TrustScoreMap, ReplayVerifier) | Proved: speedhack rejected + trust score escalated; `verify_replay` → `Clean` |
| **One-command pipeline** | `magnetite-cli` (magnetite new\|build\|dev\|deploy) | Implemented; `scripts/moat-demo.sh` runs the full pipeline |
| **JS web client** | `magnetite-web-client` (ClientNet/ServerNet, prediction, replay player) | Implemented; full-stack WS test proves Delta frames flow end-to-end |

---

## Repository Structure

```
magnetite/
├── backend/                        # Rust platform backend (Axum 0.8, SQLx 0.8.6)
│   ├── src/api/                    # 38 HTTP route modules
│   ├── src/services/               # 26 business-logic modules
│   ├── src/middleware/             # CORS, rate limiting, request logging
│   ├── src/jobs/                   # Background jobs (session cleanup, notification GC, backup, payouts)
│   ├── src/ws/                     # WebSocket handlers (comms, voice, game)
│   ├── magnetite-sdk/              # Rust SDK — AuthoritativeGame, GameLogic, Input, State, Networking
│   ├── migrations/                 # 41 SQL migration files
│   └── tests/                      # Integration tests
├── magnetite-runtime/              # Authoritative server host; magnetite-serve binary; Dockerfile
├── magnetite-sandbox/              # WasmExecutor (Wasmtime); fuel/memory/epoch limits
├── magnetite-anticheat/            # Validators, TrustScoreMap, ReplayVerifier
├── magnetite-cli/                  # magnetite new|build|dev|deploy binary (clap 4)
├── magnetite-web-client/           # JS web client (ClientNet/ServerNet); prediction; replay player
├── magnetite-e2e/                  # Integration + full-stack + scale bench tests (9 passing)
├── game-template/                  # 2D arcade starter (Bevy + GameLogic + WASM-ready)
├── game-template-authoritative/    # Arena shooter reference (AuthoritativeGame + wasm ABI)
├── game-template-fps/              # FPS starter (Bevy + rapier3d; hitscan; gamepad)
├── game-template-motorsport/       # Motorsport starter (Bevy + rapier3d; lap → points)
├── game-client-bevy/               # Bevy client (prediction/reconciliation wired to ServerNet)
├── src/                            # React 19 frontend (97 pages, 41 hooks)
│   ├── i18n/                       # I18nProvider, useTranslation, en/es/fr JSON locales
│   └── …
├── deploy/
│   ├── k8s/                        # 11 Kubernetes manifests (namespace → ingress + HPA)
│   └── nomad/                      # 6 Nomad job files
├── config/
│   └── mediamtx.yml                # MediaMTX HLS + RTMP egress config
├── .github/workflows/              # CI, deploy, game-ci, game-deploy, release
├── docker-compose.yml              # Full local stack incl. MediaMTX
├── Dockerfile.backend / Dockerfile.frontend / Dockerfile.fly
└── docs/                           # This documentation tree
    ├── index.md                    # This file — doc hub
    ├── architecture.md             # Backend modules, services, data flow
    ├── MOAT-ARCHITECTURE.md        # Frozen interface contracts
    ├── moat/                       # MOAT detail docs
    │   ├── replay-spectator.md     # ReplayLog, verify_replay, web-client, tournaments
    │   ├── scaling.md              # Topology + bench numbers
    │   ├── scale-report.md
    │   └── …
    ├── for-developers/             # Developer-facing guides
    │   ├── quickstart.md
    │   ├── sdk.md
    │   ├── build-pipeline.md
    │   ├── controllers.md
    │   ├── graphics-tiers.md
    │   ├── points-economy.md
    │   ├── marketplace.md
    │   ├── fps-starter.md
    │   ├── motorsport-starter.md
    │   └── submission.md
    ├── comms/                      # Comms suite docs
    │   ├── index.md
    │   ├── realtime.md
    │   ├── data-model.md
    │   ├── in-game.md
    │   └── streaming.md
    ├── self-hosting/               # Ops and deploy guides
    │   ├── index.md
    │   ├── quickstart.md
    │   ├── docker.md
    │   ├── fly-io.md
    │   ├── deploy.md
    │   ├── environment-variables.md
    │   ├── external-dependencies.md
    │   ├── local-infra.md
    │   ├── streaming.md
    │   ├── monitoring.md
    │   ├── ssl.md
    │   ├── database.md
    │   ├── run-it-all.md
    │   ├── troubleshooting.md
    │   └── updating.md
    ├── api-reference/              # REST API reference
    │   ├── index.md
    │   └── auth.md
    ├── security/                   # Security + sandboxing
    │   └── index.md
    ├── economy-marketplace.md
    ├── subscriptions-lifecycle.md
    ├── moderation.md
    ├── analytics.md
    ├── i18n.md
    ├── notification-preferences.md
    ├── blocking.md
    ├── content-rating.md
    ├── refunds.md
    ├── search.md
    ├── wise-iban.md
    └── requirements.md
```

---

## License

Platform and SDK are MIT licensed. Documentation is CC0.
