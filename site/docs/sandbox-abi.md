# The sandbox ABI — `mag_*`

This is the normative contract between a magnetite host and a game module. It is
a plain C-shaped calling convention over WebAssembly linear memory: eight
exported functions, one exported memory, and a 4-byte length prefix in front of
every buffer the module hands back.

**Nothing in it is Rust.** No `#[repr]`, no trait objects, no allocator, no
name mangling, no `wasm-bindgen`, no component model. The types that cross the
boundary are `i32`, `i64`, and UTF-8 JSON. Any language that can produce a
`wasm32-wasip1` module with C-ABI exports can satisfy it, and the Rust SDK is a
convenience on top of it rather than a requirement.

That matters because it changes what magnetite asks of you. You do not have to
rewrite your game in Rust to get authoritative multiplayer. You keep your
renderer — Godot, three.js, Unity, Bevy, whatever — and express only your
*rules* as a small module against this contract. The rules are what has to be
deterministic and server-side; the pixels never were.

> [!NOTE]
> **What is demonstrated, and what is argued.** The contract below is derived
> from the shipping implementation, not from a design document: the host side is
> `magnetite-sandbox/src/executor.rs` and the reference guest is
> `game-templates/authoritative/src/wasm_abi.rs`.
>
> Two modules have been run against the conformance harness, and both now report
> `34 pass · 0 fail · 0 warn`:
> [`magnetite-sandbox/conformance/reference.wat`](https://github.com/vul-os/magnetite/blob/main/magnetite-sandbox/conformance/reference.wat)
> — hand-written WebAssembly text, no Rust anywhere in it, zero imports — and the
> Rust arena-shooter template. The Rust one did not, at first: writing this page
> and running the harness against it turned up seven defects, all since fixed and
> all recorded in §8.
>
> So the hand-written module is a genuine second implementation, and it is why
> "language-agnostic" is stated as fact here rather than argued. It is **not**
> the same claim as "we support Zig / TinyGo / AssemblyScript": no module from
> any of those toolchains has been built or run, because none of those
> toolchains was available on the machine this was written on. See
> [Status](./docs.html#status) for the register these docs keep.
>
> **This version of the ABI is not compatible with the one that preceded it.**
> `mag_step` carries the tick, `mag_restore` no longer takes a length prefix, and
> `mag_abi_version` must be exported. A module built against the older, undeclared
> ABI is refused at load rather than misread. Existing consumers need a rebuild —
> see §11.

---

## 1. The interface at a glance

A conforming module exports one memory and eight functions.

| Export | Signature | Direction | Purpose |
|---|---|---|---|
| `memory` | linear memory | — | Must be exported under exactly this name. |
| `mag_abi_version` | `() -> i32` | host calls | Declare which ABI this module speaks. Must return `1`. |
| `mag_alloc` | `(i32 len) -> i32 ptr` | host calls | Give the host `len` writable bytes. |
| `mag_free` | `(i32 ptr, i32 len)` | host calls | Release a buffer. May be a no-op; must not trap. |
| `mag_init` | `(i32 ptr, i32 len)` | host → guest | Start a match from a JSON `MatchConfig`. |
| `mag_step` | `(i32 ptr, i32 len) -> i32 ptr` | host → guest | Advance one tick; return a `StepOutput` buffer. |
| `mag_snapshot` | `() -> i32 ptr` | guest → host | Return the full authoritative state. |
| `mag_restore` | `(i32 ptr, i32 len)` | host → guest | Replace state from a snapshot buffer. |
| `mag_view` | `(i64 player_id) -> i32 ptr` | guest → host | Return the bytes this one player may see. |

### The version is checked first

`mag_abi_version` is the first thing the host calls, before a single byte of
payload is exchanged. It must return `1` — the version this document describes.
Anything else, or a missing export, and the module is refused at load.

Absence is a refusal, not a default. That is deliberate: a module built against
the undeclared predecessor of this ABI cannot return a version, and the failure
mode we are buying our way out of is precisely that such a module would otherwise
be handed payloads it silently misreads. Three lines of code on your side turns
that into an error message.

Bump it when, and only when, the shape of the calls below changes.

`i32` carries both pointers and lengths, as unsigned values in signed slots —
the host casts, and a pointer above `0x7fff_ffff` is not representable. Since
the memory cap is far below 2 GiB this is not a practical constraint, but a
module must not return a pointer with the high bit set.

`mag_view` is the only export with a 64-bit parameter. Player ids are `u64`,
passed in an `i64` slot; reinterpret the bits, do not sign-extend.

Exports must be named exactly as above, with no leading underscore and no
mangling. In C, `__attribute__((export_name("mag_step")))`; in Rust,
`#[no_mangle] pub extern "C"`; in WAT, `(func (export "mag_step") …)`.

## 2. The framing

Every buffer the **module returns** is length-prefixed:

```text
ptr ──▶ ┌───────────────┬─────────────────────────────┐
        │ u32 LE length │ payload, `length` bytes     │
        └───────────────┴─────────────────────────────┘
          4 bytes          JSON (by convention; see §4)
```

The length is a **little-endian unsigned 32-bit integer** and counts the payload
only — it excludes its own four bytes. Little-endian is not a choice the
contract makes so much as one WebAssembly makes for it: `i32.store` is
little-endian by definition, so `i32.store(base, payload_len)` is a correct
prefix on every platform.

Buffers the **host passes in** are never prefixed. All three inbound calls are
framed identically:

| Call | What the host passes |
|---|---|
| `mag_init(ptr, len)` | `len` = payload length; payload starts at `ptr + 0`. |
| `mag_step(ptr, len)` | `len` = payload length; payload starts at `ptr + 0`. |
| `mag_restore(ptr, len)` | `len` = payload length; payload starts at `ptr + 0`. |

The rule is worth stating as a rule, because getting it wrong is a real cost that
was really paid: **the length is a parameter, so a prefix would only say it
twice.** An earlier revision of this ABI prefixed `mag_restore` and nothing else.
Two drivers then disagreed about whether it was prefixed — magnetite's host said
yes, wibbly's browser driver said no — and because both interpretations parse
*something*, the disagreement produced a silent no-op rather than an error. The
redundancy was the root cause. It is gone.

## 3. Memory ownership and lifetime

**The module owns all of its linear memory. The host never allocates in it and
never writes outside a region the module handed over.**

The exact sequence the host performs for one tick:

```text
p = mag_alloc(len(payload))     ; module returns writable space
  host writes {"tick":T,"inputs":[…]} at p
o = mag_step(p, len(payload))   ; module returns a prefixed buffer
  host reads 4 bytes at o -> n
  host copies n bytes from o+4
  host checks the payload's "tick" == T
mag_free(p, len(payload))
mag_free(o, 4 + n)
s = mag_snapshot()              ; module returns a prefixed buffer
  host copies it, then mag_free(s, 4 + m)
v = mag_view(player)            ; once per player that submitted input
  host copies it, then mag_free(v, 4 + k)
```

Two rules follow, and both are stricter than they may look:

1. **A returned buffer is valid until the next call to any `mag_*` export.**
   The host always copies out before it calls anything else, so a bump allocator
   that resets on `mag_step` is fine. The looser-sounding phrasing "valid until
   the next call to *the same* export" appears in the reference module's own
   comments; do not rely on it. The guarantee the host actually needs, and the
   only one it depends on, is the one stated here.

2. **A module must consume the host's input buffer before it invalidates it.**
   If `mag_step` resets a bump arena, it must fold, copy or parse the incoming
   bytes *first*, because the output buffer it then allocates may overlap them.

`mag_free` exists so that a module with a real allocator can use one. A bump
allocator may implement it as an empty function. It must never trap: the host
calls it on every buffer, including ones it obtained from the module.

`mag_alloc` has no failure channel — there is no reserved null and the host does
not check the return value against one. A module that cannot service an
allocation must **trap** (`unreachable`, `abort()`, a Rust panic under
`panic = "abort"`). Returning a pointer the host will then write through is
memory corruption inside the sandbox; trapping is a clean, classified failure.

## 4. Payloads

Only two payload schemas are normative, because only two are read by the host.

### 4.1 `mag_init` — `MatchConfig`, host → guest

```json
{
  "topology": "SingleRoom",
  "max_players": 4,
  "tick_hz": 60,
  "seed": 16045690981097406622,
  "snapshot_every": 300
}
```

| Field | Type | Meaning |
|---|---|---|
| `topology` | see below | Scale shape the host has chosen for this match. |
| `max_players` | `u32` | Hard cap on simultaneous players. |
| `tick_hz` | `u16` | Authoritative tick rate. `1000 / tick_hz` is your nominal `dt` in ms. |
| `seed` | `u64` | **The only entropy the module will ever receive.** |
| `snapshot_every` | `u16` | How often the host intends to take a full snapshot. Informational. |

`topology` is a serde externally-tagged enum, so it is a bare string for the
unit variant and a single-key object otherwise:

```json
"SingleRoom"
{ "Dedicated": { "tick_hz": 60 } }
{ "Sharded": { "tick_hz": 20, "cell_size": 500.0, "max_per_shard": 64 } }
```

A module that only cares about `seed` and `tick_hz` may ignore the rest, but it
must not fail to parse on an unrecognised topology variant — the host chooses it,
and new variants are additive.

There is no `on_join` in this ABI. Players are not announced; they appear
because a `mag_step` payload contains their id for the first time, and they
disappear because it stops. **First sight is the join**, and a module that needs
spawn logic must run it there.

This is not optional bookkeeping. A module that waits for a join signal will wait
forever, hold an empty world, and reject every input it is ever handed — which is
what the Rust SDK did until this contract was written (§8, item 4). Two
consequences for your implementation:

- Derive join order from the input array, which arrives sorted by ascending
  `player_id`. Anything you seed from join order — spawn positions, team
  assignment — is then reproducible.
- Make your join handler **idempotent**. After a `mag_restore` the players are
  whoever the snapshot says, and nothing tells you who that was; the next input
  from each of them is another "first sight". Joining a player who is already
  present must be a no-op, or a restore will duplicate everyone.

### 4.2 `mag_step` — the tick and its inputs, host → guest

An object with the authoritative tick and one entry per player who submitted an
input frame. `inputs` is empty on a tick where nobody sent anything, and that is
common; `tick` is always present.

```json
{
  "tick": 42,
  "inputs": [
    {
      "player_id": 1,
      "input": {
        "keys": {
          "forward": true,  "backward": false, "left": false, "right": false,
          "jump": false,    "crouch": false,   "attack": false,
          "secondary_attack": false, "interact": false, "sprint": false
        },
        "mouse": {
          "x": 640.0, "y": 360.0, "delta_x": 0.0, "delta_y": 0.0,
          "left_button": false, "right_button": false, "middle_button": false,
          "scroll": 0.0
        },
        "sequence": 42,
        "timestamp_ms": 1706000000000
      }
    }
  ]
}
```

| Field | Type | Notes |
|---|---|---|
| `tick` | `u64` | The tick this step advances **to**. Strictly increasing within a match. |
| `inputs[].player_id` | `u64` | Assigned by the host. Stable for the session. |
| `inputs[].input.keys` | 10 booleans | A held-state snapshot, not an event stream. |
| `inputs[].input.mouse` | 4 `f64` + 3 booleans + `f64` | Viewport pixels, origin top-left. |
| `inputs[].input.sequence` | `u64` | Client-local frame counter, for acknowledgement. |
| `inputs[].input.timestamp_ms` | `u64` | **Client-supplied and untrusted.** Not a clock. |

`tick` is the tick you must simulate, and you must echo it back in your
`StepOutput` (§4.3). Tick 0 is the state `mag_init` produced, so the first step is
tick 1.

A guest **MAY** refuse a step whose `tick` is not greater than the last one it
ran, and both reference modules in this repo do — trapping is the loudest
available answer and a mis-sequenced step is not a thing to absorb quietly. A
guest **MUST NOT** infer the tick by counting calls now that it is told; counting
is what created the two-sources-of-truth problem the field exists to remove.

Ordering guarantees, exactly:

- **At most one entry per `player_id` per tick.** The host keeps one pending
- **At most one entry per `player_id` per tick.** The host keeps one pending
  frame per connection and takes it.
- **Entries arrive sorted by ascending `player_id`.** Both the runtime's input
  drain and the Rust SDK's executor sort before stepping, precisely so that
  arrival order cannot leak into the simulation.

Depending on any *other* order is a determinism bug, and a module that sorts
defensively costs nothing. Treat the ordering above as what you will get and
sorting as what you should do anyway.

`timestamp_ms` deserves its own warning. It is a number a client chose. Reading
it as a clock hands every client a lever on your simulation and destroys
replayability at the same time. It exists for latency estimation on the host, not
for physics in the guest.

### 4.3 `mag_step` — `StepOutput`, guest → host

The one payload the host parses coming out. Everything else it treats as bytes.

```json
{
  "rejects": [
    { "player_id": 3, "reason": "RateLimited" },
    { "player_id": 7, "reason": { "IllegalAction": "speed hack" } }
  ],
  "state_hash": 11400714819323198485,
  "tick": 42
}
```

| Field | Type | Meaning |
|---|---|---|
| `rejects` | array | Players whose input this tick was refused, and why. May be empty. |
| `state_hash` | `u64` | A hash of the authoritative state **after** this tick. |
| `tick` | `u64` | The tick you simulated. Must equal the `tick` you were given. |

`tick` is required, not optional. The host compares it against the tick it asked
for and fails the step on any disagreement, which is the mechanism that makes a
guest which has lost track of the tick — restored a snapshot without adopting its
tick, or is still counting calls — **detectable instead of silently wrong**. A
module that omits the field fails to decode, which is also detectable. There is
deliberately no default: defaulting to 0 would make every such module look like it
agreed about tick 0.

`reason` is an externally-tagged enum with five variants:

```json
"RateLimited"     "OutOfBounds"     "StaleInput"     "Unauthorized"
{ "IllegalAction": "any explanatory string" }
```

`state_hash` is the load-bearing field, and the module — not the host — decides
what it means. The host copies it into the replay log verbatim and compares it
against a re-simulation. So the requirement is not a particular hash function; it
is that the value be a **pure function of the authoritative state**, identical on
every machine and every run for identical `(state, ordered inputs, tick, seed)`.
The Rust SDK uses FNV-1a 64 over the canonical JSON serialisation of the
snapshot, which is a reasonable default and not a mandate.

Do not report `0` as a legitimate hash. The host uses `state_hash = 0` as its
own signal that the guest call failed (see §6); a module that
emits `0` is indistinguishable from a module that crashed.

> The host's decoder accepts each reject as either the object above or a
> two-element array `[3, "RateLimited"]`, because serde structs deserialise from
> sequences too. The object form is normative. The array form works today and
> should not be relied on.

### 4.4 `mag_snapshot` / `mag_restore` / `mag_view` — opaque to the host

The host copies these payloads, stores them, ships them, and hands them back. It
never parses them. The framing in §2 is mandatory; the content is yours.

That freedom has a cost, so the contract makes one recommendation and two
requirements:

- **Recommended:** make them JSON. Snapshot bytes end up in checkpoints and
  replay logs, and JSON is what makes an incident debuggable by a human and a
  divergence locatable by a third party. It is also what the Rust SDK's
  host-side delta computation assumes. A module emitting CBOR is conforming and
  will simply forgo those.
- **Required:** `mag_restore(mag_snapshot())` must be a faithful round-trip.
  Snapshot, restore into a *fresh instance*, and the two must then step
  identically forward under identical inputs. Shard migration, checkpointing and
  replay-from-snapshot are all exactly this operation.
- **Required:** `mag_view(p)` must contain **only what player `p` is allowed to
  know.** These bytes are transmitted to that player's client. Enemy positions
  behind walls in a view payload are a wallhack you shipped yourself. This is the
  one place where the ABI's design does real anti-cheat work, and it only works
  if the module honours it.

`mag_snapshot` must be callable at any point after `mag_init` — the host calls it
immediately after initialising, and again after every step and every restore.

## 5. Determinism — the requirements

The contract exists to make a match re-simulable by a stranger. That is the
entire point of the sandbox, and it is enforced by absence: the host does not
give the guest the things that would break it.

**MUST:**

1. **Derive all randomness from `MatchConfig.seed`.** It is the only entropy
   that crosses the boundary. Any PRNG works as long as it is the same PRNG
   everywhere; the SDK uses xoshiro256\*\* seeded through splitmix64.
2. **Never read a clock.** `clock_time_get` and `clock_res_get` are linked but
   return `ENOSYS` (38). A language runtime that aborts when the clock fails will
   take your module down with it.
3. **Never read OS randomness.** `random_get` also returns `ENOSYS` (38). In
   particular, a hash map whose iteration order comes from a randomly seeded
   hasher is a determinism bug even when the seeding fails identically — sort
   before you iterate.
4. **Treat `timestamp_ms` as untrusted data, never as time.**
5. **Iterate in a defined order.** No hash-map iteration order, no
   pointer-address ordering, no "whatever order the inputs arrived in".
6. **Take the tick from the payload, and put it in your snapshot.** `mag_step`
   tells you the tick (§4.2), so never count calls to infer it. Serialise it in
   `mag_snapshot` and adopt it in `mag_restore`, because a restored module that
   resumes on the wrong tick breaks replay-from-snapshot for any simulation with
   a cooldown, a timer or a lifetime in it. Unlike every other rule here, this
   one is *enforced*: you echo the tick in your `StepOutput` and the host checks
   it.
7. **Put *everything* that affects the simulation in the snapshot — including
   your RNG position.** This is the same rule as (6) and the trap is subtler. A
   long-lived PRNG stream has a position; if that position is not in your
   snapshot, a restored module rewinds its randomness and diverges from the
   module the snapshot came from. Either serialise the position, or make the
   stream a pure function of the tick — which is both what the Rust SDK does
   (`StepCtx::rng` is derived per tick from `(seed, tick)`) and much the easier
   thing to get right, since the tick is handed to you and already has to be in
   the snapshot by rule (6).
8. **Avoid accumulating `f64` across ticks**, and never serialise a raw NaN bit
   pattern. WebAssembly's arithmetic is deterministic, but NaN payloads are not
   fully specified and a hash over the bits of one is not portable.

**MUST NOT** rely on threads, on `proc_exit`, on `fd_write` output being seen
(it is discarded), on environment variables or on arguments (both are empty).

The host's entire import surface is ten functions, all in
`wasi_snapshot_preview1`:

| Import | Behaviour |
|---|---|
| `clock_time_get`, `clock_res_get` | return `38` (`ENOSYS`) |
| `random_get` | returns `38` (`ENOSYS`) |
| `fd_write` | returns `0`, output discarded |
| `fd_read` | returns `0`, reads nothing |
| `proc_exit` | no-op |
| `environ_get`, `environ_sizes_get` | return `0`, no variables |
| `args_get`, `args_sizes_get` | return `0`, no arguments |

Anything else a module imports cannot be linked, and the module will be rejected
at load time. **A conforming module may import nothing at all** — the WAT
reference module imports zero functions, which is the cleanest possible position
to be in.

## 6. Failure modes

Fuel, memory and wall-clock budgets are set from `LimitsConfig`. Defaults: 10 M
fuel units per step, a 64 MiB memory cap, and a 2-epoch deadline with a 5 ms
epoch tick — a 10 ms wall-clock ceiling.

One subtlety worth internalising: **the budgets are replenished at the start of
each `mag_step` and then shared by every guest call until the next one.** A tick
is `mag_step` plus `mag_free` plus `mag_snapshot` plus one `mag_view` per active
player, and all of it draws on the same 10 M units and the same 10 ms. Budget
your snapshot and view serialisation, not just your simulation.

| Failure | What the guest sees | What the host does |
|---|---|---|
| Fuel exhausted | Trap, mid-instruction. No unwinding, no cleanup. | `SandboxError::FuelExhausted`; the tick reports `state_hash = 0`. |
| Epoch deadline passed | Trap at the next function entry or loop back-edge. | `SandboxError::EpochTimeout`; `state_hash = 0`. |
| `memory.grow` past the cap | **`memory.grow` returns `-1`. It does not trap.** | Nothing — yet. The module must handle a failed grow. A runtime that then dereferences null traps as `MemoryLimitExceeded`. |
| Initial memory above the cap | Instantiation is refused. | The module is rejected before the match starts. |
| Malformed JSON out of `mag_step` | — | `SandboxError::Serialise`; `state_hash = 0`. |
| Length prefix pointing outside memory | — | `SandboxError::InvalidGuestPointer`; `state_hash = 0`. |
| A required export missing or mistyped | — | `SandboxError::MissingExport` at load; the match never starts. |
| `mag_abi_version()` returns anything but `1` | — | `SandboxError::AbiVersionMismatch` at load, **before any payload is written**; the match never starts. |
| The echoed `tick` disagrees with the requested one | — | `SandboxError::TickMismatch`; `state_hash = 0`. |
| `mag_init` traps | Trap. | Load fails; the module is rejected. |
| Any other trap (`unreachable`, panic, OOB access) | Trap. | `SandboxError::Trap`; `state_hash = 0`. |

Two consequences a module author should plan around:

**A failed tick is reported as `state_hash = 0`, not as a crash.** The host logs
the error and keeps going. The replay verifier then reads `0` as a divergence
from the re-simulation and the match is provably unverifiable from that tick on.
That is deliberate: a sandbox failure is not allowed to look like a clean match.

**The host does not tear down the instance after a trap.** It refuels and calls
`mag_step` again on the next tick. A module that mutated half its state before
trapping resumes from that half-mutated state. Where it matters, mutate into
fresh storage and commit at the end of the step.

## 7. Checking your module

`magnetite-sandbox` ships a conformance harness. It loads any module — `.wasm`
or `.wat` — into the real sandbox, with the same engine configuration, the same
stub linker and the same resource limiter the production host uses, and reports
one line per contract clause.

```sh
cargo run -p magnetite-sandbox --bin mag-conformance -- path/to/game.wasm
```

Add `--json` for machine-readable output; `--ticks`, `--seed`, `--players`,
`--fuel` and `--memory-mib` tune the run. Exit status is 0 when nothing failed,
1 when something did.

What it decides by observation:

| Group | Checks |
|---|---|
| Shape | compiles under fuel + epoch; imports lie inside the host surface; `memory` exported; all eight functions present with exact signatures |
| Version | `mag_abi_version()` returns the version this host speaks |
| Framing | `mag_alloc` returns a writable in-bounds region; `mag_step`, `mag_snapshot` and `mag_view` prefixes stay inside linear memory; `mag_free` accepts a buffer it returned |
| Schema | the `mag_step` payload is JSON and decodes as a `StepOutput`, and its `tick` is the tick that was requested |
| Determinism | two fresh instances, same seed, same inputs, N ticks → identical `state_hash` sequences |
| Round-trip | `restore(snapshot())` is idempotent; a snapshot restored into a *fresh* instance resumes the same trajectory, on the right tick |
| Limits | a 1-unit fuel budget stops the module; a cap below its declared memory refuses instantiation; an expired epoch deadline interrupts it |

And what it measures without judging:

- whether `state_hash` actually advances across ticks;
- whether changing `MatchConfig.seed` changes anything, i.e. whether the module
  consumes the only entropy it is given;
- whether the module reads its `mag_step` payload at all — satisfied by either
  acting on the inputs *or* reporting rejects for them, since a module whose
  correct answer is "no" has still read the question.

A module can pass every mandatory check and still ignore its inputs entirely.
Those measurements are how you find out, and they are how the defects in §8 were
found.

`Status::Warn` never fails a run. It marks things that are legal under the
contract and almost certainly wrong in your module.

A run reporting no failures is evidence over the inputs the harness tried. It is
not a proof of determinism for all inputs, and it cannot be: only your own
property tests over your own state space can approach that.

## 8. Defects this contract uncovered

Writing the contract down, and running the harness against the shipping Rust
module, turned up seven defects. All seven are fixed. One design decision is
still outstanding, and it is the one thing on this page that is still broken.

Before the fixes, the shipping Rust module reported:

```text
WARN  determinism.seed_is_consumed       changing MatchConfig.seed changed nothing
WARN  mag_step.inputs_are_read           24 tick(s) with 4 players' inputs produce
                                         byte-identical hashes to 24 empty ticks
PASS  mag_restore.snapshot_is_idempotent restore(snapshot()) then snapshot()
                                         reproduces the same 41 bytes
FAIL  mag_restore.resumes_trajectory     after restoring the tick-12 snapshot into a
                                         fresh instance the trajectories differ

28 pass · 1 fail · 2 warn · verdict: NON-CONFORMING
```

The juxtaposition of the last two lines is the lesson. Restoring a snapshot and
snapshotting again reproduces the same bytes — which is *also* exactly what a
module that ignores `mag_restore` entirely does. A round-trip only shows itself
as broken when the restored state has to **resume**.

Every one of these survived because the wasm/native parity test stepped both
executors with an empty input list. Two executors simulating nothing agree
perfectly.

### Fixed

**1. The reference guest discarded every input frame.** The host encodes the
`mag_step` payload as an array of `{"player_id": …, "input": …}` objects; the
guest deserialised an array of two-element tuples and fell back to an empty list
when that failed, which it always did — serde will build a *struct* from a JSON
array but never a *tuple* from a JSON object. Fixed by decoding into a struct,
which accepts **both** encodings: the object form magnetite's host writes, and
the array form wibbly's browser driver sends — which is why wibbly was working
all along. The silent fallback is gone too: an undecodable payload now traps
rather than quietly becoming "nobody pressed a key".

**2. `mag_restore` ignored the framing.** The guest handed the whole buffer,
prefix bytes included, to a JSON parser, and `NativeExecutor::restore` swallowed
the resulting error as a silent no-op. Restore did nothing at all. Fixed on both
halves: the framing is now unambiguous and prefix-free (§2), and `restore` no
longer fails open — see item 6.

**3. The tick counter lived outside the snapshot.** The guest's counter was a
`static mut` that `mag_snapshot` did not serialise and `mag_restore` did not
reset, so a restored instance resumed on the wrong tick. Fixed: the counter is
re-seeded from `ArenaSnapshot::tick` on every restore. `ArenaSnapshot` already
carried the tick, so nothing changed on the wire.

**4. Nothing in any production path ever called `on_join`.** Only unit tests did.
So `ArenaShooter` never had a player, `validate` returned `Unauthorized` for
every input frame in both the native and the sandboxed path, and the e2e suite
worked around it with a `NopGame` that needs no join. Fixing item 1 alone would
have changed nothing observable — the inputs would have arrived and then been
rejected.

Fixed where the two paths cannot drift apart: `NativeExecutor::step` now calls
`on_join` for every player id it has not seen since the last `init` or `restore`.
First sight is the join (§4.1), the sandbox inherits it because the guest wraps
the same executor, and `verify_replay` re-joins the same players in the same
order when it re-simulates. `on_join` is now required to be idempotent.

**5. The RNG carried a stream position no snapshot could capture.** A per-match
`DeterministicRng` advances as a game draws from it, and that position was in
neither `AuthoritativeGame::Snapshot` nor the executor's serialised state — so a
restored executor rewound its randomness to the start and diverged from the
executor the snapshot came from. Silently, and only for games that actually
draw. `NativeExecutor::restore` even re-seeded from the config, which made the
rewind look deliberate.

Fixed by removing the hidden state rather than serialising it: `StepCtx::rng` is
derived per tick from `(seed, tick)`, so RNG position is a pure function of the
tick and restoring the tick restores the randomness. This changes the
`state_hash` values a given match produces. No persisted replay logs exist and
re-simulation is self-consistent, so nothing downstream breaks — but a hash
recorded before this change will not reproduce after it.

**6. `restore` failed open.** `NativeExecutor::restore` wrapped its decode in
`if let Ok(…)`, so a snapshot it could not read left the executor silently on its
previous state while every caller believed the transfer had landed. In shard
handoff that is authority moving to a node holding the wrong world. Fixed:
`restore` panics rather than pretending, a new
`GameExecutor::try_restore` returns a typed `RestoreError` for callers that
must survive a bad snapshot, and the one production caller — the shard handoff in
`magnetite-runtime/src/shard.rs` — now abandons the handoff and leaves authority
with the source.

**7. The host could not classify its own resource failures.**
`WasmExecutor::classify_trap` chose between `FuelExhausted`, `EpochTimeout`,
`MemoryLimitExceeded` and `Trap` by substring-matching `err.to_string()`. But
`wasmtime::Error` is an `anyhow::Error` whose `Display` is the outermost context
— `"error while executing at wasm backtrace: …"` — so the words `fuel`, `epoch`
and `memory` were never present and every resource failure came out as a generic
`Trap`. Its unit tests passed because they asserted against hand-written strings
wasmtime never produces: green and wrong at the same time.

Fixed by downcasting to `wasmtime::Trap` and matching the variant, with the whole
error chain reported for non-trap failures. A regression test now wraps a trap in
a backtrace context and asserts the classifier still sees through it. The harness
keeps its own `conformance::trap_reason` — no longer a workaround, but because it
calls wasmtime directly rather than through `WasmExecutor` and needs the reason
as text rather than as a `SandboxError`.

**8. `abi.rs`'s module documentation contradicted its own code.** It said the
host writes payloads at `ptr + 4` from `mag_alloc(4 + payload_len)` — true of
`mag_restore`, false of `mag_init` and `mag_step`, which allocate exactly
`payload_len` and write at `ptr + 0`. Corrected in place.

After the fixes, both the Rust module and the hand-written WAT module report
`34 pass · 0 fail · 0 warn`.

### The two decisions that were owed — both now taken

§8 previously closed with two open questions. Both are settled, and settling them
made this ABI's first breaking change. `mag_abi_version` exists so that it is also
the last one that can happen quietly.

**The tick now crosses `mag_step`.** The payload became
`{"tick": N, "inputs": [...]}` and the `StepOutput` gained a required `tick` field
that the host checks. Three things stopped being true as a result: a module no
longer has a second source of truth for the tick, a mis-sequenced step is now
something a guest can refuse, and a module that restores a snapshot without
adopting its tick is now caught on its very next step instead of diverging
quietly. That last one is the real prize — it converts "silently wrong" into an
error message, and it is exactly the class of bug this whole document was written
in response to.

The deciding factor was cost, not elegance: wibbly has to re-vendor its module
anyway to pick up the input, join and RNG fixes, so the marginal cost of a
breaking signature change was near zero. Breaking changes are cheapest when
something is already broken.

**`mag_restore` now takes bare JSON.** The prefix is gone, and with it the only
inbound framing asymmetry in the ABI. `len` was always a parameter, so the prefix
never carried information — it only created a second way to describe the same
buffer, which is what let two drivers disagree indefinitely. The dual-accepting
discriminator both reference modules briefly carried is gone too, as is the
harness's `mag_restore.tolerates_unframed_payload` check. wibbly was already
sending bare JSON and needs no change for this.

### Resolved ambiguities
### Resolved ambiguities

Specified above rather than left open: buffer lifetime is stated against *any*
`mag_*` call rather than the same export (§3); `mag_alloc` failure is a trap,
because there is no null convention to signal with (§3); `memory.grow` past the
cap returns `-1` rather than trapping, because that is what the resource limiter
does (§6).

## 9. A conforming module in 300 lines of nothing

[`magnetite-sandbox/conformance/reference.wat`](https://github.com/vul-os/magnetite/blob/main/magnetite-sandbox/conformance/reference.wat)
is a complete conforming module written directly in WebAssembly text format. It
has no dependencies, no language runtime, no allocator library and no imports. It
holds two 64-bit words of state, folds everything it is handed into a hash, emits
the three payload shapes, and round-trips its own snapshot. It is not a game —
its "rules" are a hash fold — but it passes every mandatory check in the harness,
including determinism across instances and snapshot-restore into a fresh
instance.

It is in the tree for one reason: it is the cheapest possible refutation of the
idea that this boundary is Rust-shaped. If hand-written Wasm can satisfy it,
your language can.

The shape to copy, in whatever language you are using:

```text
state:  a bump cursor, plus your simulation state, plus the last tick you ran
mag_abi_version(): return 1
mag_alloc(n):      bump the cursor, return the old value; trap if full
mag_free(p, n):    { }
mag_init(p, n):    parse MatchConfig JSON at [p, p+n); seed your PRNG; tick = 0
mag_step(p, n):    parse {"tick":T,"inputs":[…]} at [p, p+n) ← before you reset anything
                   trap if T <= your last tick
                   advance the simulation to tick T
                   reset the bump cursor
                   emit {"rejects":[…],"state_hash":H,"tick":T} with a u32 LE length in front
mag_snapshot():    emit your whole state — tick included — prefixed
mag_restore(p, n): parse [p, p+n); replace your state; adopt the snapshot's tick
mag_view(id):      emit only what player `id` may see, prefixed
```

## 10. Where this sits

- [Architecture](./docs.html#architecture) — how the sandbox fits into the tick
  loop, and how replay verification consumes `state_hash`.
- [Status](./docs.html#status) — what is running, LAN-only, mocked or absent.
- `magnetite-sandbox/src/executor.rs` — the host side of every rule above.
- `magnetite-sandbox/src/abi.rs` — the payload types and `MAG_ABI_VERSION`.
- `magnetite-sandbox/src/conformance.rs` — the harness, and the machine-readable
  version of this contract.
- `game-templates/authoritative/src/wasm_abi.rs` — the Rust reference guest.

## 11. Migrating an existing module

This version of the ABI is the first that declares itself, and it arrived with a
breaking change. If you have a module built against the older, undeclared ABI, it
will be **refused at load** — `mag_abi_version` is missing, so the host reports
`MissingExport` and the match never starts. That refusal is the feature: the
alternative is a module being handed payloads it silently misreads.

To migrate:

1. **Export `mag_abi_version() -> i32` returning `1`.** Three lines.
2. **Read `tick` from the `mag_step` payload** instead of counting calls. The
   payload is now `{"tick": N, "inputs": [...]}`; what used to be the whole
   payload is now the `inputs` field.
3. **Add `"tick"` to your `StepOutput`**, set to the tick you just simulated. The
   host rejects the step if it disagrees.
4. **Put the tick in your snapshot and adopt it in `mag_restore`.** Without this,
   step 3 will start failing immediately after any restore — which is the point.
5. **Stop expecting a length prefix in `mag_restore`.** The payload is bare JSON;
   `len` is the payload length. If you were already treating it as bare JSON, you
   have nothing to do here.

### The specific case: wibbly

[wibbly](https://github.com/vul-os/wibbly)'s `@vulos/wibbly-authority` package
drives this exact module in a browser, from a **vendored** `.wasm` at
`public/magnetite/arena-authority.wasm` with no build step. It needs a re-vendor,
and not only for the ABI change:

| Change | Does wibbly need action? |
|---|---|
| `mag_abi_version` export | **Yes** — comes free with a rebuilt module; no driver change. |
| `tick` in the `mag_step` payload | **Yes** — the driver must send `{"tick":N,"inputs":[…]}`. It currently sends the bare array. |
| `tick` in the `StepOutput` | No driver change; it already ignores fields it does not read. |
| `mag_restore` framing | **No** — wibbly already sends bare JSON with a bare `len`. It was right and the host was the one that changed. |
| Inputs being discarded (§8 item 1) | **Yes, and this is the urgent one** — comes free with a rebuilt module. |
| `on_join` never called (§8 item 4) | **Yes** — comes free with a rebuilt module. Its `restorePlayers` seeding keeps working, because `on_join` is now idempotent. |
| Per-tick RNG (§8 item 5) | No driver change, but **`state_hash` values change**. Any test pinning a hash literal will need updating; tests asserting positions will not. |

Two notes for whoever does the re-vendor.

**Its inputs may have been working, by luck.** The driver serialises
`Array<[number, Input]>`, which produces `[[1, {…}]]` — the two-element array
form. The old guest wanted exactly that, so wibbly was *not* affected by the
input-discarding defect that broke magnetite's own host. The current guest accepts
both encodings, so this keeps working; but it now also needs the surrounding
`{"tick":…}` object, which it does not currently send.

**The vendored `.wasm` records no provenance, and that is its own problem.** There
is no magnetite commit, tag, or module hash recorded next to it, so there is no way
to tell which version of the guest is shipping in a browser — which is precisely
how a driver and a module can disagree for months without anyone noticing. Before
this change there was no version to record; now there is. **Recommendation for the
re-vendor:** write a small manifest next to the `.wasm` recording the magnetite
commit it was built from, the module's `sha256`, and the `mag_abi_version` it
declares, and have the driver assert the version it expects at load. Any one of
those three would have made the framing disagreement a five-minute diagnosis.

magnetite does not modify the wibbly repo, and this section is a specification of
what is needed, not a change log of work done there.
