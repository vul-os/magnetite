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
.mg-bar{display:flex;align-items:center;gap:6px;padding:8px 13px;border-bottom:1px solid var(--dv-border);background:var(--dv-code-bg)}
.mg-bar i{width:8px;height:8px;border-radius:50%;background:var(--dv-border-2)}
.mg-bar span{margin-left:7px;font-family:var(--doc-mono);font-size:.68rem;color:var(--dv-ink-faint)}
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
   [capacity-elastic hosting](./docs.html#hosting-a-server) economically real, not just
   technically possible.
3. **Wager / tournament (optional)** — an `escrow()` is settled by
   `verify_replay` (see [Architecture](./docs.html#architecture)), so the outcome of a
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

<div class="mg-plate">
<div class="mg-bar"><i></i><i></i><i></i><span>magnetite · /wallet</span></div>
<img class="mg-light" src="./shots/wallet-light.png" alt="" loading="lazy" decoding="async" />
<img class="mg-dark" src="./shots/wallet-dark.png" alt="magnetite wallet screen: a linked address and a ledger of signed receipts, labelled non-custodial with the payment rail set to mock" loading="lazy" decoding="async" />
<div class="mg-cap"><b>The player side — receipts, not balances</b>There is no balance field on this screen because the platform holds none. Each row is a receipt verifiable against the rail’s signing key. The card names its own rail: <code>RAIL: MOCK</code>.</div>
</div>

<div class="mg-plate">
<div class="mg-bar"><i></i><i></i><i></i><span>magnetite · /developers/revenue</span></div>
<img class="mg-light" src="./shots/earnings-light.png" alt="" loading="lazy" decoding="async" />
<img class="mg-dark" src="./shots/earnings-dark.png" alt="magnetite developer revenue screen: settled USDC received, signed receipt count and a zero-basis-point protocol fee, stating there is no custodial balance or payout queue" loading="lazy" decoding="async" />
<div class="mg-cap"><b>The developer side — nothing to withdraw</b>A sale settles wallet-to-wallet at checkout, so the screen reports what already arrived rather than what is owed. The fiat balance, payout queue and bank details this replaced are deleted, not hidden. Figures are fixture data on the mock rail.</div>
</div>

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
