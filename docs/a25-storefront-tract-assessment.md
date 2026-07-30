# TRACT-shaped storefront assessment (A25)

**Verdict, up front:** the backlog's framing is half right and half stale. The
module count (~36) is exactly correct. The word **"custodial" is not** — the
money path `marketplace.rs` drives was made non-custodial by commit `87a3624`
on 2026-07-19, eleven days before this backlog line was written, and every
doc in the repo already says so. What is real is that the surface is
**centralised**: one Postgres database is sole authority for the catalogue,
one admin role has unilateral power over refunds, and nothing is
content-addressed. TRACT itself has also moved since the backlog's own
"~19 sections unwritten" note — all 23 sections (`00`–`22`) are now written,
most to normative RFC 2119 text. The single biggest finding of this
assessment: **magnetite's actual product shape (one developer, one game, one
store, digital instant-delivery items, no player-to-player trade) has no
current consumer for TRACT's two hardest, least-finished pieces — escrow and
cross-publisher product identity.** Migrating the storefront now would mean
building against a spec section its own authors mark **PROVISIONAL — pending
decision**, for a problem this codebase does not have. The smallest valuable
step is disclosure, not migration: see "Recommendation" below.

## 1. The legacy surface, counted and named

`backend/src/api/` — counted directly, not estimated:

```
$ ls backend/src/api/*.rs | wc -l
39
$ ls backend/src/api/*.rs | grep -vE '^(mod|middleware|response)\.rs$' | wc -l
36
```

39 files; `mod.rs` (module declarations), `middleware.rs` (auth/admin guards)
and `response.rs` (envelope types) are infrastructure, not domain modules. The
remaining **36** are the domain surface the backlog names:

```
achievements  admin        auth          categories    channels
communities   contact      developer     discovery     distribution
games         github       health        leaderboard   marketplace
matchmaking   messages     metrics       notifications oauth
platform      points       profile       provisioning  replays
reviews       search       sessions      social         streaming
templates     tournaments  versioning    wallet         webhooks
wishlist
```

All 36 are declared in `api/mod.rs` and mounted (checked, not assumed) — this
is live surface, not dead code.

### What `marketplace.rs` actually exposes

`backend/src/api/marketplace.rs` (411 lines) + `backend/src/services/marketplace.rs`
(1187 lines): store CRUD (developer-owned, one store per game, DB-enforced
unique constraint), item CRUD (`sku`/`name`/`price`/`currency: USD|points`/
`kind: cosmetic|item|dlc|pass`), purchase, entitlement check, refund, revenue
summary. Two currencies, two different mechanisms:

- **USD purchase (`purchase_usd`)**: atomic wallet→wallet checkout through
  `PaymentRail::checkout` (`magnetite-seams::payment`). The buyer and
  developer each need a *linked* Ed25519 wallet address (`payment::wallet_of`);
  there is no deposit, no balance, no platform account. The signed `Receipt`
  is verified (`payment::verify_receipt`) before the purchase row is even
  written, and the receipt **is** the entitlement — `verify_entitlement`
  reconstructs and re-checks the rail signature every time, so a row alone
  proves nothing.
- **Points purchase (`purchase_points`)**: `PointsService::spend` debits an
  off-chain integer balance the platform itself keeps in Postgres
  (`services/points.rs`). Points are explicitly documented as "not money."

### Custodial vs. centralised — checked in code, not assumed

**Genuinely custodial (platform holds funds or is a financial intermediary):
none found.** Checked directly:

- `backend/src/services/payment.rs` line 1: *"Payment service — NON-CUSTODIAL
  crypto only (seam §3.6)... no platform-held balances."* `rail()` panics
  rather than falling back to a mock rail in production; `require_wallet`
  refuses to transact for anyone without a linked external wallet.
- `backend/src/api/wallet.rs` line 1: *"NON-CUSTODIAL... There is no balance,
  no deposit, no withdrawal and no payout... The fiat endpoints (`/deposit`,
  `/withdraw`) and the `wallet_balances` / `wallet_transactions` custody
  tables are GONE."* `GET /api/v1/wallet` literally returns `custodial: false`
  on every response.
- `backend/src/api/platform.rs` line 3: *"the fee/payout/deposit/withdraw
  settings that used to live here... were removed — there is no
  platform-held balance."*
- `backend/src/api/admin.rs` (financial reporting, lines 92–172, 438, 764,
  801): repeatedly asserts it *reads* `payment_receipts`, "the live-written
  non-custodial ledger," and that the legacy custody table "has had no writer
  since the non-custodial payment pivot."
- `backend/src/api/tournaments.rs` carries `entry_fee` / `prize_pool` as
  plain `Decimal` columns, but **no code path anywhere charges an entry fee
  or pays out a prize** — grepped for `payment::` calls in that file: zero
  hits. These fields are unwired data, not a live custodial flow.
- git history: `87a3624 feat(pay): non-custodial crypto PaymentRail — rip
  Paystack/Wise/custody, receipt-based entitlements` (2026-07-19) is the pivot
  commit. It, `20e0a18 chore(qa): remove residual fiat/custody...` and later
  `dda9fad`/`0305f71`/`863576d` (today's own A4/A5/B5 unit fixes) are all
  downstream of that pivot, not upstream of it.

**What is genuinely centralised** (single authority, but not fund custody):

1. **One Postgres database is sole source of truth** for stores, items,
   purchases, entitlements — plain auto-generated UUIDs, not content
   addresses. Nothing here converges by hashing the way TRACT's catalogue
   does.
2. **`admin_guard_with_pool` gives one role unilateral power.** `refund_purchase`
   lets an admin void any receipt and revoke any entitlement without the
   counterpart's cryptographic sign-off — TRACT's escrow class instead
   requires "every ruling published as a signed object" (§9.5, though that
   guarantee itself is not yet a MUST — see §2 below).
3. **The points ledger is a real custodial-*shaped* control**, even though it
   is not money: the platform is the sole issuer, and `refund_purchase`'s
   points branch *mints new balance* back to a user unilaterally
   (`PointsService::award`). It is explicitly off-chain by design and the
   repo never claims otherwise, but it is the one place where "the platform
   controls a balance it did not custody as external funds" is literally true.

**Conclusion:** the backlog's "custodial `marketplace.rs` surface" is not an
accurate description of what the code does today. `ALIGNMENT.md`'s own
phase list (§7, item 14: *"TRACT-shaped catalogue, replacing the custodial
`marketplace.rs` surface"*) was written 2026-07-30 — the same day as this
backlog — eleven days **after** the non-custodial pivot landed, and the
non-custodial framing is already documented everywhere else in the same repo
(`docs/economy-marketplace.md`, `docs/architecture.md`, `docs/index.md`,
`docs/overview.md`). The word appears to have been carried forward from an
older draft of the phase list rather than re-verified against the code it
now describes. The honest label for what remains is **"centralised,
Postgres-authoritative, admin-unilateral" — not custodial.**

## 2. What TRACT actually specifies (read directly, 2026-07-30)

`kotva/profiles/tract/` — 23 numbered sections (`00-overview.md` through
`22-erasure.md`), plus `AUTHORING-BRIEF.md` and `FOUNDER-DECISIONS-TRACT.md`.
Word counts range 176–376 lines per section (7,367 lines of spec prose total,
excluding the two briefs). Every section carries a "Drafting status" banner
stating exactly what is normative vs. scoped vs. open — this is not a
sparse skeleton.

**Backlog claim re-verified: "roughly 19 sections unwritten" — this no
longer holds.** That note traces to the 2026-07-23
`wrap-tract-substrate-consolidation` memory entry. As of 2026-07-30, `git log`
shows continuous work on the profile through 2026-07-27 (`e0c3be2` conformance
vectors, `8c116b8` §22 erasure, `047829e` scoping four claims, `4257e49`
`PurchaseAttestation` fix, `839edda` an escrow edge fix) — all **23** sections
exist and carry real normative content. This is the single most important
correction in this assessment: TRACT is materially more finished than the
backlog believes.

**Established facts re-verified, still true:**

- **Product ≠ Offer, mechanically enforced (§2.2).** *"A `ProductRecord`
  (§16.5.1) and an `Offer` (§16.5.2) are separate objects: a record belongs to
  nobody and carries no seller, no price, and no availability... An
  implementation MUST NOT fold price, stock, or seller identity into a
  `ProductRecord`."* Two sellers of the same product publish one shared
  record (content-addressed, converges by construction) and two distinct
  offers.
- **Content-addressed.** Records are plaintext blobs on the DMTAP substrate,
  addressed by hash; convergence is "an emergent consequence of hashing, not
  a registry" (§2.2).
- **Four axes.** Every offer declares Item, Availability, Fulfilment and
  Consideration (`00-overview.md` line 269; detailed in §§2–5).
- **One operator class.** Confirmed in §9.5: escrow is *the* operator class —
  "permissionless entry... per-order choice by both parties... no access to
  identity keys" — entered permissionlessly and chosen per order, not a
  privileged registrar role.
- **§21's evidence contradicts parts of the design, and tax/legal remain
  assertion, not implementation.** Re-verified directly in `21-grounding.md`:
  *"the whole of trust, dispute, tax and legal (§9.6, §10, §11)"* returned
  "**nothing verified**" across two research passes, and *"§11 in particular
  currently answers by assertion — who the marketplace facilitator is when
  there is no marketplace operator... [is] unanswered here."* §9.6 says the
  same about itself: *"The whole of trust, dispute, tax and legal returned
  nothing verified across the grounding passes... This section MUST NOT be
  read as evidenced."*
- **§2.2a: the load-bearing convergence claim is unproven.** *"A July 2026
  literature pass found no deployed system achieving cross-publisher product
  identity without a licensed registry, and the one candidate for
  permissionless crawl-derived resolution was refuted 0-3 under adversarial
  verification."* The two fielded models are GS1 GTIN (licensed, fee-bearing)
  and schema.org `productGroupID` (a nominal string, no uniqueness
  guarantee). Nothing in between is deployed.

**What §9 (Settlement) specifically requires, and what is still open, read
directly rather than summarised:**

- `RailClass` (`CustodialReversible` | `NonCustodialFinal`) is part of the
  wire type on both `PaymentAttestation` and `EscrowScope`, precisely because
  it "changes the buyer's recourse" (§9.3). Substituting one for the other
  without a fresh recorded agreement is a fail-closed rejection
  (`ERR_TRACT_RAIL_CLASS_SUBSTITUTED`).
- `PaymentAttestation` (§9.4.1) references the sealed order **by content
  address only** and carries a **reference**, never funds, never card data.
- `EscrowScope` intersection at checkout (§9.4.2) is **fail-closed**: a
  missing or unparseable field means the operator does *not* cover the
  trade — never the reverse.
- **Escrow's wire representation is explicitly incomplete, marked so by the
  spec itself, not by this assessment:** *"The frozen grammar carries
  neither [a state-transition object nor a ruling object]... Making the
  lifecycle-is-signed and rulings-are-published claims normative therefore
  requires a §16 MAJOR grammar change... This section does not invent those
  bytes"* (§9.4.3). §9.7 leaves the partial-release/split object **"OPEN...
  PROVISIONAL — pending decision."** §9.5 itself flags "a verifiable record
  of rulings" as *"Not yet a MUST — an intended guarantee with no mechanism
  behind it."*
- §9.5a states, as a **measured finding, not a hypothetical**: OpenBazaar's
  escrow was also opt-in, and "bad actors simply declined it — the
  protection went unused precisely where it mattered most."
- §9.6's own honest-limits list: physical custody cannot be made trustless;
  non-custodial programmatic escrow deadlocks on genuine disputes (the
  *only* options on a `NonCustodialFinal` rail are "a timeout that defaults
  to one party" or "an indefinite lock" — no third option exists); the
  escrow operator class is "structurally permanent," unlike DMTAP's
  self-extinguishing roles.

## 3. What already exists that closes part of the gap

**soko** (`~/code/vulos/soko`, 13 crates, 4,686 lines of Rust) is the actual
TRACT reference implementation: `soko-core::Money{minor_units, currency}`,
`soko-settle` (Money/RailClass/EscrowScope/PaymentAttestation types matching
§9 above almost line for line), `soko-feed::address()` — and per its own
`Cargo.toml` dev-dependency comment, `address()` is now cross-checked
byte-for-byte against `kotva_core::id::ContentId` (`kotva-core = "0.2.0"`,
published) under `[dev-dependencies]` only, so soko's production graph gains
nothing from the check while proving the two conventions actually agree.

**soko is a sibling product repo. Owner directive #1 forbids depending on
it, full stop** — the same rule that already forced `magnetite-solana-rail`
and `magnetite-stellar-rail` to be standalone crates rather than importing
`patala` by path, and that made `rotation.rs`/`chunktree.rs` hand-copied
semantics rather than a shared crate. There is no `tract-*` crate on
crates.io to pull instead — only `kotva-core` (identity/id primitives, no
TRACT-specific types at all) is published. **Anything magnetite takes from
TRACT has to be read from the spec and hand-copied, exactly like
`rotation.rs`/`chunktree.rs`/`magnetite-stellar-rail` — there is no
dependency path, published or otherwise, that would shortcut this.**

**What magnetite already has that is TRACT-adjacent:**

- `magnetite-seams/src/package.rs` (A24-era work) — a signed,
  content-addressed `Package` format with per-file hashes and a canonical-CBOR
  signature. This is real, and it is the closest thing magnetite has to a
  TRACT `ProductRecord`. **But it is the structural opposite of §2.2's
  separation**: `Package::id` deliberately *commits to the price and the
  split* ("§7 Phase 1 item 2" in `ALIGNMENT.md`, confirmed in the module
  doc: *"none of which the old id covered"* — price and split are named
  first). TRACT's §2.2 explicitly forbids exactly this: *"An implementation
  MUST NOT fold price, stock, or seller identity into a `ProductRecord`."*
  Reusing this format as-is would be adopting the opposite of TRACT's
  Product≠Offer axis, not a step toward it.
- The two-tier `Settlement` (A14), `PaymentPointer` (A13), the Stellar rail
  (A12) and the chunk tree (A24) are all real, but none of them speak to
  TRACT's catalogue axis (§2–§5) at all — they are settlement/entitlement
  and storage primitives, not storefront-shape primitives. The rail work
  *is* relevant to §9.3's `RailClass` (see Recommendation below) — magnetite's
  rails are, in TRACT's own vocabulary, uniformly `NonCustodialFinal` (mock
  and Stellar both move value atomically with no chargeback path) — but
  nothing in magnetite's wire format says so today.

## 4. Does magnetite's actual product even need what TRACT is hardest at?

Checked, not assumed: grepped the whole backend for peer-to-peer trading,
resale, auction, or any multi-seller-same-item scenario.

```
$ grep -rniE "trade|p2p|resale|auction|marketplace_listing" api/*.rs services/*.rs
(no output)
```

**None exists.** Every purchase in magnetite today is developer → player,
one game, one store per game (DB-enforced unique constraint in
`create_store`), one price. There is no case in the current product where
two sellers publish the same item and need to converge on a shared identity
— which is exactly the scenario §2.2a says is TRACT's *least* proven claim
("no deployed system achieving cross-publisher product identity without a
licensed registry"). There is also no case where a trade needs a
third-party arbiter holding funds between two untrusting strangers — every
sale is instant-delivery digital entitlement, atomic at the rail, with
nothing to dispute custody of mid-transaction. TRACT's escrow class exists
for exactly the case magnetite does not have (a stranger paying a stranger
for something that cannot be delivered atomically, e.g. physical goods).

This mirrors A23's finding about live replay-log distribution: a capability
can be a real design fit and still have **no current consumer** in this
codebase. Recorded honestly rather than assumed away: **could not establish
any requirement, written or implied, for player-to-player trade or
third-party dispute mediation in magnetite** — if one exists it is not in
this repo's code or docs today.

## 5. Recommendation

**Do not migrate the storefront now.** Three independent reasons converge:

1. TRACT's own settlement section marks its escrow-lifecycle and ruling
   wire objects **PROVISIONAL — pending a §16 MAJOR grammar change** (§9.4.3,
   §9.7). Building against that shape today means redoing the work when the
   grammar changes, or building against something that does not exist yet.
2. There is no dependency path. soko (the implementation) is a forbidden
   sibling import; nothing TRACT-specific is on crates.io. Any adoption is
   hand-copied semantics, and hand-copying 36 modules' worth of surface in
   one pass — against a still-partially-open spec — is exactly the
   "half-migrated storefront" this assessment was asked to avoid producing.
3. The two hardest, least-finished parts of TRACT (cross-publisher product
   identity, escrow) do not have a demonstrated consumer in magnetite's
   actual product shape (§4). Adopting them now would be premature
   abstraction for a problem this codebase does not have — the same
   category of mistake A23 flagged for live replay-log transport.

**What I would NOT do first, and why:**

- **Not** the Product≠Offer split. No current scenario needs it (§4); it
  would also directly conflict with the just-shipped `Package` format, which
  intentionally folds price into the content address for a reason that made
  sense for magnetite's real problem (a wasm/web bundle's price and split
  need to be signed alongside its bytes so a node can enforce them without a
  second lookup). Splitting them now, with no consumer, would be rework with
  no payoff and a real regression risk to A24's landed work.
- **Not** escrow. It has no wire shape to build against yet (§2, §9.4.3/§9.7
  are open), no consumer in magnetite (§4), and its own spec section states
  plainly that even a fully-normative version deadlocks on genuine disputes
  with only two dishonest-feeling options (default to one party, or lock
  forever) — not a foundation to build a first storefront step on.
- **Not** a wholesale `backend/src/api/*` rewrite. 36 modules, many with real
  users' data shapes behind them (per the standing honesty rule), migrated
  against a moving 23-section spec, is the definition of "worse than none."

**What I would do first — the smallest independently valuable step:**
**disclose `RailClass` at the point of purchase.** Magnetite's rails
(`MockPaymentRail`, `magnetite-stellar-rail`) are, in TRACT's own
vocabulary, uniformly `NonCustodialFinal` — atomic, no chargeback, no
external reversal path. §9.3 requires this fact be carried on the wire and
disclosed because it *"changes the buyer's recourse."* Today nothing in
`marketplace.rs`'s purchase response, `wallet.rs`'s `LinkedWallet`, or any
doc states this to a buyer in TRACT's terms — `custodial: false` is close
but is not the same claim as "final, no reversal path, unlike a card."
Concretely, this would mean: naming the concept (a `rail_class` field or
equivalent on the purchase/receipt response, valued `NonCustodialFinal`
today, with room for `CustodialReversible` if a card/fiat rail is ever
added) and stating the recourse consequence in `docs/economy-marketplace.md`
next to the existing non-custodial language. This is:

- **Independently valuable on its own** — it is a real buyer-protection
  improvement (an undisclosed "no chargebacks" fact is exactly the kind of
  honesty gap this fanout loop has flagged repeatedly elsewhere), not
  contingent on any future TRACT work landing.
- **Shippable in one pass** — one field, one doc paragraph, no schema
  migration, no new crate, no escrow, no content-addressing.
- **Not a dependency, published or otherwise** — the *concept* (RailClass
  as a wire-carried, disclosed fact) is copied by hand from §9.3, the same
  pattern as `rotation.rs`/`chunktree.rs`/`magnetite-stellar-rail`. No code
  is imported from soko or from kotva's unpublished profile.
- **Does not strand anything** — it changes nothing about how a purchase
  executes; it only names, on the wire, a property that is already true of
  every rail magnetite has.

This is deliberately **not** proposed as done, built, or even scoped in
detail here — per this assessment's brief, that is a decision for whoever
picks up the next increment, recorded as a candidate rather than a
commitment.

## What could not be established

- Whether any magnetite consumer (a game developer, a future feature) has
  ever wanted player-to-player trade or third-party escrow — not written
  down anywhere found in this repo. Absent that requirement, §4's
  no-consumer finding is a capability question, not a backlog item with a
  waiting user.
- Why `ALIGNMENT.md`'s Phase 4 (written 2026-07-30, the same day as this
  backlog) still uses the word "custodial" for `marketplace.rs`, eleven days
  after the non-custodial pivot and next to that document's own explicit
  non-custodial seam table (§6). Could not establish whether this was a
  copy-forward from an older draft or a deliberate looser use of the word;
  either way, §1 above is the ground truth to build on, not the phrase.
- Whether soko's TRACT implementation (§3) has been run against soko's own
  conformance vectors as of today — out of scope for this assessment, which
  is about magnetite's gap, not re-auditing soko's own gate.
