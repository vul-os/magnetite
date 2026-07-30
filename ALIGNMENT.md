# Alignment — kotva substrate, patala rails, ephor optional

**Status: plan, not state.** This document describes a direction agreed in
design and the work it implies. Nothing here is claimed as built. The audited
state of the tree is [`site/docs/status.md`](site/docs/status.md), and it
governs: where this document and that one disagree, that one is true.

---

## 1. The rule this whole document obeys

kotva's [coordinator contract](https://github.com/vul-os/kotva/blob/main/coordinator/CONTRACT.md)
states it in one line:

> Every unavoidable coordinator is accountable, swappable, and self-hostable,
> and **never load-bearing**. Coordinators add reach; they never gate function.

Applied to magnetite, that forbids a design in which magnetite needs
[ephor](https://github.com/vul-os/ephor). If a magnetite node could not host a
game without a broker, the broker would be load-bearing and the contract would
be violated. So:

**Magnetite MUST function completely with zero coordinators.** A developer with
a laptop, or an operator with a $5 VPS, runs the whole thing alone. Ephor is
something you *hire for reach* — never something you wait for.

This also splits magnetite cleanly in two, and the split matters:

| | What it is | Coordinator? |
|---|---|---|
| **The authoritative node** | Runs the match. Load-bearing for that match by definition; not swappable mid-match. | **No** — it is a *peer the players chose*, like a TRACT seller's own node. |
| Relay, indexer, matcher, labeler, arbiter, oracle, payment gateway | Reach, search, moderation, adjudication | **Yes** — ephor's job, all optional |

Calling the game server a coordinator would claim a swappability it does not
have. It is a peer. Self-hosting is not the fallback here; it is the default.

## 2. Reachability rungs — ephor is needed for none of the first two

The current tree is LAN-only, and `status.md` is right about that. But the gap
is **not primarily a NAT problem** — it is an unvalidated-deployment problem.
Splitting it into rungs shows how little stands between here and a public
server.

| Rung | Who can host | Needs | State |
|---|---|---|---|
| **R0 — LAN** | anyone on the same network | `LanDiscovery` | **ships** |
| **R1 — public address** | cloud VPS, port-forward, static IP | `wss://` + WAN validation + a deploy recipe | **the gap, and it is small** |
| **R2 — host behind CGNAT** | home machines with no reachable port | a relay or tunnel | optional, later |

**Only the host needs to be reachable.** Players dial out, so no player ever
needs an open port, a relay or a public address regardless of their NAT. R2 is
therefore only about *hosts* on unreachable connections — a much smaller
population than "everyone behind NAT", and squarely optional for a catalogue
whose hosts are developers and paid operators.

This is settled prior art in the family, not a guess: FlowStock ships the same
shape and documents it in [`docs/SYNC.md`](https://github.com/vul-os/flowstock)
§Topologies — "Pair — two shops; one is reachable, the other dials it",
"Hub and spoke — head office is reachable", and explicitly "only one needs to
be reachable."

R1 needs no coordinator at all. `TrackerDiscovery` already ships, and a tracker
is an ordinary signed-ad HTTP endpoint — **you can run your own**, which means
zero third parties in the whole path. The honest blockers for R1 are:

1. **`wss://` / TLS on the node — and this is where magnetite differs from its
   siblings.** The node's socket is plaintext today;
   `magnetite-runtime/src/follow.rs:45` says "run behind TLS" and assumes a
   reverse proxy.

   FlowStock's shipped answer for node↔node is *don't build TLS into the
   binary*: run with `host: 0.0.0.0`, accept both `http://` and `https://` peer
   URLs, and delegate encryption to a trusted network — a LAN, a VPN/overlay
   (Tailscale, WireGuard, Netbird), or an HTTPS tunnel (`docs/SYNC.md`
   §Network). Adopt that for magnetite node↔node traffic; it is less code and
   it is proven.

   **It is not sufficient for magnetite, because magnetite has browsers.** A
   page served over `https://` cannot open a `ws://` socket — mixed content is
   blocked, with no operator-side workaround. Every three.js / Godot / wibbly
   client hits this immediately. FlowStock never faces it because its peers are
   its own binaries. So for browser↔node, real TLS is **mandatory rather than a
   trusted-network concern**, and the options narrow to: built-in rustls + ACME
   (best for the one-binary-anyone-can-host story), an operator's own reverse
   proxy, or a tunnel. This is also the one place ephor's
   `reachability-adapter` is closer to load-bearing for magnetite than it is
   for FlowStock — worth stating plainly rather than inheriting the sibling's
   posture unexamined.
2. **WAN validation.** Shard migration, cluster membership, session follow and
   the attested input wire are tested in-process and over LAN only. They need
   an actual two-host, cross-internet run before any claim changes.
3. **A deploy recipe** for the *node*. `fly.toml` and `Dockerfile.fly` target
   the legacy central backend (port 8080, `/health`, `migrate.sh`), not the
   node.

R2 is where a relay becomes necessary, and then ephor's `relay` kind (built,
libp2p Circuit Relay v2 + DCUtR, with a real two-peer loopback proof) is *one*
option alongside any other Circuit Relay v2 node or your own. Pure reach,
strictly optional.

## 3. kotva enters as linked crates, not as a service

This is the part that makes "don't wait for ephor" concrete. kotva ships four
real crates, and consumers pin a tag rather than tracking HEAD:

```toml
kotva-core = { git = "https://github.com/vul-os/kotva", tag = "core-v0.2.0" }
```

`kotva-core` v0.2.0 is a library. Linking it starts no service, opens no
socket, and requires no operator. What magnetite would use from it:

| kotva-core module | What magnetite does today by hand |
|---|---|
| `identity` — `IdentityKey` (Ed25519), `Identity`, `DeviceCert`, `RecoveryPolicy` | `RawKeypairAuth`, raw Ed25519 challenge/response |
| `id` — content addressing, `[0x1e] ‖ BLAKE3-256(bytes)` | game id = BLAKE3 of `(wasm + manifest)` — same hash, no agility prefix |
| `keyname` — zero-authority 8-word name from `BLAKE3(pubkey)`, checksummed | `HashNaming` — pubkey / short-hash addresses |
| `pubobj` / `pubsub` — signed public objects | `SessionAd`: signed, TTL-capped, fanned out |
| `cbor` — canonical integer-keyed deterministic CBOR | serde JSON |
| `capability` — delegated `CapabilityToken` (UCAN v1.0 profile) | operator allowlist of node pubkeys |

The identity curve already matches (Ed25519), and the content-address hash
already matches (BLAKE3-256). This is a low-friction binding, not a rewrite.

### Measured, not assumed — findings from the binding spike

A spike bound `identity`, `id` and `keyname` behind the existing seams and
measured the claims above. **Content addressing is exactly as clean as this
document claimed**: `ContentId::of(b).digest() == Hash::of(b).0` byte for byte,
conversion is a prefix byte with no rehash, and kotva's own `ContentId::verify`
accepts an id built from a magnetite `Hash`. That is asserted by test, not
argued.

Four things the spike found that this document did not anticipate:

1. **A git dependency is not feature-gateable.** With the feature *off*, nothing
   extra compiles — but `cargo --offline` still fails to *resolve*, because
   cargo reads the manifest before it considers features. "Nothing extra is
   compiled" is the wrong success criterion; "a fresh clone builds and tests
   with no network" is the right one. This is a milder recurrence of the patala
   lesson recorded at `magnetite-seams/Cargo.toml:17-32`, and the fix is the
   established house pattern: kotva providers live in their own
   `magnetite-kotva` crate, exactly as `magnetite-solana-rail` does, leaving
   `magnetite-seams` with zero git dependencies.

   **Done and verified.** `magnetite-seams`'s `Cargo.toml`, `Cargo.lock` and
   integration tests are byte-for-byte pristine, and the crate now builds and
   tests under a `CARGO_HOME` containing *no git directory at all* — so there is
   no kotva checkout to fall back on — hitting exactly the original 73 + 4 test
   baseline. Pre-extraction, that same command failed to resolve. Note also that
   adding the crate to `ci/rust-crates.json` was mandatory, not optional:
   `scripts/ci-crate-coverage.sh` fails CI on any crate present on disk but
   absent from the matrix. **Correction: that "15/15" was true inside the spike's
   worktree only.** `main` lists **14** crates — neither `magnetite-kotva` nor
   `magnetite-web-host` is merged yet, so both the crate count and the web host's
   entitlement gate exist only in unintegrated worktrees. Worth knowing before
   adding any future crate, and a reminder that worktree-local gate results are not
   repo facts.
2. **`Identity::verify` is an associated function, not a method** —
   `Token::is_valid_at` (`magnetite-seams/src/identity.rs:208`) hard-codes
   `<RawKeypairAuth as Identity>::verify`. So *any* provider whose signature
   bytes differ from raw Ed25519 silently breaks token verification. kotva only
   signs via `sign_domain(domain, msg)`, so the spike had to use an empty domain
   to stay byte-compatible. That is a workaround; the seam needs `verify` to be
   a method. **Real seam defect, tracked here, not fixed in the spike.**
3. **No seam expresses key lifecycle — the largest gap.** `DeviceCert`,
   `RecoveryPolicy`, `KeyRotation` and `MoveRecord` are the substantive half of
   `kotva_core::identity` and have nowhere to land. Magnetite's `Identity` is "a
   keypair that signs", with no notion of a key being superseded, so **a peer
   that rotates its kotva key becomes a different identity to magnetite.** For a
   platform whose whole premise is "identity is a keypair you hold", and against
   a substrate whose premise is "the key is the anchor and names are swappable
   pointers to it", that is a real hole and not a cosmetic one. It needs a seam
   change before the binding can be called done.
4. **`keyname` is not yet a stable identifier at this pin.** At `core-v0.2.0`
   the preimage is `BLAKE3(pubkey)`; kotva's `main` has since changed it to
   `BLAKE3(0x01 ‖ 0x1e ‖ pubkey)` for version binding. Same key, different name.
   So keynames from this tag must not be surfaced as long-lived user-visible
   identifiers, and tests must assert properties (determinism, checksum
   fail-closed, key binding) rather than golden vectors.

Also worth knowing: kotva-core is a heavy leaf for two seams — it pulls `hpke`,
`x-wing`, `ml-dsa`, `chacha20poly1305`, `ciborium`, `hkdf`, `sha2` and
`unicode-normalization`, none of which `identity`/`id`/`keyname` need, with no
finer-grained feature to request. Cold build, same machine and the flags CI
uses: `magnetite-seams --all-features` 1m20s, `magnetite-kotva` 2m00s — and
because the split gives kotva its own matrix leg, that cost now runs in parallel
rather than being added to the seams leg. Still worth raising upstream as a
feature-granularity request rather than absorbing silently.

One claim the spike put in doubt and then vindicated: `magnetite-seams/src/lib.rs`
states that a bare `cargo build`/`cargo test` "never touches anything outside
crates.io". Adding the git dep falsified it; the extraction makes it true again,
and its pre-change accuracy was checked rather than assumed (every dependency was
already a crates.io one). The wording therefore stands as originally written —
now measured rather than asserted.

### One wire-format boundary to get right

kotva's signing/content-address codec is **canonical integer-keyed
deterministic CBOR** — explicitly *not* serde/`ciborium` text-keyed encodings.
Magnetite is JSON throughout. The rule:

- **Signed and content-addressed objects move to canonical CBOR** — manifests,
  `SessionAd`s, receipts, capability tokens. Determinism of the bytes is the
  whole point of a signature.
- **The `mag_*` game ABI stays JSON.** It is neither signed nor on the wire —
  it is a host↔guest call boundary with a length-prefixed payload. Do not
  rewrite it. See §5.

Confusing these two would cause a large pointless refactor of the sandbox ABI.

## 4. patala is the payment binding

Replace the `PaymentRail` seam's implementations with a patala binding rather
than growing a third rail inside magnetite. Fiat and crypto arrive together,
and patala already types the distinction magnetite's seam cannot express
(`patala-core/src/capabilities.rs`):

> **Do not design this from scratch — BeepBite already ships it.**
> `backend/internal/payments/` in [beepbite](https://github.com/vul-os/beepbite)
> is a working patala binding with the exact posture magnetite wants, and its
> shape should be copied rather than reinvented:
>
> - A provider-agnostic `PaymentProvider` interface (`gateway.go`).
> - **Build-tag gated** (`gateway_patala.go` under `//go:build patala`,
>   `gateway_default.go` under `//go:build !patala`) so the default build links
>   *no gateway code at all* — behaviour is byte-for-byte identical and no
>   dependency is pulled. This is a cleaner execution of the same intent as
>   magnetite's `--features solana`.
> - One deployment-wide env var selects the processor
>   (`BEEPBITE_ONLINE_PAYMENT_PROVIDER=stripe|paystack|yoco|payfast`), with
>   per-processor credentials.
> - **Loud on misconfiguration, and better than magnetite's version of the same
>   rule:** it distinguishes "nothing configured" (fine — stay offline-only)
>   from "configured, but this build cannot honour it" (an error at startup), so
>   a typo'd provider name is never silently ignored. Magnetite's blanket
>   panic-at-startup does not currently draw that distinction and should.
> - patala-fiat is consumed as a cdylib with per-processor `fiat-*` features.

```rust
pub enum RailClass { CustodialReversible, NonCustodialFinal }
pub enum Settlement { Instant, Seconds(u32), Days(u8) }
```

### Is magnetite Solana-dependent? Structurally no; in four types yes

**Structurally it is not.** `PaymentRail` is a trait, the default is
`MockPaymentRail` (offline, deterministic), `SolanaPaymentRail` lives in its own
crate compiled only under `--features solana` and selected only by
`PAYMENT_RAIL=solana`, and an unknown or uncompiled value is fatal at startup
(`backend/src/services/payment.rs:47-74`). The default build pulls no chain
dependency, opens no socket, and the whole suite runs offline. You can develop,
test and ship without any chain code.

**But four concrete types leak chain assumptions**, and they are what would make a
second rail painful:

1. **`Leg { wallet: PubKey, amount: u64, role }` has no currency field.** The unit
   is implicit — and *that is exactly the cents-vs-micro-USDC bug above.* It is not
   a coincidence: an amount without a declared unit is a bug waiting for a second
   rail, and magnetite already has two things disagreeing about it.
2. **`PubKey` is `[u8; 32]`** (`magnetite-seams/src/identity.rs:21`) — an Ed25519
   key. That fits Solana and Stellar. An EVM address is 20 bytes of secp256k1,
   Bitcoin and Lightning are secp256k1, and a fiat destination is not a key at all
   but an opaque processor-side token. `wallet: PubKey` structurally excludes all
   of them.
3. **"Identity key doubles as wallet key, so the seam needs no key-mapping
   table"** — stated in `site/docs/payments.md` as an architectural property. It is
   a *Solana and Stellar* property (both Ed25519-native), and false for every
   secp256k1 chain and for all fiat. Agnosticism means accepting a per-rail address
   binding signed by the identity key. Not hard, but the current claim is
   one-chain-family-only and reads as general.
4. **Verification by chain readback.** `verify_receipt_for_item` re-reads the chain
   and fails closed. Right for Solana and EVM; wrong for fiat (ask a processor, and
   it is reversible) and wrong for Lightning (no global queryable ledger — the proof
   is a preimage). Verification shape has to vary by rail class.

Two further Solana specifics correctly live *inside* the rail crate but constrain
the seam and must not be assumed away: the dust floor is justified by Solana
economics (5000-lamport base fee, ~0.00204 SOL ATA rent per recipient), and
`MAX_TX_WIRE_BYTES = 1232` means **leg count has a hard cap**. A seam that assumes
unlimited legs or a universal dust constant is wrong.

### The fix is patala's vocabulary — and it is the same fix as the unit bug

`patala-core/src/rail.rs:19` is already the agnostic shape:

```rust
pub struct PayRequest {
    pub amount_minor: u64,   // smallest unit
    pub currency: String,    // ISO-4217 for fiat, asset ticker for crypto — UNIT IS EXPLICIT
    pub destination: String, // wallet address for crypto, processor token for fiat
    pub reference: String,   // idempotency / correlation key
}
```

Every leak above closes by adopting it. `currency` alongside `amount_minor` makes
the cents-vs-micro-USDC class of bug *structurally impossible*, which is stronger
than fixing the one instance — and patala-fiat's ISO-4217 table (147 currencies,
checksum-pinned, explicit exponents, and it refuses unknown codes rather than
defaulting to 2) is the authority for the conversion. `destination: String` covers
Ed25519 wallets, secp256k1 addresses and processor tokens alike.

**This changes the phase order.** The patala binding was Phase 3 item 10, after the
devnet round-trip. It should come *before* it: the round-trip is blocked on the
unit fix, and the right way to fix the unit is to adopt a money type that carries
its unit rather than to patch a constant and leave the next rail to rediscover the
problem.

### The part that genuinely cannot be made agnostic — and it is the product

Magnetite is **already** bound to patala: `magnetite-solana-rail` describes itself
as thin glue over `patala-core` / `patala-solana`, pinned to a git rev, and uses
`patala_core::PayRequest` and `patala_core::PaymentRail` directly. So most of the
"patala binding" work is done for this rail. But its own module docs name the
limit:

> `patala_core`'s seam has no multi-party split concept (`PATALA.md` §3): one
> `PayRequest` is one recipient.

**Atomic N-way split is magnetite's own layer, built past patala's seam, and it is
chain-shaped.** Solana lands every instruction in a transaction or none, so a
developer + operator + stewards split is atomic *for free, with no custom
on-chain program*. That is a real, unusual property and it is the economic model's
foundation — the whole "voluntary legs, no custody, no platform holding funds"
design rests on it.

It does not port:

| Rail | N-way atomic split |
|---|---|
| Solana | free — multiple instructions, one transaction |
| EVM | needs a custom splitter/multicall **contract**, which magnetite deliberately has none of |
| Lightning | no — N payments, independently failable |
| Fiat processor | no — N transfers, partial failure real, plus chargebacks per leg |

So rail-agnosticism is available for *paying*, and **not** available for *splitting
atomically*. Making the split agnostic means downgrading it to best-effort with
reconciliation and a partial-failure story — which weakens the guarantee that
makes the model work. That is the actual trade, and it should be made
deliberately rather than discovered.

> ## DECISION: Sui, not Solana
>
> **The chain is Sui. A `patala-sui` rail is to be written, and the Solana rail
> retired.** Decided deliberately, and the timing is the strongest part of the
> argument: **no payment has ever settled through any rail**, so the migration
> cost is zero now and only rises once real receipts exist. The cheapest moment
> to change chains is before the first one settles.
>
> Why Sui, concretely — all four are magnetite-specific, not general chain
> advocacy:
>
> 1. **No associated-token-account equivalent.** Solana's ATA requirement is what
>    forced the entire dust-floor design (~0.00204 SOL one-off rent *per
>    recipient*, which can exceed a voluntary stewards leg on a small sale). On
>    Sui, coins are objects transferred straight to an address. That complexity is
>    **deleted, not managed**.
> 2. **Programmable Transaction Blocks give the same atomicity.** `SplitCoins` +
>    `TransferObjects` in one PTB, all-or-none, with **no custom Move package** —
>    preserving the "atomic by construction, no on-chain program" property that
>    §4 identifies as the economic model's foundation.
> 3. **Far larger transaction budget** than Solana's 1232 bytes, so the hard
>    `TooManyLegs` cap becomes much less binding.
> 4. **Walrus is on Sui** — one chain for blobs and money, if Walrus is adopted
>    for the `BlobStore` seam. **This argument is weaker than an earlier revision
>    of this document claimed, and that claim is retracted: see §4a.** The
>    decision stands on reasons 1–3, which are the load-bearing ones.
>
> Ed25519 is supported on Sui, so the "identity key doubles as wallet key, no
> key-mapping table" property survives.
>
> ### §4a — Walrus: retraction and honest assessment
>
> An earlier revision of this document said the Walrus alignment "carries the most
> weight" in the Sui decision. **That was an overstatement and is withdrawn.**
> Three problems, one of which conflicts directly with code that has just been
> written:
>
> 1. **Paid bundles cannot live on public Walrus.** Walrus blobs are publicly
>    readable by blob id. `magnetite-web-host` gates paid content behind a verified
>    receipt — but if the same bytes sit on a public storage network, anyone fetches
>    them directly and **the entitlement gate is decorative**. Fixing it requires an
>    encryption layer in which the entitlement releases a key, which means either a
>    party holding keys (a coordinator, in a design built to avoid them) or
>    threshold encryption. Neither is in the current design.
>
>    Note this question is not Walrus-specific: it applies to *any* public blob
>    backend, so it is worth answering on its own merits regardless of this
>    decision.
> 2. **Storage is leased, not permanent.** Walrus blobs are paid for in epochs and
>    must be renewed. A game catalogue needs the opposite: itch.io games from 2013
>    still load. **A content-addressed package whose bytes have expired is worse
>    than a 404** — it is a dead link with a verifiable hash, which looks
>    recoverable and is not. Permanence is what kotva's bindings index lists
>    Arweave for, separately and for exactly this reason.
> 3. **Walrus has its own token (WAL).** Paying a third party in their token is not
>    minting one, so this is not a violation of the family's no-token stance — but
>    storage cost becomes denominated in a volatile asset that every operator must
>    acquire and hold, separate from the USDC everything else uses. kotva's bindings
>    index notes only "newer; behaves like a CDN, not an archive" and flags neither
>    the token nor the lease model. That is a gap in our own binding index.
>
> **And it is not needed yet.** `BlobStore` already ships **`FsBlobStore`** +
> `HttpBlobStore`. At R1 — one operator, own VPS, own disk — that is sufficient and
> free; the host *has* the bytes. (An earlier revision of this section, and
> `site/docs/seams.md` row 3, both named `LocalBlobStore` — the **in-memory** one,
> whose own comment says it is not a durability target. `FsBlobStore` is what
> actually backs R1. Both need correcting.)
>
> **Superseded by §4b below — the verified answer is "remove Walrus", not "defer
> it". See `docs/walrus-assessment.md` for the full evidence.**
>
> ### §4b — Walrus verified: remove it from the plan, do not defer it

§4a's claims were checked against primary sources. Two survived, one was **half
wrong in the half that mattered**, and a decisive finding was missing. Full
evidence in `docs/walrus-assessment.md`.

- **Public readability: confirmed, stronger than written.** Documented in `Danger`
  blocks in four places; blob ids, attributes *and* the on-chain registration are
  enumerable. Walrus Sites documents no mechanism for restricting reads.
- **Leasing: confirmed, plus corrections.** Hard ceiling of **53 epochs ≈ 2 years**
  per purchase. Expiry is *worse* than "a dead link with a verifiable hash": reads
  404 while **cached and CDN copies survive**, so the catalogue half-works, which
  is harder to diagnose than clean failure. Burning the Sui object to reclaim the
  rebate **forfeits renewal** — the cheap action and the durable action conflict.
- **The WAL claim was half wrong.** Storage is **priced in USD** at $0.023/GB/month
  and paid in WAL, with the WAL amount floating to hold the peg — volatility is
  absorbed by the protocol's oracle, **not** by the operator. §4a was wrong on the
  point that would have driven a decision. The real burden is **two balances** to
  fund and monitor (WAL for storage, SUI for gas) with **no USDC path**.

**The decisive finding: Walrus's shape does not fit a game bundle, in any
configuration.** It bills a fixed **64 MiB of per-blob metadata** on top of 4.5×
erasure expansion. A 400-file Godot bundle as 400 blobs pays for ~25 GB of metadata
(~$7/yr) to hold 8¢ of content. As one archive blob it loses per-file range serving
and caching, which §5 requires. Quilt — the intended fix for many small files —
**destroys content addressing**: a `QuiltPatchId` is determined by the composition
of the whole quilt rather than the item's own bytes, and `blobstore.rs:3` opens
"The hash IS the id." Walrus's own "avoid Quilt when…" guidance excludes magnetite
twice: when items need content-derived ids, and when every item is a hot
independently-cached object.

**Shape mismatch, not timing.** That is why this is a removal, not a deferral.

**kotva's "~1/5 cloud cost" is backwards.** Effective rate is $0.023 × 4.5 =
**$0.1035/GB-of-your-data/month**. Against July 2026 primaries: S3 Standard $0.023
(Walrus **4.5× more**), R2 $0.015 (6.9×), B2 $0.00695 (**14.9×**), Hetzner Storage
Box ~€0.0020 (~51×). Egress does not rescue it — R2 free on all classes, B2 free to
3× stored, Storage Box unlimited — so egress differentiates Walrus from AWS
specifically and nothing else. **That row in kotva's bindings index needs fixing.**

Real cost by Walrus's own formula, one blob, one year: 50 MB → $0.08, 500 MB →
$0.64, 2 GB → $2.50. SUI gas excluded because Walrus publishes no figure, and
Quilt's cited 238× gas saving implies un-batched gas is not negligible.

### §4c — Seal, and paid content on any public store

Seal reached mainnet September 2025 — **more mature than §4a implied** — and is
still wrong here, for reasons in Seal's own security docs:

1. **A fetched key cannot be revoked.** Revoking on-chain stops future requests but
   cannot retract keys already fetched. With immutable public ciphertext, **one
   buyer leaking a key once makes the game free forever**, no rotation possible.
   That is a **downgrade** from today's gate, which can stop serving tomorrow.
2. **No audit trail** — no on-chain delivery logs, so a leak cannot be attributed.
3. **Liveness is a commercial relationship**, and the server set is frozen at
   encryption time.

Against §1's coordinator test: accountable yes, self-hostable yes, swappable **no**,
never-load-bearing **no** — below threshold a paid bundle is *dead*, not degraded.
Worse than the ephor dependencies §2 is careful about.

**The partial rescue:** if the key vendor is *the developer themselves*, that is the
same shape §1 already accepts for the authoritative node — a peer the players chose.
Argues for self-hosted per-developer key vendors, against any Mysten-gated
committee. Seal's docs currently self-contradict on whether committee mode is live
on mainnet, and the mainnet aggregator requires an Enoki API key.

### §4d — evermesh: take the formats, not the backend

**§4a's hypothesis that evermesh's gateway solves paid content was wrong.** Its
"policy engine" is a **moderation denylist** — `checkBlobHash(blobId) → {allowed}`,
no viewer parameter, no session, no receipt — and its "key custody" is custody of
*identity signing keys*, unrelated to content. It serves blobs after a denylist and
CSAM check with **no per-viewer check at all**. `magnetite-web-host` already does
strictly more.

The right design is specified there and unbuilt: spec 008 §4 "Gated access", a
gateway as key vendor issuing keygrants to payers. But the kernel has **no AEAD
crate anywhere** and no `privacy` coverage group. The scheme is prose.

Worth taking, cheaply:

- **The 1 MiB Merkle chunk tree** — it makes range reads *verifiable*, which
  magnetite's per-file hashing does not.
- **The `EVMS` bundle container** — this already **is** §7 item 9b, specified and
  conformance-tested. Do not design a bundle container.
- **The `coverage.json` discipline.**

Its *manifest* is a poor fit — record kind 16 is hard-bound to one media work
(required `original`, RFC 6381 codec, duration, dimensions), with no `path → hash`
map and no way to express one. Its *blob layer* is a good fit: `hash_blob` is
byte-identical to `magnetite_seams::Hash::of`, both bare BLAKE3-256. **Neither
matches kotva's `0x1e`-prefixed `ContentId`** — so there are three hash conventions
in the family as well as three CBOR codecs. Same disease.

### §4e — the real storage work is not backend choice

`BlobStore::get` returns `Option<Vec<u8>>` (`blobstore.rs:69`) — the whole blob in
RAM. **A 2 GB Unity bundle is unservable and range serving is inexpressible, on
every backend equally.** Fixing that seam is the actual storage work. Backend
choice was never the bottleneck.

**Therefore: blob storage is `FsBlobStore` + `HttpBlobStore`. Walrus is removed
from the plan. Paid content on a public store remains unsolved, and is not blocked
on choosing a store.**

### The one genuine risk: Sui has no memo primitive
>
> **Resolve this before building the rail.** Solana's binding relies on the
> **SPL Memo** program — a canonical, pre-existing program, so using it does not
> violate "no custom program" — carrying
> `blake3("magnetite-pay-v1" ‖ buyer ‖ item)`. Two of the ten fail-closed checks
> depend on it directly: that the binding reference equals that hash, and that
> **the on-chain memo is exactly the derived binding**. That is what makes a
> receipt non-transferable between items.
>
> Sui has no native memo field, and a pure transfer PTB has nowhere obvious to
> hang arbitrary bytes — events can only be emitted from Move calls. So the
> candidate answers are:
>
> - a **small Move package** to carry the binding — which forfeits the
>   no-custom-program property that is half the reason atomic splits are cheap;
> - **deterministic transaction reconstruction**, making the digest itself the
>   binding — elegant, but fragile, since gas-coin selection, gas price and budget
>   all change the bytes;
> - an **off-chain binding** with weaker guarantees, which would be a real
>   downgrade from the current fail-closed posture and must not be adopted
>   silently.
>
> None is obviously right. This is the highest-risk unknown in the switch and the
> thing most likely to make it more expensive than it looks. Scope it first;
> everything else in the rail is structural work with a working template to copy.
>
> Solana is retired **after** the Sui rail lands, not before — the ten checks and
> the scripted-fake-RPC offline test pattern are the reference to port from.
> Leaving an unvalidated, unmaintained rail in-tree afterwards would read as a
> supported option, so it should be deleted rather than left feature-gated.

**On the general question — pure crypto on one rail, with the agnostic vocabulary
adopted anyway.** Those are two independent axes and conflating them is the error:

- **Vocabulary** (`amount_minor` + `currency`, `destination: String`) — take it
  now. It costs almost nothing, it fixes the live unit bug *structurally*, and it
  keeps a second rail from being a rewrite. Note the unit bug happened with **one
  rail**: an implicit unit is not simpler, it is under-specified.
- **Rail set** — ship exactly one (Solana/USDC). Do not build EVM, Stellar or fiat
  yet, and **do not claim agnosticism in the docs.** One rail, stated as one rail.

And the simplicity win that is actually available: **free games need no rail at
all.** A large share of an itch.io-shaped catalogue is free, so rung 0 plus free
packages is a usable platform with zero settled payments. The rail is not on the
critical path to launching.

When fiat eventually becomes necessary for reach — and for a mainstream games
audience it will — the honest options are a custodial party performing the split
off-chain, or fiat splits that are non-atomic and *say so*. Both are acceptable;
neither is free; both must be declared per rail rather than papered over.

### The one genuinely new piece of design work

Magnetite's entitlement model is "the receipt *is* the entitlement", verified
by re-reading the chain and failing closed. That is only sound because
`NonCustodialFinal` means final. A `CustodialReversible` rail has chargebacks:
a fiat receipt can be retracted *after* verifying, and there is no chain to
re-read — verification becomes "ask the gateway", which reintroduces both a
live dependency and a party who can lie.

Make it capability-driven rather than a special case:

| Rail capability | Entitlement policy |
|---|---|
| `NonCustodialFinal` + `Settlement::Instant` | granted on verify — today's behaviour |
| `CustodialReversible` + `Settlement::Days(_)` | developer's choice, declared in the manifest: hold until the settlement window closes, or grant immediately and accept revocation |

The rail declares its class; the manifest declares the policy; the runtime
still names no provider.

### Economics: voluntary legs, not a protocol fee

There is no enforceable mandatory fee in a system with no central server, so
the model is voluntary contribution with honest defaults — and it collapses to
one mechanism. `PaymentSplit`'s fixed `{developer, operator, protocol_fee_bps}`
shape becomes a list that must sum exactly to the total:

```rust
pub struct PaymentSplit { pub legs: Vec<Leg> }
pub struct Leg { pub wallet: PubKey, pub amount: u64, pub role: Role }
pub enum Role { Developer, Operator, Stewards, Other(String) }
```

**This is two layers, not one — a correction to an earlier version of this
section.** A signed manifest *cannot* carry absolute leg amounts, because for a
pay-what-you-want or tipped purchase the total is unknown until checkout. So:

| Layer | Expresses | When |
|---|---|---|
| **manifest** — `SplitLeg { wallet, share_bps, role }` | proportions, summing to exactly 10 000 | signed at publish time |
| **rail** — `Leg { wallet, amount, role }` above | absolute amounts, summing exactly to the total | resolved at checkout |

`SplitPlan::resolve(total)` is the single bridge, using largest-remainder
allocation so the parts sum exactly for any total — including 0, 1, and values
that do not divide evenly. There must be exactly one implementation of that
conversion.

Co-developers, publishers, asset-pack authors and charity splits all become the
same code path. Solana's single-transaction atomicity gives all-or-none for any
leg count, and fail-closed check #10 (no unaccounted party) already prevents
anyone slipping a leg in.

Three knobs, one owner each:

| Who | Sets | Where |
|---|---|---|
| Developer | price model (free / fixed / PWYW), own split, stewards bps | signed manifest |
| Operator | their own charge, or nothing | signed session ad |
| Player | optional tip on top | checkout |

**Two things are currently wired to the wrong party and must move:**

- `SOLANA_FEE_WALLET` → `SolanaConfig.fee_wallet` (`magnetite-solana-rail/src/lib.rs:154`)
  means the **node** picks where the "protocol fee" goes. An operator can point
  it at themselves and the receipt still verifies, because verification checks
  that claimed recipients match the chain — not that they are the right people.
  The stewards destination must come from the signed release, never from node
  env. A fork changing it is expected; a host silently redirecting it is not.
- `PROTOCOL_FEE_BPS` is also node env, which is neither the developer's choice
  nor the player's. Move the rate to whichever party's money it is.

### Both are now moved — and how the stewards address is anchored

`stewards::COMPILED_IN = option_env!("MAGNETITE_STEWARDS_WALLET")`. Because
`option_env!` resolves at compile time, the value lives in the binary's bytes,
which the release publishes under `SHA256SUMS` plus a provenance attestation that
`scripts/verify.sh` already fails closed on. Changing it therefore requires a
*different binary* whose digest will not match. **A fork changing it is expected
and visible; a host silently redirecting it is not possible.** Two enforcement
points: `plan()` refuses a `Stewards` leg naming any other wallet, and
verification denies a receipt *claiming* one to any other wallet.

It is `None` in this tree — no address is published — so a stewards leg is
**refused**, not redirected and not silently dropped.

Three correctness details the refactor surfaced, all worth keeping:

- **Check #10 must aggregate by wallet before comparing.** The chain reports one
  net token-balance delta per owner, so a co-developer who is also the operator
  appears once on chain and twice in the split. Comparing per-leg would reject an
  honest transaction.
- **There is a hard cap on leg count.** A Solana transaction is 1232 wire bytes,
  checked against the *actual serialized* bytes before submission, yielding a
  `TooManyLegs` error rather than silent truncation. Arbitrary-length splits are
  arbitrary only up to that limit.
- **Role tags travel inside the rail proof**, length-prefixed so
  `Other("a") + Other("bc")` cannot collide with `Other("ab") + Other("c")`, and
  covered by both the rail signature and the on-chain memo. The receipt seed now
  commits to every leg, amount, order and role instead of to a single fee bps.

### A pre-existing unit mismatch — and it blocks the devnet round-trip

`backend`'s `units_from_usd` yields **cents**; the Solana rail treats every amount
as **micro-USDC**. A $1.99 item reaches the rail as `199` micro-USDC rather than
`1_990_000` — a 10 000× error. No money has been mispriced because nothing has
ever settled, which is the only reason this is still a latent bug rather than an
incident.

It also constrained the dust-floor design in a way worth recording: a single
seam-wide floor of `1000` would mean 0.001 USDC on one rail and **$10** on the
other, silently skipping the *developer's* leg on any ordinary purchase — a
fail-open dressed up as a safety feature. So the dust mechanism lives in the seam
(`partition_at`) while the number lives with whoever knows the unit. The floor is
`1000` micro-USDC (0.001 USDC), chosen to beat the cost of moving it — a
5000-lamport base fee plus roughly 0.00204 SOL one-off ATA rent per recipient —
as a fixed integer rather than a live conversion, which would need an oracle and
floats. A dust-only split is refused, never settled for free.

**Fix the unit mismatch before the devnet round-trip, not after.**

Operator charging does **not** need the payment-channel program. Two forms work
on the existing `checkout_for_item` path: a revenue-share leg on sales made on
that server, or a one-time entry checkout per session. Channels become an
optimization for high-frequency metering, not a prerequisite.

## 5. Godot / JS / three.js — two unrelated hosting problems

Conflating these is what makes the work look large.

**Hosting a web build** = serve content-addressed files over HTTP and check an
entitlement receipt. No VM, no tick loop, no authority, no anti-cheat, no
determinism. `BlobStore` and `verify_receipt_for_item` already cover most of
it. This is the overwhelming majority of an itch.io-shaped catalogue, and it is
close to free.

**Hosting an authoritative match** = the node, wasmtime, ticks, shards,
migration.

### The capability ladder

| Rung | You ship | You get | State |
|---|---|---|---|
| **0** | any web bundle — three.js, Godot, Unity, Bevy-web | publish, entitlements, checkout, discovery | pieces exist, unwired |
| **1** | + a wasm authority stepped from your own render loop | determinism, replay, no server | **wibbly does this today** |
| **2** | + a hosted node | anti-cheat, interest-filtered snapshots, paid operators | LAN-validated |
| **3** | + a fleet | sharded / unbounded | unbuilt above LAN |

### The upsell only magnetite can offer — and it already works

`game-templates/authoritative/src/wasm_abi.rs` documents the sandbox contract:
eight exports (`mag_abi_version`, `mag_alloc`, `mag_free`, `mag_init`, `mag_step`,
`mag_snapshot`, `mag_restore`, `mag_view`), plain C ABI, every payload a
length-prefixed JSON blob at a returned pointer. **Nothing Rust-specific crosses that boundary.**
Any language targeting `wasm32-wasip1` can implement it — Zig, C, TinyGo,
AssemblyScript. The Rust SDK is a convenience, not a requirement.

**This is now demonstrated, not argued.** A conformance harness plus a
hand-written WAT module — no Rust, no C, no imports at all — passes 31 of 31
checks in the production sandbox (same engine config, WASI stub linker and
resource limiter as the real host, so a pass means "works in the sandbox", not
"works in a rig"). One second implementation is not a toolchain matrix, so
"we support Zig / TinyGo / AssemblyScript" is **not** claimed — but
language-agnosticism is no longer a claim about the ABI's shape, it is a
measured property.

### The harness immediately found five defects — three of them real bugs

Documenting a boundary with a test harness rather than prose found what months of
use had not:

1. **The tick number never crosses the ABI.** `mag_step` carries no tick;
   `GameExecutor::step(tick, …)` receives one and drops it. The reference guest's
   `static mut CURRENT_TICK` was neither snapshotted nor reset by `mag_restore`,
   so a restored instance resumed on the wrong tick. Replay-from-snapshot and
   shard migration both depend on this. The snapshot half is fixable guest-side;
   **putting the tick in `mag_step` is a genuine ABI change with a live
   cross-repo consumer (wibbly) and is held as a separate decision.**
2. **The reference guest discarded every input frame.** The host encodes
   `[{"player_id":…,"input":…}]`; the guest deserialised
   `Vec<(PlayerId, Input)>`, and serde will not build a tuple from a JSON object.
   It survived for months because the wasm/native parity test stepped both sides
   with **empty inputs** — so the test that existed to catch exactly this could
   not.
3. **`mag_restore` framing was not honoured**, and the failure was silent: the
   guest fed the whole `[u32 LE][JSON]` buffer to serde and
   `NativeExecutor::restore` swallowed the parse error as a no-op. A fail-open,
   in a repo whose stated rule is that no fail-open paths exist.
4. `abi.rs`'s module doc contradicted its own code about inbound pointer
   conventions — a comment, corrected in place.
5. **`WasmExecutor::classify_trap` can never classify anything.** It
   substring-matches `err.to_string()`, but `wasmtime::Error` is an
   `anyhow::Error` whose `Display` is the outer context; the trap kind is only
   reachable by downcasting to `wasmtime::Trap`. Fuel exhaustion, epoch timeout
   and memory-cap failures therefore all surface as a generic trap, so an
   operator cannot tell a runaway loop from an OOM.

Note the shape of #1 and #3 together: `snapshot_is_idempotent` passed while
`resumes_trajectory` failed. A single round-trip check would have reported
success. That is the argument for per-check reporting over one boolean.

### Two more that only an all-pass gate would surface

Requiring the harness to reach all-pass — rather than accepting "mostly passing"
— forced out two bugs nobody was looking for. Both were SDK-internal; neither
needed an ABI change.

6. **`on_join` was never called by any production path** — only by unit tests. So
   `ArenaShooter` never had a player, and *every* input returned `Unauthorized`.
   Fixing the input deserialisation alone would have changed nothing observable.
   First-sight join now happens inside `NativeExecutor::step` so native and
   sandbox cannot drift and `verify_replay` re-joins identically, and `on_join` is
   contractually idempotent — which is **load-bearing for wibbly**, whose
   `restorePlayers` seeding would otherwise duplicate players.
7. **RNG stream position was hidden state that no snapshot captured**, so a
   restored executor rewound its randomness. `StepCtx::rng` is now derived per
   tick from `(seed, tick)`. This **changes the `state_hash` values a match
   produces** — safe only because no persisted replay logs exist yet and
   re-simulation is self-consistent. It would not have been safe later.

Note the compounding: bug 2 (inputs discarded) was *masked* by bug 6 (no player
ever joined). Fixing either alone would have looked like it changed nothing. This
is the argument for an executable contract over a documented one.

### The doc audit — what was actually false

`verify_replay`'s "Running" status **survives**: it starts at tick 0 and never
restores, so the restore bugs did not touch it. It was, however, untested with
players. What was false:

- **Shard migration's "every partial failure resolves to the source keeping
  authority, state intact" was false twice** — a target that could not decode a
  snapshot kept its own state *while the handoff reported success*, and fidelity
  depended on bugs 3 and 7. Both halves corrected; the one production caller now
  abandons the handoff so the source keeps authority.
- **README's "identical `state_hash` on every tick" was true but vacuous** —
  measured with empty inputs. Now 19 tests with non-empty inputs.
- `DECISIONS.md` N3-2's "empty inputs baseline" *was the blind spot itself*;
  superseded with a new block rather than rewriting the log — the record is
  historical.
- Smaller ones: `GAPS.md` miscounted the WASI stub imports (10, not 9), and
  `lib.rs` described spawn positions as seed-derived arena corners when they are
  a circle by join index and seed-independent — wrong in both halves.

### Wibbly: unaffected, and it was coupled to the bug

Wibbly ships a **checked-in vendored `.wasm` with no recorded magnetite commit**,
which is its own provenance gap worth closing. It was unaffected by all three
guest defects — and one it had already worked around, documenting the
non-resetting tick counter and enforcing a fresh instance per match.

The important part: **wibbly was coupled to bug 2.** It sends inputs as
`[[1, {…}]]` — the tuple form the broken guest wanted. A naive "fix" to the object
form would have silently emptied its inputs. The guest struct now accepts both
forms, so a re-vendor works either way.

So the pitch to a Godot or three.js developer is not "rewrite in Rust". It is:
keep your renderer, write your *rules* as a small wasm module in whatever
language, and get authoritative multiplayer, deterministic replay and
anti-cheat that no other web-game host can offer. Making the ABI a documented
public contract is mostly writing, not engineering — and it is the highest-
leverage adoption item on this list.

### Web-bundle serving specifics that will bite

- **COOP/COEP is absent.** Godot 4 web export needs `SharedArrayBuffer`, which
  needs `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp`. Nothing in `nginx.conf`,
  `backend/src` or `frontend` sets either. This is the classic Godot-on-itch.io
  failure, and COEP then breaks cross-origin assets — bundles must be
  self-contained or same-origin. Test against a real Godot export early.
- **A bundle is many files, not one blob.** Content-addressing needs a manifest
  of `path → hash` with the root hash taken over that sorted list — not one
  hash of a tarball — or per-file caching and HTTP range serving are lost.
  Annoying to retrofit.
- **Compression.** Godot and Unity ship `.wasm.br` / `.pck.gz`; without correct
  `Content-Encoding` they fail silently or download uncompressed.
- **Label the determinism boundary.** A Godot or three.js game is not
  deterministic and cannot be replay-verified. Say so in the manifest and in
  the UI. The precedent already exists and is enforced in code:
  `InputClass::Attested` for camera gestures, explicitly never
  replay-verifiable. Rung 0 must not inherit rung 2's claims.

### Findings from building it — three that were not anticipated

Rung-0 serving is implemented in its own `magnetite-web-host` crate, whose only
magnetite edge is the seam traits: not in `backend/` (being deleted, needs
Postgres — a file server that requires a database *is* a coordinator, violating
§1), not in `magnetite-runtime` (rung 2 may depend on rung 0, never the
reverse — rung 0 must not inherit a wasm VM's build and attack surface), and not
in `magnetite-seams` (whose own rule is that it never hard-depends on an HTTP
client; a server is worse).

1. **`Cache-Control: public` on a paid bundle is a security hole.** A CDN or
   corporate proxy caches one entitled fetch and serves it to everyone behind
   it — a shared cache never re-runs the entitlement gate. Paid bundles must be
   `private` with `Vary: Cookie, …`; only free bundles may be `public`.
2. **A custom header cannot gate a paid bundle.** The browser fetches `.wasm` /
   `.pck` itself and will not attach an `X-Magnetite-Receipt` header, so a
   header-only scheme yields a loading document and a 402 on every asset. A
   path-scoped cookie is required. This is the kind of thing that is obvious
   only after a real browser rejects it.
3. **Receipt-per-request does not scale to a chain rail.** `verify_receipt_for_item`
   costs an RPC round-trip, and a Godot bundle is hundreds of assets. Verifying
   per asset is unusable on Solana. A short-lived session token exchanged once
   after the first verification is the shape of the answer; it is **not built**,
   and deliberately not invented on the spot. This is a real prerequisite for
   paid rung-0 bundles on a chain rail, tracked in the phases below.

Also: the entitlement gate must run **before** path resolution, so a paid bundle
returns identical 402s for existing and non-existing paths. The file list is part
of what was paid for.

### The COEP consequence, and why it is aligned rather than merely tolerable

`SharedArrayBuffer` is exposed only to a cross-origin-isolated document;
isolation requires `COEP: require-corp`; `require-corp` silently blocks every
cross-origin subresource lacking CORP/CORS. So CDN fonts, remote textures,
analytics and cross-origin iframes all break, and the fix is to vendor them.

That is not just a cost to absorb: **a bundle depending on a third-party host is
not reproducible from its root hash anyway.** Self-containment is what content
addressing already required. An explicit opt-out exists for bundles that do not
need isolation — a three.js scene is fine without it; Godot 4 will not boot.

### Honest verification status

No real Godot export has been exercised — Godot is not installed on the
development machine, and this was not faked. What *is* verified is stronger than
a header assertion: a headless Chromium drives the real server and reads the
actual precondition Godot 4 fails on out of the page — `isSecureContext`,
`crossOriginIsolated`, and whether `new SharedArrayBuffer(8)` constructs — with
a **negative control** (same bundle, isolation disabled) that is what makes the
positive result meaningful.

So the isolation precondition is genuinely proven. The residual untested risk is
Godot-specific export quirks (its own loader's expectations, `.pck` handling),
not cross-origin isolation. Close it by running one real export before promising
Godot support to anyone.

## 6. What magnetite keeps

Everything else becomes a binding. Magnetite owns only what nothing else
provides:

- `AuthoritativeGame` — deterministic `validate` / `step`
- the wasmtime determinism sandbox — fuel, memory cap, epoch interrupt,
  `ENOSYS` on `random_get` / `clock_time_get`
- replay verification and anti-cheat
- topology escalation — `SingleRoom` → `Dedicated` → `Sharded`
- the game package format

| Seam | Binds to |
|---|---|
| Identity / Auth | `kotva_core::identity` |
| Naming | `kotva_core::keyname` |
| Discovery | `kotva_core::pubobj` / `pubsub`; ephor `indexer` for search (optional) |
| BlobStore | Walrus (hot) / Arweave (permanent) / Filecoin (bulk) bindings |
| PaymentRail | patala |
| CommsProvider | ephor `media-relay` + SFrame/MLS (optional) |
| Reachability | direct at R1; ephor `relay` at R2 (optional) |
| Storefront | kotva **TRACT** profile |
| Moderation | ephor `labeler` (optional) |
| Matchmaking | ephor `matcher` (optional) |
| Escrow / dispute | kotva ESCROW + ephor `arbiter` (optional) |

The ~36 feature modules in `backend/src/api/` are largely absorbed by TRACT and
DMTAP profiles rather than reimplemented.

### A contribution back to kotva

Ephor's `compute` kind is marked **provisional** in CONTRACT §5, and its crate
is explicit that job submission, execution, result delivery and TEE attestation
are all future work. Its honest limit is that `attested` visibility trades
operator-trust for chip-vendor-trust (THREAT-MODEL R-4).

Magnetite's sandbox plus `verify_replay` gives something a TEE cannot:
**structural verifiability with no hardware trust**, for the class of jobs that
are deterministic. Anyone re-simulates and locates tampering. That is a
`compute` sub-profile worth proposing — deterministic, replay-verifiable
compute, where correctness is *proved* rather than attested.

## 7. Phases

Each phase is independently useful. Nothing waits on ephor.

### Phase 1 — bind and unblock (no coordinators, no chain)

1. **kotva-core binding.** `identity` + `id` + `keyname` behind the existing
   `Identity` / `Naming` seams, tag-pinned, in its own `magnetite-kotva` crate so
   `magnetite-seams` keeps zero git deps. Offline defaults stay. Spiked and
   measured — see §3. Two follow-ups it surfaced, both real seam changes rather
   than binding work:
   - make `Identity::verify` a method so a provider can supply its own
     verification, instead of `Token::is_valid_at` hard-coding
     `RawKeypairAuth`'s;
   - give the `Identity` seam a key-lifecycle notion (supersede / rotate) so a
     rotated key is the same identity. Without this, key rotation silently
     forks identity.
2. **Package format.** Signed manifest: `kind: web | wasm | web+wasm`, `entry`,
   per-file `path → hash` with a sorted root hash, `price`, split legs in bps.
   Canonical CBOR for the signed bytes. Built, in `magnetite-seams/src/package.rs`
   with `cbor.rs` alongside it, plus `magnetite package build|verify` and
   `load_verified_authority` in the node. Existing wasm game ids are unchanged
   and that is proven by a test; `Package::id()` is an additional identifier
   (it commits to price, split, determinism class and developer key), never a
   replacement. The determinism boundary is enforced at `validate`, `sign` *and*
   `verify`, so a correctly-signed `kind: web` package claiming determinism still
   fails. Three design calls worth keeping:
   - **`size` is signed but is not an input to `root`.** The root addresses
     content; identical bytes have identical length, and including size would let
     a manifest with a wrong size claim a different root for the same bytes. Size
     is checked *before* hashing, so truncation is reported as truncation rather
     than as a hash mismatch.
   - **A frozen wire vector** pins the literal CBOR hex of a known package with
     its root and id, so a refactor that changes the encoding breaks a test
     instead of silently invalidating every signature ever issued.
   - **The developer publishing key is separate from the node key.** Handing a
     node key to an operator must not hand them your publisher identity.
3. **Split legs.** `PaymentSplit` → `Vec<Leg>`, sum-exact. Stewards destination
   out of node env and into the signed release.
4. **Web-bundle serving.** COOP/COEP, precompressed assets, range requests,
   entitlement gate — built as `magnetite-web-host`. Isolation is verified in a
   real browser with a negative control; a real Godot export is still outstanding
   and is the one thing standing between this and claiming Godot support.
   Follow-ups it surfaced:
   - **One manifest, not two.** This crate needed a `root_hash` over sorted
     `path → (hash, len)` and built one, while item 2 is defining the canonical
     signed manifest. These must be reconciled to a single implementation at
     integration — the canonical CBOR-signed manifest is the source of truth and
     the web host consumes it. Two hash-root implementations that drift is the
     worst outcome available here.
   - **Entitlement session token.** Required before paid rung-0 bundles can work
     on a chain rail at all; see §5.
5. **`mag_*` ABI as a public contract.** Normative doc plus a
   language-agnostic conformance harness that loads any conforming module.

### Phase 2 — R1, the public server

6. Adopt FlowStock's node↔node posture — `0.0.0.0`, `http://` *or* `https://`
   peer URLs, encryption delegated to LAN / VPN / tunnel. Then solve
   browser↔node separately, because mixed-content blocking makes TLS mandatory
   there: rustls + ACME, proxy recipe, or tunnel. §2 has the reasoning.
7. WAN validation of migration, membership, follow and the attested wire across
   two real hosts.
8. A node deploy recipe distinct from the legacy backend's.
9. Self-hostable tracker documented as the zero-third-party path.
9b. Evaluate FlowStock's **folder transport** (`docs/SYNC.md` §Folder sync —
   Dropbox, Drive, Syncthing, a NAS mount, a USB stick) for package and
   replay-log distribution. A content-addressed package needs no live socket to
   move, so this is a genuinely networkless distribution path we get almost free.

### Phase 3 — money that actually settles

**Order matters here and it changed:** the patala binding comes before the devnet
round-trip, because the round-trip is blocked on the unit bug and the binding is
what fixes that class of bug rather than the instance. See §4.

10. **patala binding replacing the rail implementations** — adopt
    `PayRequest`/`Receipt`'s `amount_minor` + `currency` + `destination`
    vocabulary so the unit is explicit and the destination is not Ed25519-shaped,
    and port BeepBite's `backend/internal/payments/` gating shape (§4). Do not
    redesign either. This is also what retires the four Solana leaks in §4.
11. Fiat entitlement-revocation policy, capability-driven — and per-rail
    *verification* shape, since chain readback does not exist for fiat or
    Lightning.
12. **Devnet round-trip** — the gate on every economic claim. The Solana rail's
    transaction *construction* path has never run against a validator, and the
    multi-leg version is no more exercised than the single-leg one was.
    **Prerequisite: fix the cents-vs-micro-USDC mismatch above.** Attempting the
    round-trip first would move a wrong amount on devnet and read as a success.
13. Operator charging via revenue-share and entry-fee legs. No channels.

### Phase 4 — the storefront

14. TRACT-shaped catalogue, replacing the custodial `marketplace.rs` surface.
15. Voluntary-contribution visibility — a public list of games and servers
    funding the commons. Non-coercive, and it is what actually funds open
    source.

### Phase 5 — hire coordinators, when they are real

16. ephor `relay` for R2 / CGNAT.
17. `indexer`, `labeler`, `matcher`, `arbiter` as each stops being a scaffold.
18. Propose the deterministic-`compute` sub-profile.

## 8. Prior art in the family — check here before designing

Several sibling repos have already solved pieces of this in production. Read
them before writing anything in the same area.

| Repo | Solved | Where |
|---|---|---|
| **flowstock** | Reachability: only one side needs to be reachable; pair and hub-and-spoke topologies; `http://` *or* `https://` peer URLs; encryption delegated to LAN/VPN/tunnel; ephor as optional convenience only. Also folder-as-transport. | `docs/SYNC.md` §Topologies, §Network, §Independence first, §Folder sync |
| **beepbite** | A shipping patala binding: provider-agnostic interface, build-tag gating so the default build links no gateway code, one env var selects the processor, loud on misconfiguration. Also `canon/`, `money/`, `oplog/`, `nodeid/`, `idempotency/`. | `backend/internal/payments/` |
| **patala** | The rail substrate itself: `RailClass`, `Settlement`, 20 fiat processors, the crypto rails. | `patala-core/src/capabilities.rs`, `patala-fiat/` |
| **wibbly** | Rung 1 in practice — a JS/three.js render loop stepping a magnetite wasm authority as `SingleRoom`, no server. | `packages/wibbly-authority` |
| **kotva / ephor** | The substrate crates and the coordinator kinds. | `crates/kotva-core`, ephor `crates/` |

Two of these change the plan materially: FlowStock's §Network makes Phase 2
smaller, and BeepBite's `payments/` makes Phase 3's binding a port rather than a
design. Neither was found by reasoning about the architecture — both were found
by reading the siblings, which is the argument for doing that first.

**slipscan** was checked and is *not* prior art here: its rustls usage is
client-side HTTP in `slipscan-ingest`, not a server TLS or ACME solution. It
remains a useful reference for the Rust self-hosted-app shape, nothing more.

## 9. Integration debts created by building in parallel

Phase 1 was built as five independent efforts. That was the right call for
throughput, and it produced two duplications that must be resolved to one
implementation each. Both are recorded here because a duplicated invariant that
drifts is worse than either version alone.

**1. THREE canonical-CBOR implementations, not two.** `magnetite-seams/src/cbor.rs` is a
hand-written integer-keyed deterministic CBOR codec written to match
`kotva-core/src/cbor.rs` (DMTAP §18.1.1/§18.1.2) rule for rule — shortest-form
heads, definite length only, keys ascending by encoded bytes, no duplicates, no
floats, no `null`, one top-level item, nesting cap.

Keeping it hand-written is **correct**, and consistent with the finding in §3:
`magnetite-seams` must stay free of git dependencies, so it cannot link
kotva-core. The intended relationship is documented at `cbor.rs:19-41` — its `Cv`
maps 1:1 onto `kotva_core::cbor::Cv` minus `TextMap`, a strictly stricter subset.

**There is a third, and it is the most mature.** `evermesh-kernel/src/codec.rs`
is a canonical CBOR codec whose module docs state cross-implementation byte
identity as **consensus-critical** and which rejects rather than normalizes
non-canonical input — the same rules again — backed by **189 conformance vectors
replayed across three runtimes with zero failures**, plus 201 unit, 7 property and
4 frozen tests. Nothing in the family knows about the other two.

Three independent implementations of an encoding that must agree byte for byte,
with no cross-check between any of them, is a latent signature-compatibility
break. **Required: a conformance test asserting byte equality across all three
for their shared subset.** They are not feature-identical — evermesh supports Text
map keys with a decimal-integer JSON-interchange mapping, while magnetite's `Cv`
is kotva's minus `TextMap` — so the testable property is byte-identity *on the
shared subset*, not wholesale interchangeability.

It belongs in `magnetite-kotva`, the one crate legitimately depending on more than
one. And evermesh's vectors are the obvious thing to test against rather than
inventing new ones — it is the only implementation in the family that already
treats this as consensus-critical and proves it. This cannot be done inside any
single worktree and is therefore an integration task.

**2. Two root-hash implementations, and they disagree.** `magnetite-web-host`
computes a root over sorted `path → (hash, len)`; `magnetite-seams::package`
computes `BLAKE3(ROOT_DOMAIN ‖ cbor([[path, hash], …sorted]))`, deliberately
**excluding** length for the reason given in §7 item 2. These produce different
roots for the same bundle.

The package format's version is canonical — it is the signed one, and its
exclusion of length is reasoned. `magnetite-web-host` must delete its own and
consume `PackageManifest`, routing file lookups through `PackageManifest::file()`
so unlisted paths stay unreachable. That hook already exists and currently has no
caller.

**3. An unfinished doc sweep.** `PROTOCOL_FEE_BPS` no longer exists in code, but
still appears in `docs/economy-marketplace.md`, `docs/index.md`,
`docs/for-developers/*`, `docs/analytics.md`, `docs/troubleshooting.md`,
`src/pages/admin/Finance.jsx` and assorted frontend comments. The canonical env
reference, both payments docs and every machine-readable config that *set* the
dead variables were updated; the prose sweep was not finished. A documented env
var that does nothing is worse than an undocumented one.

**A correction to this document's own guidance.** Earlier revisions warned that
`magnetite-solana-rail` has an out-of-repo path dependency on `../../patala` and
would not build from a worktree. That was **wrong** — it uses a pinned git rev, and
three independent agents confirmed it builds fine. The claim came from
`site/docs/payments.md`, which asserted the crate "requires `../../patala` checked
out" and also advertised a `live_rpc` test that actually lives in patala. Both doc
claims were false and are now fixed. Worth noting as a case of a stale doc
propagating into new design guidance — the same failure mode this document's
status discipline exists to prevent.

## 10. The risk to hold in view

Everything being bound to is pre-alpha. kotva is v0.1.0 "early and evolving".
Ephor is a pre-alpha reference with six of ten kinds as scaffolds. patala has
no rail validated against a live network. Magnetite has no payment settled
through any rail. **Binding four unproven systems together does not produce one
proven system** — it produces a dependency graph in which nothing ships until
everything does, and all four are ours, so nobody else will unblock them.

The mitigation is already the house style: bind to the specs, keep a working
offline default for every seam, pin tags rather than paths, and adopt each
coordinator only once it is real. Phase 1 deliberately depends on no service,
no chain and no coordinator — it is all local libraries and local files.
