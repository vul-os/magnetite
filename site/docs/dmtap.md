# DMTAP: the integration surface (and what is actually built)

> **Status: NOT INTEGRATED.** Magnetite does not depend on DMTAP, does not link
> `dmtap-core`, and does not speak any DMTAP wire format. This page documents
> *where* DMTAP would plug in if it were adopted, and is explicit about the one
> thing that is built today — a word-based naming provider that borrows DMTAP's
> *concept* and none of its code or encoding.

## Why this page exists

DMTAP (the decentralized mail/identity protocol behind Envoir) covers ground
that overlaps several Magnetite seams:
decentralized login with key transparency and rotation, a naming ladder whose
floor is an authority-free word-name for a raw key, and a content-addressed
object/distribution substrate (MOTE / DMTAP-PUB).

Magnetite's architecture (`DECENTRALIZATION.md` §3) is built as six pluggable
seams precisely so that a protocol like DMTAP can be *an option* rather than a
foundation. The governing rule, from §3:

> **Every seam ships a non-DMTAP default so we never hard-depend on any external
> project.**

That rule holds today, and this page is the receipt.

## Which seams DMTAP would plug into

| Seam | Trait | Working non-DMTAP default (shipped) | What DMTAP could provide |
|------|-------|--------------------------------------|--------------------------|
| §3.1 Identity / Auth | `Identity`, `AuthProvider` | `RawKeypairAuth` — raw Ed25519 challenge/response, zero external services | DMTAP-Auth: decentralized login, key transparency, key rotation/recovery |
| §3.2 Naming | `Naming` | `HashNaming` — raw-hex canonical addresses + `mag_<hash>` short handles | The `name@domain` naming ladder with an 8-word zero-authority floor |
| §3.3 BlobStore | `BlobStore` | `LocalBlobStore`, `HttpBlobStore` — content-addressed, hash *is* the id | MOTE objects distributed over the DMTAP-PUB substrate |
| §3.5 Comms | `CommsProvider` | `BuiltinProvider` (plus shipped Matrix, Jitsi, LiveKit, Owncast adapters) | DMTAP messaging as one more provider |

Every row's default works **fully offline** — no network, no chain, no
homeserver — which is why CI runs with no external service at all. DMTAP is
therefore optional and, by construction, never load-bearing: removing it (or
never adding it) costs Magnetite no capability it has today.

## What is implemented today

**`magnetite_seams::keyname::KeyNameNaming`** — a second, real `Naming`
implementation behind the `dmtap` cargo feature (**off by default**; the default
build is byte-identical and gains no dependencies).

It renders an Ed25519 public key as words from an embedded 2048-word list
(11 bits per word), with an optional `<words>@domain` display for keys that
carry a local domain hint. It is zero-authority: the name is derived purely
from the key — no registry, no server, no chain.

Two forms, because the arithmetic demands it:

| Form | Words | Bits | Invertible? |
|------|-------|------|-------------|
| `short_name` | 8 | 88 (BLAKE3 fingerprint of the key) | **No.** Resolves only for keys the node has *learned* |
| `full_name` | 24 | 256 key bits + 8 checksum bits = 264 | **Yes.** A pure encoding; exact round-trip for every key |

Eight words carry 88 bits; an Ed25519 key is 256 bits. **No 8-word scheme can be
a lossless encoding of a key** — not this one, and not DMTAP's. The 8-word form
is a fingerprint/display name: you learn a key out of band (a signed session ad,
a contact exchange, a message you verified) and it then resolves under its
8-word name. The 24-word form is a complete, transcribable key, structured like
a BIP-39 mnemonic (payload + checksum, 11 bits per word) though it does not use
BIP-39's wordlist.

### The wordlist

Derived mechanically from `/usr/share/dict/words` (the **Webster's Second
International** "web2" list shipped with BSD/macOS), which is **public domain**
— the 1934 edition's copyright has expired and BSD distributes it without
licence restrictions. No BIP-39 wordlist is vendored, so no Bitcoin-project
licensing question arises. The derivation is documented and reproducible in
`magnetite-seams/src/keyname/wordlist.rs`: keep `^[a-z]{4,7}$` words, keep only
the first word for each 4-character prefix (so every word is unambiguous from
its opening), sort and de-duplicate to 15 491 candidates, then take 2048 evenly
spaced across that list so names span the whole alphabet. The list's invariants
(exactly 2048, sorted, unique, unique 4-char prefixes, ASCII lowercase) are
asserted by tests, so a corrupted list fails CI rather than silently renaming
every key.

### Why it is not called `DmtapNaming`

`DECENTRALIZATION.md` §3.2 sketches an optional `DmtapNaming`. This provider is
deliberately **not** given that name. DMTAP uses the same *concept* — an 8-word
key-name as the zero-authority floor of a `name@domain` ladder — but
`dmtap-core` is not available on this machine and is not a published crate, so
its exact wordlist, bit packing, checksum and separator could not be read.
Names produced by `KeyNameNaming` will almost certainly **not** match names
produced by DMTAP for the same key. Claiming compatibility that cannot be
verified would be a lie in a type name, which is the worst place to put one.

`KeyNameNaming` is therefore best understood as **the proof that the slot
exists**: it demonstrates the `Naming` seam accepts a second, structurally very
different implementation, and it is exactly where a real `dmtap-core`-backed
provider drops in — implementing the same trait, changing nothing else.

## What awaits the real `dmtap-core` crate

Explicitly **not built**, and not to be described as built:

- **`DmtapAuth`** (§3.1). No DMTAP-Auth login, no key transparency log, no key
  rotation or recovery. `RawKeypairAuth` is the only shipped auth provider.
- **DMTAP-compatible names** (§3.2). See above — the encoding is ours, not
  theirs, and is not wire-compatible.
- **`DmtapPubBlobStore`** (§3.3). No MOTE object encoding, no DMTAP-PUB
  transport. Blobs are local or fetched by hash over HTTP.
- **A DMTAP comms provider** (§3.5).
- **Any DMTAP dependency at all.** `magnetite-seams/Cargo.toml` lists no DMTAP
  crate. The `dmtap` cargo feature adds *zero* dependencies — it only compiles
  one extra module — and is named for the concept it anticipates, not for an
  integration it performs.

## The rule that keeps the substrate clean

**Names are a display layer over raw keys. The substrate is always the raw
Ed25519 public key.**

Nothing in the runtime, scheduler, discovery, or payment path makes an
authorization decision against a name. Signatures verify against `PubKey`;
discovery ads are keyed on `PubKey`; payment receipts settle to `PubKey`. A
naming provider can only change how a key is *shown* and what strings map back
to one, and resolution is fail-closed everywhere: unknown names, unknown words,
bad checksums, and mismatched domain hints all return `None`/`Err` rather than
guessing. That is what makes swapping — or never adopting — a naming protocol a
cosmetic change.

## Trying it

```bash
cargo test -p magnetite-seams                    # default build: feature off
cargo test -p magnetite-seams --features dmtap   # + KeyNameNaming and the seam-pluggability proofs
```

`magnetite-seams/tests/seam_pluggability.rs` drives one unmodified consumer
function against both `HashNaming` and `KeyNameNaming` and asserts identical
behaviour — the concrete evidence that the `Naming` seam is not hardwired to its
default.
