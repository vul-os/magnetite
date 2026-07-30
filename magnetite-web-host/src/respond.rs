//! Request → response resolution. No HTTP library, no socket.
//!
//! Everything that decides a status code or a header lives here, expressed over
//! plain types, so all of it is testable without binding a port. The hyper
//! binding in [`crate::server`] is a translation layer and contains no policy.
//!
//! # The five things this file exists to get right
//!
//! 1. **COOP + COEP on every response** ([`CrossOriginIsolation`]). Godot 4 web
//!    export does not boot without them.
//! 2. **Precompressed assets** — `Content-Encoding` from the suffix,
//!    `Content-Type` from what is underneath it ([`crate::media`]).
//! 3. **Range requests** — `Accept-Ranges: bytes`, `206`, `Content-Range`,
//!    `416`. A 40 MB `.pck` needs them.
//! 4. **Per-file integrity, fail closed** — every byte is re-hashed against the
//!    manifest before it leaves. See [`Outcome::IntegrityFailure`].
//! 5. **The entitlement gate** runs *before* the blob is even fetched.
//!
//! # Cache-Control, and the trap in it
//!
//! Hash-addressed URLs make `immutable` correct rather than optimistic: the URL
//! contains the bundle root hash, the root hash commits to every file's hash,
//! and a file's hash commits to its bytes. There is no sequence of events in
//! which the bytes at one of these URLs change. A new build is a new root hash
//! and therefore a new URL, so a stale cache entry is unreachable rather than
//! wrong. That is why a year-long `max-age` is safe here while `nginx.conf`
//! has to send `no-cache` for `.html` — its `/index.html` is a mutable name.
//!
//! The trap: `public` would be a hole for a paid bundle. `public` invites shared
//! caches — a CDN, a corporate proxy — to store the response and hand it to the
//! next requester, and a shared cache does not re-run the entitlement gate.
//! One entitled fetch would populate a cache that then serves the paid bundle to
//! everyone behind it. So paid bundles get `private` (browser-local only) plus a
//! `Vary` on the credential headers, and only free bundles get `public`.

use magnetite_seams::blobstore::{BlobStore, Hash};
use magnetite_seams::payment::PaymentRail;

use crate::entitlement::{self, Credentials, Refusal, Verdict};
use crate::manifest::{BundleManifest, FileEntry};
use crate::media::{self, Encoding};

/// A year, in seconds — the `max-age` ceiling RFC 9111 recommends not exceeding.
const IMMUTABLE_MAX_AGE: u32 = 31_536_000;

/// The methods a static bundle host answers. Anything else is `405`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Full response.
    Get,
    /// Headers only, with the `Content-Length` a `GET` would have produced.
    /// Loaders use it to size a download before starting it.
    Head,
}

/// A resolved request against one bundle.
#[derive(Clone, Debug)]
pub struct BundleRequest<'a> {
    /// `GET` or `HEAD`.
    pub method: Method,
    /// Bundle-relative path, already percent-decoded and stripped of the
    /// `/pkg/<root>/` prefix. Empty means "the bundle root", which resolves to
    /// [`BundleManifest::entry`].
    pub path: &'a str,
    /// Raw `Accept-Encoding`, or `""`.
    pub accept_encoding: &'a str,
    /// Raw `Range`, if present.
    pub range: Option<&'a str>,
    /// Raw `If-None-Match`, if present.
    pub if_none_match: Option<&'a str>,
    /// What the client presented for the entitlement gate.
    pub credentials: Credentials,
}

impl<'a> BundleRequest<'a> {
    /// A plain `GET` with no conditional headers and no credentials.
    pub fn get(path: &'a str) -> Self {
        Self {
            method: Method::Get,
            path,
            accept_encoding: "",
            range: None,
            if_none_match: None,
            credentials: Credentials::none(),
        }
    }
    /// Set `Accept-Encoding`.
    pub fn with_accept_encoding(mut self, v: &'a str) -> Self {
        self.accept_encoding = v;
        self
    }
    /// Set `Range`.
    pub fn with_range(mut self, v: &'a str) -> Self {
        self.range = Some(v);
        self
    }
    /// Set `If-None-Match`.
    pub fn with_if_none_match(mut self, v: &'a str) -> Self {
        self.if_none_match = Some(v);
        self
    }
    /// Attach credentials for the entitlement gate.
    pub fn with_credentials(mut self, c: Credentials) -> Self {
        self.credentials = c;
        self
    }
    /// Make it a `HEAD`.
    pub fn head(mut self) -> Self {
        self.method = Method::Head;
        self
    }
}

/// Why a response looks the way it does. Kept alongside the status so callers
/// can log and test the *reason*, not just the number — `403` alone does not
/// distinguish a rejected receipt from a wrong buyer, and `502` alone does not
/// say "someone tampered with a blob", which is the one an operator must page on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Whole file, `200`.
    Served,
    /// A byte range, `206`.
    ServedRange {
        /// First byte served, inclusive.
        start: u64,
        /// Last byte served, inclusive.
        end: u64,
        /// Length of the complete representation.
        total: u64,
    },
    /// `ETag` matched `If-None-Match`, `304`.
    NotModified,
    /// Redirect to the trailing-slash form, `301`.
    RedirectToSlash,
    /// The manifest has no such path, `404`.
    NotFound,
    /// Method other than `GET`/`HEAD`, `405`.
    MethodNotAllowed,
    /// `Range` cannot be satisfied, `416`.
    RangeNotSatisfiable {
        /// Length of the complete representation.
        total: u64,
    },
    /// The only stored representation is compressed and the client refused that
    /// encoding, `406`. Serving it anyway would hand the client bytes it cannot
    /// read; decompressing it server-side would mean shipping a brotli decoder
    /// and re-hashing, which defeats the point of storing it precompressed.
    NotAcceptable {
        /// The encoding the client would have to accept.
        needed: &'static str,
    },
    /// The gate said no, status per [`Refusal::status`].
    Refused(Refusal),
    /// The path is in the manifest but the blob store does not have the bytes,
    /// `503`. The node is incompletely seeded; retrying may work.
    BlobMissing {
        /// The content address that is missing.
        hash: Hash,
    },
    /// The bytes came back but did not hash to the manifest's value, or their
    /// length disagreed, `502`. **Nothing is served.** Not retryable — this is
    /// corruption or tampering, and the correct behaviour is to be loudly
    /// broken rather than quietly wrong.
    IntegrityFailure {
        /// What the manifest promised.
        expected: Hash,
        /// What the bytes actually hash to.
        got: Hash,
    },
}

impl Outcome {
    /// The HTTP status for this outcome.
    pub fn status(&self) -> u16 {
        match self {
            Self::Served => 200,
            Self::ServedRange { .. } => 206,
            Self::NotModified => 304,
            Self::RedirectToSlash => 301,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::NotAcceptable { .. } => 406,
            Self::RangeNotSatisfiable { .. } => 416,
            Self::Refused(r) => r.status(),
            Self::BlobMissing { .. } => 503,
            Self::IntegrityFailure { .. } => 502,
        }
    }
}

/// A response, ready for any HTTP layer to emit.
#[derive(Clone, Debug)]
pub struct BundleResponse {
    /// Status code.
    pub status: u16,
    /// Headers, in emission order.
    pub headers: Vec<(String, String)>,
    /// Body. Empty for `HEAD`, `304` and `301`; the `Content-Length` header
    /// still describes what a `GET` would have returned.
    pub body: Vec<u8>,
    /// Why (see [`Outcome`]).
    pub outcome: Outcome,
}

impl BundleResponse {
    /// First value for a header name, case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    /// Whether a header is present.
    pub fn has_header(&self, name: &str) -> bool {
        self.header(name).is_some()
    }
}

/// The cross-origin headers a Godot 4 web export needs in order to boot.
///
/// ALIGNMENT.md §5 states the problem and the cost in the same breath:
///
/// > **COOP/COEP is absent.** Godot 4 web export needs `SharedArrayBuffer`,
/// > which needs `Cross-Origin-Opener-Policy: same-origin` and
/// > `Cross-Origin-Embedder-Policy: require-corp`. [...] This is the classic
/// > Godot-on-itch.io failure, and COEP then breaks cross-origin assets —
/// > **bundles must be self-contained or same-origin.**
///
/// The mechanism, because the constraint follows from it rather than being a
/// separate rule: `SharedArrayBuffer` is only exposed to a **cross-origin
/// isolated** document, and a document is cross-origin isolated only when it is
/// a secure context (HTTPS, or `localhost`) *and* it was served with both of
/// those headers. `Cross-Origin-Embedder-Policy: require-corp` earns that
/// isolation by promising the browser that the document embeds nothing which has
/// not opted in — so under it, **every cross-origin subresource that does not
/// send `Cross-Origin-Resource-Policy` (or a permissive CORS response) is
/// blocked**, silently as far as the page is concerned.
///
/// So the constraint on bundles is not a policy choice this crate makes; it is
/// what enabling `SharedArrayBuffer` costs, everywhere, for everyone:
///
/// * A bundle that pulls a font from Google Fonts, a texture from a CDN, an
///   analytics script, or an `<iframe>` from another origin **will have those
///   requests fail** once COEP is on. The page usually does not report why.
/// * The fix is to vendor them into the bundle. That is also what makes the
///   bundle content-addressable end to end, so it is the right shape anyway: a
///   bundle whose behaviour depends on a third-party host is not reproducible
///   from its root hash.
/// * The escape hatch, for a bundle that genuinely does not need threads, is
///   [`CrossOriginIsolation::Disabled`] — no COEP, cross-origin subresources
///   work, and `SharedArrayBuffer` is unavailable. A Godot 4 export will not
///   run in that mode. A typical three.js scene will.
///
/// This crate's default is [`CrossOriginIsolation::Enabled`], because the engine
/// that fails without it fails *totally*, while the bundle that trips over it
/// fails in a way its author can see and fix by vendoring an asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CrossOriginIsolation {
    /// COOP `same-origin` + COEP `require-corp` + CORP `same-origin`.
    /// `SharedArrayBuffer` available; cross-origin subresources blocked.
    #[default]
    Enabled,
    /// Neither COOP nor COEP. Cross-origin subresources work;
    /// `SharedArrayBuffer` unavailable, so Godot 4 will not boot.
    Disabled,
}

/// How a bundle is served, beyond the manifest itself.
#[derive(Clone, Copy, Debug)]
pub struct ServePolicy {
    /// Cross-origin isolation. Default [`CrossOriginIsolation::Enabled`].
    pub isolation: CrossOriginIsolation,
    /// Whether a request for `x.wasm` may be answered with a stored
    /// `x.wasm.br`. Default `true`.
    ///
    /// Two shapes of bundle exist in the wild and both must work:
    ///
    /// * **Unity** requests `Build/g.wasm.br` by that literal URL and requires
    ///   the server to declare `Content-Encoding: br`. That path needs no
    ///   negotiation and is always handled.
    /// * **Godot behind a compressing proxy** requests `index.wasm` and expects
    ///   the server to substitute a precompressed variant if it has one. That
    ///   is this flag.
    pub negotiate_precompressed: bool,
    /// `max-age` for successful responses. Default one year.
    pub max_age: u32,
}

impl Default for ServePolicy {
    fn default() -> Self {
        Self {
            isolation: CrossOriginIsolation::default(),
            negotiate_precompressed: true,
            max_age: IMMUTABLE_MAX_AGE,
        }
    }
}

/// One hosted bundle: its manifest, the blob store holding its files, and the
/// policy for serving it.
pub struct HostedBundle {
    /// The manifest. Always validated on construction.
    pub manifest: BundleManifest,
    /// Serving policy.
    pub policy: ServePolicy,
    root: Hash,
}

impl HostedBundle {
    /// Validate `manifest` and take its root hash.
    ///
    /// The root hash is computed once and stored, not recomputed per request:
    /// it is a hash over the whole file list, and doing that on every asset
    /// fetch would make a 400-file bundle quadratic.
    pub fn new(manifest: BundleManifest, policy: ServePolicy) -> crate::error::Result<Self> {
        manifest.validate()?;
        let root = manifest.root_hash();
        Ok(Self {
            manifest,
            policy,
            root,
        })
    }

    /// With the default policy.
    pub fn with_defaults(manifest: BundleManifest) -> crate::error::Result<Self> {
        Self::new(manifest, ServePolicy::default())
    }

    /// The bundle's content address; also the `<root>` segment of its URLs.
    pub fn root(&self) -> Hash {
        self.root
    }

    /// Resolve one request.
    ///
    /// Order of operations is deliberate and each step is a gate on the next:
    ///
    /// 1. method — cheapest rejection
    /// 2. **entitlement** — before any path is resolved and before any blob is
    ///    fetched, so a paid bundle leaks neither its file list (via 404 vs 403)
    ///    nor a byte of content, and an unentitled flood cannot make the node do
    ///    blob I/O
    /// 3. path → [`FileEntry`]
    /// 4. encoding negotiation
    /// 5. range (`Range`), parsed against the manifest's own length — no blob
    ///    I/O needed yet, so an unsatisfiable range (`416`) is answered
    ///    without ever touching the blob store
    /// 6. conditional request (`If-None-Match`) — also needs only the
    ///    manifest's hash, not the bytes
    /// 7. fetch **and verify** the blob — the *last* gate before touching
    ///    storage, and deliberately so: it is the one step whose cost scales
    ///    with the blob's size, so every gate that does not need bytes runs
    ///    first. This step still calls the whole-blob [`BlobStore::get`], on
    ///    every response, ranged or not — **not** [`BlobStore::get_range`].
    ///    See the comment above the fetch itself for why: a partial fetch
    ///    cannot be re-verified against the whole-blob hash without reading
    ///    the whole blob anyway, and this crate's own
    ///    `a_range_request_over_a_tampered_file_is_also_refused` test requires
    ///    that a tampered file is refused identically whether the request was
    ///    ranged or not. `get_range` is real, tested, and correctly avoids
    ///    materializing a large blob for backends that implement it — see
    ///    `magnetite-seams`' `blobstore` module — but wiring it in here would
    ///    trade that guarantee away, which this pass does not do.
    pub async fn respond<B, R>(
        &self,
        blobs: &B,
        rail: Option<&R>,
        req: &BundleRequest<'_>,
    ) -> BundleResponse
    where
        B: BlobStore + ?Sized,
        R: PaymentRail + ?Sized,
    {
        // 1. Method.
        if !matches!(req.method, Method::Get | Method::Head) {
            return self.plain(Outcome::MethodNotAllowed, "method not allowed");
        }

        // 2. Entitlement, first. Note this runs before path resolution, so a
        //    paid bundle's 402/403 is identical for a path that exists and one
        //    that does not — the file list is part of what was paid for.
        match entitlement::evaluate(&self.manifest.pricing, rail, &req.credentials) {
            Verdict::Granted => {}
            // A14: the receipt verified locally but the rail could not
            // re-confirm it against a chain right now. This node has not
            // been given a policy for serving on that tier — deciding one
            // (an operator opt-in? which bundles?) is real product work this
            // pass does not do, the same way backlog item A15 documents a
            // boundary without building the exchange across it. Until such a
            // policy exists the fail-closed default applies: refuse, loudly
            // and distinguishably (`Refusal::PendingSettlement`), rather than
            // silently reusing `ReceiptRejected`'s reason, which would be a
            // lie — this receipt did not fail, it is merely unconfirmed.
            Verdict::GrantedUnsettled => {
                return self.plain(
                    Outcome::Refused(Refusal::PendingSettlement),
                    Refusal::PendingSettlement.reason(),
                )
            }
            Verdict::Refused(r) => return self.plain(Outcome::Refused(r), r.reason()),
        }

        // 3. Path. Empty means the entry document.
        let requested = if req.path.is_empty() {
            self.manifest.entry.as_str()
        } else {
            req.path
        };
        let Some(entry) = self.manifest.lookup(requested) else {
            return self.plain(Outcome::NotFound, "not found in bundle manifest");
        };

        // 4. Encoding. Either the stored path already declares one (Unity), or
        //    we may substitute a stored precompressed sibling (Godot via proxy).
        let (stored_encoding, _logical) = media::split_encoding(&entry.path);
        let mut negotiated = false;
        let mut vary_accept_encoding = false;
        let mut serving = entry;
        let mut encoding = stored_encoding;

        if stored_encoding == Encoding::Identity && self.policy.negotiate_precompressed {
            if let Some((variant, enc)) = self.precompressed_variant(&entry.path) {
                // A variant exists, so the answer now depends on Accept-Encoding
                // and the response must say so — otherwise a cache populated by
                // a br-capable client would serve br bytes to one that is not.
                vary_accept_encoding = true;
                if media::accepts(req.accept_encoding, enc) {
                    serving = variant;
                    encoding = enc;
                    negotiated = true;
                }
            }
        } else if stored_encoding != Encoding::Identity
            && !media::accepts(req.accept_encoding, stored_encoding)
        {
            // The URL literally names a compressed file and the client says it
            // cannot decode that. There is nothing honest to send.
            return self.plain(
                Outcome::NotAcceptable {
                    needed: stored_encoding.header_value().unwrap_or("identity"),
                },
                "the only stored representation uses a content coding you did not accept",
            );
        }

        // 5. Range. Parsed against the manifest's own `len` — no blob store
        //    I/O needed to know how large the representation is, so an
        //    unsatisfiable range answers `416` without ever fetching a byte.
        //
        //    Ranges are only honoured for the representation the URL names. When
        //    step 4 *substituted* a compressed variant, the offsets the client
        //    asked about refer to a representation we are not sending — RFC 9110
        //    defines `Content-Range` over the encoded bytes, so serving a range
        //    of the brotli stream against a request that meant the wasm stream
        //    would be silently wrong. Sending the whole thing instead is always
        //    legal (`Range` is a request the server MAY ignore) and is never
        //    wrong.
        let total = serving.len;
        let ranged = match req.range {
            Some(spec) if !negotiated => match parse_range(spec, total) {
                RangeParse::None => None,
                RangeParse::Satisfiable { start, end } => Some((start, end)),
                RangeParse::Unsatisfiable => {
                    let mut h = self.base_headers(vary_accept_encoding);
                    h.push(("Content-Range".into(), format!("bytes */{total}")));
                    h.push(("Accept-Ranges".into(), "bytes".into()));
                    return BundleResponse {
                        status: 416,
                        headers: h,
                        body: Vec::new(),
                        outcome: Outcome::RangeNotSatisfiable { total },
                    };
                }
            },
            _ => None,
        };

        // 6. Conditional. ETag is the file's content hash — a strong validator
        //    for free, computable from the manifest alone, and exactly right:
        //    same hash means same bytes, always. Also needs no blob I/O.
        let etag = format!("\"{}\"", serving.hash.to_hex());
        if let Some(inm) = req.if_none_match {
            if if_none_match_matches(inm, &etag) {
                let mut h = self.base_headers(vary_accept_encoding);
                h.push(("ETag".into(), etag));
                return BundleResponse {
                    status: 304,
                    headers: h,
                    body: Vec::new(),
                    outcome: Outcome::NotModified,
                };
            }
        }

        // 7. Bytes, then integrity. FAIL CLOSED.
        //
        //    Deliberately still `get` — the whole blob — for EVERY response,
        //    ranged or not. This is a considered decision, not an oversight of
        //    magnetite-seams A9's `get_range`: see the doc comment above
        //    `respond` and the note on `BlobStore::get_range` for why this
        //    method does not (yet) read a genuinely partial `Range` through
        //    it. In short — `a_range_request_over_a_tampered_file_is_also_refused`
        //    (`tests/serving.rs`) requires that a range response over
        //    tampered content is refused exactly like a whole one, and there
        //    is no way to verify a *slice* against a whole-blob hash without
        //    reading the whole blob, which would silently defeat the point of
        //    `get_range` anyway. Preserving that guarantee costs the same
        //    memory here today as before this change; genuinely fixing it
        //    needs a streaming/verify-while-reading shape, which is the
        //    larger, deferred design this pass's seam change does not attempt.
        //
        //    `FsBlobStore` and `HttpBlobStore` already re-verify on read, but
        //    `BlobStore` is a *pluggable seam*: a third-party implementation
        //    (Walrus, Arweave, Filecoin — ALIGNMENT.md §6) is under no
        //    obligation to, and `LocalBlobStore` does not. Verifying here means
        //    the guarantee holds for every backend, present and future, rather
        //    than for the two that happen to implement it today. It is one
        //    BLAKE3 pass over bytes already in memory.
        let Some(bytes) = blobs.get(&serving.hash).await else {
            return self.plain(
                Outcome::BlobMissing { hash: serving.hash },
                "bundle file is listed in the manifest but absent from this node's blob store",
            );
        };
        let got = Hash::of(&bytes);
        if got != serving.hash || bytes.len() as u64 != serving.len {
            // Note the length check shares this branch: a blob whose length
            // disagrees with the manifest is refused even in the impossible case
            // that its hash matched, because `Content-Length` and `Content-Range`
            // are computed from the manifest and must not describe other bytes.
            return self.plain(
                Outcome::IntegrityFailure {
                    expected: serving.hash,
                    got,
                },
                "bundle file failed its integrity check and was not served",
            );
        }

        let mut headers = self.base_headers(vary_accept_encoding);
        headers.push((
            "Content-Type".into(),
            media::content_type(&serving.path).into(),
        ));
        if let Some(ce) = encoding.header_value() {
            headers.push(("Content-Encoding".into(), ce.into()));
        }
        headers.push(("ETag".into(), etag));
        headers.push(("Accept-Ranges".into(), "bytes".into()));

        let (status, body, outcome) = match ranged {
            Some((start, end)) => {
                let len = end - start + 1;
                headers.push((
                    "Content-Range".into(),
                    format!("bytes {start}-{end}/{total}"),
                ));
                headers.push(("Content-Length".into(), len.to_string()));
                // Slicing AFTER verifying the whole blob is the point: the
                // integrity check covers the complete representation, so a range
                // response is exactly as trustworthy as a full one. Hashing only
                // the requested slice would let a tampered file be exfiltrated
                // range by range, each slice "verifying" against nothing.
                let body = match req.method {
                    Method::Get => bytes[start as usize..=end as usize].to_vec(),
                    Method::Head => Vec::new(),
                };
                (206, body, Outcome::ServedRange { start, end, total })
            }
            None => {
                headers.push(("Content-Length".into(), total.to_string()));
                let body = match req.method {
                    Method::Get => bytes,
                    Method::Head => Vec::new(),
                };
                (200, body, Outcome::Served)
            }
        };

        BundleResponse {
            status,
            headers,
            body,
            outcome,
        }
    }

    /// Find a stored precompressed sibling of `path`, best encoding first.
    fn precompressed_variant(&self, path: &str) -> Option<(&FileEntry, Encoding)> {
        for enc in Encoding::PRECOMPRESSED {
            let suffix = enc.suffix()?;
            if let Some(f) = self.manifest.lookup(&format!("{path}{suffix}")) {
                return Some((f, enc));
            }
        }
        None
    }

    /// Headers on every response, success or failure.
    ///
    /// COOP/COEP go on failures too. A `403` that omits them would, if a client
    /// retried into it while already isolated, be a document without isolation —
    /// and more practically, an error page served into a cross-origin-isolated
    /// context without COEP is itself blocked, so the developer sees a blank
    /// frame instead of the 403 that would have told them what was wrong.
    fn base_headers(&self, vary_accept_encoding: bool) -> Vec<(String, String)> {
        let mut h: Vec<(String, String)> = Vec::with_capacity(10);

        if self.policy.isolation == CrossOriginIsolation::Enabled {
            // The two headers Godot 4 cannot boot without.
            h.push(("Cross-Origin-Opener-Policy".into(), "same-origin".into()));
            h.push(("Cross-Origin-Embedder-Policy".into(), "require-corp".into()));
            // Not required for the bundle's own same-origin subresources, which
            // COEP permits regardless. This is the other direction: it stops
            // *another* origin embedding these bytes into its own isolated page,
            // which for a paid bundle would be hotlinking around the gate.
            h.push(("Cross-Origin-Resource-Policy".into(), "same-origin".into()));
        }

        // `nosniff` is what makes `application/octet-stream` a safe default for
        // an unknown extension: the browser will not promote a guess to script.
        h.push(("X-Content-Type-Options".into(), "nosniff".into()));

        // Rung 0 is not replay-verifiable and must not be mistaken for rung 2.
        // ALIGNMENT.md §5: "Say so in the manifest and in the UI."
        h.push((
            "X-Magnetite-Determinism".into(),
            self.manifest.determinism.as_str().into(),
        ));
        h.push(("X-Magnetite-Bundle-Root".into(), self.root.to_hex()));

        // See the module docs for why `public` is a hole on a paid bundle.
        let paid = self.manifest.pricing.is_paid();
        let scope = if paid { "private" } else { "public" };
        h.push((
            "Cache-Control".into(),
            format!("{scope}, max-age={}, immutable", self.policy.max_age),
        ));

        let mut vary: Vec<&str> = Vec::new();
        if vary_accept_encoding {
            vary.push("Accept-Encoding");
        }
        if paid {
            // The response depends on the credential, so a cache keyed only on
            // the URL would be wrong even within one browser profile.
            vary.push("Cookie");
            vary.push(crate::host::RECEIPT_HEADER);
        }
        if !vary.is_empty() {
            h.push(("Vary".into(), vary.join(", ")));
        }
        h
    }

    /// A bodied text response for a non-success outcome.
    fn plain(&self, outcome: Outcome, message: &str) -> BundleResponse {
        let status = outcome.status();
        let mut headers = self.base_headers(false);
        headers.push(("Content-Type".into(), "text/plain; charset=utf-8".into()));
        // An error is not immutable content; caching a 404 for a year would
        // outlive the reason for it (e.g. a blob that finished replicating).
        if let Some(cc) = headers
            .iter_mut()
            .find(|(k, _)| k == "Cache-Control")
            .map(|(_, v)| v)
        {
            *cc = "no-store".into();
        }
        if let Outcome::IntegrityFailure { .. } = outcome {
            headers.push(("X-Magnetite-Integrity".into(), "failed".into()));
        }
        let body = format!("{status} {message}\n").into_bytes();
        headers.push(("Content-Length".into(), body.len().to_string()));
        BundleResponse {
            status,
            headers,
            body,
            outcome,
        }
    }
}

/// Whether `If-None-Match` matches `etag`.
///
/// Handles `*`, comma-separated lists, and the `W/` weak prefix. A weak match is
/// sufficient for `304` per RFC 9110 §13.1.2, and it is the correct semantics
/// anyway: our validator is the content hash, so equal validators means equal
/// bytes with no weakness to worry about.
fn if_none_match_matches(header: &str, etag: &str) -> bool {
    let strip = |s: &str| s.trim().trim_start_matches("W/").trim().to_string();
    let want = strip(etag);
    header
        .split(',')
        .any(|c| c.trim() == "*" || strip(c) == want)
}

/// The outcome of parsing a `Range` header.
#[derive(Debug, PartialEq, Eq)]
enum RangeParse {
    /// No range to apply — absent, malformed, or a form we choose to ignore.
    /// Send the whole representation; `Range` is advisory.
    None,
    /// A single satisfiable range, inclusive on both ends.
    Satisfiable {
        /// First byte.
        start: u64,
        /// Last byte.
        end: u64,
    },
    /// Syntactically valid but outside the representation → `416`.
    Unsatisfiable,
}

/// Parse a single-range `bytes=` specifier against a known `total`.
///
/// Supports the three forms browsers and engine loaders actually send:
/// `bytes=0-1023`, `bytes=1024-` (open-ended, what a resumed download sends),
/// and `bytes=-512` (suffix, the last N bytes — how a loader reads a trailing
/// index).
///
/// Multi-range (`bytes=0-9,20-29`) returns [`RangeParse::None`] rather than
/// being implemented: the reply would have to be a `multipart/byteranges`
/// document, no game loader asks for it, and a whole-file `200` is a legal and
/// correct answer to any `Range`. Silently serving only the *first* range while
/// claiming `206` — the usual shortcut — would corrupt the client's buffer.
///
/// A zero-length representation is always unsatisfiable: there is no byte index
/// that exists in it, and `bytes=0-0` on an empty file must be a `416`, not an
/// empty `206`.
fn parse_range(spec: &str, total: u64) -> RangeParse {
    let Some(set) = spec.trim().strip_prefix("bytes=") else {
        return RangeParse::None; // unknown unit: ignore, send everything
    };
    let set = set.trim();
    if set.contains(',') {
        return RangeParse::None;
    }
    let Some((first, last)) = set.split_once('-') else {
        return RangeParse::None;
    };
    let (first, last) = (first.trim(), last.trim());

    let (start, end) = if first.is_empty() {
        // Suffix form: the last `n` bytes.
        let Ok(n) = last.parse::<u64>() else {
            return RangeParse::None;
        };
        if n == 0 || total == 0 {
            return RangeParse::Unsatisfiable;
        }
        (total.saturating_sub(n), total - 1)
    } else {
        let Ok(start) = first.parse::<u64>() else {
            return RangeParse::None;
        };
        if total == 0 || start >= total {
            return RangeParse::Unsatisfiable;
        }
        let end = if last.is_empty() {
            total - 1
        } else {
            match last.parse::<u64>() {
                // An end beyond the last byte is clamped, not refused —
                // RFC 9110 §14.1.2 requires it, and `bytes=0-99999999` is what a
                // loader sends when it does not yet know the file size.
                Ok(e) => e.min(total - 1),
                Err(_) => return RangeParse::None,
            }
        };
        if end < start {
            return RangeParse::Unsatisfiable;
        }
        (start, end)
    };
    RangeParse::Satisfiable { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_forms_browsers_actually_send() {
        // Closed.
        assert_eq!(
            parse_range("bytes=0-1023", 4096),
            RangeParse::Satisfiable {
                start: 0,
                end: 1023
            }
        );
        // Open-ended (resumed download).
        assert_eq!(
            parse_range("bytes=1024-", 4096),
            RangeParse::Satisfiable {
                start: 1024,
                end: 4095
            }
        );
        // Suffix (trailing index read).
        assert_eq!(
            parse_range("bytes=-512", 4096),
            RangeParse::Satisfiable {
                start: 3584,
                end: 4095
            }
        );
        // Suffix longer than the file clamps to the whole file.
        assert_eq!(
            parse_range("bytes=-9999", 100),
            RangeParse::Satisfiable { start: 0, end: 99 }
        );
        // End past EOF is clamped, per RFC 9110 — this is the one that breaks
        // loaders if you 416 it instead.
        assert_eq!(
            parse_range("bytes=0-99999999", 10),
            RangeParse::Satisfiable { start: 0, end: 9 }
        );
        // Single byte.
        assert_eq!(
            parse_range("bytes=9-9", 10),
            RangeParse::Satisfiable { start: 9, end: 9 }
        );
    }

    #[test]
    fn unsatisfiable_and_ignored_ranges_are_distinguished() {
        // Start at or past EOF → 416.
        assert_eq!(parse_range("bytes=10-20", 10), RangeParse::Unsatisfiable);
        assert_eq!(parse_range("bytes=10-", 10), RangeParse::Unsatisfiable);
        // Reversed → 416.
        assert_eq!(parse_range("bytes=5-2", 10), RangeParse::Unsatisfiable);
        // Empty representation has no satisfiable byte.
        assert_eq!(parse_range("bytes=0-0", 0), RangeParse::Unsatisfiable);
        assert_eq!(parse_range("bytes=-1", 0), RangeParse::Unsatisfiable);
        assert_eq!(parse_range("bytes=-0", 10), RangeParse::Unsatisfiable);
        // Ignored: unknown unit, multi-range, garbage.
        assert_eq!(parse_range("items=0-1", 10), RangeParse::None);
        assert_eq!(parse_range("bytes=0-9,20-29", 100), RangeParse::None);
        assert_eq!(parse_range("bytes=abc", 10), RangeParse::None);
        assert_eq!(parse_range("bytes=1-x", 10), RangeParse::None);
        assert_eq!(parse_range("nonsense", 10), RangeParse::None);
    }

    #[test]
    fn if_none_match_handles_lists_stars_and_weak_tags() {
        let tag = "\"abc\"";
        assert!(if_none_match_matches("*", tag));
        assert!(if_none_match_matches("\"abc\"", tag));
        assert!(if_none_match_matches("W/\"abc\"", tag));
        assert!(if_none_match_matches("\"zzz\", \"abc\"", tag));
        assert!(!if_none_match_matches("\"zzz\"", tag));
        assert!(!if_none_match_matches("", tag));
    }
}
