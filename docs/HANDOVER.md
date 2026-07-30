# Handover — 2026-07-30

**You are taking over an in-flight, multi-repo effort.** This file is written for
whoever picks it up (human or LLM) and is accurate as of the moment it was written.
Verify state before acting on it — several agents were mid-write when it was drafted.

**First rule, learned expensively today: verify, do not trust.** Roughly a dozen
claims in these repos turned out false when tested, including two I asserted myself
and acted on. Every "green" below was confirmed by running it, not by reading a
report.

---

## 1. Read these first, in this order

| Document | What it is |
|---|---|
| `docs/cross-repo-backlog.md` | **The master list.** 45 items across six repos, tagged BLOCKER / DECIDED / FOUND / OPEN, plus the corrections and false assumptions |
| `ALIGNMENT.md` | Architecture: kotva substrate binding, reachability rungs, the Stellar decision, integration debts (§9) |
| `docs/chain-candidates.md` | 1,209 lines, 16 L1s, every yes/no with a primary-source URL |
| `patala/docs/shared-economics.md` | Family-wide payment design; the `patala-stellar` multi-op decision |
| `vuna/docs/03-economics.md` | Vuna's economic model — **untracked, commit it** |
| `kotva/substrate/SOVEREIGNTY.md` | Owner ruling, 2026-07-30. Constrains everything below |

---

## 2. Repo state

### magnetite — branch `integration/phase1`, tree CLEAN, all five streams merged

```
2812e3c web-host: cargo fmt
4645c4b web-host: follow PaymentSplit -> Vec<Leg>
b01db26 merge: magnetite-web-host (stream 5/5)
d70c0a5 merge: PaymentSplit -> Vec<Leg> + stewards + dust floor (stream 4/5)
ff50290 A2: mark magnetite-seams::cbor as a temporary SOVEREIGNTY.md §3.5 violation
9ad65ef merge: signed package format (stream 3/5)
4187d6e merge: mag_* ABI fixes + doc audit (stream 2/5)
90202ee merge: magnetite-kotva binding (stream 1/5)
bebe824 docs: ALIGNMENT §9 integration debts + cross-repo backlog + chain candidates
```

`magnetite-seams` passes **116 unit + 8 integration, 0 failures**. Two new crates exist:
`magnetite-kotva`, `magnetite-web-host`.

**Not yet on `main`.** Nothing has been pushed anywhere.

**Three items remain, all small and verified outstanding:**

| # | Item | Exact state |
|---|---|---|
| **A4** | Add `magnetite-web-host` to CI | `ci/rust-crates.json` has **15** entries, zero matches for `web-host`. Needs 16. `scripts/ci-crate-coverage.sh` **fails CI** on any crate on disk but absent from the matrix, so this is mandatory |
| **A10** | Wrong blob store named | `site/docs/seams.md` still says `LocalBlobStore` (the in-memory one whose own comment says "NOT a durability target"). Should be `FsBlobStore` |
| **A5** | **cents vs micro-USDC — 10,000× error** | `units_from_usd` yields **cents**; the rail consumes **micro-USDC**. Three call sites: `backend/src/api/sessions.rs:105`, `backend/src/services/marketplace.rs:530`, `backend/src/services/marketplace.rs:1119`. Nothing has ever settled so no money was mispriced. **This gates every future payment** |

### kotva — dirty, an agent was mid-rewiring

Modified: `Cargo.toml`, `Cargo.lock`, `bindings/README.md`, `crates/kotva-core/Cargo.toml`,
`crates/kotva-core/src/cbor.rs`, and `crates/kotva-cbor/{src/lib.rs,src/json.rs,tests/evermesh_conformance.rs}`.

- `crates/kotva-cbor` **exists, is in `[workspace] members`, compiles, and passes 29 unit + 5 conformance tests.**
- `bindings/README.md`'s Walrus row **was corrected by hand** (the "~1/5 cloud cost" claim was backwards by ~22× vs S3). Do not revert it.
- **UNANSWERED AND IMPORTANT:** do the five tests in `tests/evermesh_conformance.rs` validate against evermesh's *actual* corpus at `/Users/pc/code/vulos/evermesh/tools/conformance`, or a local copy? **A self-referential conformance test proves nothing**, and cross-validating the three canonical-CBOR implementations was the whole point. Answer this before trusting it.
- Outstanding: finish rewiring `kotva-core` onto `kotva-cbor` and delete its own `cbor.rs`, **keeping `kotva-core`'s public API byte-for-byte unchanged** — it is tag-pinned by envoir, ephor and the Go/WASM bindings. If the API cannot be preserved, stop rather than change it.

### vuna — dirty, an agent was mid-brand

Modified `brand/logo.svg`; untracked `brand/{mark.svg,favicon.svg,tokens.css,README.md}`,
`docs/03-economics.md`, `package-lock.json`, and a stray `.tmp-render.mjs` in the repo
root that should be deleted or moved under `scripts/`.

Outstanding: landing page, README rewrite, Tauri app UI pass, screenshots. **The README
must keep its status table and every "not built yet" caveat** — vuna is v0 preview,
runs on mock data, and does not crawl-to-query end to end.

### patala — DO NOT TOUCH WITHOUT READING §3

Tree clean. Log shows a **concurrent session** committing.

---

## 3. Critical warning: a second session is working in patala

patala's reflog shows commits landing *during* this session's work:

```
38b79f9 fix: complete the StellarRpc::load_account_holdings impls my last commit broke
fe264fa rails: offline destination validation, reachable from every binding
3b241c8 brand: redraw logo mark (real beep+tray / clearer cowrie shell)
1e450d9 brand: settle on the cowrie, dual-license the crates, and prove the claims
```

**Consequences, confirmed:**

- **`atomic_multi_party` was destroyed.** It had been threaded through `patala-core` and
  all 20+ fiat rails and verified compiling. It now has **zero occurrences** anywhere in
  patala except the doc describing it. It must be redone.
- Two agents were duplicating that session's work — it was independently redrawing the
  same cowrie mark.
- Attribution became unreliable: uncommitted changes were credited to the wrong agent.

**Both patala agents were deliberately paused and should stay paused** until it is
established who owns that repo. Before doing anything in patala, check whether B1
(multi-operation), B2 (`patala-sui` removal — currently absent, so done) and B3
(`atomic_multi_party` — gone) actually exist.

---

## 4. Working practices that mattered

**Commit per increment.** Eight agents died on 600s no-progress watchdogs. The magnetite
integration lost nothing across three stalls because it committed after each stream. The
patala brand agent lost an entire design iteration because it refined an SVG in context.
Same environment, opposite outcomes. **Treat agent context as scratch that may vanish.**

**Verify state before resuming a dead agent.** Every resume in this session started from
a hand-checked snapshot. Twice the agent's own account of where it had got to was wrong.

**Read the sibling repos before designing.** This changed the answer four times:
flowstock had already solved reachability; beepbite already had a working patala binding;
evermesh has the most-tested canonical codec *and* receipt semantics ahead of magnetite's;
evermesh's `EVMS` container already satisfies a roadmap item. In each case the family's
own index was silent or wrong.

**These repos have an aggressive honest-status culture.** `status.md` distinguishes
Running / LAN-only / Mock-only / Not built. Do not soften a caveat while improving prose.
A prettier README that dilutes "no rail has been run against a live network" is a
regression.

---

## 5. Errors made in this session — do not inherit them

| Claim | Truth |
|---|---|
| "Solana can't do offline signing (~60s blockhash)" | **False.** Durable nonces: "valid until executed." Solana is out on validator weight, not this |
| "Sui can't do offline signing (gas ObjectRefs)" | **False.** `TransactionExpiration::None` — "transactions do not expire by default" |
| "Walrus's WAL exposes operators to volatility" | **Half false.** Priced in USD; the oracle absorbs volatility. Real burden is two balances, no USDC path |
| "Evermesh's gateway solves paid content" | **False.** Its policy engine is a moderation denylist with no viewer parameter |
| "Stellar is the only viable chain" | **False.** Radix, Sui and Solana also clear every hard filter |
| "15/15 crates in CI" | Worktree-local. `main` had 14 |
| "`LocalBlobStore` backs single-operator hosting" | It is in-memory. `FsBlobStore` does |

The pattern: **every claim that got checked, moved.** Assume the same of anything below
that has not been verified.

---

## 6. Suggested order

1. **Finish magnetite**: A4, A10, then A5 (or document A5 precisely at all three call
   sites if the fix is larger than it looks). Full gates, then merge `integration/phase1`
   to `main`.
2. **Commit vuna's `docs/03-economics.md`** — it is untracked and is the only copy.
3. **Resolve the patala ownership question** before touching that repo.
4. **Answer the kotva conformance-corpus question**, then finish the `kotva-core` rewiring.
5. **Then** the rest of `docs/cross-repo-backlog.md` — the seam defects (A7 `Identity::verify`
   as a method, A8 key lifecycle where a rotated key currently becomes a different
   identity), A9 (`BlobStore::get` holds whole blobs in RAM, so range serving is
   inexpressible on every backend), A19 (`wss`/TLS, which gates every browser game), and
   wibbly's re-vendor (F1/F2).

**The single gate on all economic claims across all six products remains: one real
payment settled on a testnet. It has never happened.**
