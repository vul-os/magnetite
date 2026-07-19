# Overview

**A game is a content-addressed portable object. A node is generic compute that
fills its own hardware. The chain is the wallet. Discovery is a phonebook, not
an authority. Everything social — chat, voice, video, streaming — is a
pluggable integration, not something we build.**

Magnetite is a decentralized, self-hostable Rust game platform. There is no
central cloud: anyone runs the single `magnetite` node binary. Identity is a
keypair. Payments are non-custodial crypto. Comms are provided by existing
decentralized systems (Matrix/Element, Jitsi, LiveKit, Owncast/PeerTube)
through one adapter seam. The game runtime — authoritative simulation, WASM
sandbox, deterministic replay and anti-cheat — is the one thing Magnetite owns
outright, and it's the part that was decentralization-ready from day one.

## The moat: one Rust game, jam to AAA

Write your game once against `magnetite-sdk::authority::AuthoritativeGame`.
The platform runs it at any scale by escalating topology, not by rewriting
game code:

| Topology | Player count | How |
|----------|-------------|-----|
| `SingleRoom` | up to ~16 | one process, broadcast-all |
| `Dedicated` | up to ~256 | authoritative server, interest-filtered snapshots |
| `Sharded` | AAA / unbounded | spatial shards + cross-shard handoff |

- **Wasmtime sandbox** — game logic compiles to `wasm32-wasip1` and runs with a
  fuel budget, a memory cap, and an epoch-interrupt wall clock. No OS
  randomness, no wall clock inside the guest.
- **Replay-verified anti-cheat** — the server is authoritative; clients send
  inputs, never state. Every tick's inputs and state hash land in a
  `ReplayLog`; `verify_replay` re-simulates from scratch and proves tampering.
- **One-command pipeline** — `magnetite new` / `build` / `dev` / `deploy`
  already take a game from a fresh scaffold to a live, playable instance with
  zero backend required for local development.

## What changes: decentralization

The parts that used to require a central server — identity, payments,
discovery, chat/voice/streaming — are being pulled out from behind one
authority (a database row, a JWT secret, a central registry) and put behind
**pluggable seams**. Every seam ships a working, non-custodial, non-cloud
default so the platform never hard-depends on any external network or
service:

- **Identity / Auth** — raw Ed25519 keypair by default; any external identity
  provider plugs in behind the trait.
- **Naming** — a display layer over raw keys; never the substrate itself.
- **BlobStore** — games and assets are content-addressed; the hash *is* the id.
- **Discovery** — a dumb, swappable tracker (anyone runs one) plus LAN
  discovery; never a central authority.
- **CommsProvider** — chat/voice/video/streaming are adapters over existing
  decentralized systems, not something Magnetite builds or operates.
- **PaymentRail** — non-custodial, wallet-to-wallet crypto. No balances, no
  payouts, no custody.

See [Architecture](architecture.md) for how the seams fit together, and
[Hosting a server](hosting-a-server.md) for the capacity-elastic node model —
bring any box, and it scales to your hardware.
