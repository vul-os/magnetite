# Magnetite Documentation

**Magnetite is a decentralized platform for Rust games.** A game is a
content-addressed portable object. A node is generic compute that fills its own
hardware. Discovery is a phonebook, not an authority. Payments are
non-custodial. Chat, voice, video, and streaming are pluggable integrations
Magnetite does not build.

Game logic is authored in Rust and runs server-authoritative inside a WASM
sandbox. `magnetite dev` runs a game with zero backend — no cloud account, no
database, no payment provider.

> **Read the new chapters first.** The chapters listed under *Start here* are
> written against the shipped decentralized system. The older reference pages
> below them predate that redesign; they have been corrected, but they still
> describe the platform in its "one big backend" shape.

---

## Start here

| Chapter | Description |
|---------|-------------|
| [Overview](./overview.md) | What Magnetite is, and what it deliberately is not |
| [Getting started](./getting-started.md) | Install the CLI, scaffold a game, run it with zero backend |
| [Architecture](./architecture.md) | The six seams and how they compose |
| [Hosting a server](./hosting-a-server.md) | Capacity-elastic nodes, shards, discovery |
| [The zero-third-party path](./zero-third-party-path.md) | What needs nobody else — discovery, reachability, packages, payments — and where a third party is genuinely unavoidable |
| [Payments](./payments.md) | Non-custodial checkout, signed receipts, hosting fees |
| [Comms](./comms.md) | Pluggable chat/voice/video/streaming providers |
| [Screenshots](./screenshots.md) | Landing, docs, and app gallery |

## Reference

| Section | Description |
|---------|-------------|
| [Developer Quickstart](./for-developers/quickstart.md) | Clone template → implement → build WASM → publish |
| [SDK Reference](./for-developers/sdk.md) | `magnetite-sdk` crate reference |
| [Build & Distribution Pipeline](./for-developers/build-pipeline.md) | How games go from source to players |
| [MOAT Architecture](./MOAT-ARCHITECTURE.md) | Frozen interface contracts: AuthoritativeGame, Topology, ReplayLog |
| [Replay & Spectator](./moat/replay-spectator.md) | ReplayLog, verify_replay, web-client protocol, tournaments |
| [Comms Suite (builtin provider)](./comms/index.md) | The in-house chat/presence/voice/streaming stack, now one adapter among several |
| [Self-Hosting Guide](./self-hosting/index.md) | Docker Compose, Fly.io, k8s, Nomad deployments |
| [Security & Sandboxing](./security/index.md) | Auth, sandboxing, anti-cheat, signature verification |
| [Project History](./project/index.md) | Progress log, recorded decisions, audits, roadmap, task lists |
| [API Reference](./api-reference/index.md) | REST API endpoints |

---

## Platform feature docs

Each documents one backend surface. None of these needs its own hub page —
they're linked here directly so every chapter in the tree is reachable from
this index.

| Chapter | Description |
|---------|-------------|
| [Analytics](./analytics.md) | Developer dashboard, per-game player/session/revenue series |
| [Blocking](./blocking.md) | User block/unblock system |
| [Content Rating](./content-rating.md) | Content rating enum + scaffold flow |
| [Economy & Marketplace](./economy-marketplace.md) | Points economy + developer marketplace, non-custodial |
| [i18n](./i18n.md) | `I18nProvider` / `useTranslation`, en/es/fr locales |
| [Moderation](./moderation.md) | Review-report moderation pipeline |
| [Notification Preferences](./notification-preferences.md) | Per-user category/channel preferences |
| [Refunds](./refunds.md) | A refund is a void of a signed receipt — non-custodial, no chargeback |
| [Requirements](./requirements.md) | Server hardware minimums; capacity is emergent, not tiered |
| [Search](./search.md) | Full-text search endpoint |
| [Stellar History Retention](./stellar-history-retention.md) | A26: what degrades if Horizon prunes chain history, and why it fails closed |
| [Subscriptions (removed)](./subscriptions-lifecycle.md) | Documents that the subscriptions REST API/UI were removed; the data model is dormant |
| [Troubleshooting](./troubleshooting.md) | Deployment/DB/OAuth/payment issues, platform-wide |
| [Color Palette](./color-palette.md) | Brand color swatches |

## MOAT deep dives

[`MOAT-ARCHITECTURE.md`](./MOAT-ARCHITECTURE.md) is canonical; these are
readable guides into specific parts of it.

| Chapter | Description |
|---------|-------------|
| [Architecture Overview](./moat/architecture-overview.md) | Readable crate-map guide (defers to MOAT-ARCHITECTURE.md for the frozen interfaces) |
| [Quickstart](./moat/quickstart.md) | 5-minute CLI path: new → build → dev → deploy |
| [Develop in the browser](./moat/develop-in-browser.md) | Studio → wasm → dev → web-client, no local toolchain |
| [Develop in the browser — Studio UI](./moat/develop-in-browser-studio.md) | The in-browser IDE (`/developers/studio`) |
| [Play in the browser](./moat/play-in-browser.md) | Web client play path, `ClientNet`/`ServerNet` protocol |
| [Replay & Spectator](./moat/replay-spectator.md) | `ReplayLog` recording/verification, replay playback, tournaments |
| [Scaling design](./moat/scaling.md) | The `SingleRoom`/`Dedicated`/`Sharded` topology ladder |
| [Scale report](./moat/scale-report.md) | `magnetite-e2e` test suite + bench numbers backing the ladder |

## Program history & assessments

| Chapter | Description |
|---------|-------------|
| [Cross-repo backlog](./cross-repo-backlog.md) | The live A1–A26 backlog spanning magnetite/patala/kotva/evermesh/vuna/wibbly — start here for "what's actually in progress" |
| [Handover, 2026-07-30](./HANDOVER.md) | **Historical** — a one-time session handover, superseded by the backlog above |
| [Walrus assessment](./walrus-assessment.md) | Research note, dated: `BlobStore` backend candidate assessment |
| [Sui binding spike](./sui-binding-spike.md) | Research note, dated: item-binding spike, no code changed |
| [Folder-transport assessment](./folder-transport-assessment.md) | Research note: FlowStock's folder-transport pattern, applicability to magnetite |
| [A25 storefront/TRACT assessment](./a25-storefront-tract-assessment.md) | Research note, dated: TRACT-shaped storefront gap — a map, not a build |
| [Chain candidates](./chain-candidates.md) | Research note: 16+11-chain sweep for offline-signing/payment chain selection |

---

## What Magnetite Provides

| Concern | What the platform handles |
|---------|--------------------------|
| **Game runtime** | Server-authoritative deterministic simulation (`AuthoritativeGame`); WASM sandbox (Wasmtime) with fuel/memory/epoch limits |
| **Content addressing** | A game is identified by the BLAKE3 hash of its module. `load_verified_game` re-hashes before executing and fails closed on mismatch |
| **Capacity-elastic hosting** | One `magnetite` node binary measures its own cores/RAM/bandwidth; shard count and player cap are **emergent from the hardware**, never config constants |
| **Discovery** | Nodes self-advertise signed `SessionAd`s to a swappable HTTP tracker (`/api/v1/discovery/*`) or over mDNS on the LAN. The tracker is a phonebook — it certifies nothing |
| **Payments** | Non-custodial. `PaymentRail::checkout` produces a signature-verified `Receipt`; the receipt **is** the entitlement. Developer takes the whole subtotal; `PROTOCOL_FEE_BPS` defaults to 0. Default rail is `MockPaymentRail` (offline). **A real chain rail is `TODO(chain)` and is not built** |
| **Comms** | `CommsProvider` seam. `builtin` (default, offline) plus config-gated Matrix / Jitsi / LiveKit / Owncast adapters. Magnetite builds no chat, voice, or streaming service |
| **Anti-cheat** | Deterministic `ReplayLog` + `verify_replay` — anyone can re-simulate a match to prove tampering |
| **Matchmaking** | Queue join/leave/status; player pairing by skill and region |
| **Real-time netcode** | WebSocket state-sync (`ClientNet`/`ServerNet`); prediction + delta compression |
| **Persistence** | Leaderboards (Redis + Postgres), achievements, session history, replays |
| **Social** | Friends, invites, block/unblock, activity feed, notifications (real-time WS) |
| **Analytics** | Developer dashboard, per-game player/session/revenue series |
| **i18n** | EN, ES, FR locales; browser-language auto-detection; `I18nProvider` + `useTranslation` |
| **PWA / mobile** | Installable progressive web app; responsive at 360/768/1280 |
| **Orchestration** | Docker Compose (local), Fly.io, Kubernetes (`deploy/k8s/`), Nomad (`deploy/nomad/`) |

### What was removed

Paystack, Wise, custodial wallet balances, deposits, withdrawals, payouts, the
70/30 revenue split, and ZAR→USD conversion were **deleted from the codebase**.
There is no fiat on-ramp and nothing for the platform to hold or pay out. See
[Payments](./payments.md).

### What is honestly not done

- **A real chain rail.** `MockPaymentRail` signs receipts locally. `CHAIN_RPC_URL`, `CHAIN_ID`, and `STABLECOIN_ADDRESS` are placeholders.
- **Cross-node shard handoff over the network.** The `HandoffTransport` seam and the loopback transport are real and tested; `NetworkHandoffTransport` fails closed with a documented TODO.
- **Multi-tracker gossip.** A client queries the trackers it is configured with; there is no discovery-of-trackers.
- **Automatic cluster rebalancing from the CLI.** `magnetite node` can now join a cluster (`--cluster-peer`, `--handoff-addr`) and its node keypair is persisted (`~/.magnetite/node.key`), but driving migrations still means calling the scheduler/transport from code.

---

## The MOAT

| Differentiator | Crates | Status |
|---------------|--------|--------|
| **Scale primitive** | `backend/magnetite-sdk` (AuthoritativeGame, Topology, MatchConfig, `scaling` feature), `magnetite-runtime` (TickScheduler, ShardManager, capacity measurement) | Proved: multi-shard determinism on one box; scale bench in `magnetite-e2e` |
| **Wasmtime sandbox** | `magnetite-sandbox` (WasmExecutor, LimitsConfig, WASI stubs) | Proved: `wasm_sandbox_parity_with_native` — identical `state_hash` vs `NativeExecutor` over 30 ticks |
| **Anti-cheat by construction** | `magnetite-anticheat` (validators, TrustScoreMap, ReplayVerifier) | Proved: speedhack rejected + trust score escalated; `verify_replay` → `Clean` |
| **Pluggable seams** | `magnetite-seams` (Identity, Naming, BlobStore, Discovery, CommsProvider, PaymentRail) | Every seam ships a working default that needs zero external services |
| **One-command pipeline** | `magnetite-cli` (`magnetite new\|build\|dev\|node\|deploy`) | Implemented; `scripts/moat-demo.sh` runs the full pipeline |
| **JS web client** | `magnetite-web-client` (ClientNet/ServerNet, prediction, replay player) | Implemented; full-stack WS test proves Delta frames flow end-to-end |

---

## Repository Structure

```
magnetite/
├── backend/                        # Rust platform backend (Axum 0.8, SQLx 0.8.6)
│   ├── src/api/                    # 40 HTTP route modules (incl. discovery)
│   ├── src/services/               # 23 business-logic modules
│   ├── src/comms/                  # CommsProvider adapters + identity bridge + paid-room gate
│   ├── src/middleware/             # CORS, rate limiting, request logging
│   ├── src/jobs/                   # Background jobs (session cleanup, notification GC, backup, verification GC)
│   ├── src/ws/                     # WebSocket handlers (comms, voice, game, gauges)
│   ├── src/superadmin/             # Operator dashboards + non-custodial settlement checks
│   ├── magnetite-sdk/              # Rust SDK — AuthoritativeGame, GameLogic, Input, State, scaling
│   ├── migrations/                 # 46 SQL migration files
│   └── tests/                      # Integration tests (incl. live-HTTP discovery tests)
├── magnetite-seams/                # THE SEAMS: Identity, Naming, BlobStore, Discovery, Comms, PaymentRail
├── magnetite-runtime/              # Node host: capacity measurement, shard host, tracker client
├── magnetite-sandbox/              # WasmExecutor (Wasmtime); fuel/memory/epoch limits
├── magnetite-anticheat/            # Validators, TrustScoreMap, ReplayVerifier
├── magnetite-cli/                  # magnetite new|build|dev|node|deploy binary (clap 4)
├── magnetite-web-client/           # JS web client (ClientNet/ServerNet); prediction; replay player
├── magnetite-e2e/                  # Integration, full-stack, sharded, scale-bench, decentralized-loop tests
├── game-templates/                 # Starter game crates (dir name == template catalog id)
│   ├── arcade/                     # 2D arcade starter (Bevy + GameLogic + WASM-ready)
│   ├── authoritative/              # Arena shooter reference (AuthoritativeGame + wasm ABI)
│   ├── fps/                        # FPS starter (Bevy + rapier3d; hitscan; gamepad)
│   └── motorsport/                 # Motorsport starter (Bevy + rapier3d; lap → points)
├── game-client-bevy/               # Bevy client (prediction/reconciliation wired to ServerNet)
├── src/                            # React 19 frontend (server browser, wallet, storefront)
│   ├── i18n/                       # I18nProvider, useTranslation, en/es/fr JSON locales
│   └── …
├── site/                           # Landing page + docs viewer (site/docs/ mirrors selected chapters)
├── deploy/
│   ├── k8s/                        # 11 Kubernetes manifests (namespace → ingress + HPA)
│   └── nomad/                      # 6 Nomad job files
├── config/
│   └── mediamtx.yml                # OPTIONAL MediaMTX config (docker compose --profile media)
├── .github/workflows/              # CI, deploy, game-ci, game-deploy, release
├── docker-compose.yml              # Local stack; MediaMTX is an opt-in profile, not a dependency
├── Dockerfile.backend / Dockerfile.frontend / Dockerfile.fly
├── DECENTRALIZATION.md             # The redesign spec — seams, backlog, guardrails (kept at root)
└── docs/                           # This documentation tree
    └── project/                    # Program history: progress log, decisions, audits, roadmap
```

---

## License

Platform and SDK are MIT licensed. Documentation is CC0.
