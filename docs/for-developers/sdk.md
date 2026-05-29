# SDK Reference

Complete reference for the Magnetite Rust SDK.

## GameLogic Trait

Core trait for implementing game logic.

```rust
pub trait GameLogic {
    type Input: serde::Serialize + serde::de::DeserializeOwned;
    type Output: serde::Serialize + serde::de::DeserializeOwned;
    type State: serde::Serialize + serde::de::DeserializeOwned;

    fn on_player_join(&mut self, player: PlayerId) -> Result<(), GameError>;
    fn on_player_leave(&mut self, player: PlayerId) -> Result<(), GameError>;
    fn on_input(&mut self, player: PlayerId, input: Self::Input) -> Result<(), GameError>;
    fn on_tick(&mut self) -> Option<Self::Output>;
}
```

### Method Signatures

| Method | Parameters | Returns | Required |
|--------|------------|---------|----------|
| `on_player_join` | `player: PlayerId` | `Result<(), GameError>` | Yes |
| `on_player_leave` | `player: PlayerId` | `Result<(), GameError>` | Yes |
| `on_input` | `player: PlayerId`, `input: Self::Input` | `Result<(), GameError>` | Yes |
| `on_tick` | - | `Option<Self::Output>` | Yes |
| `on_start` | - | `Result<(), GameError>` | No |
| `on_end` | - | `Result<(), GameError>` | No |

## Input/Output Types

### Input Types

Inputs are player actions sent to the game server.

```rust
#[derive(Serialize, Deserialize)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
    Action,
}

#[derive(Serialize, Deserialize)]
pub struct AimInput {
    pub x: f32,
    pub y: f32,
}
```

### Output Types

Outputs are broadcast to all players each tick.

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GameOutput {
    Tick { state: GameState },
    GameOver { winner: PlayerId, scores: HashMap<PlayerId, u32> },
    PlayerJoined { player: PlayerId },
    PlayerLeft { player: PlayerId },
}

#[derive(Serialize, Deserialize)]
pub struct GameState {
    pub players: Vec<PlayerState>,
    pub arena: ArenaState,
    pub round: u32,
}
```

## State Management

### PlayerId

Unique identifier for each player.

```rust
pub struct PlayerId(pub String);
```

### Game State

```rust
pub trait GameState {
    fn initial() -> Self;
    fn save(&self) -> Vec<u8>;
    fn load(data: &[u8]) -> Self;
}
```

### Serialization

All state types must implement `Serialize` and `DeserializeOwned`.

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct MyGameState {
    pub positions: HashMap<PlayerId, Position>,
    pub scores: HashMap<PlayerId, u32>,
    pub tick: u64,
}
```

## Examples

### Number Guessing Game

```rust
use magnetite_sdk::{GameLogic, Input, Output, State};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Default)]
pub struct NumberGuessing {
    pub secret: u32,
    pub guesses: HashMap<PlayerId, u32>,
    pub winner: Option<PlayerId>,
}

#[derive(Serialize, Deserialize)]
pub struct Guess(u32);

#[derive(Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum Result {
    Correct { winner: PlayerId },
    Wrong { player: PlayerId, diff: u32 },
}

impl GameLogic for NumberGuessing {
    type Input = Guess;
    type Output = Result;
    type State = NumberGuessing;

    fn on_start(&mut self) -> Result<(), GameError> {
        self.secret = rand::random::<u32>() % 100;
        Ok(())
    }

    fn on_input(&mut self, player: PlayerId, input: Guess) -> Result<(), GameError> {
        if self.winner.is_some() {
            return Err(GameError::GameOver);
        }
        self.guesses.insert(player, input.0);
        Ok(())
    }

    fn on_tick(&mut self) -> Option<Self::Output> {
        for (player, guess) in &self.guesses {
            if *guess == self.secret {
                self.winner = Some(player.clone());
                return Some(Result::Correct { winner: player.clone() });
            }
            return Some(Result::Wrong {
                player: player.clone(),
                diff: (*guess as i32 - self.secret as i32).unsigned_abs() as u32,
            });
        }
        None
    }
}
```

### Real-time Arena Game

```rust
#[derive(Serialize, Deserialize)]
pub struct Position { pub x: f32, pub y: f32 }

#[derive(Serialize, Deserialize)]
pub struct Velocity { pub dx: f32, pub dy: f32 }

#[derive(Serialize, Deserialize)]
pub struct PlayerState {
    pub id: PlayerId,
    pub pos: Position,
    pub vel: Velocity,
    pub health: u32,
}

#[derive(Default)]
pub struct ArenaGame {
    pub players: HashMap<PlayerId, PlayerState>,
}

impl GameLogic for ArenaGame {
    type Input = Velocity;
    type Output = ArenaOutput;
    type State = ArenaGame;

    fn on_input(&mut self, player: PlayerId, input: Velocity) -> Result<(), GameError> {
        if let Some(state) = self.players.get_mut(&player) {
            state.vel = input;
        }
        Ok(())
    }

    fn on_tick(&mut self) -> Option<ArenaOutput> {
        for state in self.players.values_mut() {
            state.pos.x += state.vel.dx * TICK_DT;
            state.pos.y += state.vel.dy * TICK_DT;
        }
        Some(ArenaOutput::StateUpdate {
            players: self.players.values().cloned().collect()
        })
    }
}
```

## Error Handling

```rust
#[derive(Debug)]
pub enum GameError {
    InvalidInput(String),
    GameOver,
    PlayerNotFound,
    InsufficientFunds,
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::InvalidInput(s) => write!(f, "Invalid input: {}", s),
            GameError::GameOver => write!(f, "Game has ended"),
            GameError::PlayerNotFound => write!(f, "Player not found"),
            GameError::InsufficientFunds => write!(f, "Insufficient funds"),
        }
    }
}

impl std::error::Error for GameError {}
```
