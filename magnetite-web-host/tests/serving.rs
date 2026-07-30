//! End-to-end tests over a real HTTP/1.1 socket, fully offline.
//!
//! Each test names the failure it prevents rather than the function it calls.
//! Every one of them is the offline half of a claim; the browser half — that a
//! document served this way is actually cross-origin isolated — is
//! `scripts/verify-web-bundle-isolation.mjs`.
//!
//! **No real Godot 4 / Unity / three.js export is exercised here.** See
//! `tests/common/mod.rs` for why and for what the fixture does and does not
//! establish.

mod common;

use common::{Req, PCK_LEN};
use magnetite_seams::blobstore::{BlobStore, Hash};
use magnetite_web_host::respond::{CrossOriginIsolation, ServePolicy};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// 1. COOP / COEP — the headers Godot 4 cannot boot without
// ---------------------------------------------------------------------------

/// The headline claim. ALIGNMENT.md §5: "Nothing in `nginx.conf`, `backend/src`
/// or `frontend` sets either. This is the classic Godot-on-itch.io failure."
#[tokio::test]
async fn the_entry_document_carries_coop_and_coep() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(h.base()).send(h.addr).await;

    assert_eq!(r.status, 200);
    r.expect_header("Cross-Origin-Opener-Policy", "same-origin");
    r.expect_header("Cross-Origin-Embedder-Policy", "require-corp");
    r.expect_header("Content-Type", "text/html; charset=utf-8");
    assert_eq!(
        r.body,
        h.bytes_of("index.html"),
        "entry resolves to index.html"
    );
}

/// COEP on the document alone is not enough: a subresource served without the
/// headers is blocked inside an isolated context, so the wasm and the pack would
/// fail to load even with a correct document.
#[tokio::test]
async fn every_subresource_carries_the_isolation_headers_too() {
    let h = common::spawn(false, ServePolicy::default()).await;
    for path in [
        "index.js",
        "index.wasm",
        "index.pck",
        "index.worker.js",
        "index.audio.worklet.js",
        "index.icon.png",
    ] {
        let r = Req::get(format!("{}{path}", h.base())).send(h.addr).await;
        assert_eq!(r.status, 200, "{path}");
        r.expect_header("Cross-Origin-Opener-Policy", "same-origin");
        r.expect_header("Cross-Origin-Embedder-Policy", "require-corp");
        r.expect_header("Cross-Origin-Resource-Policy", "same-origin");
        r.expect_header("X-Content-Type-Options", "nosniff");
    }
}

/// An error response inside an isolated context that lacks COEP is blocked by the
/// browser before the page can read its status — the developer sees a network
/// error instead of the 404 naming the path they got wrong.
#[tokio::test]
async fn error_responses_carry_the_isolation_headers() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}nope.png", h.base())).send(h.addr).await;
    assert_eq!(r.status, 404);
    r.expect_header("Cross-Origin-Embedder-Policy", "require-corp");
    r.expect_header("Cache-Control", "no-store");
}

/// The documented escape hatch for bundles that need cross-origin assets more
/// than they need threads. A Godot 4 export will not boot in this mode.
#[tokio::test]
async fn isolation_can_be_turned_off_for_bundles_that_need_cross_origin_assets() {
    let h = common::spawn(
        false,
        ServePolicy {
            isolation: CrossOriginIsolation::Disabled,
            ..ServePolicy::default()
        },
    )
    .await;
    let r = Req::get(h.base()).send(h.addr).await;
    assert_eq!(r.status, 200);
    r.expect_no_header("Cross-Origin-Opener-Policy");
    r.expect_no_header("Cross-Origin-Embedder-Policy");
}

// ---------------------------------------------------------------------------
// 2. Precompressed assets — Content-Encoding paired with the right Content-Type
// ---------------------------------------------------------------------------

/// The exact pair from ALIGNMENT.md §5. A `.wasm.br` requested by that literal
/// URL — Unity's loader does this — must be `application/wasm` + `br`. Get the
/// type wrong and `WebAssembly.instantiateStreaming` refuses it; omit the
/// encoding and the browser hands brotli bytes to the wasm decoder.
#[tokio::test]
async fn a_precompressed_wasm_is_application_wasm_with_content_encoding_br() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.wasm.br", h.base()))
        .with("Accept-Encoding", "gzip, deflate, br")
        .send(h.addr)
        .await;

    assert_eq!(r.status, 200);
    r.expect_header("Content-Type", "application/wasm");
    r.expect_header("Content-Encoding", "br");
    assert_eq!(
        r.body,
        h.bytes_of("index.wasm.br"),
        "stored compressed bytes are transferred verbatim, never re-encoded"
    );
    // Sanity: the wire body really is brotli, i.e. shorter than the wasm and not
    // equal to it. Otherwise the test would pass on a server that ignored .br.
    assert_ne!(r.body, h.bytes_of("index.wasm"));
    assert!(r.body.len() < h.bytes_of("index.wasm").len());
}

/// Same rule, gzip, on a Godot resource pack.
#[tokio::test]
async fn a_precompressed_pck_is_octet_stream_with_content_encoding_gzip() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.pck.gz", h.base()))
        .with("Accept-Encoding", "gzip")
        .send(h.addr)
        .await;

    assert_eq!(r.status, 200);
    r.expect_header("Content-Type", "application/octet-stream");
    r.expect_header("Content-Encoding", "gzip");
    assert_eq!(r.body, h.bytes_of("index.pck.gz"));
}

/// An uncompressed file must NOT claim an encoding. A spurious
/// `Content-Encoding: br` on plain wasm is the same failure in reverse.
#[tokio::test]
async fn identity_assets_declare_no_content_encoding() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.js", h.base()))
        .with("Accept-Encoding", "gzip, br")
        .send(h.addr)
        .await;
    assert_eq!(r.status, 200);
    r.expect_header("Content-Type", "text/javascript; charset=utf-8");
    r.expect_no_header("Content-Encoding");
    assert_eq!(r.body, h.bytes_of("index.js"));
}

/// The Godot-behind-a-proxy shape: the loader asks for `index.wasm` and the
/// server substitutes the stored `index.wasm.br` when the client can read it.
/// `Vary: Accept-Encoding` is mandatory here — without it a cache populated by a
/// br-capable client would serve brotli to one that is not.
#[tokio::test]
async fn a_request_for_plain_wasm_is_answered_with_the_precompressed_variant() {
    let h = common::spawn(false, ServePolicy::default()).await;

    let br = Req::get(format!("{}index.wasm", h.base()))
        .with("Accept-Encoding", "gzip, deflate, br")
        .send(h.addr)
        .await;
    assert_eq!(br.status, 200);
    br.expect_header("Content-Type", "application/wasm");
    br.expect_header("Content-Encoding", "br");
    assert_eq!(br.body, h.bytes_of("index.wasm.br"));
    assert!(
        br.header("Vary").unwrap_or("").contains("Accept-Encoding"),
        "a negotiated response must Vary on Accept-Encoding, got {:?}",
        br.header("Vary")
    );

    // A client that cannot read brotli gets the identity bytes from the same URL.
    let id = Req::get(format!("{}index.wasm", h.base()))
        .with("Accept-Encoding", "gzip")
        .send(h.addr)
        .await;
    assert_eq!(id.status, 200);
    id.expect_no_header("Content-Encoding");
    assert_eq!(id.body, h.bytes_of("index.wasm"));

    // And a client sending no Accept-Encoding at all.
    let none = Req::get(format!("{}index.wasm", h.base()))
        .send(h.addr)
        .await;
    none.expect_no_header("Content-Encoding");
    assert_eq!(none.body, h.bytes_of("index.wasm"));
}

/// A URL that literally names a compressed file, requested by a client that says
/// it cannot decode that coding. Handing the bytes over anyway would be sending
/// something the client cannot read while claiming otherwise.
#[tokio::test]
async fn a_client_refusing_the_only_stored_coding_gets_406_not_broken_bytes() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.wasm.br", h.base()))
        .with("Accept-Encoding", "gzip")
        .send(h.addr)
        .await;
    assert_eq!(r.status, 406);
    assert!(r.body.len() < 200, "406 must not carry the asset");
}

// ---------------------------------------------------------------------------
// 3. Range requests — a 40 MB .pck needs them
// ---------------------------------------------------------------------------

/// The core range claim: `206`, correct `Content-Range`, and byte-exact content.
#[tokio::test]
async fn a_range_request_returns_206_with_exactly_the_requested_bytes() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let pck = h.bytes_of("index.pck");
    assert_eq!(pck.len(), PCK_LEN);

    let r = Req::get(format!("{}index.pck", h.base()))
        .with("Range", "bytes=1024-2047")
        .send(h.addr)
        .await;

    assert_eq!(r.status, 206);
    r.expect_header("Content-Range", &format!("bytes 1024-2047/{PCK_LEN}"));
    r.expect_header("Content-Length", "1024");
    r.expect_header("Accept-Ranges", "bytes");
    assert_eq!(
        r.body,
        pck[1024..2048],
        "the exact slice, not an off-by-one"
    );
}

/// The three forms a loader actually sends, over the wire.
#[tokio::test]
async fn open_ended_suffix_and_clamped_ranges_all_work() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let pck = h.bytes_of("index.pck");

    // Resumed download.
    let open = Req::get(format!("{}index.pck", h.base()))
        .with("Range", "bytes=4000-")
        .send(h.addr)
        .await;
    assert_eq!(open.status, 206);
    open.expect_header("Content-Range", &format!("bytes 4000-4095/{PCK_LEN}"));
    assert_eq!(open.body, pck[4000..]);

    // Trailing-index read.
    let suffix = Req::get(format!("{}index.pck", h.base()))
        .with("Range", "bytes=-16")
        .send(h.addr)
        .await;
    assert_eq!(suffix.status, 206);
    suffix.expect_header("Content-Range", &format!("bytes 4080-4095/{PCK_LEN}"));
    assert_eq!(suffix.body, pck[4080..]);

    // End past EOF is clamped, not refused — a 416 here breaks loaders that
    // request a fixed window before they know the size.
    let clamped = Req::get(format!("{}index.pck", h.base()))
        .with("Range", "bytes=4090-999999")
        .send(h.addr)
        .await;
    assert_eq!(clamped.status, 206);
    clamped.expect_header("Content-Range", &format!("bytes 4090-4095/{PCK_LEN}"));
    assert_eq!(clamped.body, pck[4090..]);

    // First byte only — the smallest legal range.
    let one = Req::get(format!("{}index.pck", h.base()))
        .with("Range", "bytes=0-0")
        .send(h.addr)
        .await;
    assert_eq!(one.status, 206);
    assert_eq!(one.body, b"G");
}

#[tokio::test]
async fn an_unsatisfiable_range_is_416_with_the_total_size() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.pck", h.base()))
        .with("Range", "bytes=99999-")
        .send(h.addr)
        .await;
    assert_eq!(r.status, 416);
    r.expect_header("Content-Range", &format!("bytes */{PCK_LEN}"));
}

/// A multi-range request must not get a `206` carrying only the first range —
/// that silently corrupts the client's buffer. A full `200` is the correct,
/// legal answer.
#[tokio::test]
async fn a_multi_range_request_is_answered_in_full_not_partially() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.pck", h.base()))
        .with("Range", "bytes=0-9,20-29")
        .send(h.addr)
        .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body, h.bytes_of("index.pck"));
}

/// `Accept-Ranges: bytes` is how a loader knows it may range at all.
#[tokio::test]
async fn full_responses_advertise_range_support() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.pck", h.base()))
        .send(h.addr)
        .await;
    r.expect_header("Accept-Ranges", "bytes");
}

/// `HEAD` must report the size a `GET` would return, with no body. Loaders use it
/// to allocate before downloading.
#[tokio::test]
async fn head_reports_the_full_length_with_no_body() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::head(format!("{}index.pck", h.base()))
        .send(h.addr)
        .await;
    assert_eq!(r.status, 200);
    r.expect_header("Content-Length", &PCK_LEN.to_string());
    r.expect_header("Accept-Ranges", "bytes");
    assert!(r.body.is_empty(), "HEAD must not carry a body");
}

// ---------------------------------------------------------------------------
// 4. Integrity — fail closed
// ---------------------------------------------------------------------------

/// The rule the whole crate turns on: never serve unverified bytes. The blob
/// store here lies about one hash, exactly as a corrupted disk or a dishonest
/// remote backend would.
#[tokio::test]
async fn a_tampered_file_is_refused_and_its_bytes_never_reach_the_client() {
    let (blobs, manifest) = common::fixture_manifest().await;
    let wasm_hash = manifest
        .lookup("index.wasm")
        .expect("fixture has index.wasm")
        .hash;

    let evil = b"malicious replacement payload".to_vec();
    let lying = common::LyingBlobs {
        inner: Arc::clone(&blobs),
        lie_about: wasm_hash,
        lie_with: evil.clone(),
    };
    let (addr, root) = common::spawn_generic(lying, manifest).await;
    let base = format!("/pkg/{}/", root.to_hex());

    let r = Req::get(format!("{base}index.wasm")).send(addr).await;
    assert_eq!(r.status, 502, "an integrity failure must not be a 200");
    r.expect_header("X-Magnetite-Integrity", "failed");
    assert!(
        !r.body.windows(evil.len()).any(|w| w == &evil[..]),
        "not one byte of the substituted payload may appear in the response"
    );

    // Everything else still serves: the refusal is per file, not a whole-node
    // outage, so one corrupt blob does not take the catalogue down.
    let ok = Req::get(format!("{base}index.html")).send(addr).await;
    assert_eq!(ok.status, 200);
}

/// A range request over a tampered file must fail too. Verifying only the
/// requested slice would let a tampered file be exfiltrated range by range, each
/// slice "verifying" against nothing.
#[tokio::test]
async fn a_range_request_over_a_tampered_file_is_also_refused() {
    let (blobs, manifest) = common::fixture_manifest().await;
    let pck_hash = manifest.lookup("index.pck").expect("fixture").hash;
    let lying = common::LyingBlobs {
        inner: Arc::clone(&blobs),
        lie_about: pck_hash,
        // Same length as the real pack, so only the hash betrays it.
        lie_with: vec![0xAA; PCK_LEN],
    };
    let (addr, root) = common::spawn_generic(lying, manifest).await;

    let r = Req::get(format!("/pkg/{}/index.pck", root.to_hex()))
        .with("Range", "bytes=0-15")
        .send(addr)
        .await;
    assert_eq!(r.status, 502);
    assert!(!r.body.contains(&0xAA));
}

/// A file in the manifest whose blob this node does not hold is a `503` — the
/// node is incompletely seeded, which is retryable — and explicitly not a `200`
/// with an empty body.
#[tokio::test]
async fn a_missing_blob_is_503_not_an_empty_200() {
    struct Empty;
    #[async_trait::async_trait]
    impl BlobStore for Empty {
        async fn put(&self, bytes: &[u8]) -> Hash {
            Hash::of(bytes)
        }
        async fn get(&self, _h: &Hash) -> Option<Vec<u8>> {
            None
        }
        async fn has(&self, _h: &Hash) -> bool {
            false
        }
    }
    let (_blobs, manifest) = common::fixture_manifest().await;
    let (addr, root) = common::spawn_generic(Empty, manifest).await;
    let r = Req::get(format!("/pkg/{}/index.wasm", root.to_hex()))
        .send(addr)
        .await;
    assert_eq!(r.status, 503);
}

// ---------------------------------------------------------------------------
// 5. The entitlement gate
// ---------------------------------------------------------------------------

/// A paid bundle serves nothing — not even the entry document — without a
/// receipt. `402` rather than `403` so a client knows to go and buy it.
#[tokio::test]
async fn an_unpaid_request_for_a_paid_bundle_is_refused() {
    let h = common::spawn(true, ServePolicy::default()).await;

    for path in ["", "index.html", "index.wasm", "index.pck"] {
        let r = Req::get(format!("{}{path}", h.base())).send(h.addr).await;
        assert_eq!(
            r.status, 402,
            "path {path:?} must be gated, not just the entry"
        );
        assert!(
            !r.body.starts_with(b"<!doctype"),
            "no content may leak in a refusal"
        );
    }
}

/// The paid path, working. Same URLs, same server, one receipt.
#[tokio::test]
async fn a_paid_request_with_a_verified_receipt_succeeds() {
    let h = common::spawn(true, ServePolicy::default()).await;

    // Via the cookie, which is the form the browser attaches to subresource
    // requests — the only form that makes a paid bundle actually playable.
    let doc = Req::get(h.base())
        .with("Cookie", &format!("mag_receipt={}", h.receipt_hex))
        .send(h.addr)
        .await;
    assert_eq!(doc.status, 200);
    assert_eq!(doc.body, h.bytes_of("index.html"));
    doc.expect_header("Cross-Origin-Embedder-Policy", "require-corp");

    // Via the header, for a script-issued fetch.
    let wasm = Req::get(format!("{}index.pck", h.base()))
        .with("X-Magnetite-Receipt", &h.receipt_hex)
        .send(h.addr)
        .await;
    assert_eq!(wasm.status, 200);
    assert_eq!(wasm.body, h.bytes_of("index.pck"));
}

/// A receipt whose bytes were edited must not verify. This is the payment seam's
/// own fail-closed check reached through the HTTP path.
#[tokio::test]
async fn a_tampered_receipt_is_refused_with_403() {
    let h = common::spawn(true, ServePolicy::default()).await;

    let mut receipt: serde_json::Value =
        serde_json::from_slice(&hex::decode(&h.receipt_hex).expect("hex")).expect("json");
    receipt["total"] = serde_json::json!(1);
    let forged = hex::encode(serde_json::to_vec(&receipt).expect("json"));

    let r = Req::get(h.base())
        .with("X-Magnetite-Receipt", &forged)
        .send(h.addr)
        .await;
    assert_eq!(r.status, 403);
}

/// Garbage in the receipt slot is a `400`, so a developer sees "your receipt is
/// malformed" rather than a `402` that reads as "you did not pay".
#[tokio::test]
async fn a_malformed_receipt_is_a_400_not_a_silent_402() {
    let h = common::spawn(true, ServePolicy::default()).await;
    for bad in ["zzzz", "deadbeef"] {
        let r = Req::get(h.base())
            .with("X-Magnetite-Receipt", bad)
            .send(h.addr)
            .await;
        assert_eq!(r.status, 400, "receipt {bad:?}");
    }
}

/// `public` on a paid bundle would let a CDN or corporate proxy cache one
/// entitled fetch and serve it to everyone behind it, with no gate in the path.
#[tokio::test]
async fn paid_bundles_are_never_publicly_cacheable_and_free_ones_are() {
    let paid = common::spawn(true, ServePolicy::default()).await;
    let r = Req::get(paid.base())
        .with("Cookie", &format!("mag_receipt={}", paid.receipt_hex))
        .send(paid.addr)
        .await;
    assert_eq!(r.status, 200);
    let cc = r.header("Cache-Control").unwrap_or("");
    assert!(cc.contains("private"), "paid Cache-Control was {cc:?}");
    assert!(!cc.contains("public"), "paid Cache-Control was {cc:?}");
    let vary = r.header("Vary").unwrap_or("");
    assert!(
        vary.contains("Cookie"),
        "paid response must Vary on Cookie, got {vary:?}"
    );

    let free = common::spawn(false, ServePolicy::default()).await;
    let f = Req::get(free.base()).send(free.addr).await;
    let cc = f.header("Cache-Control").unwrap_or("");
    assert!(cc.contains("public"), "free Cache-Control was {cc:?}");
    assert!(cc.contains("immutable"), "free Cache-Control was {cc:?}");
}

// ---------------------------------------------------------------------------
// 6. Routing, caching and the traversal surface
// ---------------------------------------------------------------------------

/// Without the redirect, `index.html` at `/pkg/<root>` resolves its relative
/// `index.wasm` against `/pkg/`, and every asset 404s. Works locally, breaks in
/// production.
#[tokio::test]
async fn the_bundle_root_without_a_trailing_slash_redirects() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("/pkg/{}", h.root.to_hex()))
        .send(h.addr)
        .await;
    assert_eq!(r.status, 301);
    r.expect_header("Location", &h.base());
}

/// `immutable` with a one-year `max-age` is only correct because the URL contains
/// the root hash: the bytes at a given URL can never change, so a stale entry is
/// unreachable rather than wrong.
#[tokio::test]
async fn hash_addressed_urls_are_cached_immutably_and_etags_give_304() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.pck", h.base()))
        .send(h.addr)
        .await;
    assert_eq!(r.status, 200);
    r.expect_header("Cache-Control", "public, max-age=31536000, immutable");
    r.expect_header("X-Magnetite-Bundle-Root", &h.root.to_hex());

    // The ETag is the file's own content hash.
    let etag = r.header("ETag").expect("ETag present").to_string();
    assert_eq!(
        etag,
        format!("\"{}\"", h.entry_of("index.pck").hash.to_hex())
    );

    let cached = Req::get(format!("{}index.pck", h.base()))
        .with("If-None-Match", &etag)
        .send(h.addr)
        .await;
    assert_eq!(cached.status, 304);
    assert!(cached.body.is_empty());
}

/// Rung 0 must not inherit rung 2's claims. ALIGNMENT.md §5: "Say so in the
/// manifest and in the UI."
#[tokio::test]
async fn every_response_labels_the_bundle_as_not_replay_verifiable() {
    let h = common::spawn(false, ServePolicy::default()).await;
    for path in ["", "index.wasm", "index.pck"] {
        let r = Req::get(format!("{}{path}", h.base())).send(h.addr).await;
        r.expect_header("X-Magnetite-Determinism", "not-replay-verifiable");
    }
}

/// Traversal, in every spelling. There is no filesystem path join anywhere in the
/// serving path — a request either names a key the manifest already listed or it
/// misses — so these are structurally impossible rather than filtered. Asserted
/// anyway, because "structurally impossible" is a claim that needs a test.
#[tokio::test]
async fn path_traversal_is_refused_in_every_spelling() {
    let h = common::spawn(false, ServePolicy::default()).await;
    for evil in [
        "../../../../etc/passwd",
        "..%2f..%2fetc%2fpasswd",
        "%2e%2e/%2e%2e/etc/passwd",
        "./index.html/../../../etc/passwd",
        "index.html/../../etc/passwd",
        "/etc/passwd",
        "a//b",
        ".",
        "..",
    ] {
        let r = Req::get(format!("{}{evil}", h.base())).send(h.addr).await;
        assert!(
            r.status == 404 || r.status == 400,
            "{evil:?} returned {} — must be 404 or 400",
            r.status
        );
        assert!(
            !r.text().contains("root:"),
            "{evil:?} appears to have served /etc/passwd"
        );
    }
}

/// A cache-busting query string on an immutable URL is meaningless but loaders
/// append one anyway; 404-ing it would break them for no benefit.
#[tokio::test]
async fn a_query_string_is_ignored_rather_than_404d() {
    let h = common::spawn(false, ServePolicy::default()).await;
    let r = Req::get(format!("{}index.wasm?v=12345", h.base()))
        .send(h.addr)
        .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body, h.bytes_of("index.wasm"));
}

#[tokio::test]
async fn unknown_roots_paths_and_methods_are_handled() {
    let h = common::spawn(false, ServePolicy::default()).await;

    // A well-formed hash that names no bundle.
    let other = Hash::of(b"not a published bundle");
    let r = Req::get(format!("/pkg/{}/index.html", other.to_hex()))
        .send(h.addr)
        .await;
    assert_eq!(r.status, 404);

    // A root segment that is not a hash at all.
    assert_eq!(Req::get("/pkg/notahash/x").send(h.addr).await.status, 404);
    // Outside the prefix.
    assert_eq!(Req::get("/").send(h.addr).await.status, 404);
    // Health.
    assert_eq!(Req::get("/healthz").send(h.addr).await.status, 200);
    // Writes are not a thing a content-addressed file server does.
    assert_eq!(Req::method("POST", h.base()).send(h.addr).await.status, 405);
    assert_eq!(
        Req::method("DELETE", format!("{}index.html", h.base()))
            .send(h.addr)
            .await
            .status,
        405
    );
}

/// A paid bundle must not answer "that file does not exist" to an unentitled
/// client: the file list is part of what was paid for, and a 404/402 split would
/// let anyone enumerate a bundle's contents for free.
#[tokio::test]
async fn a_paid_bundle_does_not_leak_its_file_list_through_status_codes() {
    let h = common::spawn(true, ServePolicy::default()).await;
    let real = Req::get(format!("{}index.wasm", h.base()))
        .send(h.addr)
        .await;
    let fake = Req::get(format!("{}secret-level-9.pck", h.base()))
        .send(h.addr)
        .await;
    assert_eq!(real.status, 402);
    assert_eq!(
        real.status, fake.status,
        "an existing and a non-existing path must be indistinguishable before payment"
    );
    assert_eq!(real.text(), fake.text());
}
