<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>

# Playing a game

This page walks through what a player actually sees, using the real app in
this repository (the screens below are the reference client, captured
against deterministic mock data — see [Screenshots](#screenshots)).

## 1. Browse the catalogue

The marketplace lists games by content-addressed identity: each entry is
keyed by the BLAKE3 hash of its wasm module plus manifest, not a database row
a central authority could quietly swap. Every game carries a content rating
set by its developer.

## 2. Find a server

Open a game and the client asks the discovery layer — a phonebook, never an
authority — for sessions currently advertising that game's hash. Each row in
the server browser shows the node's own signed, self-declared capacity
(cores, RAM, shard ceiling) and its hosting price. **Today this works over
LAN and against a self-run HTTP tracker.** Cross-operator discovery over the
open internet is specified but not built — see the
[status ledger](#status) for exactly what that means.

## 3. What determinism buys you as a player

The host runs your inputs through a WASM sandbox and steps the world; your
client never gets to assert its own position, hit, or score. Every match
produces a `ReplayLog` that anyone — not just the host — can re-simulate from
scratch and compare hashes against. If someone accuses a match of being
rigged, that is a question with a checkable answer instead of a moderator's
guess. See [Overview](#overview) and [Architecture](#architecture) for how
the sandbox and replay verifier work.

## 4. Paying for something

If a game or item has a price, checkout is wallet-to-wallet through the
`PaymentRail` seam: the platform holds no balance and is never in the money
path. What your wallet screen shows is **receipts** — each one independently
verifiable against the rail's signing key — not a balance, because there is
no balance to show. **The rail that ships today is a deterministic offline
mock** that signs receipts so the whole flow works with no network and no
real money moves. See [Payments](#payments) and
[Economy & Marketplace](#economy-marketplace) for the full picture,
including what a real chain rail would still need.

## 5. Comms

Chat, voice, and video are not something this project builds. A session may
point you at Matrix, Jitsi, LiveKit, or Owncast depending on what the host
configured; a small built-in fallback covers the rest. See [Comms](#comms).

## What this does not cover yet

- **Playing with strangers over the open internet.** Multi-node operation is
  proven on a LAN; there is no NAT traversal or relay yet, so a server you
  find must already be directly reachable.
- **Camera/gesture input.** The attested-input wire exists and is exercised
  by [wibbly](https://github.com/vul-os/wibbly), a separate camera-gesture
  game — no game in this repository consumes that input today.

See the [status ledger](#status) for the authoritative, line-by-line account
of what is running versus specified.
