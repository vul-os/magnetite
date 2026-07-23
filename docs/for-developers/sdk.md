# SDK Reference

`magnetite-sdk` is the Rust crate every Magnetite game depends on.
It lives at `backend/magnetite-sdk/` and is MIT licensed.

Add it to your game's `Cargo.toml`:

```toml
[dependencies]
magnetite-sdk = { path = "../backend/magnetite-sdk" }
# or, once published to crates.io:
magnetite-sdk = "0.1"
```

---

## Modules

| Module | Public re-exports |
|--------|-------------------|
| `game` | `GameLogic`, `GameMetadata` |
| `input` | `Input`, `KeyState`, `MouseState`, `InputEvent`, `KeyCode`, `Action`, `Direction` |
| `state` | `GameState`, `PlayerId`, `PlayerState`, `Position`, `Rotation`, `Snapshot` |
| `networking` | `Connection`, `Message`, `NetworkManager`, `ServerNetworkManager`, `StateSyncProtocol` |

---

## `GameLogic` trait (`game`)

The single trait you must implement. The platform calls your impl on every game tick.

```rust
pub trait GameLogic {
    fn new() -> Self;
    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action;
    fn tick(&mut self);
    fn state(&self) -> &GameState;
    fn players(&self) -> Vec<PlayerId>;
    fn metadata(&self) -> GameMetadata;
    fn snapshot(&self) -> Snapshot;
    fn restore(&mut self, snapshot: Snapshot);
}
```

All eight methods are required — none have default implementations.

| Method | Called by platform | Notes |
|--------|--------------------|-------|
| `new` | session start | Construct initial world state |
| `handle_input` | on each client input packet | Mutate state; return an `Action` |
| `tick` | `metadata().tick_rate` Hz | Advance simulation |
| `state` | after each tick | Return `&GameState` — platform reads it for broadcast |
| `players` | on player join/leave | List of active `PlayerId`s |
| `metadata` | session initialisation | Name, version, player counts, `tick_rate`, description |
| `snapshot` | rollback / replay / cloud save | Capture full state as a `Snapshot` |
| `restore` | rollback / replay resume | Rebuild state from a `Snapshot` |

### `GameMetadata`

```rust
pub struct GameMetadata {
    pub name:        String,   // human-readable display name
    pub version:     String,   // semantic version, e.g. "1.2.3"
    pub max_players: usize,    // max simultaneous players
    pub min_players: usize,    // min players required to start
    pub tick_rate:   u32,      // authoritative tick rate in Hz
    pub description: String,   // storefront blurb (plain text, ≤ 200 chars)
}
```

`GameMetadata` implements `Default`, so you can fill only the fields you care
about with `..GameMetadata::default()`.

---

## Input types (`input`)

### `Input`

The full snapshot of one player's controls at a single timestamp.

```rust
pub struct Input {
    pub keys:         KeyState,
    pub mouse:        MouseState,
    pub sequence:     u64,   // monotonically increasing frame counter (client-local)
    pub timestamp_ms: u64,   // client wall-clock time in ms (for latency measurement)
}
```

### `KeyState`

```rust
pub struct KeyState {
    pub forward:          bool,
    pub backward:         bool,
    pub left:             bool,
    pub right:            bool,
    pub jump:             bool,
    pub crouch:           bool,
    pub attack:           bool,
    pub secondary_attack: bool,
    pub interact:         bool,
    pub sprint:           bool,
}
```

### `MouseState`

```rust
pub struct MouseState {
    pub x:            f64,  // absolute cursor position
    pub y:            f64,
    pub delta_x:      f64,  // frame delta (use for camera rotation)
    pub delta_y:      f64,
    pub left_button:  bool,
    pub right_button: bool,
}
```

### `KeyCode` (enum)

`Forward | Backward | Left | Right | Jump | Crouch | Attack | SecondaryAttack | Interact | Sprint | Escape | Custom(u8)`

### `Action` (enum returned by `handle_input`)

```rust
pub enum Action {
    Move { direction: Direction },
    Jump,
    Crouch,
    Attack,
    None,
}

pub enum Direction { Forward, Backward, Left, Right }
```

### `InputEvent` (stream variant)

Used when replaying or streaming individual events rather than snapshots:

```rust
pub enum InputEvent {
    Press(KeyCode),
    Release(KeyCode),
    MouseMove { x: f64, y: f64 },
    MouseDelta { dx: f64, dy: f64 },
}
```

---

## State types (`state`)

### `PlayerId`

Opaque `u64` wrapper. Copy, Hash, Eq.

```rust
let pid = PlayerId::new(7);
assert_eq!(pid.as_u64(), 7);
```

### `Position`

Right-handed Y-up world-space coordinates (same as Bevy default).

```rust
pub struct Position { pub x: f32, pub y: f32, pub z: f32 }
// helper:
let dist = a.distance_to(b);
```

### `Rotation`

Euler angles in degrees.

```rust
pub struct Rotation { pub pitch: f32, pub yaw: f32, pub roll: f32 }
```

### `PlayerState`

Per-player data the platform always reads. Extend game-specific data via the `custom` field.

```rust
pub struct PlayerState {
    pub id:         PlayerId,
    pub position:   Position,
    pub rotation:   Rotation,
    pub health:     f32,
    pub max_health: f32,
    pub alive:      bool,
    pub score:      i64,
    pub custom:     serde_json::Value,  // game-specific payload
}

// helper:
let fraction = ps.health_fraction();   // clamped [0.0, 1.0]
```

### `GameState`

Authoritative server state at one tick.

```rust
pub struct GameState {
    pub tick:    u64,
    pub players: Vec<PlayerState>,
    pub world:   serde_json::Value,  // arbitrary simulation data
}

// helpers:
let ps: Option<&PlayerState>     = state.player(pid);
let ps: Option<&mut PlayerState> = state.player_mut(pid);
let removed: Option<PlayerState> = state.remove_player(pid);
```

### `Snapshot`

Wraps `GameState` with a tick counter and a non-cryptographic checksum for
rollback/replay and state-divergence detection.

```rust
let snap = Snapshot::new(tick, state);
assert!(snap.verify());
// Serialise/send/deserialise — checksum survives:
let json = serde_json::to_string(&snap)?;
let snap2: Snapshot = serde_json::from_str(&json)?;
assert!(snap2.verify());
```

---

## Networking types (`networking`)

These types are used by the platform's game-server runtime. Game logic code typically
does not call them directly, but they are public so advanced integrations can use them.

### `Message` (protocol enum)

```rust
pub enum Message {
    PlayerJoin(PlayerId),
    PlayerLeave(PlayerId),
    Input(Input),
    StateSync(GameState),
    StateSyncRequest,
    PlayerJoined { player_id: PlayerId, state: GameState },
    PlayerLeft { player_id: PlayerId },
    Ping,
    Pong,
    Error(String),
}
```

### `StateSyncProtocol`

A tick/state/checksum triple used for client reconciliation (GGPO-style rollback netcode).

```rust
let proto = StateSyncProtocol::new(tick, state);
assert!(proto.is_valid());
```

### `NetworkManager` / `ServerNetworkManager` / `Connection`

Low-level TCP helpers for native clients and the server runtime. Browser clients use
WebSocket instead (handled by the platform's `ws/game.rs` module).

---

## Example: minimal game

```rust
use magnetite_sdk::{
    export_game,
    game::{GameLogic, GameMetadata},
    input::{Action, Input},
    state::{GameState, PlayerId, Snapshot},
};

pub struct PongGame {
    state: GameState,
}

impl GameLogic for PongGame {
    fn new() -> Self {
        Self { state: GameState::default() }
    }

    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action {
        if let Some(ps) = self.state.player_mut(player_id) {
            if input.keys.forward  { ps.position.y += 5.0; }
            if input.keys.backward { ps.position.y -= 5.0; }
        }
        Action::None
    }

    fn tick(&mut self) {
        self.state.tick += 1;
        // advance ball, detect collisions, update scores …
    }

    fn state(&self) -> &GameState {
        &self.state
    }

    fn players(&self) -> Vec<PlayerId> {
        self.state.players.iter().map(|p| p.id).collect()
    }

    fn metadata(&self) -> GameMetadata {
        GameMetadata {
            name: "pong".into(),
            max_players: 2,
            min_players: 2,
            tick_rate: 60,
            description: "Two-player Pong".into(),
            ..GameMetadata::default()
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot::new(self.state.tick, self.state.clone())
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.state = snapshot.state;
    }
}

export_game!(PongGame);
```

---

## See also

- [Developer Quickstart](./quickstart.md)
- [Build & Distribution Pipeline](./build-pipeline.md)
- [Architecture Overview](../architecture.md)
