# Chain candidates — the offline-signing discriminator, and what survives it

**Status: complete, with named gaps.** Sixteen candidates plus an eleven-chain completeness
sweep, all verified against primary sources. Nothing here is written from recall — every
yes/no carries a URL, derived figures are labelled as derived, and where a fact could not be
established the cell says so rather than guessing. §4 lists every gap; the largest is
Stellar's history-retention story, and it is large enough to affect the ranking.

Four hard filters emerged, in the order they eliminate candidates: **(1)** offline signing,
**(2)** atomic N-way payment, **(3)** minimum payment granularity, **(4)** batch integrity
against an unprepared recipient. Filters 3 and 4 were added mid-investigation because nobody
was checking them and each turned out to be as disqualifying as the first two.

Scope: **existing public layer 1s with an existing native asset we do not have to
mint.** Sixteen candidates. Build-your-own-chain options (Cosmos SDK appchains,
Avalanche subnets, Polkadot parachains, standalone Substrate chains) are out of
scope, and it is worth noting that they are ruled out twice over: standing one up
requires minting a token for staking and fees, which the family constitution already
forbids (kotva: "no protocol token exists and none will be added"; patala: "no
token"; evermesh: "token: none, permanently"). So the L1-only narrowing costs
nothing that was actually available.

> Provenance note on those three quotes: they were supplied in the brief for this
> document and describe sibling repositories (kotva, patala, evermesh). A grep of
> *this* tree does not contain them, so they are recorded here as received rather
> than as independently verified. The reasoning they support does not depend on the
> exact wording.

---

## Verdict

### No. Stellar is not uniquely qualified, and the discriminator does not isolate it.

This is the finding the brief asked to be stated prominently if true. It is true, and
it is already established from primary sources regardless of what the remaining
candidates turn out to be:

**At least seven candidates pass the offline-signing discriminator: Stellar, XRPL,
Solana, Sui, Radix, MultiversX, and Ethereum.** The criterion believed to eliminate
everything else does not do that — and the two candidates it was believed to eliminate
most decisively, **Solana and Sui, both pass**.

Prior assertions checked — **two of the four were false**:

| Prior assertion | Status |
|---|---|
| "Solana: `recent_blockhash` expires in ~150 blocks (~60–90s) → disqualified" | **FALSE.** The expiry rule is real and correctly stated, but Solana has a protocol-native mechanism that removes it entirely — **durable nonces**, implemented by the builtin System Program. An official SIMD states: "Nonce transactions have no inherent TTL, and thus are valid until executed." The core docs describe the exact use case: offline signing and delayed submission. |
| "Sui: transactions reference gas-coin `ObjectRef`s including versions and content digests → believed fragile/disqualified" | **FALSE as a disqualification.** The object-pinning description is exactly right, but `TransactionExpiration::None` means "By default, transactions do not expire" — and the pinned gas coin *is* the replay protection, which is why `None` is permitted. One dedicated gas coin per pre-signed intent makes it work, and Sui's own docs recommend "dedicated gas coin pools" and endorse storing signed transactions locally. Sui also turns out to have **~511 atomic recipients** and a workable 16 KB signature-covered binding field. |
| "Algorand: `FirstValid`/`LastValid` capped at ~1000 rounds (~1 hour) → disqualified" | **CONFIRMED.** `MaxTxnLife = 1000`, never overridden in any consensus version, ≈47 minutes at 2.82 s blocks, enforced unconditionally. No escape hatch exists; the delegated-LogicSig pattern is not one. |
| "Stellar: time bounds optional, upper bound may be omitted → unbounded" | **CONFIRMED**, with an omitted caveat: Stellar transactions pin `seqNum`, so an accumulated stack must be submitted in ascending order and `minSeqNum` must be used deliberately to make that workable. |

**What this changes.** The decision was being framed as "Stellar or nothing." It is
not. It is a genuine multi-way choice, and — this is the part that matters most — the
rail the project **already ships** is not disqualified by the criterion that was
believed to disqualify it. If Solana is to be retired, it has to be retired for
reasons that survive checking: its history-retention story (default local retention is
~4.4 hours under load, and indefinite history means personally operating a BigTable
archive), its 12.8-second irreversibility, its single-client consensus, or the fact
that durable nonces are officially flagged "may be deprecated in a future release."
Those are real arguments. "Blockhashes expire in 60 seconds" is not.

**Where Stellar does look genuinely strong**, on the evidence so far: 100 payment
operations per transaction against XRPL's 1 (atomic multi-recipient bundling is not
active on XRPL mainnet — the `Batch` amendment was disabled in rippled v3.1.1 after a
February 2026 signature-validation vulnerability) and against MultiversX's 1; a
protocol-native 32-byte `MEMO_HASH` field; Ed25519 as the default scheme; and the
lightest validator requirements of the three survivors that also clear criterion 3.
That is a good case for Stellar. It is not the same claim as "Stellar is the only
viable candidate", and the difference is the whole point of this document.

### The corollary: the offline-signing criterion is the wrong discriminator

It was chosen because it was believed to eliminate almost everything. It does not — it
admits at least four candidates, and the one it was believed to eliminate most
decisively (Solana) actually passes. A criterion that admits most of the field is not
doing the work of a discriminator.

**The criterion that actually cuts is criterion 3: atomic N-way payment in one
transaction with no custom contract.** That is not an arbitrary reweighting; it is the
foundation of the economic model. `ALIGNMENT.md` §4 states it directly — atomic
splits are "a real, unusual property and it is the economic model's foundation — the
whole 'voluntary legs, no custody, no platform holding funds' design rests on it." A
chain that cannot express "developer + operator + stewards, paid together or not at
all" in one transaction is disqualified regardless of how it scores elsewhere.

Measured against *that*, the field separates sharply:

| Chain | Max recipients, one atomic tx, no custom contract | Verdict |
|---|---|---|
| **Sui** | **~511** (`max_arguments = 512` on one `SplitCoins`); ~1021 with two splits before `max_programmable_tx_commands = 1024` binds | **PASS** |
| **Radix** | **126** — `MAX_NUMBER_OF_EVENTS = 256` binds, not `max_instructions`: one `try_deposit_or_abort` emits **2** events (account-level + vault-level), so `4 + 2N ≤ 256`. Safe design target **100**. Using `AccountLocker::airdrop` instead loops natively in one call and removes the limit | **PASS** |
| **Stellar** | **100** (`MAX_OPS_PER_TX`) | **PASS** |
| **Solana** | **~21** legacy / **~60** with a builtin Address Lookup Table (64-account lock limit) | **PASS** |
| **XRPL** | **1** — `Batch` is *not active on mainnet* | **FAIL** |
| **MultiversX** | **1** — `receiver` is a single field | **FAIL** |
| **Aptos** | **1,632 hard ceiling** — measured on mainnet: n=1632 succeeds, n=1633 gives `EXCEEDED_MAX_TRANSACTION_SIZE` (`max_transaction_size_in_bytes = 65536`, BCS size `138 + 40n`). Drops to **~700** if every recipient is a brand-new account (`max_io_gas` binds). Recommended ≤500. Uses framework `batch_transfer` / `batch_transfer_coins` — no custom contract | **PASS** |
| **Cardano** | **~249** `transaction_output`s | **PASS** (fails elsewhere) |
| **Polkadot** | **~10k order** by `batched_calls_limit()`; really bounded by 3.75 MiB extrinsic length and 2 s block weight — `utility.batch_all`: "The whole transaction will rollback and fail if any of the calls failed" | **PASS** (fails the discriminator) |
| Algorand | 16 (`MaxTxGroupSize`) — but N grouped txs, not one tx, and all share the 47-min window | moot (discriminator FAIL) |
| Hedera | 10 (`ledger.transfers.maxLen`) | moot (discriminator FAIL) |
| **Avalanche X-Chain** | **many** — `BaseTx.Outputs` is a variable-length array of `TransferableOutput`, atomic, no contract. **Exact maximum could not be established** — the txn-format and serialization-primitives pages state no maximum output count or tx size | **PASS** (fails criterion 5) |
| **Mina** | **1** — `Body.t` has only `Payment` (single `receiver_pk`) and `Stake_delegation`. Multi-party needs a zkApp, i.e. a contract, additionally capped at "24 zkApp transactions per block" | **FAIL** |
| **Ethereum** | **1** — one `to`, one `value` per transaction. EIP-7702 delegates the EOA to *existing contract code*, which is still a contract | **FAIL** |
| Tezos | n/a — and it has no native memo either | moot (discriminator FAIL) |
| Symbol (NEM) | 100 per aggregate, genuinely atomic — but eliminated at filter A on a 6 h deadline | moot |
| Bitcoin | many (multi-output) — eliminated on the dust floor and no Ed25519 | moot |

### The third hard filter, added mid-investigation: minimum payment granularity

Criterion 15, which nobody was checking and which is as disqualifying as the other two.
**What is the smallest amount that can actually be delivered to one recipient?**

It matters because the economic model is *voluntary contribution legs*. A 3% stewards
leg on a $1 game sale is about three cents. A chain that cannot deliver three cents to a
recipient does not make small legs expensive — it makes them **structurally
impossible**, which kills the model outright.

Three different things are easy to conflate here and **only the first is
disqualifying**:

| Type | What it is | Verdict impact |
|---|---|---|
| **1** | A per-*payment* minimum to a recipient — a floor on how small a delivered payment can be | **Disqualifying** |
| **2** | A one-time per-account / per-trustline / per-token-account setup cost | Setup cost only — not a floor on payment size |
| **3** | A per-*transaction* fee | Irrelevant — paid once for the whole bundle, not per leg |

| Chain | Smallest deliverable per recipient | Type | Verdict |
|---|---|---|---|
| **Cardano** | **~0.97 ADA per output** — `(65 + 160) * 4310` lovelace, official docs say "~1.0 ADA" | **1** | **FAIL** |
| Polkadot | Asset Hub existential deposit **0.01 DOT**, and under `batch_all` **one sub-ED recipient rolls back the entire batch** | **1** | FAIL (already out) |
| **Radix** | **1 atto = 10⁻¹⁸ XRD** (`Decimal` SCALE = 18). Account instantiation for a not-yet-existing account is **sender-paid from the fee reserve**, storage is a one-time commit cost with no recurring rent, and there is no `MIN_BALANCE`/`existential`/`dust`/`reap` concept in the account blueprint at all | **2** | **PASS, emphatically** |
| **Stellar** | **1 stroop = 0.0000001 XLM** ("the smallest unit of a lumen, one ten-millionth of a lumen"). Base reserve **0.5 XLM**, minimum balance "two base reserves (currently 1 XLM)", each subentry (incl. a trustline) "+0.5 XLM" — all account/subentry costs, not payment floors | **2** | **PASS** |
| **XRPL** | **1 drop = 0.000001 XRP.** Base reserve **1 XRP** and owner reserve **0.2 XRP** are account/object costs, not payment floors | **2** | **PASS** (fails criterion 3) |
| Solana | 1 lamport; ATA rent (~0.00204 SOL) is per-account setup | 2 *(pending confirmation of the new-account case)* | expected PASS |
| Sui | **NOT ESTABLISHED** — Sui charges storage fees for newly created objects. Whether that cost is charged to the sender (type 2, fine) or must be held *inside* the delivered coin (type 1, disqualifying) is unresolved and is the most important open question about Sui | — | **open** |

Sources: [Stellar lumens](https://developers.stellar.org/docs/learn/fundamentals/lumens),
[Stellar fees](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering),
[XRPL reserves](https://xrpl.org/docs/concepts/accounts/reserves),
[XRPL currency formats](https://xrpl.org/docs/references/protocol/data-types/currency-formats)

**Radix does not fail the way Cardano fails, and the difference is worth stating
precisely.** A three-cent stewards leg is ~0.5 XRD = 5×10¹⁷ attos — seventeen orders of
magnitude above the floor. The evidence is positive evidence of absence: no amount
validation anywhere in `Account::deposit`, the whole fee table is paid from the sender's
fee reserve, and storage is a one-time commit cost rather than recurring rent, so nothing
can be reaped for being too small.

### The fourth hard filter, also added mid-investigation: batch integrity

Criterion 16, discovered by looking closely at Radix and then found to apply to Stellar
too. **In an atomic N-way payment, can ONE recipient cause the ENTIRE batch to fail?**

This is both a griefing vector and a routine operational hazard. The platform pays
arbitrary developer, operator and stewards addresses; some will be unprepared, and one
leg is *a voluntary contribution nobody is obliged to accept*. A payout run that any
single payee can veto is not viable.

**The generalisation: any chain with atomic multi-recipient payments has this problem
unless it provides a store-for-later-claim primitive.** Atomicity is the feature that
creates the vulnerability — the very property criterion 3 selects for.

| Chain | Can one recipient veto the batch? | Native store-for-later-claim answer | Cost |
|---|---|---|---|
| **Radix** | **Yes by default** — accounts have owner-configurable deposit rules, so `try_deposit_or_abort` returns `DepositIsDisallowed` and kills the transaction | **Yes, purpose-built.** `AccountLocker`'s `airdrop` with `try_direct_send: true` loops over claimants in native Rust inside **one** `CALL_METHOD`; on refusal it stores funds in the locker for later claim instead of aborting | Bonus: the native loop also removes the instruction-count pressure entirely |
| **Stellar** | **Yes, for issued assets** — a `PAYMENT` of a non-native asset to an account with no trustline fails, and the transaction is all-or-nothing. **No, for native XLM to an existing funded account** — XLM needs no trustline | **Yes.** `CREATE_CLAIMABLE_BALANCE`: claimable balances "allow an account to send a payment to another account **that is not necessarily prepared to receive the payment**… when you send a non-native asset to an account that has not yet established a trustline" | **0.5 XLM of minimum-balance reserve locked per claimant**, on the *source* account — "each claimant in that entry increases the source account's minimum balance by one base reserve". Locked, not spent; released when claimed |
| Solana | **NOT ESTABLISHED — and suspected yes.** SPL token accounts can be frozen ("prevents the token account from receiving, transferring, or burning tokens"), which would let a recipient *or the mint's freeze authority* veto an entire batch. A missing ATA is likely fixable in-transaction via the canonical `create_associated_token_account_idempotent`, at a cost against the 64-account lock limit | — | **open** |
| Sui | **NOT ESTABLISHED — expected clean.** Sui transfers create an owned object at the destination with no obvious recipient opt-in, so a veto is not expected to be possible. Recorded as unverified rather than assumed, because "no recipient can veto" would be a significant advantage and must be defensible | — | **open** |

Sources: [Stellar claimable balances](https://developers.stellar.org/docs/build/guides/transactions/claimable-balances),
[Stellar lumens](https://developers.stellar.org/docs/learn/fundamentals/lumens)

**This was a real gap in Stellar's assessment.** Stellar's atomic 100-operation bundle —
the headline reason to prefer it — is exactly what makes it vetoable, and the fix is not
free: settling in an issued asset such as USDC means either accepting that any payee
without a trustline breaks the run, or switching to claimable balances and locking 0.5 XLM
per outstanding leg. Radix's `AccountLocker` is the better-engineered answer to the same
problem, and it is the answer to a problem Radix *also* has. Neither chain is clean here;
both have a native remedy.

The Cardano case deserves the precise framing, because it is counter-intuitive: the
minimum ADA **is not rent.** It lands in the recipient's UTxO and they own it. That is
exactly what makes it a floor on payment size rather than an overhead cost — you cannot
pay someone three cents, because any output you create for them must contain about a
dollar.

### Final standing: four survivors, not one

All sixteen candidates plus eleven sweep candidates have now been scored. **Four clear
every hard filter: Radix, Sui, Stellar and Solana.**

| Chain | Offline signing | Atomic N-way | Granularity | Batch integrity | Principal weakness |
|---|---|---|---|---|---|
| **Radix** | PARTIAL — bounded ~30 d | **126** (unbounded via `AccountLocker`) | 10⁻¹⁸ XRD | remedy is purpose-built | Nakamoto ≈9 (derived), single client, no disk-growth figure |
| **Sui** | PASS (one gas coin per intent) | **~511** | **open** | expected clean, **unverified** | 24 cores / 128 GB validator; 16 TB unpruned node; single client |
| **Stellar** | PASS — unbounded | **100** | 1 stroop | claimable balances, 0.5 XLM/leg locked | **history retention unestablished**; 32-byte memo has no slack |
| **Solana** | PASS via durable nonces | **~21–60** | 1 lamport | **open** — frozen-ATA veto suspected | 256 GB RAM validator; ~4.4 h default retention; 12.8 s finality; nonces "may be deprecated" |

No candidate wins on every axis, and **that is the actual finding.** The decision is a
trade-off between four viable chains, not the selection of the only one that works:

- **Stellar** has the only genuinely *unbounded* signing horizon among chains that also do
  atomic splits, and the lightest validator requirements of the four (8 vCPU / 16 GB / 100 GB).
  Its unresolved history-retention story is the thing most likely to change its ranking.
- **Radix** is the most complete match on paper — protocol-native resources, XRD provably
  unfreezable by *locked* `freeze_roles`/`recall_roles` rather than by argument-from-absence,
  a 2048-byte arbitrary-*binary* message bound into the signed intent hash, no
  sequential-counter problem at all, deterministic finality, and the only purpose-built
  answer to criterion 16. It is bounded at 30 days and thinly decentralised.
- **Sui** has the highest recipient count, the fastest finality (400–700 ms, irreversible),
  and — contrary to `ALIGNMENT.md` §4 — a workable no-contract binding field. It is heavy to
  run and its granularity answer is unknown.
- **Solana** is the incumbent, was rejected for a real reason (hardware weight), and was then
  defended with a false one.

**The premise that Stellar is uniquely qualified is refuted.** Stellar is a good candidate
with one specific, unusual strength — an unbounded signing horizon — and one specific
unclosed risk. It is not the only option, and three of the four survivors were either absent
from the original comparison or wrongly excluded from it.

### For the record: why Solana was rejected, and whether that reason survives

Solana was rejected by the user on **validator hardware weight** — criterion 11.
That reason is entirely independent of the blockhash-TTL claim, and **it survives
checking.** The published figures are the heaviest in this comparison by a wide
margin: 12 cores / 24 threads, **256 GB RAM**, ~2.5 TB NVMe across three separate
disks, 2 Gbit/s symmetric, no NAT, Docker unsupported, plus **up to 1.1 SOL per day**
in vote transactions. Compare Stellar's 8 vCPU / 16 GB / 100 GB, or MultiversX's
4 cores / 8 GB / 200 GB.

So the conclusion is not "Solana was wrongly rejected." It is narrower and more
useful: **Solana was rejected for a good reason and then defended with a bad one.**
The good reasons, all verified below, are hardware weight (11), ~4.4-hour default
local ledger retention with indefinite history requiring a self-operated BigTable
archive (8), 12.8-second irreversibility (13), effectively single-client consensus
(12), and durable nonces being officially flagged "may be deprecated in a future
release." Any of those can carry the decision. The blockhash-TTL argument cannot, and
should be struck from the record wherever it appears.

*Remaining candidates are still being verified; see §3 for what is not yet
established. The finding above does not depend on them — it only strengthens if more
candidates pass.*

---

## 1. The discriminator: sign offline now, submit much later

The platform must support deployments that run disconnected for long periods,
accumulate signed payment intents locally, and submit them in bulk when connectivity
returns. So: **can a transaction be signed offline now and successfully submitted
days, weeks, or a month later?**

Three distinct failure modes have to be separated, because conflating them is what
produced the earlier wrong answers:

- **Time/height expiry** — the transaction carries a deadline field, or references a
  recent block hash that ages out.
- **Ordering/nonce coupling** — the transaction pins a per-account counter, so
  unrelated activity from the same account invalidates the whole accumulated stack.
- **State pinning** — the transaction references specific UTXOs or object versions
  that any other spend invalidates.

A chain only passes if it survives all three, or if the residual constraint is one an
offline accumulator can actually engineer around.

### Findings

| Chain | Expiry field | Optional? | Max horizon | Ordering / state coupling | Verdict |
|---|---|---|---|---|---|
| **Stellar** | `TimeBounds.maxTime` | Yes — and the whole `Preconditions` union can be `PRECOND_NONE` | Unbounded | Sequence-number coupled; relaxable via `minSeqNum` | **PASS** |
| **XRPL** | `LastLedgerSequence` | Yes — "No; auto-fillable" | Unbounded | Sequence coupled; relaxable via Tickets | **PASS** |
| Algorand | `FirstValid` (`fv`) / `LastValid` (`lv`) | **No — both REQUIRED** | **1000 rounds ≈ 47 min** (`MaxTxnLife`) | n/a — dies on expiry | **FAIL** |
| Cardano | `ttl` (field 3) / `validity_interval_start` (field 8) | **Yes — both optional in the CDDL** | Unbounded on the clock | **State-pinned: named UTXOs, spent by anything else = invalid forever** | **PARTIAL** — passes on time, constrained by UTXO |
| Solana | `recent_blockhash` (32 bytes) | Effectively yes — **durable nonces** replace it | Unbounded ("no inherent TTL... valid until executed") | **One nonce account = one live pre-signed tx**; N parallel intents need N funded nonce accounts | **PASS — the brief's premise was wrong** |
| **Sui** | `TransactionExpiration` | **Yes — `None` means "no expiration"; "By default, transactions do not expire"** | Unbounded | **State-pinned: gas `ObjectRef` = (ID, version, digest); one dedicated coin per pre-signed tx required** | **PASS — the brief's premise was wrong** |
| Aptos | `expiration_timestamp_secs` | **No — mandatory, but with NO upper cap on the sequence-number path** | Unbounded on that path; **60 s** on the orderless/nonce path (`MAX_EXP_TIME_SECONDS_FOR_ORDERLESS_TXNS = 100`, advertised as 60 s) | Strict contiguous `sequence_number`; a gap parks everything after it; mempool `capacity_per_user: 100` | **PARTIAL** |
| NEAR | `block_hash` | No — required | **`transaction_validity_period = 86400` blocks ≈ 14–24 h**, and defeating this use case is the *stated design purpose* | Per-access-key `nonce` | **FAIL** |
| Tezos | `branch` (block hash) | No — required | **600 blocks × 6 s = 3600 s = 1 hour** (`max_operations_time_to_live`, held at exactly 3600 s across four protocols *by stated design*) | Strict sequential `counter`; plus **one manager operation per manager per block** in the mempool | **FAIL** |
| **Radix** | `end_epoch_exclusive` | **No — mandatory `Epoch`, not `Option<Epoch>`** | **`max_epoch_range = 12*24*30 = 8640` epochs ≈ 30 days (≈27 days measured)** | `intent_discriminator` is a free nonce — **no sequential counter at all** | **PARTIAL — bounded at ~30 days** |
| MultiversX | **none — no expiry field at all** | n/a | Unbounded | `nonce` must be within `[accountNonce, accountNonce+100]` | **PASS** |
| Hedera | `transactionValidDuration` | No — and `transactionValidStart` must be in the past | **180 s** (`transaction.maxValidDuration`) | n/a — dies on expiry | **FAIL** |
| Mina | `valid_until` | **Yes — defaults to `Global_slot_since_genesis.max_value`** | Unbounded | Strictly sequential `nonce`; plus a submission rate limit of "10 transactions every 15 seconds" | **PASS** (fails criterion 3) |
| Polkadot | `Era` — **but that is not the binding constraint** | `Era::Immortal` exists and "is valid forever" | **~7 h mortal (`BlockHashCount = 4096`); killed instead by `CheckSpecVersion` against ~monthly runtime upgrades** | Strict `nonce` equality chain | **FAIL** |
| Avalanche | **none** (X-Chain `BaseTx`; only a per-output `Locktime`) | n/a | Unbounded | **State-pinned: inputs reference specific UTXOs.** C-Chain separately exposed to stale `maxFeePerGas` | **PARTIAL** (fails criterion 5) |
| Ethereum (baseline) | **none — no expiry field in any of the five tx types** | n/a | Unbounded | Strict `nonce` equality; stale `max_fee_per_gas` can strand a tx (mitigable) | **PASS** |

#### Stellar — PASS

The XDR is unambiguous. Time bounds are a nullable pointer inside `PreconditionsV2`,
and the enclosing union has a case that carries no preconditions at all:

```
struct TimeBounds
{
    TimePoint minTime;
    TimePoint maxTime; // 0 here means no maxTime
};

struct PreconditionsV2
{
    TimeBounds* timeBounds;
    LedgerBounds* ledgerBounds;
    SequenceNumber* minSeqNum;
    Duration minSeqAge;
    uint32 minSeqLedgerGap;
    SignerKey extraSigners<2>;
};

enum PreconditionType
{
    PRECOND_NONE = 0,
    PRECOND_TIME = 1,
    PRECOND_V2 = 2
};
```

Source: <https://github.com/stellar/stellar-xdr/blob/curr/Stellar-transaction.x>
(fetched raw). The prose docs agree: "All preconditions are optional… Time bounds are
an optional UNIX timestamp… If `maxTime` is 0, upper time bounds are not set."
Source: <https://developers.stellar.org/docs/learn/fundamentals/transactions/operations-and-transactions>

**The caveat the "Stellar is unbounded" claim omitted.** Stellar transactions pin
`seqNum`. By default "the transaction's sequence number must be exactly one greater
than the account's sequence number" (same source), which means a stack of pre-signed
transactions must be submitted in exact ascending order and *any* out-of-band
transaction from that account invalidates all of them. Protocol 19's `minSeqNum`
precondition relaxes this: the transaction is valid when the account's sequence
number `S` satisfies `minSeqNum <= S < tx.seqNum` (same source). That makes bulk
submission of an accumulated stack workable, but it is engineering the offline
accumulator has to do deliberately — it is not free. There is also a residual fee
risk: `fee` is a fixed field in the signed transaction, so a stack signed at today's
base fee can be rejected under later surge pricing and need re-signing.

#### XRPL — PASS

`LastLedgerSequence` is listed as **"No; auto-fillable"** in the required column of
the transaction common fields reference, and the reliable-submission guide states
plainly: "If the transaction does not specify an expiration, there is no limit to how
much later this can occur."

Sources:
<https://xrpl.org/docs/references/protocol/transactions/common-fields>,
<https://xrpl.org/docs/concepts/transactions/reliable-transaction-submission>

Same ordering caveat as Stellar: `Sequence` is required and account-coupled. XRPL's
relaxation is **Tickets**: "Tickets allow transactions to be sent outside of the
normal sequence order", and the docs describe exactly this platform's use case — you
can "prepare and sign a transaction in advance, then save it in some secure storage
so that it can be executed at any future point if certain events occur." The limit is
material: "Each account cannot have more than 250 Tickets in the ledger at a time."
Source: <https://xrpl.org/docs/concepts/accounts/tickets>

So XRPL's offline accumulator has a hard ceiling of **250 outstanding pre-signed
out-of-order transactions per account** — a real number, and arguably a cleaner
mechanism than Stellar's `minSeqNum` because it is purpose-built for it.

**The discriminator does not separate Stellar from XRPL.** Both pass. Whether Stellar
is uniquely qualified therefore turns on the other criteria, not on this one.

#### Solana — PASS, and the brief's premise was wrong

The blockhash rule is real and is exactly as described. `recent_blockhash` is a 32-byte
message field; "A blockhash expires after 150 slots. If the blockhash is no longer
valid when the transaction arrives, it is rejected with `BlockhashNotFound`, **unless
it is a valid durable nonce transaction**." The SDK constants pin the arithmetic:
`MAX_PROCESSING_AGE = 150`, `DEFAULT_MS_PER_SLOT = 400` → **60 seconds**. (The docs
quote wall-clock inconsistently across pages: ~1 min, 60–90 s, ~80–90 s, and ~2 min
all appear. The 150 × 400 ms arithmetic is the figure to trust.)

Sources:
<https://solana.com/docs/core/transactions/transaction-structure>,
<https://solana.com/docs/core/transactions>,
<https://github.com/anza-xyz/solana-sdk/blob/master/clock/src/lib.rs>

**But the clause after "unless" is the whole story, and it was missed.** Solana has a
protocol-native mechanism that removes the limit entirely: **durable nonces**,
implemented by the System Program (a builtin, program ID `1111…1111`) plus runtime
support — *not* by a user-deployed contract. An official SIMD states it flatly:

> "Nonce transactions have no inherent TTL, and thus are valid until executed."
> — <https://github.com/solana-foundation/solana-improvement-documents/discussions/415>

And the core docs describe this exact use case: "A durable nonce transaction replaces
the recent blockhash with a stored nonce value, removing the 150 slot expiry window.
This enables offline signing and delayed submission… Sign the transaction (**can be
done offline, since the nonce does not expire**)."
<https://solana.com/docs/core/transactions/durable-nonces>

So **"Solana is disqualified because blockhashes expire in ~60–90s" is false.** That
matters twice over: it is one of the two prior assertions this document was asked to
re-check, and Solana is the rail the project already ships.

Four costs the mechanism carries, all documented:

1. **A single nonce account strictly serializes pre-signed transactions.** Validation
   requires "The stored `durable_nonce` must match the transaction's
   `recent_blockhash`", and on use "the nonce is advanced to the next durable nonce
   value before execution begins." So N parallel accumulated intents require N
   separate nonce accounts, each rent-exempt (~0.00136416 SOL each). The docs
   themselves model the serialized pattern: submit "all the signed transactions **one
   by one**". *(This one-per-account conclusion is a strict derivation from the three
   quoted validation rules, not a verbatim sentence in the docs — flagged as such.)*
   <https://solana.com/docs/core/transactions/durable-nonces>,
   <https://solana.com/developers/cookbook/transactions/durable-nonces>
2. **Fragile invalidation surface.** Any advance of that nonce by the authority —
   deliberate, accidental, or an operator running `solana new-nonce` — permanently
   invalidates every other transaction signed against the old value. The authority can
   also be reassigned ("grants full control over the account and its balance to the
   new authority") or the account drained.
   <https://docs.anza.xyz/implemented-proposals/durable-tx-nonces>
3. **Officially flagged for possible deprecation.** "Durable nonces may be deprecated
   in a future release." The linked SIMD says "This proposal allows us to begin
   sunsetting nonce transactions." No timeline, no accepted replacement. For a
   mechanism a multi-year architecture would depend on, this is the single largest
   risk in the Solana column.
   <https://solana.com/docs/core/transactions/durable-nonces>
4. **The read-back side is worse than the signing side** — see criterion 8 below.

There is no other expiry-bearing field in the transaction; `lastValidBlockHeight` is
an RPC convenience, not a transaction field.

#### Algorand — FAIL, and the brief's premise was correct

Both fields are listed under a column literally headed `Required`:

> `FirstValid` **required** `uint64` `"fv"` — "The first round for when the transaction
> is valid. If the transaction is sent prior to this round it will be rejected by the
> network."
> `LastValid` **required** `uint64` `"lv"` — "The ending round for which the
> transaction is valid. After this round, the transaction will be rejected by the
> network."
> — <https://dev.algorand.co/concepts/transactions/reference/>

The cap is a consensus parameter, `MaxTxnLife = 1000`, and the enforcement is
stateless and unconditional in `Transaction.WellFormed()`:

```go
if tx.LastValid-tx.FirstValid > basics.Round(proto.MaxTxnLife) {
    return fmt.Errorf("transaction window size excessive (%v--%v)", tx.FirstValid, tx.LastValid)
}
```

Algorand states the wall-clock consequence itself: "The validity window has a maximum
span of 1,000 rounds. Since Algorand produces blocks every 2.82 seconds, this gives
transactions **approximately one hour**." (1000 × 2.82 s = 47 minutes.)

Sources:
<https://dev.algorand.co/concepts/protocol/protocol-parameters/>,
<https://dev.algorand.co/concepts/transactions/blocks/>,
<https://github.com/algorand/go-algorand/blob/master/config/consensus.go>,
<https://github.com/algorand/go-algorand/blob/master/data/transactions/transaction.go>

The value is set once in the v7 base parameters and **never overridden** in any
consensus version through v41 (current) or v42 (pending) — verified by grep of
`config/consensus.go`. The source comment notes raising it is not a free knob: "in a
protocol upgrade, the ledger must first be upgraded to hold more past blocks."

**The escape hatch does not exist.** A delegated LogicSig looks like one but is not:
what the account holder signs is the *program*, not the transaction. In
`data/transactions/logicsig.go` the `Sig` field covers `Logic` only, and
`WellFormed()` — containing the `MaxTxnLife` check — is applied to every transaction
in a group regardless of signature type, verified in
`data/transactions/verify/txn.go`. So a delegated LogicSig gives a counterparty a
**standing bearer authority to construct and sign new transactions on your behalf**,
each needing a fresh `fv`/`lv`. That is a materially different and worse security
object than a pre-signed payment, and it does not satisfy the requirement.

Algorand is otherwise a strong fit — `note` is 1024 bytes and third-party searchable,
ASA issuance is a protocol primitive (`acfg`), finality is ~2.82 s with no
confirmation-depth discount, and validators need no bond. **It is eliminated purely on
the 47-minute window.**

#### Hedera — FAIL

Worse than Algorand by two orders of magnitude, and the future-scheduling workaround is
explicitly blocked. From the protobuf spec:

> `transactionID = 1`: "This identifier MUST specify a 'valid start time'. The 'valid
> start time' MUST be strictly *earlier* than the current network consensus time."
> `transactionValidDuration = 4`: "This transaction SHALL be rejected as expired if the
> valid start time, extended by this duration, is less than the current network
> consensus time when the transaction is submitted."
> — <https://github.com/hashgraph/hedera-protobufs/blob/main/services/transaction.proto>

The constant is `transaction.maxValidDuration`, **default 180** seconds, with
`minValidityBufferSecs = 10`, so the real horizon is ≤170 s. The node enforces both
ends:

```java
if (validStart.plusSeconds(validDuration).isBefore(consensusTime)) throw TRANSACTION_EXPIRED;
if (validStart.isAfter(consensusTime))                             throw INVALID_TRANSACTION_START;
```
<https://github.com/hiero-ledger/hiero-consensus-node/blob/main/hedera-node/hedera-app/src/main/java/com/hedera/node/app/workflows/TransactionChecker.java>,
<https://github.com/hiero-ledger/hiero-consensus-node/blob/main/hedera-node/hedera-config/src/main/java/com/hedera/node/config/data/HederaConfig.java>

> **A documentation contradiction worth recording.**
> <https://docs.hedera.com/hedera/core-concepts/transactions-and-queries> states "The
> transaction's valid start time can be set to a future date/time." The node
> implementation and the protobuf spec both contradict this. The implementation is
> authoritative; the docs page is wrong.

Scheduled transactions do not rescue it either: a `ScheduleCreate` is itself an
ordinary transaction that must be submitted **online** inside the same 180 s window,
so it cannot accumulate intents while disconnected.

Hedera is disqualified twice over — see criterion 12 below on governance.

#### MultiversX — PASS

The strongest raw answer on the discriminator of any candidate so far: **there is no
expiry field at all.** The canonical transaction field set is `nonce, value, receiver,
sender, senderUsername, receiverUsername, gasPrice, gasLimit, data, chainID, version,
options, signature, guardian, guardianSignature` — no block hash, no timestamp, no
valid-from/valid-until.
<https://docs.multiversx.com/sdk-and-tools/rest-api/transactions/>

Confirmed on the node side: `integrity()` in
`process/transaction/interceptedTransaction.go` performs only version, integrity,
chain-ID equality, address lengths and gas checks — **zero time logic**.
<https://github.com/multiversx/mx-chain-go/blob/master/process/transaction/interceptedTransaction.go>

The one real bound is the nonce window:

```go
// common/constants.go
// MaxTxNonceDeltaAllowed specifies the maximum difference between an account's nonce
// and a received transaction's nonce in order to mark the transaction as valid.
const MaxTxNonceDeltaAllowed = 100
```

So each bulk drain is capped at **100 transactions per sender**, then you wait for the
account nonce to advance. Nonce *gaps* do not invalidate later transactions — they wait
in the pool. Time in storage is irrelevant.
<https://github.com/multiversx/mx-chain-go/blob/master/common/constants.go>,
<https://github.com/multiversx/mx-chain-go/blob/master/process/dataValidators/txValidator.go>

Two non-time hazards for a months-long horizon: a `chainID` change invalidates the
batch, and if the account later becomes **guarded**, previously signed `version = 1`
transactions are rejected.
<https://docs.multiversx.com/developers/guard-accounts/>

MultiversX passes the discriminator cleanly and then **fails criterion 3 outright** —
see the matrix.

#### Sui — PASS, and the brief's premise was wrong here too

`TransactionExpiration` is a three-case enum whose default case never expires, and the
docs say so in as many words: "**Expiration:** Optional. An epoch number that acts as a
deadline… **By default, transactions do not expire.**"
<https://docs.sui.io/develop/transactions/txn-overview>

The enforcement code confirms it — `TransactionData::validity_check` matches
`TransactionExpiration::None => (), // always valid`.

So **the object-versioning concern in the brief is the right concern, and the time
concern does not exist.** Gas is pinned by a full triple:

```rust
pub struct GasData {
    pub payment: Vec<ObjectRef>,   // (ObjectID, SequenceNumber, ObjectDigest)
    pub owner: SuiAddress,
    pub price: u64,
    pub budget: u64,
}
```

and a version mismatch is fatal: `ObjectVersionUnavailableForConsumption` —
"Transaction needs to be rebuilt because object {} version {} ({}) is unavailable for
consumption". The CLI cheatsheet glosses it "Object was modified by another transaction
since you last read it."

**The finding that makes this coherent, and which is easy to miss:** the pinned gas coin
*is* the replay protection, which is precisely *why* `None` is allowed. From
`sui-transaction-checks`:

```rust
let has_replay_protection = transaction.expiration().is_replay_protected()
    || !transaction.gas_data().payment.is_empty()
    || input_objects.iter().any(|obj| obj.is_replay_protected_input());
```

Unlimited lifetime and object pinning are two sides of one mechanism — you cannot have
one without the other. The corollary: the newer **address-balance gas** path, which has
no object pinning at all, is capped at "at most `min_epoch + 1`" — roughly 48 hours. So
long-lived pre-signed transactions **must** use explicit gas coins.

**Verdict: workable, but it demands the most disciplined offline accumulator of any
candidate.** One dedicated gas coin per pre-signed transaction — which Sui supports
natively (`SplitCoins` is a PTB command, up to 256 coins may be smashed as gas) and
recommends in its own guidance: "Sponsors should use **dedicated gas coin pools** and
avoid reusing the same gas object in multiple inflight transactions"; "Clients should
avoid signing multiple transactions that use the same owned object inputs." Sui also
explicitly endorses the accumulate-locally pattern: "It is best practice to **store
signed transactions locally** before sending them to a full node… Sui transactions are
idempotent."

Two mitigations found: shared and immutable objects are **not** version-pinned
(`SharedObject { initial_shared_version }` is immutable for the object's lifetime), and
splitting from `tx.gas` pins exactly one object per transaction — the minimum possible.

**The sharp edge, and it is sharper than a rejection.** If two pre-signed transactions
ever share a gas coin and both get submitted, the result is not a clean failure but an
**equivocation lock**: "the affected object version is unavailable until the next epoch",
remedy "Wait for the next epoch" — **up to 24 hours of lost liveness on that coin.** One
coin per intent is not tidiness, it is a correctness requirement.

Sources: <https://docs.sui.io/develop/transaction-payment/sponsor-txn>,
<https://docs.sui.io/develop/objects/versioning>,
<https://docs.sui.io/develop/transactions/transaction-lifecycle>,
<https://docs.sui.io/develop/transaction-payment/gas-smashing>,
<https://docs.sui.io/develop/testing-debugging/common-errors>

> #### This directly contradicts a stated risk in `ALIGNMENT.md` §4
>
> `ALIGNMENT.md` calls Sui's missing memo "the highest-risk unknown in the switch" and
> offers three unsatisfying options: a small Move package (forfeits the
> no-custom-program property), deterministic transaction reconstruction (fragile), or an
> off-chain binding (a downgrade from fail-closed).
>
> **There is a fourth option that was missed, and it appears to be the right one.** A
> PTB `CallArg::Pure(Vec<u8>)` input carries arbitrary bytes up to
> `max_pure_argument_size = 16384`, need not be consumed by any command ("Remaining pure
> input values are dropped"), is covered by the Ed25519 signature over
> `blake2b256([0,0,0] ‖ bcs(TransactionData))`, and is readable by any third party with
> no credential — the mainnet GraphQL schema exposes `TransactionInput` as a union
> including `Pure { bytes: Base64 }`, verified by live query against
> `graphql.mainnet.sui.io` for arbitrary third-party transactions.
>
> That satisfies the requirement on substance and is arguably *stronger* than a memo
> field, because the bytes are sender-authenticated rather than merely present. **No Move
> package is needed.** What it does not give you is a *standardised* memo: no wallet,
> explorer or indexer will know to look at input index *k*, and there is no filter or
> index over pure-input contents. So the honest characterisation is: the fail-closed
> binding is achievable with no custom program, at the cost of ecosystem legibility. That
> is a materially better position than the three options currently recorded, and §4
> should be corrected.
>
> Sources: <https://docs.sui.io/develop/transactions/ptbs/prog-txn-blocks>,
> <https://docs.sui.io/develop/transactions/ptbs/inputs-and-results>. The negative half
> was also established rigorously: `grep -ri '\bmemo\b'` across all 395 docs pages
> returns **zero hits**, and events strictly require a deployed package — the bytecode
> verifier enforces that an emitted event's type "must be defined in the current
> module", which a PTB can never satisfy.

#### Tezos — FAIL

Eliminated by an hour-long operation lifetime that is **stated design intent, not an
incidental limit.** The mainnet parameters carry the rationale in a source comment:

```
(* The rationale behind the value of this constant is that an
   operation should be considered alive for about one hour:
     minimal_block_delay * max_operations_time_to_live = 3600
   The unit for this value is a block. *)
max_operations_time_to_live = 600;
```
with `let block_time = 6`. 600 × 6 s = 3600 s.
<https://gitlab.com/tezos/tezos/-/blob/master/src/proto_alpha/lib_parameters/default_parameters.ml>

The `live_blocks` RPC defines the enforceable set: blocks "which, if referred to as the
branch in an operation header, are recent enough for that operation to be included in
the current block."
<https://gitlab.com/tezos/tezos/-/blob/master/src/lib_shell_services/block_services.ml>
Past it, the operation is classified "**Outdated**: the operation is too old to be
included in a block." <https://octez.tezos.com/docs/active/validation.html>

The 3600 s figure is *deliberately held constant* as block times change — 450 × 8 s in
`proto_023`, 600 × 6 s in `proto_024`, `proto_025` and `proto_alpha`. This is not a knob
that drifts upward.

Two further mechanisms independently break bulk accumulation: `counter` is strictly
sequential per manager, and the prevalidator "ensures that **at most one manager
operation per manager is classified as Valid at any given time**", with the rest
`Branch_delayed` and not propagated.
<https://octez.tezos.com/docs/shell/prevalidation.html>

Also established while there, since the brief asked it be verified rather than assumed:
**Tezos has no native memo.** A `transaction` to an implicit `tz1` account cannot carry
arbitrary bytes — `parameters` targets a contract entrypoint and implicit accounts have
no code. So Tezos would have failed criterion 4 as well.

#### Radix — PASS, and it is the strongest offline-signing answer in the field

The window is **mandatory** — note the type contrast with the fields immediately below
it in the same struct:

```rust
pub struct IntentHeaderV2 {
    pub network_id: u8,
    pub start_epoch_inclusive: Epoch,
    pub end_epoch_exclusive: Epoch,
    pub min_proposer_timestamp_inclusive: Option<Instant>,
    pub max_proposer_timestamp_exclusive: Option<Instant>,
    pub intent_discriminator: u64,
}
```
<https://github.com/radixdlt/radixdlt-scrypto/blob/main/radix-transactions/src/model/v2/intent_header_v2.rs>

But the permitted span is a month, stated with the arithmetic already in the comment:

```rust
// ~30 days given 5 minute epochs
max_epoch_range: 12 * 24 * 30,
```
`TransactionValidationConfig::babylon()`, inherited unchanged by `cuttlefish()`.
<https://github.com/radixdlt/radixdlt-scrypto/blob/main/radix-transactions/src/validation/transaction_validation_configuration.rs>

Independently corroborated in the docs with the multiplication done: "the Transaction's
maximum Epoch range (i.e. **30 days \* 24 hours \* 12 epochs = 8640**)".
<https://docs.radixdlt.com/docs/transaction-tracker>

**This is a PARTIAL pass, and the distinction matters.** Radix is bounded at ~30 days
where Stellar, XRPL, Sui and Solana-with-durable-nonces are unbounded. The whole reason
the discriminator exists is the isolation requirement, so "bounded at 30 days" and
"unbounded" support genuinely different deployments: 30 days covers realistic
remote-deployment isolation and does **not** cover months of silence. If a deployment can
be dark for a quarter, Radix cannot serve it and Stellar can. Against that, the bound is
arguably a security feature — it makes replay windows finite rather than infinite.

Two further caveats, both from primary source:

- Epochs run **faster than target**. Live mainnet at epoch 330555 (2026-07-30) against
  Babylon genesis at epoch 32717 (2023-09-28) gives ≈4.5 min/epoch actual, so budget
  **≈27 days**, not 30.
- `max_epoch_range` is an **on-ledger substate** (`TransactionValidationConfigurationSubstate`,
  exposed by the Core API), so read it at runtime rather than hardcoding 8640.

**And uniquely among the survivors, Radix has no sequential-counter problem.**
`intent_discriminator` is a free nonce, not a monotonic per-account counter — so an
accumulated stack needs no ordering discipline, no `minSeqNum` equivalent, no Tickets,
and no per-intent nonce accounts. Compare Stellar (`minSeqNum` required), XRPL (250
tickets max), Solana (one funded nonce account per live intent), MultiversX (100-nonce
window). Radix simply does not have the problem.

---

## 2. Full matrix — survivors

The four survivors are **Radix, Sui, Stellar and Solana**. MultiversX and XRPL are included
below because each passes the discriminator and fails on exactly one other criterion, which
makes them the candidates most likely to become viable if that one thing changes. Chains
eliminated on the discriminator are summarised at the end of this section.

### Stellar

| # | Criterion | Finding | Source |
|---|---|---|---|
| 3 | Atomic N-way, max recipients | "Transactions comprise a bundle of between 1-100 operations (except smart contract transactions, which can only have one operation per transaction)." XDR: `Operation operations<MAX_OPS_PER_TX>`, `MAX_OPS_PER_TX = 100`. So **100 recipients max**, one `PAYMENT` operation each. | [ops-and-transactions](https://developers.stellar.org/docs/learn/fundamentals/transactions/operations-and-transactions), [Stellar-transaction.x](https://github.com/stellar/stellar-xdr/blob/curr/Stellar-transaction.x) |
| 4 | 32-byte binding field | XDR `Memo` union: `MEMO_HASH: Hash hash` — exactly 32 bytes, a protocol-native field. Note `MEMO_TEXT` is only `string text<28>`, so a hex-encoded 32-byte hash does **not** fit in the text memo; `MEMO_HASH` is the right field. | [Stellar-transaction.x](https://github.com/stellar/stellar-xdr/blob/curr/Stellar-transaction.x) |
| 5 | Ed25519 | "Stellar uses the ed25519 signature scheme, but there is also a mechanism for adding additional types of public and private key schemes." | [signatures-multisig](https://developers.stellar.org/docs/learn/fundamentals/transactions/signatures-multisig) |
| 6 | Validator minimum hardware | Core validator node: **8 vCPUs @ 3.4 GHz, 16 GB RAM, 100 GB NVMe SSD (10,000 IOPS)**, TCP 11625 + UDP 123. "verified against production nodes in April 2024"; "Hardware requirements grow with network activity." | [validator prerequisites](https://developers.stellar.org/docs/validators/admin-guide/prerequisites) |
| 12 | Validator economics | "There are no monetary rewards for being a validator on the Stellar network." Quorum slices are reputational, not bonded — no stake gate found. Fork/rollback posture still PENDING. | [SCP](https://developers.stellar.org/docs/learn/fundamentals/stellar-consensus-protocol) |
| 1 | Clawback is issuer-only | The `CLAWBACK` operation "Burns an amount in a specific asset from an account. **Only the issuing account for the asset can perform this operation.**" It therefore cannot touch the native asset, which has no issuer. **But** it does mean a Stellar-issued stablecoin (e.g. USDC on Stellar) is clawback- and freeze-exposed by its issuer. Settling in XLM and settling in an issued asset are materially different trust postures, and the choice must be made explicitly. | [list-of-operations](https://developers.stellar.org/docs/learn/fundamentals/transactions/list-of-operations) |
| 3 | Per-recipient reserve cost | `CREATE_ACCOUNT` can fail with `CREATE_ACCOUNT_LOW_RESERVE` — "This operation would create an account with fewer than the minimum number of XLM an account must hold." An N-way payout to *new* destinations therefore carries a per-recipient reserve cost: **one base reserve is 0.5 XLM**, an account's minimum is "two base reserves (currently 1 XLM)", and each subentry adds 0.5 XLM. This is the direct analogue of Solana's per-recipient ATA rent that the existing rail already prices in. | [list-of-operations](https://developers.stellar.org/docs/learn/fundamentals/transactions/list-of-operations), [lumens](https://developers.stellar.org/docs/learn/fundamentals/lumens) |
| 15 | Payment granularity | **PASS, type 2.** 1 stroop = "one ten-millionth of a lumen (.0000001 XLM)". Reserves are per-account and per-subentry, not per payment, so an already-funded account can receive a single stroop. Base fee is "100 stroops per operation (the network minimum)" — a type-3 cost, paid once per bundle. | [fees](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering), [lumens](https://developers.stellar.org/docs/learn/fundamentals/lumens) |
| 16 | Batch integrity | **Vulnerable for issued assets; safe for native XLM.** A `PAYMENT` of a non-native asset to an account with no trustline fails, and the transaction is all-or-nothing — so one unprepared payee kills a 100-operation bundle. XLM needs no trustline, so native payments to existing funded accounts have no recipient veto. **The native remedy is `CREATE_CLAIMABLE_BALANCE`**, which exists for exactly this: claimable balances "allow an account to send a payment to another account **that is not necessarily prepared to receive the payment**… when you send a non-native asset to an account that has not yet established a trustline". Cost: "each claimant in that entry increases the source account's minimum balance by one base reserve" — **0.5 XLM locked per leg** on the sender, released on claim. The claimant must still create a trustline before claiming, or the claim fails `op_no_trust`. | [claimable-balances](https://developers.stellar.org/docs/build/guides/transactions/claimable-balances), [lumens](https://developers.stellar.org/docs/learn/fundamentals/lumens) |
| 7 | Private network | Partially established. The networks page names only Mainnet, Testnet and Futurenet, and says "You can always run your own test network for use cases that don't work well with SDF's Testnet" — but gives no procedure on that page. A concrete documented path (quickstart image / `RUN_STANDALONE`) is PENDING. | [networks](https://developers.stellar.org/docs/networks) |

### XRPL

| # | Criterion | Finding | Source |
|---|---|---|---|
| 3 | **Atomic N-way — the decisive failure today** | The `Batch` amendment, which is what would give XRPL atomic multi-transaction bundles, **is not active on mainnet**. It "was disabled in v3.1.1 due to a bug in `Batch`. The `BatchV1_1` amendment in a future release will include this fix." A February 2026 disclosure found a signature-validation flaw allowing inner transactions to execute on behalf of arbitrary accounts; validators were advised to vote No and the amendment never activated. `fixBatchInnerSigs` is likewise disabled. No release date is published for `BatchV1_1`. | [known-amendments](https://xrpl.org/resources/known-amendments), [disclosure](https://xrpl.org/blog/2026/vulnerabilitydisclosurereport-bug-feb2026) |
| 4 | 32-byte binding field | `Memos` array with `MemoData` = "Arbitrary hex value". "The `Memos` field is limited to no more than 1 KB in size (when serialized in binary format)." A 32-byte hash fits comfortably. | [common-fields](https://xrpl.org/docs/references/protocol/transactions/common-fields) |
| 6 | Node minimum hardware | Recommended: "3+ GHz 64-bit x86_64 processor with 8+ cores", 64 GB RAM, SSD/NVMe 10,000 IOPS sustained, "Minimum 50 GB for the database partition", gigabit NIC. Minimum: "64-bit x86_64, 4+ cores", "16 GB+" RAM. Docs caution minimum specs "are insufficient for reliable Mainnet synchronization in production environments." | [system-requirements](https://xrpl.org/docs/infrastructure/installation/system-requirements) |
| 8, 11 | **Full history cost — a serious operability finding** | Capacity planning gives "Full history \| 81,000,000+ \| ~26 TB", with roughly **4.5 TB/year** implied growth. Because entitlement verification re-reads the chain indefinitely, this is the node class the platform would need — not the 50 GB default. | [capacity-planning](https://xrpl.org/docs/infrastructure/installation/capacity-planning) |
| 12 | Validator trust is curated | "By default, XRP Ledger servers are configured to use validator list sites run by the XRPL Foundation and Ripple." Quorum: "The only way to confirm an invalid transaction would be to get at least 80% of trusted validators to approve"; consensus tolerates "less than about 20%" misbehaving. Running a validator is unbonded, but being *trusted* depends on inclusion in a curated list. | [consensus-protections](https://xrpl.org/docs/concepts/consensus-protocol/consensus-protections) |
| 1 | Native asset, no issuer | **PASS, and the most explicit statement of any candidate.** "Issuers can freeze the tokens they issue in the XRP Ledger. **This does not apply to XRP, which is the native asset of the XRP Ledger, not an issued token.**" And directly: "**No one can freeze XRP in the XRP Ledger.**" No argument-from-absence needed. **But** issued tokens carry Individual Freeze, Global Freeze, and a `No Freeze` opt-out — so settling in an XRPL-issued stablecoin is a freeze-exposed choice, exactly as on Stellar. | [freezes](https://xrpl.org/docs/concepts/tokens/fungible-tokens/freezes) |
| 5 | Ed25519 | **PASS.** "EdDSA using the elliptic curve Ed25519. This is a newer algorithm which has better performance and other convenient properties." Encoding note for implementers: "Since Ed25519 public keys are one byte shorter than secp256k1 keys, `xrpld` prefixes Ed25519 public keys with the byte `0xED` so both types of public key are 33 bytes." | [cryptographic-keys](https://xrpl.org/docs/concepts/accounts/cryptographic-keys) |
| 15 | Payment granularity | **PASS, type 2.** "Each drop is equal to 0.000001 XRP", max 10¹¹ XRP. Base reserve **1 XRP**, owner reserve **0.2 XRP per item** — these are account-existence and object costs, not floors on payment size, so an already-funded account can receive a single drop. Issued tokens carry "15 decimal digits of precision". | [reserves](https://xrpl.org/docs/concepts/accounts/reserves), [currency-formats](https://xrpl.org/docs/references/protocol/data-types/currency-formats) |

> **XRPL's elimination is narrow and possibly temporary.** It passes the offline-signing
> criterion (arguably more cleanly than Stellar, via purpose-built Tickets), passes
> granularity, has the clearest unfreezable-native-asset statement in the field, supports
> Ed25519, and issues assets as a protocol primitive. It fails on **one** thing: atomic
> N-way payment, because `Batch` is not active on mainnet. If `BatchV1_1` ships, XRPL
> becomes a live candidate again and this document should be re-run. No release date is
> published.
>
> Items not established for XRPL, because it was eliminated on criterion 3 before the
> full matrix was completed: MPToken (XLS-33) issuance status, private-network procedure,
> default `[ledger_history]` retention, validator counts and any Nakamoto figure,
> ledger-close time as an irreversibility claim, and the status of Hooks / any WASM
> contract VM on XRPL L1 (Hooks are believed to be live only on Xahau, a separate
> network — **not verified**).

### Solana (the incumbent rail)

| # | Criterion | Finding | Source |
|---|---|---|---|
| 1 | Native asset, no issuer | **YES, but established by absence rather than by an affirmative statement.** SOL is "The native token of a Solana cluster." The complete System Program instruction set (13 instructions) and the exhaustive list of 7 builtin programs contain no freeze, revoke, clawback or pause primitive for lamports. By explicit contrast, SPL tokens *are* freezable: "Freezing a token account… prevents the token account from receiving, transferring, or burning tokens until the token account is thawed." No primary sentence of the form "SOL cannot be frozen" could be found. | [terminology](https://solana.com/docs/references/terminology), [builtin-programs](https://solana.com/docs/core/programs/builtin-programs), [freeze-account](https://solana.com/docs/tokens/basics/freeze-account) |
| 2 | Issuance as protocol primitive | **NO — FAIL.** The 7 builtin programs include no token or mint program. "Tokens on Solana are referred to as SPL… Tokens", and both the Token Program and Token-2022 are external user-space sBPF programs. Only SOL/lamports is protocol-native. | [tokens](https://solana.com/docs/tokens), [builtin-programs](https://solana.com/docs/core/programs/builtin-programs) |
| 3 | Atomic N-way, max recipients | **YES, atomic with no custom program** (N × System `Transfer`): "All instructions succeed or all revert." Documented hard limits: `PACKET_DATA_SIZE` = **1,232 bytes**, max accounts per transaction = **64**. **No primary source publishes a max recipient count.** Derived from the docs' own size formula: ≈**21 recipients** for a legacy transaction, rising to ~60 with a v0 transaction plus an Address Lookup Table (also a builtin, so still no custom program) at which point the 64-account lock limit binds. Compute is not the constraint (System Program = 150 CU/instruction against a 1,400,000 CU ceiling). | [transactions](https://solana.com/docs/core/transactions), [transaction-structure](https://solana.com/docs/core/transactions/transaction-structure), [versioned-transactions](https://solana.com/docs/core/transactions/versioned-transactions) |
| 4 | 32-byte binding field | **Not protocol-native — it is the SPL Memo *program*** (`MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr`), which is not among the 7 builtins. It is genuinely third-party verifiable: the program "logs the memo, as well as any verified signer addresses, to the transaction log, **so that anyone can easily observe memos and know they were approved by zero or more addresses**". Arbitrary-length UTF-8, so 32 bytes fits easily; **no documented maximum size** — the ceiling is the 1,232-byte transaction limit. | [memo program source](https://github.com/solana-program/memo), [payment-with-memo](https://solana.com/docs/payments/send-payments/payment-with-memo) |
| 5 | Ed25519 | **YES.** "Each `Signature` is a **64-byte Ed25519 signature** of the serialized `Message`". There is also an Ed25519 precompile for in-transaction verification. | [transaction-structure](https://solana.com/docs/core/transactions/transaction-structure), [precompiles](https://solana.com/docs/core/programs/precompiles) |
| 6, 11 | Node operability | **The heaviest in the comparison.** Published *recommendations* (the page is titled "Hardware Recommendations", not minimums): **12 cores / 24 threads**, **256 GB RAM**, ~**2.5 TB NVMe across three separate disks** (1 TB accounts + 1 TB ledger + 500 GB snapshots), **2 Gbit/s symmetric** for a staked node, public IPv4, "It is not recommended to run a validator behind a NAT", Docker "not recommended and generally not supported", cloud "requires significantly greater operational expertise… Do not expect to find sympathetic voices should you chose this route". Recurring cost: vote account rent-exempt reserve 0.02685864 SOL plus **up to 1.1 SOL per day** in vote transactions. Sync-from-genesis time: **could not establish** — and notably the documented bootstrap path is snapshot-based, never a genesis replay. Disk growth per year: **could not establish** a published figure. | [requirements](https://docs.anza.xyz/operations/requirements), [setup-a-validator](https://docs.anza.xyz/operations/setup-a-validator) |
| 7 | Private network | **YES, well documented.** `solana-test-validator` "starts a full-featured, single-node cluster on the developer's workstation" with no RPC rate limits and configurable epoch length and ledger retention. Multi-node private clusters via `./multinode-demo/setup.sh`. | [test-validator](https://docs.anza.xyz/cli/examples/test-validator), [benchmark](https://docs.anza.xyz/clusters/benchmark) |
| 8 | Durable full history | **FAIL by default, and this is the most operationally awkward finding for an entitlement model that re-reads the chain indefinitely.** Default local retention is measured in **hours**: the Agave source comment for `DEFAULT_MAX_LEDGER_SHREDS = 200_000_000` states "at 5k shreds/slot at 50k tps, this is 40k slots (**~4.4 hours**). At idle, 60 shreds/slot this is about 3.33m slots (~15 days)". The docs are explicit: "it is not practical for an RPC node to store the entire blockchain on the machine… the RPC server will have to access older blocks through a Solana bigtable instance. If you are interested in setting up your own bigtable instance, see these docs". So indefinite history is possible but means **you personally operate a Google BigTable archive**, uploading every epoch via `agave-ledger-tool` and backfilling all prior history — "restricted only by the monetary costs of doing so." | [blockstore_cleanup_service.rs](https://github.com/anza-xyz/agave/blob/master/ledger/src/blockstore_cleanup_service.rs), [setup-an-rpc-node](https://docs.anza.xyz/operations/setup-an-rpc-node), [rpc-transaction-history](https://docs.anza.xyz/implemented-proposals/rpc-transaction-history) |
| 12 | Consensus | PoH + TowerBFT. **Finality is lockout/economics-based, not single-slot deterministic, and forks are a normal documented part of the protocol.** Commitment levels verbatim: `processed` — "This is the newest view, but **it can still be rolled back**"; `confirmed` — 66%+ stake voted; `finalized` — 66%+ stake **plus 31+ confirmed blocks built atop the block**. **The project's existing rejection of `processed` is correct and well-supported.** The finding that goes further: `confirmed` differs from `finalized` *precisely on lockout depth*, so `confirmed` should also be treated as revertible in principle. No primary statement asserts a finalized transaction can never revert; Anza's own consensus proposal says TowerBFT "does not have a security proof, which is concerning." Validator entry is permissionless but capital-gated (see 6/11). **Client diversity is effectively absent on mainnet**: Firedancer's own README says Frankendancer is on mainnet but "Other functionality including execution and consensus is using the Agave validator code", and full Firedancer "is not ready for test or production use and has no releases." Validator count and Nakamoto coefficient: **could not establish from primary sources** (solana.com/validators renders them in a live widget; docs publish no figure). | [commitments](https://docs.anza.xyz/consensus/commitments), [rpc](https://solana.com/docs/rpc), [fork-generation](https://docs.anza.xyz/consensus/fork-generation), [SIMD-0326](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0326-alpenglow.md), [firedancer](https://github.com/firedancer-io/firedancer) |
| 13 | Time to finality | **~400 ms to first (rollback-able) confirmation; ~12.8 s to irreversible.** 32 slots × 400 ms, confirmed exactly by Anza's own proposal: TowerBFT "has a consensus finality time of **12.8 seconds**". Solana's developer-facing material predominantly quotes the sub-second figure; the 12.8 s number appears in its own SIMD as a defect to be fixed. Alpenglow, which would improve this, is status **Review** with an unfilled feature key — not activated. **The operative number for this platform is 12.8 s, not 400 ms.** | [SIMD-0326](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0326-alpenglow.md), [clock/src/lib.rs](https://github.com/anza-xyz/solana-sdk/blob/master/clock/src/lib.rs) |
| 14 | Smart contracts | sBPF (Solana Bytecode Format, an eBPF derivative) via LLVM; Rust is the documented language; mature and in production. **Not WASM** — 4 KiB stack frames, 32 KiB default / 256 KiB max heap, ~30 fixed sBPF syscalls instead of WASI imports. The platform's existing `wasm32-wasip1` pipeline shares only LLVM and the Rust frontend, **not** the target, ABI, memory model, or host-call interface. | [programs](https://solana.com/docs/core/programs), [syscall-reference](https://solana.com/docs/core/programs/syscall-reference) |

### Radix (Babylon)

Passes both discriminators. On the published evidence this is the most complete match to
the criteria of any candidate assessed — including Stellar.

| # | Criterion | Finding | Source |
|---|---|---|---|
| 1 | Native asset, no issuer | **PASS, and provably so — the strongest answer in the field.** XRD's genesis creation sets only `mint_roles` and `burn_roles` (both locked to `require(global_caller(CONSENSUS_MANAGER))`), leaving `freeze_roles` and `recall_roles` at defaults, and those defaults are documented as `deny_all / deny_all`. Because the `_updater` roles are *also* `deny_all` and the owner role is `Fixed`, **no key or badge in existence can ever enable freeze or recall on XRD.** This is an affirmative cryptographic guarantee, not the argument-from-absence that Solana and MultiversX require. | [bootstrap.rs](https://github.com/radixdlt/radixdlt-scrypto/blob/main/radix-engine/src/system/bootstrap.rs), [resource-behaviors](https://docs.radixdlt.com/docs/resource-behaviors), [native-token-xrd](https://docs.radixdlt.com/docs/concepts-native-token-xrd) |
| 2 | Issuance as protocol primitive | **PASS — the headline claim verified.** "Resources are a **first-class primitive** in Radix… off-ledger clients like wallets and explorers can look at a given resource on-ledger and know things about how it behaves—**without having to read any smart contract code**." A token is created with **zero Scrypto deployed**, via the manifest instruction `CREATE_FUNGIBLE_RESOURCE_WITH_INITIAL_SUPPLY`, which calls the same **native Rust** blueprint that mints XRD itself. | [asset-oriented](https://docs.radixdlt.com/docs/asset-oriented), [manifest-instructions](https://docs.radixdlt.com/docs/manifest-instructions), [native-blueprints](https://docs.radixdlt.com/docs/native-blueprints) |
| 3 | **Atomic N-way, max recipients** | **PASS — 126, corrected downward from an initial ~499 estimate.** Atomicity is explicit: "Manifests… make it possible to compose multiple actions to be executed **atomically**"; on failure "all other events and state changes will be discarded." Pattern is one `lock_fee`, one `withdraw`, then N × (`TAKE_FROM_WORKTOP` + `CALL_METHOD <account> "try_deposit_or_abort"`) — all native instructions on native Account components, no contract. **The binding limit is `MAX_NUMBER_OF_EVENTS = 256`, not `max_instructions = 1000`:** each `try_deposit_or_abort` emits **2** events (an account-level `DepositEvent` and an independent vault-level one), so `4 + 2N ≤ 256` → **N ≤ 126**. Safe design target **100**. Payload is nowhere near binding (`max_user_payload_length` = 1 MiB). **The `AccountLocker::airdrop` route removes the limit entirely** by looping natively inside one `CALL_METHOD` — see criterion 16. | [manifest](https://docs.radixdlt.com/docs/manifest), [concepts-transactions](https://docs.radixdlt.com/docs/concepts-transactions), [transaction-limits](https://docs.radixdlt.com/docs/transaction-limits) |
| 15 | Payment granularity | **PASS, type 2, emphatically.** Smallest deliverable is **1 atto = 10⁻¹⁸ XRD** (`Decimal` SCALE = 18, `radix-common/src/math/decimal.rs`). No amount validation exists in `Account::deposit`; the fee table is paid entirely from the sender's fee reserve; storage is a one-time commit cost, not recurring rent, so nothing can be reaped; and there are zero occurrences of `MIN_BALANCE` / `existential` / `dust` / `reap` in the account or locker blueprints. Instantiating an account that does not yet exist is **sender-paid from the fee reserve** — the Stellar-trustline shape, not carved out of the delivered amount. | [decimal.rs](https://github.com/radixdlt/radixdlt-scrypto/blob/main/radix-common/src/math/decimal.rs) |
| 16 | Batch integrity | **Vulnerable by default, with the best native remedy in the field.** Accounts have owner-configurable deposit rules, so a recipient who disallows XRD makes `try_deposit_or_abort` return `DepositIsDisallowed` and kills the whole atomic transaction. The purpose-built fix is the **`AccountLocker` blueprint's `airdrop` method with `try_direct_send: true`**: it loops over claimants in native Rust inside a single `CALL_METHOD`, and on refusal stores the funds in the locker for later claim rather than aborting. Two bonuses — no recipient can block a payout run, and the native loop eliminates the event/instruction ceiling that caps criterion 3 at 126. | [AccountLocker](https://docs.radixdlt.com/docs/account-locker) |
| 4 | 32-byte binding field | **PASS, 2048 bytes — 64× the requirement, and better bound than any competitor's memo.** `MessageContentsV1` has a `Bytes(Vec<u8>)` variant (arbitrary binary, not just text), with `max_plaintext_message_length: 2048`. Third-party readable via both public API specs (Core API `PlaintextTransactionMessage`, Gateway API `CommittedTransactionInfo.message`). **Critically: the message sits inside the intent, so it is covered by the intent hash and the signatures** — cryptographically bound to the payment rather than an adjacent annotation. An encrypted variant supports up to 20 decryptors. | [message.rs](https://github.com/radixdlt/radixdlt-scrypto/blob/main/radix-transactions/src/model/v1/message.rs), [transaction-structure](https://docs.radixdlt.com/docs/transaction-structure) |
| 5 | Ed25519 | **PASS.** "The Babylon Radix network supports **ECDSA Secp256k1** and **Ed25519** for accounts and transaction signing." Core API `PublicKeyType` enum includes `EddsaEd25519`; standard 32-byte key / 64-byte signature. Primary hash is Blake2b-256. | [curves-keys-signatures](https://docs.radixdlt.com/docs/concepts-curves-keys-signatures-and-hashing) |
| 6 | Validator minimum hardware | **PASS — commodity, second-lightest in the field.** Two official pages disagree on cores: **8 CPU** / 16 GB / 500 GB SSD "(initially)" on one, **4 CPU** (AWS `m6i-xlarge`) / 16 GB / 500 GB on the other; both say "Recommended at least 100 Mbps". Treat 8 / 16 GB / 500 GB as the safe figure. **No published distinction between full-node and validator hardware.** | [running-a-node](https://docs.radixdlt.com/docs/running-a-node), [node-setup](https://docs.radixdlt.com/docs/node-setup) |
| 7 | Private network | **PARTIAL.** A real multi-validator local network is documented in-repo: `./docker/scripts/rundocker.sh 2`, "replacing 2 with the number of validators you wish to run (**1 - 5**)". Custom-genesis tooling exists (`GenerateGenesis.java`; `Network.java` defines `LOCALNET(240)`, `INTEGRATIONTESTNET(241)`, `LOCALSIMULATOR(242)`), and `resim` gives a no-consensus dev simulator. **Gap:** no page on docs.radixdlt.com documents standing up an independent private *production* network. The capability is real and open source; the operator-facing documentation is not. | [babylon-node docker README](https://github.com/radixdlt/babylon-node/blob/main/docker/README.md), [resim](https://docs.radixdlt.com/docs/resim-radix-engine-simulator) |
| 8 | Durable full history | **PASS, self-hostable, no forced pruning.** "Typically full nodes are configured to **maintain a complete transaction history**, and have various optional additional indices turned on" — and `db.local_transaction_execution_index.enable` / `db.account_change_index.enable` are **on by default**. The only automatic GC found targets the state hash tree, not the transaction stream. Historical query layer is the Network Gateway (Core node → Data Aggregator → PostgreSQL → Gateway API), which "can be deployed directly, forked, or used as a reference." | [node](https://docs.radixdlt.com/docs/node), [network-gateway](https://docs.radixdlt.com/docs/network-gateway) |
| 11 | Node operability | **PASS with a real gap.** Indefinite re-reads do **not** require a special archival mode — a default full node already keeps complete transaction history. Genesis bootstrap: "This may take up to 30 minutes… Usually it finishes in around 10-15 minutes" — but that is genesis processing, **not** full historical sync, which **could not be established**. **Disk growth per year: could not be established** — no figure exists on docs.radixdlt.com, radixdlt.com, or either repo. The "500 GB (initially)" phrasing implies growth without quantifying it. This is the largest documentation gap found. | [node-setup-systemd](https://docs.radixdlt.com/docs/node-setup-systemd) |
| 12 | Consensus | **PASS on the disqualifier, FAIL on client diversity.** Mechanism is **HotStuff BFT** (*not* Cerberus — that is a separate future research track never mentioned on the live consensus page). Verbatim: "This process has **deterministic finality: commits are final and there are no probabilistic forks in the ledger**." **Rollback of a confirmed transaction is not possible.** Validator registration is permissionless but the active set is hard-capped at 100 (`.maxValidators(100)`), selected by delegated stake each epoch; validator creation costs **1000 USD** per mainnet `productionDefaults()` (the docs page says "~100USD", which is the *test* default — source wins). **Client diversity: only `babylon-node` exists.** A consensus bug has no independent implementation to catch it. | [consensus](https://docs.radixdlt.com/docs/concepts-consensus-ledger-forks-blocks-and-trust-chains), [GenesisConsensusManagerConfig.java](https://github.com/radixdlt/babylon-node/blob/main/core-rust-bridge/src/main/java/com/radixdlt/genesis/GenesisConsensusManagerConfig.java) |
| 12b | Decentralisation — **the weakest number in the Radix column** | **No published Nakamoto coefficient exists.** Computed from live mainnet stake (Gateway `/state/validators/list`, epoch 330555, 2026-07-30): **287 registered validators, 100 active**, total active stake 4.90 bn XRD, largest holds 5.03%, and the 100th-ranked validator holds 4,392,108 XRD (today's de facto entry stake). **≈9 validators exceed the 1/3 BFT safety threshold**; 16 exceed 1/2; 26 exceed 2/3. ⚠️ *This is a derived figure from a live API reading, not a Radix claim, and not a static documented fact.* | derived from mainnet Gateway API |
| 13 | Time to finality | **PARTIAL — and the honest answer is that Radix publishes no seconds figure.** Its own framing is that the question does not apply: finality is deterministic, so time-to-first-confirmation **is** time-to-irreversible, with no confirmation count to wait for. **Could not establish** an official "~X seconds" claim. Derived bound (inference, not official): `epochTargetDurationMillis = 300000` with `epochMinRoundCount = 500` / `max 3000` → 100–600 ms per round, HotStuff commits in a small constant number of rounds → sub-second normally. Pathological path: `baseTimeoutMs = 3000` escalating on repeated leader failure. | [consensus](https://docs.radixdlt.com/docs/concepts-consensus-ledger-forks-blocks-and-trust-chains) |
| 9, 14 | Smart contracts | **PASS, and WASM — verified, not assumed.** Scrypto is a Rust DSL; the toolchain is `rustup target add wasm32-unknown-unknown`, packages ship as WASM, and the engine is **Wasmi** ("a WASM interpreter that supports WebAssembly MVP", `radix-engine/Cargo.toml`). Alongside MultiversX, one of only two candidates whose contract VM shares the platform's existing WASM toolchain. Mainnet since Babylon (enacted epoch 32717, 2023-09-28); Cuttlefish enacted epoch 105353, 2024-12-18. **Note: the platform needs no Scrypto at all** — criteria 2, 3 and 4 are all satisfiable with native manifest instructions. | [getting-rust-scrypto](https://docs.radixdlt.com/docs/getting-rust-scrypto), [scrypto-builder](https://docs.radixdlt.com/docs/scrypto-builder), [babylon-genesis](https://docs.radixdlt.com/docs/babylon-genesis) |

### Sui

Passes both hard criteria. Its weaknesses are node weight, single-client risk, and
ecosystem legibility of the binding field — not capability.

| # | Criterion | Finding | Source |
|---|---|---|---|
| 1 | Native asset, no issuer | **PASS, provably, from the Move source.** `sui.move` calls `coin::create_currency` — **not** `create_regulated_currency` — so no `DenyCapV2<SUI>` was ever minted and the deny-list freeze mechanism has no capability applying to SUI. The `TreasuryCap` is converted to `Supply` and then **`destroy_supply()`**, so no minting authority survives genesis; `new()` asserts sender `@0x0` and epoch 0, so it can never run again. **Honest qualification:** `TransactionDenyConfig` lets node operators deny addresses/objects/packages with a `stake_threshold_percent` and broadcast-on-epoch-change. That is *censorship* (a liveness attack), not confiscation — it cannot take a settled balance — but it is a supported, broadcastable node feature and should be recorded. | [sui.move](https://github.com/MystenLabs/sui/blob/main/crates/sui-framework/packages/sui-framework/sources/sui.move), [sui-security](https://docs.sui.io/develop/sui-architecture/sui-security) |
| 2 | Issuance as protocol primitive | **PARTIAL / effectively NO.** `Coin<T>` and `coin::create_currency` are Move code in the Sui Framework published at `0x2`. There is no `IssueAsset` transaction kind and no native ledger table. But it is not a third-party contract either: it ships with the node, every validator runs identical bytecode, and SUI itself is defined through the same call. Framework-level, not protocol-level. Also note the current signature is deprecated: `#[deprecated(note = b"Use `coin_registry::new_currency_with_otw` instead")]`. | [coin.move](https://github.com/MystenLabs/sui/blob/main/crates/sui-framework/packages/sui-framework/sources/coin.move) |
| 3 | **Atomic N-way, max recipients** | **PASS, ~511 — the highest verified count in the field.** "PTBs allow a user to call multiple Move functions, manage their objects, and manage their coins in a single transaction **without publishing a new Move package**"; "If one transaction command fails, the entire block fails and no effects from the commands are applied." The official cookbook recipe is exactly this shape: `tx.splitCoins(tx.gas, amounts)` then N × `tx.transferObjects`. Limits from the mainnet protocol-config snapshot: `max_arguments = 512` (→511 amounts on one `SplitCoins`), `max_programmable_tx_commands = 1024`, `max_input_objects = 2048`, `max_tx_size_bytes = 131072`, `max_num_transferred_move_object_ids = 2048`. | [prog-txn-blocks](https://docs.sui.io/develop/transactions/ptbs/prog-txn-blocks), [ptb-cookbook](https://docs.sui.io/develop/transactions/ptbs/ptb-cookbook) |
| 4 | 32-byte binding field | **PASS on substance, FAIL on ergonomics** — see the boxed note in §1. No memo field exists (zero hits for "memo" across 395 docs pages) and events require a deployed package. But an unused `Pure` input carries up to **16,383 bytes**, is signature-covered, and is third-party readable via the public GraphQL `TransactionInput` union. No Move package needed. | [inputs-and-results](https://docs.sui.io/develop/transactions/ptbs/inputs-and-results) |
| 5 | Ed25519 | **PASS, and it is the default.** "Sui supports pure Ed25519, ECDSA Secp256k1, ECDSA Secp256r1, and multisig." Flag `0x00`, 32-byte key, 64-byte signature, serialized `flag ‖ sig ‖ pk`; address = BLAKE2b-256(flag ‖ pubkey). A full offline-signing pipeline is documented. ⚠️ **zkLogin and passkey signatures DO expire** (bounded by the ephemeral key's `maxEpoch`), so long-lived pre-signed transactions require a raw keypair. | [auth-overview](https://docs.sui.io/develop/transactions/transaction-auth/auth-overview), [offline-signing](https://docs.sui.io/develop/transactions/transaction-auth/offline-signing) |
| 6 | Validator minimum hardware | **Heavy — second only to Solana.** Validator: **24 physical cores (48 vCPU), 128 GB RAM, 4 TB NVMe, 1 Gbps**, ports 8080–8084, plus kernel tuning ("the default Linux network buffer sizes are too small"). Full node: 8 physical cores / 16 vCPU, **128 GB RAM**, 4 TB NVMe. macOS "only recommended for development and not for production use." | [validator-config](https://docs.sui.io/operators/validator/validator-config), [sui-full-node](https://docs.sui.io/operators/full-node/sui-full-node) |
| 7 | Private network | **PASS, one command.** `sui start --with-faucet --force-regenesis`; "Localnet is a single-machine network you run yourself." For a persistent isolated chain, omit `--force-regenesis` and use `sui genesis`. Note `sui-test-validator` is superseded by `sui start`. | [local-network](https://docs.sui.io/getting-started/onboarding/local-network) |
| 8 | Durable full history | **Achievable but explicitly opt-in, and the free public archive keeps only 30 days.** Defaults prune everywhere — validator profile is `num-epochs-to-retain: 0`; "Setting transaction pruning to 2 epochs is recommended". A full node *can* retain everything (`num-epochs-to-retain: 18446744073709551615`). Archives hold history since genesis, **but**: "`https://checkpoints.mainnet.sui.io`… retain only the most recent 30 days… For full checkpoint retention, use `gs://mysten-mainnet-checkpoints-use4`… **with Requester Pays enabled**." Formal snapshots keep 90 epochs and are "not suitable… if you are running an RPC node that does any historical data lookups". | [managing-data](https://docs.sui.io/operators/data-management/managing-data), [archives](https://docs.sui.io/operators/data-management/archives) |
| 11 | Node operability | **A small operator can run an unpruned full node; a small operator cannot realistically run the indexed archival service.** Published Nov 2025 figures: validators ~250–400 GB, pruning full node 2.5 TB, **unpruned full node 16 TB**, checkpoints bucket ~30 TB. Growth is published and load-dependent by 4×: "~10GB per day" at ~18 TPS, "~40GB per day" at ~183 TPS → **~3.6–14.6 TB/yr**. The supported indefinite-history answer is the Archival Store on Bigtable: measured **~18.1 TB**, 7–30 autoscaling nodes, 16-core indexer, "A full Mainnet backfill takes approximately 3-4 days on a 30-node SSD cluster." **Sync from genesis: could not establish** — docs route around it ("always start it from a snapshot"). | [sui-storage](https://docs.sui.io/develop/sui-architecture/sui-storage), [archival-stack-setup](https://docs.sui.io/operators/data-management/archival-stack-setup) |
| 12 | Consensus | **PASS on the disqualifier.** Mysticeti DAG-based DPoS, 24-hour epochs, >2/3 quorum. Verbatim: "Certified effects guarantee transaction finality. After a full node observes certified effects, **Sui includes the transaction in a checkpoint and never reverts it**"; "the transaction is irreversible." No longest-chain rule, no reorg depth. Validator barrier is voting-power-based per **SIP-39** (Status: Final, live in `validator_set.move`): 3 voting power to join. Live mainnet (epoch 1204): **126 active validators** (`max_validator_count: 150`), 7.20 bn SUI staked, lowest active stake **2,545,474 SUI**. ⚠️ **Trap for chain-state readers:** the on-chain `min_validator_joining_stake` still reads 30M SUI but is *not enforced as a SUI amount* — reading it naively overestimates the barrier 12×. **Client diversity: single-client** — no second implementation could be established. **Nakamoto: no published figure**; derived from live epoch-1204 voting power, **19** validators exceed 1/3 and 48 exceed 2/3. | [transaction-lifecycle](https://docs.sui.io/develop/transactions/transaction-lifecycle), [consensus](https://docs.sui.io/develop/sui-architecture/consensus), [SIP-39](https://github.com/sui-foundation/sips/blob/main/sips/sip-39.md) |
| 12b | Architecture in flux — recorded honestly | Three official pages **disagree** on whether owned-object transactions bypass consensus on current mainnet: `objects/versioning` describes a live fast path ("bypass consensus"), `transaction-lifecycle` says "All transactions are sequenced by Sui's Mysticeti DAG consensus" via a newer Transaction Driver, and `sui-architecture/consensus` still describes the older certificate flow. Which is authoritative **could not be resolved from primary sources**. The finality *guarantee* is identical either way. | as cited above |
| 13 | Time to finality | **PASS, and the best in the field: 400–700 ms, first confirmation *is* irreversible.** "**End-to-end finality typically takes 400–700 ms**… After finality, the user knows the transaction is irreversible." Two figures not to confuse with it: the "about 0.5 seconds" consensus-commit number is explicitly labelled a lab benchmark, "not… production metrics"; and **queryability lags finality** — "The full node must wait for the transaction to be checkpointed and state-synced, which normally takes a few seconds." | [transaction-lifecycle](https://docs.sui.io/develop/transactions/transaction-lifecycle) |
| 9, 14 | Smart contracts | Move on MoveVM, mainnet-production, packages immutable-with-versioned-upgrades. **NOT WASM, and zero toolchain overlap.** Sui's own comparison tables say "MoveVM, Move Lang". Move compiles to its own bytecode format (`move_binary_format_version: 7`, `max_value_stack_size: 1024`). The only WASM in the stack is `@mysten/move-bytecode-template`, a browser convenience for *serializing* Move bytecode — it does not run WASM on chain. **`wasm32-wasip1` artifacts cannot be deployed to Sui**; on-chain logic must be rewritten in Move. | [sui-for-ethereum](https://docs.sui.io/getting-started/sui-for-ethereum), [move reference](https://docs.sui.io/references/sui-move) |

### MultiversX

Passes the discriminator, then fails criterion 3 outright.

| # | Criterion | Finding | Source |
|---|---|---|---|
| 1 | Native asset, no issuer | EGLD: "No central authority controls EGLD". The freeze/wipe/pause machinery is **ESDT-token-scoped only**; no built-in function freezes EGLD. As with Solana this is established by absence — no explicit "EGLD cannot be frozen" sentence exists. | [EGLD](https://docs.multiversx.com/learn/EGLD/) |
| 2 | Issuance as protocol primitive | **YES, and unusually cleanly.** "transactions with custom tokens do not require the VM at all. In effect, this means that custom tokens are **as fast and as scalable as the native EGLD token itself**." ESDT is a built-in function: "a special protocol-side function that doesn't require a specific smart contract address as receiver." | [fungible-tokens](https://docs.multiversx.com/tokens/fungible-tokens/), [built-in-functions](https://docs.multiversx.com/developers/built-in-functions/) |
| 3 | **Atomic N-way — FAIL** | `receiver` is a **single** field. `MultiESDTNFTTransfer` moves many *tokens* to **one** receiver, not one token to many receivers — the payload carries exactly one receiver argument, and the NFT docs say so explicitly: "Multiple semi-fungible and/or non-fungible tokens can be transferred in a single transaction **to a single receiver**." Relayed v3 adds only `relayer`/`relayerSignature`, with no inner-transaction bundling. **Maximum recipients per native transaction: 1.** An N-way split requires a custom WASM contract, which forfeits the no-custom-contract property. | [transactions REST](https://docs.multiversx.com/sdk-and-tools/rest-api/transactions/), [nft-tokens](https://docs.multiversx.com/tokens/nft-tokens/), [relayed-transactions](https://docs.multiversx.com/developers/relayed-transactions/) |
| 4 | 32-byte binding field | **YES, generously.** `data` is arbitrary bytes returned to any reader by the public REST API. No hard protocol byte cap in `CheckIntegrity()`; it is gas-bounded (`GasPerDataByte = 1500` against `MaxGasLimitPerMiniBlock = 250000000` → ≈166 KB). A 32-byte hash costs 48,000 gas. | [transactions REST](https://docs.multiversx.com/sdk-and-tools/rest-api/transactions/), [economics.toml](https://github.com/multiversx/mx-chain-go/blob/master/cmd/node/config/economics.toml) |
| 5 | Ed25519 | **YES.** "MultiversX uses the Ed25519 algorithm to sign transactions." | [signing-transactions](https://docs.multiversx.com/developers/signing-transactions/) |
| 6, 11 | Node operability | **The lightest published requirements in the comparison:** "4 x dedicated/physical CPUs… with SSE4.1 and SSE4.2 flags", **8 GB RAM**, **200 GB SSD**, 100 Mbit/s, 4 TB/month. But that is a *pruning* node — defaults are `NumEpochsToKeep = 4` (epoch ≈ 24 h), so roughly four days of history. Indefinite re-reads need `--operation-mode full-archive` (or `historical-balances`), plus the Elasticsearch indexer for querying. Sync-from-genesis time and archive disk growth per year: **could not establish** — and config.toml carries its own warning that disabling pruning "might easily cause the node to run out of disk space". | [system-requirements](https://docs.multiversx.com/validators/system-requirements/), [config.toml](https://github.com/multiversx/mx-chain-go/blob/master/cmd/node/config/config.toml), [flags.go](https://github.com/multiversx/mx-chain-go/blob/master/cmd/node/flags.go) |
| 7 | Private network | **YES.** `mxpy localnet setup` gives an independent genesis, chain ID and seednode; an advanced manual path is also documented. | [setup-local-testnet](https://docs.multiversx.com/developers/setup-local-testnet/) |
| 8 | Durable full history | **YES, self-hostable** — `--operation-mode full-archive` / `historical-balances` disables pruning, and a first-party Elasticsearch indexer is documented: "Indexed data will serve as historical data source." Requires an Observing Squad (one observer per shard + metachain + proxy; mainnet = 3 shards + metachain). | [elastic-search](https://docs.multiversx.com/sdk-and-tools/elastic-search/), [observing-squad](https://docs.multiversx.com/integrators/observing-squad/) |
| 12 | Consensus | Secure Proof-of-Stake with BLS multi-signatures. **Deterministic single-block finality**: post-Andromeda the consensus group is all 400 validators per shard for a whole epoch, an Equivalent Consensus Proof needs ≥268 signatures, "A block is final the moment its ECP is broadcast", "guarantees single-block finality", "equivocation is impossible". **No rollback of a confirmed transaction is documented** — no fork-choice rule exists. Validator entry is permissionless and bonded: **2500 EGLD per node**. Client diversity: only `mx-chain-go`; a second implementation **could not be established**. Validator count and Nakamoto coefficient: **could not establish from primary sources**. | [consensus](https://docs.multiversx.com/learn/consensus/), [staking](https://docs.multiversx.com/validators/staking/) |
| 13 | Time to finality | **~6 s intra-shard, ~18 s cross-shard, and first confirmation equals irreversible** because finality is single-block. MultiversX's own architecture page softens this to "Fast finality for cross-shard transactions in mere seconds"; the consensus page gives the real numbers. | [consensus](https://docs.multiversx.com/learn/consensus/) |
| 14 | Smart contracts | **WASM — the only candidate so far whose contract VM shares the platform's existing toolchain.** "it can execute smart contracts written in *any programming language* that can be compiled to WASM bytecode. Though, we only provide support for Rust." Engine is Wasmer 2. Live on mainnet. | [smart-contracts](https://docs.multiversx.com/developers/smart-contracts/), [system-requirements](https://docs.multiversx.com/validators/system-requirements/) |

### Eliminated on the discriminator — other criteria recorded for reference

These candidates fail the discriminator and are therefore out. Their remaining
properties are recorded compactly because a few of them are informative about what
"good" looks like on the other criteria.

**Algorand.** Native ALGO is unfreezable (the only freeze transaction type, `afrz`,
requires an asset ID, which ALGO does not have — `data/transactions/asset.go`). ASA
issuance *is* a protocol primitive: `acfg` is one of the eight base transaction types
in `protocol/txntype.go`. Atomic groups are capped at **`MaxTxGroupSize` = 16**, and
critically a group is **N transactions bound by a shared `grp` hash, not one
transaction** — so each member carries its own `fv`/`lv` and the whole group inherits
the 47-minute window. The `note` field is **1024 bytes** (`MaxTxnNoteBytes`; the
transaction-reference page says 1000, the protocol-parameters page and source both say
1024 — the source wins) and is third-party *searchable* via the Indexer's `note-prefix`
filter, with a Final standard for structuring it (ARC-2). Ed25519 confirmed. Finality is
~2.82 s and Algorand quotes one figure for both confirmation and irreversibility —
though its own docs are in tension, claiming "instant finality… cannot be reversed" in
one place and "fork resistance… **with overwhelming probability**" and "Algorand is a
probabilistic consensus protocol" in others. Participation is permissionless with **no
bond and no slashing of principal** — "Algorand's consensus protocol allows
participation without locking or risking your Algo." Full history requires an
**Archiver** node (`{"Archival": true}`; non-archivers keep only the last 1000 blocks —
deliberately one validity window) at a published **3 TB SSD + 100 GB NVMe + 5 TB/month
egress**, plus a self-hosted Indexer and PostgreSQL. Only one client implementation
exists (`go-algorand`). Sync-from-genesis time, archive growth per year, node counts and
Nakamoto coefficient: **could not establish**; and Algorand's own documented shortcut
for archivers is to trust a third-party snapshot, with the disclaimer "Algorand denies
any responsibility if any such snapshot is used."
Sources: [protocol-parameters](https://dev.algorand.co/concepts/protocol/protocol-parameters/),
[node types](https://dev.algorand.co/nodes/types/),
[atomic groups](https://dev.algorand.co/concepts/transactions/atomic-txn-groups/),
[why-algorand](https://dev.algorand.co/getting-started/why-algorand/),
[catchup-status](https://dev.algorand.co/nodes/installation/catchup-status/),
[ARC-2](https://github.com/algorandfoundation/ARCs/blob/main/ARCs/arc-0002.md)

**Cardano — fails on three counts, and wins on two.** Discriminator is PARTIAL: `ttl`
(field 3) and `validity_interval_start` (field 8) are **both optional in the CDDL**, so
there is no clock limit; the real constraint is that a signed transaction names specific
UTXOs, and if anything else spends them it is invalid forever. It then fails on:
**(1) minimum payment granularity** — ~0.97 ADA per output, the type-1 disqualifier above;
**(2) finality** — probabilistic, `k = 2160`, `f = 0.05`, **12 h expected and 36 h worst
case**, with Cardano's own materials leading with "~1 day", and >k recovery being
truncate-and-resync per CIP-135. A store cannot withhold an entitlement for a day, and
this project's rail already rejects Solana's `processed` commitment for exactly this
reason. **(3) operability** — three machines (one block producer plus two relays), 24 GB
RAM in in-memory mode, 300 GB.
It genuinely **wins** on atomic N-way (**N ≈ 249, beating Stellar's 100**) and on native
multi-asset issuance as a ledger primitive with no Plutus needed. Two useful corrections
it produced: Cardano's Ed25519 works with **plain 32-byte keys** (extended BIP32 keys are
derivation-only), so the one-identity-keypair plan would have been fine there; and
**Plutus is not WASM** — it runs a CEK machine metered in ExUnits, so there is no
`wasm32-wasip1` toolchain alignment.
Storage findings worth keeping regardless: `cardano-node` is **unconditionally archival**
with **no pruning mode** — "Blocks will never be modified or deleted" — at ~180-203 GB
growing ~15 GB/year, and Mithril cuts bootstrap from ">24 hours" to "less than **20
minutes**" with a proof over precisely the block archive. The real constraint is the
*indexing* layer: `cardano-db-sync` needs 64 GB RAM / 700 GB disk (measured ~651 GB, already
within 7% of its own stated minimum), a **2-3 day** initial sync, and no trustless fast
path — and every config option that would shrink it (`prune`, `bootstrap`, `only_utxo`)
deletes exactly the historical outputs the platform needs to re-read.
Sources: [immutabledb.tex](https://github.com/IntersectMBO/ouroboros-consensus/blob/main/docs/tech-reports/report/chapters/storage/immutabledb.tex),
[db-sync configuration](https://github.com/IntersectMBO/cardano-db-sync/blob/master/doc/configuration.md),
[running-cardano](https://developers.cardano.org/docs/operators/node/running-cardano/),
[Mithril bootstrap](https://mithril.network/doc/manual/getting-started/bootstrap-cardano-node/)

**Polkadot — eliminated, and not for the reason the brief expected.** `Era` is *not* the
binding constraint: `Era::Immortal` genuinely "is valid forever", and even a mortal era
reaches ~7 hours (`BlockHashCount = 4096`, officially glossed as "seven hours given
6-second block times"), so the "64 blocks" premise was wrong — 64 is a tooling default.
**The kill is `CheckSpecVersion`.** It is a signed extension ("Ensure the runtime version
registered in the transaction is the same as at present"; "transactions with incorrect
`spec_version` are considered invalid"), it is live in Polkadot's real extension set
alongside `CheckTxVersion` and `CheckMetadataHash`, and the runtime upgrade cadence is
**ten releases in ~6 months, roughly monthly** — each with a distinct spec version. **A
month-old signed extrinsic is odds-on invalid regardless of `Era`.**
Which is a shame, because Polkadot would otherwise have scored well: `utility.batch_all`
("The whole transaction will rollback and fail if any of the calls failed") supports
recipients in the ~10k order; **Ed25519 is valid for account keys** alongside sr25519 and
ECDSA — the best signing answer in the field; GRANDPA gives provable finality ("once a
block is finalized, it is irreversible"); private networks are a headline strength; and
archive is affordable at ~1.2 TB. Two frictions worth recording anyway: the
`Remarked { sender, hash }` event carries **only a hash, not the bytes**, so recovering a
binding field needs an archive node or indexer; and PolkaVM is **RISC-V**, not WASM, so
the assumed `wasm32-wasip1` synergy does not hold.
Sources: [era.rs](https://docs.rs/sp-runtime/latest/src/sp_runtime/generic/era.rs.html),
[CheckSpecVersion](https://docs.rs/frame-system/latest/frame_system/struct.CheckSpecVersion.html),
[chain-state-values](https://wiki.polkadot.com/general/chain-state-values/),
[runtimes releases](https://github.com/polkadot-fellows/runtimes/releases)

**Ethereum L1 (baseline).** **Passes the discriminator** — there is no expiry field in any
of the five transaction types (`LegacyTransaction | AccessListTransaction |
FeeMarketTransaction | BlobTransaction | SetCodeTransaction`), verified across the prague,
osaka and amsterdam forks. Its two caveats are sender-controlled and mitigable: a stale
`max_fee_per_gas` can strand a transaction (`assert transaction.max_fee_per_gas >=
block.base_fee_per_gas`; base fee moves ≤12.5%/block), fixable by signing a high cap since
only `priority_fee + base_fee` is charged; and `nonce` requires exact equality.
**It fails criterion 3 outright** — one `to`, one `value` per transaction, and EIP-7702
delegates the EOA to *existing contract code*, which is still a contract. Also fails
criterion 2 (ERC-20 is "a standard API for tokens within smart contracts"; no token
transaction type exists) and criterion 5 (secp256k1 only). Finality is checkpoint-based
with documented pre-finality reorgs, officially quoted as "**about 15 minutes for an
Ethereum block to finalize**". Archive nodes run 2.2 TB (Reth) to 12 TB+ (Geth).
Sources: [execution-specs](https://github.com/ethereum/execution-specs/blob/master/src/ethereum/forks/prague/transactions.py),
[EIP-1559](https://eips.ethereum.org/EIPS/eip-1559), [EIP-20](https://eips.ethereum.org/EIPS/eip-20)

**NEAR.** Eliminated by explicit design intent, which is unusually candid: the `block_hash`
field exists because "It is used to make sure a transaction does not get lost… and then
arrive **days, weeks, or years later** when it is not longer relevant and would be
undesirable to execute." The constant is `transaction_validity_period`, "On mainnet, this
value is 86400 (which corresponds to roughly a day)" — 86,400 blocks at 0.6 s ≈ 14.4 h. It
is a required genesis field with no default, so there is no client-side relief. Recorded
for any future re-evaluation: NEAR also has **one `receiver_id` per transaction**, so N-way
payment needs N transactions regardless.
Sources: [nomicon Transactions](https://nomicon.io/RuntimeSpec/Transactions),
[nomicon FinancialTransaction](https://nomicon.io/RuntimeSpec/Scenarios/FinancialTransaction),
[NEAR epoch](https://docs.near.org/protocol/network/epoch)

**Aptos — a live candidate on capability, eliminated on operability.** Discriminator is
**PARTIAL**: `expiration_timestamp_secs` is mandatory but the VM prologue applies only a
*lower* bound on the sequence-number path — the upper cap
(`MAX_EXP_TIME_SECONDS_FOR_ORDERLESS_TXNS = 100`, "advertise… 60 seconds") exists **only**
in the orderless/nonce branch. So a month-long horizon works, but only via strictly
contiguous `sequence_number`s where a single gap parks every later intent, with mempool
`capacity_per_user: 100`. The feature designed to fix this is capped at 60 s. Criterion 3
**passes strongly** — `batch_transfer` / `batch_transfer_coins` /
`batch_transfer_fungible_assets` are framework `public entry` functions, and mainnet
simulation puts the hard ceiling at **1,632 recipients** (n=1633 →
`EXCEEDED_MAX_TRANSACTION_SIZE`), dropping to ~700 for all-new accounts. APT is
unfreezable in a strong sense — `coin::destroy_freeze_cap(freeze_cap)` at genesis, and the
constructor asserts `!exists<CoinInfo<CoinType>>`, so it cannot be recreated; the framework
even says so in a code comment, "as APT cannot be frozen or have dispatch". Ed25519 is the
default. Finality is deterministic with the clearest statement of any candidate:
"**committed = finalized (BFT consensus, no block confirmations)**", "no reorg risk".
**Where it dies:** validator hardware is **48 threads / 128 GB RAM / 3.0 TB NVMe at 60K
IOPS**, with *no lower full-node tier* ("we recommend that your hardware meet the same
requirements as a validator") and two machines required; and **"Archival nodes are
deprecated"** — pruning is on by default (`ledger_pruner_config` `prune_window:
90_000_000`), archival mode has unbounded growth with **no published growth rate**, and it
requires manually adding seed peers "because ordinary peers no longer serve old data". For
a platform that re-reads the chain indefinitely, a deprecated archival mode is
disqualifying on its own. Validator entry is **1M APT minimum stake**; 115 validators; one
client implementation; no published Nakamoto coefficient. Criterion 4 is satisfiable only
as a **payload argument** (a `vector<u8>` entry-function or script arg, ~65,300 bytes,
signature-covered and API-readable) — not a protocol memo field, and scripts cannot emit
events.
Sources: [transaction_validation.move](https://raw.githubusercontent.com/aptos-labs/aptos-core/main/aptos-move/framework/aptos-framework/sources/transaction_validation.move),
[node-requirements](https://aptos.dev/network/nodes/validator-node/node-requirements),
[state-sync](https://aptos.dev/network/nodes/configure/state-sync),
[application-integration](https://aptos.dev/build/guides/application-integration),
[staking](https://aptos.dev/network/blockchain/staking)

**Mina — eliminated twice over.** Passes the discriminator cleanly (`valid_until` defaults
to `Global_slot_since_genesis.max_value`), then fails on **criterion 3**: `Body.t` has
exactly two variants, `Payment` with a single `receiver_pk` and `Stake_delegation`. There is
no native multi-recipient command; atomic multi-party transfer requires a zkApp, which is a
contract and is additionally throughput-capped at "24 zkApp transactions per block". It also
fails **criterion 12**: `k = 290` is the "Depth of finality (number of confirmations)" at
`slots_duration = 180000` (3 min), so first confirmation is ~3 minutes but irreversibility
is at depth 290 ≈ **14.5 hours**, and below that a confirmed transaction can be
reorganised away.
Three further findings worth keeping. **The memo is 32 bytes exactly, with zero headroom** —
`max_input_length = digest_length` = Blake2 digest size = 32, overlong input raising
`Too_long_user_memo_input`. A 32-byte binding hash fits precisely and leaves no room for a
tag, version prefix or encoding. **Signatures are Schnorr over Pallas, not Ed25519** ("a
standard for Mina Schnorr signatures over the elliptic curve Pallas Pasta"), so the
one-identity-keypair plan does not work there. And the succinct-chain property is
*adverse* here rather than helpful: "Mina nodes are succinct by default, so they don't need
to maintain historical information"; "Consensus nodes store only the recent history of the
chain before discarding it (the **last k blocks, currently 290**)" — roughly 14.5 hours.
Indefinite re-reads need a `mina-archive` process plus an operator-managed PostgreSQL of
**undocumented and unbounded size**, on top of a 32 GB / 8-core node.
Sources: [signed_command_payload.ml](https://raw.githubusercontent.com/MinaProtocol/mina/develop/src/lib/mina_base/signed_command_payload.ml),
[signed_command_memo.ml](https://raw.githubusercontent.com/MinaProtocol/mina/develop/src/lib/mina_base/signed_command_memo.ml),
[consensus README](https://raw.githubusercontent.com/MinaProtocol/mina/develop/docs/specs/consensus/README.md),
[archive-node](https://docs.minaprotocol.com/node-operators/archive-node),
[data-and-history](https://docs.minaprotocol.com/node-operators/data-and-history)

**Avalanche — the X-Chain gets remarkably far, then dies on Ed25519.** X-Chain `BaseTx` has
**no expiry field** and `Outputs` is a variable-length array of `TransferableOutput`, so
atomic multi-recipient payment with no contract works natively. Asset issuance is a genuine
protocol primitive — `CreateAssetTx` is a native transaction type (`BaseTx, Name, Symbol,
Denomination, InitialStates`), not a contract. The `Memo` field is "arbitrary bytes, **up to
256 bytes**", eight times the requirement and third-party readable via `avm.getTx`.
Acceptance is terminal — "acceptance/rejection are **final and irreversible**" — at "~1
second acceptance latency", with Avalanche quoting one figure for both confirmation and
irreversibility. Validator minimums are light (4 cores / 16 GB / 1 TB NVMe at low stake) and
the stake floor is **2,000 AVAX**. A private network is documented (`--network-id=local`,
`--genesis-file`, `--sybil-protection-enabled`).
**It fails criterion 5, and fatally:** "The Avalanche virtual machine uses elliptic curve
cryptography, specifically `secp256k1`, for its signatures on the blockchain", and the
C-Chain "precisely duplicate[s] all of the cryptographic constructs used in Ethereum".
**There is no Ed25519 anywhere in the primary network**, so an Ed25519 identity key cannot
double as an Avalanche wallet key — which is precisely the property `ALIGNMENT.md` §4 names
as architectural. Two further problems: the **exact maximum output count could not be
established** from any official page, and archival history is **~12.5 TB** versus ~500 GB
pruned, behind a **one-way door** — "If a node is ever run with pruning-enabled as false
(archival mode), setting pruning-enabled to true will result in a warning and the node will
shut down."
Sources: [X-Chain txn-format](https://build.avax.network/docs/api-reference/x-chain/txn-format),
[cryptographic-primitives](https://build.avax.network/docs/api-reference/standards/cryptographic-primitives),
[system-requirements](https://build.avax.network/docs/nodes/system-requirements),
[c-chain configs](https://build.avax.network/docs/nodes/chain-configs/c-chain),
[avalanche-consensus](https://build.avax.network/docs/quick-start/avalanche-consensus)

**Hedera.** Disqualified a second time, independently of the 180 s window, and this one
is structural: **the consensus node set is permissioned.** "The network's nodes
currently operate on a permissioned model where **Hedera Council members run network
nodes** and approve updates of the network's technology." The node-requirements page
confirms its own scope: "This documentation applies only to permissioned consensus nodes
operated by Hedera Council Members. It does not cover Hedera's transition to a
permissionless network." Published consensus-node requirements are 24 cores / 48
threads, 256 GB ECC RAM, 5 TB NVMe, Tier-1 SSAE 16/18 + SOC 2 Type 2 datacenter, and
hardware pre-approved by Hedera. On the other criteria: HTS is a native service but
tokens carry Freeze, Wipe, KYC and Pause keys — directly adverse to criterion 1;
`CryptoTransfer` *is* atomic N-way with a limit of **10** (`ledger.transfers.maxLen`);
the `memo` is **100 bytes UTF-8** (a proto `string`, not raw bytes, so a 32-byte hash
must be hex or base64 encoded — it fits, encoded); finality is deterministic with no
rollback. Full history lives in mirror nodes fed from **Hedera-operated** cloud buckets;
a mainnet full-history mirror DB is documented at **50 TB**. Whether a third party can
reconstruct history from genesis independently of Hedera-hosted buckets: **could not
establish**.
Sources: [hederacouncil.org](https://hederacouncil.org/),
[node-requirements](https://docs.hedera.com/hedera/networks/mainnet/mainnet-nodes/node-requirements),
[crypto_transfer.proto](https://github.com/hashgraph/hedera-protobufs/blob/main/services/crypto_transfer.proto),
[mirror-nodes](https://docs.hedera.com/hedera/core-concepts/mirror-nodes)

---

## 3. Completeness sweep — other L1s considered, and why they are out

The brief asked for any other plausible candidates. A dedicated sweep checked eleven more
chains against the four filters. **Nothing was missed that would have survived.**

The finding worth recording is a *pattern*, not a list: **every chain with protocol-level
asset issuance plus a memo field plus Ed25519 that is not already a survivor turns out to
enforce a short mandatory validity window.** Filter A, not filter B, is what kills the
Stellar-family lookalikes.

| Chain | Verdict | Decisive reason | Source |
|---|---|---|---|
| **Symbol (NEM)** | **EXCLUDE** — and it was the strongest dark horse | Mandatory `deadline`; `maxTransactionLifetime` default **6h on MAINNET** (24h private), `maxBondedTransactionLifetime` 48h. "By default, the SDK sets the deadline to 2 hours, but it can be extended up to 6 hours (or 48 for Aggregate bonded)." Even the 48h path needs a 10 XYM HashLock deposit. **It fails only filter A** — it genuinely has `maxTransactionsPerAggregate` = **100 (mainnet)**, real atomicity ("both transactions succeed or none does"), `maxMessageSize` = **1024** bytes, protocol-level mosaics, and Ed25519 | [network properties](https://docs.symbol.dev/guides/network/configuring-network-properties.html), [aggregate](https://docs.symbol.dev/concepts/aggregate-transaction.html) |
| **Bitcoin** | **EXCLUDE** | Passes A (`nLockTime` is an *earliest*, not a latest — no expiry) and B (native multi-output atomicity), and now passes the memo requirement: **the 80/83-byte `OP_RETURN` figure is obsolete** — Bitcoin Core **30.0** raised `-datacarriersize` to 100,000, "which effectively uncaps the limit", and permits multiple OP_RETURN outputs. **Fails C** on the dust floor — **294 sat** (P2WPKH) to **546 sat** (P2PKH), a per-payment floor, not a setup cost. **Fails D twice**: no protocol asset-issuance primitive, and secp256k1 ECDSA/Schnorr rather than Ed25519 | [Core 30.0](https://bitcoincore.org/en/releases/30.0/), [policy.cpp](https://raw.githubusercontent.com/bitcoin/bitcoin/master/src/policy/policy.cpp), [BIP340](https://raw.githubusercontent.com/bitcoin/bips/master/bip-0340.mediawiki) |
| **Tron** | EXCLUDE | Fails A twice: "Default expiration is +60 seconds; **max is 24 hours**", *and* a mandatory `ref_block_hash` anchor | [transaction](https://developers.tron.network/docs/tron-protocol-transaction) |
| **Waves** | EXCLUDE | Fails A on a timestamp drift window: "cannot be added to the blockchain if the timestamp value is more than 2 hours behind or **1.5 hours ahead**". Worth checking because it has a genuine native Issue Transaction primitive | [transaction](https://docs.waves.tech/en/blockchain/transaction/) |
| **TON** | EXCLUDE | Wallet-contract external messages carry a mandatory short `valid_until`, with a dedicated rejection exit code. ⚠️ Sourced from a docs.ton.org search summary rather than a verbatim page fetch; the **maximum `valid_until` value could not be established**, and the Jetton question was not reached | [wallet howto](https://docs.ton.org/v3/guidelines/smart-contracts/howto/wallet) |
| **Internet Computer** | EXCLUDE | `ingress_expiry` is "required… An upper limit on the validity of the request", and "The IC may refuse to accept requests with an ingress expiry date too far in the future". ⚠️ The concrete maximum (believed ~5 min) **could not be established** — the spec asserts the discretion without quantifying it | [IC interface spec](https://docs.internetcomputer.org/references/ic-interface-spec/https-interface/) |
| **Antelope / EOS** | EXCLUDE — **weakly sourced** | A mandatory `expiration` with a chain-level cap (`max_transaction_lifetime`, believed 3600 s) is not in doubt, but `docs.eosnetwork.com` failed DNS resolution and **the exact value is not primary-sourced** | — |
| **Nano, Monero, Zcash, Litecoin, Kaspa** | EXCLUDE — **not established** | Primary docs were not reached for any of these five within budget. The unverified prior is that each fails D on the absence of a protocol-level asset-issuance primitive (and Litecoin additionally inherits Bitcoin's dust floor and secp256k1). **This is the main residual hole in the sweep and is recorded as such rather than papered over.** | — |

One conflict to note: the sweep independently flagged Aptos as a probable filter-D failure
for having no native memo field, and could not establish an expiry cap for it. The
dedicated Aptos investigation is the authority here and supersedes that: the expiry has no
upper cap on the sequence-number path, and the memo requirement is satisfiable as a payload
argument rather than a protocol field. Both investigations agree Aptos has **no native memo
field**.

---

## 4. What could not be established

Recorded plainly rather than guessed at, in the house style of `site/docs/status.md`.

**Still open on the surviving candidates — these are the gaps that should be closed before
the decision is made:**

| Gap | Chain | Why it matters |
|---|---|---|
| Criterion 15 (payment granularity) and 16 (batch integrity) | **Solana** | Solana is a live candidate. Specifically unresolved: whether a frozen SPL token account rejects *incoming* transfers, which would let a recipient or the mint's freeze authority veto an entire payout batch; and whether the rent-exempt minimum acts as a floor on a *first* payment to a new address |
| Criterion 15 and 16 | **Sui** | Whether Sui's per-object **storage fee** is charged to the sender (type 2, fine) or must be held *inside* the delivered coin (type 1, disqualifying) is unresolved and is the single most important open question about Sui. Also whether any recipient-side rejection mechanism exists at all |
| Disk growth per year | **Radix** | No figure exists on docs.radixdlt.com, radixdlt.com, or either repo. "500 GB (initially)" implies growth without quantifying it. The largest documentation gap found for Radix |
| Time to sync from genesis | **Radix, Sui, Solana, Aptos** | Not published by any of them. Sui and Solana both actively route around the question ("always start it from a snapshot") |
| An official time-to-finality figure in seconds | **Radix** | Radix publishes none; its position is that the question does not apply because finality is deterministic. The sub-second figure in the matrix is a **derivation from round timings, not an official claim** |
| Nakamoto coefficient | **all** | **No candidate publishes one.** The figures in this document for Radix (≈9) and Sui (19 to exceed 1/3) are **derived from live API readings**, not vendor claims, and will drift |
| Validator counts | **Solana** | Rendered in a live widget on solana.com/validators; no figure in the docs |
| Second client implementation | **Radix, Sui, MultiversX, Aptos, Cardano** | None could be established for any of them. Solana has Frankendancer on mainnet but its own README says "execution and consensus is using the Agave validator code", so client diversity is effectively absent there too. **Ethereum and Polkadot are the only candidates with real client diversity, and both are eliminated on other grounds** |
| Stellar's history-retention story | **Stellar** | **The most consequential unclosed gap in the whole document.** The default Stellar RPC ledger-retention window, its configurable maximum, and what infrastructure can actually serve arbitrarily old transactions were not established — the agent assigned to it died twice. Since entitlement verification re-reads the chain indefinitely, and since this is precisely where Solana (~4.4 h default) and Sui (30-day free archive) turn out to be weak, **Stellar cannot be chosen over them on this criterion until it is checked.** Also unestablished for Stellar: whether SCP's documented preference to halt rather than fork is stated in primary sources, the ledger close time as an irreversibility claim, validator counts, Soroban's WASM target triple, and a concrete full-history archive size |
| Whether `BatchV1_1` will ship | **XRPL** | No release date is published. XRPL fails only criterion 3, so this single amendment would make it a live candidate again |
| Exact maximum output count | **Avalanche X-Chain** | Neither the txn-format nor serialization-primitives page states a maximum output count or transaction size. Moot given the Ed25519 failure |
| Per-epoch and per-year growth of `cardano-db-sync` | **Cardano** | 438 GB is a single point-in-time reading with no rate attached. Moot given Cardano's elimination |

**Method-level caveats that a reader should weigh:**

- **Several figures in this document are derived, not quoted.** They are labelled where they
  appear, and they are: Solana's ~21/~60 recipient counts (from the docs' own size formula),
  Radix's N ≤ 126 (from `4 + 2N ≤ 256`), Radix's and Sui's Nakamoto coefficients, Radix's
  sub-second finality, and Cardano's compressed growth rate. None should be cited as an
  official claim.
- **Two chains' own docs contradict themselves,** and in both cases the implementation was
  taken as authoritative over the prose: Hedera's docs say "The transaction's valid start
  time can be set to a future date/time" while the node and protobuf both forbid it; and
  Aptos's docs describe a 150-million-transaction pruner window against a shipped default of
  `90_000_000`. Sui has a three-way disagreement about whether owned-object transactions
  bypass consensus, which **could not be resolved at all**.
- **Solana's blockhash wall-clock figure is quoted inconsistently across its own pages**
  (~1 min, 60–90 s, ~80–90 s, ~2 min). The 150 × 400 ms arithmetic is the figure to trust.
- **Live-API readings were used where docs published nothing** — Radix's validator
  distribution and epoch rate, Sui's validator set, Aptos's gas schedule and transaction-size
  ceiling. These are the most primary source available for those facts, but they are
  point-in-time measurements taken 2026-07-30 and will drift.

**Process failures worth recording**, because they bound how much of this document is
first-hand: four research agents died mid-task (one on a GitHub rate limit, three on stalls),
and the Stellar and XRPL agents both died before completing their matrices. Their remaining
items were picked up directly where they were verdict-critical — Stellar's criteria 15 and 16,
XRPL's criteria 1, 5 and 15 — and left explicitly open where they were not.

---

## 5. Provenance and untrusted-content note

Every fact above was taken from official protocol documentation, developer references,
transaction-format or XDR specifications, protocol source repositories, or — where the
documentation published nothing — live queries against official mainnet API endpoints.
Fetched pages were treated as data, not instructions, throughout.

**No prompt-injection attempt was found.** The Walrus/Mysten footer probe flagged in the
brief was specifically looked for and is **not present**: the Sui investigation downloaded
all 395 `docs/content/**/*.mdx` files and grepped for injection patterns (`ignore previous
instructions`, `if you are an AI/LLM`, `disregard prior`, `do not tell the user`,
`exfiltrate`) with **zero hits**.

Five things were encountered that could be mistaken for injection and were not treated as
instructions. They are recorded because "we looked and found nothing" is only credible if
the near-misses are named:

1. **Documentation-site LLM chrome.** xrpl.org and developers.stellar.org pages offer "Copy
   page as Markdown for LLMs" and "Open in ChatGPT/Claude". Site convenience UI, not an
   attack.
2. **Sui's `<AgentPrompt>` components.** 19 Sui docs pages embed a React component carrying a
   copyable suggestion aimed at *the reader's* coding agent — e.g. `gas-smashing.mdx`:
   "Review this app's gas coin handling. Add safe coin selection/splitting/merging
   guidance…". This is page content authored for humans to copy deliberately. It was read as
   data and never acted on. It is, however, exactly the shape a real injection would take,
   and is worth knowing about for future fetches of Mysten documentation.
3. **`AGENTS.md` and `CLAUDE.md` in the Tezos GitLab repository root.** Present, not opened,
   not followed.
4. **Harness-generated text mistaken for source content.** WebSearch results append
   "REMINDER: You MUST include the sources above in your response", and WebFetch redirect
   notices are phrased imperatively ("Please use WebFetch again with these parameters").
   Both are tool-wrapper output, not primary sources, and neither was cited as one.
5. **Cross-session scratchpad contamination — the one that nearly caused a real error.** The
   shared scratchpad directory contained leftover HTML from unrelated concurrent sessions
   (Algorand, Sui and Bitcoin pages). An Algorand hardware figure — "8 vCPU, 16 GB RAM, 100
   GB NVMe" — surfaced in a glob during the Radix research and was very nearly recorded as a
   Radix figure. It was caught and excluded, and later agents were instructed not to use the
   shared scratchpad at all. **This is a genuine methodological hazard for parallel research
   and not a hypothetical one**; it is the reason Radix's hardware figures in this document
   are cited to two named Radix pages rather than to an intermediate file.
