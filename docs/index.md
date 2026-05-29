# Magnetite Documentation

**Magnetite** is the open-source platform for building, distributing, and monetizing Rust games —
that scale from a weekend game jam to a COD-size AAA title.

Game logic is authored in Rust. Clients compile Bevy to WASM (browser) and to native. The platform
is server-authoritative and sandboxed, providing hosting, matchmaking, real-time netcode, persistence,
and payment rails — so developers only write game logic.

---

## Quick Links

| Section | Description |
|---------|-------------|
| [Developer Quickstart](./for-developers/quickstart.md) | Clone template → implement → build WASM → publish |
| [SDK Reference](./for-developers/sdk.md) | `magnetite-sdk` crate reference |
| [Build & Distribution Pipeline](./for-developers/build-pipeline.md) | How games go from source to players |
| [Architecture Overview](./architecture.md) | Backend modules, services, and data flow |
| [Self-Hosting Guide](./self-hosting/index.md) | Docker Compose and Fly.io deployments |
| [Security & Sandboxing](./security/index.md) | Auth, anti-cheat, and deployment hardening |
| [API Reference](./api-reference/index.md) | REST API endpoints |

---

## What Magnetite Provides

| Concern | What the platform handles |
|---------|--------------------------|
| **Distribution** | Storefront/marketplace; players discover, play (browser WASM or native), and pay |
| **Hosting** | Server-authoritative Rust game servers; WASM artifacts served to browsers |
| **Matchmaking** | Queue join/leave/status; player pairing |
| **Real-time netcode** | WebSocket state-sync; client SDK connection types |
| **Persistence** | Leaderboards, achievements, session history, replays |
| **Payments** | USDC via Circle; Paystack fiat on-ramp; 15% platform fee; playtime-based developer payouts |
| **Social** | Friends, invites, notifications |
| **Analytics** | Developer dashboard, revenue breakdown, session stats |

---

## Repository Structure

```
magnetite/
├── backend/                  # Rust platform backend (Axum 0.7, SQLx 0.8)
│   ├── src/api/              # HTTP route modules (auth, games, wallet, developer, …)
│   ├── src/services/         # Business-logic layer
│   ├── src/middleware/       # CORS, rate limiting, request logging
│   ├── src/jobs/             # Background jobs (session cleanup, notification GC, backup)
│   ├── src/ws/               # WebSocket game handler
│   ├── magnetite-sdk/        # Rust SDK — GameLogic trait, Input, State, Networking
│   ├── migrations/           # SQL migration files
│   └── tests/                # Integration tests
├── game-template/            # Bevy + magnetite-sdk starter (compiles to WASM)
├── src/                      # React 19 frontend
├── e2e/                      # Playwright end-to-end tests
└── docs/                     # This documentation tree
```

## License

Platform and SDK are MIT licensed. Documentation is CC0.
