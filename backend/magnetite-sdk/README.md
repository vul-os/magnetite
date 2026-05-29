# Magnetite SDK

A Rust SDK for building multiplayer games with Magnetite.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
magnetite-sdk = { path = "../magnetite-sdk" }
serde = { version = "1", features = ["derive"] }
```

## Usage

```rust
use magnetite_sdk::{
    Action, Direction, GameLogic, GameMetadata, GameState, Input, KeyCode, KeyState,
    MouseState, PlayerId, PlayerState, Position, Rotation,
};

struct SimpleGame {
    state: GameState,
    tick: u64,
}

impl GameLogic for SimpleGame {
    fn new() -> Self {
        Self {
            state: GameState {
                players: vec![PlayerState {
                    id: PlayerId::new(1),
                    position: Position { x: 0.0, y: 0.0, z: 0.0 },
                    rotation: Rotation { pitch: 0.0, yaw: 0.0 },
                    health: 100.0,
                }],
            },
            tick: 0,
        }
    }

    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action {
        if input.keys.forward {
            if let Some(player) = self.state.players.iter_mut().find(|p| p.id == player_id) {
                player.position.z += 0.1;
            }
            Action::Move { direction: Direction::Forward }
        } else if input.keys.backward {
            if let Some(player) = self.state.players.iter_mut().find(|p| p.id == player_id) {
                player.position.z -= 0.1;
            }
            Action::Move { direction: Direction::Backward }
        } else if input.keys.jump {
            Action::Jump
        } else if input.keys.attack {
            Action::Attack
        } else {
            Action::None
        }
    }

    fn tick(&mut self) {
        self.tick += 1;
    }

    fn state(&self) -> GameState {
        self.state.clone()
    }

    fn players(&self) -> Vec<PlayerId> {
        self.state.players.iter().map(|p| p.id).collect()
    }

    fn metadata(&self) -> GameMetadata {
        GameMetadata {
            name: "Simple Game".to_string(),
            max_players: 8,
            tick_rate: 60,
        }
    }
}

fn main() {
    let mut game = SimpleGame::new();

    let input = Input {
        keys: KeyState {
            forward: true,
            ..Default::default()
        },
        mouse: MouseState::default(),
        timestamp: 0,
    };

    let action = game.handle_input(PlayerId::new(1), input);
    game.tick();

    println!("Action: {:?}", action);
    println!("State: {:?}", game.state());
    println!("Players: {:?}", game.players());
    println!("Metadata: {:?}", game.metadata());
}
```

## Features

- `GameLogic` trait for implementing game logic
- `Input` handling for keyboard and mouse
- `GameState` serialization for networking
- `NetworkManager` for client/server communication
- `StateSyncProtocol` for reliable state synchronization
- Player join/leave handling
