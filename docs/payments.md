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
    async fn open_channel(&self, peer: &PubKey) -> Channel;
    async fn escrow(&self, terms: WagerTerms) -> Escrow;
    fn verify_receipt(&self, r: &Receipt) -> bool;
}
```

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
   technically possible.
3. **Wager / tournament (optional)** — an `escrow()` is settled by
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

## Chain choices

The default target is a stablecoin (USDC) on an L2 (Base or Arbitrum), with
payment channels layered on top for micro-transactions so per-join or
per-message costs don't hit the chain individually. Solana is also a
reasonable target since its Ed25519-native keys let a player's identity key
double as their wallet key. On-chain state is kept deliberately minimal, and
the rail is configurable — nothing about the game runtime or scheduler needs
to know which chain is in use.

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
