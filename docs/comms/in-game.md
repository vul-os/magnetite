# In-Game Chat and Voice

Every Magnetite game session automatically gets a paired **text channel** and
**voice room**. Players in the same lobby or match can chat and talk without
leaving the game. An **in-game overlay** (rendered by the frontend shell) exposes
the comms UI while the Bevy game is running in the background.

The SDK surface for this is the `platform::comms` module.

---

## How it works end to end

```
1. Game calls platform::comms::join_session(session_id)

2. Backend creates (or reuses) a voice_room + text channel
   tied to the game session.

3. Backend returns CommsSession { channel_id, voice_room_id }

4. SDK opens the WS comms connection:
   • subscribes to chat.message events for channel_id
   • joins voice_room_id (voice.join → SDP/ICE signaling)

5. Game overlay (frontend) renders:
   • chat panel (text channel)
   • voice panel (participant list + mute button)

6. Player disconnects or session ends:
   • platform::comms::leave_session(session_id) called by SDK
   • Backend removes voice_participant row; GC's empty rooms
```

---

## SDK surface — `platform::comms`

> These types are the **designed API surface** for Wave 6. Implementation lives in
> the `magnetite-sdk` crate under `src/platform/comms.rs`.

### `CommsHandle`

The primary entry point. Obtained by calling `platform::comms::connect()` in your
Bevy plugin or game initialization code.

```rust
use magnetite_sdk::platform::comms::{CommsHandle, CommsConfig};

let config = CommsConfig {
    session_id: my_match_id.to_string(),
    auth_token: platform_token.clone(),
    backend_url: "wss://api.magnetite.gg".into(),
};

let comms: CommsHandle = platform::comms::connect(config).await?;
```

### `CommsHandle` methods

```rust
impl CommsHandle {
    /// Send a chat message to the session's text channel.
    pub async fn send_message(&self, content: &str) -> Result<MessageId>;

    /// Subscribe to incoming chat messages.
    /// The callback is invoked on the Tokio runtime; forward to Bevy via a channel.
    pub fn on_message<F>(&self, callback: F)
    where
        F: Fn(ChatMessage) + Send + Sync + 'static;

    /// Mute or unmute the local microphone.
    pub async fn set_muted(&self, muted: bool) -> Result<()>;

    /// Subscribe to voice participant updates (join, leave, mute changes).
    pub fn on_voice_update<F>(&self, callback: F)
    where
        F: Fn(VoiceUpdate) + Send + Sync + 'static;

    /// Retrieve the current list of voice participants.
    pub async fn participants(&self) -> Result<Vec<Participant>>;

    /// Cleanly leave both the text channel and voice room.
    pub async fn leave(self) -> Result<()>;
}
```

### Key types

```rust
pub struct CommsConfig {
    pub session_id:  String,   // lobby / match UUID
    pub auth_token:  String,   // JWT from platform::auth
    pub backend_url: String,   // wss://… — defaults to the platform base URL
}

pub struct ChatMessage {
    pub message_id: String,
    pub author_id:  String,
    pub username:   String,
    pub content:    String,
    pub timestamp:  chrono::DateTime<chrono::Utc>,
}

pub struct Participant {
    pub user_id:  String,
    pub username: String,
    pub muted:    bool,
}

pub enum VoiceUpdate {
    Joined(Participant),
    Left   { user_id: String },
    Muted  { user_id: String, muted: bool },
}

pub type MessageId = String;
```

### Error type

```rust
pub enum CommsError {
    NotConnected,
    Unauthorized,
    SessionNotFound,
    Transport(String),
}
```

---

## Integration example — Bevy plugin

```rust
use bevy::prelude::*;
use magnetite_sdk::platform::comms::{CommsHandle, CommsConfig, ChatMessage, VoiceUpdate};

pub struct InGameCommsPlugin;

#[derive(Resource)]
struct CommsResource(CommsHandle);

#[derive(Event)]
struct IncomingChat(ChatMessage);

#[derive(Event)]
struct VoiceChanged(VoiceUpdate);

impl Plugin for InGameCommsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<IncomingChat>()
           .add_event::<VoiceChanged>()
           .add_systems(Startup, setup_comms)
           .add_systems(Update, (display_chat, update_voice_ui));
    }
}

fn setup_comms(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    session: Res<GameSession>,
    auth: Res<PlatformAuth>,
) {
    let handle = runtime.block_on(async {
        platform::comms::connect(CommsConfig {
            session_id: session.id.clone(),
            auth_token:  auth.token.clone(),
            backend_url: "wss://api.magnetite.gg".into(),
        })
        .await
        .expect("comms connect failed")
    });

    // Forward incoming events into Bevy's event system.
    let (tx_chat, rx_chat) = std::sync::mpsc::channel();
    handle.on_message(move |msg| { let _ = tx_chat.send(msg); });

    commands.insert_resource(CommsResource(handle));
    // … store rx_chat in a resource; poll it in an Update system
}

fn display_chat(
    comms: Res<CommsResource>,
    mut events: EventWriter<IncomingChat>,
    // … poll the channel receiver …
) {
    // emit IncomingChat events for UI rendering
}

fn update_voice_ui(
    // read VoiceChanged events, update participant list UI
) {}
```

---

## Lobby vs. match provisioning

| Scenario | Behavior |
|----------|---------|
| **Lobby** | `join_session` called when the lobby is created; all invited players auto-join the voice+text room. Room persists until the lobby closes. |
| **Match** | `join_session` called when the matchmaker assigns seats; room is linked to `match_id` instead of `lobby_id`. Room is GC'd when all players leave or the match reports `game_over`. |
| **Spectator** | Spectators join the text channel only; they do not join the voice room unless explicitly invited. |

---

## In-game overlay (frontend)

The React frontend renders a lightweight **comms overlay** while a game is active.
It is a separate React subtree mounted over the game canvas — not rendered by Bevy —
so it works for both WASM browser games and native clients that embed the platform
shell.

The overlay includes:

- **Chat panel** — scrollable message history + text input. Collapses to a badge
  count when minimized.
- **Voice panel** — participant list with speaking indicators and per-user mute
  buttons (local). A prominent self-mute toggle is always visible.
- **Presence indicator** — shows which friends are spectating or in a different match.

The overlay communicates with the backend through the same `wss://` connection used
by the SDK — it subscribes to the same `channel_id` and `voice_room_id` that the
SDK handed the game.

---

## External streaming from in-game

A player can **go live** from within the match. The overlay provides a "Go Live"
button that:

1. Calls `POST /api/v1/comms/streams` with `voice_room_id` + optional `title` and
   external RTMP stream key.
2. Backend creates the `streams` row and returns `watch_url`.
3. The overlay shows the watch URL and a stop-streaming button.
4. Community members see the `stream.started` WS event and can tune in without
   joining the game.

---

## See also

- [Comms Overview](./index.md)
- [Realtime Protocol](./realtime.md)
- [Data Model](./data-model.md)
- [SDK Reference](../for-developers/sdk.md)
- [Architecture Overview](../architecture.md)
