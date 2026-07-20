# Magnetite → Decentralized Games Platform (Redesign Spec + Program Backlog)

> **Status:** ACTIVE redesign. This document is the single source of truth for the
> decentralization program. Every agent/wave builds against the seams and backlog defined here.
> Do not invent parallel abstractions — implement the seams below.

## 0. Vision (one sentence)

**A game is a content-addressed portable object. A node is generic compute that fills its own
hardware. The chain is the wallet. Discovery is a phonebook, not an authority. Everything social
(chat/voice/video/streaming) is a pluggable integration, not something we build.**

No central cloud. Anyone runs the single `magnetite` node binary. Identity is a keypair. Payments
are non-custodial crypto. Comms are provided by existing decentralized systems (Matrix/Element,
Jitsi, LiveKit, Owncast/PeerTube) through one adapter seam. The game runtime (authoritative sim,
WASM sandbox, deterministic replay/anti-cheat) is the one thing we own and is already ~90% there.

## 1. What we KEEP (the moat — already decentralization-ready)

- `magnetite-sdk::authority::AuthoritativeGame` — deterministic `validate`/`step`.
- WASM sandbox (`magnetite-sandbox`) — same `(state, ordered cmds, tick, seed)` → same result anywhere.
- `ReplayLog` + `verify_replay` (`magnetite-anticheat`) — anyone re-simulates to prove tampering.
- Topology ladder `SingleRoom → Dedicated → Sharded` — but multi-node Sharded is unbuilt ("Bucket D").
- `magnetite dev` already runs a game with ZERO backend. `magnetite deploy` already takes an arbitrary URL.
- Artifacts already carry a sha256 → already content-addressable, just served by URL today.

## 2. What we CUT / DEMOTE

- **Fiat + custody:** Paystack, Wise, `wallet_balances`/`wallet_transactions`/`developer_balances`
  (the latter has no schema anyway), `payout`/`payout_requests` split-brain, ZAR→USD conversion,
  subscription ZAR charging. All deleted — non-custodial crypto makes custody unnecessary.
- **Central identity:** single `JWT_SECRET` + `users` table as the *only* identity authority →
  demoted to one `Identity` provider behind the seam. Keypair identity is the default.
- **Home-grown chat/voice/streaming:** `communities`/`channels`/`messages`/`ws/comms`/`ws/voice`/
  `streaming` + MediaMTX → demoted to the **`builtin` CommsProvider** (optional fallback). Lead with
  Matrix/Jitsi providers. Do NOT delete outright — keep as one adapter among many.
- **Central server registry:** `runtime_instances` rows + poll `/provisioning/pending` → replaced by
  self-advertising nodes + tracker/DHT discovery.

## 3. THE SEAMS (implement exactly these — everything plugs in behind them)

All seams live in a new crate `magnetite-seams` (traits + default impls). Nothing in the game
runtime, scheduler, or payment path may name a provider-specific type — they see only these traits.
**Every seam ships a working offline default so we never hard-depend on any external project.**

### 3.1 `Identity` / `Auth`
```rust
trait Identity {
    fn pubkey(&self) -> PubKey;                       // Ed25519
    fn sign(&self, msg: &[u8]) -> Sig;
    fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool;
}
trait AuthProvider {                                  // sign-a-challenge login
    async fn challenge(&self, pk: &PubKey) -> Challenge;
    async fn verify_login(&self, resp: LoginResponse) -> Result<Session>;
    // node acts as IdP: mint scoped, short-lived creds for external systems
    async fn mint_scoped_token(&self, pk: &PubKey, aud: Audience, scope: Scope) -> Token;
}
```
- **Default provider:** `RawKeypairAuth` — raw Ed25519 challenge/response. No external deps.
- Any external identity provider (OIDC bridge, a decentralized-login protocol) plugs in behind this
  trait as a feature-gated module, never referenced by non-provider code. None ships today.

### 3.2 `Naming`
```rust
trait Naming {
    async fn resolve(&self, name: &str) -> Option<PubKey>;   // human name → key
    fn display(&self, pk: &PubKey) -> String;                // key → human display
}
```
- **Default:** `HashNaming` — raw pubkey / short-hash addresses.
- **Optional:** `KeyNameNaming` (`--features keyname`) — word-based, zero-authority key-names.
  Zero dependencies; exists to prove this seam is genuinely swappable, not hardwired.
- **Rule:** human names are a *display layer* over raw keys. Substrate is always raw keys.

### 3.3 `BlobStore` (content-addressed games + assets)
```rust
trait BlobStore {
    async fn put(&self, bytes: &[u8]) -> Hash;        // BLAKE3/sha256; hash IS the id
    async fn get(&self, hash: &Hash) -> Option<Bytes>;
    async fn has(&self, hash: &Hash) -> bool;
}
```
- **Default:** `LocalBlobStore` + `HttpBlobStore` (serve by hash over HTTP). Iroh/BitTorrent adapter later.
- Game id = hash of (wasm module + manifest). No central registry row required to identify a game.

### 3.4 `Discovery` (the phonebook — never an authority)
```rust
trait Discovery {
    async fn announce(&self, session: SessionAd) -> Result<()>;   // node self-advertises
    async fn find(&self, game: Hash, filter: Filter) -> Vec<SessionAd>;
}
struct SessionAd { game: Hash, node: NodeAddr, capacity: Capacity, ping_hint: u32,
                   price: Option<Price>, chat_room: Option<RoomAddr>, voice_room: Option<RoomAddr> }
```
- **Default:** `TrackerDiscovery` — dumb, swappable HTTP tracker (BitTorrent-style; anyone runs one,
  redundant). Plus `LanDiscovery` (mDNS) for local. DHT adapter later.
- Replaces the central `runtime_instances`-poll model entirely.

### 3.5 `CommsProvider` (chat / voice / video / streaming — pluggable, we build none of it)
```rust
trait CommsProvider {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr;        // match / lobby / community
    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred;
    async fn teardown(&self, room: &RoomAddr) -> Result<()>;
}
```
- Providers: `MatrixProvider` (text/DMs/presence/spaces via Element homeservers),
  `JitsiProvider` (voice+video SFU), `LiveKitProvider` (voice+video at scale),
  `OwncastProvider`/`PeerTubeProvider` (live + VOD), `BuiltinProvider` (the demoted old stack).
- **Identity bridge:** the node mints scoped creds via `AuthProvider::mint_scoped_token`
  (Matrix OpenID/SSO, Jitsi JWT, LiveKit token) from the player's keypair. One login → SSO into comms.
- Join credential may be gated behind a payment receipt (§3.6) — paid room → token only after pay.

### 3.6 `PaymentRail` (non-custodial crypto — no balances, no payouts, no custody)
```rust
trait PaymentRail {
    async fn checkout(&self, buyer: &PubKey, split: PaymentSplit) -> Receipt; // atomic, wallet→wallet
    async fn open_channel(&self, peer: &PubKey) -> Channel;                   // micro-payments
    async fn escrow(&self, terms: WagerTerms) -> Escrow;                      // optional wagers
    fn verify_receipt(&self, r: &Receipt) -> bool;                            // signed / on-chain proof
}
struct PaymentSplit { developer: Split, operator: Option<Split>, protocol_fee_bps: u16 }
```
- Money flows: (a) **item/DLC purchase** → atomic split to developer wallet; entitlement =
  signed receipt keyed `(buyer pk, game hash, item)`; node reads receipt to grant. (b) **hosting fee**
  → operator paid per-seat/per-hour via payment channel (no gas per join); this is the incentive to
  bring big servers. (c) **wager/tournament (optional)** → escrow settled by `verify_replay`.
- **Chain:** stablecoin (USDC) on an L2 (Base/Arbitrum) + payment channels for micro-txns, OR Solana
  (Ed25519-native → identity key can double as wallet key). Keep on-chain state minimal. Configurable.
- **Default (dev/test):** `MockPaymentRail` — deterministic signed receipts, no chain, so CI runs offline.
- **Points/XP:** stay OFF-chain, signed per-game ledgers. Not money. Not tokenized by default.
- Protocol fee: checkout has a `protocol_fee_bps` param, **default 0** (decide later via governance).

### 3.7 `InputProvider` (where input comes from — and what it can be proven to be)

```rust
enum InputClass { Deterministic, Attested }   // the load-bearing type
trait InputProvider {
    fn class(&self) -> InputClass;                       // truthful, always
    async fn submit(&self, event: InputEvent) -> Result<()>;  // fails closed on class mismatch
    async fn drain(&self, now_ms: u64) -> Vec<InputEvent>;
    fn plausibility_limits(&self) -> Option<&PlausibilityLimits>;
}
```

- **Default:** `LocalDeviceInput` — a deterministic keyboard/gamepad-style queue. Offline,
  dependency-free, replay-verifiable. It **refuses attested events at runtime**, so the class
  boundary is enforced rather than merely documented.
- **The point of this seam is the boundary it draws.** Everything else here assumes deterministic
  input: `ReplayLog` + `verify_replay` (§1) can prove tampering only because replaying the same
  ordered commands from the same seed reproduces the same state. A camera-gesture stream (the
  reason this seam exists — see `wibbly/WIBBLY.md` §6) is a **nondeterministic sensor reading** and
  **cannot be replay-verified, at any point, ever**. It is *client-attested*: the client asserts
  what happened and the host decides whether to believe it.
- **What a host can actually do with attested input:** `PlausibilityGate` screens per player for
  rate, per-kind cooldown, human-reachable velocity, a confidence floor, timestamp sanity, and
  monotonic sequence numbers. Rejection means "not physically reachable". **Acceptance means
  nothing stronger than "not obviously impossible."** A cheater who synthesises *plausible* events
  is not detectable here and never will be — there is a test
  (`a_plausible_synthetic_event_is_indistinguishable_from_a_real_one`) asserting exactly that, so
  the limit stays written down in code.
- A `SignedAttestedEvent` signature proves **authorship, not truth**: it stops one player forging
  events in another's name and stops a relay editing them in flight. A cheater signs their own
  fabricated events with their own genuine key and passes every time.
- **Rule:** never settle a wager escrow (§3.6) or issue a competitive ranking from attested input
  on the strength of replay proof. `InputClass::is_replay_verifiable()` exists so that decision is
  made in code, not by a reader remembering this paragraph.
- **Built:** the traits, both event classes, the signed-event wrapper, the plausibility gate, and
  two providers — `LocalDeviceInput` (the default) and `AttestedEventInput`, a transport-agnostic
  host-side ingress for attested events.
- **Built (wire ingress).** The seam previously had no way to be reached from a network: `ClientNet`
  had no attested variant and the server had no route, so a client emitting a correctly signed event
  was talking to an open socket that dropped every frame *silently*. That gap is closed:
  - `ClientNet::AttestedEvent { signed: Box<SignedAttestedEvent> }`, serde tag `attested_event`
    (`magnetite-sdk`, behind the `scaling` feature that pulls in `magnetite-seams`). `PubKey`/`Sig`
    are hex strings on the wire, matching their hand-written serializers.
  - `magnetite_runtime::attested::AttestedIngress` — the route. **Per connection**, so one peer's
    traffic cannot spend another's budget. Order is load-bearing: connection rate limit
    (`MAX_ATTESTED_FRAMES_PER_SEC = 60`, applied *before* verification so a flood cannot be turned
    into signature-verification CPU burn) → signature → `PlausibilityGate` → queue. Gate state still
    advances **only on acceptance**, so a rejected flood cannot evict an honest player's rate budget.
  - `ServerNet::AttestedAck { seq }` / `ServerNet::AttestedReject { seq, reason }` — the client is
    told, rather than left inferring a drop from silence. Deliberately *not* `ServerNet::Ack`/
    `Reject`: those carry the client-local `u32` input sequence and instruct the client to discard
    `PredictionBuffer` frames at or below it, and attested events number in an unrelated `u64` space.
    The counters are separate because the input classes are, so the response channel is too.
  - **Fail-closed.** Unsigned attested frames have no wire representation at all (`signed` is
    required, so wibbly's unsigned shape fails to deserialize); malformed, wrongly-signed,
    implausible and flooding frames are each refused explicitly. An attested frame has no reachable
    path to the deterministic `ConnectionManager` queue — pinned by tests over real sockets in
    `magnetite-runtime/tests/attested_wire.rs`, because an attested event admitted down the
    deterministic path would leave `verify_replay` still passing while no longer proving anything.
  - Both ends are pinned to a **shared golden vector**: the exact frame JSON in
    `magnetite-runtime/src/attested.rs` is the same fixture as in wibbly's
    `packages/wibbly-magnetite/test/wire.test.ts`, signed by `RawKeypairAuth::from_seed([7u8; 32])`.
- **This delivers attested input; it does not make it verifiable.** The route adds *delivery* and a
  sanity screen and nothing else. A cheater who never touched a camera still hand-writes numbers
  inside human bounds, signs them with their own genuine key, and passes every check — the route has
  its own test (`a_plausible_synthetic_event_passes_the_whole_route`) restating at the wire edge what
  `a_plausible_synthetic_event_is_indistinguishable_from_a_real_one` pins at the gate. **None of this
  is anti-cheat, verification, or security**, and no future version of that file can make it so.
- **Not built:** anything that *produces* a gesture event. `AttestedEventInput` contains no camera
  capture, no pose model, and no vendor code; magnetite has no such code anywhere. Wibbly is still
  not a dependency of magnetite. What changed is only that the socket wibbly plugs into is now
  actually reachable over the network — the producer remains entirely on the client side.
- **Not built:** any consumer. Accepted events sit in the per-connection `AttestedEventInput` queue;
  no shipped game drains it, because no game in this repo has a gesture input to consume. Draining
  it is a game's job, and `InputClass::is_replay_verifiable()` returns `false` for everything in it.

## 4. Generic capacity-elastic node (the "bring any server → scales to infinity" property)

Collapse `backend` + `magnetite-runtime` into one `magnetite` node binary.
- Node **measures its own hardware** (cores/RAM/bandwidth) → advertises `Capacity`.
- A **world = a set of shards** (spatial cell / room / instance). Players live in shards; crossing a
  boundary is a handoff. Node runs as many shards as its box holds → **player cap is emergent from
  hardware, never a config constant.** More cores → more shards.
- **Cluster** (operator brings many boxes) → shard mesh with cross-node handoff. **Past the cluster,
  other operators' nodes join the mesh** (federated compute, paid via §3.6). This is real "Bucket D".
  - **Built:** `magnetite_runtime::fleet` — an Ed25519 mutually-authenticated TCP channel keyed on the
    node keypair (peer key is *pinned*, so the right address is not proof of the right node), carrying a
    **two-phase, epoch-fenced shard migration**: offer state → target validates/stages + acks → commit →
    commit-ack → *only then* does the source release authority. Every partial failure (ack timeout,
    rejection, dropped connection, target crash) resolves to **source retains authority** with state
    intact; a monotonic per-shard epoch fences duplicates, replays, and stale owners. Determinism is
    asserted across the migration boundary. `SpreadScheduler` places shards on ≥2 real nodes by capacity.
    Deliberately depends on **no** external protocol and **no** libp2p — cross-node handoff is core
    game functionality and must not rest on an optional dependency.
  - **Built:** `magnetite_runtime::cluster` — the fleet now **configures itself** and **players follow
    their shard**.
    - *Discovery-driven routing, membership-gated.* `RouteDirectory` derives `PeerRoute`s from the
      signed `SessionAd`s already in the phonebook, with the pinned key taken from the **signed ad**
      (never from the address). Because discovery is an *open phonebook*, a derived route is not
      permission to receive a shard: `ClusterMembership` is the operator-authorized set of node
      **public keys**, deny-by-default (empty ⇒ nobody), and it is checked when an ad is observed,
      again at migration time (so even a hand-registered route to a non-member is refused), and on the
      inbound `FleetNode` allowlist. Announcing that you host a game never makes you eligible to
      receive shards of a world you were not admitted to; expired/lapsed leases are not routed to;
      revocation takes effect on the next lookup.
    - *Session follow.* On a **committed** migration the source signs a `SignedRedirect` per affected
      player carrying the target's `{addr, pubkey}`, shard, new epoch, expiry, and a single-use
      `FollowToken`. The client verifies the redirect against the node key it already authenticated
      (a forged redirect is inert), reconnects, and **pins** the target key. The target admits only a
      member-issued, correctly-signed, unexpired, unredeemed token bound to that exact player, shard,
      target node, and the epoch it actually owns. Redirect, not proxy — the source does not stay in
      the path. Nothing is minted on a failed or rolled-back migration.
  - **Not proven:** tested over real sockets in-process and on a LAN only. **No NAT traversal, no relay,
    no WAN validation** — nodes must be directly reachable. Internet-scale fleets are not demonstrated.
- The game declares only *how to partition state into shards* (`trait Shardable`). A pluggable
  `ShardScheduler` places shards onto whatever capacity exists. Generic by construction.

## 5. Program backlog (waves). Task IDs are stable; agents claim by ID.

Legend: **[O]** = Opus-class agent, **[S]** = Sonnet-class agent. One writer per file set per wave.

### Wave 0 — Foundation (serialized before backend waves)
- **F1 [O]** Create `magnetite-seams` crate: all traits in §3 + default impls (`RawKeypairAuth`,
  `HashNaming`, `Local/HttpBlobStore`, `TrackerDiscovery`+`LanDiscovery`, `BuiltinProvider` shim,
  `MockPaymentRail`). Must compile + unit tests for each default. No provider-specific deps.

### Wave 1 — Backend tracks (parallel, DISJOINT file sets)
- **P1 [O]** PAYMENTS: rip fiat/custody (§2), wire `PaymentRail` into marketplace purchase (atomic
  split + signed-receipt entitlements), hosting-fee channel stub, subscriptions→operator-pay or removed.
  Owns: `backend/src/services/{payment,wallet,payout,wise}.rs`, `api/{wallet,webhooks,subscriptions,developer,marketplace}.rs`, economy migrations, `.env*`.
- **C1 [O]** COMMS: `CommsProvider` adapter module + `Matrix`/`Jitsi`/`LiveKit`/`Owncast` providers +
  node-as-IdP token minting; demote old chat/voice/streaming to `BuiltinProvider`. Owns:
  `backend/src/services/{communities,streaming,presence}.rs`, `ws/{comms,voice}.rs`, `api/{communities,channels,messages,streaming}.rs`, new `backend/src/comms/`.
- **N1 [O]** NODE+DISCOVERY: content-address games (serve wasm by hash via `BlobStore`),
  self-advertise via `Discovery`, server-browser API, collapse runtime/provisioning. Owns:
  `backend/src/services/{provisioning,distribution,games}.rs`, `api/{provisioning,distribution,games}.rs`, `magnetite-runtime/`, `magnetite-cli/`.
- **G1 [O]** SCALING: `Shardable` trait + `ShardScheduler` + capacity measurement + multi-node handoff
  scaffold (real Bucket D, scaffold w/ clear TODOs + tests for single-box multi-shard). Owns:
  `magnetite-sdk/` topology, `magnetite-runtime/` shard host. (Coordinate with N1 on runtime files — N1 owns bins, G1 owns shard/topology modules.)

### Wave 2 — Presentation track (parallel with backend; needs style-study output)
- **L1 [S]** LANDING page in Vulos house style (match ofisi/wede), own accent, hero + sections,
  screenshots embedded. **D1 [S]** DOCS site (same generator as ofisi/wede), chapters covering §1–§6
  architecture, screenshots. **R1 [S]** README rewrite (decentralized-games pitch, screenshots, badges).
  **SC1 [S]** Screenshotter: `npm run screenshotter` mirroring ofisi/wede (Playwright), captures
  landing + docs + app routes → images referenced by landing/docs/README.

- **IN1 [O]** INPUT: `InputProvider` seam (§3.7) — `InputClass` boundary, `PlausibilityGate`,
  `LocalDeviceInput` default + `AttestedEventInput` ingress. **Done**, now including the **wire
  ingress**: `ClientNet::AttestedEvent` + `magnetite_runtime::attested::AttestedIngress` +
  `ServerNet::AttestedAck`/`AttestedReject`, so a browser client's signed event is actually ingested
  rather than silently dropped. Still no gesture *producer* and no consumer in this repo — see §3.7
  "Not built". Owns: `magnetite-seams/src/input.rs`, `magnetite-runtime/src/attested.rs`.

### Wave 3 — Integration & optional providers
- **I1 [O]** ~~Wire DMTAP optional providers~~ — **DROPPED 2026-07-19 (founder call).** DMTAP was
  optional by design (every seam has a working default), so integrating it bought nothing and would
  have added a dependency on a private sibling repo. Nothing in Magnetite depends on DMTAP.
  Superseded by `KeyNameNaming` (`--features keyname`), which proves the `Naming` seam is swappable
  with zero dependencies.
- **I2 [O]** End-to-end: `magnetite dev` + tracker + content-addressed game + mock crypto purchase +
  Matrix/Jitsi room, all offline-runnable. Integration test.
- **I3 [S]** Generate real screenshots, embed everywhere, final README/docs/landing polish.
- **QA [O]** Build + clippy + tests green across workspace; remove dead fiat code; update `.env.example`,
  docker-compose, k8s/nomad, self-hosting docs to reflect no-cloud model.

### Loop / Definition of done
Waves repeat until: workspace builds + clippy clean + tests green; fiat fully removed; seams + defaults
in place with every seam defaulting to a working offline provider; landing + docs + README shipped with screenshots via
`npm run screenshotter`; GH description/topics set (done); `magnetite dev` runs the full offline demo.

## 6. Guardrails for all agents
- One writer per file set per wave. Do not touch files outside your claimed set.
- Program against §3 traits only; never leak a provider type into runtime/scheduler/payment code.
- Every seam keeps a working non-chain default; CI must pass with zero external services.
- Keep the game core (authority/sandbox/replay) intact — that's the moat.
- Record progress in `DECENTRALIZATION_PROGRESS.md` (append-only log: task id, files touched, status).
