# Payments

**Non-custodial crypto. No fiat, no balances, no payouts, no custody.**

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
| `SolanaPaymentRail` | `solana` (needs `--features solana`) | real SPL USDC transfers, chain-verified receipts | **yes, on mainnet** |

## Three money flows

1. **Item / DLC purchase** — a `checkout()` call splits payment atomically
   between the developer and, optionally, the hosting operator
   (`PaymentSplit { developer, operator, protocol_fee_bps }`). The
   entitlement a player owns *is* a signed receipt keyed
   `(buyer pubkey, game hash, item)` — the node checks the receipt to grant
   access, not a database row it owns.
2. **Hosting fee** — the incentive to bring a big server. An operator gets
   paid per-seat or per-hour over a payment channel (`open_channel`), so
   joining a match doesn't cost on-chain gas per player. This is what makes
   [capacity-elastic hosting](hosting-a-server.md) economically real, not just
   technically possible. **Mock rail only today** — the Solana rail has no
   channel program and refuses the call (see below).
3. **Wager / tournament (optional)** — **mock rail only today.** An `escrow()` is settled by
   `verify_replay` (see [Architecture](architecture.md)), so the outcome of a
   wagered match is provable from the replay log, not adjudicated by a
   platform.

## What this replaces

The old model ran fiat balances through Paystack (deposits, subscriptions) and
paid developers out through Wise, with a platform-held 70/30 split and a
`ZAR → USD` conversion step. All of that — Paystack, Wise, `wallet_balances`,
`wallet_transactions`, `developer_balances`, the payout/payout-requests
split-brain — is cut. Custody was only ever needed because the platform held
the money in between; a non-custodial rail removes the reason to hold it at
all.

## The Solana / USDC rail

Solana is the first real rail: its Ed25519-native keys let a player's identity
key double as their wallet key, so the seam needs no key-mapping table.

It lives in its own crate, `magnetite-solana-rail` (not a `magnetite-seams`
feature — `magnetite-seams` itself has zero dependency, even optional, on the
sibling `patala` repo, so its own `cargo build`/`cargo test` never touch it).
`backend` compiles it in **only** with `--features solana` and selects it
**only** by `PAYMENT_RAIL=solana`. With the feature off the default build
pulls in no chain dependencies, opens no sockets, and every test runs offline.

### What is real

* **`checkout_for_item` builds ONE transaction** containing one SPL
  `TransferChecked` instruction per party — developer, optional operator,
  optional protocol fee — plus an SPL Memo carrying the `(buyer, item)` binding.
  Because it is a single transaction, the split is atomic *by construction*:
  Solana lands every leg or none. **No custom on-chain program is involved.**
* **`verify_receipt` / `verify_receipt_for_item` re-read the chain.** The
  receipt is treated purely as a *claim*.
* **Money math is integer-only.** USDC has 6 decimals; every amount is a count
  of smallest units (micro-USDC). No floats appear anywhere in the money path,
  and the rail refuses to produce a plan whose parts do not sum exactly to the
  total. The fee is `subtotal * bps / 10_000` with truncating integer division.

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
3. the binding reference equals `blake3("magnetite-pay-v1" ‖ buyer ‖ item)` —
   so a receipt cannot be re-pointed at another item by editing a field;
4. the bound item is the item the *caller* is asking about;
5. payouts sum exactly to the total, and the rail signature is intact;
6. the transaction is known to the cluster **at the configured commitment**
   (`confirmed` or `finalized`; `processed` is rejected outright because it can
   be rolled back);
7. `meta.err` is null — a landed-but-failed transaction moved nothing;
8. the buyer appears in `accountKeys` as a **signer**;
9. the on-chain memo is exactly the derived binding;
10. net token-balance deltas for the configured mint are exactly `-total` for
    the buyer and `+amount` for each claimed recipient, with **no unaccounted
    party** gaining or losing that mint in the transaction.

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
| `SOLANA_FEE_WALLET` | only if `PROTOCOL_FEE_BPS > 0` | base58 fee destination |
| `SOLANA_KEYPAIR_PATH` | no | solana-CLI JSON keyfile — **`chmod 600`, owned by the service user** |
| `SOLANA_KEYPAIR` | no | base58 secret key (prefer the keyfile) |

With neither key variable set the rail is **verify-only**, which is the correct
posture for a server that never spends. Key material is never logged, never
serialized into a receipt, and never written to the database; error messages
never quote it.

**Misconfiguration is fatal.** An unknown `PAYMENT_RAIL`, `PAYMENT_RAIL=solana`
on a binary built without `--features solana`, a missing RPC URL or cluster, an
unparseable mint, or a fee with no fee wallet all **panic at startup**. The
process must not silently fall back to the mock rail: the mock signs receipts
for free, so a production fallback would hand out every paid item, paid room and
hosted server for nothing.

> ⚠️ **`SOLANA_CLUSTER=mainnet-beta` moves real money.** Real USDC leaves real
> wallets and cannot be reversed. Start on devnet.

### Testing

Unit tests are **offline**: the JSON-RPC is behind the `SolanaRpc` trait and CI
runs against a scripted fake. They cover the split math (zero and non-zero fee,
exact sums, truncation), acceptance of a good transaction, and rejection of each
of: wrong recipient, wrong amount, wrong mint (both on chain and claimed),
unconfirmed, failed transaction, insufficient commitment, wrong buyer, buyer
not a signer, wrong item binding, replay of a valid receipt against another
item, chain memo binding a different item, missing memo, unaccounted extra
recipient, missing binding, tampered signature, and RPC error.

```sh
cd magnetite-solana-rail && cargo test   # requires ../../patala checked out
```

An opt-in live test is `#[ignore]`d and additionally gated on an env var:

```sh
solana-test-validator -r &          # or point at devnet
cd magnetite-solana-rail && MAGNETITE_SOLANA_LIVE_RPC=http://127.0.0.1:8899 \
  cargo test live_rpc -- --ignored --nocapture
```

**Honest status: this rail has been exercised offline only.** The verification
path is fully covered against a scripted RPC; the transaction *construction*
path (message serialization, associated-token-account derivation) is covered by
unit tests but has **not** been run against devnet, a local validator, or
mainnet. Do not point it at mainnet until someone has completed a devnet
round-trip.

## Protocol fee

`checkout()` takes a `protocol_fee_bps` parameter. The default is **0** —
whether and how much of a platform fee to take is a governance decision, not
a hard-coded constant.

## Points and XP are not money

Platform points/XP ledgers stay **off-chain**, as signed per-game ledgers.
They are not tokenized by default and are not part of the `PaymentRail` at
all — they're a game-scoped scoring system, not a currency.

## Development and CI

The default `MockPaymentRail` issues deterministic, signed receipts with no
chain involved, so `magnetite dev`, tests, and CI all run fully offline —
nothing about developing or testing a game requires a wallet, an RPC
endpoint, or real funds.
