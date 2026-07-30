//! Shared test scaffolding: a fixture bundle shaped like a Godot 4 web export,
//! and a raw HTTP/1.1 client.
//!
//! # No real engine export was exercised
//!
//! Stated here because it is the load-bearing caveat for every test that uses
//! this file. ALIGNMENT.md §5 says "Test against a real Godot export early". No
//! Godot, Unity or Emscripten toolchain is installed in the environment this was
//! written in — `which godot`, `godot --version`, `/Applications`, Homebrew and
//! Spotlight all came back empty — so [`godot_shaped_files`] *mimics the file
//! layout* of a Godot 4.x web export instead:
//!
//! ```text
//! index.html                  the document
//! index.js                    the loader
//! index.wasm                  the engine module
//! index.wasm.br               precompressed variant of the above
//! index.pck                   the resource pack (the big one — range requests)
//! index.pck.gz                precompressed variant of the above
//! index.audio.worklet.js      AudioWorklet processor
//! index.worker.js             the threads worker (why SharedArrayBuffer is needed)
//! index.icon.png              favicon
//! ```
//!
//! What that fixture *can* prove, and does: the file layout resolves, the headers
//! are right, the encodings pair correctly with their content types, ranges work
//! on the pack, integrity fails closed, and the gate refuses.
//!
//! What it **cannot** prove: that a real Godot 4 export boots. The `.wasm` here
//! is four bytes of magic number, not an engine. The nearest available substitute
//! for that claim is `scripts/verify-web-bundle-isolation.mjs`, which drives a
//! real Chromium against this server and asserts `crossOriginIsolated === true`
//! and that `SharedArrayBuffer` is constructible — the precondition Godot 4
//! actually fails on. Passing it is necessary, not sufficient.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use magnetite_seams::blobstore::{BlobStore, LocalBlobStore};
use magnetite_seams::identity::{PubKey, RawKeypairAuth};
use magnetite_seams::payment::{MockPaymentRail, PaymentRail, PaymentSplit, Receipt};
use magnetite_web_host::manifest::{BundleKind, BundleManifest, FileEntry, Pricing};
use magnetite_web_host::respond::{HostedBundle, ServePolicy};
use magnetite_web_host::{host::WebHost, ingest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Size of the fake resource pack. Big enough that ranges are meaningful,
/// small enough to keep the test fast.
pub const PCK_LEN: usize = 4096;

/// One fixture file: bundle path and bytes.
pub struct Fixture {
    pub path: &'static str,
    pub bytes: Vec<u8>,
}

/// The entry document. Written so the browser check in
/// `scripts/verify-web-bundle-isolation.mjs` can read the isolation state out of
/// the DOM, and so a human opening it sees whether isolation worked.
pub const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>magnetite web-bundle fixture</title></head>
<body>
<canvas id="canvas"></canvas>
<pre id="probe">pending</pre>
<script src="index.js"></script>
</body>
</html>
"#;

/// The loader. Stands in for `index.js` from a Godot export and reports exactly
/// the two facts a Godot 4 export depends on.
pub const INDEX_JS: &str = r#"// Fixture loader. A real Godot 4 export's index.js instantiates index.wasm and
// mounts index.pck; this one only reports whether the environment that
// instantiation requires actually exists.
(function () {
  var state = {
    crossOriginIsolated: (typeof crossOriginIsolated !== 'undefined') && crossOriginIsolated === true,
    sharedArrayBuffer: (typeof SharedArrayBuffer !== 'undefined'),
    sabConstructs: false,
    secureContext: (typeof isSecureContext !== 'undefined') && isSecureContext === true,
  };
  try { new SharedArrayBuffer(8); state.sabConstructs = true; } catch (e) { state.sabConstructs = false; }
  window.__magnetiteProbe = state;
  document.getElementById('probe').textContent = JSON.stringify(state);
})();
"#;

/// Deterministic bytes for a fake resource pack: `GDPC` magic then a counter, so
/// any byte range can be checked against a formula rather than a golden blob.
pub fn pck_bytes() -> Vec<u8> {
    let mut v = Vec::with_capacity(PCK_LEN);
    v.extend_from_slice(b"GDPC");
    while v.len() < PCK_LEN {
        v.push((v.len() % 251) as u8);
    }
    v
}

/// Fake wasm: the real magic number and version, then filler. Enough for a
/// `Content-Type: application/wasm` assertion; not an engine.
pub fn wasm_bytes() -> Vec<u8> {
    let mut v = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    v.extend((0..512u32).map(|i| (i % 97) as u8));
    v
}

/// Real brotli, at the quality an engine's export step would use.
pub fn brotli(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let params = brotli::enc::BrotliEncoderParams {
        quality: 5,
        ..Default::default()
    };
    brotli::BrotliCompress(&mut std::io::Cursor::new(bytes), &mut out, &params)
        .expect("brotli compression of an in-memory buffer cannot fail");
    out
}

/// Real gzip.
pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(bytes).expect("in-memory write");
    e.finish().expect("in-memory finish")
}

/// The fixture file set, in Godot 4 web-export layout. See the module docs for
/// what this does and does not establish.
pub fn godot_shaped_files() -> Vec<Fixture> {
    let wasm = wasm_bytes();
    let pck = pck_bytes();
    vec![
        Fixture {
            path: "index.html",
            bytes: INDEX_HTML.as_bytes().to_vec(),
        },
        Fixture {
            path: "index.js",
            bytes: INDEX_JS.as_bytes().to_vec(),
        },
        Fixture {
            path: "index.wasm",
            bytes: wasm.clone(),
        },
        // Precompressed sibling of index.wasm — the negotiated case.
        Fixture {
            path: "index.wasm.br",
            bytes: brotli(&wasm),
        },
        Fixture {
            path: "index.pck",
            bytes: pck.clone(),
        },
        Fixture {
            path: "index.pck.gz",
            bytes: gzip(&pck),
        },
        Fixture {
            path: "index.audio.worklet.js",
            bytes: b"// AudioWorklet processor\nregisterProcessor;\n".to_vec(),
        },
        // The file that makes SharedArrayBuffer non-optional for Godot 4: the
        // engine's threads build posts its heap to this worker.
        Fixture {
            path: "index.worker.js",
            bytes: b"// threads worker\nself.onmessage = function () {};\n".to_vec(),
        },
        Fixture {
            path: "index.icon.png",
            bytes: b"\x89PNG\r\n\x1a\n fixture".to_vec(),
        },
    ]
}

/// A live host over the fixture, plus the bits a test needs to talk to it.
pub struct Harness {
    pub addr: SocketAddr,
    pub root: magnetite_seams::blobstore::Hash,
    pub blobs: Arc<LocalBlobStore>,
    pub files: Vec<FileEntry>,
    pub receipt_hex: String,
    pub item: String,
}

impl Harness {
    /// Content of a fixture file by path.
    pub fn bytes_of(&self, path: &str) -> Vec<u8> {
        godot_shaped_files()
            .into_iter()
            .find(|f| f.path == path)
            .map(|f| f.bytes)
            .unwrap_or_else(|| panic!("no fixture named {path}"))
    }
    /// The manifest entry for a path.
    pub fn entry_of(&self, path: &str) -> FileEntry {
        self.files
            .iter()
            .find(|f| f.path == path)
            .cloned()
            .unwrap_or_else(|| panic!("no manifest entry for {path}"))
    }
    /// The bundle's URL prefix, e.g. `/pkg/<hex>/`.
    pub fn base(&self) -> String {
        format!("/pkg/{}/", self.root.to_hex())
    }
}

/// Stand up a fixture bundle on a real loopback socket.
///
/// A real TCP listener rather than calling `WebHost::handle` directly, because
/// half of what is under test is whether headers survive an actual HTTP/1.1
/// serialization — `Content-Length` framing on `HEAD`, `206` bodies, header
/// casing. Testing the resolver alone would miss all of it.
pub async fn spawn(pricing_paid: bool, policy: ServePolicy) -> Harness {
    let blobs = Arc::new(LocalBlobStore::new());
    let mut files = Vec::new();
    for f in godot_shaped_files() {
        files.push(ingest(blobs.as_ref(), f.path, &f.bytes).await);
    }

    let free = BundleManifest::new(BundleKind::Web, "index.html", files.clone(), Pricing::Free)
        .expect("fixture manifest is well formed");
    let (pricing, item) = if pricing_paid {
        let p = Pricing::paid_for_bundle(&free.root_hash());
        let item = match &p {
            Pricing::Paid { item } => item.clone(),
            Pricing::Free => unreachable!(),
        };
        (p, item)
    } else {
        (Pricing::Free, String::new())
    };
    let manifest = BundleManifest { pricing, ..free };
    let files = manifest.files.clone();

    let rail = MockPaymentRail::new();
    let receipt = mint_receipt(&rail, &item).await;
    let receipt_hex = hex::encode(serde_json::to_vec(&receipt).expect("receipt serializes"));

    let mut host = WebHost::new(SharedBlobs(Arc::clone(&blobs))).with_rail(rail);
    let root = host.publish(HostedBundle::new(manifest, policy).expect("valid bundle"));

    let (tx, rx) = tokio::sync::oneshot::channel();
    let host = Arc::new(host);
    tokio::spawn(async move {
        let _ = magnetite_web_host::server::serve(
            host,
            "127.0.0.1:0".parse().expect("literal addr"),
            move |a| {
                let _ = tx.send(a);
            },
        )
        .await;
    });
    let addr = rx.await.expect("server reports its bound address");

    Harness {
        addr,
        root,
        blobs,
        files,
        receipt_hex,
        item,
    }
}

/// A `BlobStore` the test keeps a handle to, so it can tamper with the bytes
/// behind the host's back.
pub struct SharedBlobs(pub Arc<LocalBlobStore>);

#[async_trait::async_trait]
impl BlobStore for SharedBlobs {
    async fn put(&self, bytes: &[u8]) -> magnetite_seams::blobstore::Hash {
        self.0.put(bytes).await
    }
    async fn get(&self, hash: &magnetite_seams::blobstore::Hash) -> Option<Vec<u8>> {
        self.0.get(hash).await
    }
    async fn has(&self, hash: &magnetite_seams::blobstore::Hash) -> bool {
        self.0.has(hash).await
    }
}

/// Build the fixture's blobs and manifest without starting a server, so a test
/// can substitute its own blob store (see [`LyingBlobs`]).
pub async fn fixture_manifest() -> (Arc<LocalBlobStore>, BundleManifest) {
    let blobs = Arc::new(LocalBlobStore::new());
    let mut files = Vec::new();
    for f in godot_shaped_files() {
        files.push(ingest(blobs.as_ref(), f.path, &f.bytes).await);
    }
    let m = BundleManifest::new(BundleKind::Web, "index.html", files, Pricing::Free)
        .expect("fixture manifest is well formed");
    (blobs, m)
}

/// Serve an arbitrary blob store, with no payment rail. Returns the bound
/// address and the bundle root.
pub async fn spawn_generic<B>(
    blobs: B,
    manifest: BundleManifest,
) -> (SocketAddr, magnetite_seams::blobstore::Hash)
where
    B: BlobStore + Send + Sync + 'static,
{
    let mut host: WebHost<B, MockPaymentRail> = WebHost::new(blobs);
    let root =
        host.publish(HostedBundle::new(manifest, ServePolicy::default()).expect("valid bundle"));
    let (tx, rx) = tokio::sync::oneshot::channel();
    let host = Arc::new(host);
    tokio::spawn(async move {
        let _ = magnetite_web_host::server::serve(
            host,
            "127.0.0.1:0".parse().expect("literal addr"),
            move |a| {
                let _ = tx.send(a);
            },
        )
        .await;
    });
    (rx.await.expect("bound address"), root)
}

/// A blob store that answers a chosen hash with the wrong bytes, and everything
/// else honestly. Simulates on-disk corruption or a dishonest remote backend.
pub struct LyingBlobs {
    pub inner: Arc<LocalBlobStore>,
    pub lie_about: magnetite_seams::blobstore::Hash,
    pub lie_with: Vec<u8>,
}

#[async_trait::async_trait]
impl BlobStore for LyingBlobs {
    async fn put(&self, bytes: &[u8]) -> magnetite_seams::blobstore::Hash {
        self.inner.put(bytes).await
    }
    async fn get(&self, hash: &magnetite_seams::blobstore::Hash) -> Option<Vec<u8>> {
        if *hash == self.lie_about {
            return Some(self.lie_with.clone());
        }
        self.inner.get(hash).await
    }
    async fn has(&self, _hash: &magnetite_seams::blobstore::Hash) -> bool {
        true
    }
}

/// A deterministic mock receipt bound to `item`.
pub async fn mint_receipt(rail: &MockPaymentRail, item: &str) -> Receipt {
    let buyer: PubKey = RawKeypairAuth::from_seed([42u8; 32]).node_pubkey();
    rail.checkout_for_item(
        &buyer,
        item,
        PaymentSplit::to_developer(RawKeypairAuth::from_seed([43u8; 32]).node_pubkey(), 500),
    )
    .await
    .expect("mock rail always issues")
}

// --- raw HTTP/1.1 client ---------------------------------------------------
//
// Hand-rolled on purpose. An HTTP client library normalizes headers, may
// transparently decompress, and may retry — all of which would hide exactly what
// these tests assert. Writing the request bytes and reading the response bytes
// means every assertion is about what actually went over the wire.

/// A parsed response, with the body as raw bytes (never decompressed).
#[derive(Debug)]
pub struct Wire {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Wire {
    /// First value of a header, case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    /// Assert a header equals a value, with a readable failure.
    pub fn expect_header(&self, name: &str, want: &str) {
        assert_eq!(
            self.header(name),
            Some(want),
            "header {name}: expected {want:?}, got {:?} (status {})",
            self.header(name),
            self.status
        );
    }
    /// Assert a header is absent.
    pub fn expect_no_header(&self, name: &str) {
        assert_eq!(
            self.header(name),
            None,
            "header {name} should be absent, got {:?}",
            self.header(name)
        );
    }
    /// Body as UTF-8, for text responses.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

/// A request builder for the raw client.
pub struct Req {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Req {
    /// `GET target`.
    pub fn get(target: impl Into<String>) -> Self {
        Self {
            method: "GET".into(),
            target: target.into(),
            headers: Vec::new(),
        }
    }
    /// `HEAD target`.
    pub fn head(target: impl Into<String>) -> Self {
        Self {
            method: "HEAD".into(),
            target: target.into(),
            headers: Vec::new(),
        }
    }
    /// Any method, so `405` can be tested.
    pub fn method(method: &str, target: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            target: target.into(),
            headers: Vec::new(),
        }
    }
    /// Add a header.
    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Send it and read the whole response.
    ///
    /// `Connection: close` is always sent so the server closes the socket and
    /// the body is unambiguously "everything until EOF" — no chunked decoding
    /// and no keep-alive bookkeeping in the test client.
    pub async fn send(self, addr: SocketAddr) -> Wire {
        let mut out = format!("{} {} HTTP/1.1\r\n", self.method, self.target);
        out.push_str("Host: 127.0.0.1\r\n");
        for (k, v) in &self.headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        out.push_str("Connection: close\r\n\r\n");

        let mut sock = TcpStream::connect(addr).await.expect("connect to harness");
        sock.write_all(out.as_bytes()).await.expect("write request");
        sock.flush().await.expect("flush");
        let mut raw = Vec::new();
        sock.read_to_end(&mut raw).await.expect("read response");
        parse(&raw)
    }
}

fn parse(raw: &[u8]) -> Wire {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response has a header/body separator");
    let head = std::str::from_utf8(&raw[..split]).expect("headers are ASCII");
    let body = raw[split + 4..].to_vec();

    let mut lines = head.split("\r\n");
    let status_line = lines.next().expect("status line");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("cannot parse status from {status_line:?}"));

    let headers = lines
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (k, v) = l.split_once(':').unwrap_or((l, ""));
            (k.trim().to_string(), v.trim().to_string())
        })
        .collect();

    Wire {
        status,
        headers,
        body,
    }
}
