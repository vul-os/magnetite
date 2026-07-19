//! Seam §3.2 — `Naming`.
//!
//! Names are a **display layer over raw keys**; the substrate is always the
//! Ed25519 key. The default [`HashNaming`] uses the full-hex key as the
//! canonical, zero-authority address and derives a short BLAKE3 display handle
//! for UIs. An optional in-memory alias registry lets a node map human names to
//! keys without ever becoming the authority (aliases are local hints).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::identity::PubKey;

/// Human-name ↔ key resolution (§3.2).
#[async_trait::async_trait]
pub trait Naming {
    /// Resolve a human name (or raw-hex address) to a key.
    async fn resolve(&self, name: &str) -> Option<PubKey>;
    /// Render a key as a human-friendly display string.
    fn display(&self, pk: &PubKey) -> String;
}

/// Default naming: raw-hex canonical addresses + short BLAKE3 display handles.
///
/// - `resolve` accepts the canonical 64-char hex key, or any locally registered
///   alias.
/// - `display` returns a stable `mag_<10hex>` short handle (BLAKE3 of the key).
pub struct HashNaming {
    aliases: Mutex<HashMap<String, PubKey>>,
    /// Number of hex chars shown in the short display handle.
    pub short_len: usize,
}

impl Default for HashNaming {
    fn default() -> Self {
        Self {
            aliases: Mutex::new(HashMap::new()),
            short_len: 10,
        }
    }
}

impl HashNaming {
    /// Fresh naming with no aliases.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a local human alias -> key hint (never authoritative).
    pub fn register(&self, name: &str, pk: PubKey) {
        self.aliases.lock().unwrap().insert(name.to_string(), pk);
    }

    /// The canonical, name-less address for a key (full hex).
    pub fn canonical(&self, pk: &PubKey) -> String {
        pk.to_hex()
    }
}

#[async_trait::async_trait]
impl Naming for HashNaming {
    async fn resolve(&self, name: &str) -> Option<PubKey> {
        // Local alias hint wins if present.
        if let Some(pk) = self.aliases.lock().unwrap().get(name) {
            return Some(*pk);
        }
        // Otherwise treat the name as a raw-hex address (the substrate form).
        PubKey::from_hex(name).ok()
    }

    fn display(&self, pk: &PubKey) -> String {
        let digest = blake3::hash(&pk.0);
        let hex = hex::encode(digest.as_bytes());
        format!("mag_{}", &hex[..self.short_len.min(hex.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> PubKey {
        PubKey([byte; 32])
    }

    #[tokio::test]
    async fn canonical_hex_roundtrips() {
        let naming = HashNaming::new();
        let pk = key(0xAB);
        let canonical = naming.canonical(&pk);
        assert_eq!(canonical.len(), 64);
        let resolved = naming.resolve(&canonical).await.expect("resolves hex");
        assert_eq!(resolved, pk);
    }

    #[tokio::test]
    async fn alias_resolves_and_display_is_stable() {
        let naming = HashNaming::new();
        let pk = key(0x11);
        naming.register("alice", pk);
        assert_eq!(naming.resolve("alice").await, Some(pk));

        let d1 = naming.display(&pk);
        let d2 = naming.display(&pk);
        assert_eq!(d1, d2, "display is deterministic");
        assert!(d1.starts_with("mag_"));
        // Distinct keys get distinct handles.
        assert_ne!(naming.display(&pk), naming.display(&key(0x12)));
    }

    #[tokio::test]
    async fn junk_names_do_not_resolve() {
        let naming = HashNaming::new();
        assert_eq!(naming.resolve("not-a-key").await, None);
    }
}
