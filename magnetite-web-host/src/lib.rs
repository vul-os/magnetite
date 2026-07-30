//! # magnetite-web-host — rung-0 web-bundle hosting
//!
//! Serve a three.js / Godot 4 / Unity / Bevy-web export as a content-addressed
//! package, with an entitlement check. That is the whole job.
//!
//! ALIGNMENT.md §5 draws the line this crate sits on:
//!
//! > **Hosting a web build** = serve content-addressed files over HTTP and check
//! > an entitlement receipt. No VM, no tick loop, no authority, no anti-cheat,
//! > no determinism.
//! >
//! > **Hosting an authoritative match** = the node, wasmtime, ticks, shards,
//! > migration.
//!
//! Nothing in this crate ticks, steps, validates a move, or loads a wasm module.
//! It reads bytes out of a [`BlobStore`](magnetite_seams::blobstore::BlobStore),
//! checks their hash, and writes them to a socket with the right headers. The
//! headers are the hard part.
//!
//! ## Why it is a separate crate
//!
//! See the comment block at the top of `Cargo.toml`. Short version: `backend/`
//! is being deleted and needs Postgres; `magnetite-runtime` path-depends on
//! `backend/magnetite-sdk` and pulls wasmtime, which rung 0 must not inherit;
//! `magnetite-seams` is trait definitions that deliberately have no HTTP
//! dependency. A standalone crate whose only magnetite edge is the seam traits
//! is what is left, and it is the pattern the tree already uses.
//!
//! A node can therefore serve its own catalogue with **no central service, no
//! coordinator, no chain and no network** beyond its own listening socket —
//! ALIGNMENT.md §1's hard requirement.
//!
//! ## The four things that bite, and where each is handled
//!
//! | ALIGNMENT.md §5 bullet | Handled in |
//! |---|---|
//! | COOP/COEP absent ⇒ Godot 4 will not boot | [`respond::CrossOriginIsolation`] |
//! | a bundle is many files, root hash over the sorted `path → hash` list | [`manifest::BundleManifest::root_hash`] |
//! | precompressed `.wasm.br` / `.pck.gz` need the right `Content-Encoding` | [`media`] |
//! | label the determinism boundary | [`manifest::DeterminismClass`] |
//!
//! Plus the two the task adds: per-file BLAKE3 verification that
//! [fails closed](respond::Outcome::IntegrityFailure), and an
//! [entitlement gate](entitlement::evaluate) that refuses when it cannot verify.
//!
//! ## The COEP tradeoff, stated once
//!
//! `SharedArrayBuffer` is exposed only to a cross-origin-isolated document, and
//! isolation requires `Cross-Origin-Embedder-Policy: require-corp`, which blocks
//! every cross-origin subresource that has not opted in. So a bundle served with
//! the headers Godot 4 needs **must be self-contained or same-origin**: no CDN
//! fonts, no remote textures, no third-party analytics, no cross-origin iframes.
//! That is not a policy this crate chose; it is the price of `SharedArrayBuffer`
//! anywhere. It also happens to be the right shape for a content-addressed
//! package — a bundle whose behaviour depends on a third-party host is not
//! reproducible from its root hash. [`respond::CrossOriginIsolation::Disabled`]
//! opts out for bundles that do not need threads, at the cost of Godot 4 not
//! running.
//!
//! ## Status
//!
//! Honest, per `site/docs/status.md` conventions:
//!
//! * The serving path — headers, encodings, ranges, integrity, gate — is built
//!   and tested offline over a real HTTP/1.1 socket.
//! * `crossOriginIsolated === true` and `SharedArrayBuffer` availability are
//!   confirmed in a real Chromium against this server by
//!   `scripts/verify-web-bundle-isolation.mjs`.
//! * **No real Godot 4, Unity or three.js export has been served through it.**
//!   No engine toolchain is installed in the environment this was written in.
//!   The tests use a fixture that mimics the Godot 4 web-export file layout.
//!   Until a real export boots, treat engine compatibility as *expected from the
//!   spec*, not *demonstrated*.
//! * No manifest is signed, and no receipt has ever settled through a real
//!   payment rail — the gate is exercised against `MockPaymentRail`.
//!
//! ## Shape of use
//!
//! ```no_run
//! use std::sync::Arc;
//! use magnetite_seams::blobstore::{BlobStore, LocalBlobStore};
//! use magnetite_seams::payment::MockPaymentRail;
//! use magnetite_web_host::{
//!     manifest::{BundleKind, BundleManifest, FileEntry, Pricing},
//!     respond::HostedBundle,
//!     host::WebHost,
//! };
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let blobs = LocalBlobStore::new();
//! let doc = b"<!doctype html><canvas id=canvas></canvas>";
//! let hash = blobs.put(doc).await;
//!
//! let manifest = BundleManifest::new(
//!     BundleKind::Web,
//!     "index.html",
//!     [FileEntry { path: "index.html".into(), hash, len: doc.len() as u64 }],
//!     Pricing::Free,
//! )?;
//!
//! let mut host: WebHost<_, MockPaymentRail> = WebHost::new(blobs);
//! let root = host.publish(HostedBundle::with_defaults(manifest)?);
//! println!("http://127.0.0.1:8080/pkg/{}/", root.to_hex());
//!
//! magnetite_web_host::server::serve(
//!     Arc::new(host),
//!     "127.0.0.1:8080".parse()?,
//!     |addr| println!("bound {addr}"),
//! ).await?;
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod entitlement;
pub mod error;
pub mod host;
pub mod manifest;
pub mod media;
pub mod respond;

#[cfg(feature = "server")]
pub mod server;

pub use error::{Error, Result};
pub use host::{RawRequest, WebHost, PKG_PREFIX, RECEIPT_COOKIE, RECEIPT_HEADER};
pub use manifest::{BundleKind, BundleManifest, DeterminismClass, FileEntry, Pricing};
pub use respond::{
    BundleRequest, BundleResponse, CrossOriginIsolation, HostedBundle, Method, Outcome, ServePolicy,
};

/// Add a file to a blob store and return the [`FileEntry`] describing it.
///
/// The convenience that keeps the two halves honest: the hash in the manifest is
/// computed from the same bytes that were stored, in one place, so a bundle
/// cannot be built with a manifest that disagrees with its own blobs. Doing it by
/// hand in two steps is how that skew gets in.
pub async fn ingest<B: magnetite_seams::blobstore::BlobStore + ?Sized>(
    blobs: &B,
    path: impl Into<String>,
    bytes: &[u8],
) -> FileEntry {
    let hash = blobs.put(bytes).await;
    FileEntry {
        path: path.into(),
        hash,
        len: bytes.len() as u64,
    }
}
