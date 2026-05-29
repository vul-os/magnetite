# For Developers

Build competitive mini-games on Magnetite using Rust.

## Quickstart

```bash
# Install CLI
cargo install magnetite-cli

# Create new game
magnetite init my-game
cd my-game

# Run example game
magnetite run --example rps
```

## SDK Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
magnetite-sdk = "0.4"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
```

## Your First Game

### 1. Create Project Structure

```
my-game/
├── Cargo.toml
├── magnetite.yaml
├── src/
│   └── lib.rs
└── tests/
    └── integration.rs
```

### 2. Implement GameLogic Trait

```rust
use magnetite_sdk::{GameLogic, Input, Output, State};

pub struct MyGame {
    state: GameState,
}

pub struct GameState {
    score: HashMap<PlayerId, u32>,
    round: u32,
}

impl GameLogic for MyGame {
    type Input = Move;
    type Output = GameEvent;
    type State = GameState;

    fn on_tick(&mut self) -> Option<Self::Output> {
        self.state.round += 1;
        Some(GameEvent::RoundStart(self.state.round))
    }
}
```

### 3. Configure Game Settings

```yaml
# magnetite.yaml
name: my-game
version: 1.0.0
max_players: 4
min_players: 2
tick_rate: 30
timeout_seconds: 300
entry_fee: 100
prize_pool: 80
auto_start: true
```

## Game Lifecycle

```
┌─────────────┐
│  CREATED    │
└──────┬──────┘
       │ start()
       ▼
┌─────────────┐
│  WAITING    │◄──────────────┐
└──────┬──────┘               │
       │ min_players reached  │ player_leave()
       ▼                      │
┌─────────────┐               │
│  STARTING   │───────────────┘
└──────┬──────┘
       │ countdown complete
       ▼
┌─────────────┐
│  PLAYING    │
└──────┬──────┘
       │ on_tick returns Some(Output::GameOver)
       ▼
┌─────────────┐
│  ENDED      │
└─────────────┘
```

### State Transitions

| Method | State Change | Description |
|--------|--------------|-------------|
| `on_player_join` | `WAITING` | New player enters lobby |
| `on_player_leave` | `WAITING` | Player exits lobby |
| `start` | `WAITING` → `STARTING` | Game begins countdown |
| `on_tick` | `PLAYING` | Main game loop |
| `end_game` | `PLAYING` → `ENDED` | Game concludes |

### Example: Complete Game Loop

```rust
impl GameLogic for MyGame {
    fn on_tick(&mut self) -> Option<Self::Output> {
        match self.state.phase {
            Phase::Playing => {
                if self.check_game_over() {
                    Some(Output::GameOver {
                        winner: self.determine_winner(),
                        scores: self.state.score.clone(),
                    })
                } else {
                    self.process_round();
                    Some(Output::Tick {
                        state: self.get_public_state(),
                    })
                }
            }
            Phase::Waiting | Phase::Starting => None,
        }
    }
}
```

## Next Steps

- [SDK Reference](/for-developers/sdk/) - Complete trait documentation
- [Game Submission](/for-developers/submission/) - Submit to production
