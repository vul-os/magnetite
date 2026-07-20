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
/* the status key — a designed element, not a bolted-on disclaimer */
.mg-key{display:grid;grid-template-columns:repeat(4,1fr);gap:1px;background:var(--dv-border);border:1px solid var(--dv-border);border-radius:10px;overflow:hidden;margin:1.6rem 0}
@media(max-width:760px){.mg-key{grid-template-columns:1fr 1fr}}
@media(max-width:420px){.mg-key{grid-template-columns:1fr}}
.mg-k{background:var(--dv-surface);padding:15px}
.mg-k i{display:block;width:24px;height:3px;border-radius:2px;background:var(--kc);margin-bottom:9px}
.mg-k b{display:block;font-family:var(--doc-mono);font-size:.68rem;font-weight:600;letter-spacing:.11em;text-transform:uppercase;color:var(--kc)}
.mg-k span{display:block;font-size:.8rem;line-height:1.5;color:var(--dv-ink-3);margin-top:5px}
.mg-k.live{--kc:#17803D} .mg-k.lan{--kc:#A45B00} .mg-k.mock{--kc:#C4006B} .mg-k.no{--kc:#737C90}
:root[data-theme="dark"] .mg-k.live{--kc:#6EE79B}
:root[data-theme="dark"] .mg-k.lan{--kc:#FFC24D}
:root[data-theme="dark"] .mg-k.mock{--kc:#FF74B2}
:root[data-theme="dark"] .mg-k.no{--kc:#8892A6}
.mg-s{font-family:var(--doc-mono);font-size:.7rem;font-weight:600;letter-spacing:.07em;text-transform:uppercase;white-space:nowrap}
.mg-s.live{color:#17803D} .mg-s.lan{color:#A45B00} .mg-s.mock{color:var(--mg-bnd)} .mg-s.no{color:#737C90}
:root[data-theme="dark"] .mg-s.live{color:#6EE79B}
:root[data-theme="dark"] .mg-s.lan{color:#FFC24D}
:root[data-theme="dark"] .mg-s.mock{color:#FF74B2}
:root[data-theme="dark"] .mg-s.no{color:#8892A6}
</style>

# Status — what actually runs today

Magnetite is mid-conversion from a conventional central-backend game platform to
the no-central-cloud design in `DECENTRALIZATION.md`. That spec describes the
**intent**. This page describes the **state**, audited against the tree rather
than against the pitch, so that nothing elsewhere in these docs reads as more
finished than it is.

<div class="mg-key">
  <div class="mg-k live"><i></i><b>Running</b><span>Built, tested, exercised. You can run it today.</span></div>
  <div class="mg-k lan"><i></i><b>LAN only</b><span>Real, tested code — but never validated beyond a local network.</span></div>
  <div class="mg-k mock"><i></i><b>Mock only</b><span>The interface is implemented; what sits behind it is a deterministic offline stub.</span></div>
  <div class="mg-k no"><i></i><b>Not built</b><span>Specified in the redesign doc, absent from the tree.</span></div>
</div>

## Working

| Capability | What it is | State |
|---|---|---|
| Authoritative simulation | `AuthoritativeGame` — deterministic `validate` / `step` in the SDK. | <span class="mg-s live">Running</span> |
| WASM sandbox | Wasmtime, `wasm32-wasip1`, fuel budget, memory cap, epoch interrupt. No OS randomness and no wall clock inside the guest. | <span class="mg-s live">Running</span> |
| Replay verification | `ReplayLog` + `verify_replay`. Anyone re-simulates from scratch and locates tampering. | <span class="mg-s live">Running</span> |
| Anti-cheat | Composable validator chain, per-player trust scoring, warn → kick → ban escalation. | <span class="mg-s live">Running</span> |
| Zero-backend dev loop | `magnetite dev` builds to wasm and serves a live match with no server, no database and no account. | <span class="mg-s live">Running</span> |
| Seam crate | Every seam is a trait with a default that needs no external service, so the suite runs fully offline. | <span class="mg-s live">Running</span> |
| Content-addressed games | Game id is the hash of wasm + manifest; loading verifies the hash and fails closed. | <span class="mg-s live">Running</span> |
| Capacity-elastic node | The node measures its own cores and RAM and derives its shard and player budget from them — never a config constant. | <span class="mg-s live">Running</span> |
| Signed discovery | Nodes sign and lease their own `SessionAd`s, TTL-capped, over LAN and ordinary HTTP trackers, fanned out redundantly. | <span class="mg-s live">Running</span> |
| Comms adapters | Matrix, Jitsi, LiveKit and Owncast behind one `CommsProvider` trait, in `backend/src/comms/providers.rs`, with the old in-house stack demoted to a fallback. | <span class="mg-s live">Running</span> |

## Working, but LAN-only

| Capability | What it is | State |
|---|---|---|
| Shard migration | Two-phase, epoch-fenced handoff. Every partial failure — ack timeout, rejection, dropped connection, target crash — resolves to "the source keeps authority" with state intact. | <span class="mg-s lan">LAN only</span> |
| Cluster membership | Deny-by-default operator allowlist of node public keys, checked when an ad is observed, again at migration time, and on the inbound connection allowlist. | <span class="mg-s lan">LAN only</span> |
| Session follow | Signed, single-use redirects move players with their shard. A forged redirect is inert; nothing is minted on a rolled-back migration. | <span class="mg-s lan">LAN only</span> |
| Attested input wire | Signed events reach a live node over a real socket: per-connection rate limit, then signature, then plausibility gate, then queue. | <span class="mg-s lan">LAN only</span> |

These are real, tested code, not scaffolding. They have been exercised
**in-process and over a LAN only**. There is no NAT traversal, no relay and no
WAN validation, so nodes must already be directly reachable at their advertised
address.

> [!WARNING]
> **Internet-scale fleets between strangers' machines are not demonstrated.**
> If your plan depends on operators joining a mesh across the open internet,
> that rung — the federated `Sharded` topology, "Bucket D" — is unbuilt. Plan
> accordingly.

## Not built

| Capability | What it is | State |
|---|---|---|
| On-chain payments | The `PaymentRail` seam models atomic wallet-to-wallet splits, signed-receipt entitlements, hosting channels and wager escrow. What ships is `MockPaymentRail`, a deterministic offline mock that signs receipts so CI can run without a network. No chain is wired up and no real payment has ever settled through it. | <span class="mg-s mock">Mock only</span> |
| Multi-node `Sharded` | The topology rung above a single operator's cluster, where other operators' nodes join the mesh. | <span class="mg-s no">Not built</span> |
| Trackerless discovery | A DHT adapter behind the same `Discovery` trait. Specified and unwritten. | <span class="mg-s no">Not built</span> |
| Sensor input producers | Nothing in magnetite *generates* an attested event. No camera capture, no pose model, no vendor SDK anywhere in the tree. | <span class="mg-s no">Not built</span> |
| Attested input consumers | No shipped game *drains* the accepted-event queue either. Accepted events sit there. | <span class="mg-s no">Not built</span> |
| Removal of the old backend | Fiat billing, custody, balances and payout queues are gone. Parts of the older central API are still present. | <span class="mg-s no">In progress</span> |

Both ends of the attested-input path are missing on purpose. The seam exists to
draw a boundary — see [The seams](./docs.html#seams) — not to ship
gesture control.

## Screenshots

The images in these docs are captured from the app in the repository, running
against deterministic mock data with no backend, database or wasm build. They
show the **shape of the interface**, not a live network: every address, price
and receipt is fixture data.

<div class="mg-plate">
<div class="mg-bar"><i></i><i></i><i></i><span>magnetite · /wallet</span></div>
<img class="mg-light" src="./shots/wallet-light.png" alt="" loading="lazy" decoding="async" />
<img class="mg-dark" src="./shots/wallet-dark.png" alt="magnetite wallet screen showing a linked address and a ledger of signed receipts, labelled non-custodial with the payment rail set to mock" loading="lazy" decoding="async" />
<div class="mg-cap"><b>The wallet screen labels its own rail</b>Note the card reads <code>RAIL: MOCK</code> and the API reports <code>custodial: false</code>. That is the deterministic offline rail described above. There are no balances on this screen because the platform holds none — the list is receipts, each verifiable against the rail's signing key.</div>
</div>

Three screens are published: the [server browser](./docs.html#hosting-a-server),
this wallet, and [developer revenue](./docs.html#payments). Other
surfaces in the app — the developer dashboard in particular — still render
pre-redesign custodial framing and are deliberately **not** shown here, because
a screenshot of a concept the redesign deleted would be a false claim.

Diagrams elsewhere in these docs are diagrams, not captures.
