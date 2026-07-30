# Stellar history retention — what degrades, what survives (A26)

**Not a gate** — `patala-stellar`'s `verify` splits into an offline
cryptographic check and a separate online Horizon check, and only the
second depends on retained chain history at all. This document answers the
narrow question A26 actually asks: **if Horizon prunes (or a testnet reset
wipes) history, which of magnetite's guarantees degrade, and which survive?**
It is written against the one real data point available — a payment that
settled today — rather than against retention policy in the abstract.

## The concrete case this is answered against

`patala-stellar`, tx `32663937fe1407f9de3e781effa6ac9f4b1d29340ea63e72f6335a6c91effb89`,
ledger `3882739`, **Stellar testnet**, submitted and read back through
`horizon-testnet.stellar.org` (`patala-stellar/src/tests.rs:1011`, and
`README.md:307` — this is SDF's own public testnet Horizon, not a
third-party or self-hosted instance). `patala-stellar/src/lib.rs:88-99`
records this as the one real settlement to date: single-leg, testnet, a
self-issued `"USDC"`-coded asset (not Circle's own issuer) — mainnet is
"UNVERIFIED AGAINST LIVE" (`lib.rs:117`). Nothing here upgrades that.

## What `verify` actually checks (`patala-stellar/src/lib.rs:60-84`)

Steps 1–6, **entirely offline, no Horizon involved**:

1. Rail/currency match on the receipt.
2. `proof` parses as a `StellarBinding`.
3. Claimed asset (code + issuer) matches the rail's *configured* issuer.
4. `binding.amount_stroops == receipt.amount_minor` (outer receipt and inner
   proof must agree).
5. The memo-hash binding re-derives from `(rail id, source, destination,
   reference)` — a receipt cannot be silently re-pointed.
6. **The whole `Transaction` is rebuilt from the binding's own scalar
   fields, hashed, and the claimed signature is Ed25519-verified against
   the claimed source over that hash** — "a genuine cryptographic guarantee
   checkable without Horizon at all."

Step 7, **online, the only step that touches Horizon**:

7. Horizon is asked for this transaction hash. **Not found ⇒ deny.** Found ⇒
   `successful` must be `true`, and Horizon's returned envelope XDR is
   decoded and compared operation-for-operation against the binding — never
   trusting Horizon's summary fields alone.

Step 8: any RPC *failure* (unreachable, error) at step 7 propagates as `Err`
— an operational failure to check, never an implied "verified" — but a
*clean* 404 ("Horizon has never heard of this hash",
`patala-stellar/src/rpc.rs:173-176`) is not a failure at all: it flows
through `lib.rs:617`, `Ok(None) => return Ok(false)`, i.e. a definite,
fail-closed **not verified**. Pruning and a clean 404 are indistinguishable
to this code path — both look like "Horizon doesn't have it."

## Primary-sourced retention facts

| Fact | Detail | Source |
|---|---|---|
| Self-hosted Horizon default | `HISTORY_RETENTION_COUNT` defaults to **0**, meaning **no purge, ever** — full history kept indefinitely by default. An operator who wants a bounded window sets it explicitly (SDF's own recommendation: `518400` ledgers ≈ 30 days). Reaping runs hourly once a non-zero window is set. | [developers.stellar.org — Configuring (Horizon admin guide)](https://developers.stellar.org/docs/data/apis/horizon/admin-guide/configuring) |
| SDF's own public **mainnet** Horizon (`horizon.stellar.org`) | Truncated to a **rolling one-year window**, effective **August 1, 2024**. Alternatives named for older data: Hubble (public BigQuery dataset of full history), third-party Horizon providers, self-hosting with a custom `HISTORY_RETENTION_COUNT`. | [stellar.org — "SDF's Horizon: Limiting Data to 1 Year"](https://stellar.org/blog/foundation-news/sdf-s-horizon-limiting-data-to-1-year) |
| **Testnet** (`horizon-testnet.stellar.org` — the instance today's tx used) | Not a rolling window at all: the **whole network resets to genesis 2–4 times per year**, announced at least two weeks ahead on the Stellar status dashboard. "Resets clear all ledger entries (accounts, trustlines, offers, smart contract data, etc.), transactions, and historical data **from Stellar Core, Horizon, and the Stellar RPC**." | [developers.stellar.org — Networks](https://developers.stellar.org/docs/networks) |
| Stellar RPC (`stellar-rpc`/Soroban RPC — a **separate**, newer JSON-RPC service; **not** what `patala-stellar` calls) | Default retention window **120,960 ledgers ≈ 7 days**; a private instance may raise it, "we do not recommend values longer than 7 days." `getTransaction` returns `NOT_FOUND` outside the window; recommended fallbacks are self-indexing, a third-party indexer, or Hubble. | [developers.stellar.org — getTransaction (RPC API reference)](https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getTransaction) |

Noted explicitly because it changes which row applies: `patala-stellar`'s
`HorizonRpc` calls classic Horizon's REST `GET /transactions/{hash}`
(`patala-stellar/src/rpc.rs:313,353-354`), **not** the newer Stellar RPC's
`getTransaction`. The relevant retention story for the code that exists today
is Horizon's, and — for the specific tx this document is about — testnet's
full-reset policy, not either of the rolling-window numbers.

## The concrete answer

**Survives any amount of Horizon pruning, including a full testnet reset:**
the offline cryptographic check, steps 1–6. It is reproducible forever from
a copy of the receipt alone; it never issues a Horizon call, so no retention
policy anywhere touches it. This is the literal content of "self-proving
receipt": authenticity of *what was signed* does not live in Horizon's
database and cannot be pruned out of existence.

**Degrades, and eventually disappears, once Horizon no longer holds the
ledger:** confirmation that the signed transaction was actually **submitted
and included** — step 7 — and the operation-for-operation cross-check of
Horizon's envelope against the binding, which only matters for as long as
step 7 still finds anything to cross-check. Once Horizon prunes the ledger
(rolling window) or the network resets (testnet), `verify()` returns a clean
`Ok(false)`. This is a **false negative**, not a false positive: a genuinely
settled payment reads as unverified. It is not a security hole — a forged
receipt still fails steps 1–6 regardless of what Horizon does or doesn't
have, and pruning can only ever remove a "found," never manufacture one — but
it is a real availability loss for anyone who needs to *prove*, after the
fact, that a specific old receipt corresponds to a real on-chain event.

**Sharpest instance: this exact payment.** Tx
`32663937fe1407f9de3e781effa6ac9f4b1d29340ea63e72f6335a6c91effb89` is on
**testnet**. Testnet's story is not "keep for a year" (that's mainnet) — it
is a hard wipe, 2–4 times a year, of Stellar Core, Horizon, *and* Stellar RPC
together, back to genesis. The next scheduled reset — which could be weeks
or a few months away, not a year — will make this tx permanently
unconfirmable via Horizon: not "old," gone. The source account itself will
no longer exist. After that point, re-running
`StellarRail::verify`/`live_testnet_round_trip_settles_a_real_payment`
against `horizon-testnet.stellar.org` for this hash will return `Ok(false)`
forever, and only the raw receipt bytes plus the offline check will still
attest to what was signed on 2026-07-30.

## A live-code finding that sharpens the answer further

The design that is supposed to make this "not a gate" already exists in
`magnetite-seams/src/payment.rs:543-634` (A14): `Settlement::Settled` vs
`Settlement::SignedUnsettled`, exactly so a caller can distinguish "I
re-confirmed this against a chain" from "I only checked the receipt's own
signature and arithmetic; the chain didn't confirm it just now" — and never
conflate the two ("**Never** treat this as `Settled`," `payment.rs:575`).

But checked against the actual rails, **it is not wired up anywhere real
yet**, which the module's own doc comment gets wrong:
`payment.rs:626` claims `magnetite_solana_rail::SolanaPaymentRail` is "the
first one that does" override `verify_receipt_for_item_tiered`. It does not:
`magnetite-solana-rail/src/lib.rs` and `src/tests.rs` are the crate's only
two source files, and neither mentions `Settlement` or `tiered` at all — a
grep for both across the crate returns nothing. The only implementation that
exercises the tiered path is a **test-only** double,
`payment.rs:1315-1355`'s `DisconnectAwareRail`, built specifically to prove
the tier distinction is real in principle. The in-progress
`magnetite-stellar-rail` crate (being built concurrently by another agent as
of this writing, and not modified here) implements `PaymentRail`
(`magnetite-stellar-rail/src/lib.rs:928`) but, as of this read, does not
override `verify_receipt_for_item_tiered` either.

That means the default trait method governs today
(`payment.rs:627-633`): it reports `Settled` on success and **`None`**
(refused, indistinguishable from an invalid receipt) on any failure —
including a Horizon "not found" that is really just pruning or a testnet
reset, not a forged signature. **So the honest, current-code answer is: none
of magnetite's guarantees gracefully degrade today** — the A14 mechanism
that would let a receipt survive Horizon's amnesia as "signed but
unconfirmed" is designed and unit-tested but not yet connected to any real
chain rail. Retention loss currently reads exactly like forgery: a flat "no."
Wiring a rail's Horizon-404 case to `Some(Settlement::SignedUnsettled)`
instead of `None` is what would actually retire the retention question, per
A26's own framing — recorded here as an open recommendation, not a change
made (this session did not touch `magnetite-stellar-rail`, per its
constraint).

## What could not be established

- **Exact reaping mechanics.** Horizon's admin guide says a purge removes
  "historical data related to older ledgers" once a non-zero
  `HISTORY_RETENTION_COUNT` is exceeded, but does not document the precise
  set of tables/rows affected or whether it is transactional. Recorded as
  could-not-establish rather than guessed.
- **Whether any magnetite (as opposed to patala/kotva) code path re-verifies
  a chain receipt long after original settlement**, i.e. whether retention
  loss is reachable in practice today. Could not establish — consistent with
  the standing fact that magnetite itself has never settled anything; there
  is no real historical magnetite receipt to test this against yet.
- **Whether Circle's own USDC issuer on Stellar mainnet is subject to any
  retention behavior different from Horizon's general policy.** Not a
  distinct retention question as far as could be found — mainnet Horizon's
  one-year rolling truncation applies regardless of asset issuer — but
  called out because mainnet itself remains wholly unverified against live
  infrastructure by this project, per `patala-stellar/README.md`.
