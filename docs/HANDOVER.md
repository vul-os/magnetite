# Handover — 2026-07-30

> **[2026-07-31] HISTORICAL — a one-time session handover, already superseded.**
> Written to hand a specific in-flight session to its successor; several of its
> own claims were overtaken within the same day by later commits in this repo
> (e.g. item 5's "A19 wss/TLS still needed" was closed by `97fa148`, and its
> closing line about no economic claim ever settling is contradicted by the
> Stellar testnet settlement `docs/cross-repo-backlog.md` A12/A26 record,
> which landed after this file was written). Kept for the record of that
> handover, not as current status — read
> [`docs/cross-repo-backlog.md`](cross-repo-backlog.md) for the live backlog
> and [`docs/project/DECENTRALIZATION_PROGRESS.md`](project/DECENTRALIZATION_PROGRESS.md)
> for the active wave log instead.

**You are taking over an in-flight, multi-repo effort.** Written for whoever picks it
up. Accurate as of writing; **verify before acting** — two repos have uncommitted work
and at least one has a concurrent session.

**First rule, learned expensively today: verify, do not trust.** Roughly a dozen claims
in these repos turned out false when tested, including two I asserted and acted on.
Every "green" below was confirmed by running it.

---

## 1. Read these first, in order

| Document | What it is |
|---|---|
| `docs/cross-repo-backlog.md` | **The master list.** 45 items across six repos, tagged BLOCKER / DECIDED / FOUND / OPEN |
| `ALIGNMENT.md` | Architecture: kotva binding, reachability rungs, the Stellar decision, integration debts (§9) |
| `docs/chain-candidates.md` | 1,209 lines, 16 L1s, every yes/no with a primary-source URL |
| `patala/docs/shared-economics.md` | Family payment design; the `patala-stellar` multi-op decision |
| `vuna/docs/03-economics.md` | Vuna's economic model — **UNTRACKED, commit it** |
| `kotva/substrate/SOVEREIGNTY.md` | Owner ruling 2026-07-30. Constrains everything |

---

## 2. State by repo

### magnetite — DONE and committed. Branch `integration/phase1`, tree clean.

```
0305f71 A4/A5/A10: micro-USDC unit fix, web-host in CI, FsBlobStore
2812e3c web-host: cargo fmt
b01db26 merge: magnetite-web-host (stream 5/5)
d70c0a5 merge: PaymentSplit -> Vec<Leg> + stewards + dust floor (stream 4/5)
ff50290 A2: mark magnetite-seams::cbor as a temporary SOVEREIGNTY.md §3.5 violation
9ad65ef merge: signed package format (stream 3/5)
4187d6e merge: mag_* ABI fixes + doc audit (stream 2/5)
90202ee merge: magnetite-kotva binding (stream 1/5)
```

All five parallel streams merged. **A4, A5 and A10 are complete.** `magnetite-seams`
116+8 tests, `backend` 126 lib tests, all test targets compile, fmt clean, CI crate
coverage gate passes at 16 crates. The five agent worktrees were removed after
verifying each commit was an ancestor of HEAD; **the branches are kept**
(`git branch --list 'worktree-*'`).

**Nothing pushed. Nothing merged to `main`.** That merge is the obvious next action and
was deliberately left to you.

**Note `860e819 brand: refine logo mark` is not from this session's work** — see §3.

**A5 detail, because it matters.** `units_from_usd` multiplied by 100 (cents) while
every rail consumes micro-USDC (6 decimals) — a 10,000× under-charge. It is now
`micro_usdc_from_usd`, returns `Result`, and refuses unrepresentable prices instead of
`unwrap_or(u64::MAX)`. **Four tests were asserting the bug** ($19.99 → 1999), which is
exactly why it survived; they now assert micro-USDC and there is a new test that
overflow refuses.

### kotva — 7 files dirty, near-complete

`crates/kotva-cbor` exists, is in `[workspace] members`, compiles, and passes **29 unit
+ 5 conformance tests**. `bindings/README.md`'s Walrus row was corrected by hand — do
not revert it.

**Outstanding:**
- **UNANSWERED, and blocking trust in the whole exercise:** do the five tests in
  `tests/evermesh_conformance.rs` validate against evermesh's **actual** corpus at
  `/Users/pc/code/vulos/evermesh/tools/conformance`, or a local copy? A self-referential
  conformance test proves nothing, and cross-validating the three canonical-CBOR
  implementations was the entire point.
- Finish rewiring `kotva-core` onto `kotva-cbor` and delete its own `cbor.rs`, **keeping
  `kotva-core`'s public API byte-for-byte unchanged** — it is tag-pinned by envoir,
  pier and the Go/WASM bindings. If the API cannot be preserved, stop rather than
  change it.
- The agent reported `cargo package` succeeds, i.e. publication-ready. **It was told not
  to publish.** Crates.io publication is approved in principle and is what lets
  `magnetite-seams` drop its duplicate codec (A2), but it is permanent — confirm before
  doing it.

### vuna — 48 files dirty, brand work complete, uncommitted

A full brand/landing/README/app pass finished and self-verified. Highlights worth
knowing before you review it:

- **Contrast measured, not estimated**, WCAG 2.1 with tables in `brand/README.md`. Three
  real failures were found and fixed: `--ink-3` at 4.47:1 on `--sunk`; bright gold at
  **2.46:1** on paper, under even the non-text floor, so light-theme gold graphics use a
  deeper token and bright gold is only permitted as a fill under dark ink; and control
  borders got a dedicated `--edge` token at ≥3:1.
- **Accent hue moved off rust to grain gold** because vuna sat within a few degrees of
  both kotva and patala — three of five siblings in the same orange read as one product.
- **Mock labelling is enforced, not asserted.** The screenshot script refuses to write an
  app screenshot unless a mock disclosure is inside the captured region. It failed a run
  and blocked the write, which is the proof it works.
- Status caveats preserved verbatim and **strengthened** — "132 tests green is not 132
  features shipped… zero live nodes, zero real users and zero bytes of real index".
- Left deliberately: `app/src-tauri/icons/*` still carry the old mark (needs the
  packaging toolchain; regeneration command documented), and two pre-existing unreferenced
  `docs/shots/hero-*.png`.

**`docs/03-economics.md` is untracked and is the only copy.** Commit it first.

### patala — DO NOT TOUCH. See §3.

---

## 3. Critical: concurrent sessions are committing in these repos

patala's reflog shows commits landing *during* this session's work:

```
38b79f9 fix: complete the StellarRpc::load_account_holdings impls my last commit broke
fe264fa rails: offline destination validation, reachable from every binding
3b241c8 brand: redraw logo mark (real beep+tray / clearer cowrie shell)
```

**Confirmed damage:** `atomic_multi_party` was threaded through `patala-core` and all
20+ fiat rails and verified compiling. It now has **zero occurrences** anywhere in patala
except the doc describing it. Two agents were also duplicating that session's work — it
was independently redrawing the same cowrie mark.

**Both patala agents were paused and should stay paused** until repo ownership is
settled. Before doing anything there, re-verify B1 (multi-operation), B2 (`patala-sui`
removal — currently absent, so done) and B3 (`atomic_multi_party` — gone, must be redone).

**magnetite may have the same issue**: `860e819 brand: refine logo mark` appeared on
`integration/phase1` and is not from any agent this session launched. Check before
assuming the branch is solely yours.

---

## 4. Working practices that earned their keep

**Commit per increment.** Ten agents died on 600s no-progress watchdogs. The magnetite
integration lost nothing across four stalls because it committed after each stream; the
patala brand agent lost an entire design iteration because it refined an SVG in context.
Same environment, opposite outcomes. **Treat agent context as scratch that may vanish.**

**Verify state before resuming a dead agent.** Every resume started from a hand-checked
snapshot. Twice the agent's own account of where it had got to was wrong.

**Read the sibling repos before designing.** This changed the answer four times:
flowstock had already solved reachability; beepbite already had a working patala binding;
evermesh has the most-tested canonical codec *and* receipt semantics ahead of magnetite's;
evermesh's `EVMS` container already satisfies a roadmap item. Each time the family's own
index was silent or wrong.

**Aggressive honest-status culture.** `status.md` distinguishes Running / LAN-only /
Mock-only / Not built. Never soften a caveat while improving prose.

---

## 5. Errors made this session — do not inherit them

| Claim | Truth |
|---|---|
| "Solana can't sign offline (~60s blockhash)" | **False.** Durable nonces — "valid until executed." Solana is out on validator weight |
| "Sui can't sign offline (gas ObjectRefs)" | **False.** `TransactionExpiration::None` — "transactions do not expire by default" |
| "Walrus's WAL exposes operators to volatility" | **Half false.** Priced in USD; the oracle absorbs it. Real burden is two balances, no USDC path |
| "Evermesh's gateway solves paid content" | **False.** Its policy engine is a moderation denylist with no viewer parameter |
| "Stellar is the only viable chain" | **False.** Radix, Sui and Solana also clear every hard filter |
| "15/15 crates in CI" | Worktree-local. `main` had 14 |
| "`LocalBlobStore` backs single-operator hosting" | It is in-memory. `FsBlobStore` does |

**Every claim that got checked, moved.** Assume the same of anything not yet verified.

---

## 6. Next actions

1. **Merge `integration/phase1` → `main`** in magnetite (after checking §3's stray commit).
2. **Commit vuna** — `docs/03-economics.md` first, it is the only copy; then review the
   brand work (design is subjective, look at it before committing 48 files).
3. **Answer kotva's conformance-corpus question**, finish the `kotva-core` rewiring, then
   decide on crates.io publication.
4. **Settle patala ownership**, then redo B3 and finish B1.
5. Then the backlog: A7 (`Identity::verify` as a method), A8 (key lifecycle — a rotated
   key currently becomes a different identity), A9 (`BlobStore::get` holds whole blobs in
   RAM, so range serving is inexpressible on every backend), A19 (`wss`/TLS, which gates
   every browser game), F1/F2 (wibbly re-vendor + provenance).

**The gate on every economic claim across all six products is unchanged: one real payment
settled on a testnet. It has never happened.**
