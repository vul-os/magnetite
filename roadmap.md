# Magnetite - Open Source Gaming Platform

**Magnetite (Fe₃O₄)** - Iron oxide, magnetic, grounded. The foundation upon which things are built.

*Open source games. Real money. No middlemen.*

---

## Concept

Magnetite is a platform where developers host open-source games via GitHub, players pay with USDC, and Magnetite provides the hosting, real-time infrastructure, and payment rails.

**Core principle:** Developers own their code, Magnetite provides the platform layer, players pay for access.

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Platform Backend | Rust (Axum/Actix-web) |
| Game Engine | Bevy (MIT) |
| Game Physics | rapier (MIT) |
| Networking | quinn (QUIC) |
| Game Sandbox | Wasmtime (WASM) + gVisor (native) |
| Database | PostgreSQL (SQLx) |
| Cache/State | Redis |
| Payments | USDC (Circle SDK) |
| Fiat On-Ramp | Paystack (South Africa) |
| Infrastructure | Fly.io (Firecracker VMs) |
| Client | Rust → WASM (Bevy) |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     MAGNETITE PLATFORM                      │
├─────────────────────────────────────────────────────────────┤
│  HTTP/WebSocket Gateway (Rust/Axum)                        │
│  ├── /api/auth                                             │
│  ├── /api/wallet                                           │
│  ├── /api/games                                            │
│  ├── /api/matchmaking                                      │
│  └── /ws/game/{id}                                         │
├─────────────────────────────────────────────────────────────┤
│  Shared Services                                            │
│  ├── Leaderboards (Redis)                                   │
│  ├── Achievements                                          │
│  ├── Matchmaking                                           │
│  ├── Anti-Cheat                                            │
│  └── Payout Engine                                          │
├─────────────────────────────────────────────────────────────┤
│  Game Instance Manager                                      │
│  ├── Isolated game sessions                                │
│  ├── Resource limits                                        │
│  └── Lifecycle management                                   │
├─────────────────────────────────────────────────────────────┤
│  Game Instances (Developer Code)                            │
│  ├── Client: Rust/WASM (Bevy)                             │
│  └── Server: Rust (native, sandboxed)                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Developer Workflow

### Game Submission

1. Clone magnetite-sdk template
2. Implement `GameLogic` trait
3. Define shared protocol types
4. Push to GitHub
5. Register repo on Magnetite dashboard

### SDK Structure

```rust
use magnetite_sdk::*;

struct MyGame {
    players: HashMap<PlayerId, Player>,
    state: GameState,
}

impl GameLogic for MyGame {
    fn new() -> Self { ... }
    fn handle_input(&mut self, player: PlayerId, input: Input) -> Action { ... }
    fn tick(&mut self) { ... }
    fn state(&self) -> &GameState { ... }
}

magnetite_sdk::export_game!(MyGame);
```

### CI/CD Pipeline

```
GitHub Webhook → Platform CI:
1. Pull source
2. cargo build --target wasm32-unknown-unknown
3. Security scan
4. Deploy to sandbox → automated test
5. Manual review
6. Live on platform
```

---

## Payment Flow

### Subscription Tiers

| Tier | Price | Access |
|------|-------|--------|
| Free | $0/mo | Limited to free games |
| Basic | $4.99/mo | 10 hours/month |
| Pro | $9.99/mo | 50 hours/month |
| Unlimited | $19.99/mo | Unlimited hours |

### Developer Revenue

- Platform takes 15% platform fee on subscription revenue
- Developers earn based on playtime their games receive
- Monthly payout based on proportional playtime

---

## Security

### Game Isolation

| Runtime | Isolation Level | Use Case |
|---------|-----------------|----------|
| Wasmtime | Sandboxed WASM | Untrusted code |
| gVisor | Container sandbox | Native binaries |

### Anti-Cheat Layers

**Layer 1: Platform (Global)**
- Velocity detection
- Anomaly detection
- Device fingerprinting
- Global ban list

**Layer 2: Game Instance (Per-Game)**
- Server-authoritative state
- Input validation
- Replay storage
- Custom rules

---

## Database Schema

```sql
-- Users
CREATE TABLE users (
    id UUID PRIMARY KEY,
    username TEXT UNIQUE,
    email TEXT UNIQUE,
    password_hash TEXT,
    wallet_address TEXT,
    created_at TIMESTAMPTZ
);

-- Games
CREATE TABLE games (
    id UUID PRIMARY KEY,
    developer_id UUID REFERENCES users(id),
    github_repo TEXT,
    title TEXT,
    description TEXT,
    status TEXT,
    created_at TIMESTAMPTZ
);

-- Transactions
CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    game_id UUID REFERENCES games(id),
    type TEXT,
    amount DECIMAL(10, 6),
    created_at TIMESTAMPTZ
);

-- Leaderboards
CREATE TABLE scores (
    id UUID PRIMARY KEY,
    game_id UUID REFERENCES games(id),
    user_id UUID REFERENCES users(id),
    score BIGINT,
    recorded_at TIMESTAMPTZ
);
```

---

## Roadmap

### Phase 1: Foundation (Months 1-3)

- [ ] Platform API (auth, wallet, basic CRUD)
- [ ] Developer dashboard
- [ ] Simple HTML5 game hosting
- [ ] Paystack integration
- [ ] Basic Fly.io deployment

### Phase 2: Rust Integration (Months 4-6)

- [ ] magnetite-sdk Rust crate
- [ ] Bevy game template
- [ ] WASM game build pipeline
- [ ] WebSocket game connections
- [ ] Leaderboards, achievements

### Phase 3: Real-Time Multiplayer (Months 7-9)

- [ ] Matchmaking system
- [ ] Quinn UDP networking
- [ ] Game instance manager
- [ ] gVisor isolation
- [ ] Anti-cheat layer 1

### Phase 4: Monetization (Months 10-12)

- [ ] Subscription tiers (Free, Basic, Pro, Unlimited)
- [ ] Platform fee (15%) and playtime-based developer revenue
- [ ] Developer payouts based on proportional playtime
- [ ] Anti-cheat layer 2

### Phase 5: Scale (Year 2)

- [ ] Multi-region deployment
- [ ] Advanced matchmaking
- [ ] Tournament system
- [ ] Developer API
- [ ] Mobile client

---

## Team

| Role | Responsibilities |
|------|------------------|
| Platform Engineer | Rust backend, API, Fly.io |
| Game Engineer | Bevy, WASM, game templates |
| Security | Anti-cheat, isolation, audits |

---

## Timeline

| Milestone | Time |
|-----------|------|
| MVP (HTML5 + wallet) | 2 months |
| First paid game | 4 months |
| Rust WASM support | 6 months |
| Real-time multiplayer | 9 months |
| Developer marketplace | 12 months |

---

## Open Source

| Component | License | Repo |
|-----------|---------|------|
| Platform API | MIT | github.com/magnetite/magnetite |
| SDK | MIT | github.com/magnetite/sdk |
| Game Template | MIT | github.com/magnetite/game-template |
| Docs | CC0 | github.com/magnetite/docs |

---

## Key Decisions

| Question | Decision |
|----------|----------|
| Client language | Rust (WASM via Bevy) |
| Server language | Rust (native) |
| Payment | USDC via Circle |
| Fiat on-ramp | Paystack (SA users) |
| Dev payout | Direct USDC wallet |
| Game isolation | Wasmtime + gVisor |
| Infra | Fly.io |
| Database | PostgreSQL + Redis |
| Open source | MIT |

---

## Risks

| Risk | Mitigation |
|------|------------|
| Chicken-and-egg (devs/users) | Start with curated games |
| Security vulnerabilities | Wasmtime + gVisor isolation |
| Crypto UX barrier | Paystack on-ramp for fiat |
| Regulatory (SA) | Partner with licensed exchanges |
| Developer adoption | Excellent SDK, documentation |

---

*Built with Rust. Powered by open source.*
