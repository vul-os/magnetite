<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>
<style>
.mg-s{font-family:var(--doc-mono);font-size:.7rem;font-weight:600;letter-spacing:.07em;text-transform:uppercase;white-space:nowrap}
.mg-s.live{color:#17803D} .mg-s.lan{color:#A45B00} .mg-s.mock{color:var(--mg-bnd)} .mg-s.no{color:#737C90}
:root[data-theme="dark"] .mg-s.live{color:#6EE79B}
:root[data-theme="dark"] .mg-s.lan{color:#FFC24D}
:root[data-theme="dark"] .mg-s.mock{color:#FF74B2}
:root[data-theme="dark"] .mg-s.no{color:#8892A6}
.mg-plate{margin:1.9rem 0;border:1px solid var(--dv-border);border-radius:10px;overflow:hidden;background:var(--dv-surface);box-shadow:var(--dv-shadow-sm)}
.mg-cap{padding:11px 15px;border-top:1px solid var(--dv-border);background:var(--dv-code-bg);font-family:var(--doc-mono);font-size:.76rem;line-height:1.6;color:var(--dv-ink-3)}
.mg-cap b{color:var(--accent);font-weight:600;letter-spacing:.09em;text-transform:uppercase;font-size:.76rem;display:block;margin-bottom:3px}
.mg-cap.edge b{color:var(--mg-bnd)}
:root[data-theme="dark"] .mg-cap.edge b{color:#FF74B2}
.mg-plate pre{margin:0;border:0;border-radius:0}
</style>

# Web bundles — hosting a Godot, Unity or three.js build

Hosting a web game and hosting an authoritative match are **two unrelated
problems**, and treating them as one is what makes the second one's cost look
like the first one's.

| | What it is | What it needs |
|---|---|---|
| **Hosting a web build** | serve content-addressed files over HTTP, check an entitlement receipt | a socket |
| **Hosting an authoritative match** | run the simulation and be trusted about it | the node, wasmtime, ticks, shards, migration |

This page is about the first one — **rung 0** of the
[capability ladder](./docs.html#architecture). No VM, no tick loop, no authority,
no anti-cheat, no determinism. It lives in the `magnetite-web-host` crate and it
needs no central service, no coordinator, no chain and no network beyond its own
listening socket. A developer with a laptop runs the whole thing alone.

## Status

> [!WARNING]
> **No real Godot 4, Unity or three.js export has been served through this
> yet.** No engine toolchain was available on the machine it was written on, so
> the tests run against a *fixture that mimics the Godot 4 web-export file
> layout* — `index.html`, `index.js`, `index.wasm`, `index.pck`, a `.wasm.br` and
> a `.pck.gz`. The `.wasm` in that fixture is a magic number, not an engine.
>
> What *is* verified in a real browser is the precondition Godot 4 fails on: a
> headless Chromium loading a bundle from this server reports
> `crossOriginIsolated === true` and constructs a `SharedArrayBuffer`, and the
> same bundle served with isolation off reports `false` and cannot. That is
> necessary, not sufficient. Treat engine compatibility as *expected from the
> spec*, not *demonstrated*.

| Capability | State |
|---|---|
| Content-addressed multi-file bundle, root hash over the sorted `path → hash` list | <span class="mg-s live">Running</span> |
| COOP + COEP on the document and every subresource | <span class="mg-s live">Running</span> |
| `crossOriginIsolated` / `SharedArrayBuffer` confirmed in real Chromium | <span class="mg-s live">Running</span> |
| Precompressed `.br` / `.gz` with the right `Content-Encoding` + `Content-Type` | <span class="mg-s live">Running</span> |
| HTTP range requests — `Accept-Ranges`, `206`, `416` | <span class="mg-s live">Running</span> |
| Per-file BLAKE3 verification, fail closed | <span class="mg-s live">Running</span> |
| Entitlement gate on a paid bundle | <span class="mg-s mock">Mock only</span> — the gate is real; the only rail it has been exercised against is `MockPaymentRail` |
| Booted from a real engine export | <span class="mg-s no">Not built</span> |
| Signed manifests (canonical CBOR) | <span class="mg-s no">Not built</span> |
| TLS on the bundle host | <span class="mg-s no">Not built</span> — see [Secure context](#sec-secure-context-is-not-optional) |
| Receipt→session token exchange | <span class="mg-s no">Not built</span> — see [The cost of receipt-per-request](#sec-the-cost-of-receipt-per-request) |

## Try it

```
cd magnetite-web-host
cargo run --bin serve-web-bundle -- /path/to/your/export
```

Every file under the directory is read, hashed, and listed in a manifest. The
root hash of that manifest is the URL:

```
bundle root  8f2c…                      (BLAKE3 over the sorted file list)
files        9 (5312 bytes)
entry        index.html
pricing      free
isolation    COOP same-origin + COEP require-corp
listening    http://127.0.0.1:8080/pkg/8f2c…/
```

Nothing is written to disk, no database is opened, and no account exists.

## The URL is the version

```
/pkg/<root-hash>/            the entry document
/pkg/<root-hash>/<path>      one file
```

The root hash is BLAKE3 over the **sorted** `path → (hash, length)` list, plus
the bundle kind and the entry path — not over a tarball. That choice buys three
things a single archive hash cannot:

* **Per-file caching.** Each file gets its own strong `ETag` (its content hash),
  so a patch that changes one texture does not invalidate the 40 MB pack.
* **Range serving.** A `206` can be answered without rehydrating an archive.
* **`Cache-Control: immutable`, correctly.** The bytes at one of these URLs can
  never change, because the URL commits to the manifest and the manifest commits
  to every file's bytes. A new build is a new root hash is a new URL, so a stale
  cache entry is *unreachable* rather than *wrong*. Compare `nginx.conf`, which
  must send `no-cache` for `/index.html` because that name is mutable.

A node can therefore host any number of versions of a game at once with no
version table.

### One caching trap worth naming

`Cache-Control: public` on a **paid** bundle is a hole. `public` invites shared
caches — a CDN, a corporate proxy — to store the response and hand it to the
next requester, and a shared cache does not re-run the entitlement check. One
entitled fetch would populate a cache that then serves the paid bundle to
everyone behind it. So paid bundles get `private, max-age=…, immutable` plus a
`Vary` on the credential headers; only free bundles get `public`.

## COOP / COEP — and the constraint that comes with it

A Godot 4 web export needs `SharedArrayBuffer`. The browser exposes
`SharedArrayBuffer` only to a **cross-origin isolated** document, and a document
is cross-origin isolated only when it is a secure context *and* it was served
with both of:

```
Cross-Origin-Opener-Policy:   same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Nothing in `nginx.conf`, `backend/src` or `frontend` sets either, which is the
classic Godot-on-itch.io failure: the export is fine, the host is wrong, and the
page just never starts.

<div class="mg-plate">
<pre><code>GET /pkg/8f2c…/ HTTP/1.1

HTTP/1.1 200 OK
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Resource-Policy: same-origin
X-Content-Type-Options: nosniff
X-Magnetite-Determinism: not-replay-verifiable
Cache-Control: public, max-age=31536000, immutable
Content-Type: text/html; charset=utf-8</code></pre>
<div class="mg-cap"><b>The document, and every subresource under it</b>COOP/COEP on the document alone is not enough — under COEP a subresource served <em>without</em> the headers is blocked, so the <code>.wasm</code> and the <code>.pck</code> would fail to load behind a perfectly correct document. Error responses carry them too, or a 404 inside an isolated context is blocked before the page can read its status and the developer sees a network error instead of the path they got wrong.</div>
</div>

### Bundles must be self-contained or same-origin

This is the price, and it is not a policy magnetite chose — it is what
`SharedArrayBuffer` costs anywhere, for everyone.

`Cross-Origin-Embedder-Policy: require-corp` earns isolation by promising the
browser that the document embeds nothing which has not opted in. Under it,
**every cross-origin subresource that does not send
`Cross-Origin-Resource-Policy` or a permissive CORS response is blocked** — and
blocked quietly, as far as the page is concerned.

| In your bundle | Under COEP |
|---|---|
| a font from Google Fonts | **blocked** |
| a texture or model from a CDN | **blocked** |
| an analytics or ads script | **blocked** |
| a cross-origin `<iframe>` | **blocked** |
| anything served from the bundle itself | fine |

So: **vendor your assets into the bundle.** That is also what makes a bundle
content-addressable end to end — a bundle whose behaviour depends on a
third-party host is not reproducible from its root hash, so the constraint and
the design want the same thing.

The escape hatch is `--no-isolation`, which drops both headers. Cross-origin
subresources then work and `SharedArrayBuffer` is unavailable. A typical
three.js scene is fine that way. **A Godot 4 export will not boot.**

### Secure context is not optional

`crossOriginIsolated` also requires a **secure context**: `https://`, or
`http://localhost` / `127.0.0.1`. On any other address, a bundle served over
plain HTTP gets its COOP and COEP headers, is *not* isolated, and Godot 4 still
will not boot. There is no TLS in the bundle host today — put a reverse proxy or
tunnel in front of it. This is the same blocker the node's WebSocket has.

## Compression

Godot and Unity ship precompressed assets, and getting the header pair wrong
fails in one of two silent ways. For a URL ending in `.br` or `.gz`, the
compression suffix is a **transfer** property and the remaining extension is the
**content** property:

| Stored file | `Content-Type` | `Content-Encoding` |
|---|---|---|
| `index.wasm` | `application/wasm` | — |
| `index.wasm.br` | `application/wasm` | `br` |
| `index.pck.gz` | `application/octet-stream` | `gzip` |
| `g.framework.js.br` | `text/javascript; charset=utf-8` | `br` |

* Omit `Content-Encoding: br` and the browser hands brotli bytes to
  `WebAssembly.instantiateStreaming`, which rejects on the magic number.
* Send `application/octet-stream` on a `.wasm` and `instantiateStreaming`
  refuses it before decoding anything — it requires `application/wasm`.

Both look like "the game just doesn't start".

Two request shapes are supported because both exist in the wild:

* **Unity** requests `Build/g.wasm.br` by that literal URL and depends entirely
  on the server to declare the encoding. Always handled.
* **Godot behind a compressing proxy** requests `index.wasm` and expects the
  server to substitute a stored `index.wasm.br` when the client can read it.
  Handled, with `Vary: Accept-Encoding` — mandatory here, or a cache populated
  by a brotli-capable client would serve brotli to one that is not.

A client that explicitly refuses the only stored coding gets a `406`, not bytes
it cannot read.

## Range requests

`Accept-Ranges: bytes` on every full response, `206` with `Content-Range` for a
satisfiable range, `416` with `bytes */<total>` for one that is not. Open-ended
(`bytes=1024-`), suffix (`bytes=-512`) and past-EOF-clamped forms all work,
because those are what loaders and resumed downloads actually send.

Two deliberate non-behaviours:

* A multi-range request gets a full `200`, not a `206` carrying only the first
  range. Answering a multi-range with one range while claiming `206` silently
  corrupts the client's buffer; ignoring `Range` is always legal.
* A range is not served for a *negotiated* compressed substitution. `Content-Range`
  offsets are defined over the encoded bytes, so serving a slice of the brotli
  stream to a request that meant the wasm stream would be quietly wrong.

## Integrity — fail closed

Every file is re-hashed against the manifest **before any byte is written to the
socket**. A file that does not match is a `502` with
`X-Magnetite-Integrity: failed` and no content, not a `200`. A file in the
manifest whose blob this node does not hold is a `503` — incompletely seeded, and
retryable — not an empty `200`.

Two details that matter:

* **Ranges do not weaken it.** The complete representation is verified and *then*
  sliced. Hashing only the requested slice would let a tampered file be
  exfiltrated range by range, each slice verifying against nothing.
* **The check is not delegated to the blob store.** `FsBlobStore` and
  `HttpBlobStore` already re-verify on read, but `BlobStore` is a pluggable seam
  and a third-party backend is under no obligation to. Verifying at the serving
  layer means the guarantee holds for every backend rather than the two that
  happen to implement it today.

Path traversal is not filtered, it is structurally absent: there is no
`Path::join`, no `canonicalize` and no filesystem access on a request path
anywhere in the serving path. A request either names a byte-identical key the
manifest already listed, or it misses.

## The entitlement gate

A paid bundle serves **nothing** — not even the entry document — without a
receipt that the payment rail accepts for the bundle's item. The receipt *is* the
entitlement, and the check fails closed at every branch that cannot prove
otherwise:

| Situation | Status |
|---|---|
| free bundle | served; the rail is never consulted |
| paid, no receipt | `402` |
| paid, receipt does not verify for this item | `403` |
| paid, receipt bought by someone other than the authenticated requester | `403` |
| paid, **no payment rail configured** | `503` |

That last row is the one worth dwelling on. The tempting behaviour — "no rail
configured, so treat everything as free" — is a fail-open reachable by a missing
environment variable, and it gives away the paid catalogue. A node that cannot
evaluate entitlement serves no paid content.

An unentitled request for a path that exists and one for a path that does not are
**indistinguishable**: the gate runs before path resolution, so the file list is
part of what was paid for and nobody can enumerate a bundle for free.

### Why the receipt travels in a cookie

A custom `X-Magnetite-Receipt` header only covers fetches the page's own script
issues. It does not cover the requests that matter: the browser fetches
`index.wasm`, `index.pck` and every texture *itself*, and it does not attach
custom headers to those. Gate a paid bundle on a header alone and the document
loads while every asset `402`s — a blank canvas that looks like a broken gate.

A cookie scoped to the bundle's path is attached automatically to same-origin
subresource requests, which is the only mechanism that covers them. Both forms
are accepted.

### The cost of receipt-per-request

The gate runs on **every** file request, because every file is content the buyer
paid for and there is no session token in this design. With `MockPaymentRail`
that is a local signature check and free. With a chain rail whose verification
re-reads the chain, it is **one RPC per asset**, and a Godot export is hundreds
of assets. That does not work.

The fix is a short-lived scoped token minted once from a verified receipt and
checked locally after. It is **not built**: it is new protocol and it needs a key
the node holds, and inventing it inside a file server is the wrong place. What
*is* built is the boundary — entitlement evaluation is one function with one call
site, so the exchange has exactly one place to land. Until then, free bundles are
fully usable and paid bundles are usable with a rail whose verification is local.

## Rung 0 is not rung 2

A Godot or three.js game is arbitrary JS and wasm running in the player's
browser. There is no authoritative step function, no `ReplayLog`, and nothing to
re-simulate — so it **cannot be replay-verified**, and it must not inherit the
claims that the authoritative node earns. Every response says so:

```
X-Magnetite-Determinism: not-replay-verifiable
```

The manifest's determinism field has exactly one possible value, on purpose:
there is no way to spell "this web bundle is replay-verifiable", because there is
no code that could check it. The same precedent is already enforced for
`InputClass::Attested` on camera gestures.

A bundle that wants determinism, replay and anti-cheat writes its *rules* as a
small wasm module against the seven-export `mag_*` ABI — in any language that
targets `wasm32-wasip1`, not necessarily Rust — and keeps its existing renderer.
That is rung 1, and it is the upsell no other web-game host can offer.

## Verifying it yourself

```
cd magnetite-web-host && cargo test          # 55 tests, fully offline
node scripts/verify-web-bundle-isolation.mjs  # real Chromium, 21 checks
```

The browser script includes a **negative control**: it serves the same bundle
with isolation off and requires `crossOriginIsolated === false` and a failing
`new SharedArrayBuffer(8)`. Without that, a pass would be consistent with the
browser isolating the document for some unrelated reason and the headers doing
nothing.
