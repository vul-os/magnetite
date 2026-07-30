//! Seam §3.3 — `BlobStore` (content-addressed games + assets).
//!
//! The hash IS the id: a blob is named by the BLAKE3 hash of its own bytes, so
//! no central registry row is needed to identify anything. A wasm module's id is
//! `Hash::of(module_bytes)`; a web bundle is many blobs, one per file, named the
//! same way. What ties a set of blobs together into a publishable unit — the
//! sorted `path → hash` list, its root hash, the price, the split, the
//! determinism class and the developer's signature — is
//! [`crate::package`], not this seam.
//!
//! Defaults:
//! - [`LocalBlobStore`] — in-memory, BLAKE3-addressed (works fully offline).
//!   Dies with the process, so it is NOT a durability target.
//! - [`FsBlobStore`] — on-disk, BLAKE3-addressed, atomic writes. Blobs outlive
//!   the process; put it on a shared mount for them to outlive the machine.
//! - [`HttpBlobStore`] — a thin read-through stub that fetches a blob by hash
//!   over HTTP. The actual byte transfer is behind the [`BlobFetcher`] trait so
//!   the crate pulls in **no HTTP dependency** and unit-tests without a network.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Result, SeamError};

/// A BLAKE3 content address.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 32]);

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hash({})", hex::encode(self.0))
    }
}

impl Hash {
    /// Compute the content address of some bytes.
    pub fn of(bytes: &[u8]) -> Self {
        Hash(*blake3::hash(bytes).as_bytes())
    }
    /// Lowercase-hex encoding.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
    /// Parse a 64-char hex hash.
    pub fn from_hex(s: &str) -> Result<Self> {
        let raw = hex::decode(s).map_err(|e| SeamError::Invalid(format!("hash hex: {e}")))?;
        let arr: [u8; 32] = raw
            .try_into()
            .map_err(|_| SeamError::Invalid("hash must be 32 bytes".into()))?;
        Ok(Hash(arr))
    }
}

impl Serialize for Hash {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(self.0))
    }
}
impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Hash::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Content-addressed blob storage (§3.3).
#[async_trait::async_trait]
pub trait BlobStore {
    /// Store bytes; the returned [`Hash`] is their content address.
    async fn put(&self, bytes: &[u8]) -> Hash;
    /// Fetch bytes by content address, if present.
    ///
    /// Hands back the **entire** blob as one `Vec<u8>`. That is exactly
    /// right for a small blob and exactly wrong for a multi-gigabyte one —
    /// there is no way to ask this method for anything narrower, on any
    /// backend, because the seam itself has no narrower shape. See
    /// [`get_range`](Self::get_range) for the one that does.
    async fn get(&self, hash: &Hash) -> Option<Vec<u8>>;
    /// Cheap existence check.
    async fn has(&self, hash: &Hash) -> bool;

    /// Fetch a byte range `[offset, offset + len)` of a blob by content
    /// address, without requiring the rest of it ever be materialized.
    ///
    /// This is what makes range serving — HTTP `Range` requests, resumable
    /// downloads, streaming a large game bundle instead of loading it whole —
    /// expressible at all. Before this method existed, [`get`](Self::get) was
    /// the *only* way to read anything out of a [`BlobStore`], and it can only
    /// hand back a fully materialized `Vec<u8>`. A 2 GB Unity bundle is
    /// unservable a byte range at a time through that shape, on a filesystem
    /// backend, an in-memory one, or a future remote one equally — the defect
    /// was in the seam, not in any one implementation.
    ///
    /// Semantics (implementors must preserve these; callers may rely on them):
    /// - Absent hash → `None`, same as [`get`](Self::get).
    /// - `offset >= total_len` (this includes landing exactly at the byte
    ///   count, i.e. one past the last valid index) → `None`: there is no
    ///   byte at that position. This matches how HTTP range parsing already
    ///   treats `start >= total` as unsatisfiable rather than empty.
    /// - `len == 0` at a valid (in-bounds) `offset` → `Some(Vec::new())`: a
    ///   well-formed request for zero bytes, distinct from an out-of-bounds
    ///   one.
    /// - `offset + len` running past `total_len` → clamped to what actually
    ///   exists, so the caller gets the (possibly short) tail rather than an
    ///   error. A range covering `offset: 0, len: total_len` (or larger)
    ///   returns the whole blob.
    ///
    /// # The default is honest, not free
    ///
    /// The default implementation calls [`get`](Self::get) — the *entire*
    /// blob — and slices the result in memory. That is correct for every
    /// existing implementor, including ones written before this method
    /// existed, so adding it does not break anyone. It is **not** an efficient
    /// default: it materializes the whole blob first, which is precisely the
    /// behavior this method exists to let callers avoid. It is provided so a
    /// backend that has not been taught about ranges yet still compiles and
    /// still answers correctly.
    ///
    /// A backend capable of holding a blob too large to fit comfortably in
    /// RAM — a filesystem store that can seek, an HTTP/S3-shaped remote store
    /// that can issue a `Range`/ranged `GetObject` request — **must** override
    /// this method. Leaving such a backend on the default reintroduces the
    /// exact bug this seam exists to fix, one call deeper, silently: the type
    /// signature promises a range read, but the implementation still pulls
    /// everything into memory to produce it.
    ///
    /// # Integrity is weaker here than in `get`, by necessity — and that is
    /// stated, not hidden
    ///
    /// Callers of [`get`](Self::get) (see `magnetite-web-host`) can and do
    /// re-verify the returned bytes against the whole-blob content hash,
    /// because they hold the whole blob. A ranged read fundamentally cannot
    /// carry the same guarantee: hashing a slice proves nothing about a hash
    /// computed over the complete content, and hashing the *whole* blob just
    /// to authenticate a small slice would defeat the point of this method.
    /// A backend or caller that needs tamper-detection on a ranged read has to
    /// provide it itself — a chunked / Merkle-tree hash scheme, for instance —
    /// this seam does not provide one, and nothing here should be read as
    /// claiming it does.
    async fn get_range(&self, hash: &Hash, offset: u64, len: u64) -> Option<Vec<u8>> {
        let bytes = self.get(hash).await?;
        let total = bytes.len() as u64;
        if offset >= total {
            return None;
        }
        let end = offset.saturating_add(len).min(total);
        Some(bytes[offset as usize..end as usize].to_vec())
    }
}

/// In-memory, BLAKE3-addressed default. Offline, no external services.
#[derive(Default)]
pub struct LocalBlobStore {
    inner: Mutex<HashMap<Hash, Vec<u8>>>,
}

impl LocalBlobStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }
    /// Number of stored blobs.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
    /// Whether the store holds no blobs.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait::async_trait]
impl BlobStore for LocalBlobStore {
    async fn put(&self, bytes: &[u8]) -> Hash {
        let h = Hash::of(bytes);
        self.inner.lock().unwrap().insert(h, bytes.to_vec());
        h
    }
    async fn get(&self, hash: &Hash) -> Option<Vec<u8>> {
        self.inner.lock().unwrap().get(hash).cloned()
    }
    async fn has(&self, hash: &Hash) -> bool {
        self.inner.lock().unwrap().contains_key(hash)
    }
}

/// Pluggable byte transport for [`HttpBlobStore`]. A real implementation wraps
/// `reqwest`/`hyper`; tests inject an in-memory fake. Keeping it a trait means
/// this crate never hard-depends on an HTTP client.
#[async_trait::async_trait]
pub trait BlobFetcher: Send + Sync {
    /// GET the body at `url`, or `None` on 404 / error.
    async fn get(&self, url: &str) -> Option<Vec<u8>>;

    /// GET `[offset, offset + len)` of the body at `url` — for a real
    /// implementation this is exactly an HTTP `Range:
    /// bytes=offset-(offset+len-1)` request, and `None` covers both "not
    /// found" and "the server said 416".
    ///
    /// Default: fetches the *whole* body via [`get`](Self::get) and slices
    /// it, which is correct but pulls the entire remote object over the wire
    /// just to answer a range query — exactly the cost this method exists
    /// to avoid. A transport backed by a real HTTP client should override
    /// this to send an actual `Range` header rather than downloading
    /// everything and discarding most of it.
    async fn get_range(&self, url: &str, offset: u64, len: u64) -> Option<Vec<u8>> {
        let bytes = self.get(url).await?;
        let total = bytes.len() as u64;
        if offset >= total {
            return None;
        }
        let end = offset.saturating_add(len).min(total);
        Some(bytes[offset as usize..end as usize].to_vec())
    }
}

/// Thin read-through blob store that serves content by hash over HTTP.
///
/// `get`/`has` fetch `"{base_url}/blob/{hex}"` via the [`BlobFetcher`]. Fetched
/// bytes are **verified against the requested hash** before being returned, so a
/// dishonest server cannot substitute content. `put` is a client-side no-op that
/// only computes the address (uploads belong to a writable backend, not this
/// read-through view) — documented stub per §3.3.
pub struct HttpBlobStore<F: BlobFetcher> {
    base_url: String,
    fetcher: F,
}

impl<F: BlobFetcher> HttpBlobStore<F> {
    /// Build over a base URL (trailing slash tolerated) and a fetcher.
    pub fn new(base_url: impl Into<String>, fetcher: F) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            fetcher,
        }
    }
    /// The canonical fetch URL for a hash.
    pub fn url_for(&self, hash: &Hash) -> String {
        format!("{}/blob/{}", self.base_url, hash.to_hex())
    }
}

#[async_trait::async_trait]
impl<F: BlobFetcher> BlobStore for HttpBlobStore<F> {
    async fn put(&self, bytes: &[u8]) -> Hash {
        // Read-through view: no remote write. The address is computable locally.
        Hash::of(bytes)
    }
    async fn get(&self, hash: &Hash) -> Option<Vec<u8>> {
        let bytes = self.fetcher.get(&self.url_for(hash)).await?;
        // Content addressing is only meaningful if we verify what we got back.
        if Hash::of(&bytes) == *hash {
            Some(bytes)
        } else {
            None
        }
    }
    async fn has(&self, hash: &Hash) -> bool {
        self.get(hash).await.is_some()
    }

    /// Delegates to [`BlobFetcher::get_range`] — a real fetcher turns this
    /// into an actual HTTP `Range:` request, so a remote backend genuinely
    /// avoids pulling the whole object over the wire, not just avoiding it in
    /// local memory.
    ///
    /// Deliberately does **not** hash-verify the result the way `get` does:
    /// this store has no cheap way to learn `total_len` without an extra
    /// request, so unlike [`FsBlobStore`] or the default on
    /// [`BlobStore::get_range`], out-of-bounds `offset`/`len` are not
    /// rejected locally either — that judgment is left to the remote server,
    /// which is the party that actually knows the object's length (a real
    /// HTTP server answers an invalid range with `416`, which a real fetcher
    /// surfaces here as `None`). And per the trait doc: authenticating a
    /// slice against a whole-blob hash is not possible without reading the
    /// whole blob, which would defeat the purpose of asking for a range.
    async fn get_range(&self, hash: &Hash, offset: u64, len: u64) -> Option<Vec<u8>> {
        self.fetcher
            .get_range(&self.url_for(hash), offset, len)
            .await
    }
}

/// On-disk, BLAKE3-addressed store: one file per blob, named by its hex hash.
///
/// # Why this exists
///
/// [`LocalBlobStore`] is in-memory, so anything written to it **dies with the
/// process**. That is fine for tests and for content a node can re-fetch, but it
/// makes it useless as a durability target: a shard checkpoint that vanishes
/// with the node that wrote it cannot be restored by a survivor, which is the
/// entire point of writing one.
///
/// This store writes blobs to a directory, so they outlive the process. What
/// that buys you depends on *where the directory is*, and the distinction
/// matters:
///
/// | Directory | Survives process restart | Survives losing the machine |
/// |---|---|---|
/// | node-local disk | yes | **no** |
/// | shared mount / network filesystem | yes | yes |
///
/// Pointing this at node-local disk and expecting cross-machine recovery is the
/// obvious way to be disappointed at the worst moment. For a survivor on
/// another box to rebuild a dead node's shard, the directory must be reachable
/// from both.
///
/// Writes are atomic: bytes go to a temporary file in the same directory and are
/// renamed into place, so a crash mid-write cannot leave a truncated blob under
/// a hash that claims to describe complete content. Reads re-verify the content
/// address, so a corrupted or tampered file is reported missing rather than
/// returned as if it were genuine.
pub struct FsBlobStore {
    root: std::path::PathBuf,
}

impl FsBlobStore {
    /// Use `root` as the blob directory, creating it if absent.
    pub fn new(root: impl Into<std::path::PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)
            .map_err(|e| SeamError::Invalid(format!("blob dir {}: {e}", root.display())))?;
        Ok(Self { root })
    }

    /// The directory blobs are written to.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn path_for(&self, hash: &Hash) -> std::path::PathBuf {
        self.root.join(hash.to_hex())
    }
}

#[async_trait::async_trait]
impl BlobStore for FsBlobStore {
    async fn put(&self, bytes: &[u8]) -> Hash {
        let hash = Hash::of(bytes);
        let final_path = self.path_for(&hash);
        // Already present: content addressing means identical hash ⇒ identical
        // bytes, so re-writing would be pure cost.
        if final_path.exists() {
            return hash;
        }
        // Write to a temp name in the SAME directory, then rename. Rename is
        // atomic within a filesystem, so a reader never observes a partial blob
        // under a hash that promises whole content.
        let tmp = self
            .root
            .join(format!(".tmp-{}-{}", hash.to_hex(), std::process::id()));
        if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &final_path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        hash
    }

    async fn get(&self, hash: &Hash) -> Option<Vec<u8>> {
        let bytes = std::fs::read(self.path_for(hash)).ok()?;
        // Re-verify: a file that no longer hashes to its own name is corrupt or
        // tampered with, and must read as absent rather than as genuine content.
        if Hash::of(&bytes) == *hash {
            Some(bytes)
        } else {
            None
        }
    }

    async fn has(&self, hash: &Hash) -> bool {
        self.get(hash).await.is_some()
    }

    /// A real range read: seeks to `offset` and reads only `len` (clamped)
    /// bytes, so a 2 GB blob on disk never has to fit in memory whole just to
    /// serve a small slice of it.
    ///
    /// This is the actual point of the seam change — the default on
    /// [`BlobStore::get_range`] would call [`Self::get`] here and defeat that,
    /// so this override exists specifically to avoid it.
    ///
    /// Trade-off, stated rather than hidden: unlike [`Self::get`], this does
    /// **not** re-hash anything before returning, because doing so would mean
    /// reading the whole file — again defeating the point. A tampered file is
    /// still caught by a full [`Self::get`]/[`Self::has`] call, and by the
    /// atomic-write guarantee at [`Self::put`] time (a reader never observes a
    /// partial write), but a ranged read on its own does not re-verify content
    /// against `hash` the way a full read does.
    async fn get_range(&self, hash: &Hash, offset: u64, len: u64) -> Option<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};

        let mut file = std::fs::File::open(self.path_for(hash)).ok()?;
        let total = file.metadata().ok()?.len();
        if offset >= total {
            return None;
        }
        let end = offset.saturating_add(len).min(total);
        let want = (end - offset) as usize;
        if want == 0 {
            return Some(Vec::new());
        }
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut buf = vec![0u8; want];
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_put_get_roundtrip_by_hash() {
        let store = LocalBlobStore::new();
        let wasm = b"\x00asm fake game module + manifest";
        let h = store.put(wasm).await;
        assert_eq!(h, Hash::of(wasm), "id is the content hash");
        assert!(store.has(&h).await);
        assert_eq!(store.get(&h).await.as_deref(), Some(&wasm[..]));
        // Unknown hash misses.
        assert!(!store.has(&Hash::of(b"other")).await);
        assert_eq!(store.get(&Hash::of(b"other")).await, None);
    }

    #[test]
    fn hash_hex_roundtrips() {
        let h = Hash::of(b"abc");
        assert_eq!(Hash::from_hex(&h.to_hex()).unwrap(), h);
    }

    /// In-memory fake server keyed by URL.
    struct FakeServer {
        blobs: std::collections::HashMap<String, Vec<u8>>,
    }
    #[async_trait::async_trait]
    impl BlobFetcher for FakeServer {
        async fn get(&self, url: &str) -> Option<Vec<u8>> {
            self.blobs.get(url).cloned()
        }
    }

    #[tokio::test]
    async fn http_store_fetches_and_verifies_by_hash() {
        let payload = b"served-by-hash".to_vec();
        let h = Hash::of(&payload);

        // Seed a fake server at the exact url the store will request.
        let base = "https://tracker.example";
        let url = format!("{base}/blob/{}", h.to_hex());
        let mut blobs = std::collections::HashMap::new();
        blobs.insert(url.clone(), payload.clone());

        let store = HttpBlobStore::new(base, FakeServer { blobs });
        assert_eq!(store.url_for(&h), url);
        assert_eq!(store.get(&h).await, Some(payload));
        assert!(store.has(&h).await);
        // Missing blob.
        assert_eq!(store.get(&Hash::of(b"absent")).await, None);
    }

    #[tokio::test]
    async fn http_store_rejects_tampered_bytes() {
        // Server returns the WRONG bytes for a hash -> store must reject them.
        let wanted = Hash::of(b"honest");
        let base = "https://evil.example";
        let url = format!("{base}/blob/{}", wanted.to_hex());
        let mut blobs = std::collections::HashMap::new();
        blobs.insert(url, b"tampered".to_vec());
        let store = HttpBlobStore::new(base, FakeServer { blobs });
        assert_eq!(store.get(&wanted).await, None);
    }

    // --- FsBlobStore -------------------------------------------------------

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "magnetite-blobs-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[tokio::test]
    async fn fs_store_roundtrips_by_hash() {
        let dir = temp_dir("roundtrip");
        let store = FsBlobStore::new(&dir).unwrap();
        let h = store.put(b"shard state").await;
        assert_eq!(h, Hash::of(b"shard state"));
        assert!(store.has(&h).await);
        assert_eq!(store.get(&h).await.as_deref(), Some(&b"shard state"[..]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The property that makes this usable as a durability target: a *new*
    /// store opened over the same directory still sees the blob. This is what
    /// `LocalBlobStore` cannot do, and why checkpoints written to it die with
    /// their node.
    #[tokio::test]
    async fn fs_store_survives_being_reopened() {
        let dir = temp_dir("reopen");
        let h = {
            let store = FsBlobStore::new(&dir).unwrap();
            store.put(b"outlives the process").await
        };
        let reopened = FsBlobStore::new(&dir).unwrap();
        assert_eq!(
            reopened.get(&h).await.as_deref(),
            Some(&b"outlives the process"[..])
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file that no longer hashes to its own name must read as ABSENT, not be
    /// handed back as genuine content — otherwise a corrupted checkpoint would
    /// restore a shard to a state nobody ever simulated.
    #[tokio::test]
    async fn fs_store_reports_a_tampered_blob_as_missing() {
        let dir = temp_dir("tampered");
        let store = FsBlobStore::new(&dir).unwrap();
        let h = store.put(b"genuine").await;
        std::fs::write(dir.join(h.to_hex()), b"swapped out").unwrap();
        assert_eq!(store.get(&h).await, None);
        assert!(!store.has(&h).await);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn fs_store_missing_hash_is_none_not_an_error() {
        let dir = temp_dir("missing");
        let store = FsBlobStore::new(&dir).unwrap();
        assert_eq!(store.get(&Hash::of(b"never written")).await, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Temp files from an interrupted write must never be mistaken for blobs.
    #[tokio::test]
    async fn fs_store_ignores_stray_temp_files() {
        let dir = temp_dir("stray");
        let store = FsBlobStore::new(&dir).unwrap();
        std::fs::write(dir.join(".tmp-garbage"), b"half a blob").unwrap();
        let h = store.put(b"real").await;
        assert_eq!(store.get(&h).await.as_deref(), Some(&b"real"[..]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- get_range boundary matrix ------------------------------------
    //
    // The same scenarios are run against every backend that has its own
    // implementation (`LocalBlobStore` on the default, `FsBlobStore` on its
    // seek-based override) so the seam's documented semantics hold
    // regardless of which backend is behind the trait object.

    async fn assert_range_boundaries<S: BlobStore + Sync>(store: &S) {
        let content = b"0123456789"; // total_len = 10
        let h = store.put(content).await;

        // A range spanning the whole blob.
        assert_eq!(
            store.get_range(&h, 0, 10).await.as_deref(),
            Some(&content[..]),
            "whole-blob range"
        );

        // len running past EOF: clamp to what exists, do not error.
        assert_eq!(
            store.get_range(&h, 5, 1000).await.as_deref(),
            Some(&content[5..]),
            "len past EOF clamps to the tail"
        );

        // A middle slice, the ordinary case.
        assert_eq!(
            store.get_range(&h, 2, 3).await.as_deref(),
            Some(&content[2..5]),
            "ordinary middle slice"
        );

        // Zero-length range at a valid offset: well-formed empty result, NOT
        // absent.
        assert_eq!(
            store.get_range(&h, 4, 0).await,
            Some(Vec::new()),
            "zero-length range is Some(empty), not None"
        );

        // offset exactly at EOF (== total_len): no byte exists there.
        assert_eq!(
            store.get_range(&h, 10, 1).await,
            None,
            "offset at EOF is None"
        );
        assert_eq!(
            store.get_range(&h, 10, 0).await,
            None,
            "offset at EOF is None even for a zero-length request"
        );

        // offset past EOF: also None, same bucket as "at EOF".
        assert_eq!(
            store.get_range(&h, 999, 1).await,
            None,
            "offset past EOF is None"
        );

        // Unknown hash: absent, like `get`.
        assert_eq!(
            store.get_range(&Hash::of(b"never stored"), 0, 1).await,
            None,
            "unknown hash is None"
        );
    }

    #[tokio::test]
    async fn local_store_get_range_boundaries() {
        let store = LocalBlobStore::new();
        assert_range_boundaries(&store).await;
    }

    #[tokio::test]
    async fn fs_store_get_range_boundaries() {
        let dir = temp_dir("range-boundaries");
        let store = FsBlobStore::new(&dir).unwrap();
        assert_range_boundaries(&store).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A zero-length blob: every offset is simultaneously "at EOF" and "past
    /// EOF", and both must read as `None`, matching the HTTP-layer rule that a
    /// zero-length representation is always unsatisfiable (`respond.rs`'s
    /// `parse_range` already special-cases this for the same reason).
    #[tokio::test]
    async fn get_range_on_an_empty_blob_is_always_none() {
        let store = LocalBlobStore::new();
        let h = store.put(b"").await;
        assert_eq!(store.get_range(&h, 0, 0).await, None);
        assert_eq!(store.get_range(&h, 0, 5).await, None);
    }

    /// The point of the whole exercise: a blob much larger than would be
    /// comfortable to hold twice in memory (once "on disk", once in the
    /// `Vec` a naive `get` would allocate) round-trips correctly through a
    /// real seek, and a request near the tail does not require reading the
    /// blob from the front.
    #[tokio::test]
    async fn fs_store_get_range_on_a_large_blob_does_not_require_the_whole_thing() {
        let dir = temp_dir("large-range");
        let store = FsBlobStore::new(&dir).unwrap();

        // 8 MiB of deterministic, non-repeating content so a wrong offset is
        // detectable rather than accidentally matching.
        let mut big = Vec::with_capacity(8 * 1024 * 1024);
        for i in 0..(8 * 1024 * 1024u64) {
            big.push((i % 251) as u8);
        }
        let h = store.put(&big).await;

        // A 64 KiB slice near the very end.
        let start = big.len() as u64 - 65_536;
        let got = store.get_range(&h, start, 65_536).await.unwrap();
        assert_eq!(got.len(), 65_536);
        assert_eq!(got, &big[start as usize..]);

        // A slice that runs past the end clamps rather than erroring.
        let tail = store.get_range(&h, big.len() as u64 - 10, 1_000_000).await;
        assert_eq!(tail.as_deref(), Some(&big[big.len() - 10..]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `BlobFetcher::get_range` proves the seam is genuinely extensible to a
    /// remote backend: a fetcher that tracks whether its whole-body `get` was
    /// ever called demonstrates `HttpBlobStore::get_range` reaches the
    /// fetcher's OWN ranged method rather than falling back to fetching
    /// everything — the same shape a real HTTP client would use to issue an
    /// actual `Range:` header instead of downloading the full object.
    struct RangeCountingServer {
        body: Vec<u8>,
        full_get_calls: std::sync::atomic::AtomicUsize,
        range_get_calls: std::sync::atomic::AtomicUsize,
    }
    #[async_trait::async_trait]
    impl BlobFetcher for RangeCountingServer {
        async fn get(&self, _url: &str) -> Option<Vec<u8>> {
            self.full_get_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Some(self.body.clone())
        }
        async fn get_range(&self, _url: &str, offset: u64, len: u64) -> Option<Vec<u8>> {
            self.range_get_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let total = self.body.len() as u64;
            if offset >= total {
                return None;
            }
            let end = offset.saturating_add(len).min(total);
            Some(self.body[offset as usize..end as usize].to_vec())
        }
    }

    #[tokio::test]
    async fn http_store_get_range_uses_the_fetchers_ranged_method_not_a_full_fetch() {
        let body = b"served-in-slices-not-whole".to_vec();
        let h = Hash::of(&body);
        let fetcher = RangeCountingServer {
            body,
            full_get_calls: std::sync::atomic::AtomicUsize::new(0),
            range_get_calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let store = HttpBlobStore::new("https://cdn.example", fetcher);

        let got = store.get_range(&h, 7, 6).await;
        assert_eq!(got.as_deref(), Some(&b"in-sli"[..]));
        assert_eq!(
            store
                .fetcher
                .range_get_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the ranged path must reach BlobFetcher::get_range"
        );
        assert_eq!(
            store
                .fetcher
                .full_get_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            0,
            "get_range must not fall back to fetching the whole body"
        );
    }
}
