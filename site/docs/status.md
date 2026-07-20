# Status — what actually runs today

Magnetite is mid-conversion from a conventional central-backend game platform to
the no-central-cloud design in `DECENTRALIZATION.md`. That spec describes the
**intent**. This page describes the **state**, audited against the tree rather
than against the pitch, so that nothing elsewhere in these docs reads as more
finished than it is.

## Working

| Capability | What it is |
|---|---|
| Authoritative simulation | `AuthoritativeGame` — deterministic `validate` / `step` in the SDK. |
| WASM sandbox | Wasmtime, `wasm32-wasip1`, fuel budget, memory cap, epoch interrupt. No OS randomness and no wall clock inside the guest. |
| Replay verification | `ReplayLog` + `verify_replay`. Anyone re-simulates from scratch and proves tampering. |
| Anti-cheat | Composable validator chain, per-player trust scoring, warn → kick → ban escalation. |
| Zero-backend dev loop | `magnetite dev` builds to wasm and serves a live match with no server, no database and no account. |
| Seam crate | Every seam is a trait with a default that needs no external service, so the suite runs fully offline. |
| Content-addressed games | Game id is the hash of wasm + manifest; loading verifies the hash and fails closed. |
| Capacity-elastic node | The node measures its own cores and RAM and derives its shard and player budget from them — never a config constant. |
| Signed discovery | Nodes sign and lease their own `SessionAd`s, TTL-capped, over LAN and ordinary HTTP trackers, fanned out redundantly. |
| Comms adapters | Matrix, Jitsi, LiveKit and Owncast behind one `CommsProvider` trait, with the old in-house stack demoted to a fallback. |

## Working, but LAN-only

Cross-node shard migration and cluster membership are real, tested code, not
scaffolding — a two-phase, epoch-fenced handoff where every partial failure
resolves to "the source keeps authority", a deny-by-default operator allowlist,
and signed single-use redirects that move players with their shard.

They have been exercised **in-process and over a LAN only**. There is no NAT
traversal, no relay and no WAN validation, so nodes must already be directly
reachable at their advertised address. **Internet-scale fleets between
strangers' machines are not demonstrated.** Plan accordingly.

## Not built

- **On-chain payments.** The `PaymentRail` seam models atomic wallet-to-wallet
  splits, signed-receipt entitlements, hosting channels and wager escrow. What
  ships is `MockPaymentRail`, a deterministic offline mock that signs receipts so
  CI can run without a network. No chain is wired up and no real payment has ever
  settled through it.
- **Trackerless discovery.** A DHT adapter is specified and unwritten.
- **Sensor input producers.** The `InputProvider` seam accepts and screens
  client-attested events, but nothing in magnetite *generates* one. There is no
  camera capture, no pose model and no vendor SDK anywhere in the tree. See
  [the seams page](#seams) for why that boundary is drawn explicitly.
- **Full removal of the pre-redesign backend.** Fiat billing, custody, balances
  and payout queues are gone. Parts of the older central API are still present.

## Screenshots

There are none. Magnetite's surfaces are a CLI and a library, and no product
screenshots have been captured — so none have been invented for the landing page
or these docs. The diagrams you see are diagrams.
