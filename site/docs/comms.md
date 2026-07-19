# Comms

**Chat, voice, video, and streaming are pluggable integrations. Magnetite
builds none of it.**

Every game gets a lobby and an in-match room, but the systems behind those
rooms are existing decentralized comms platforms, wired in behind one seam —
not a home-grown chat/voice/streaming stack Magnetite operates and maintains.

```rust
trait CommsProvider {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr;
    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred;
    async fn teardown(&self, room: &RoomAddr) -> Result<()>;
}
```

## Providers

| Provider | Covers |
|----------|--------|
| `MatrixProvider` | Text, DMs, presence, and spaces via Element homeservers |
| `JitsiProvider` | Voice + video SFU |
| `LiveKitProvider` | Voice + video at scale |
| `OwncastProvider` / `PeerTubeProvider` | Live streaming + VOD |
| `BuiltinProvider` | The demoted fallback shim — always available, never the primary path |
| `DmtapCommsProvider` (optional) | MOTE messaging over the DMTAP substrate |

Matrix, Jitsi, and LiveKit are the lead providers precisely because they
already exist, are already decentralized (or self-hostable), and already have
communities maintaining them. Magnetite's job is the adapter, not the
homeserver.

## One login, every room

The identity seam is what makes single-sign-on into third-party comms
possible without a separate account per system. The node acts as an identity
provider for the player's own keypair:

```rust
async fn mint_scoped_token(&self, pk: &PubKey, aud: Audience, scope: Scope) -> Token;
```

A Matrix OpenID token, a Jitsi JWT, a LiveKit token — all minted from the same
keypair the player already used to log into the game. There's no separate
Matrix account to create and no separate Jitsi password to remember.

## Paid rooms

A `JoinCred` can be gated behind a payment receipt (see [Payments](payments.md)):
a paid room only issues a join credential after `PaymentRail::verify_receipt`
succeeds. This is how ticketed tournaments, subscriber-only voice channels, or
paid watch-parties work without Magnetite ever touching the money itself —
the comms room just checks a signed receipt before letting someone in.

## Discovery carries room addresses

A `SessionAd` — the thing a node advertises to [Discovery](hosting-a-server.md#discovery-is-a-phonebook-not-a-gatekeeper)
— can carry optional `chat_room` and `voice_room` addresses alongside the game
session itself, so finding a match and finding its lobby chat happen in one
lookup.

## What this replaces

The old model ran its own Discord-class stack in-house: `communities` /
`channels` / `messages` tables, `ws/comms` and `ws/voice` WebSocket handlers,
and RTMP streaming egress through a self-hosted MediaMTX instance. That stack
isn't deleted — it's demoted to the `BuiltinProvider`, kept as one adapter
among many rather than the thing every game is forced to depend on.
