//! [`WebHost`] — a catalogue of bundles, URL routing, and credential
//! extraction. Still no HTTP library: this layer takes a method, a target and a
//! list of header pairs, so every routing and credential decision is testable
//! without binding a port. [`crate::server`] is the only file that knows what
//! hyper is.
//!
//! # URL shape
//!
//! ```text
//! /pkg/<root-hash-hex>/            -> the bundle's entry document
//! /pkg/<root-hash-hex>/<path>      -> one file
//! /pkg/<root-hash-hex>             -> 301 to the trailing-slash form
//! /healthz                         -> 200
//! ```
//!
//! The root hash in the path is what makes `Cache-Control: immutable` sound
//! (see [`crate::respond`]) and it is why a node can host any number of versions
//! of a game at once without a version table: the URL *is* the version.
//!
//! The trailing-slash redirect is not cosmetic. `index.html` references its
//! siblings relatively (`index.js`, `index.wasm`), and a browser resolves those
//! against the document's base URL. Served at `/pkg/<root>` — no slash — the
//! base is `/pkg/`, so every asset request becomes `/pkg/index.wasm` and 404s.
//! This is a classic way to ship a bundle that works locally and breaks in
//! production; the `301` removes the possibility.

use std::collections::HashMap;

use magnetite_seams::blobstore::{BlobStore, Hash};
use magnetite_seams::payment::{PaymentRail, Receipt};

use crate::entitlement::Credentials;
use crate::manifest::normalize_path;
use crate::respond::{
    BundleRequest, BundleResponse, CrossOriginIsolation, HostedBundle, Method, Outcome,
};

/// URL prefix under which bundles are served.
pub const PKG_PREFIX: &str = "/pkg/";

/// Header carrying a hex-encoded JSON [`Receipt`].
pub const RECEIPT_HEADER: &str = "X-Magnetite-Receipt";

/// Cookie carrying a hex-encoded JSON [`Receipt`].
///
/// # Why a cookie exists at all
///
/// A custom request header only works for a fetch the *page's own script*
/// issues. It does not work for the requests that actually matter here: the
/// browser fetches `index.wasm`, `index.pck`, `index.js` and every texture
/// itself, on behalf of the document, and it does not attach custom headers to
/// those. Gate a paid bundle on a header alone and the entry document loads
/// while every subresource 402s — the game shows a blank canvas and the operator
/// concludes the gate is broken.
///
/// A cookie scoped to the bundle's path is attached automatically to
/// same-origin subresource requests, which is the only mechanism that covers
/// them. See [`WebHost::handle`] for the honest cost of this design.
pub const RECEIPT_COOKIE: &str = "mag_receipt";

/// One request, reduced to what routing needs.
#[derive(Clone, Debug)]
pub struct RawRequest<'a> {
    /// `"GET"`, `"HEAD"`, …
    pub method: &'a str,
    /// Request target: path plus any query string.
    pub target: &'a str,
    /// Header name/value pairs, as received.
    pub headers: Vec<(&'a str, &'a str)>,
}

impl<'a> RawRequest<'a> {
    /// A bare `GET`.
    pub fn get(target: &'a str) -> Self {
        Self {
            method: "GET",
            target,
            headers: Vec::new(),
        }
    }
    /// A bare `HEAD`.
    pub fn head(target: &'a str) -> Self {
        Self {
            method: "HEAD",
            target,
            headers: Vec::new(),
        }
    }
    /// Add a header.
    pub fn with(mut self, name: &'a str, value: &'a str) -> Self {
        self.headers.push((name, value));
        self
    }
    fn header(&self, name: &str) -> Option<&'a str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }
    fn cookie(&self, name: &str) -> Option<&'a str> {
        for (k, v) in &self.headers {
            if !k.eq_ignore_ascii_case("cookie") {
                continue;
            }
            for pair in v.split(';') {
                if let Some((n, val)) = pair.split_once('=') {
                    if n.trim() == name {
                        return Some(val.trim());
                    }
                }
            }
        }
        None
    }
}

/// A node's rung-0 catalogue: bundles keyed by root hash, one blob store, one
/// optional payment rail.
///
/// Generic over both seams, so nothing here names a provider: the blob store may
/// be in-memory, on disk, or a Walrus/Arweave binding, and the rail may be the
/// offline mock, the Solana rail, or absent.
pub struct WebHost<B, R> {
    bundles: HashMap<Hash, HostedBundle>,
    blobs: B,
    rail: Option<R>,
}

impl<B, R> WebHost<B, R>
where
    B: BlobStore,
    R: PaymentRail,
{
    /// An empty host over a blob store, with no payment rail.
    ///
    /// A host with no rail can serve free bundles and **refuses every paid one**
    /// ([`crate::entitlement::Refusal::NoRail`]). That is the fail-closed
    /// direction: a node that cannot check entitlements serves no paid content
    /// rather than all of it.
    pub fn new(blobs: B) -> Self {
        Self {
            bundles: HashMap::new(),
            blobs,
            rail: None,
        }
    }

    /// Attach the payment rail used to verify receipts.
    pub fn with_rail(mut self, rail: R) -> Self {
        self.rail = Some(rail);
        self
    }

    /// Publish a bundle. Returns its root hash, which is its URL prefix.
    pub fn publish(&mut self, bundle: HostedBundle) -> Hash {
        let root = bundle.root();
        self.bundles.insert(root, bundle);
        root
    }

    /// Root hashes of every published bundle.
    pub fn roots(&self) -> Vec<Hash> {
        self.bundles.keys().copied().collect()
    }

    /// Look up a published bundle.
    pub fn bundle(&self, root: &Hash) -> Option<&HostedBundle> {
        self.bundles.get(root)
    }

    /// Route and answer one request.
    ///
    /// # The honest cost of receipt-per-request
    ///
    /// A paid bundle's gate runs on **every** file request, because every file
    /// is content the buyer paid for and there is no session token in this
    /// design — the receipt *is* the entitlement (ALIGNMENT.md §4). With
    /// `MockPaymentRail` that is a local signature check and free. With a chain
    /// rail whose `verify_receipt_for_item` re-reads the chain, it is **one RPC
    /// per asset**, and a Godot export is hundreds of assets. That does not
    /// work, and pretending otherwise would be the kind of claim this repo's
    /// `status.md` exists to prevent.
    ///
    /// The fix is a short-lived scoped token minted once from a verified
    /// receipt and checked locally thereafter. It is **not built here**: it is
    /// new protocol, it needs a key the node holds, and inventing it inside a
    /// file server would be the wrong place. What is built is the boundary —
    /// [`crate::entitlement::evaluate`] is one function with one call site, so
    /// the exchange has exactly one place to land (backlog A15).
    ///
    /// A14's [`crate::entitlement::Verdict::Granted`] /
    /// [`crate::entitlement::Verdict::GrantedUnsettled`] split adds a
    /// constraint to that future design, not a reason to build it now: the
    /// token would have to be stamped with the [`magnetite_seams::payment::Settlement`]
    /// tier it was minted under, and a cache lookup would have to re-check
    /// that stamp rather than assume every cached token means `Settled` —
    /// otherwise the token layer would silently launder a signed-but-unsettled
    /// grant into an indistinguishable-from-settled one on the second and
    /// later requests, exactly the conflation this repo's fail-closed rule
    /// forbids at the receipt layer. Naming the constraint here is the whole
    /// contribution; the token itself is still not built.
    ///
    /// Until then: free bundles are fully usable, and paid bundles are usable
    /// with a rail whose verification is local.
    pub async fn handle(&self, req: &RawRequest<'_>) -> BundleResponse {
        let method = match req.method {
            m if m.eq_ignore_ascii_case("GET") => Method::Get,
            m if m.eq_ignore_ascii_case("HEAD") => Method::Head,
            _ => return site_response(405, "method not allowed"),
        };

        // Query strings are not part of a content address. Cache-busting query
        // params are meaningless on an immutable URL, but loaders append them
        // anyway, so they are stripped rather than 404'd.
        let path = req.target.split(['?', '#']).next().unwrap_or("");

        if path == "/healthz" {
            return site_response(200, "ok");
        }

        let Some(rest) = path.strip_prefix(PKG_PREFIX) else {
            return site_response(404, "not found");
        };

        // Split "<root-hex>[/<file-path>]".
        let (root_hex, file_path) = match rest.split_once('/') {
            Some((r, f)) => (r, Some(f)),
            None => (rest, None),
        };
        let Ok(root) = Hash::from_hex(root_hex) else {
            return site_response(404, "not found");
        };
        let Some(bundle) = self.bundles.get(&root) else {
            return site_response(404, "not found");
        };

        // No trailing slash: redirect, or every relative subresource resolves
        // one directory too high. See the module docs.
        let Some(file_path) = file_path else {
            let mut r = site_response(301, "moved permanently");
            r.headers
                .push(("Location".into(), format!("{PKG_PREFIX}{}/", root.to_hex())));
            r.outcome = Outcome::RedirectToSlash;
            return r;
        };

        let Some(decoded) = percent_decode(file_path) else {
            return site_response(400, "malformed percent-encoding in request path");
        };

        // Empty means the bundle root, which `respond` maps to the entry
        // document. Anything else must normalize; a path that does not is
        // reported as a plain 404 rather than a distinct error, so probing for
        // traversal handling learns nothing.
        let resolved = if decoded.is_empty() {
            String::new()
        } else {
            match normalize_path(&decoded) {
                Ok(p) => p,
                Err(_) => return site_response(404, "not found"),
            }
        };

        let credentials = match self.credentials(req) {
            Ok(c) => c,
            Err(why) => return site_response(400, why),
        };

        let mut br = BundleRequest {
            method,
            path: &resolved,
            accept_encoding: req.header("accept-encoding").unwrap_or(""),
            range: req.header("range"),
            if_none_match: req.header("if-none-match"),
            credentials,
        };
        // `HEAD` is set through the struct rather than the builder here because
        // `method` was already parsed.
        br.method = method;

        bundle.respond(&self.blobs, self.rail.as_ref(), &br).await
    }

    /// Pull a receipt out of the header or the cookie.
    ///
    /// Note what is deliberately **not** read: the requester's identity. Binding
    /// a receipt to a session ([`crate::entitlement::Requester::Authenticated`])
    /// is only a restriction, so a client-supplied "I am key X" header would be
    /// worthless — a client would simply supply the buyer's key and satisfy it.
    /// An authenticated requester can only come from the `Identity` seam
    /// verifying a challenge, which is not wired to this path, so this layer
    /// always produces [`crate::entitlement::Requester::Anonymous`] and receipts
    /// are bearer credentials here. Stated plainly rather than implied by a
    /// header that looks like security and is not.
    fn credentials(&self, req: &RawRequest<'_>) -> Result<Credentials, &'static str> {
        let raw = req
            .header(RECEIPT_HEADER)
            .or_else(|| req.cookie(RECEIPT_COOKIE));
        let Some(raw) = raw else {
            return Ok(Credentials::none());
        };
        let bytes = hex::decode(raw.trim()).map_err(|_| "receipt is not valid hex")?;
        let receipt: Receipt =
            serde_json::from_slice(&bytes).map_err(|_| "receipt is not a valid JSON receipt")?;
        // A malformed receipt is a 400, not a silent "no receipt". Both refuse
        // access; only one of them tells the developer what is wrong.
        Ok(Credentials::bearer(receipt))
    }
}

/// A response not scoped to any bundle: health, routing misses, bad requests.
///
/// Carries COOP/COEP too. A `404` for a missing asset is fetched from inside a
/// cross-origin-isolated document, and under COEP a response without the headers
/// is blocked by the browser before the page can observe its status — so the
/// developer sees a network error rather than the 404 that would have told them
/// which path was wrong.
pub fn site_response(status: u16, message: &str) -> BundleResponse {
    let body = format!("{status} {message}\n").into_bytes();
    let headers = vec![
        ("Cross-Origin-Opener-Policy".into(), "same-origin".into()),
        ("Cross-Origin-Embedder-Policy".into(), "require-corp".into()),
        ("Cross-Origin-Resource-Policy".into(), "same-origin".into()),
        ("X-Content-Type-Options".into(), "nosniff".into()),
        ("Cache-Control".into(), "no-store".into()),
        ("Content-Type".into(), "text/plain; charset=utf-8".into()),
        ("Content-Length".into(), body.len().to_string()),
    ];
    BundleResponse {
        status,
        headers,
        body,
        outcome: match status {
            200 => Outcome::Served,
            301 => Outcome::RedirectToSlash,
            405 => Outcome::MethodNotAllowed,
            _ => Outcome::NotFound,
        },
    }
}

/// Whether `isolation` would expose `SharedArrayBuffer`, given a secure context.
///
/// Exists so a caller (a CLI check, a doc test, an operator's preflight) can ask
/// the question ALIGNMENT.md §5 says to test for, without starting a browser.
pub fn exposes_shared_array_buffer(isolation: CrossOriginIsolation) -> bool {
    isolation == CrossOriginIsolation::Enabled
}

/// Decode `%XX` escapes. Returns `None` on a truncated or non-hex escape, or if
/// the result is not UTF-8.
///
/// `+` is left alone: it means a space in a query string, never in a path.
fn percent_decode(s: &str) -> Option<String> {
    fn nib(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() {
                return None;
            }
            out.push(nib(b[i + 1])? * 16 + nib(b[i + 2])?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_is_strict() {
        assert_eq!(percent_decode("a/b.png").unwrap(), "a/b.png");
        assert_eq!(percent_decode("a%20b.png").unwrap(), "a b.png");
        // `..` smuggled through an escape decodes, and is then refused by
        // `normalize_path` — the two layers together are what closes it.
        assert_eq!(percent_decode("%2e%2e/x").unwrap(), "../x");
        assert!(normalize_path(&percent_decode("%2e%2e/x").unwrap()).is_err());
        // Truncated / non-hex escapes are refused rather than passed through.
        assert!(percent_decode("a%2").is_none());
        assert!(percent_decode("a%").is_none());
        assert!(percent_decode("a%zz").is_none());
        // `+` is not a space in a path.
        assert_eq!(percent_decode("a+b").unwrap(), "a+b");
    }

    #[test]
    fn cookies_are_parsed_out_of_a_combined_header() {
        let req = RawRequest::get("/").with("Cookie", "other=1; mag_receipt=deadbeef; z=2");
        assert_eq!(req.cookie(RECEIPT_COOKIE), Some("deadbeef"));
        assert_eq!(req.cookie("absent"), None);
        let none = RawRequest::get("/");
        assert_eq!(none.cookie(RECEIPT_COOKIE), None);
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let req = RawRequest::get("/").with("ACCEPT-ENCODING", "br");
        assert_eq!(req.header("accept-encoding"), Some("br"));
    }
}
