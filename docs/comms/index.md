# Comms — Overview

Magnetite ships a **Discord-class communications suite** as a first-class platform
service. Every game can plug into it without building its own chat or voice stack:
servers (communities), channels, real-time text chat, member presence, voice rooms,
and game streaming all come pre-wired.

---

## Pillars

| Pillar | Transport | Scale path |
|--------|-----------|------------|
| **Text chat** | WebSocket (Axum WS layer) | Horizontal pod replication + shared broadcast |
| **Presence** | WebSocket heartbeat + server-side sweep | Same WS layer |
| **Voice** | WebRTC mesh (backend as signaling server, SDP/ICE over WS) | Drop-in SFU (LiveKit / mediasoup) |
| **Streaming** | In-platform watch via HLS/WebRTC; RTMP egress to Twitch/YouTube | CDN-backed media server |

---

## Concept hierarchy

```
Community (server / guild)
  └─ Channels
        ├─ Text channels  →  Messages
        ├─ Voice rooms    →  Voice participants (WebRTC peers)
        └─ (future) Announcement / Forum channels

Direct Messages (DMs)
  └─ Virtual 1-to-1 channel, same message store

Voice rooms
  └─ Voice participants
  └─ (optional) Stream — go live / watch
```

---

## Communities (servers / guilds)

A **community** is a persistent space — think Discord server or Slack workspace —
that groups together channels, members, roles, and permissions.

Key properties:

- `id`, `name`, `description`, `icon_url`, `owner_id`
- `created_at`, `updated_at`
- Membership table (`channel_members`) holds `user_id`, `role` (`owner | admin | member`),
  and `joined_at`.

See [data-model.md](./data-model.md) for the full schema.

---

## Channels

A channel belongs to a community and has a **kind** (`text` or `voice`). Future kinds
(`announcement`, `forum`) are reserved in the schema.

- Text channels accumulate **Messages**.
- Voice channels maintain a **VoiceRoom** with zero-or-more live participants.

---

## Text chat

Messages are persisted in the `messages` table. They are delivered in real time over
the Axum WebSocket layer — the same connection already used for game-state and
notification streaming.

WebSocket message shape:

```json
{ "type": "chat.message",
  "channel_id": "uuid",
  "author_id": "uuid",
  "content": "gg",
  "nonce": "client-dedup-id",
  "timestamp": "2026-05-30T12:34:56Z" }
```

The server writes the row to Postgres and fans out the event to all channel
subscribers over the broadcast channel. Clients that miss messages while
disconnected re-sync via `GET /api/v1/comms/channels/:channel_id/messages`.

Full protocol details: [realtime.md — Chat protocol](./realtime.md#text-chat-protocol).

---

## Presence

Presence tracks whether a user is **online**, **in-game** (tied to a match/lobby ID),
**streaming**, or **offline**. It is maintained by the WS connection:

- Connection open → presence mark `online` (or `in_game` if joined via SDK).
- Heartbeat (every 30 s) → server resets the expiry timer.
- Connection close or heartbeat timeout → server marks `offline` and broadcasts
  `presence.update` to subscribers.

Full flow: [realtime.md — Presence](./realtime.md#presence).

---

## Voice

Voice is **real-time audio** among room participants. Magnetite uses **WebRTC** for the
actual media path; the backend acts as the **signaling server**, relaying SDP offers/
answers and ICE candidates over the existing WebSocket connection.

For small rooms (up to ~15 participants) the peers form a **mesh** — each client sends
its audio track to every other peer. At larger scale the architecture documents an SFU
(Selective Forwarding Unit — e.g. LiveKit or mediasoup) as the production upgrade path:
the signaling protocol remains unchanged; only the peer the client negotiates with changes
from "other participant" to "SFU endpoint".

Full signaling flow: [realtime.md — Voice & WebRTC signaling](./realtime.md#voice--webrtc-signaling).

---

## Streaming (go live / watch)

A user in a voice room can **go live**, which:

1. Creates a `streams` row linked to their `voice_room_id`.
2. Optionally starts an **RTMP egress** relay to an external service
   (Twitch / YouTube — developer configures stream key in settings).
3. Exposes an **HLS / WebRTC watch URL** so other community members can
   watch in-platform without joining the voice room.

Viewers connect to the watch URL; the backend (or CDN) serves the HLS playlist.
Heavy media infrastructure (CDN transcoding, RTMP ingest relay) is documented as
the scale path and is not required for local or self-hosted deployments.

---

## In-game usage

When a game session (lobby or match) is created, Magnetite **auto-provisions** a paired
voice+text room for participants. The SDK surface is `platform::comms`.

Full integration guide: [in-game.md](./in-game.md).

---

## REST API surface

The comms REST API is prefixed at `/api/v1/comms/` and covers communities, channels,
members, and message history. It follows the same request/response envelope as every
other Magnetite API module:

```json
{ "success": true, "data": { … } }
```

Key endpoint groups:

| Group | Prefix |
|-------|--------|
| Communities | `GET/POST /communities` |
| Community detail | `GET/PUT/DELETE /communities/:id` |
| Members | `GET /communities/:id/members`, `POST /communities/:id/join`, `DELETE /communities/:id/leave` |
| Channels | `GET/POST /communities/:id/channels` |
| Messages | `GET/POST /communities/:id/channels/:cid/messages` |
| Voice rooms | `GET /voice-rooms/:id`, `POST /voice-rooms/:id/join`, `DELETE /voice-rooms/:id/leave` |
| Streams | `GET /streams/:id`, `POST /streams` (go live), `DELETE /streams/:id` |
| Presence | `GET /presence/:user_id` |

See [realtime.md](./realtime.md) for the WebSocket event types layered on top.

---

## See also

- [Realtime Protocol](./realtime.md) — WS chat/presence protocol + WebRTC voice signaling
- [Data Model](./data-model.md) — communities / channels / messages / voice_rooms / streams
- [In-Game Usage](./in-game.md) — SDK `platform::comms` for lobby/match chat+voice
- [Architecture Overview](../architecture.md)
- [SDK Reference](../for-developers/sdk.md)
