# Cross-repo backlog — everything decided or discovered on 2026-07-30

Captured so it is not lost. Spans **magnetite, patala, kotva, evermesh, vuna, wibbly**.
Status markers: **[BLOCKER]** gates other work · **[DECIDED]** decision made, work
outstanding · **[FOUND]** discovered defect · **[OPEN]** needs a decision.

Design records this summarises: `ALIGNMENT.md`, `docs/chain-candidates.md`,
`docs/sui-binding-spike.md`, `docs/walrus-assessment.md`,
`patala/docs/shared-economics.md`, `vuna/docs/03-economics.md`,
`evermesh/DECISIONS.md` (X1–X3), `kotva/substrate/SOVEREIGNTY.md`.

---

## A. Magnetite — integration debt

| # | Item | State |
|---|---|---|
| A1 | **Integrate five worktrees**: kotva binding, PaymentSplit legs + stewards, package format, web-host, ABI fixes. Seven bug fixes and ~12 doc corrections are sitting unmerged | **[BLOCKER]** |
| A2 | Collapse duplicate canonical CBOR — `magnetite-seams/src/cbor.rs` vs `kotva-cbor`. **Now a normative violation**: SOVEREIGNTY.md §3.5 requires running the *shared compiled* algebra, so byte-equality is not sufficient | **[BLOCKER]** |
| A3 | Collapse duplicate root hash — `magnetite-web-host` includes file length, `magnetite-seams::package` deliberately excludes it. Package format is canonical; web-host must consume `PackageManifest` and route lookups through `PackageManifest::file()` (exists, no caller) | **[BLOCKER]** |
| A4 | Add `magnetite-kotva` + `magnetite-web-host` to `main` and `ci/rust-crates.json` (currently **14**, not the 15 reported from a worktree). `scripts/ci-crate-coverage.sh` fails on unregistered crates | **[BLOCKER]** |
| A5 | **Fix cents-vs-micro-USDC mismatch** — `backend`'s `units_from_usd` yields cents, the rail consumes micro-USDC. 10,000× error. Gates any settlement | **[BLOCKER]** |
| A6 | Finish `PROTOCOL_FEE_BPS` doc sweep: `docs/economy-marketplace.md`, `docs/index.md`, `docs/for-developers/*`, `docs/analytics.md`, `docs/troubleshooting.md`, `src/pages/admin/Finance.jsx` | **[FOUND]** |
| A7 | Seam defect — `Identity::verify` is an associated function; `Token::is_valid_at` hard-codes `RawKeypairAuth`, so any provider with different signature bytes silently breaks token verification. Make it a method | **[FOUND]** |
| A8 | Seam defect — no key-lifecycle notion. `DeviceCert`/`RecoveryPolicy`/`KeyRotation`/`MoveRecord` have nowhere to land, so **a rotated key becomes a different identity**. Largest gap in the kotva binding | **[FOUND]** |
| A9 | `BlobStore::get` returns `Option<Vec<u8>>` — whole blob in RAM. A 2 GB Unity bundle is unservable and range serving is inexpressible **on every backend equally**. This is the real storage work | **[FOUND]** |
| A10 | `site/docs/seams.md` row 3 names `LocalBlobStore` (in-memory, "NOT a durability target"); `FsBlobStore` is what backs R1 | **[FOUND]** |
| A11 | Three content-hash conventions in the family: magnetite and evermesh both bare BLAKE3-256, kotva's `ContentId` is `0x1e`-prefixed. Same disease as the CBOR codecs, unaddressed | **[OPEN]** |
| A12 | Retire `magnetite-solana-rail` **after** the Stellar rail lands — port the ten fail-closed checks and the scripted-fake-RPC pattern first, then delete rather than leave feature-gated. **2026-07-30: precondition met** (patala-stellar settled one testnet payment, tx `32663937fe1407f9de3e781effa6ac9f4b1d29340ea63e72f6335a6c91effb89`, ledger `3882739`) and `magnetite-stellar-rail` now exists — standalone (no patala dependency), all ten checks ported and individually mutation-tested (disable/red/revert), 63 offline tests green, fmt+clippy clean. **Deletion of `magnetite-solana-rail` is BLOCKED, not skipped**: `backend/Cargo.toml`'s `solana` feature path-depends on it and `backend/src/services/payment.rs` wires `PAYMENT_RAIL=solana` to it — both are `backend/`, off-limits this session because another agent held uncommitted changes there (including that exact file). Deleting the crate without also editing those two files would break `backend`'s build outright (a documented Cargo path-dependency defect, not a feature-flag issue). Half-done is correct per this item's own decision text: land the rail, leave solana in place, do not delete a working rail to replace it with one that cannot yet be wired in. Next iteration: once `backend/` is free, swap `backend/Cargo.toml`'s `magnetite-solana-rail` dep + `solana` feature for `magnetite-stellar-rail`, delete the old crate, remove it from `ci/rust-crates.json` | **[PARTIALLY DONE — new rail landed, deletion blocked by `backend/` territory conflict]** |
| A13 | Adopt evermesh's `PaymentPointer` registry rather than minting a parallel declaration format | **[DECIDED]** |
| A14 | Two-tier entitlement: **settled** (chain-verified) vs **signed-but-unsettled** (locally verified, pending). Required for disconnected operation; retires the chain-retention question | **[DECIDED]** |
| A15 | Entitlement session token — `verify_receipt_for_item` costs an RPC per asset, and a Godot bundle is hundreds of assets. Not built, deliberately not invented; `evaluate` has one call site so it has one place to land | **[OPEN]** |
| A16 | Real Godot 4 export never exercised. Isolation precondition *is* verified in a real browser with a negative control; residual risk is Godot's own loader and `.pck` handling | **[FOUND]** |
| A17 | Rung-0 templates: three.js, Godot web, Unity wasm | **[DECIDED]** |
| A18 | `mag_*` ABI: a non-Rust module beyond hand-written WAT (Zig / TinyGo / AssemblyScript). No toolchain available on this machine | **[OPEN]** |
| A19 | `wss://` / TLS on the node — **partially addressed 2026-07-30.** The node's own listener is still plaintext `ws://` and unchanged (`magnetite-runtime/src/server.rs`); a reverse-proxy recipe (`magnetite-runtime/deploy/Caddyfile.example`) now exists, matching flowstock's/pango's own CLOUD-NODE.md pattern, and was verified end-to-end locally: a real TLS handshake + WebSocket upgrade + the node's real `ServerNet::Welcome` frame, through Caddy's local CA (`docs/hosting-a-server.md` "Running players over `wss://`"). rustls+ACME in-process remains unbuilt and unexercised — no domain was available to prove real issuance. This unblocks R1 for an operator willing to run a proxy; it does not make the node itself TLS-capable | **[PARTIALLY UNBLOCKED for R1]** |
| A20 | WAN validation of shard migration, cluster membership, session follow and the attested input wire across two real hosts | **[OPEN]** |
| A21 | Node deploy recipe distinct from the legacy backend's (`fly.toml` targets the old central backend) | **[OPEN]** |
| A22 | Document the self-hostable tracker as the zero-third-party path | **[OPEN]** |
| A23 | Evaluate FlowStock's **folder transport** (`docs/SYNC.md` §Folder sync) for package and replay-log distribution — content-addressed packages need no live socket. **Assessed** (`docs/folder-transport-assessment.md`): it is real, tested Go code (`flowstock/backend/internal/sync/folder.go`), not just a doc pattern — but it is not a dependency (Go, coupled to FlowStock's own CRDT store; kotva has no equivalent to publish). **Decided for packages**: adopt the *pattern* only — a content-addressed naming convention plus the manifest/chunk-tree verification magnetite already has is sufficient; any file-sync tool (rsync/Syncthing/Dropbox/USB) already does the copying, no new transport code. **Still open for replay logs**: a *finished* log needs one small addition it currently lacks (a whole-log hash + signature — `backend/src/api/replays.rs` stores an unsigned, uncontent-addressed JSON blob today) before the same conclusion applies; a *live, still-recording* log is the right shape for FlowStock's design (single writer, tailed incrementally) but would be new magnetite code, and would need an authenticity layer FlowStock's own bare folder path also lacks (its per-op signing is a separate, network-only mechanism) | **[DECIDED for packages] / [OPEN for replay logs]** |
| A24 | Take evermesh's **1 MiB Merkle chunk tree** (makes range reads *verifiable*) and the **`EVMS` bundle container** — the latter already satisfies roadmap item 9b, specified and conformance-tested. Do not design a bundle container | **[DECIDED]** |
| A25 | TRACT-shaped storefront replacing the custodial `marketplace.rs` surface (~36 legacy API modules) | **Assessed** (`docs/a25-storefront-tract-assessment.md`), too large to build in one pass by design — map + smallest-step recommendation only, not attempted. Module count confirmed exactly (36, counted directly: 39 files in `backend/src/api/` minus `mod.rs`/`middleware.rs`/`response.rs`). **"Custodial" does not hold**: `marketplace.rs`'s USD path is atomic wallet→wallet through `PaymentRail` (non-custodial since `87a3624`, 2026-07-19 — eleven days before this backlog line was written) and `wallet.rs`/`platform.rs` both assert no balance/deposit/withdraw exists; checked directly, not assumed. What's real is **centralised**, not custodial: one Postgres DB is sole catalogue authority, admin has unilateral refund/void power, nothing is content-addressed. Only the points ledger is genuinely platform-minted/controlled, and it is explicitly not money. **TRACT itself has moved past the 2026-07-23 "~19 sections unwritten" note** — all 23 sections (`00`–`22`) are now written, most to normative RFC 2119 text (re-verified by reading the files, not the note); Product≠Offer/content-addressed/four-axes/one-operator-class and §21's evidence-contradicts-design + tax-and-legal-still-assertion all re-confirmed still true by direct reading. soko (13 crates) is the real TRACT implementation, cross-checked byte-for-byte against `kotva-core::ContentId` — but it's a sibling product, forbidden to depend on by owner directive #1, and no `tract-*` crate is published to crates.io, so any adoption is hand-copied semantics only, same as `rotation.rs`/`chunktree.rs`. **No player-to-player trade, resale, or auction exists anywhere in magnetite** (grepped, zero hits) — TRACT's two hardest, least-finished pieces (cross-publisher product identity, escrow) have no current consumer here; escrow's own wire objects are marked **PROVISIONAL — pending a §16 MAJOR change** in TRACT §9.4.3/§9.7. **Recommendation: do not migrate now** — no stable wire shape to build against, no dependency path, no consumer for the hard parts. Explicitly NOT-first: the Product≠Offer split (no consumer, and conflicts with `Package::id` intentionally folding price+split into the content address for A24's real problem) and escrow (no wire shape, no consumer, deadlocks by the spec's own admission). **Smallest first step, not yet built**: disclose `RailClass` (`NonCustodialFinal`, per TRACT §9.3) at point of purchase — one field plus a doc paragraph, independently valuable as a buyer-honesty fix, no schema migration, no new crate | **[OPEN] — assessed, decision needed** |
| A26 | Verify Stellar history retention — *not* a gate (self-proving receipts retire it), but worth knowing. **Assessed** (`docs/stellar-history-retention.md`, primary sources with URLs): `patala-stellar::verify`'s steps 1–6 (signature/arithmetic self-consistency) are offline and survive any retention loss forever; only step 7 (Horizon confirms the tx and its envelope) degrades, and it fails **closed** (a clean 404 ⇒ `Ok(false)`, a false negative, never a false accept). Concretely for today's testnet tx (`32663937fe…`, ledger `3882739`, `horizon-testnet.stellar.org`): testnet does a **full reset to genesis 2–4×/year** (Stellar Core + Horizon + Stellar RPC together) — a harder deadline than mainnet's rolling one-year Horizon truncation. **2026-07-30, wired**: `magnetite-stellar-rail` now overrides `verify_receipt_for_item_tiered` (`StellarPaymentRail::verify_tiered_async`) — checks 1–5 run offline exactly as before; a Horizon miss (`Ok(None)`) or an operational failure to even ask (`Err`) both degrade to `Settlement::SignedUnsettled`, while Horizon *answering* that the transaction failed or disagreed with the receipt (checks 7–10) still refuses outright. Mutation-tested per branch (miss→`Settled`, RPC-error→`Settled`, offline-tamper→`SignedUnsettled`, and chain-refusal→`SignedUnsettled` all independently confirmed to go red, then reverted); 7 new tests, 70/70 green. `magnetite-solana-rail` (scheduled for retirement, A12) deliberately left untouched. `magnetite-web-host`'s policy was revisited and **kept refuse-by-default** for `Verdict::GrantedUnsettled` — see `entitlement.rs`'s doc for the argument (asymmetric, irreversible loss vs. a recoverable retry; the rail's own `HorizonRpc` is still unproven live; an opt-in serving policy is separate, real product work). A Horizon miss (pruned or reset) now reads as "signed but unconfirmed" at the rail layer, though this node still declines to serve on that tier today by considered policy, not by omission | **[DECIDED] — wired into `magnetite-stellar-rail`; web-host policy kept conservative on purpose** |

## B. Patala

| # | Item | State |
|---|---|---|
| B1 | **Extend `patala-stellar` to multi-operation.** Builds one `Payment` (`tx.rs:169`), verification rejects multiples (`tx.rs:237`). Must land **here, not in a consumer adapter** — magnetite needs it for splits, vuna for citation splits, soko for multi-seller orders. Highest-leverage dedup available | **[DECIDED]** |
| B2 | Discard the abandoned `patala-sui/` (untracked) plus modified `Cargo.toml`/`Cargo.lock` — the Sui decision is withdrawn | **[DECIDED]** |
| B3 | Add `atomic_multi_party: bool` to `RailCapabilities`, so a consumer declares its requirement and an incapable rail is **refused** rather than silently degrading. A fiat processor cannot be atomic — N payouts are N API calls | **[DECIDED]** |
| B4 | **Recurring primitive** — N pre-signed, time-bounded transactions on a dedicated source account, `minSeqNum`/`minSeqAge`/`minSeqLedgerGap` to relax sequence coupling. Non-custodial, no contract, cancellable | **[DECIDED]** |
| B5 | Adopt `PayRequest`'s `amount_minor` + `currency` vocabulary across consumers — makes A5's bug class structurally impossible | **[DECIDED]** |
| B6 | Brand: logo, colour scheme, font. Landing page, README, UI/UX, screenshots | **[OPEN]** |
| B7 | **Stellar testnet round-trip** — the gate on every economic claim in every product | **[BLOCKER]** |

## C. Kotva

| # | Item | State |
|---|---|---|
| C1 | Finish `kotva-cbor` extraction — agent died mid-task; `src/json.rs` and `tests/evermesh_conformance.rs` untracked, `src/lib.rs` modified | **[BLOCKER for A2]** |
| C2 | Cross-validate all three CBOR codecs against **evermesh's 189 conformance vectors** — the only implementation in the family treating byte-identity as consensus-critical and proving it. Test the shared subset (evermesh has Text map keys; magnetite's is kotva-minus-`TextMap`) | **[BLOCKER for A2]** |
| C3 | Publish `kotva-cbor` to crates.io — user authorised the approach; **confirm explicitly before publishing**, it is permanent | **[DECIDED]** |
| C4 | Fix `bindings/README.md` Walrus row: "~1/5 cloud cost" is **backwards by ~22× vs S3 and ~75× vs B2** (effective $0.1035/GB/month). Add the WAL-token and 53-epoch lease caveats, neither currently flagged | **[FOUND]** |
| C5 | Recurring `PAY` semantics as a profile-level concern — `PAY` is already one of the seven primitives | **[OPEN]** |

## D. Evermesh

| # | Item | State |
|---|---|---|
| D1 | **Proposed, not applied**: allocate a `stellar` `PaymentPointer` type (next free id after 4). Needs a spec revision with `003-kinds-registry.md` coordination — it allocates a wire id | **[OPEN]** |
| D2 | Evermesh adopts **no chain**; `010` §1's rail-neutral registry stands. Recorded as X1 | **[DECIDED]** |
| D3 | `010` §2's receipt discipline is prior art the siblings adopt, not the reverse | **[DECIDED]** |

## E. Vuna

| # | Item | State |
|---|---|---|
| E1 | Brand: logo, colour scheme, font. Landing page, README, UI/UX, screenshots | **[OPEN]** |
| E2 | Economics model written (`docs/03-economics.md`). First build step is **not** economic: `vuna-query` + `vuna-node` are Wave 2 stubs and it cannot crawl-to-query | **[OPEN]** |
| E3 | Signed tariff + **consumer-signed** usage receipts — accounting only, no money, netting to zero. Testable offline | **[OPEN]** |
| E4 | Unsolved: **under-reported citations**. A node saves money by claiming fewer citations. Consumer-signed receipts over answer + citation list help; not solved | **[OPEN]** |

## F. Wibbly

| # | Item | State |
|---|---|---|
| F1 | **Re-vendor the wasm** — needed for the input-discarding fix, the `on_join` fix, the per-tick RNG change, and the new `tick` field in `mag_step`. No change needed for `mag_restore` framing (it was already right; the host changed) | **[BLOCKER]** |
| F2 | Record provenance beside the vendored `.wasm`: magnetite commit, module `sha256`, declared `mag_abi_version`, with the driver asserting the version at load. Its absence is *why* a driver and a module disagreed for months | **[FOUND]** |

---

## Corrections made to this project's own claims

Recorded because the pattern matters more than the individual errors — in every case
a claim survived because nothing tested it.

- **`verify_replay` "Running" survives** — it starts at tick 0 and never restores.
- **Shard migration's "state intact" was false twice** — a target that could not decode
  a snapshot kept its own state *while the handoff reported success*.
- **README's "identical `state_hash` on every tick" was true but vacuous** — measured
  with empty inputs, which is why the input bug survived for months.
- **`on_join` was never called by any production path**, so the reference game never had
  a player and every input returned `Unauthorized`. It *masked* the input bug: fixing
  either alone would have looked like it changed nothing.
- `GAPS.md` miscounted WASI stub imports (10, not 9). `lib.rs` described spawn positions
  as seed-derived arena corners; they are a circle by join index, seed-independent.
- `site/docs/payments.md` claimed the crate "requires `../../patala` checked out" and
  advertised a `live_rpc` test living in patala. Both false — it uses a pinned git rev.
  **This stale doc propagated into new design guidance.**
- `DECISIONS.md` N3-2's "empty inputs baseline" *was* the blind spot; superseded rather
  than rewritten.

## Assumptions that moved when checked

- **Solana offline signing** — durable nonces remove the blockhash TTL entirely.
  "Valid until executed." Solana's rejection stands on validator weight instead.
- **Sui offline signing** — `TransactionExpiration::None`, "transactions do not expire
  by default." The pinned gas coin *is* the replay protection.
- **Walrus's WAL exposure** — storage is priced in **USD**; the oracle absorbs
  volatility, not the operator. The real burden is two balances and no USDC path.
- **Evermesh's gateway** — its "policy engine" is a moderation denylist with no viewer
  parameter; `magnetite-web-host` already does strictly more.
- **Cardano** — fails on ~0.97 ADA minimum per output (a floor on payment *size*), not
  primarily on finality. And its Ed25519 works with plain 32-byte keys.
- **Stellar is not uniquely qualified** — Radix, Sui and Solana also clear every hard
  filter.
