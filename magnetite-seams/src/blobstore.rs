//! Seam §3.3 — `BlobStore` (content-addressed games + assets).
//!
//! The hash IS the id. A game id is the BLAKE3 hash of its (wasm + manifest)
//! bytes, so no central registry row is needed to identify a game.
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
    async fn get(&self, hash: &Hash) -> Option<Vec<u8>>;
    /// Cheap existence check.
    async fn has(&self, hash: &Hash) -> bool;
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
        let tmp = self.root.join(format!(".tmp-{}-{}", hash.to_hex(), std::process::id()));
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
}
