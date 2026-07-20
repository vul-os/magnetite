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
