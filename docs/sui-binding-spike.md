# Sui item binding — research spike

> **Status: research spike. Nothing here is implemented.** No rail crate was
> created, `magnetite-seams` was not touched, and no code in this repo changed.
> This document answers one question — *how does a Sui payment carry a
> tamper-evident, third-party-verifiable item binding, given that Sui has no
> memo primitive* — and it is the resolution of the "one genuine risk" recorded
> in `ALIGNMENT.md` §4 under **DECISION: Sui, not Solana**.
>
> Every Sui capability asserted below is cited to a primary source: Sui's own
> docs, the `MystenLabs/sui` tree at `main`, the checked-in OpenRPC spec, or
> Circle's documentation. Where a source is a search-result summary rather than
> a page I read, or where I could not establish something at all, it says so.
> Research date **2026-07-30**; `MystenLabs/sui` `main`, `MAX_PROTOCOL_VERSION = 132`.

---

## 1. Answer first

**The binding goes in a dedicated `CallArg::Pure` input on the payment PTB.**
That is Sui's structural equivalent of the SPL Memo instruction, and it is
better than the memo in one respect that matters: a pure input is part of
`TransactionData`, so it is covered by the **transaction digest**, which means a
verifier can check the bytes against the digest it looked up rather than
trusting what an RPC told it.

No Move package. No custom on-chain program. The
"atomic-by-construction, nothing deployed" property that `ALIGNMENT.md` §4 calls
the economic model's foundation survives intact.

Concretely, one input carrying:

```
b"MGNT-PAY-1" || blake3("magnetite-pay-v1" || buyer_pubkey || len(item) || item)
```

— a 10-byte ASCII domain tag plus the existing 32-byte
`binding_reference()` from `magnetite-solana-rail/src/lib.rs:174`, BCS-encoded
as a `vector<u8>` pure argument. The derivation function does not change at all.

**The headline for `ALIGNMENT.md`: the memo risk is smaller than §4 feared, and
the switch is *not* materially more expensive on that axis.** Two *other* costs
are larger than §4 assumes, and they are in §9 — Sui's JSON-RPC is being
switched off *this week*, and Sui full nodes prune transaction history after a
recommended two epochs (~48h), which collides head-on with "the receipt is the
entitlement, re-verified against the chain forever".

---

## 2. The property that matters: can a non-sender verify it?

Yes, and cryptographically rather than by trust.

### 2.1 What is inside a Sui transaction

`TransactionData` is versioned; `V1` is
([`crates/sui-types/src/transaction.rs:2388`](https://github.com/MystenLabs/sui/blob/main/crates/sui-types/src/transaction.rs)):

```rust
pub struct TransactionDataV1 {
    pub kind: TransactionKind,      // ProgrammableTransaction { inputs, commands }
    pub sender: SuiAddress,
    pub gas_data: GasData,          // { payment: Vec<ObjectRef>, owner, price, budget }
    pub expiration: TransactionExpiration,
}
```

and a PTB is (same file, `:979`):

```rust
pub struct ProgrammableTransaction {
    pub inputs: Vec<CallArg>,       // CallArg::Pure(Vec<u8>) | Object(..) | FundsWithdrawal(..)
    pub commands: Vec<Command>,
}
```

So the pure inputs are *in the signed transaction body*, sitting beside the
`SplitCoins`/`TransferObjects` commands that move the money. There is no way to
have one without the other: the doc comment on `ProgrammableTransaction::commands`
states "A failure in any command will result in the failure of the entire
transaction" (`transaction.rs:979-986`), and the whole struct is one signed unit.

### 2.2 The digest commits to it

`TransactionData::digest()` is `TransactionDigest::new(default_hash(self))`
(`transaction.rs:2847`), and `default_hash` is Blake2b-256 over the `Signable`
encoding (`crates/sui-types/src/crypto.rs:1566`, `:81`). The blanket `Signable`
impl for `BcsSignable` types — and `TransactionData` is one
(`crypto.rs:1510`) — writes the Rust type name and `::` *before* the BCS bytes
(`crypto.rs:1521-1532`):

```
TransactionDigest = Blake2b256( b"TransactionData::" || bcs(TransactionData) )
```

That prefix is easy to miss and would silently break a reimplementation, so it
is spelled out here. Note the digest covers `gas_data` — payment object refs
*including their versions and digests*, owner, price and budget — which is what
kills candidate 2 in §4.

### 2.3 How a third party reads it back

Two independent routes, and the rail should use both:

| route | field | source |
|---|---|---|
| **decoded** | `Input.pure` — "A move value serialized as BCS" | gRPC `sui/rpc/v2/input.proto`, per [Full Node gRPC Message Definitions](https://docs.sui.io/references/fullnode-protocol-messages) |
| **raw, self-checking** | `Transaction.bcs` — "This `Transaction` serialized as BCS" | same page, `sui/rpc/v2/transaction.proto` |

The raw route is the strong one: fetch the bytes, BCS-decode them yourself,
recompute `Blake2b256(b"TransactionData::" || bytes)`, and require it to equal
the digest you asked for. At that point the RPC is no longer trusted for
anything about the transaction body — it is only trusted for *existence and
finality*. Sui's `ExecutedTransaction` also carries `signatures`, `effects`,
`checkpoint`, `timestamp` and `balance_changes` (same page), so the buyer's own
Ed25519 signature over `intent || bcs(TransactionData)` can be checked locally
too.

On the deprecated JSON-RPC the equivalents are `rawTransaction` — described in
the checked-in OpenRPC spec as "BCS encoded [SenderSignedData] that includes
input object references"
([`crates/sui-open-rpc/spec/openrpc.json`](https://github.com/MystenLabs/sui/blob/main/crates/sui-open-rpc/spec/openrpc.json),
`TransactionBlockResponse.rawTransaction`) — and `showInput`, which renders
each pure input as `{ "type": "pure", "value": …, "valueType": string|null }`
(same spec, `SuiCallArg`).

**Answer: yes.** A verifier who never saw the transaction being built, holding
only `(digest, buyer_pubkey, item)`, can obtain the bytes, prove they are the
bytes of that digest, and read the binding out of them. Nothing about the
binding is sender-private.

---

## 3. Can an *unused* pure input exist? Yes — and there is a fallback if that ever changes

This is the load-bearing detail, because a binding input is not an argument to
anything.

The PTB docs state it directly: at the end of execution "Remaining pure input
values are dropped (all permissible types have `copy` and `drop`)"
([PTB concepts](https://docs.sui.io/concepts/transactions/prog-txn-blocks)).

The execution source agrees, and shows *why*: pure inputs are typed by
inference from their use site. `Context::resolve_location` only registers a
`PureInput` (and only then attaches a `BytesConstraint` and runs the
permissible-type check) when some command actually resolves that input
([`typing/translate.rs:283-302`](https://github.com/MystenLabs/sui/blob/main/sui-execution/latest/sui-adapter/src/static_programmable_transactions/typing/translate.rs);
checks in [`typing/verify/input_arguments.rs`](https://github.com/MystenLabs/sui/blob/main/sui-execution/latest/sui-adapter/src/static_programmable_transactions/typing/verify/input_arguments.rs)).
An input nobody references is never typed and never checked. Pre-execution
`validity_check` bounds sizes and counts but does not require inputs to be used
(`transaction.rs:1621-1660`, `:833`).

**Fallback if that behaviour ever tightens.** `sui::hash::blake2b256` is
`public native fun blake2b256(data: &vector<u8>): vector<u8>`
([`sui-framework/sources/crypto/hash.move`](https://github.com/MystenLabs/sui/blob/main/crates/sui-framework/packages/sui-framework/sources/crypto/hash.move)).
It has no type parameters, so it is not subject to the private-generics rule
(§4.4), it is `public`, so a PTB may call it
("PTBs can call any `public` function and any `entry` function"), it takes the
input by reference, and its `vector<u8>` return has `drop` so the result can be
discarded. One extra command, `0x2` only, still nothing deployed. A
`MakeMoveVec(Some(vector<u8>), [binding])` whose result is dropped works the
same way.

So the binding can be made a *used* input using nothing but the canonical
framework — the "no custom program" property does not depend on the unused-input
behaviour holding. That is what makes this recommendation robust rather than
clever.

**Not established, and it needs a devnet round-trip:** that a PTB with an unused
pure input is accepted end-to-end by a live validator and that the input comes
back through gRPC `Input.pure`. Everything above is source and doc reading. It
is a ten-minute experiment and it must be run before the rail is written; the
outcome only decides whether to add the one `blake2b256` command.

---

## 4. Candidate evaluation

### 4.1 A small Move package — rejected, and the cost is concrete

What it would buy: an `emit`ed event, indexed and queryable, and the option of
on-chain enforcement.

What it costs, itemised:

- **A deployed, upgradeable object with an owner.** Publishing yields a package
  and an `UpgradeCap`. Whoever holds that cap can publish a new version. So the
  package becomes a **trusted component with a key-holder** — in a project whose
  stewards address is deliberately compile-time-pinned so that "a fork changing
  it is expected and visible; a host silently redirecting it is not possible"
  (`ALIGNMENT.md` §4). A live upgrade authority is exactly the shape of thing
  that design refuses. Burning the cap fixes custody at the price of never
  fixing a bug.
- **Audit surface on the money path.** Today the money path is `SplitCoins` +
  `TransferObjects`: framework code, audited by Mysten and everyone else. A
  package puts magnetite-authored Move between the buyer's coins and the
  recipients. Even a package that only emits an event must be *proven* to only
  emit an event, forever, across upgrades.
- **Per-network deployment and pinning.** A package ID per network, in config,
  version-pinned, plus the release-verification story for "which package is the
  real one" — a second trust anchor beside the USDC coin type.
- **It buys nothing the pure input does not already give.** The event would be a
  *derived* fact about a transaction whose bytes already contain the binding. It
  adds queryability, not integrity.

Rejected. Not because a Move package is hard, but because it converts a
zero-trusted-component design into a one-trusted-component design in exchange
for indexing convenience.

### 4.2 Deterministic transaction reconstruction — not viable, and the reason is exact

The idea: make the digest itself the binding by having the verifier rebuild the
transaction byte-for-byte.

It fails on `GasData`. The digest covers
`GasData { payment: Vec<ObjectRef>, owner, price, budget }` (`transaction.rs:2302`,
`:2388`), and `ObjectRef` is `(ObjectID, SequenceNumber, ObjectDigest)`. To
reproduce the bytes a verifier would have to know:

- **which** SUI coin objects the sender chose to pay gas with — unbounded choice
  among the sender's coins;
- **their exact versions and content digests at signing time** — these change
  with every unrelated transaction the sender makes;
- the **gas price** the sender picked (only bounded below by the epoch reference
  gas price and above by `max_gas_price`, `transaction.rs:3331`, `:3428`);
- the **gas budget** (any value between `min_transaction_cost` and
  `max_gas_budget`, `transaction.rs:3436-3462`);
- the input coin object refs, and — under sponsorship — a different `GasData.owner`.

Each is a free choice by the sender. The verifier was not the sender.
`SequenceNumber`/`ObjectDigest` alone make it hopeless: they are not derivable
from anything the verifier holds. Nothing here is fixable by convention; you
would be asking a third party to guess a sender's wallet state at a past
instant.

`ALIGNMENT.md` calls this "elegant, but fragile". Sharper: **it is not fragile,
it is impossible for a third-party verifier**, and it should be struck from the
candidate list rather than left as a maybe. (It would work for the *sender*, who
kept the bytes — which is precisely the worthless case.)

### 4.3 An off-chain binding — what it would actually cost

For completeness, since §4 lists it. A rail-signed statement "digest D pays for
item I", stored beside the receipt.

- **Check 3** (binding reference equals the derived hash) survives in form and
  dies in substance: it becomes a check that magnetite's own signature is
  self-consistent, not that the chain says so.
- **Check 9** (the on-chain memo is exactly the derived binding) is **lost
  outright.** There is no on-chain artefact to compare against.
- Consequence: whoever holds the rail signing key can bind any transaction to
  any item, and *nobody can detect it from the chain*. Two receipts naming
  different items for one digest are both valid. Worse, the entitlement now
  depends on a magnetite-held key, which reintroduces a trusted party into a
  design whose entire premise is that there isn't one — and it is the same key
  that `magnetite-solana-rail` is careful to document as "a self-consistency
  marker, **NOT** the security boundary" (`lib.rs:206-209`).

Not recommended, and it is not needed, because §1 works.

### 4.4 Sui-specific options with no Solana analogue

**Events from a PTB — definitively impossible.** `ALIGNMENT.md` says "events can
only be emitted from Move calls"; the precise rule is stronger. `sui::event::emit`
carries an *internal* type parameter: `FUNCTIONS_TO_CHECK` lists
`(SUI_EVENT_EMIT_EVENT, &[true])` and `(SUI_EVENT_EMIT_AUTHENTICATED, &[true])`
([`sui-verifier/src/private_generics_verifier_v2.rs:122-145`](https://github.com/MystenLabs/sui/blob/main/sui-execution/latest/sui-verifier/src/private_generics_verifier_v2.rs)),
and the PTB type-checker rejects any such call with a comment that states the
reason outright — "If we find an internal type parameter, the call is
automatically invalid — since we are not in a module and cannot define any types
to satisfy the internal constraint"
([`typing/verify/move_functions.rs`, `check_private_generics_v2`](https://github.com/MystenLabs/sui/blob/main/sui-execution/latest/sui-adapter/src/static_programmable_transactions/typing/verify/move_functions.rs)).
The docs say the same in prose. So: **no events without a package, confirmed at
source level.** Also note primitives are not emittable as events even from a
module, so a `vector<u8>` memo event would need a wrapper struct anyway.

**`TransferObjects` recipient as a carrier — works, and costs the wrong thing.**
A recipient is a bare 32-byte address with no existence requirement, so
`SplitCoins(gas, [0])` + `TransferObjects([zero_coin], H)` where `H` is the
binding hash *does* put 32 bytes on chain, and more durably than a transaction:
the created object stays in the live object set and records its
`previous_transaction`, so it is discoverable long after transaction history is
pruned (§9.2). Rejected anyway:

- storage is charged at "100 units per byte" with 99% rebatable on deletion
  ([Gas fees](https://docs.sui.io/concepts/tokenomics/gas-in-sui)) — so it is a
  real, per-purchase, non-recoverable-in-practice deposit, which is
  *the same shape as the Solana ATA rent that motivated the whole dust floor*
  and that the Sui decision was supposed to delete;
- it burns a coin to an address nobody controls, which reads like an accident;
- it needs a second lookup (`getOwnedObjects(H)`) to be useful, so verification
  gets more moving parts, not fewer.

Worth keeping in the back pocket *only* if the pruning problem in §9.2 forces a
durable on-chain marker. Then it becomes interesting rather than silly. Note
that the amount deposited would still be quantified: I did not compute the SUI
cost of one coin object.

**Dynamic fields — no.** `dynamic_field::add` requires `&mut UID`
([`dynamic_field.move:38`](https://github.com/MystenLabs/sui/blob/main/crates/sui-framework/packages/sui-framework/sources/dynamic_field.move),
with the source comment "we use `&mut UID` in several spots for access
control"), and `sui::object` exposes no public function returning `&mut UID` —
every `UID` accessor there is by immutable reference or `public(package)`
([`object.move`](https://github.com/MystenLabs/sui/blob/main/crates/sui-framework/packages/sui-framework/sources/object.move)).
I explored a `object::new()` → `dynamic_field::add` → `object::delete` chain,
and **could not establish that it works** (a fresh `UID` has no `drop`, must be
consumed, and I did not verify whether deleting a `UID` that has children is
permitted). It is an abuse of framework internals for no gain over a pure input;
recorded as explored, not recommended, not verified.

**Coin object naming — no such thing.** Coins carry no label; `CoinMetadata` is
per-currency, not per-coin.

**`TransactionExpiration::ValidDuring { nonce: u32 }` — too small.** 32 bits
(`transaction.rs:2334`).

**Sponsored transactions — a genuine bonus, unrelated to binding.**
`GasData.owner` may differ from `sender`; both sign; the user needs no SUI
([Sponsored transactions](https://docs.sui.io/concepts/transactions/sponsored-transactions)).
A buyer can therefore pay in USDC without holding the gas token, which Solana's
fee-payer arrangement also allows but which is worth writing down because it
changes onboarding. The verifier must keep checking `sender`, not the gas owner.

---

## 5. The ten fail-closed checks

Source: `site/docs/payments.md` "Fail-closed verification", implemented at
`magnetite-solana-rail/src/lib.rs:395-465`. Verdicts are for the §1
recommendation.

| # | check (Solana wording) | verdict | Sui form |
|---|---|---|---|
| 1 | receipt carries a chain binding, and the chain is `solana` | **modified** (label only) | chain is `sui`; also require `TransactionKind` = `ProgrammableTransaction`, so a system transaction can never be presented as a purchase |
| 2 | claimed mint equals the configured USDC mint | **modified** (mechanism) | claimed coin **type** equals the configured `pkg::usdc::USDC` (§7). A full type string, not a 32-byte mint; compare canonically, and pin per network |
| 3 | binding reference equals `blake3("magnetite-pay-v1" ‖ buyer ‖ item)` | **preserved, unchanged** | pure local arithmetic; `binding_reference()` ports verbatim |
| 4 | the bound item is the item the caller is asking about | **preserved, unchanged** | unchanged |
| 5 | payouts sum exactly to the total, rail signature intact | **preserved, unchanged** | unchanged |
| 6 | transaction known at the configured commitment (`processed` rejected) | **modified** (mechanism; not weaker) | Sui has no commitment levels. Require `checkpoint` present — the OpenRPC spec defines it as "The checkpoint number when this transaction was included and hence finalized", and "if a transaction appears in a certified checkpoint … it is considered finalized" ([lifecycle](https://docs.sui.io/develop/transactions/transaction-lifecycle)). Refusing an executed-but-not-yet-checkpointed transaction is the analogue of refusing `processed`. **The mapping is a judgement call and is called out as one.** |
| 7 | `meta.err` is null | **preserved** (mechanism) | `effects.status == success` (`ExecutionStatus` is `success` \| `failure{error}`, OpenRPC spec) |
| 8 | the buyer appears in `accountKeys` as a signer | **modified, with a narrowing** | `Transaction.sender == Blake2b256(0x00 ‖ buyer_pubkey)` (`base_types.rs:916-935`), **and** verify the buyer's Ed25519 signature from `ExecutedTransaction.signatures` over `intent ‖ bcs(TransactionData)` locally. *Stronger* than the Solana check, which trusts the RPC's `accountKeys`. **Narrowing:** only single-key Ed25519 senders are supportable — a multisig or zkLogin buyer's address is not derivable from one pubkey, so such buyers fail closed. Consistent with `PubKey = [u8; 32]`, but it is a real restriction and belongs in the rail docs |
| 9 | **the on-chain memo is exactly the derived binding** | **modified** (mechanism; strengthened) | **exactly one** pure input whose bytes are `b"MGNT-PAY-1" ‖ binding`; zero or two or more ⇒ deny. Plus: recompute the digest from `Transaction.bcs` and require it to equal the queried digest, so the bytes are self-proving rather than RPC-asserted |
| 10 | net token-balance deltas exactly `-total` / `+amount`, no unaccounted party | **preserved** (mechanism) | `balance_changes`: `{owner, coinType, amount}` with "negative amount means spending coin value" (OpenRPC `BalanceChange`). Filter to the configured coin type — the buyer's SUI delta is gas and is ignored; require `AddressOwner`; aggregate by owner before comparing, exactly as `ALIGNMENT.md` §4 already requires |

**Nothing is lost.** Six checks are unchanged or mechanically translated, three
are strengthened (8, 9, and the digest self-check), one (6) is a defensible
remapping onto a different finality model and is flagged as a judgement rather
than an equivalence. The single substantive narrowing is Ed25519-only senders in
check 8.

Two checks should be **added**, both with no Solana counterpart:

11. **digest self-check** — `Blake2b256(b"TransactionData::" ‖ bcs(TransactionData))`
    equals the digest looked up. Without this, `Input.pure` is an RPC claim.
12. **no `Publish` or `Upgrade` command** in the PTB. A purchase transaction has
    no business deploying code, and refusing it keeps "no custom on-chain
    program" true of the *transaction*, not just of the rail's intent.

---

## 6. Real numbers: PTB limits

From [`crates/sui-protocol-config/src/lib.rs`](https://github.com/MystenLabs/sui/blob/main/crates/sui-protocol-config/src/lib.rs)
at `main` (`MAX_PROTOCOL_VERSION = 132`), with the enforcing checks in
`crates/sui-types/src/transaction.rs`. Several checks are strict `<`, so the
usable maximum is one less than the constant — a detail worth having right in a
`TooManyLegs` guard.

| limit | value | usable | enforced at |
|---|---|---|---|
| `max_tx_size_bytes` | 131 072 (128 KiB) | 128 KiB | never overridden after v1 |
| `max_programmable_tx_commands` | 1 024 | **1 023** (`commands.len() < …`) | `transaction.rs:1624` |
| `max_arguments` (per command) | 512 | **511** (`args.len() < …`) | `transaction.rs:1253`, `:1475` |
| `max_pure_argument_size` | 16 384 | **16 383** (`p.len() < …`) | `transaction.rs:837` |
| `max_input_objects` | 2 048 | 2 048 (objects + receiving only; pure inputs excluded) | `transaction.rs:1632` |
| `max_type_arguments` | 16 | 16 | `transaction.rs:242` |
| `max_tx_gas` (budget ceiling) | 50 000 000 000 000 MIST = **50 000 SUI** (since protocol v72; was 50 SUI from v3) | — | `transaction.rs:3436` |
| `max_gas_price` | 50 000 000 000 MIST = 50 SUI (since v72) | — | `transaction.rs:3428` |
| minimum budget | `base_tx_cost_fixed × gas_price` | — | `transaction.rs:3456` (`GasBudgetTooLow`) |
| gas price floor | ≥ the epoch reference gas price | — | `transaction.rs:3331` |

**The leg-count cap, as a number.** The natural shape is one `SplitCoins` with
N amounts, then one `TransferObjects` per recipient (a single `TransferObjects`
may send many objects but only to one address):

- `SplitCoins` amounts ≤ 511 → **≤ 511 legs**;
- commands = 1 + N ≤ 1023 → N ≤ 1022, not binding;
- size: each leg is a few dozen bytes against 128 KiB, not binding.

So **`TooManyLegs` fires at 511 legs**, versus a handful on Solana's 1232-byte
transaction. `ALIGNMENT.md`'s "much less binding" is right; the number is 511,
and it should be written as 511 rather than "larger". Above ~50 legs, gas rather
than the protocol limit will be the practical wall, and that is unquantified
here.

Two caveats. Sui's protocol config is **versioned and per-network**: these are
the `main`-branch values, and I did not verify which protocol version Mainnet is
running. The rail should **read the limits from the chain** (gRPC/JSON-RPC
`getProtocolConfig`) rather than compile in 511 — the wrong hardcoded constant
is a silent fail-open in the direction of building a transaction that gets
rejected, or a silent fail-closed in the direction of refusing a legal split.
And gas must be budgeted in **SUI/MIST**, never in USDC — a second unit in the
money path, in a codebase that already has one live unit bug
(`ALIGNMENT.md` §4, cents vs micro-USDC). Declare it explicitly.

---

## 7. USDC on Sui: native, and the coin types

**Native, issued by Circle.** Circle states "USDC is native to Sui and can be
found at this contract address" and warns that "Bridged forms of USDC, such as
wUSDC, are not issued by Circle nor supported by Circle Mint"
([Circle — USDC on Sui](https://www.circle.com/multi-chain-usdc/sui)). There is
a Circle migration path from the previously bridged token
([Migration guide](https://www.circle.com/blog/sui-migration-guide),
[launch announcement](https://www.circle.com/blog/now-available-native-usdc-on-sui)).

| network | coin type |
|---|---|
| **Mainnet** | `0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC` |
| **Testnet** | `0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC` |
| Devnet / localnet | **none published.** Must be configured explicitly; a missing value is a startup panic, per the existing misconfiguration-is-fatal rule |

Both from Circle's own docs
([Quickstart: set up and transfer USDC on Sui](https://developers.circle.com/stablecoins/quickstart-setup-transfer-usdc-sui),
corroborated by the page above). A bridged Wormhole wUSDC type also circulates;
I saw it only in a search summary, not in a primary source, so it is not quoted
here — the rail's defence is that it accepts exactly one configured type and
denies everything else, which needs no list of impostors.

**Decimals: not established.** USDC is 6 decimals elsewhere and the
micro-USDC assumption very probably carries over, but I did not find a Circle or
Sui primary source stating the on-chain `decimals` for the Sui coin. Given that
`ALIGNMENT.md` §4 documents a live 10 000× unit bug from exactly this class of
assumption, **the rail must read `CoinMetadata.decimals` for the configured coin
type at startup and refuse to run if it is not what the configuration declares.**
Do not hardcode 6.

---

## 8. Sketch of the transaction and the verification

Not an implementation — enough to argue about, and to hand to whoever writes
`patala-sui`.

**Build** (one PTB, N legs):

```
inputs:
  0: Pure( bcs(vector<u8>) of  b"MGNT-PAY-1" || binding_reference(buyer, item) )   # the binding
  1: Object( ImmOrOwned buyer's Coin<USDC> )
  2: Pure( u64 leg_0_amount )   … k: Pure( u64 leg_{N-1}_amount )
  k+1: Pure( address leg_0_recipient )  …                                          # derived from each PubKey
commands:
  0: SplitCoins( Input(1), [Input(2) … Input(k)] )                 -> N coins
  1: TransferObjects( [Result(0,0)], Input(k+1) )
  …
  N: TransferObjects( [Result(0,N-1)], Input(k+N) )
  # optional, only if unused pure inputs turn out to be rejected:
  #  MoveCall 0x2::hash::blake2b256( &Input(0) )   — result dropped
```

All-or-none by construction. No package. Input 0 is inert and exists only to be
read back.

**Verify** `(digest, buyer_pubkey, item, claimed_legs)`:

1. local: derive the binding; check the claimed item, the sums, the rail
   signature (checks 3, 4, 5) — no network yet;
2. fetch the transaction with effects, signatures, balance changes, checkpoint;
3. decode `Transaction.bcs`; recompute the digest; require equality (check 11);
4. require `ProgrammableTransaction`; require no `Publish`/`Upgrade`
   (checks 1, 12);
5. require exactly one pure input equal to `TAG ‖ binding` (check 9);
6. require `sender == address(buyer_pubkey)` and a valid buyer signature
   (check 8);
7. require `checkpoint` present and `status == success` (checks 6, 7);
8. from `balance_changes`, keep the configured coin type only, aggregate by
   owner, require `-total` for the buyer and `+amount` per claimed recipient with
   no unaccounted party (checks 2, 10);
9. anything unreachable, absent, malformed, or panicking ⇒ `false`.

The offline scripted-fake-RPC test pattern from `magnetite-solana-rail` ports
directly, and the negative-case list in `site/docs/payments.md` ("Testing")
gains: wrong coin type, missing binding input, **two** binding inputs, binding
present but digest mismatch, sender not the buyer, no checkpoint, a `Publish`
command present, and a balance change to an unaccounted address.

---

## 9. Two costs larger than `ALIGNMENT.md` §4 assumes

The memo risk resolves cheaply. These two do not, and both are about *reading*
Sui rather than writing it. Neither is a reason to reverse the decision; both
are reasons the rail is more work than "structural work with a working template
to copy".

### 9.1 JSON-RPC is being switched off this week

> "**JSON-RPC is deprecated**. Migrate to either gRPC or GraphQL RPC before the
> week of July 27, 2026, when JSON-RPC is disabled on Sui Foundation mainnet
> full nodes. Full decommission, including code removal, is planned for
> mid-October 2026."
> — [`docs/content/snippets/json-rpc-deprecation.mdx`](https://github.com/MystenLabs/sui/blob/main/docs/content/snippets/json-rpc-deprecation.mdx)

Today is 2026-07-30: that week is now. The gRPC API is described as "a generally
available, type-safe full node API that **replaces JSON-RPC** on full nodes"
([Accessing data](https://docs.sui.io/develop/accessing-data/data-serving)).

Consequences:

- the rail's chain-reading seam must target **gRPC** (or GraphQL), not
  `sui_getTransactionBlock`. The OpenRPC spec is cited above because it is the
  most precise published description of the *fields*; it is not the API to build
  on;
- Solana's rail is one HTTP JSON-RPC trait with a scripted fake. Sui's is
  protobuf plus generated clients, or GraphQL. The offline-fake test pattern
  still works, but the "thin glue" estimate should absorb an RPC-client
  migration that Sui is performing under us;
- and **RPC providers are mid-migration too** — the deprecation notice tells
  users to ask providers to enable gRPC. Node operators running magnetite need
  an endpoint that supports whichever surface the rail picks. That is an
  operator-facing requirement, and it is new.

### 9.2 Sui full nodes prune transaction history — and entitlements are verified forever

This is the sharper one, and it is a direct collision with the entitlement model
rather than an inconvenience.

- "Sustainable disk usage requires Sui nodes (validators and full nodes) to
  prune the information about historic object versions, as well as **historic
  transactions with the corresponding effects and events**, including old
  checkpoint data."
- "`num-epochs-to-retain-for-checkpoints: X` … The checkpoints, including their
  transactions, effects, and events, are pruned up to X epochs ago. **Setting
  transaction pruning to 2 epochs is recommended.**"
- and, naming the method: direct lookups "require historic data in some cases,
  such as `sui_tryGetPastObject` and **`sui_getTransactionBlock`**".
  — [Managing data](https://docs.sui.io/operators/data-management/managing-data)

An epoch is "about 24 hours" on Mainnet and Testnet
([Epochs](https://docs.sui.io/develop/sui-architecture/epochs)). So a
conformantly-configured full node may not know a transaction that is **~48 hours
old**.

Magnetite verifies a receipt *every time an entitlement is exercised*, fails
closed when it cannot, and never caches — "'Cannot verify' never grants an
entitlement" (`site/docs/payments.md`). Composed with pruning, that reads:
**a player's purchase stops unlocking their item roughly two days after they
bought it**, on any node pointed at an ordinary pruned full node. Fail-closed
does the right thing and the outcome is still a broken product.

Available answers, none free:

1. **GraphQL with an Archival backend.** "GraphQL can route supported historical
   lookups to Archival when the GraphQL operator configures it." Note the
   asymmetry the same page states: "Full node gRPC does not implicitly fall back
   to Archival; gRPC clients must query an Archival Service endpoint directly."
   So §9.1's "use gRPC" and this answer pull in opposite directions and the
   choice has to be made deliberately.
2. **An Archival Service endpoint** queried directly — a second dependency, and
   an operator-facing requirement.
3. **Verify once, then persist a proof.** Store the raw `TransactionData` bytes
   in the receipt at purchase time. Re-verification then re-derives the digest
   and re-reads the binding *locally*, needing the chain only for the facts that
   cannot be self-proved: finality and balance changes. This is attractive —
   §2.2 makes the bytes self-proving — but it is a **genuine reduction**: checks
   6, 7 and 10 would rest on a one-time observation rather than a live read, so a
   node that lies at that instant is believed forever. It must be designed and
   declared, not slid in.
4. **Run a retaining full node** (`num-epochs-to-retain-for-checkpoints`
   unset/large) — pushes disk cost onto operators, and every operator, in a
   design whose premise is ordinary machines.

**Honest framing:** this is not Sui-specific in kind — Solana RPC nodes prune
ledger history too, and the Solana rail has never been run against a chain, so
the problem is latent there as well rather than solved. What is Sui-specific is
that the recommended retention is **two epochs**, the archival path is a
**separate service** with different access depending on which API you chose, and
all of it is documented clearly enough that "we didn't know" is not available.
`ALIGNMENT.md` §4 costs the switch as chain-shaped work on the *writing* side and
does not cost the *reading* side at all. It should.

---

## 10. What I could not establish

Listed because the repo's honest-status culture makes these more useful than a
confident guess.

1. **That a PTB with an entirely unused pure input is accepted by a live
   validator, and that the bytes come back through gRPC `Input.pure`.** Docs and
   source both say yes (§3); it is unexercised. **Highest-value next action:** a
   devnet PTB with one unused pure input, read back, digest recomputed. It
   decides whether the `0x2::hash::blake2b256` no-op command is needed.
2. **The exact framing of gRPC `Transaction.bcs`** — specifically whether it
   includes the `TransactionData` enum variant tag, so that
   `Blake2b256(b"TransactionData::" ‖ bytes)` reproduces the digest without
   re-encoding. Same experiment settles it. Until it does, the digest self-check
   (check 11) is *designed* but not *proven*.
3. **USDC's on-chain `decimals` on Sui**, from a primary source (§7). Read
   `CoinMetadata`; do not hardcode.
4. **The live protocol version on Mainnet and Testnet**, hence which of the
   versioned limits in §6 are actually in force. Read them from the chain.
5. **The gRPC `BalanceChange` message's exact fields.** The
   [message-definitions page](https://docs.sui.io/references/fullnode-protocol-messages)
   confirms `ExecutedTransaction` carries `balance_changes` but did not render
   the message; the field shape quoted in §5 is from the JSON-RPC OpenRPC spec,
   which is authoritative for JSON-RPC and only indicative for gRPC.
6. **The SUI cost of one created coin object** (storage at 100 units/byte, 99%
   rebatable) — so the claim that Sui "deletes" the per-recipient cost that
   drove the dust floor is **not fully true**: the *ATA derivation* complexity is
   deleted, but a per-recipient storage charge remains, smaller and rebatable.
   `ALIGNMENT.md` §4's "That complexity is **deleted, not managed**" overstates
   it slightly and should be softened to name the residue. Quantifying it also
   tells us whether the dust floor survives at all, and at what value.
7. **Whether a `object::new` → `dynamic_field::add` → `object::delete` chain is
   even executable** in a PTB (§4.4). Explored, unresolved, not recommended
   regardless.
8. **Gas cost of a realistic N-leg payment PTB**, hence the practical (as
   opposed to protocol) leg ceiling.

---

## 11. Bottom line for `ALIGNMENT.md` §4

- **The "one genuine risk" is resolved, and cheaply.** The item binding rides in
  a `CallArg::Pure` input; it is covered by the transaction digest; any third
  party can read it and prove it belongs to that digest; **no Move package, no
  custom on-chain program**, and a canonical-framework fallback
  (`0x2::hash::blake2b256`) exists if the unused-input path ever closes. The
  §4 sentence "None is obviously right" can be replaced with an answer.
- **All ten fail-closed checks survive.** Three get stronger, one (finality) is a
  declared remapping, one narrowing (Ed25519-only senders), and two new checks
  are added. Nothing is lost.
- **Strike "deterministic transaction reconstruction" from the candidate list.**
  Gas-coin object refs — including versions and content digests — are inside the
  digest. It is not fragile; it is impossible for anyone but the sender.
- **Strike the off-chain binding too.** It loses check 9 outright and makes the
  entitlement depend on a magnetite-held key.
- **The leg cap is 511**, not "larger than Solana".
- **USDC on Sui is native Circle USDC**, with published Mainnet and Testnet coin
  types and nothing canonical on devnet.
- **Two under-costed items:** Sui's JSON-RPC is being disabled this week, so the
  rail must be built against gRPC or GraphQL; and Sui's recommended
  transaction retention is two epochs (~48h), which is incompatible as-is with
  verifying receipts against the chain forever. The second is the one that needs
  a design decision before the rail is written, and it is a decision about
  magnetite's entitlement model, not about Sui.
