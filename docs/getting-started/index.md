# Getting Started

> This page is a brief orientation. The full step-by-step guide is at
> [Developer Quickstart](../for-developers/quickstart.md).

Get up and running with Magnetite — the open-source platform for building, distributing,
and monetizing Rust games at any scale.

## Prerequisites

| Requirement | Version | Notes |
|-------------|---------|-------|
| Rust | 1.70+ | Required for game development |
| Docker | 24.0+ | For local development |
| PostgreSQL | 15+ | Database (via Docker) |
| Redis | 7+ | Caching and sessions |

## Quick Start (5 Minutes)

### 1. Install the CLI

```bash
cargo install magnetite-cli
```

### 2. Initialize Your Project

```bash
magnetite init my-first-game
cd my-first-game
```

### 3. Start Local Services

```bash
docker compose up -d
magnetite dev
```

Your game server is now running at `http://localhost:8080`.

## Account Creation

### Web Dashboard

1. Visit [dashboard.magnetite.dev](https://dashboard.magnetite.dev)
2. Click **Sign Up**
3. Verify your email
4. Complete KYC verification (required for payouts)

### CLI Authentication

```bash
magnetite auth login
# Opens browser for OAuth flow
```

### Environment Variables

```bash
MAGNETITE_API_KEY=your_api_key
MAGNETITE_API_SECRET=your_api_secret
```

## First Game

Let's create a simple rock-paper-scissors game.

### 1. Define Game Logic

```rust
// src/lib.rs
use magnetite_sdk::{GameLogic, Input, Output, State};

pub struct RpsGame {
    pub state: GameState,
}

#[derive(Default)]
pub struct GameState {
    pub player_choices: HashMap<PlayerId, Choice>,
    pub round: u32,
}

#[derive(Serialize, Deserialize)]
pub enum Choice {
    Rock,
    Paper,
    Scissors,
}

impl GameLogic for RpsGame {
    type Input = Choice;
    type Output = GameResult;
    type State = GameState;

    fn on_player_join(&mut self, player: PlayerId) -> Result<()> {
        self.state.player_choices.insert(player, Choice::Rock);
        Ok(())
    }

    fn on_input(&mut self, player: PlayerId, input: Self::Input) -> Result<()> {
        self.state.player_choices.insert(player, input);
        Ok(())
    }

    fn on_tick(&mut self) -> Option<Self::Output> {
        if self.state.player_choices.len() == 2 {
            Some(self.determine_winner())
        } else {
            None
        }
    }
}
```

### 2. Configure Game

```yaml
# magnetite.yaml
name: rock-paper-scissors
version: 1.0.0
max_players: 2
tick_rate: 1
entry_fee: 100
prize_pool: 180
```

### 3. Test Locally

```bash
magnetite run --players 2
```

### 4. Submit Your Game

```bash
magnetite submit --prod
```

See [Game Submission](/for-developers/submission/) for the full process.

## Next Steps

- [Developer Guide](/for-developers/) - Deep dive into game development
- [SDK Reference](/for-developers/sdk/) - Complete SDK documentation
- [API Reference](/api-reference/) - Backend API docs
