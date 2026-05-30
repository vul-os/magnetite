# Streaming — Go Live & Watch

Magnetite supports **game streaming** as a first-class platform feature. Any player can go
live from a voice room, sharing their screen or game capture; other players can watch
in-platform or via an external service (Twitch, YouTube).

---

## How it works

```
Streamer                    Backend                    Viewers
   │                           │                          │
   │  POST /streams (go live)  │                          │
   │──────────────────────────►│  INSERT streams row      │
   │                           │  status = 'live'         │
   │                           │                          │
   │  GoLivePanel              │                          │
   │  getDisplayMedia()        │                          │
   │  (screen / tab capture)   │                          │
   │                           │                          │
   │  WebRTC offer (to backend)│                          │
   │──────────────────────────►│                          │
   │                           │  Relay to viewers        │
   │                           │─────────────────────────►│
   │                           │                          │
   │  [Optional] RTMP egress   │                          │
   │  rtmp://live.twitch.tv/…  │◄─────────────────────────│
   │  (via configured relay)   │                          │
   │                           │                          │
   │  DELETE /streams/:id      │  status = 'ended'        │
   │──────────────────────────►│                          │
```

---

## Data model

The `streams` table (migration `20260530_communities.sql`) tracks each streaming session:

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `streamer_id` | UUID | References `users.id` |
| `community_id` | UUID | Optional — community this stream is associated with |
| `channel_id` | UUID | Optional — voice/stream channel |
| `title` | TEXT | Stream title |
| `game_id` | UUID | Optional — game being played |
| `status` | TEXT | `offline` / `live` / `ended` |
| `viewer_count` | INTEGER | Live viewer count |
| `hls_url` | TEXT | HLS playlist URL for in-platform watch |
| `rtmp_key` | TEXT | RTMP stream key (streamer only) |
| `external_rtmp_url` | TEXT | RTMP destination (Twitch / YouTube ingest) |
| `started_at` | TIMESTAMPTZ | When the stream went live |
| `ended_at` | TIMESTAMPTZ | When the stream ended |

---

## REST API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/comms/streams` | Browse live and recent streams |
| `GET` | `/api/comms/streams/:id` | Stream detail (title, status, viewer_count, hls_url) |
| `POST` | `/api/comms/streams` | Go live (creates `streams` row; returns stream ID + RTMP key) |
| `DELETE` | `/api/comms/streams/:id` | End stream (sets `status = 'ended'`, records `ended_at`) |

All write endpoints require `Authorization: Bearer <token>`.

---

## Frontend components

### `Streams.jsx` (`/streams`)

Browse page: a grid of `StreamCard` components showing live and recent streams.
Each card links to the stream detail / player view.

### `StreamPlayer`

Embeds the stream playback. Uses:
- **HLS** (via `video` + native HLS or `hls.js`) when `hls_url` is available.
- **WebRTC** (direct peer connection) as the low-latency in-platform watch path for small rooms.

### `GoLivePanel`

The streamer UI:

1. **Capture**: calls `navigator.mediaDevices.getDisplayMedia({ video: true, audio: true })`
   to capture a tab, window, or screen.
2. **Go live**: calls `POST /api/comms/streams`; receives `stream_id` and `rtmp_key`.
3. **RTMP egress**: if an external RTMP URL (Twitch / YouTube ingest) is configured in
   the streamer's settings, the panel displays the RTMP key for use with OBS or a relay.
4. **End stream**: calls `DELETE /api/comms/streams/:id` and stops all tracks.

```jsx
import GoLivePanel from '../components/streaming/GoLivePanel';

// Inside a Communities or VoiceRoom page:
<GoLivePanel channelId={channel.id} onStreamStarted={(stream) => setActiveStream(stream)} />
```

---

## RTMP egress (external streaming)

To stream to Twitch or YouTube:

1. The streamer configures their **stream key** in Magnetite settings.
2. When going live, `GoLivePanel` records the external RTMP URL in the `streams` row.
3. An RTMP relay (e.g. nginx-rtmp, stunnel, or a CDN relay) forwards the media
   from the Magnetite ingest to the external destination.

The Magnetite backend does **not** itself transcode video — it stores configuration and
coordinates metadata. A production deployment uses a CDN-backed RTMP relay (the scale path
documented in the architecture).

---

## In-game streaming

Games that use the `platform::comms` SDK module automatically appear in the streams browser
when the player goes live. The SDK sets `game_id` on the `streams` row, so the stream shows
up in that game's community channel and in platform-wide browse.

---

## Scale path

| Current (foundation) | Scale path |
|----------------------|------------|
| WebRTC mesh relay (backend as signaling) | LiveKit / mediasoup SFU |
| HLS from streamer's browser | CDN-backed RTMP ingest + adaptive bitrate transcoding |
| Single-replica broadcast | Redis Pub/Sub + multi-region fan-out |

---

## SDK (`platform::streaming`)

The SDK exposes a `StreamClient` for game servers and in-game code:

```rust
use magnetite_sdk::platform::streaming::{
    ExternalRtmpTarget, GoLiveRequest, StreamClient, StreamConfig,
};

let client = StreamClient::new(StreamConfig { … });

// Start a stream from a match/lobby.
let stream = client.go_live(GoLiveRequest {
    title: "Ranked Match #4212".to_string(),
    game_id: Some(GAME_ID),
    external_rtmp: Some(ExternalRtmpTarget {
        url: "rtmp://live.twitch.tv/app".to_string(),
        key: player_settings.twitch_key.clone(),
    }),
}).await?;

// End the stream.
client.end_stream(stream.id).await?;
```

Key types: `StreamClient`, `StreamConfig`, `GoLiveRequest`, `StreamInfo`, `StreamStatus`,
`ExternalRtmpTarget`, `StreamId`, `StreamEvent`, `ClientStreamMessage`, `ServerStreamMessage`,
`StreamErrorCode`.

---

## See also

- [Comms Overview](./index.md)
- [Realtime Protocol](./realtime.md) — WebRTC signaling details
- [In-Game Usage](./in-game.md) — auto-provisioned stream rooms per match
- [Architecture Overview](../architecture.md)
