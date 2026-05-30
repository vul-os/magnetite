# Realtime Protocol

This page covers:

1. The **WebSocket chat and presence protocol** — how messages are sent and
   received in text channels, and how presence state is tracked.
2. The **WebRTC voice signaling flow** — how the backend acts as a signaling
   server to set up peer-to-peer audio connections between room participants.

---

## WebSocket transport

All comms real-time events travel over the **same Axum WebSocket connection** that
the platform already uses for game state and notifications. The connection URL is:

```
wss://<host>/ws/notifications
```

After authentication (JWT Bearer sent as a query parameter or initial handshake
message) the server registers the connection to the authenticated user. Every
subsequent message is a JSON envelope:

```json
{
  "type": "<event-type>",
  "payload": { … }
}
```

The `type` field is a dot-separated namespace — `chat.*`, `presence.*`, `voice.*`,
`stream.*`.

---

## Text chat protocol

### Sending a message (client → server)

```json
{
  "type": "chat.send",
  "payload": {
    "channel_id": "019...",
    "content": "gg wp",
    "nonce": "client-generated-dedup-id"
  }
}
```

`nonce` is an opaque client string (UUID v4 recommended) that lets the client
detect its own echo and avoid displaying the message twice.

### Message confirmed (server → client)

Sent back to the **sender** only:

```json
{
  "type": "chat.ack",
  "payload": {
    "nonce": "client-generated-dedup-id",
    "message_id": "019...",
    "timestamp": "2026-05-30T12:00:00Z"
  }
}
```

### Message broadcast (server → channel subscribers)

Sent to **all other** subscribers of the channel (and to the sender after ACK):

```json
{
  "type": "chat.message",
  "payload": {
    "message_id": "019...",
    "channel_id": "019...",
    "author_id": "019...",
    "author_username": "dragonfly",
    "content": "gg wp",
    "timestamp": "2026-05-30T12:00:00Z"
  }
}
```

### Message history (REST fallback)

Clients that reconnect or first load a channel fetch history over REST:

```
GET /api/v1/comms/channels/:channel_id/messages?before=<message_id>&limit=50
```

Response:

```json
{
  "success": true,
  "data": {
    "messages": [ { "message_id": "…", "author_id": "…", "content": "…", "timestamp": "…" } ],
    "has_more": true
  }
}
```

### Channel subscription

Clients subscribe to a channel over WS before receiving real-time events:

```json
{ "type": "chat.subscribe", "payload": { "channel_id": "019..." } }
```

Server confirms:

```json
{ "type": "chat.subscribed", "payload": { "channel_id": "019..." } }
```

Unsubscribe:

```json
{ "type": "chat.unsubscribe", "payload": { "channel_id": "019..." } }
```

---

## Presence

Presence tracks the live state of a user:

| Status | Meaning |
|--------|---------|
| `online` | WS connection open; not in a game |
| `in_game` | WS open; actively in a match (lobby or live session) |
| `streaming` | WS open; actively broadcasting a stream |
| `offline` | WS closed or heartbeat timed out |

### Heartbeat

The client sends a heartbeat every **30 seconds**:

```json
{ "type": "presence.heartbeat" }
```

The server resets the user's expiry timer on each received heartbeat. If the timer
exceeds 45 seconds without a heartbeat, the server marks the user `offline`.

### Presence update broadcast

Whenever a user's presence changes, the server broadcasts to all users who share
at least one community with that user:

```json
{
  "type": "presence.update",
  "payload": {
    "user_id": "019...",
    "status": "in_game",
    "game_id": "019...",         // present when status == "in_game"
    "session_id": "019...",      // present when status == "in_game"
    "stream_id": "019..."        // present when status == "streaming"
  }
}
```

### Querying presence (REST)

```
GET /api/v1/comms/presence/:user_id
```

Returns the current `status` and any associated IDs. Useful for first-load before
WS events arrive.

---

## Voice — WebRTC signaling

Voice is **real-time peer-to-peer audio** using **WebRTC**. The backend does **not**
handle media itself; it acts as a **signaling server** that routes SDP and ICE
messages between peers over the established WebSocket connection.

### Overview of the signaling flow

```
Peer A                  Backend (signaling)          Peer B
  │                           │                         │
  │── voice.join ────────────►│                         │
  │                           │── voice.peer_joined ───►│
  │                           │◄── voice.join ──────────│
  │                           │                         │
  │  [Peer A creates RTCPeerConnection, makes offer]    │
  │── voice.offer ───────────►│                         │
  │  { sdp, to: Peer B }      │── voice.offer ─────────►│
  │                           │   { sdp, from: Peer A } │
  │                           │                         │
  │                           │  [Peer B creates answer]│
  │                           │◄── voice.answer ────────│
  │◄── voice.answer ──────────│   { sdp, from: Peer B } │
  │                           │                         │
  │  [ICE trickle begins]     │                         │
  │── voice.ice ─────────────►│── voice.ice ───────────►│
  │◄── voice.ice ─────────────│◄── voice.ice ───────────│
  │                           │                         │
  │  [Direct P2P audio established — no backend media]  │
  │◄══════════════════════ WebRTC audio ═══════════════►│
```

The backend is a **pure relay** for SDP and ICE. Audio bytes flow directly
between peers (or through a TURN server if NAT traversal requires it).

### Step 1 — Join a voice room

Client sends:

```json
{
  "type": "voice.join",
  "payload": { "voice_room_id": "019..." }
}
```

Server:

1. Inserts a `voice_participants` row for the user.
2. Responds to the joiner with the current participant list:

```json
{
  "type": "voice.joined",
  "payload": {
    "voice_room_id": "019...",
    "participants": [
      { "user_id": "019...", "username": "dragonfly", "muted": false }
    ]
  }
}
```

3. Broadcasts to existing participants:

```json
{
  "type": "voice.peer_joined",
  "payload": { "user_id": "019...", "username": "iron_wolf" }
}
```

The joiner is expected to initiate a WebRTC offer with **every existing participant**.

### Step 2 — SDP offer (initiator → responder, via backend)

```json
{
  "type": "voice.offer",
  "payload": {
    "voice_room_id": "019...",
    "to": "019...-user-id",
    "sdp": "v=0\r\no=- …"
  }
}
```

Backend relays to the `to` user:

```json
{
  "type": "voice.offer",
  "payload": {
    "voice_room_id": "019...",
    "from": "019...-user-id",
    "sdp": "v=0\r\no=- …"
  }
}
```

### Step 3 — SDP answer

```json
{
  "type": "voice.answer",
  "payload": {
    "voice_room_id": "019...",
    "to": "019...-user-id",
    "sdp": "v=0\r\no=- …"
  }
}
```

Backend relays with `to` replaced by `from`.

### Step 4 — ICE candidates (trickle ICE)

Both peers send candidates as they are discovered by the browser's ICE agent:

```json
{
  "type": "voice.ice",
  "payload": {
    "voice_room_id": "019...",
    "to": "019...-user-id",
    "candidate": "candidate:… UDP …"
  }
}
```

Backend relays as above. The exchange continues until both sides have established
a direct (or TURN-relayed) connection.

### Step 5 — Peer leaves

Client sends:

```json
{ "type": "voice.leave", "payload": { "voice_room_id": "019..." } }
```

Server removes the `voice_participants` row and broadcasts:

```json
{
  "type": "voice.peer_left",
  "payload": { "user_id": "019...", "voice_room_id": "019..." }
}
```

Remaining peers close the RTCPeerConnection toward the departed user.

### Mute state

Mute is a local browser decision (track enabled/disabled) but the server
tracks it for UI purposes:

```json
{ "type": "voice.mute",   "payload": { "voice_room_id": "019..." } }
{ "type": "voice.unmute", "payload": { "voice_room_id": "019..." } }
```

Server updates `voice_participants.muted` and broadcasts a `voice.mute_update`
event to all room participants.

---

## Scale path — SFU

The **mesh** topology above is efficient for small rooms (up to roughly 15
simultaneous speakers). Beyond that, each sender's uplink is a bottleneck.

The scale path is a **Selective Forwarding Unit (SFU)** — a media server that
receives one stream per sender and forwards selected streams to each receiver.
Candidates: **LiveKit** (Go, open source) or **mediasoup** (Node.js, open source).

The signaling protocol above does **not change**; only the peer identity changes:

- Mesh: `to` / `from` are participant `user_id` values.
- SFU: the backend introduces an `"sfu"` virtual peer; clients negotiate with the SFU
  endpoint instead. The SFU handles the fan-out.

Operator guide for deploying an SFU alongside Magnetite will be documented in the
self-hosting section when the SFU integration is productionized.

---

## Streaming events

When a user goes live, the backend emits:

```json
{
  "type": "stream.started",
  "payload": {
    "stream_id": "019...",
    "broadcaster_id": "019...",
    "voice_room_id": "019...",
    "watch_url": "https://…/streams/019.../hls/index.m3u8",
    "title": "Rust FPS speedrun"
  }
}
```

When the stream ends:

```json
{
  "type": "stream.ended",
  "payload": { "stream_id": "019..." }
}
```

---

## See also

- [Comms Overview](./index.md)
- [Data Model](./data-model.md)
- [In-Game Usage](./in-game.md)
- [Architecture Overview](../architecture.md)
