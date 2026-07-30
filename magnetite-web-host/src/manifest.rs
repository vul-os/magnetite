//! The bundle manifest — `path → hash` for every file, root-hashed over the
//! sorted list.
//!
//! ALIGNMENT.md §5 is specific about why this shape and not a tarball hash:
//!
//! > **A bundle is many files, not one blob.** Content-addressing needs a
//! > manifest of `path → hash` with the root hash taken over that sorted list —
//! > not one hash of a tarball — or per-file caching and HTTP range serving are
//! > lost.
//!
//! Both consequences are load-bearing here. Per-file hashes are what let
//! [`crate::respond`] emit a strong `ETag` and an `immutable` `Cache-Control`
//! for each asset, and what let it verify a single file before serving it
//! instead of rehydrating a whole archive. A tarball hash would force the
//! server to hold and hash the entire bundle to answer a request for one 200-byte
//! `.json`.
//!
//! # Not signed, and not CBOR yet
//!
//! The root hash here is taken over an explicit length-prefixed encoding (see
//! [`BundleManifest::root_hash`]). ALIGNMENT.md §3 requires that
//! content-addressed and signed objects move to **canonical integer-keyed
//! deterministic CBOR** when `kotva-core` is bound, and §7 phase 1 item 2 is
//! the signed manifest itself. Neither is built. So, plainly:
//!
//! * these root hashes **will change** when the encoding moves to canonical
//!   CBOR — treat them as a v1 local format, not a stable public identifier;
//! * a manifest carries **no signature**, so nothing here proves *who*
//!   published a bundle. The operator supplies the manifest. See
//!   [`Pricing::Paid`] for the one place where that actually matters.

use std::collections::BTreeMap;

use magnetite_seams::blobstore::Hash;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Domain separation for the root hash. Bump the suffix if the preimage layout
/// changes, so an old and a new encoding can never collide on a root hash.
const ROOT_HASH_DOMAIN: &[u8] = b"magnetite-web-bundle-root-v1\0";

/// What kind of thing the package is. Rung 0 is `Web` only.
///
/// ALIGNMENT.md §7 phase 1 item 2 specifies `kind: web | wasm | web+wasm`. Only
/// `Web` is represented here because only `Web` is what this crate serves —
/// declaring the other two would be claiming a code path that does not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleKind {
    /// A plain web bundle: three.js, Godot 4 web export, Unity WebGL, Bevy-web.
    /// Served as files. Nothing is executed server-side.
    Web,
}

/// Whether a bundle's frames can be replay-verified. Rung 0: never.
///
/// ALIGNMENT.md §5, "Label the determinism boundary":
///
/// > A Godot or three.js game is not deterministic and cannot be
/// > replay-verified. Say so in the manifest and in the UI. [...] Rung 0 must
/// > not inherit rung 2's claims.
///
/// The precedent is `InputClass::Attested` in the input seam. This enum has one
/// variant on purpose: there is no way to spell "this web bundle is
/// replay-verifiable", because there is no code that could check it. A bundle
/// that wants determinism ships a wasm authority and is rung 1, not rung 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeterminismClass {
    /// Rendered by arbitrary JS/wasm in the player's browser. No authoritative
    /// step function, no `ReplayLog`, nothing to re-simulate. Emitted on every
    /// response as `X-Magnetite-Determinism: not-replay-verifiable` so a client
    /// cannot mistake a rung-0 bundle for a rung-2 match.
    NotReplayVerifiable,
}

impl DeterminismClass {
    /// The header value form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotReplayVerifiable => "not-replay-verifiable",
        }
    }
}

/// Whether serving this bundle requires an entitlement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pricing {
    /// Anyone may fetch every file. No receipt, no rail, no identity.
    Free,
    /// Every file — including the entry document — requires a receipt that
    /// [`magnetite_seams::payment::PaymentRail::verify_receipt_for_item`]
    /// accepts for `item`.
    ///
    /// # The item string must come from a trusted source
    ///
    /// A receipt is bound to an item string, so whoever controls that string
    /// controls what a receipt unlocks. Manifests are **not signed yet**
    /// (ALIGNMENT.md §7 phase 1 item 2), which means an untrusted uploaded
    /// manifest could name a $0.01 item and unlock a $40 bundle. Until signed
    /// manifests exist, take `item` from the operator's own configuration or
    /// from a signed release — never from a publisher-supplied file you did not
    /// verify.
    ///
    /// [`Pricing::paid_for_bundle`] sidesteps the problem entirely by deriving
    /// the item string from the bundle's own root hash, which is not
    /// forgeable: change any byte of any file and the item string changes with
    /// it. Prefer it unless you need one entitlement to span several versions.
    Paid {
        /// The item identifier the receipt must be bound to.
        item: String,
    },
}

impl Pricing {
    /// `Paid` with the item string derived from a bundle root hash:
    /// `"game:<root-hex>"`, matching the `"game:…"` convention the payment
    /// rails' tests already use.
    ///
    /// Unforgeable by construction — the item names the exact bytes — at the
    /// cost of being per-version: a new build is a new root hash is a new item,
    /// so a receipt does not carry across a patch release. That tradeoff is the
    /// right default while manifests are unsigned.
    pub fn paid_for_bundle(root: &Hash) -> Self {
        Self::Paid {
            item: format!("game:{}", root.to_hex()),
        }
    }

    /// Whether an entitlement check is required.
    pub fn is_paid(&self) -> bool {
        matches!(self, Self::Paid { .. })
    }
}

/// One file in the bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Bundle-relative path, e.g. `index.html`, `index.wasm`, `assets/a.png`.
    ///
    /// Normalized by [`BundleManifest::new`]: no leading `/`, no `.` or `..`
    /// segment, no empty segment, no backslash, no NUL, no `%` escape. See
    /// [`normalize_path`].
    pub path: String,
    /// BLAKE3 of the bytes **as stored**, which for `index.wasm.br` means the
    /// hash of the *brotli* bytes, not of the wasm they decode to. The server
    /// transfers stored bytes verbatim and never decompresses, so this is the
    /// hash of exactly what goes on the wire — which is the only hash a
    /// fail-closed check can use.
    pub hash: Hash,
    /// Length of the stored bytes. Redundant with the blob, and checked against
    /// it anyway: a manifest that disagrees with its own content is refused
    /// rather than reconciled.
    pub len: u64,
}

/// A rung-0 web bundle: every file, its hash, and the policy for serving it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    /// Always [`BundleKind::Web`] today.
    pub kind: BundleKind,
    /// The document served for a request to the bundle root, e.g. `index.html`.
    /// Must be present in `files`.
    pub entry: String,
    /// Every file, **sorted by path**, paths unique. [`BundleManifest::new`]
    /// establishes both; deserializing re-checks via [`BundleManifest::validate`].
    pub files: Vec<FileEntry>,
    /// Free, or gated on a receipt.
    pub pricing: Pricing,
    /// Always [`DeterminismClass::NotReplayVerifiable`] for rung 0.
    pub determinism: DeterminismClass,
}

impl BundleManifest {
    /// Build a manifest, normalizing and sorting `files` and rejecting anything
    /// that could not be served safely.
    ///
    /// Refuses: an empty bundle, an `entry` absent from `files`, a duplicate
    /// path (two hashes for one URL is unresolvable, not a preference), and any
    /// path that fails [`normalize_path`].
    pub fn new(
        kind: BundleKind,
        entry: impl Into<String>,
        files: impl IntoIterator<Item = FileEntry>,
        pricing: Pricing,
    ) -> Result<Self> {
        // A BTreeMap gives the sort and the duplicate check in one pass, and
        // its iteration order IS the canonical order the root hash is taken
        // over — so the canonical order cannot drift from the stored order.
        let mut by_path: BTreeMap<String, FileEntry> = BTreeMap::new();
        for mut f in files {
            let path = normalize_path(&f.path)?;
            if let Some(prev) = by_path.get(&path) {
                if prev.hash != f.hash || prev.len != f.len {
                    return Err(Error::DuplicatePath(path));
                }
                // Byte-identical duplicate: harmless, keep the first.
                continue;
            }
            f.path = path.clone();
            by_path.insert(path, f);
        }
        if by_path.is_empty() {
            return Err(Error::EmptyBundle);
        }
        let entry = normalize_path(&entry.into())?;
        if !by_path.contains_key(&entry) {
            return Err(Error::EntryMissing(entry));
        }
        Ok(Self {
            kind,
            entry,
            files: by_path.into_values().collect(),
            pricing,
            determinism: DeterminismClass::NotReplayVerifiable,
        })
    }

    /// Re-check the invariants [`BundleManifest::new`] establishes. Call this on
    /// any manifest that arrived as bytes (deserialized, read off disk, fetched)
    /// rather than through the constructor.
    ///
    /// A manifest is untrusted input. Serde will happily rebuild one with
    /// `../../etc/passwd` in it, or with the same path twice, because the
    /// constructor's checks are not part of the derived `Deserialize`. Skipping
    /// this is how a traversal gets in.
    pub fn validate(&self) -> Result<()> {
        if self.files.is_empty() {
            return Err(Error::EmptyBundle);
        }
        let mut prev: Option<&str> = None;
        for f in &self.files {
            let norm = normalize_path(&f.path)?;
            if norm != f.path {
                return Err(Error::UnnormalizedPath(f.path.clone()));
            }
            match prev {
                Some(p) if p >= f.path.as_str() => {
                    // Equal means duplicate, greater means unsorted. Both break
                    // the binary search in `lookup` and the root hash's
                    // canonical order.
                    return Err(if p == f.path.as_str() {
                        Error::DuplicatePath(f.path.clone())
                    } else {
                        Error::UnsortedFiles
                    });
                }
                _ => {}
            }
            prev = Some(&f.path);
        }
        if normalize_path(&self.entry)? != self.entry {
            return Err(Error::UnnormalizedPath(self.entry.clone()));
        }
        if self.lookup(&self.entry).is_none() {
            return Err(Error::EntryMissing(self.entry.clone()));
        }
        Ok(())
    }

    /// The bundle's content address: BLAKE3 over the domain tag and the sorted
    /// `path → (hash, len)` list, plus `kind` and `entry`.
    ///
    /// Every field is length-prefixed before being hashed. Concatenating
    /// variable-length fields without lengths would let two different manifests
    /// share a preimage — `("ab", "c")` and `("a", "bc")` are the same bytes —
    /// and a content address with collisions is not a content address.
    ///
    /// `entry` and `kind` are inside the hash because they change what the
    /// bundle *is*: the same files with a different entry document is a
    /// different bundle and must not answer to the same URL. `pricing` is
    /// deliberately **outside** it, so that changing a price does not
    /// invalidate every cached asset — and note the consequence, which is that
    /// the root hash does not commit to the price. That is only safe because
    /// pricing is applied by the operator serving the bundle, not carried as a
    /// claim by the bundle. It stops being safe the moment manifests are signed
    /// and a price becomes part of what the signature attests; when that lands,
    /// pricing moves inside the preimage.
    pub fn root_hash(&self) -> Hash {
        let mut pre = Vec::with_capacity(64 + self.files.len() * 48);
        pre.extend_from_slice(ROOT_HASH_DOMAIN);
        pre.push(match self.kind {
            BundleKind::Web => 0u8,
        });
        push_len_prefixed(&mut pre, self.entry.as_bytes());
        pre.extend_from_slice(&(self.files.len() as u64).to_le_bytes());
        for f in &self.files {
            push_len_prefixed(&mut pre, f.path.as_bytes());
            pre.extend_from_slice(&f.hash.0);
            pre.extend_from_slice(&f.len.to_le_bytes());
        }
        Hash::of(&pre)
    }

    /// Find a file by its normalized bundle-relative path.
    ///
    /// This is the *entire* path resolution in this crate. There is no
    /// `Path::join`, no `canonicalize`, no filesystem access on the request
    /// path at any point — a request either names a byte-identical key that the
    /// manifest already listed, or it 404s. Path traversal is therefore not
    /// filtered, it is structurally unrepresentable: `../../etc/passwd` is
    /// rejected by [`normalize_path`] on the way in, and even if it were not,
    /// looking it up would just miss.
    pub fn lookup(&self, path: &str) -> Option<&FileEntry> {
        self.files
            .binary_search_by(|f| f.path.as_str().cmp(path))
            .ok()
            .map(|i| &self.files[i])
    }

    /// Total stored size of the bundle.
    pub fn total_len(&self) -> u64 {
        self.files.iter().map(|f| f.len).sum()
    }
}

fn push_len_prefixed(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Normalize and validate a bundle-relative path, or explain the refusal.
///
/// Accepts `a/b/c.png`. Strips one leading `/` and any leading `./`, because
/// archive tools emit both and the difference is not meaningful. Refuses
/// everything else that could mean two things:
///
/// | Refused | Why |
/// |---|---|
/// | empty, or `/`-terminated | names a directory, not a file |
/// | a `..` segment | traversal, the classic static-server hole |
/// | a `.` segment, or `//` | two spellings of one path defeats hash-keyed caching and lets a gate be bypassed by respelling |
/// | `\`, NUL, control chars | Windows path separators and smuggling |
/// | `%` | a path must arrive already percent-decoded; leaving an escape in means the caller decoded zero or two times, and `%2e%2e` is `..` |
/// | `:` | Windows drive letters and NTFS alternate data streams |
/// | > 1024 bytes | no legitimate bundle path, and it bounds the work |
pub fn normalize_path(raw: &str) -> Result<String> {
    let refuse = |why: &str| {
        Err(Error::BadPath {
            path: raw.to_string(),
            why: why.to_string(),
        })
    };

    if raw.len() > 1024 {
        return refuse("longer than 1024 bytes");
    }
    let mut s = raw.strip_prefix('/').unwrap_or(raw);
    while let Some(rest) = s.strip_prefix("./") {
        s = rest;
    }
    if s.is_empty() {
        return refuse("empty");
    }
    if s.ends_with('/') {
        return refuse("names a directory");
    }
    for seg in s.split('/') {
        match seg {
            "" => return refuse("empty path segment (`//`)"),
            "." => return refuse("`.` path segment"),
            ".." => return refuse("`..` path segment (traversal)"),
            _ => {}
        }
    }
    for c in s.chars() {
        match c {
            '\\' => return refuse("backslash"),
            '%' => return refuse("percent escape (path must arrive decoded)"),
            ':' => return refuse("colon"),
            c if c.is_control() => return refuse("control character"),
            _ => {}
        }
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, body: &[u8]) -> FileEntry {
        FileEntry {
            path: path.into(),
            hash: Hash::of(body),
            len: body.len() as u64,
        }
    }

    fn tiny() -> BundleManifest {
        BundleManifest::new(
            BundleKind::Web,
            "index.html",
            [
                entry("index.html", b"<!doctype html>"),
                entry("g.wasm", b"\0asm"),
            ],
            Pricing::Free,
        )
        .unwrap()
    }

    #[test]
    fn files_are_sorted_and_entry_is_required() {
        let m = tiny();
        assert_eq!(
            m.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            ["g.wasm", "index.html"],
            "constructor sorts regardless of input order"
        );
        assert!(m.lookup("g.wasm").is_some());
        assert!(m.lookup("nope").is_none());

        let missing = BundleManifest::new(
            BundleKind::Web,
            "index.html",
            [entry("only.js", b"x")],
            Pricing::Free,
        );
        assert!(matches!(missing, Err(Error::EntryMissing(_))));
    }

    /// The root hash must be order-insensitive to input and byte-sensitive to
    /// content — that is the whole contract of "root hash over the sorted list".
    #[test]
    fn root_hash_is_order_independent_but_content_sensitive() {
        let a = BundleManifest::new(
            BundleKind::Web,
            "index.html",
            [entry("index.html", b"doc"), entry("a/b.png", b"img")],
            Pricing::Free,
        )
        .unwrap();
        let b = BundleManifest::new(
            BundleKind::Web,
            "index.html",
            [entry("a/b.png", b"img"), entry("index.html", b"doc")],
            Pricing::Free,
        )
        .unwrap();
        assert_eq!(a.root_hash(), b.root_hash(), "input order must not matter");

        let changed = BundleManifest::new(
            BundleKind::Web,
            "index.html",
            [entry("index.html", b"doc"), entry("a/b.png", b"IMG")],
            Pricing::Free,
        )
        .unwrap();
        assert_ne!(a.root_hash(), changed.root_hash(), "one byte must move it");

        // Pricing is outside the preimage, by design (see `root_hash`).
        let mut priced = a.clone();
        priced.pricing = Pricing::paid_for_bundle(&a.root_hash());
        assert_eq!(a.root_hash(), priced.root_hash());
    }

    /// Same files, different entry document ⇒ different bundle. If these
    /// collided, two bundles would share a URL space and a cache.
    #[test]
    fn entry_and_kind_are_inside_the_root_hash() {
        let files = || [entry("a.html", b"A"), entry("b.html", b"B")];
        let a = BundleManifest::new(BundleKind::Web, "a.html", files(), Pricing::Free).unwrap();
        let b = BundleManifest::new(BundleKind::Web, "b.html", files(), Pricing::Free).unwrap();
        assert_ne!(a.root_hash(), b.root_hash());
    }

    /// Length prefixing is what stops `("ab","c")` and `("a","bc")` sharing a
    /// preimage. Without it these two manifests would have the same root hash.
    #[test]
    fn root_hash_length_prefixes_defeat_concatenation_collisions() {
        let a = BundleManifest::new(
            BundleKind::Web,
            "ab",
            [entry("ab", b"x"), entry("c", b"y")],
            Pricing::Free,
        )
        .unwrap();
        let b = BundleManifest::new(
            BundleKind::Web,
            "a",
            [entry("a", b"x"), entry("bc", b"y")],
            Pricing::Free,
        )
        .unwrap();
        assert_ne!(a.root_hash(), b.root_hash());
    }

    #[test]
    fn traversal_and_ambiguous_paths_are_refused() {
        for bad in [
            "../secret",
            "a/../../secret",
            "a/./b",
            "a//b",
            "",
            "/",
            "dir/",
            "a\\b",
            "a%2e%2e/b",
            "C:/win",
            "a\0b",
        ] {
            assert!(
                normalize_path(bad).is_err(),
                "must refuse {bad:?}, it is ambiguous or a traversal"
            );
        }
        // Accepted, with the tolerated prefixes stripped.
        assert_eq!(normalize_path("index.html").unwrap(), "index.html");
        assert_eq!(normalize_path("/index.html").unwrap(), "index.html");
        assert_eq!(normalize_path("./a/b.png").unwrap(), "a/b.png");
        assert_eq!(normalize_path("a/b/c.wasm.br").unwrap(), "a/b/c.wasm.br");
    }

    #[test]
    fn duplicate_paths_with_different_content_are_refused() {
        let dup = BundleManifest::new(
            BundleKind::Web,
            "i.html",
            [entry("i.html", b"one"), entry("i.html", b"two")],
            Pricing::Free,
        );
        assert!(matches!(dup, Err(Error::DuplicatePath(_))));

        // Byte-identical duplicate is collapsed, not refused.
        let same = BundleManifest::new(
            BundleKind::Web,
            "i.html",
            [entry("i.html", b"one"), entry("i.html", b"one")],
            Pricing::Free,
        )
        .unwrap();
        assert_eq!(same.files.len(), 1);
    }

    /// A deserialized manifest has NOT been through the constructor. If
    /// `validate` did not re-run the checks, a hand-written JSON manifest could
    /// smuggle in a traversal path or an unsorted list.
    #[test]
    fn deserialized_manifests_are_revalidated() {
        let good = serde_json::to_string(&tiny()).unwrap();
        let back: BundleManifest = serde_json::from_str(&good).unwrap();
        back.validate().unwrap();
        assert_eq!(back.root_hash(), tiny().root_hash());

        let evil = r#"{
            "kind":"web","entry":"index.html",
            "files":[{"path":"../../etc/passwd","hash":"00","len":0}],
            "pricing":{"kind":"free"},"determinism":"not-replay-verifiable"
        }"#;
        // The bogus hash fails first; use a well-formed one to reach the path check.
        let evil = evil.replace("\"00\"", &format!("\"{}\"", Hash::of(b"x").to_hex()));
        let m: BundleManifest = serde_json::from_str(&evil).unwrap();
        assert!(matches!(m.validate(), Err(Error::BadPath { .. })));

        let unsorted = BundleManifest {
            kind: BundleKind::Web,
            entry: "index.html".into(),
            files: vec![entry("index.html", b"a"), entry("g.wasm", b"b")],
            pricing: Pricing::Free,
            determinism: DeterminismClass::NotReplayVerifiable,
        };
        assert!(matches!(unsorted.validate(), Err(Error::UnsortedFiles)));
    }

    #[test]
    fn derived_paid_item_names_the_exact_bytes() {
        let m = tiny();
        let root = m.root_hash();
        assert_eq!(
            Pricing::paid_for_bundle(&root),
            Pricing::Paid {
                item: format!("game:{}", root.to_hex())
            }
        );
    }
}
