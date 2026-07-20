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
.mg-plate > svg{display:block;width:100%;height:auto;background:var(--dv-surface)}
.mg-cap{padding:11px 15px;border-top:1px solid var(--dv-border);background:var(--dv-code-bg);font-family:var(--doc-mono);font-size:.76rem;line-height:1.6;color:var(--dv-ink-3)}
.mg-cap b{color:var(--accent);font-weight:600;letter-spacing:.09em;text-transform:uppercase;font-size:.68rem;display:block;margin-bottom:3px}
.mg-cap.edge b{color:var(--mg-bnd)}
:root[data-theme="dark"] .mg-cap.edge b{color:#FF74B2}
</style>

# The seams

Everything provider-specific in magnetite plugs in behind a small set of traits
that live in one crate, `magnetite-seams`. Nothing in the game runtime, the
scheduler or the payment path may name a provider-specific type — they see only
these traits.

**Every seam ships a working offline default.** No seam requires a network, a
chain, a homeserver or an account, which is why the whole test suite runs with
zero external services. A provider-specific adapter lives behind its own
feature-gated module and is never referenced by non-provider code.

| # | Seam | Default | Status |
|---|---|---|---|
| 1 | `Identity` / `AuthProvider` | `RawKeypairAuth` — raw Ed25519 challenge/response | Ships |
| 2 | `Naming` | `HashNaming` — pubkey / short-hash addresses | Ships |
| 3 | `BlobStore` | `LocalBlobStore` + `HttpBlobStore` | Ships |
| 4 | `Discovery` | `LanDiscovery` + `TrackerDiscovery` | Ships |
| 5 | `CommsProvider` | `BuiltinProvider`, plus Matrix / Jitsi / LiveKit / Owncast adapters | Ships |
| 6 | `PaymentRail` | `MockPaymentRail` — deterministic, offline | **Mock only — no chain** |
| 7 | `InputProvider` | `LocalDeviceInput` — deterministic keyboard/gamepad | Ships (see the caveat) |

<div class="mg-plate">
<svg viewBox="0 0 900 300" role="img" aria-label="Seam map: the game runtime, scheduler and payment path sit above the magnetite-seams trait boundary and never name a provider type. Below it, each of seven seams has a working offline default, with optional adapters behind feature gates.">
<g font-family="var(--doc-mono)" font-size="10.5">
<rect x="30" y="30" width="840" height="46" rx="8" fill="var(--accent)" opacity=".08"/>
<rect x="30" y="30" width="840" height="46" rx="8" fill="none" stroke="var(--accent)" stroke-width="1.4"/>
<text x="450" y="52" fill="var(--dv-ink)" text-anchor="middle" font-size="12">game runtime · scheduler · payment path</text>
<text x="450" y="68" fill="var(--dv-ink-3)" text-anchor="middle" font-size="9.5">may not name a provider-specific type</text>
<line x1="30" y1="98" x2="870" y2="98" stroke="var(--accent)" stroke-width="1.6" stroke-dasharray="6 5"/>
<text x="450" y="92" fill="var(--accent)" text-anchor="middle" font-size="9" letter-spacing="1.8">— magnetite-seams · TRAITS ONLY —</text>
<g fill="none" stroke="var(--dv-border-2)">
<rect x="30" y="118" width="112" height="58" rx="7"/>
<rect x="152" y="118" width="112" height="58" rx="7"/>
<rect x="274" y="118" width="112" height="58" rx="7"/>
<rect x="396" y="118" width="112" height="58" rx="7"/>
<rect x="518" y="118" width="112" height="58" rx="7"/>
<rect x="640" y="118" width="112" height="58" rx="7"/>
</g>
<rect x="762" y="118" width="108" height="58" rx="7" fill="none" stroke="var(--mg-bnd)" stroke-width="1.6"/>
<g text-anchor="middle" font-size="10">
<text x="86" y="140" fill="var(--dv-ink-2)">Identity</text>
<text x="208" y="140" fill="var(--dv-ink-2)">Naming</text>
<text x="330" y="140" fill="var(--dv-ink-2)">BlobStore</text>
<text x="452" y="140" fill="var(--dv-ink-2)">Discovery</text>
<text x="574" y="140" fill="var(--dv-ink-2)">Comms</text>
<text x="696" y="140" fill="var(--dv-ink-2)">Payment</text>
<text x="816" y="140" fill="var(--mg-bnd)">Input</text>
</g>
<g text-anchor="middle" font-size="9" fill="var(--dv-ink-faint)">
<text x="86" y="158">RawKeypair</text>
<text x="208" y="158">HashNaming</text>
<text x="330" y="158">Local + Http</text>
<text x="452" y="158">LAN + Tracker</text>
<text x="574" y="158">Builtin</text>
<text x="696" y="158">Mock</text>
<text x="816" y="158">LocalDevice</text>
</g>
<g text-anchor="middle" font-size="8.5">
<text x="86" y="170" fill="var(--mg-live)">DEFAULT SHIPS</text>
<text x="208" y="170" fill="var(--mg-live)">DEFAULT SHIPS</text>
<text x="330" y="170" fill="var(--mg-live)">DEFAULT SHIPS</text>
<text x="452" y="170" fill="var(--mg-live)">DEFAULT SHIPS</text>
<text x="574" y="170" fill="var(--mg-live)">DEFAULT SHIPS</text>
<text x="696" y="170" fill="var(--mg-spec)">MOCK ONLY</text>
<text x="816" y="170" fill="var(--mg-bnd)">SEE §7</text>
</g>
<text x="30" y="212" fill="var(--dv-ink-faint)" font-size="9" letter-spacing="1.6">OPTIONAL ADAPTERS — FEATURE-GATED, NEVER REQUIRED</text>
<g fill="none" stroke="var(--dv-border)" stroke-dasharray="3 4">
<rect x="152" y="226" width="112" height="30" rx="6"/>
<rect x="396" y="226" width="112" height="30" rx="6"/>
<rect x="518" y="226" width="112" height="30" rx="6"/>
<rect x="640" y="226" width="112" height="30" rx="6"/>
</g>
<g text-anchor="middle" font-size="9" fill="var(--dv-ink-faint)">
<text x="208" y="245">KeyName</text>
<text x="452" y="245">DHT — unbuilt</text>
<text x="574" y="245">Matrix · Jitsi</text>
<text x="696" y="245">chain — unbuilt</text>
</g>
<text x="30" y="284" fill="var(--dv-ink-3)" font-size="10">Every default works with no network, no chain and no account — which is why CI runs fully offline.</text>
</g>
</svg>
<div class="mg-cap"><b>Figure 1 — the trait boundary</b>The dashed rule is the rule: everything above it is written against traits. Swapping a provider is a configuration change, not a refactor — and the optional adapters below can all be absent without the platform losing a capability.</div>
</div>

## 1. Identity / Auth

Identity is a keypair. `RawKeypairAuth` implements sign-a-challenge login over
Ed25519 with no external dependency, and doubles as a lightweight identity
provider: it mints short-lived, audience- and scope-bound tokens so external
comms systems can be entered from a single keypair login.

## 2. Naming

Human names are a **display layer** over raw keys; the substrate is always the
raw key. The default is short-hash addressing. An optional word-based key-name
provider (`--features keyname`) adds no dependencies and exists to prove the seam
is genuinely swappable rather than hardwired to its default.

## 3. BlobStore

Content addressing: the hash *is* the id. A game's id is the hash of its wasm
module plus manifest, so identifying a game needs no central registry row. Local
and HTTP stores ship; peer-to-peer distribution is a later adapter behind the
same trait.

## 4. Discovery

A phonebook, never an authority. Nodes self-advertise the sessions they host;
each advertisement is **signed by the hosting node's key and leased** with a
capped TTL, so a tracker can refuse forged entries without thereby gaining any
say over who may host what. A node that dies cannot leave a stale entry for long.
Announcements fan out across several phonebooks: being listed on one is enough to
host, and one unreachable tracker never blocks a node.

## 5. CommsProvider

Chat, voice, video and streaming are adapters, not a product magnetite builds.
The node mints scoped join credentials from the player's keypair, so one login
carries into every room; a credential may be gated behind a payment receipt.

## 6. PaymentRail

Non-custodial by design: no balances, no payout queue, no custody. An entitlement
is a signed receipt keyed to `(buyer, game, item)`; hosting fees ride a payment
channel; wagers settle from escrow.

**What ships is the deterministic offline mock.** No chain integration exists, so
nothing here moves real money today. The protocol fee parameter defaults to `0`.

## 7. InputProvider — and the boundary it draws

This seam exists to make one distinction impossible to lose track of.

Magnetite's core claim is that a match is *reproducible*: clients send inputs,
the host steps the sim, every tick lands in a `ReplayLog`, and a third party can
re-run `verify_replay` and **prove** tampering from the record alone. That works
only because the inputs are deterministic.

Not every input source can offer that. A camera-gesture stream is a
**nondeterministic sensor reading**. There is no way to re-derive "the player
swung at 6.2 m/s" from a log, because the pixels that produced it are gone and
were never authoritative. So input is split into two classes:

| Class | Example | Replay-verifiable | What the host can prove |
|---|---|---|---|
| `Deterministic` | keyboard, gamepad, scripted bot | **yes** | tampering, from the log alone |
| `Attested` | camera gesture, IMU, any sensor | **no** | only *implausibility*, never intent |

<div class="mg-plate">
<svg viewBox="0 0 900 320" role="img" aria-label="The class boundary: deterministic input flows into the replay-verifiable core where a log proves tampering. Attested sensor input crosses a boundary where verification stops; it is rate-limited, signature-checked and screened for plausibility, but a plausible fabricated event passes every check.">
<g font-family="var(--doc-mono)" font-size="10.5">
<!-- deterministic side -->
<rect x="24" y="46" width="404" height="240" rx="10" fill="var(--accent)" opacity=".05"/>
<rect x="24" y="46" width="404" height="240" rx="10" fill="none" stroke="var(--accent)" stroke-width="1.4"/>
<text x="44" y="72" fill="var(--accent)" font-size="9.5" letter-spacing="1.6">INSIDE THE RECORD</text>
<rect x="52" y="90" width="150" height="42" rx="7" fill="none" stroke="var(--dv-border-2)"/>
<text x="127" y="108" fill="var(--dv-ink-2)" text-anchor="middle" font-size="10">keyboard · gamepad</text>
<text x="127" y="123" fill="var(--dv-ink-faint)" text-anchor="middle" font-size="9">LocalDeviceInput</text>
<rect x="250" y="90" width="150" height="42" rx="7" fill="none" stroke="var(--accent)"/>
<text x="325" y="108" fill="var(--dv-ink)" text-anchor="middle" font-size="10">ordered command</text>
<text x="325" y="123" fill="var(--accent)" text-anchor="middle" font-size="9">Deterministic</text>
<rect x="52" y="164" width="348" height="46" rx="7" fill="none" stroke="var(--dv-border-2)"/>
<text x="226" y="184" fill="var(--dv-ink-2)" text-anchor="middle" font-size="10.5">ReplayLog → verify_replay</text>
<text x="226" y="200" fill="var(--dv-ink-faint)" text-anchor="middle" font-size="9">re-simulate · compare hashes · locate divergence</text>
<text x="226" y="240" fill="var(--mg-live)" text-anchor="middle" font-size="11">is_replay_verifiable() → true</text>
<text x="226" y="262" fill="var(--dv-ink-3)" text-anchor="middle" font-size="9.5">Tampering can be PROVEN from the record alone.</text>
<!-- the boundary -->
<line x1="450" y1="30" x2="450" y2="300" stroke="var(--mg-bnd)" stroke-width="2" stroke-dasharray="7 6"/>
<text x="450" y="22" fill="var(--mg-bnd)" text-anchor="middle" font-size="9" letter-spacing="1.6">VERIFICATION STOPS HERE</text>
<!-- attested side -->
<rect x="472" y="46" width="404" height="240" rx="10" fill="var(--mg-bnd)" opacity=".05"/>
<rect x="472" y="46" width="404" height="240" rx="10" fill="none" stroke="var(--mg-bnd)" stroke-width="1.4"/>
<text x="492" y="72" fill="var(--mg-bnd)" font-size="9.5" letter-spacing="1.6">OUTSIDE THE RECORD</text>
<rect x="500" y="90" width="150" height="42" rx="7" fill="none" stroke="var(--dv-border-2)" stroke-dasharray="3 3"/>
<text x="575" y="108" fill="var(--dv-ink-2)" text-anchor="middle" font-size="10">camera · IMU</text>
<text x="575" y="123" fill="var(--dv-ink-faint)" text-anchor="middle" font-size="9">no producer in tree</text>
<rect x="698" y="90" width="150" height="42" rx="7" fill="none" stroke="var(--mg-bnd)"/>
<text x="773" y="108" fill="var(--dv-ink)" text-anchor="middle" font-size="10">signed assertion</text>
<text x="773" y="123" fill="var(--mg-bnd)" text-anchor="middle" font-size="9">Attested</text>
<rect x="500" y="164" width="348" height="46" rx="7" fill="none" stroke="var(--dv-border-2)"/>
<text x="674" y="184" fill="var(--dv-ink-2)" text-anchor="middle" font-size="10.5">rate limit → signature → PlausibilityGate</text>
<text x="674" y="200" fill="var(--dv-ink-faint)" text-anchor="middle" font-size="9">"not physically reachable" is all it can reject</text>
<text x="674" y="240" fill="var(--mg-bnd)" text-anchor="middle" font-size="11">is_replay_verifiable() → false</text>
<text x="674" y="262" fill="var(--dv-ink-3)" text-anchor="middle" font-size="9.5">A plausible fabricated event passes every check.</text>
</g>
<g stroke="var(--dv-border-2)" stroke-width="1.5" fill="none" marker-end="url(#sar)">
<path d="M206 111 H244"/><path d="M325 136 V160"/>
<path d="M654 111 H692"/><path d="M773 136 V160"/>
</g>
<defs><marker id="sar" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="5" markerHeight="5" orient="auto"><path d="M0 0 L10 5 L0 10 z" fill="var(--dv-border-2)"/></marker></defs>
</svg>
<div class="mg-cap edge"><b>Figure 2 — the boundary this seam exists to draw</b>The two halves look symmetrical and are not. On the left, a log proves what happened. On the right, a host can only refuse the impossible — and a signature proves <em>authorship, not truth</em>, because a cheater signs their own fabricated numbers with their own genuine key.</div>
</div>

`InputEvent` is an enum rather than a struct with a flag, so a consumer cannot
read the payload without first matching on the class — the compiler makes you
answer "which guarantee does this carry?". `InputClass::is_replay_verifiable()`
exists so that decisions like "may I settle a wager escrow from this?" are made
in code rather than from a reader's memory.

### What a host can actually do with attested input

`PlausibilityGate` screens each event, per player, against: a rate limit, a
per-kind cooldown, a human-reachable velocity ceiling, a confidence floor,
timestamp sanity, and monotonic sequence numbers (so a captured event cannot be
re-sent). Screening state advances **only on acceptance**, so a flood of rejected
events cannot push an honest player's own events out of the rate window.

Rejection means "this is not physically reachable". **Acceptance means nothing
stronger than "not obviously impossible."**

### Two things this seam does not give you

1. **A signature proves authorship, not truth.** `SignedAttestedEvent` stops one
   player forging events in another player's name and stops a relay editing them
   in flight. It does not make the sensor reading true — a cheater signs their own
   fabricated events with their own genuine key and passes verification every
   time.
2. **A plausible fake is undetectable.** A cheater who synthesises events inside
   human bounds passes every check that exists, and `verify_replay` cannot help,
   because this input class is outside its reach by construction. This is a
   property of sensor input, not a defect to be fixed in a later version. There
   is a test named
   `a_plausible_synthetic_event_is_indistinguishable_from_a_real_one` asserting
   exactly this, so the limit stays written down in code.

### What is built

The traits, both event classes, the signed-event wrapper, the plausibility gate,
and two providers: `LocalDeviceInput` (the default — a deterministic
keyboard/gamepad queue that *refuses* attested events at runtime, so the class
boundary is enforced rather than merely documented) and `AttestedEventInput`, a
transport-agnostic host-side ingress for attested events.

### What is not built

Anything that **produces** a gesture event. `AttestedEventInput` contains no
camera capture, no pose model and no vendor code, and magnetite has no such code
anywhere in the tree. The seam was designed so a camera-gesture client could plug
into magnetite's sessions, discovery and payments without magnetite growing a
computer-vision dependency — but no such client is wired up here. Today this seam
is the socket, and only the socket.
