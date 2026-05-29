# magnetite-sdk

Rust SDK for building multiplayer games on the [Magnetite](https://magnetite.gg)
platform — games that scale from a weekend game jam to a COD-size AAA title.

## Features

- **`GameLogic` trait** — a single trait covers the full server-side lifecycle:
  construct, handle input, tick, snapshot/restore for save and replay, player
  join/leave hooks.
- **Strongly-typed input** — `KeyCode`, `InputEvent`, `KeyState`, `MouseState`,
  and `Input` frames separate raw hardware events from the game's action space.
- **Versioned wire protocol** — `Envelope<T>` wraps every message with a
  `PROTOCOL_VERSION` header, sequence number, and timestamp. `ClientMessage` /
  `ServerMessage` cover the full handshake and game loop.
- **Networking abstractions** — `ServerConfig` + `TickLoop` manage tick rate and
  snapshot cadence; `PredictionBuffer` supports GGPO-style client-side rollback;
  `InterestManager` trait enables area-of-interest culling for large worlds
  (built-in: `FullInterest`, `RadiusInterest`).
- **`export_game!` macro** — one line registers your game with the Magnetite
  runtime, emitting the C FFI glue needed for dynamic loading by the server and
  WASM host.
- **No heavy dependencies** — only `serde` + `serde_json`. A future `binary`
  feature flag will add MessagePack for high-throughput AAA scenarios.
- **MIT license.**

## Installation

```toml
[dependencies]
magnetite-sdk = { path = "../magnetite-sdk" }
```

## Quick start

```rust
use magnetite_sdk::{
    export_game,
    game::{GameLogic, GameMetadata},
    input::{Action, Input},
    state::{GameState, PlayerId, PlayerState, Position, Rotation, Snapshot},
};

struct MyGame {
    state: GameState,
}

impl GameLogic for MyGame {
    fn new() -> Self {
        MyGame { state: GameState::default() }
    }

    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action {
        if input.keys.forward {
            if let Some(p) = self.state.player_mut(player_id) {
                p.position.z += 0.1;
            }
            return Action::Move {
                direction: magnetite_sdk::input::Direction::Forward,
                sprint: input.keys.sprint,
            };
        }
        Action::None
    }

    fn tick(&mut self) {
        self.state.tick += 1;
    }

    fn state(&self) -> &GameState {
        &self.state
    }

    fn players(&self) -> Vec<PlayerId> {
        self.state.players.iter().map(|p| p.id).collect()
    }

    fn metadata(&self) -> GameMetadata {
        GameMetadata {
            name: "My Game".to_string(),
            version: "0.1.0".to_string(),
            max_players: 16,
            min_players: 1,
            tick_rate: 60,
            description: "An example Magnetite game.".to_string(),
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot::new(self.state.tick, self.state.clone())
    }

    fn restore(&mut self, snap: Snapshot) {
        self.state = snap.state;
    }

    fn on_player_join(&mut self, player_id: PlayerId) {
        self.state.players.push(PlayerState {
            id: player_id,
            position: Position::default(),
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        });
    }

    fn on_player_leave(&mut self, player_id: PlayerId) {
        self.state.remove_player(player_id);
    }
}

// Register with the Magnetite runtime.
export_game!(MyGame);
```

## Module overview

| Module | Key types |
|---|---|
| `game` | `GameLogic`, `GameMetadata` |
| `input` | `Input`, `KeyState`, `MouseState`, `InputEvent`, `KeyCode`, `Action`, `Direction` |
| `state` | `GameState`, `PlayerState`, `PlayerId`, `Position`, `Rotation`, `Snapshot` |
| `protocol` | `Envelope`, `ClientMessage`, `ServerMessage`, `ErrorCode`, `PROTOCOL_VERSION` |
| `networking` | `ServerConfig`, `TickLoop`, `PredictionBuffer`, `InterestManager`, `FullInterest`, `RadiusInterest`, `ServerNetworkManager`, `Codec`, `FramedTransport` |

## Snapshot / Restore (save, replay, prediction)

`GameLogic::snapshot` captures the full game state as a `Snapshot` (state +
tick + checksum). `GameLogic::restore` rolls back to that snapshot exactly.

The platform calls `snapshot` periodically and stores it in the replay buffer.
Client-side prediction calls `snapshot` before speculative ticks, then `restore`
when the server's authoritative response arrives.

```rust
# use magnetite_sdk::state::{GameState, Snapshot};
let state = GameState::default();
let snap = Snapshot::new(42, state.clone());
assert!(snap.verify());
let json = serde_json::to_string(&snap).unwrap();
let snap2: Snapshot = serde_json::from_str(&json).unwrap();
assert!(snap2.verify());
```

## Wire protocol

Every message is wrapped in `Envelope<T>`:

```text
{ "version": 1, "seq": 42, "timestamp_ms": 1000000, "body": { "type": "input_frame", ... } }
```

Version mismatches are detected before deserialising the body — the server
sends `ServerMessage::Error { code: PROTOCOL_MISMATCH }` and drops the
connection.

## Interest management (AAA scale)

For large games, implement `InterestManager` to return only the state each
player should see:

```rust
use magnetite_sdk::networking::{InterestManager, RadiusInterest};
use magnetite_sdk::state::{GameState, PlayerId};

// Built-in: full broadcast (small games).
let full = magnetite_sdk::networking::FullInterest;

// Built-in: cull players outside 200m.
let radius = RadiusInterest::new(200.0);
```

## Tick rate & server loop

```rust
use magnetite_sdk::networking::{ServerConfig, TickLoop};

let cfg = ServerConfig::builder()
    .tick_rate(128)          // competitive FPS
    .snapshot_interval(64)   // snapshot every 64 ticks
    .max_players(100)
    .build();

let mut tl = TickLoop::from_config(&cfg);
loop {
    tl.advance();
    // ... call game.tick(), broadcast state ...
    if tl.should_snapshot() {
        // ... broadcast Snapshot ...
    }
    std::thread::sleep(tl.tick_duration());
}
```

## Client-side prediction

```rust
use magnetite_sdk::input::Input;
use magnetite_sdk::networking::PredictionBuffer;

let mut buf = PredictionBuffer::new(128);

// On send:
buf.push(my_input_frame);

// On server ack (sequence N confirmed):
buf.acknowledge(acked_sequence);

// Pending frames must be re-simulated on top of the server's snapshot:
for frame in buf.pending() {
    game.handle_input(my_player_id, *frame);
    game.tick();
}
```

## License

MIT
