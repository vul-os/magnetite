<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>
<style>
.mg-plate{margin:1.9rem 0;border:1px solid var(--dv-border);border-radius:10px;overflow:hidden;background:var(--dv-surface);box-shadow:var(--dv-shadow-sm)}
.mg-plate img{display:block;width:100%;height:auto;margin:0}
.mg-dark{display:none}
:root[data-theme="dark"] .mg-light{display:none}
:root[data-theme="dark"] .mg-dark{display:block}
.mg-cap{padding:11px 15px;border-top:1px solid var(--dv-border);background:var(--dv-code-bg);font-family:var(--doc-mono);font-size:.76rem;line-height:1.6;color:var(--dv-ink-3)}
.mg-cap b{color:var(--accent);font-weight:600;letter-spacing:.09em;text-transform:uppercase;font-size:.68rem;display:block;margin-bottom:3px}
.mg-cap code,.mg-cap b code{font-size:1em}
.mg-bar{display:flex;align-items:center;gap:6px;padding:8px 13px;border-bottom:1px solid var(--dv-border);background:var(--dv-code-bg)}
.mg-bar i{width:8px;height:8px;border-radius:50%;background:var(--dv-border-2)}
.mg-bar span{margin-left:7px;font-family:var(--doc-mono);font-size:.68rem;color:var(--dv-ink-faint)}
/* --dv-ink-faint reads ~3.4:1 on --dv-code-bg at this size — short of AA. */
:root[data-theme="light"] .mg-bar span{color:#5A6171}
:root[data-theme="dark"] .mg-bar span{color:#768096}
</style>

# Payments

**Non-custodial crypto. No fiat, no balances, no payouts, no custody.**

> [!WARNING]
> **Only the mock rail ships.** Everything on this page describes a model that
> is implemented behind the `PaymentRail` trait — but the single rail present in
> the tree is `MockPaymentRail`, a deterministic offline stub that signs
> receipts so CI can run without a network. **No chain is integrated and no real
> payment has ever settled through magnetite.** The fiat and custody code
> described under "What this replaces" is genuinely deleted; nothing was added in
> its place.

Magnetite never holds player or developer money. There is no
`wallet_balances` table, no `payout_requests` queue, and no platform bank
account standing between a purchase and the person who earned it. Every money
movement is a direct, wallet-to-wallet, on-chain (or channel) transaction, and
the `PaymentRail` seam is the only thing in the codebase that knows which
chain or rail is in use.

```rust
trait PaymentRail {
    async fn checkout(&self, buyer: &PubKey, split: PaymentSplit) -> Receipt;
    async fn checkout_for_item(&self, buyer: &PubKey, item: &str, split: PaymentSplit)
        -> Result<Receipt, PaymentError>;
    async fn open_channel(&self, peer: &PubKey) -> Result<Channel, PaymentError>;
    async fn escrow(&self, terms: WagerTerms) -> Result<Escrow, PaymentError>;
    fn verify_receipt(&self, r: &Receipt) -> bool;
    fn verify_receipt_for_item(&self, r: &Receipt, item: &str) -> bool;
}
```

Two rails ship today:

| rail | `PAYMENT_RAIL` | status | moves money? |
|---|---|---|---|
| `MockPaymentRail` | `mock` (**default**) | deterministic, offline, signs its own receipts | no |
| `SolanaPaymentRail` | `solana` (needs `--features solana`) | builds real SPL USDC transfers; verification re-reads the chain. **Never run against a validator** | **would, on mainnet** |

## Three money flows

1. **Item / DLC purchase** — a `checkout()` call splits payment atomically
   across a **list of legs** (`PaymentSplit { legs: Vec<Leg> }`, see below). The
   entitlement a player owns *is* a signed receipt keyed
   `(buyer pubkey, game hash, item)` — the node checks the receipt to grant
   access, not a database row it owns.
2. **Hosting fee** — the incentive to bring a big server. An operator gets
   paid per-seat or per-hour over a payment channel (`open_channel`), so
   joining a match doesn't cost on-chain gas per player. This is what makes
   [capacity-elastic hosting](./docs.html#hosting-a-server) economically real, not just
   technically possible. **Mock rail only today** — the Solana rail has no
   channel program and refuses the call (see below).
3. **Wager / tournament (optional)** — **mock rail only today.** An `escrow()` is settled by
   `verify_replay` (see [Architecture](./docs.html#architecture)), so the outcome of a
   wagered match is provable from the replay log, not adjudicated by a
   platform.

## The split is a list of legs

A payment is a list of destinations that must sum **exactly** to what the buyer
is charged. There is no fixed `{developer, operator, fee}` shape:

```rust
pub struct PaymentSplit { pub legs: Vec<Leg> }
pub struct Leg { pub wallet: PubKey, pub amount: u64, pub role: Role }
pub enum Role { Developer, Operator, Stewards, Other(String) }
```

Co-developers, publishers, asset-pack authors, operator revenue shares, charity
splits and the voluntary contribution to magnetite's own maintainers are all the
same code path and the same arithmetic. `Role` is a **label**, not a branch —
except for one case, `Stewards`, whose destination is not the caller's to choose
(below).

### Proportions and amounts are different layers

A signed manifest cannot carry absolute amounts: it is signed at publish time,
and for a pay-what-you-want or tipped purchase the total is not known until
checkout. So proportions and amounts live at different layers, with exactly one
conversion between them:

| layer | carries | signed when |
|---|---|---|
| package manifest | proportions — basis points summing to exactly `10_000` | at publish |
| checkout / rail | absolute `Leg` amounts summing exactly to the total | at purchase |

The manifest side's `SplitPlan::resolve(total)` is the **only** place basis
points become amounts, using largest-remainder allocation so the result is
sum-exact for every total — including 0, 1, and totals that do not divide
evenly. Neither `PaymentSplit` nor either rail performs any proportional
allocation: they take amounts as given and check that they sum exactly. The
`subtotal * bps / 10_000` that used to live in both rails is deleted, because
two implementations of that arithmetic disagree at the first awkward total.

The rules the money math obeys:

* **the total is derived, never asserted.** `PaymentSplit` has no total field, so
  there is nothing a caller can set that disagrees with the legs;
* **integers only.** Every amount is a count of the currency's smallest unit
  (micro-USDC on the Solana rail: 6 decimals). No floating point appears anywhere
  in the money path, and there is **no rate, percentage or basis-points figure**
  in the seam at all — a split arrives with every amount already decided;
* **overflow refuses, never saturates.** A leg list that cannot be summed in
  `u64` is rejected. `u64::MAX` would be a lie about what the buyer owes;
* **the rail refuses any plan whose paid legs do not sum exactly to the receipt
  total**, and refuses a plan with no payable leg at all;
* **the receipt binding commits to every leg.** See below.

### The receipt is bound to one distribution

The binding reference is

```
blake3("magnetite-pay-v2" ‖ buyer ‖ len(item) ‖ item ‖ split_digest(legs))
```

where `split_digest` is a domain-separated hash over the ordered legs — each
wallet, each amount, each role tag (length-prefixed, so no two distinct leg lists
can encode to the same bytes) — and it is carried **both** in the receipt and as
the on-chain memo.

The previous shape folded `protocol_fee_bps` into the receipt seed. That
committed to a *rate* and to nothing about where the money went: two different
distributions of the same total produced the same seed, so a receipt could be
re-pointed from one to the other. It now cannot — and because the memo is part of
the on-chain record, the distribution is pinned by something the receipt's holder
cannot edit.

### Dust floor: skipped, never fatal

A leg below the paying rail's dust floor is **skipped**. The instruction is not
emitted, the buyer is not charged for it, and the total shrinks by exactly that
amount — so a rounding-dust voluntary contribution can never break a purchase.

The Solana rail's floor is **`DUST_FLOOR_MICRO_USDC = 1_000` — 0.001 USDC, a
tenth of a cent.** A leg must be worth at least what it costs to move, and the
payer covers two costs a small leg does not shrink below: the transaction's base
fee (5 000 lamports per signature, on the order of a tenth of a cent) and, one-off
per recipient, the rent to create the destination's associated token account
(~0.00204 SOL, two orders of magnitude more). Below a tenth of a cent the leg
costs the buyer more than the recipient receives; it is negative-sum, so it is not
sent. It is a fixed integer chosen once and deliberately **not** a live currency
conversion — converting would need a price oracle (a party who can lie) and
floating point (which has no place in the money path).

Two things the floor deliberately does **not** do:

* **it never makes a purchase free.** A split where *every* leg is dust is
  refused (`NoPayableLeg`), not settled as a zero-value entitlement.
* **it is not a seam-wide constant.** A value-based floor needs to know what a
  unit is worth, and the seam does not: the backend denominates in USD cents
  today while the chain rail denominates in micro-USDC. One constant would mean
  0.001 USDC on one rail and $10 on the other — and on the second it would
  silently skip the *developer's* leg on any ordinary purchase, which is a
  fail-open dressed as a safety feature. So the mechanism lives in the seam and
  the number lives with whoever knows the unit. `MockPaymentRail` defaults to
  `ZERO_ONLY_DUST_FLOOR = 1` (only a zero-amount leg is dust) and takes an
  explicit floor from a caller that does know.

> **Known unit inconsistency, pre-existing.** `backend`'s `units_from_usd` yields
> **cents** (`price × 100`), while the Solana rail treats every amount as
> **micro-USDC**. A $1.99 item therefore reaches the chain rail as 199
> micro-USDC. Nothing has ever settled through that path so no money has been
> mispriced, but the two must be reconciled before a devnet round-trip.

## Where the stewards contribution goes

`Role::Stewards` is a **voluntary** contribution to magnetite's maintainers.
There is no enforceable mandatory fee in a system with no central server, so
there is no protocol fee — and, importantly, **neither the rate nor the
destination is a node-operator setting.**

| who | sets | where |
|---|---|---|
| Developer | price model, own split, stewards rate | signed manifest |
| Operator | their own charge, or nothing | signed session ad |
| Player | optional tip on top | checkout |

### Why the old wiring was wrong

`PROTOCOL_FEE_BPS` and `SOLANA_FEE_WALLET` are **gone**. Both were node
environment variables, and both governed someone else's money:

* the **rate** was set by whoever ran the machine, on a *developer's* sale.
* the **destination** was worse, because verification could not catch the abuse.
  The ten checks below prove that the recipients a receipt *claims* are the ones
  the chain actually paid — **not that they are the right parties.** An operator
  who pointed `SOLANA_FEE_WALLET` at their own wallet produced receipts that
  verified perfectly while redirecting other people's contribution to
  themselves.

### How the destination is sourced now

The address is **compiled into the binary** at build time, from
`MAGNETITE_STEWARDS_WALLET` (via `option_env!`, which the *compiler* evaluates —
no runtime environment can change it). A release publishes those bytes under a
`SHA256SUMS` manifest plus a provenance attestation
(`scripts/release-checksums.sh`, `scripts/verify.sh`), and `verify.sh` fails
closed on any mismatch, including an absent manifest.

So changing the address requires producing a **different binary**, whose digest
will not match the published manifest. That is the intended difference in kind:
**a fork changing it is expected and visible; a host silently redirecting it is
not possible.** Anyone may build their own magnetite pointing the stewards leg
wherever they like — what they cannot do is that *while claiming to run the
published release*.

Two enforcement points, not one:

* **at charge time**, `plan()` refuses a `Stewards` leg naming any wallet other
  than the compiled-in address — so labelling a leg `Stewards` does not let a
  caller choose where it goes;
* **at verify time**, a receipt *claiming* a stewards leg to any other wallet is
  denied, and the receipt's stewards figure must equal the sum of its stewards
  legs.

> **No stewards address is compiled into this tree.** Magnetite has no published
> one yet, so `MAGNETITE_STEWARDS_WALLET` is unset in every build here and
> `stewards::COMPILED_IN` is `None`. While it is `None` a stewards leg is
> **refused** — not redirected to some default, and not silently dropped. Absence
> fails closed like everything else.

### The devnet-only override

`MAGNETITE_STEWARDS_WALLET_DEVNET` lets a developer exercise the stewards path
against a local validator without rebuilding. It has exactly three states, and
the third is the one that matters:

| state | outcome |
|---|---|
| unset (and nothing compiled in) | **not an error.** No stewards leg can be paid; one is refused if requested |
| set on devnet / testnet / localnet | honoured |
| set while `SOLANA_CLUSTER=mainnet-beta`, or unparseable | **fatal at startup** |

It is never *ignored*, because "set and quietly disregarded" is how an override
becomes a fallback nobody notices. On a build or rail that cannot honour it at
all — `PAYMENT_RAIL=mock`, which moves no money — the process says so loudly at
startup rather than pretending the variable did something.

## What this replaces

The old model ran fiat balances through Paystack (deposits, subscriptions) and
paid developers out through Wise, with a platform-held 70/30 split and a
`ZAR → USD` conversion step. All of that — Paystack, Wise, `wallet_balances`,
`wallet_transactions`, `developer_balances`, the payout/payout-requests
split-brain — is cut. Custody was only ever needed because the platform held
the money in between; a non-custodial rail removes the reason to hold it at
all.

<div class="mg-plate">
<div class="mg-bar"><i></i><i></i><i></i><span>magnetite · /wallet</span></div>
<img class="mg-light" src="./shots/wallet-light.png" alt="" loading="lazy" decoding="async" />
<img class="mg-dark" src="./shots/wallet-dark.png" alt="magnetite wallet screen: a linked address and a ledger of signed receipts, labelled non-custodial with the payment rail set to mock" loading="lazy" decoding="async" />
<div class="mg-cap"><b>The player side — receipts, not balances</b>There is no balance field on this screen because the platform holds none. Each row is a receipt verifiable against the rail’s signing key. The card names its own rail: <code>RAIL: MOCK</code>.</div>
</div>

<div class="mg-plate">
<div class="mg-bar"><i></i><i></i><i></i><span>magnetite · /developers/earnings</span></div>
<img class="mg-light" src="./shots/earnings-light.png" alt="" loading="lazy" decoding="async" />
<img class="mg-dark" src="./shots/earnings-dark.png" alt="magnetite developer revenue screen: settled USDC received, signed receipt count and a zero-basis-point protocol fee, stating there is no custodial balance or payout queue" loading="lazy" decoding="async" />
<div class="mg-cap"><b>The developer side — nothing to withdraw</b>A sale settles wallet-to-wallet at checkout, so the screen reports what already arrived rather than what is owed. The fiat balance, payout queue and bank details this replaced are deleted, not hidden. Figures are fixture data on the mock rail.</div>
</div>

## The Solana / USDC rail

Solana is the first real rail: its Ed25519-native keys let a player's identity
key double as their wallet key, so the seam needs no key-mapping table.

It lives in its own crate, `magnetite-solana-rail` (not a `magnetite-seams`
feature — `magnetite-seams` itself has zero dependency, even optional, on the
sibling `patala` repo, so its own `cargo build`/`cargo test` never touch it).
`backend` compiles it in **only** with `--features solana` and selects it
**only** by `PAYMENT_RAIL=solana`. With the feature off the default build
pulls in no chain dependencies, opens no sockets, and every test runs offline.

### What is patala's, and what is magnetite's

The rail lives in its own crate over the sibling `patala` repo, and the division
matters for what each is responsible for.

**patala owns the cryptography and chain primitives**: Ed25519 keys and signing,
base58, program-derived and associated-token-account derivation, SPL
`TransferChecked` and Memo instruction encoding, legacy message serialization,
the wire transaction format, the JSON-RPC client and the `SolanaRpc` trait, and
the cluster / commitment / mint configuration. None of it is reimplemented in
magnetite.

**magnetite owns the multi-leg split**, because patala cannot express it:
`patala_core::PaymentRail::charge` moves money to exactly one destination, and
its `verify` checks exactly one recipient's balance delta (`PATALA.md` §3). So
`checkout_item` composes patala's instruction primitives into one transaction
paying every leg, and the ten checks below run over an arbitrary number of
recipients.

An earlier version of this crate delegated whole `charge`/`verify` calls to
patala and **refused** any split with more than one non-zero leg. That refusal is
gone because the capability is now real — not because the constraint was wished
away. When patala grows a multi-destination seam, this should delegate again.

### What is real

* **`checkout_for_item` builds ONE transaction** containing one SPL
  `TransferChecked` instruction **per leg** — any number of them — plus an SPL
  Memo carrying the `(buyer, item, legs)` binding. Because it is a single
  transaction, the split is atomic *by construction*: Solana lands every leg or
  none. **No custom on-chain program is involved.**
* **An over-large split is refused, never truncated.** The serialized wire
  transaction is measured against Solana's 1232-byte limit *before* submission;
  over it, the rail returns `TooManyLegs`. Dropping a leg would break
  sum-exactness, and letting the cluster reject it would leave a buyer who
  believes they paid.
* **`verify_receipt` / `verify_receipt_for_item` re-read the chain.** The
  receipt is treated purely as a *claim*.
* **Money math is integer-only.** USDC has 6 decimals; every amount is a count
  of smallest units (micro-USDC). No floats appear anywhere in the money path,
  no rate or percentage exists in the rail, and it refuses to produce a plan
  whose legs do not sum exactly to the total.

### What is NOT real — read this before promising anything

* **`open_channel` returns an error.** Payment channels need an on-chain program
  that does not exist. The rail returns
  `PaymentError::Unsupported("payment channels")` rather than a stub that looks
  like it worked. The hosting-fee flow therefore does **not** work on the Solana
  rail today; it works on the mock rail only.
* **`escrow` returns an error**, for the same reason
  (`PaymentError::Unsupported("wager escrow")`). Wagers are mock-only.
* **`checkout` (the item-less form) produces an unbindable receipt** that always
  fails verification. Chain receipts must name their item. Use
  `checkout_for_item`.
* **The rail is non-custodial and will not pretend otherwise.** It can only sign
  for a wallet whose key it holds. For a buyer's own wallet, use
  `build_message()` to produce an unsigned transaction, have the buyer's wallet
  sign and submit it, then call `receipt_for_signature()` and verify.

### Fail-closed verification

`verify_receipt_for_item` checks **all** of the following, and returns `false`
if *any* of them cannot be established:

1. the receipt carries a chain binding, and the chain is `solana`;
2. the claimed mint equals the configured USDC mint;
3. the binding reference equals
   `blake3("magnetite-pay-v2" ‖ buyer ‖ item ‖ split_digest(legs))` — so a
   receipt cannot be re-pointed at another item **or at a different
   distribution** by editing a field. The legs are read from the rail proof and
   must match the receipt's payouts exactly, and any leg claiming the `stewards`
   role must pay the compiled-in stewards address;
4. the bound item is the item the *caller* is asking about;
5. payouts sum exactly to the total (checked, no overflow), the stewards figure
   does not exceed the total and equals the sum of the stewards legs, and the
   rail signature is intact;
6. the transaction is known to the cluster **at the configured commitment**
   (`confirmed` or `finalized`; `processed` is rejected outright because it can
   be rolled back);
7. `meta.err` is null — a landed-but-failed transaction moved nothing;
8. the buyer appears in `accountKeys` as a **signer**;
9. the on-chain memo is exactly the derived binding;
10. net token-balance deltas for the configured mint are exactly `-total` for
    the buyer and `+amount` for each claimed recipient — **at any leg count** —
    with **no unaccounted party** gaining or losing that mint in the
    transaction. Recipients are aggregated by wallet first, because the chain
    reports one net delta per owner and the same wallet may legitimately hold two
    legs (a co-developer who is also the operator); comparing per-leg would
    reject that honest case. Balance deltas rather than instruction contents mean
    a transfer cancelled out by a hidden reverse transfer in the same transaction
    cannot pass.

> **There is no fail-open path.** RPC unreachable, RPC error, transaction
> unknown, unconfirmed, malformed JSON, panicking verification thread — every
> one of them returns `false`. "Cannot verify" never grants an entitlement,
> because a fail-open here would give paid items away for free. The cost of the
> conservative choice is that an RPC outage temporarily blocks purchases; the
> cost of the other choice is unbounded.

A transaction bound to one item is **not** redeemable for another: the memo is
part of the on-chain record, and the caller-supplied item is checked against it.

### Configuration

| env | required | meaning |
|---|---|---|
| `PAYMENT_RAIL` | no (default `mock`) | `mock` or `solana` |
| `SOLANA_RPC_URL` | **yes** | JSON-RPC endpoint, `http(s)` |
| `SOLANA_CLUSTER` | **yes** | `mainnet-beta` \| `devnet` \| `testnet` \| `localnet` |
| `SOLANA_COMMITMENT` | no (default `finalized`) | `confirmed` or `finalized` |
| `SOLANA_USDC_MINT` | no | base58 mint; defaults to the canonical mint for the cluster |
| `MAGNETITE_STEWARDS_WALLET_DEVNET` | no | base58; **devnet/testnet/localnet only** — fatal on mainnet |
| `SOLANA_KEYPAIR_PATH` | no | solana-CLI JSON keyfile — **`chmod 600`, owned by the service user** |
| `SOLANA_KEYPAIR` | no | base58 secret key (prefer the keyfile) |

With neither key variable set the rail is **verify-only**, which is the correct
posture for a server that never spends. Key material is never logged, never
serialized into a receipt, and never written to the database; error messages
never quote it.

**Misconfiguration is fatal.** An unknown `PAYMENT_RAIL`, `PAYMENT_RAIL=solana`
on a binary built without `--features solana`, a missing RPC URL or cluster, an
unparseable mint, an unparseable stewards address, or a stewards override on
mainnet all **panic at startup**. The process must not silently fall back to the
mock rail: the mock signs receipts for free, so a production fallback would hand
out every paid item, paid room and hosted server for nothing.

The one case that is deliberately **not** fatal is *nothing configured*: a build
with no compiled-in stewards address boots fine and simply cannot pay a stewards
leg. Refusing to start over an absent voluntary contribution would be fail-closed
in the wrong place.

> ⚠️ **`SOLANA_CLUSTER=mainnet-beta` moves real money.** Real USDC leaves real
> wallets and cannot be reversed. Start on devnet.

### Testing

Unit tests are **offline**: the JSON-RPC is behind the `SolanaRpc` trait and CI
runs against a scripted fake. They cover:

* **the split math** — legs summing exactly; legs that do not sum (overflow)
  refused; zero-leg, single-leg, three-leg and eight-leg splits; a leg exactly at
  the dust floor paid and one below it skipped; a dust-only split refused rather
  than settled free; one wallet appearing in two legs; too many legs refused
  rather than truncated;
* **the stewards destination** — a `Stewards` leg to any other wallet refused; a
  `Stewards` leg with nothing compiled in refused rather than dropped; a receipt
  claiming a stewards leg to a foreign wallet denied; the mainnet override fatal
  and the devnet override honoured;
* **distribution binding** — a receipt bound to one distribution failing against
  another of the same total, a forged role tag, edited amounts, and a rail proof
  disagreeing with the payouts;
* **every earlier rejection** — wrong recipient, wrong amount, wrong mint (both
  on chain and claimed), unconfirmed, failed transaction, missing `meta.err`,
  insufficient commitment, wrong buyer, buyer not a signer, wrong item binding,
  replay of a valid receipt against another item, chain memo binding a different
  item, missing memo, unaccounted party gaining *or* losing the mint, missing
  binding, tampered signature, tampered stewards amount, unparseable proof, and
  RPC error.

```sh
cd magnetite-solana-rail && cargo test
```

`patala-core` / `patala-solana` are **git dependencies pinned to a rev**, not
`path = "../../patala"` deps, so this needs no sibling checkout — only the rev in
`Cargo.lock` (cached under `~/.cargo/git`, or fetched once). The
`../../patala` requirement this page used to state is no longer true.

There is **no live-validator test in this crate.** (`patala-solana` has an
`#[ignore]`d `live_rpc` test of its own; that one exercises patala's RPC client,
not magnetite's split or its transaction construction.)

**Honest status: nothing here has ever touched a chain.** No payment has settled
through this rail. The verification path is fully covered against a scripted
RPC, and the transaction *construction* path — message serialization,
associated-token-account derivation, the multi-leg instruction list, the
1232-byte size check — is covered by unit tests but has **never been run against
a validator**: not devnet, not a local `solana-test-validator`, not mainnet. The
multi-leg construction is newer still and no more exercised than the rest of it.
Nothing on this page is chain-verified. Do not point it at mainnet before someone
completes a devnet round-trip (`ALIGNMENT.md` §7, Phase 3 item 12).

## There is no protocol fee

There was a `protocol_fee_bps` parameter, defaulted to 0 and read from
`PROTOCOL_FEE_BPS`. It is gone, and it is not coming back under another name: a
system with no central server cannot enforce a mandatory fee, so calling one a
"protocol fee" claimed an authority that does not exist.

What replaces it is a voluntary `Role::Stewards` leg, whose rate belongs to
whoever's money it is and whose destination comes from the signed release. Today
that rate is **0** in every code path, because the signed package manifest that
would carry a developer's declared rate is not built yet
(`ALIGNMENT.md` §7, Phase 1 item 2). When it lands, the rate is read from *the
manifest* — not from the environment.

## Points and XP are not money

Platform points/XP ledgers stay **off-chain**, as signed per-game ledgers.
They are not tokenized by default and are not part of the `PaymentRail` at
all — they're a game-scoped scoring system, not a currency.

## Development and CI

The default `MockPaymentRail` issues deterministic, signed receipts with no
chain involved, so `magnetite dev`, tests, and CI all run fully offline —
nothing about developing or testing a game requires a wallet, an RPC
endpoint, or real funds.
