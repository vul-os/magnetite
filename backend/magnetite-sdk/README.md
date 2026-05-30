# magnetite-sdk

Rust SDK for building multiplayer games on the [Magnetite](https://magnetite.gg)
platform — games that scale from a weekend game jam to a COD-size AAA title.

## Features

- **`GameLogic` trait** — a single trait covers the full server-side lifecycle:
  construct, handle input, tick, snapshot/restore for save and replay, player
  join/leave hooks.
- **Strongly-typed input** — `KeyCode`, `InputEvent`, `KeyState`, `MouseState`,
  and `Input` frames separate raw hardware events from the game's action space.
- **Gamepad / controller input** (`input::gamepad`) — first-class gamepad support
  via `GamepadState`, `GamepadButton`, and `GamepadAxis`, plus a unified
  `InputMap` / `InputBinding` layer that maps gamepad _and_ keyboard/mouse inputs
  to high-level `GameAction`s (so a game reads intents, not raw buttons).
  Platform binding: Web Gamepad API (WASM) / gilrs (native).
- **Graphics / engine tiers** (`graphics`) — `GraphicsTier` (Lite2D, Standard3D,
  Advanced3D) + `RenderConfig` so simple jam games stay lightweight and FPS /
  motorsport titles scale up to WebGPU / Vulkan with HDR, rapier3d substeps, and
  high-quality shadows. `RenderConfig::supports(EngineCapability)` for
  compile-time / runtime assertions.
- **Versioned wire protocol** — `Envelope<T>` wraps every message with a
  `PROTOCOL_VERSION` header, sequence number, and timestamp. `ClientMessage` /
  `ServerMessage` cover the full handshake and game loop.
- **Networking abstractions** — `ServerConfig` + `TickLoop` manage tick rate and
  snapshot cadence; `PredictionBuffer` supports GGPO-style client-side rollback;
  `InterestManager` trait enables area-of-interest culling for large worlds
  (built-in: `FullInterest`, `RadiusInterest`).
- **Platform services** (`platform`) — typed clients for all shared platform
  services:
  - `platform::comms` — text chat, presence, and WebRTC voice signaling.
  - `platform::points` — points / XP / score economy (award, spend, balance,
    ledger, **match score submission**); server-authoritative with idempotency keys.
  - `platform::marketplace` — in-game store items, purchases (USDC / Paystack /
    points), and entitlements with a local cache.
  - `platform::cloud_save` — per-player named save slots (opaque blobs) with
    optimistic version conflict detection.
  - `platform::streaming` — **go-live broadcasting** (RTMP ingest key + HLS
    distribution), **spectator watch** (HLS player URL + live viewer count),
    stream discovery (list / get), and external RTMP egress to Twitch / YouTube.
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
| `graphics` | `GraphicsTier`, `RenderConfig`, `RenderConfigBuilder`, `EngineCapability` |
| `input` | `Input`, `KeyState`, `MouseState`, `InputEvent`, `KeyCode`, `Action`, `Direction` |
| `input::gamepad` | `GamepadState`, `GamepadButton`, `GamepadAxis`, `GamepadEvent`, `InputMap`, `InputBinding`, `InputSource`, `GameAction` |
| `state` | `GameState`, `PlayerState`, `PlayerId`, `Position`, `Rotation`, `Snapshot` |
| `protocol` | `Envelope`, `ClientMessage`, `ServerMessage`, `ErrorCode`, `PROTOCOL_VERSION` |
| `networking` | `ServerConfig`, `TickLoop`, `PredictionBuffer`, `InterestManager`, `FullInterest`, `RadiusInterest`, `ServerNetworkManager`, `Codec`, `FramedTransport` |
| `platform::comms` | `CommsClient`, `CommsConfig`, `ChatMessage`, `VoiceSignal`, `ClientCommsMessage`, `ServerCommsMessage`, `PresenceStatus` |
| `platform::points` | `PointsClient`, `PointsConfig`, `AwardPointsRequest`, `SpendPointsRequest`, `ScoreSubmission`, `PointsBalance`, `LedgerEntry`, `ClientPointsMessage`, `ServerPointsMessage` |
| `platform::marketplace` | `MarketplaceClient`, `MarketplaceConfig`, `StoreItem`, `Entitlement`, `PurchaseRequest`, `PurchaseResult`, `PaymentMethod`, `ItemType` |
| `platform::cloud_save` | `CloudSaveClient`, `CloudSaveConfig`, `SaveRequest`, `SaveSlot`, `SaveSlotMeta`, `ClientCloudSaveMessage`, `ServerCloudSaveMessage` |
| `platform::streaming` | `StreamClient`, `StreamConfig`, `GoLiveRequest`, `ExternalRtmpTarget`, `StreamInfo`, `StreamStatus`, `ClientStreamMessage`, `ServerStreamMessage`, `StreamEvent`, `StreamErrorCode` |

## Graphics / engine tiers

Every game declares a tier so the platform can provision the right runtime:

```rust
use magnetite_sdk::graphics::{GraphicsTier, RenderConfig};

// Weekend jam game — tiny WASM build, Canvas 2D.
let simple = RenderConfig::new(GraphicsTier::Lite2D);

// FPS starter — WebGL2/WebGPU, shadows, rapier3d.
let fps = RenderConfig::builder()
    .tier(GraphicsTier::Standard3D)
    .target_fps(60)
    .shadow_quality(2)
    .build();

// Motorsport AAA — WebGPU/Vulkan, HDR, 8 physics substeps.
let motorsport = RenderConfig::builder()
    .tier(GraphicsTier::Advanced3D)
    .target_fps(120)
    .hdr(true)
    .post_processing(true)
    .physics_substeps(8)
    .build();
```

Use `RenderConfig::supports(EngineCapability)` for runtime assertions:

```rust
use magnetite_sdk::graphics::{EngineCapability, GraphicsTier, RenderConfig};

let cfg = RenderConfig::new(GraphicsTier::Advanced3D);
assert!(cfg.supports(EngineCapability::Hdr));
assert!(cfg.supports(EngineCapability::VehiclePhysics));
```

## Gamepad / controller input

`input::gamepad` provides first-class controller support with a unified binding
layer — the same `InputMap` covers gamepad buttons/axes and keyboard/mouse keys,
so games read `GameAction`s rather than raw hardware events.

```rust
use magnetite_sdk::input::gamepad::{
    GameAction, GamepadButton, GamepadEvent, InputMap,
    InputBinding, InputSource,
};

// Default Xbox-style map (South = Jump, RightBumper = Fire, …).
let mut map = InputMap::default();

// Process a gamepad event.
let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
assert!(actions.contains(&GameAction::Jump));

// Process a keyboard event (same map, same actions).
use magnetite_sdk::input::{InputEvent, KeyCode};
let kb_actions = map.process_input(&InputEvent::Press(KeyCode::Jump));
assert!(kb_actions.contains(&GameAction::Jump));

// Remap at runtime.
map.bind(InputBinding {
    source: InputSource::Gamepad(GamepadButton::South),
    action: GameAction::Crouch,
});
```

### Platform binding

| Target | How to connect |
|---|---|
| **Browser (WASM)** | Poll `navigator.getGamepads()` each animation frame; convert button values → `GamepadEvent::ButtonPressed/Released` and axis values → `GamepadEvent::AxisMoved` |
| **Native (desktop)** | Use [gilrs](https://crates.io/crates/gilrs): call `gilrs.next_event()` each tick and convert `gilrs::EventType` → `GamepadEvent` |

The SDK keeps `gilrs` out of its dependencies so WASM builds stay light.

## Platform services

### Points / XP economy

```rust
use magnetite_sdk::platform::points::{
    AwardPointsRequest, PointsClient, PointsConfig, SpendPointsRequest,
};

let mut client = PointsClient::new(PointsConfig {
    user_id: "u-42".to_string(),
    auth_token: "jwt".to_string(),
});

// (Server-side) award points after a match win.
let msg = client.award_message(AwardPointsRequest {
    amount: 500,
    reason: "match_win".to_string(),
    game_id: Some("fps-starter".to_string()),
    idempotency_key: Some("match-42-win".to_string()),
});

// (Client-side) spend points on a cosmetic.
let spend_msg = client.spend_message(SpendPointsRequest {
    amount: 200,
    reason: "cosmetic_unlock".to_string(),
    item_id: Some("skin-neon".to_string()),
});
```

### In-game marketplace

```rust
use magnetite_sdk::platform::marketplace::{
    MarketplaceClient, MarketplaceConfig, PaymentMethod, PurchaseRequest,
};

let mut client = MarketplaceClient::new(MarketplaceConfig {
    user_id: "u-42".to_string(),
    game_id: "fps-starter".to_string(),
    auth_token: "jwt".to_string(),
});

// List store items.
let list_msg = client.list_items_message(None);

// Purchase an item.
let buy_msg = client.purchase_message(PurchaseRequest {
    item_id: "skin-neon".to_string(),
    payment_method: PaymentMethod::Usd,
    idempotency_key: None,
});

// Check entitlement after server responds.
assert!(!client.has_entitlement("skin-neon")); // not yet purchased
```

### Cloud saves

```rust
use magnetite_sdk::platform::cloud_save::{
    CloudSaveClient, CloudSaveConfig, SaveRequest,
};

let mut client = CloudSaveClient::new(CloudSaveConfig {
    user_id: "u-42".to_string(),
    game_id: "fps-starter".to_string(),
    auth_token: "jwt".to_string(),
});

// Save the current game state as a JSON blob.
let save_data = serde_json::to_vec(&serde_json::json!({
    "level": 3, "health": 80, "score": 9500,
})).unwrap();

let msg = client.save_message(SaveRequest {
    slot: "autosave".to_string(),
    data: save_data,
    version: None, // force-overwrite; pass Some(n) for optimistic locking
});

// Load.
let load_msg = client.load_message("autosave");
```

### Streaming (go-live / spectator)

```rust
use magnetite_sdk::platform::streaming::{
    ExternalRtmpTarget, GoLiveRequest, ServerStreamMessage, StreamClient, StreamConfig,
    StreamEvent,
};

// --- Broadcaster side ---
let mut broadcaster = StreamClient::new(StreamConfig {
    user_id: "u-caster".to_string(),
    auth_token: "jwt".to_string(),
});

let go_live_msg = broadcaster.go_live(GoLiveRequest {
    title: "FPS Ranked Stream".to_string(),
    game_id: Some("fps-starter".to_string()),
    community_id: None,
    channel_id: None,
    // Forward to Twitch as well.
    external_rtmp: Some(ExternalRtmpTarget {
        platform: "twitch".to_string(),
        rtmp_url: "rtmp://live.twitch.tv/live".to_string(),
    }),
});
// Send go_live_msg over the platform WebSocket.

// The platform responds with StreamStarted — hand the bytes to handle_server_message:
let event = broadcaster
    .handle_server_message(ServerStreamMessage::StreamStarted {
        stream_id: "s-42".to_string(),
        rtmp_key: "live-key-abc".to_string(),
        hls_url: Some("https://cdn.magnetite.gg/hls/s-42.m3u8".to_string()),
    })
    .unwrap();
// Push RTMP to rtmp://ingest.magnetite.gg/live/<rtmp_key>
assert!(broadcaster.is_live());

// --- Spectator side ---
let mut spectator = StreamClient::new(StreamConfig {
    user_id: "u-viewer".to_string(),
    auth_token: "jwt".to_string(),
});

let watch_msg = spectator.watch("s-42");
// Send watch_msg; receive WatchReady with the HLS URL → open the media player.
assert!(spectator.is_watching("s-42"));
```

### Score / match submission

```rust
use magnetite_sdk::platform::points::{
    ClientPointsMessage, PointsClient, PointsConfig, ScoreSubmission,
};

let client = PointsClient::new(PointsConfig {
    user_id: "u-42".to_string(),
    auth_token: "jwt".to_string(),
});

// Submit at the end of a match (server-side game logic only).
let msg = client.submit_score_message(ScoreSubmission {
    match_id: "match-101".to_string(),
    game_id: "fps-starter".to_string(),
    score: 12_000,
    placement: Some(1),
    kills: Some(25),
    deaths: Some(2),
    assists: Some(7),
    duration_secs: Some(600),
    extra: None,
    idempotency_key: Some("match-101-p1".to_string()),
});
// Send msg over the platform WebSocket.
// Platform responds with ServerPointsMessage::ScoreSubmitted { bonus_points, season_rank, … }
```

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
