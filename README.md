# Magnetite

**Magnetite (Fe₃O₄)** - Iron oxide, magnetic, grounded. The foundation upon which things are built.

*Open source games. Real money. No middlemen.*

## Overview

Magnetite is a platform where developers host open-source games via GitHub, players pay with USDC, and Magnetite provides the hosting, real-time infrastructure, and payment rails.

**Core principle:** Developers own their code, Magnetite provides the platform layer, players pay for access.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Platform Backend | Rust (Axum) |
| Game Engine | Bevy (MIT) |
| Database | PostgreSQL (SQLx) |
| Cache/State | Redis |
| Payments | USDC (Circle SDK) |
| Fiat On-Ramp | Paystack |
| Client | React + Vite |
| Infrastructure | Fly.io |

## Project Structure

```
magnetite/
├── backend/              # Rust backend
│   ├── src/
│   │   ├── api/        # HTTP API handlers
│   │   ├── db/         # Database pool
│   │   ├── services/   # Business logic
│   │   └── ws/         # WebSocket handlers
│   ├── migrations/     # SQL migrations
│   └── tools/          # Dev tools (migrate.sh)
├── src/                 # React frontend
│   ├── api/            # API client
│   ├── components/     # UI components
│   ├── context/        # React contexts
│   ├── data/           # Mock data
│   ├── hooks/          # Custom hooks
│   └── pages/          # Page components
└── ...
```

## Getting Started

### Prerequisites

- Rust 1.75+
- Node.js 18+
- PostgreSQL 15+

### Backend Setup

```bash
cd backend
cargo build

# Set environment variables
export DATABASE_URL="postgres://postgres:postgres@localhost:54322/postgres"
export JWT_SECRET="your-secret-key"

cargo run
```

### Frontend Setup

```bash
npm install
npm run dev
```

### Running Migrations

```bash
cd backend/tools
./migrate.sh up      # Run pending migrations
./migrate.sh status  # Check migration status
./migrate.sh reset   # Reset database (WARNING: drops all tables)
```

## Features

- **Marketplace**: Browse and play open-source games
- **Developer Dashboard**: Host your games, track earnings
- **USDC Wallet**: Deposit, withdraw, play sessions
- **Matchmaking**: Find opponents automatically
- **Real-time**: WebSocket game connections

## API Routes

| Endpoint | Description |
|----------|-------------|
| `POST /api/auth/register` | Register new user |
| `POST /api/auth/login` | Login and get JWT |
| `GET /api/auth/me` | Get current user |
| `GET /api/wallet/balance` | Get USDC balance |
| `GET /api/games` | List all games |
| `POST /api/games` | Create new game |
| `GET /api/games/:id/leaderboard` | Get game leaderboard |
| `POST /api/matchmaking/join` | Join matchmaking queue |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     MAGNETITE PLATFORM                      │
├─────────────────────────────────────────────────────────────┤
│  HTTP/WebSocket Gateway (Rust/Axum)                        │
│  ├── /api/auth  /api/wallet  /api/games  /api/matchmaking  │
│  └── /ws/game/{id}                                         │
├─────────────────────────────────────────────────────────────┤
│  Shared Services: Leaderboards, Achievements, Matchmaking  │
├─────────────────────────────────────────────────────────────┤
│  Game Instances (Developer Code) - Rust/WASM (Bevy)        │
└─────────────────────────────────────────────────────────────┘
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

---

*Built with Rust. Powered by open source.*
