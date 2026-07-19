# Getting started

Magnetite is one binary and one SDK. You do not need a database, a cloud
account, or a payment provider to write and play a game locally.

## Install

```bash
cargo install magnetite-cli
```

Requires a recent stable Rust toolchain with the `wasm32-wasip1` target:

```bash
rustup target add wasm32-wasip1
```

## Scaffold a game

```bash
magnetite new my-game
cd my-game
```

This generates a crate implementing `AuthoritativeGame` from
`magnetite-sdk::authority` — the frozen trait boundary for deterministic
`validate`/`step` game logic. See [`game-template-authoritative/`](../game-template-authoritative/)
for the canonical reference implementation (a top-down arena shooter).

## Build

```bash
magnetite build
```

Compiles your game to `wasm32-wasip1` and produces `game.wasm`. The artifact
already carries a sha256 hash — it's content-addressable from the moment it's
built (see [Architecture](architecture.md#blobstore)).

## Run it — zero backend

```bash
magnetite dev
```

`magnetite dev` builds your game, boots a `WasmExecutor` inside a sandboxed
`SingleRoom` server, and serves it at `ws://127.0.0.1:<port>`. No database, no
Postgres, no Redis, no cloud account. Connect with
[`magnetite-web-client`](../magnetite-web-client/) or the Bevy client in
[`game-client-bevy/`](../game-client-bevy/).

## Bring a server

```bash
magnetite node                                   # LAN discovery, zero config
TRACKER_URL=https://tracker.example.org magnetite node   # also announce to a tracker
```

Point the node at hardware you actually own. It measures its own cores, RAM,
and bandwidth, content-addresses the game (BLAKE3), verifies the hash before
executing it, and advertises what it can hold. With no `TRACKER_URL` set it
uses `LanDiscovery` (mDNS) and needs nothing external at all. Player capacity
is emergent from the box — not a config constant you have to guess at. See
[Hosting a server](hosting-a-server.md) for the full capacity-elastic model.

## Next

- [Architecture](architecture.md) — the seams and how they compose
- [Payments](payments.md) — non-custodial checkout, hosting fees, wagers
- [Comms](comms.md) — pluggable chat/voice/video/streaming
- [Screenshots](screenshots.md) — the docs/landing gallery
