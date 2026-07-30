//! The **game package** — magnetite's unit of publication (`ALIGNMENT.md` §7,
//! Phase 1 item 2).
//!
//! # What changed and why
//!
//! Historically a game *was* a wasm module: the game id was
//! `BLAKE3(wasm + manifest)` bytes and `magnetite-runtime`'s
//! `load_verified_game` re-hashed the blob before running it. That is exactly
//! right for rung 1+ of the capability ladder and useless for rung 0, where the
//! thing being published is a **web bundle** — a three.js / Godot / Unity export
//! that is hundreds of files, has no wasm authority at all, and cannot be
//! replay-verified even in principle.
//!
//! A [`Package`] generalizes the unit of publication to cover both, and to say
//! out loud which one it is:
//!
//! | [`PackageKind`] | Ladder rung | Replay-verifiable |
//! |---|---|---|
//! | [`PackageKind::Web`] | 0 — any web bundle | **never** ([enforced](PackageManifest::validate)) |
//! | [`PackageKind::Wasm`] | 1 — a wasm authority | may be |
//! | [`PackageKind::WebAndWasm`] | 1 — renderer + authority | may be |
//!
//! # Five properties this format is built around
//!
//! 1. **Per-file hashes, not one blob hash.** [`PackageManifest::files`] is a
//!    sorted list of `(path, hash, size)` and [`PackageManifest::root`] is a
//!    hash over that sorted `path → hash` list. It is deliberately **not** the
//!    hash of a tarball: a tarball hash destroys per-file HTTP caching, range
//!    requests, and partial re-download on update, and retrofitting per-file
//!    hashes later would change every published id.
//! 2. **Canonical bytes for the signature.** The signed bytes are
//!    domain-separated canonical integer-keyed deterministic CBOR (see
//!    [`crate::cbor`]) — never serde output. A signature over bytes two honest
//!    implementations can disagree about is worth nothing.
//! 3. **Determinism is a checkable field, not a doc comment.**
//!    [`DeterminismClass`] follows the precedent [`crate::InputClass`] set for
//!    camera gestures: the guarantee is a value with an
//!    [`is_replay_verifiable`](DeterminismClass::is_replay_verifiable) predicate
//!    so the decision is made in code. `kind: web` with
//!    `determinism: deterministic` is **refused at validation**, because a
//!    package with no wasm authority has nothing to re-simulate.
//! 4. **Fail closed, everywhere.** Every check returns `Result`; there is no
//!    "warn and continue" path. A missing file, a size mismatch, a hash
//!    mismatch, a non-canonical byte, an unknown map key, a bad signature, a
//!    split that does not sum, an unrecognized format version — all refuse.
//! 5. **Money is declared by the developer, in the signed release.** The
//!    [`SplitPlan`] rides inside the signed manifest, so the stewards /
//!    co-developer / charity destinations cannot be redirected by node
//!    environment variables (`ALIGNMENT.md` §4 — `SOLANA_FEE_WALLET` and
//!    `PROTOCOL_FEE_BPS` are wired to the wrong party today).
//!
//! # Backwards compatibility with existing wasm-only games
//!
//! **No existing game id changes.** A wasm-only game's id is
//! `BLAKE3(module bytes)`, and [`PackageManifest::wasm_only`] puts exactly that
//! hash in the single [`FileEntry`], so the blob keeps its content address and
//! `load_verified_game` keeps working untouched.
//! [`PackageManifest::legacy_game_id`] hands the pre-package id back so a
//! wasm-only package can still be resolved by it.
//!
//! The [`Package::id`] of that package is a **new, additional** identifier (it
//! commits to the price, the split, the determinism class and the developer key,
//! none of which the old id covered). It is not a replacement for the module's
//! content address and the two are not interchangeable.
//!
//! # Not done here — read before assuming
//!
//! * **Nothing in this module has been run against a real Godot, Unity or
//!   three.js export.** The tests use synthetic bundles. The web-bundle *serving*
//!   concerns from `ALIGNMENT.md` §5 — COOP/COEP headers for
//!   `SharedArrayBuffer`, `Content-Encoding` for `.wasm.br` / `.pck.gz`, range
//!   requests — are a separate Phase 1 item and none of them are implemented.
//! * The reversible-rail entitlement policy (`ALIGNMENT.md` §4: hold until the
//!   settlement window closes, or grant and accept revocation) is *not* a field
//!   here yet. It belongs in this manifest and is deliberately left out rather
//!   than guessed at.
//! * [`crate::payment::PaymentSplit`] **is now the `Vec<Leg>` shape**
//!   `ALIGNMENT.md` §4 specifies — that separate item has since landed. This
//!   module therefore shares its [`Role`] and its resolved [`Leg`] rather than
//!   declaring parallel copies, and [`SplitPlan::resolve`] is the one bridge from
//!   the manifest's proportional [`SplitLeg`] to those absolute legs. (An earlier
//!   revision of this file said `PaymentSplit` "is untouched" and defined its own
//!   `Role`/`ResolvedLeg`; both were written before the payment refactor landed
//!   and were false once it did.)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::blobstore::Hash;
use crate::cbor::{self, CborError, Cv, MapReader};
use crate::identity::{Identity, IdentityVerifier, PubKey, RawKeypairAuth, Sig};
// ONE `Role`, ONE resolved `Leg` — both defined in `crate::payment`, not here.
//
// `ALIGNMENT.md` §4 ("Economics: voluntary legs, not a protocol fee") specifies a
// single `Role { Developer, Operator, Stewards, Other(String) }` shared by both
// layers, and names the rail-layer type `Leg { wallet, amount, role }`. This
// module owns only the *manifest* layer — [`SplitLeg`], which is proportional
// (`share_bps`) because a signature made at publish time cannot commit to an
// absolute amount for a pay-what-you-want purchase.
//
// [`SplitPlan::resolve`] is the bridge between the two, and §4 is explicit that
// "there must be exactly one implementation of that conversion". A second `Role`
// here would put a silent mapping at the money boundary — the same class of
// defect as the duplicate CBOR codecs and the duplicate root hashes recorded in
// `ALIGNMENT.md` §9.
use crate::payment::{Leg, Role};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The only package format version this code accepts. An unknown version is
/// refused rather than best-effort parsed — a verifier that guesses at a future
/// format is a verifier that can be lied to.
pub const PACKAGE_FORMAT_V1: u64 = 1;

/// Domain tag for the root hash over the sorted `path → hash` list.
pub const ROOT_DOMAIN: &[u8] = b"magnetite/package/root/v1";
/// Domain tag for the developer's signature over the manifest.
pub const MANIFEST_DOMAIN: &[u8] = b"magnetite/package/manifest/v1";
/// Domain tag for the package id.
pub const ID_DOMAIN: &[u8] = b"magnetite/package/id/v1";

/// Basis-point denominator: a [`SplitPlan`]'s shares must sum to exactly this.
pub const BPS_TOTAL: u32 = 10_000;

/// Longest single file path admitted in a manifest.
pub const MAX_PATH_LEN: usize = 1024;
/// Most files one package may declare. A bound exists so a hostile manifest
/// cannot make a verifier walk an unbounded list; Godot/Unity exports are
/// dozens to low thousands of files.
pub const MAX_FILES: usize = 65_536;
/// Most split legs one package may declare.
pub const MAX_LEGS: usize = 64;
/// Longest currency label.
pub const MAX_CURRENCY_LEN: usize = 16;
/// Longest [`Role::Other`] label.
pub const MAX_ROLE_LABEL_LEN: usize = 32;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a package was refused. Every variant means **do not load this**.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackageError {
    /// The bytes were not canonical CBOR, or did not match the schema.
    #[error("package encoding: {0}")]
    Cbor(#[from] CborError),

    /// A format version this build does not know.
    #[error("unsupported package format version {0} (this build accepts {PACKAGE_FORMAT_V1})")]
    UnsupportedFormat(u64),

    /// A file path that must not be served or written.
    #[error("invalid file path {path:?}: {why}")]
    InvalidPath {
        /// The offending path.
        path: String,
        /// What is wrong with it.
        why: &'static str,
    },

    /// The file list is not in strictly ascending path order. Canonical form
    /// requires it, so that the signed bytes are reproducible.
    #[error("file list is not sorted / has duplicate paths (at index {0})")]
    FileListNotSorted(usize),

    /// No files at all, or more than [`MAX_FILES`].
    #[error("file count {0} is out of range 1..={MAX_FILES}")]
    FileCount(usize),

    /// The declared root does not match the root computed from the file list.
    #[error("root mismatch: manifest says {want}, file list hashes to {got}")]
    RootMismatch {
        /// The root the manifest declares.
        want: String,
        /// The root recomputed from `files`.
        got: String,
    },

    /// An entry path required by the [`PackageKind`] is absent or unexpected.
    #[error("{0}")]
    Entry(&'static str),

    /// The entry path is not in the file list.
    #[error("entry {0:?} is not listed in the package files")]
    EntryNotListed(String),

    /// A `kind: web` package claimed to be deterministic, or a package with no
    /// wasm authority did. Refused — see [`DeterminismClass`].
    #[error(
        "kind {0} has no wasm authority, so it cannot be deterministic or replay-verifiable; \
         a web-only package MUST declare determinism: non-deterministic"
    )]
    DeterminismClaimWithoutAuthority(&'static str),

    /// A price field is unusable.
    #[error("price: {0}")]
    Price(&'static str),

    /// A split field is unusable.
    #[error("split: {0}")]
    Split(String),

    /// A file listed in the manifest is not available from the source.
    #[error("file {0:?} is missing")]
    FileMissing(String),

    /// A file's byte length is not what the manifest declares. Catches
    /// truncation and padding before the hash is even computed.
    #[error("file {path:?}: manifest declares {want} bytes, source has {got}")]
    FileSize {
        /// Path of the offending file.
        path: String,
        /// Declared length.
        want: u64,
        /// Actual length.
        got: u64,
    },

    /// A file's content hash is not what the manifest declares.
    #[error("file {path:?}: content hashes to {got}, manifest declares {want} — refusing")]
    FileHash {
        /// Path of the offending file.
        path: String,
        /// Declared hash, hex.
        want: String,
        /// Actual hash, hex.
        got: String,
    },

    /// The developer signature did not verify over the canonical manifest bytes.
    #[error("package signature does not verify under the declared developer key")]
    BadSignature,

    /// I/O while reading a package or a bundle directory.
    #[error("io: {0}")]
    Io(String),
}

/// Result alias for package operations.
pub type Result<T> = std::result::Result<T, PackageError>;

// ---------------------------------------------------------------------------
// Kind
// ---------------------------------------------------------------------------

/// What a package contains — and therefore which rung of the capability ladder
/// it sits on (`ALIGNMENT.md` §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PackageKind {
    /// A web bundle only: three.js, Godot 4 web export, Unity WebGL, Bevy-web.
    /// Rung 0 — publish, entitlements, checkout, discovery. **No authority, no
    /// determinism, no replay, no anti-cheat.**
    Web,
    /// A wasm authority only — the historical magnetite game. Rung 1+.
    Wasm,
    /// Both: your own renderer plus a wasm authority stepped from it. Rung 1+.
    WebAndWasm,
}

impl PackageKind {
    /// CBOR discriminant. Stable — part of the wire format.
    fn code(self) -> u64 {
        match self {
            PackageKind::Web => 1,
            PackageKind::Wasm => 2,
            PackageKind::WebAndWasm => 3,
        }
    }

    fn from_code(c: u64) -> std::result::Result<Self, CborError> {
        match c {
            1 => Ok(PackageKind::Web),
            2 => Ok(PackageKind::Wasm),
            3 => Ok(PackageKind::WebAndWasm),
            _ => Err(CborError::UnknownDiscriminant(K_KIND, c)),
        }
    }

    /// Whether this package carries browser-servable content.
    pub fn has_web(self) -> bool {
        matches!(self, PackageKind::Web | PackageKind::WebAndWasm)
    }

    /// Whether this package carries a wasm authority module.
    ///
    /// This is the predicate that gates a [`DeterminismClass::Deterministic`]
    /// claim: with nothing to re-simulate there is nothing to verify.
    pub fn has_wasm(self) -> bool {
        matches!(self, PackageKind::Wasm | PackageKind::WebAndWasm)
    }

    /// Stable lowercase label, as it appears in docs and CLI output.
    pub fn label(self) -> &'static str {
        match self {
            PackageKind::Web => "web",
            PackageKind::Wasm => "wasm",
            PackageKind::WebAndWasm => "web+wasm",
        }
    }

    /// Parse the label form (`"web"`, `"wasm"`, `"web+wasm"`).
    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "web" => Some(PackageKind::Web),
            "wasm" => Some(PackageKind::Wasm),
            "web+wasm" | "webwasm" | "both" => Some(PackageKind::WebAndWasm),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

/// Whether a package's execution can be **replay-verified**.
///
/// This is the same discipline [`crate::InputClass`] applies to input, for the
/// same reason: magnetite's one real moat is that a deterministic sim's log can
/// be re-run by anybody, so a divergence *is* evidence of tampering. That
/// property does not travel. A Godot or three.js build renders on the client
/// with wall-clock time, floating-point drift, GPU state and OS entropy in the
/// loop; there is no seed from which anyone could reproduce it, so "verify the
/// replay" is not a thing that can be attempted, let alone failed.
///
/// Rung 0 must therefore not inherit rung 2's claims, and the check must be
/// mechanical:
///
/// * `kind: web` **must** declare [`Self::NonDeterministic`].
///   [`PackageManifest::validate`] refuses otherwise.
/// * Call [`Self::is_replay_verifiable`] instead of matching on the variant
///   whenever the question is "may I treat this as evidence?" — before settling
///   a wager escrow, publishing a verified leaderboard, or claiming anti-cheat.
/// * A `Deterministic` claim on a package that *does* carry a wasm authority is
///   still only the developer's assertion that their module is deterministic. It
///   means "replay verification is applicable to this package", not "this
///   package has been replay-verified". The sandbox's fuel / memory /
///   `ENOSYS`-on-`random_get` limits are what actually hold the module to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeterminismClass {
    /// Not reproducible; **never** replay-verifiable, at any point, ever.
    /// The only legal value for [`PackageKind::Web`].
    NonDeterministic,
    /// A deterministic wasm authority: replay verification is applicable.
    Deterministic,
}

impl DeterminismClass {
    /// Whether `verify_replay` can say anything about this package.
    ///
    /// **Fails closed by construction**: only `Deterministic` returns `true`.
    pub fn is_replay_verifiable(self) -> bool {
        matches!(self, DeterminismClass::Deterministic)
    }

    fn code(self) -> u64 {
        match self {
            DeterminismClass::NonDeterministic => 1,
            DeterminismClass::Deterministic => 2,
        }
    }

    fn from_code(c: u64) -> std::result::Result<Self, CborError> {
        match c {
            1 => Ok(DeterminismClass::NonDeterministic),
            2 => Ok(DeterminismClass::Deterministic),
            _ => Err(CborError::UnknownDiscriminant(K_DETERMINISM, c)),
        }
    }

    /// Stable lowercase label for docs, CLI output and UI.
    pub fn label(self) -> &'static str {
        match self {
            DeterminismClass::NonDeterministic => "non-deterministic",
            DeterminismClass::Deterministic => "deterministic",
        }
    }
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

/// One file in a package: where it is served, what it hashes to, how long it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// Bundle-relative path, `/`-separated. Never absolute, never containing a
    /// `.` or `..` segment — see [`validate_path`].
    pub path: String,
    /// BLAKE3 content address of this file's bytes. This is the per-file hash
    /// that makes HTTP caching and range serving possible.
    pub hash: Hash,
    /// Byte length. Checked before hashing so truncation is named as truncation.
    ///
    /// Note: `size` is covered by the developer's **signature** but is not an
    /// input to [`PackageManifest::root`]. The root is the address of the
    /// bundle's *content*, and identical content has identical length — a
    /// truncated file fails on its hash regardless, so putting size in the root
    /// would only let a manifest with a wrong size claim a different root for
    /// the same bytes.
    pub size: u64,
}

/// Validate a bundle-relative path.
///
/// This is a security boundary, not tidiness: these paths are joined onto a
/// filesystem root when building and used as HTTP request keys when serving. A
/// `..` segment or a leading `/` is a directory escape.
pub fn validate_path(path: &str) -> Result<()> {
    let bad = |why: &'static str| {
        Err(PackageError::InvalidPath {
            path: path.to_string(),
            why,
        })
    };
    if path.is_empty() {
        return bad("empty");
    }
    if path.len() > MAX_PATH_LEN {
        return bad("longer than MAX_PATH_LEN");
    }
    if path.starts_with('/') {
        return bad("absolute paths are refused");
    }
    if path.ends_with('/') {
        return bad("trailing slash (this is a file list, not a directory list)");
    }
    if path.contains('\\') {
        return bad("backslash (paths are '/'-separated on every platform)");
    }
    if path.contains(':') {
        return bad("colon (breaks on Windows and in URL parsing)");
    }
    if path.chars().any(|c| c.is_control()) {
        return bad("control character");
    }
    for seg in path.split('/') {
        match seg {
            "" => return bad("empty path segment (`//`)"),
            "." => return bad("`.` path segment"),
            ".." => return bad("`..` path segment — directory escape"),
            _ => {}
        }
    }
    Ok(())
}

/// Where a verifier reads a package's file bytes from.
///
/// Synchronous and offline on purpose: verification must be callable from a
/// build tool, a test, and a request handler without dragging in an async
/// runtime or a network client.
pub trait FileSource {
    /// Return the bytes at `path`, or `None` if absent.
    ///
    /// Implementations MUST NOT interpret `path` as anything but a
    /// manifest-listed key, and MUST NOT fall back to a different file when the
    /// exact path is missing.
    fn read(&self, path: &str) -> Option<Vec<u8>>;
}

/// An in-memory bundle. The test and single-process source.
#[derive(Clone, Debug, Default)]
pub struct MemoryFiles(pub BTreeMap<String, Vec<u8>>);

impl MemoryFiles {
    /// Empty bundle.
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    /// Insert a file, replacing any file at the same path.
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.0.insert(path.into(), bytes.into());
    }
    /// Builder-style [`Self::insert`].
    pub fn with(mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        self.insert(path, bytes);
        self
    }
}

impl FileSource for MemoryFiles {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.0.get(path).cloned()
    }
}

/// A bundle rooted at a directory on disk.
///
/// [`FileSource::read`] re-validates the path before joining it, even though a
/// validated manifest cannot contain a bad one — defence in depth, because this
/// type is also reachable with a hand-built path.
#[derive(Clone, Debug)]
pub struct DirFiles(pub PathBuf);

impl DirFiles {
    /// Root a source at `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self(dir.into())
    }
}

impl FileSource for DirFiles {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        if validate_path(path).is_err() {
            return None;
        }
        std::fs::read(self.0.join(path)).ok()
    }
}

// ---------------------------------------------------------------------------
// Price
// ---------------------------------------------------------------------------

/// How a package is priced. Amounts are in the smallest unit of
/// [`PackagePrice::currency`] — integers only, because the signed bytes forbid
/// floats and money in floats is a bug anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceModel {
    /// No payment required. There is exactly one encoding of free — a
    /// [`PriceModel::Fixed`] with `amount: 0` is **refused** at validation, so
    /// "is this free?" has one answer.
    Free,
    /// A set price.
    Fixed {
        /// Required amount, `> 0`.
        amount: u64,
    },
    /// Pay what you want, with a floor and a hint.
    Pwyw {
        /// Smallest accepted payment. May be `0` ("including nothing").
        min: u64,
        /// What the developer suggests. Must be `>= min`.
        suggested: u64,
    },
}

/// A package's price: a model plus the currency its amounts are denominated in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackagePrice {
    /// Free / fixed / pay-what-you-want.
    pub model: PriceModel,
    /// Currency label, e.g. `"USDC"`. Non-empty, ASCII alphanumeric,
    /// `<= MAX_CURRENCY_LEN` bytes. Carried even when
    /// [`PriceModel::Free`] so a later paid version does not change units
    /// silently.
    pub currency: String,
}

impl PackagePrice {
    /// A free package priced in `currency`.
    pub fn free(currency: impl Into<String>) -> Self {
        Self {
            model: PriceModel::Free,
            currency: currency.into(),
        }
    }
    /// A fixed-price package.
    pub fn fixed(amount: u64, currency: impl Into<String>) -> Self {
        Self {
            model: PriceModel::Fixed { amount },
            currency: currency.into(),
        }
    }
    /// A pay-what-you-want package with a floor and a suggestion.
    pub fn pwyw(min: u64, suggested: u64, currency: impl Into<String>) -> Self {
        Self {
            model: PriceModel::Pwyw { min, suggested },
            currency: currency.into(),
        }
    }

    /// The smallest payment that entitles a buyer — `0` for free.
    ///
    /// This is the value to hand [`crate::payment::receipt_admits`], which
    /// already short-circuits `0` to "free needs nothing".
    pub fn min_units(&self) -> u64 {
        match self.model {
            PriceModel::Free => 0,
            PriceModel::Fixed { amount } => amount,
            PriceModel::Pwyw { min, .. } => min,
        }
    }

    /// What the storefront should pre-fill.
    pub fn suggested_units(&self) -> u64 {
        match self.model {
            PriceModel::Free => 0,
            PriceModel::Fixed { amount } => amount,
            PriceModel::Pwyw { suggested, .. } => suggested,
        }
    }

    /// Whether `paid` satisfies this price. **Fails closed** on underpayment.
    pub fn admits(&self, paid: u64) -> bool {
        paid >= self.min_units()
    }

    fn validate(&self) -> Result<()> {
        if self.currency.is_empty() {
            return Err(PackageError::Price("currency label is empty"));
        }
        if self.currency.len() > MAX_CURRENCY_LEN {
            return Err(PackageError::Price("currency label too long"));
        }
        if !self.currency.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(PackageError::Price(
                "currency label must be ASCII alphanumeric",
            ));
        }
        match self.model {
            PriceModel::Free => {}
            PriceModel::Fixed { amount } => {
                if amount == 0 {
                    return Err(PackageError::Price(
                        "fixed price of 0 is refused — use the free model, so there is \
                         exactly one encoding of free",
                    ));
                }
            }
            PriceModel::Pwyw { min, suggested } => {
                if suggested < min {
                    return Err(PackageError::Price(
                        "pay-what-you-want suggested amount is below the minimum",
                    ));
                }
            }
        }
        Ok(())
    }

    fn to_cv(&self) -> Cv {
        // Presence is fixed by the model so there is one canonical encoding:
        // Free → {1,4}; Fixed → {1,2,4}; Pwyw → {1,2,3,4}, with min present
        // even when it is 0.
        let mut m = vec![];
        match self.model {
            PriceModel::Free => m.push((KP_MODEL, Cv::U64(1))),
            PriceModel::Fixed { amount } => {
                m.push((KP_MODEL, Cv::U64(2)));
                m.push((KP_MIN, Cv::U64(amount)));
            }
            PriceModel::Pwyw { min, suggested } => {
                m.push((KP_MODEL, Cv::U64(3)));
                m.push((KP_MIN, Cv::U64(min)));
                m.push((KP_SUGGESTED, Cv::U64(suggested)));
            }
        }
        m.push((KP_CURRENCY, Cv::Text(self.currency.clone())));
        Cv::Map(m)
    }

    fn from_cv(v: Cv) -> std::result::Result<Self, CborError> {
        let mut r = MapReader::new(v)?;
        let model_code = r.u64(KP_MODEL)?;
        let min = r.opt_u64(KP_MIN)?;
        let suggested = r.opt_u64(KP_SUGGESTED)?;
        let currency = r.text(KP_CURRENCY)?;
        r.finish()?;
        let model = match model_code {
            1 => {
                if min.is_some() || suggested.is_some() {
                    return Err(CborError::UnknownKey(KP_MIN));
                }
                PriceModel::Free
            }
            2 => {
                if suggested.is_some() {
                    return Err(CborError::UnknownKey(KP_SUGGESTED));
                }
                PriceModel::Fixed {
                    amount: min.ok_or(CborError::MissingKey(KP_MIN))?,
                }
            }
            3 => PriceModel::Pwyw {
                min: min.ok_or(CborError::MissingKey(KP_MIN))?,
                suggested: suggested.ok_or(CborError::MissingKey(KP_SUGGESTED))?,
            },
            other => return Err(CborError::UnknownDiscriminant(KP_MODEL, other)),
        };
        Ok(Self { model, currency })
    }
}

// ---------------------------------------------------------------------------
// Split
// ---------------------------------------------------------------------------

/// The wire code for a [`Role`] inside a signed manifest.
///
/// Free function rather than a method because [`Role`] is defined in
/// [`crate::payment`] — deliberately, see the note on [`SplitLeg`]. Keeping the
/// codes here keeps manifest wire-format knowledge in the module that owns the
/// manifest format, instead of pushing it into the rail vocabulary.
///
/// These codes are **stable**: they appear in signed bytes. Never renumber; only
/// append.
fn role_code(role: &Role) -> u64 {
    match role {
        Role::Developer => 1,
        Role::Operator => 2,
        Role::Stewards => 3,
        Role::Other(_) => 4,
    }
}

/// One declared share of a package's revenue.
///
/// Shares are **basis points**, not absolute amounts, because a signed manifest
/// has to describe pay-what-you-want and tipped purchases where the total is not
/// known until checkout. [`SplitPlan::resolve`] turns them into the sum-exact
/// absolute legs `ALIGNMENT.md` §4 specifies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitLeg {
    /// Destination wallet (an Ed25519 key doubles as a wallet key).
    pub wallet: PubKey,
    /// Share in basis points, `> 0`. All legs must sum to [`BPS_TOTAL`].
    pub share_bps: u32,
    /// Whose leg this is.
    pub role: Role,
}

// A resolved, absolute payout leg is `crate::payment::Leg` — imported above, not
// redeclared here. `ALIGNMENT.md` §4 names it as the rail layer's type, and the
// rail is the only thing that consumes a resolved leg.

/// The developer's declared revenue split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitPlan {
    /// Legs, in the developer's declared order (order is signed, so it is
    /// stable; it is also the tie-break order in [`Self::resolve`]).
    pub legs: Vec<SplitLeg>,
}

impl SplitPlan {
    /// The whole total to one wallet.
    pub fn all_to(wallet: PubKey, role: Role) -> Self {
        Self {
            legs: vec![SplitLeg {
                wallet,
                share_bps: BPS_TOTAL,
                role,
            }],
        }
    }

    /// Split `total` across the legs so the parts sum to **exactly** `total`.
    ///
    /// Uses largest-remainder allocation: every leg gets
    /// `floor(total * bps / 10000)`, then the leftover units go to the legs with
    /// the largest fractional remainders, ties broken by declared order. So no
    /// unit is invented and none is lost — which matters because Solana gives
    /// all-or-none atomicity for the whole transaction and fail-closed check #10
    /// refuses a payout list with an unaccounted party.
    ///
    /// Zero-share legs cannot exist (validation refuses them), so a resolved leg
    /// is `0` only when `total` is too small to reach it. Legs are returned in
    /// declared order, including any that resolved to `0`, so the caller sees
    /// every declared party.
    pub fn resolve(&self, total: u64) -> Vec<Leg> {
        let mut out: Vec<Leg> = Vec::with_capacity(self.legs.len());
        // (remainder, index) for leftover distribution.
        let mut rems: Vec<(u128, usize)> = Vec::with_capacity(self.legs.len());
        let mut assigned: u64 = 0;
        for (i, leg) in self.legs.iter().enumerate() {
            let num = total as u128 * leg.share_bps as u128;
            let floor = (num / BPS_TOTAL as u128) as u64;
            rems.push((num % BPS_TOTAL as u128, i));
            assigned = assigned.saturating_add(floor);
            out.push(Leg {
                wallet: leg.wallet,
                amount: floor,
                role: leg.role.clone(),
            });
        }
        let mut leftover = total.saturating_sub(assigned);
        // Descending remainder, ascending index — fully deterministic.
        rems.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, i) in rems {
            if leftover == 0 {
                break;
            }
            out[i].amount += 1;
            leftover -= 1;
        }
        out
    }

    fn validate(&self) -> Result<()> {
        if self.legs.is_empty() || self.legs.len() > MAX_LEGS {
            return Err(PackageError::Split(format!(
                "leg count {} is out of range 1..={MAX_LEGS}",
                self.legs.len()
            )));
        }
        let mut sum: u32 = 0;
        let mut seen: Vec<&PubKey> = Vec::with_capacity(self.legs.len());
        let mut has_developer = false;
        for leg in &self.legs {
            if leg.share_bps == 0 {
                return Err(PackageError::Split(
                    "a zero-share leg is refused — omit the leg instead".into(),
                ));
            }
            sum = sum
                .checked_add(leg.share_bps)
                .ok_or_else(|| PackageError::Split("share basis points overflow".into()))?;
            if seen.iter().any(|w| w.0 == leg.wallet.0) {
                // Two legs to one wallet are indistinguishable in the resulting
                // payout list, so the manifest must merge them rather than let a
                // reader disagree about who was paid what.
                return Err(PackageError::Split(format!(
                    "wallet {} appears in more than one leg — merge them",
                    hex::encode(leg.wallet.0)
                )));
            }
            seen.push(&leg.wallet);
            if let Role::Other(label) = &leg.role {
                if label.is_empty() || label.len() > MAX_ROLE_LABEL_LEN {
                    return Err(PackageError::Split(
                        "Role::Other label is empty or too long".into(),
                    ));
                }
                if label.chars().any(|c| c.is_control()) {
                    return Err(PackageError::Split(
                        "Role::Other label contains a control character".into(),
                    ));
                }
            }
            if leg.role == Role::Developer {
                if has_developer {
                    return Err(PackageError::Split("more than one developer leg".into()));
                }
                has_developer = true;
            }
        }
        if sum != BPS_TOTAL {
            return Err(PackageError::Split(format!(
                "shares sum to {sum} bps, must be exactly {BPS_TOTAL}"
            )));
        }
        if !has_developer {
            return Err(PackageError::Split(
                "no developer leg — every package must name where the developer's \
                 share goes, even at 1 bps"
                    .into(),
            ));
        }
        Ok(())
    }

    fn to_cv(&self) -> Cv {
        Cv::Array(
            self.legs
                .iter()
                .map(|leg| {
                    let mut m = vec![
                        (KL_WALLET, Cv::Bytes(leg.wallet.0.to_vec())),
                        (KL_SHARE_BPS, Cv::U64(leg.share_bps as u64)),
                        (KL_ROLE, Cv::U64(role_code(&leg.role))),
                    ];
                    if let Role::Other(label) = &leg.role {
                        m.push((KL_ROLE_LABEL, Cv::Text(label.clone())));
                    }
                    Cv::Map(m)
                })
                .collect(),
        )
    }

    fn from_cv(items: Vec<Cv>) -> std::result::Result<Self, CborError> {
        let mut legs = Vec::with_capacity(items.len());
        for it in items {
            let mut r = MapReader::new(it)?;
            let wallet = r.bytes_exact(KL_WALLET, 32)?;
            let share = r.u64(KL_SHARE_BPS)?;
            let role_code = r.u64(KL_ROLE)?;
            let label = r.opt_text(KL_ROLE_LABEL)?;
            r.finish()?;
            let role = match (role_code, label) {
                (1, None) => Role::Developer,
                (2, None) => Role::Operator,
                (3, None) => Role::Stewards,
                (4, Some(l)) => Role::Other(l),
                // A label on a named role, or a missing label on `Other`, is a
                // second encoding of the same thing — refuse it.
                (1..=3, Some(_)) => return Err(CborError::UnknownKey(KL_ROLE_LABEL)),
                (4, None) => return Err(CborError::MissingKey(KL_ROLE_LABEL)),
                (other, _) => return Err(CborError::UnknownDiscriminant(KL_ROLE, other)),
            };
            let share_bps =
                u32::try_from(share).map_err(|_| CborError::TypeMismatch(KL_SHARE_BPS))?;
            legs.push(SplitLeg {
                wallet: PubKey(
                    <[u8; 32]>::try_from(wallet.as_slice())
                        .map_err(|_| CborError::WrongLength(KL_WALLET, 32, wallet.len()))?,
                ),
                share_bps,
                role,
            });
        }
        Ok(Self { legs })
    }
}

// ---------------------------------------------------------------------------
// Wire-format field numbers
// ---------------------------------------------------------------------------
//
// These integers ARE the schema. Renaming a Rust field is free; changing a
// number here is a format break and needs PACKAGE_FORMAT_V1 bumped.

// PackageManifest
const K_FORMAT: u64 = 1;
const K_KIND: u64 = 2;
const K_WEB_ENTRY: u64 = 3;
const K_WASM_ENTRY: u64 = 4;
const K_FILES: u64 = 5;
const K_ROOT: u64 = 6;
const K_PRICE: u64 = 7;
const K_SPLIT: u64 = 8;
const K_DETERMINISM: u64 = 9;
const K_DEVELOPER: u64 = 10;

// PackagePrice
const KP_MODEL: u64 = 1;
const KP_MIN: u64 = 2;
const KP_SUGGESTED: u64 = 3;
const KP_CURRENCY: u64 = 4;

// SplitLeg
const KL_WALLET: u64 = 1;
const KL_SHARE_BPS: u64 = 2;
const KL_ROLE: u64 = 3;
const KL_ROLE_LABEL: u64 = 4;

// Package (the signed envelope)
const KS_MANIFEST: u64 = 1;
const KS_SIG: u64 = 2;

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// Everything about a package that is *not* its file list — the developer's
/// declared terms.
///
/// Grouped into one struct because these four travel together everywhere and
/// because `determinism` and `developer` must not be positionally confusable
/// with a price or a split at a call site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageTerms {
    /// What it costs.
    pub price: PackagePrice,
    /// Where the money goes.
    pub split: SplitPlan,
    /// Whether it can be replay-verified. Refused as
    /// [`DeterminismClass::Deterministic`] with no wasm authority present.
    pub determinism: DeterminismClass,
    /// The developer identity key that will sign it.
    pub developer: PubKey,
}

/// The signed description of a package.
///
/// Construct with [`Self::web`], [`Self::wasm_only`], [`Self::web_and_wasm`] or
/// [`Self::from_dir`], then [`Self::sign`]. Never trust a manifest that has not
/// been through [`Package::verify`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageManifest {
    /// Format version. Must be [`PACKAGE_FORMAT_V1`].
    pub format: u64,
    /// What this package contains.
    pub kind: PackageKind,
    /// Path a browser loads first (e.g. `index.html`). Present **iff**
    /// `kind.has_web()`.
    pub web_entry: Option<String>,
    /// Path of the wasm authority module (e.g. `game.wasm`). Present **iff**
    /// `kind.has_wasm()`.
    pub wasm_entry: Option<String>,
    /// Every file in the package, sorted ascending by path bytes, no duplicates.
    pub files: Vec<FileEntry>,
    /// Hash over the sorted `path → hash` list — see [`Self::compute_root`].
    pub root: Hash,
    /// The developer's price.
    pub price: PackagePrice,
    /// The developer's revenue split.
    pub split: SplitPlan,
    /// Whether this package can be replay-verified. Refused as
    /// [`DeterminismClass::Deterministic`] when there is no wasm authority.
    pub determinism: DeterminismClass,
    /// The developer identity key. Bound **inside** the signed bytes, so a
    /// third party cannot re-sign this manifest and claim authorship of it.
    pub developer: PubKey,
}

impl PackageManifest {
    /// The path a client loads first — the spec's single `entry`.
    ///
    /// The web entry wins when both exist: for `web+wasm` the browser loads the
    /// page, which then loads the authority.
    pub fn entry(&self) -> &str {
        self.web_entry
            .as_deref()
            .or(self.wasm_entry.as_deref())
            .unwrap_or_default()
    }

    /// Whether this package can be replay-verified. Prefer this over reading
    /// [`Self::determinism`] directly.
    pub fn is_replay_verifiable(&self) -> bool {
        self.determinism.is_replay_verifiable()
    }

    /// Look up a listed file by exact path — `O(log n)` over the sorted list.
    ///
    /// A web server MUST route through this and serve nothing it does not
    /// return: an unlisted file in the bundle directory is outside the signature
    /// and must be unreachable.
    pub fn file(&self, path: &str) -> Option<&FileEntry> {
        self.files
            .binary_search_by(|e| e.path.as_str().cmp(path))
            .ok()
            .map(|i| &self.files[i])
    }

    /// Total declared size of the package in bytes.
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// The pre-package game id of a wasm-only package, if it has one.
    ///
    /// **This is the compatibility hinge.** Before packages existed, a game id
    /// was `BLAKE3(module bytes)`. For a [`PackageKind::Wasm`] package holding
    /// exactly one file, that hash is still right there in the file list, so an
    /// existing wasm-only game resolves by its existing id and its blob keeps
    /// its existing content address. Returns `None` for anything else — a
    /// multi-file or web-bearing package never had a legacy id.
    pub fn legacy_game_id(&self) -> Option<Hash> {
        if self.kind == PackageKind::Wasm && self.files.len() == 1 {
            Some(self.files[0].hash)
        } else {
            None
        }
    }

    /// Hash over the sorted `(path, hash)` list.
    ///
    /// **Sorting happens here.** The list is sorted by path before hashing, so a
    /// caller who assembles files in directory-walk order, or in reverse, gets
    /// the same root — the root addresses a *set* of files, not a sequence.
    /// (`validate` separately requires the stored list to already be in that
    /// order, so the signed bytes are reproducible.)
    ///
    /// `size` is not an input; see [`FileEntry::size`] for why.
    pub fn compute_root(files: &[FileEntry]) -> Hash {
        let mut sorted: Vec<&FileEntry> = files.iter().collect();
        sorted.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        let list = Cv::Array(
            sorted
                .iter()
                .map(|f| Cv::Array(vec![Cv::Text(f.path.clone()), Cv::Bytes(f.hash.0.to_vec())]))
                .collect(),
        );
        let mut h = blake3::Hasher::new();
        h.update(ROOT_DOMAIN);
        h.update(&cbor::encode(&list));
        Hash(*h.finalize().as_bytes())
    }

    /// Sort the file list and recompute [`Self::root`] from it.
    ///
    /// Call after mutating `files`. `sign` does it for you.
    pub fn canonicalize(&mut self) {
        self.files
            .sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        self.root = Self::compute_root(&self.files);
    }

    /// Check every structural invariant. **Fails closed.**
    ///
    /// Checked here, in this order:
    ///
    /// 1. known format version,
    /// 2. file count in range, every path valid, list strictly ascending,
    /// 3. entry paths present exactly as the `kind` requires, and listed,
    /// 4. no determinism claim without a wasm authority,
    /// 5. `root` matches the file list,
    /// 6. price is usable and has one canonical encoding,
    /// 7. split legs sum to exactly [`BPS_TOTAL`] and name a developer.
    pub fn validate(&self) -> Result<()> {
        if self.format != PACKAGE_FORMAT_V1 {
            return Err(PackageError::UnsupportedFormat(self.format));
        }

        if self.files.is_empty() || self.files.len() > MAX_FILES {
            return Err(PackageError::FileCount(self.files.len()));
        }
        for (i, f) in self.files.iter().enumerate() {
            validate_path(&f.path)?;
            if i > 0 && self.files[i - 1].path.as_bytes() >= f.path.as_bytes() {
                return Err(PackageError::FileListNotSorted(i));
            }
        }

        match (self.kind.has_web(), &self.web_entry) {
            (true, None) => {
                return Err(PackageError::Entry(
                    "kind includes web but no web entry is declared",
                ))
            }
            (false, Some(_)) => {
                return Err(PackageError::Entry(
                    "a web entry is declared but the kind has no web half",
                ))
            }
            _ => {}
        }
        match (self.kind.has_wasm(), &self.wasm_entry) {
            (true, None) => {
                return Err(PackageError::Entry(
                    "kind includes wasm but no wasm entry is declared",
                ))
            }
            (false, Some(_)) => {
                return Err(PackageError::Entry(
                    "a wasm entry is declared but the kind has no wasm half",
                ))
            }
            _ => {}
        }
        for e in [self.web_entry.as_deref(), self.wasm_entry.as_deref()]
            .into_iter()
            .flatten()
        {
            validate_path(e)?;
            if self.file(e).is_none() {
                return Err(PackageError::EntryNotListed(e.to_string()));
            }
        }

        // The rung-0 guarantee boundary, enforced rather than documented.
        if self.determinism.is_replay_verifiable() && !self.kind.has_wasm() {
            return Err(PackageError::DeterminismClaimWithoutAuthority(
                self.kind.label(),
            ));
        }

        let computed = Self::compute_root(&self.files);
        if computed != self.root {
            return Err(PackageError::RootMismatch {
                want: self.root.to_hex(),
                got: computed.to_hex(),
            });
        }

        self.price.validate()?;
        self.split.validate()?;
        Ok(())
    }

    /// Check that a source really holds the declared bytes. **Fails closed** on
    /// the first missing file, wrong length, or wrong hash.
    ///
    /// Length is checked before the hash so a truncated file is reported as a
    /// truncation rather than as an opaque hash mismatch.
    pub fn verify_contents<S: FileSource + ?Sized>(&self, src: &S) -> Result<()> {
        for f in &self.files {
            let bytes = src
                .read(&f.path)
                .ok_or_else(|| PackageError::FileMissing(f.path.clone()))?;
            if bytes.len() as u64 != f.size {
                return Err(PackageError::FileSize {
                    path: f.path.clone(),
                    want: f.size,
                    got: bytes.len() as u64,
                });
            }
            let got = Hash::of(&bytes);
            if got != f.hash {
                return Err(PackageError::FileHash {
                    path: f.path.clone(),
                    want: f.hash.to_hex(),
                    got: got.to_hex(),
                });
            }
        }
        Ok(())
    }

    // -- constructors --------------------------------------------------------

    fn new(
        kind: PackageKind,
        web_entry: Option<String>,
        wasm_entry: Option<String>,
        files: Vec<FileEntry>,
        terms: PackageTerms,
    ) -> Self {
        let mut m = Self {
            format: PACKAGE_FORMAT_V1,
            kind,
            web_entry,
            wasm_entry,
            files,
            root: Hash([0u8; 32]),
            price: terms.price,
            split: terms.split,
            determinism: terms.determinism,
            developer: terms.developer,
        };
        m.canonicalize();
        m
    }

    /// A rung-0 web bundle. Always [`DeterminismClass::NonDeterministic`] —
    /// there is no parameter for it, because there is no other legal value.
    pub fn web(
        web_entry: impl Into<String>,
        files: Vec<FileEntry>,
        price: PackagePrice,
        split: SplitPlan,
        developer: PubKey,
    ) -> Self {
        Self::new(
            PackageKind::Web,
            Some(web_entry.into()),
            None,
            files,
            PackageTerms {
                price,
                split,
                // Not a parameter: there is no other legal value for `kind: web`.
                determinism: DeterminismClass::NonDeterministic,
                developer,
            },
        )
    }

    /// A wasm-authority package from the module bytes.
    ///
    /// The single [`FileEntry`] hash is `BLAKE3(wasm)`, i.e. **the existing game
    /// id**, so the module keeps its content address and
    /// `magnetite_runtime::node::load_verified_game` still resolves it.
    pub fn wasm_only(
        wasm_entry: impl Into<String>,
        wasm: &[u8],
        price: PackagePrice,
        split: SplitPlan,
        determinism: DeterminismClass,
        developer: PubKey,
    ) -> Self {
        let wasm_entry = wasm_entry.into();
        let files = vec![FileEntry {
            path: wasm_entry.clone(),
            hash: Hash::of(wasm),
            size: wasm.len() as u64,
        }];
        Self::new(
            PackageKind::Wasm,
            None,
            Some(wasm_entry),
            files,
            PackageTerms {
                price,
                split,
                determinism,
                developer,
            },
        )
    }

    /// A rung-1 package: a web bundle plus a wasm authority.
    pub fn web_and_wasm(
        web_entry: impl Into<String>,
        wasm_entry: impl Into<String>,
        files: Vec<FileEntry>,
        price: PackagePrice,
        split: SplitPlan,
        determinism: DeterminismClass,
        developer: PubKey,
    ) -> Self {
        Self::new(
            PackageKind::WebAndWasm,
            Some(web_entry.into()),
            Some(wasm_entry.into()),
            files,
            PackageTerms {
                price,
                split,
                determinism,
                developer,
            },
        )
    }

    /// Walk `dir` and build a manifest over every regular file in it.
    ///
    /// * Paths are `/`-separated and relative to `dir`; every one is validated.
    /// * **Symlinks are refused**, not followed: a symlink out of the bundle
    ///   would publish a hash for content that is not in the bundle.
    /// * Nothing is skipped. A stray `.DS_Store` or `.git` entry lands in the
    ///   manifest and changes the root — silently excluding files would make the
    ///   root depend on an invisible rule. Clean the directory, or exclude
    ///   before calling.
    /// * The result is deterministic: identical directory contents give
    ///   identical `files`, `root`, signing bytes and id, on any platform and in
    ///   any filesystem order. No timestamps are recorded.
    pub fn from_dir(
        dir: &Path,
        kind: PackageKind,
        web_entry: Option<String>,
        wasm_entry: Option<String>,
        terms: PackageTerms,
    ) -> Result<Self> {
        let mut files = Vec::new();
        collect_dir(dir, dir, &mut files)?;
        Ok(Self::new(kind, web_entry, wasm_entry, files, terms))
    }

    // -- canonical bytes ----------------------------------------------------

    /// This manifest as a canonical CBOR value.
    fn to_cv(&self) -> Cv {
        let mut m = vec![
            (K_FORMAT, Cv::U64(self.format)),
            (K_KIND, Cv::U64(self.kind.code())),
            (
                K_FILES,
                Cv::Array(
                    self.files
                        .iter()
                        .map(|f| {
                            Cv::Array(vec![
                                Cv::Text(f.path.clone()),
                                Cv::Bytes(f.hash.0.to_vec()),
                                Cv::U64(f.size),
                            ])
                        })
                        .collect(),
                ),
            ),
            (K_ROOT, Cv::Bytes(self.root.0.to_vec())),
            (K_PRICE, self.price.to_cv()),
            (K_SPLIT, self.split.to_cv()),
            (K_DETERMINISM, Cv::U64(self.determinism.code())),
            (K_DEVELOPER, Cv::Bytes(self.developer.0.to_vec())),
        ];
        if let Some(e) = &self.web_entry {
            m.push((K_WEB_ENTRY, Cv::Text(e.clone())));
        }
        if let Some(e) = &self.wasm_entry {
            m.push((K_WASM_ENTRY, Cv::Text(e.clone())));
        }
        Cv::Map(m)
    }

    /// The exact bytes the developer signs: `MANIFEST_DOMAIN ‖ canonical CBOR`.
    ///
    /// Domain-separated so a manifest signature can never be replayed as a
    /// session-ad, receipt or attested-input signature by the same key.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let body = cbor::encode(&self.to_cv());
        let mut out = Vec::with_capacity(MANIFEST_DOMAIN.len() + body.len());
        out.extend_from_slice(MANIFEST_DOMAIN);
        out.extend_from_slice(&body);
        out
    }

    /// Parse a manifest from canonical CBOR bytes. **Structure only** — call
    /// [`Self::validate`] (or go through [`Package::verify`]) before trusting it.
    pub fn from_canonical_cbor(bytes: &[u8]) -> Result<Self> {
        Self::from_cv(cbor::decode(bytes)?)
    }

    fn from_cv(v: Cv) -> Result<Self> {
        let mut r = MapReader::new(v)?;
        let format = r.u64(K_FORMAT)?;
        // Refuse an unknown version before reading anything else: field numbers
        // are only meaningful within a version.
        if format != PACKAGE_FORMAT_V1 {
            return Err(PackageError::UnsupportedFormat(format));
        }
        let kind = PackageKind::from_code(r.u64(K_KIND)?)?;
        let web_entry = r.opt_text(K_WEB_ENTRY)?;
        let wasm_entry = r.opt_text(K_WASM_ENTRY)?;
        let files_cv = r.array(K_FILES)?;
        let root = r.bytes_exact(K_ROOT, 32)?;
        let price = PackagePrice::from_cv(r.require(K_PRICE)?)?;
        let split = SplitPlan::from_cv(match r.require(K_SPLIT)? {
            Cv::Array(a) => a,
            _ => return Err(PackageError::Cbor(CborError::TypeMismatch(K_SPLIT))),
        })?;
        let determinism = DeterminismClass::from_code(r.u64(K_DETERMINISM)?)?;
        let developer = r.bytes_exact(K_DEVELOPER, 32)?;
        r.finish()?;

        let mut files = Vec::with_capacity(files_cv.len());
        for f in files_cv {
            let items = match f {
                Cv::Array(a) if a.len() == 3 => a,
                _ => return Err(PackageError::Cbor(CborError::TypeMismatch(K_FILES))),
            };
            let path = match &items[0] {
                Cv::Text(s) => s.clone(),
                _ => return Err(PackageError::Cbor(CborError::TypeMismatch(K_FILES))),
            };
            let hash = match &items[1] {
                Cv::Bytes(b) if b.len() == 32 => Hash(<[u8; 32]>::try_from(b.as_slice()).unwrap()),
                Cv::Bytes(b) => {
                    return Err(PackageError::Cbor(CborError::WrongLength(
                        K_FILES,
                        32,
                        b.len(),
                    )))
                }
                _ => return Err(PackageError::Cbor(CborError::TypeMismatch(K_FILES))),
            };
            let size = match &items[2] {
                Cv::U64(n) => *n,
                _ => return Err(PackageError::Cbor(CborError::TypeMismatch(K_FILES))),
            };
            files.push(FileEntry { path, hash, size });
        }

        Ok(Self {
            format,
            kind,
            web_entry,
            wasm_entry,
            files,
            root: Hash(<[u8; 32]>::try_from(root.as_slice()).unwrap()),
            price,
            split,
            determinism,
            developer: PubKey(<[u8; 32]>::try_from(developer.as_slice()).unwrap()),
        })
    }

    // -- signing ------------------------------------------------------------

    /// Canonicalize, validate, and sign with the developer's key.
    ///
    /// Refuses if `signer`'s public key is not the manifest's declared
    /// [`Self::developer`] — signing under a key the manifest does not name
    /// would produce a package that can never verify.
    ///
    /// **A7 finish.** Generic over any [`Identity`], not just
    /// [`RawKeypairAuth`] — every existing caller passes a `&RawKeypairAuth`
    /// today and keeps compiling unchanged (the compiler infers `I =
    /// RawKeypairAuth`), but a package can now genuinely be signed by any
    /// provider. Pair with [`Package::verify_for`], which is the
    /// provider-generic counterpart of [`Package::verify`].
    pub fn sign<I: Identity>(mut self, signer: &I) -> Result<Package> {
        if signer.pubkey().0 != self.developer.0 {
            return Err(PackageError::BadSignature);
        }
        self.canonicalize();
        self.validate()?;
        let sig = signer.sign(&self.signing_bytes());
        Ok(Package {
            manifest: self,
            sig,
        })
    }
}

/// Recursively collect regular files under `root`, refusing symlinks.
fn collect_dir(root: &Path, dir: &Path, out: &mut Vec<FileEntry>) -> Result<()> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| PackageError::Io(format!("{}: {e}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| PackageError::Io(format!("{}: {e}", dir.display())))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| PackageError::Io(format!("{}: {e}", path.display())))?;
        if meta.file_type().is_symlink() {
            return Err(PackageError::Io(format!(
                "{} is a symlink — refusing to publish a hash for content outside the bundle",
                path.display()
            )));
        }
        if meta.is_dir() {
            collect_dir(root, &path, out)?;
            continue;
        }
        if !meta.is_file() {
            return Err(PackageError::Io(format!(
                "{} is neither a regular file nor a directory",
                path.display()
            )));
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| PackageError::Io(format!("{} escaped the bundle root", path.display())))?;
        let mut parts = Vec::new();
        for comp in rel.components() {
            match comp {
                std::path::Component::Normal(s) => parts.push(
                    s.to_str()
                        .ok_or_else(|| PackageError::InvalidPath {
                            path: rel.to_string_lossy().into_owned(),
                            why: "path is not valid UTF-8",
                        })?
                        .to_string(),
                ),
                _ => {
                    return Err(PackageError::InvalidPath {
                        path: rel.to_string_lossy().into_owned(),
                        why: "unexpected path component",
                    })
                }
            }
        }
        let rel_path = parts.join("/");
        validate_path(&rel_path)?;
        let bytes = std::fs::read(&path)
            .map_err(|e| PackageError::Io(format!("{}: {e}", path.display())))?;
        out.push(FileEntry {
            path: rel_path,
            hash: Hash::of(&bytes),
            size: bytes.len() as u64,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Signed package
// ---------------------------------------------------------------------------

/// A [`PackageManifest`] plus the developer's signature over its canonical
/// bytes. **This is the unit of publication.**
///
/// A `Package` that came from anywhere but [`Self::verify`] has proven nothing.
/// Get a [`VerifiedPackage`] before serving, loading, or charging for anything.
#[derive(Clone, Debug)]
pub struct Package {
    /// The signed manifest.
    pub manifest: PackageManifest,
    /// Ed25519 signature by `manifest.developer` over
    /// [`PackageManifest::signing_bytes`].
    pub sig: Sig,
}

// `Sig` is deliberately not `PartialEq` in `identity`, so equality is written
// out here rather than derived. Signature bytes are public, so a plain
// comparison is fine — and equality is only used for round-trip assertions,
// never as an authorization decision (that is [`Package::verify`]).
impl PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest && self.sig.0 == other.sig.0
    }
}
impl Eq for Package {}

/// A [`Package`] whose structure, root and signature have all been checked.
///
/// Only obtainable from [`Package::verify`], and it borrows the package rather
/// than copying it, so there is no way to construct one by mistake. Getting one
/// does **not** mean the file bytes are present — call
/// [`Self::verify_contents`] for that.
#[derive(Debug)]
pub struct VerifiedPackage<'a> {
    inner: &'a Package,
}

impl<'a> VerifiedPackage<'a> {
    /// The verified manifest.
    pub fn manifest(&self) -> &'a PackageManifest {
        &self.inner.manifest
    }
    /// The verified package's id.
    pub fn id(&self) -> Hash {
        self.inner.id()
    }
    /// Whether this package can be replay-verified.
    pub fn is_replay_verifiable(&self) -> bool {
        self.inner.manifest.is_replay_verifiable()
    }
    /// Check the actual bytes against the verified manifest. **Fails closed.**
    pub fn verify_contents<S: FileSource + ?Sized>(&self, src: &S) -> Result<()> {
        self.inner.manifest.verify_contents(src)
    }
}

impl Package {
    /// The package id: `BLAKE3(ID_DOMAIN ‖ developer ‖ manifest signing bytes)`.
    ///
    /// Deterministic, developer-bound, and independent of the signature bytes
    /// (so it cannot be perturbed by a different-but-valid encoding of the same
    /// Ed25519 signature). It commits to the price, the split, the determinism
    /// class and every file hash.
    ///
    /// **This is a new id, not a replacement for a wasm module's content
    /// address.** See [`PackageManifest::legacy_game_id`].
    pub fn id(&self) -> Hash {
        let mut h = blake3::Hasher::new();
        h.update(ID_DOMAIN);
        h.update(&self.manifest.developer.0);
        h.update(&self.manifest.signing_bytes());
        Hash(*h.finalize().as_bytes())
    }

    /// The kotva-style content address of this package:
    /// `[0x1e] ‖ BLAKE3-256(...)`, i.e. [`Self::id`] behind the multihash
    /// agility prefix.
    ///
    /// Provided so the eventual `kotva_core::id::ContentId` binding is a
    /// re-wrapping rather than a re-hashing. Magnetite's own APIs use the bare
    /// 32-byte [`Hash`], matching every other id in the tree.
    pub fn content_id(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(33);
        v.push(0x1e); // multiformats `blake3` code, as kotva-core uses
        v.extend_from_slice(&self.id().0);
        v
    }

    /// Check everything that can be checked without the file bytes:
    /// structure, entry/kind agreement, the determinism rule, the root over the
    /// file list, and the developer's signature **under `RawKeypairAuth`'s
    /// algorithm specifically**.
    ///
    /// This is the default because [`Self::sign`] was, until this A7 pass,
    /// the only way to produce a `Package` at all, and it only ever accepted
    /// `&RawKeypairAuth` — so every package this crate has ever been able to
    /// sign is one this hard-coded check can correctly verify. Now that
    /// [`Self::sign`] is generic over any [`Identity`], a package signed by a
    /// *different* provider will fail here — not with a confusing error, but
    /// exactly like a bad signature, because the two are indistinguishable
    /// through this method. **Prefer [`Self::verify_for`]** for any package
    /// that might have been signed by a provider other than `RawKeypairAuth`.
    ///
    /// **There is no fail-open path.** Every failure returns `Err`.
    pub fn verify(&self) -> Result<VerifiedPackage<'_>> {
        self.manifest.validate()?;
        if !<RawKeypairAuth as Identity>::verify(
            &self.manifest.developer,
            &self.manifest.signing_bytes(),
            &self.sig,
        ) {
            return Err(PackageError::BadSignature);
        }
        Ok(VerifiedPackage { inner: self })
    }

    /// **A7 finish.** Verify against `provider` — the identity provider
    /// actually used to [`Self::sign`] this package — instead of assuming
    /// `RawKeypairAuth`. The provider-generic counterpart of [`Self::verify`],
    /// mirroring [`crate::identity::Token::is_valid_for`].
    ///
    /// **There is no fail-open path.** Every failure returns `Err`.
    pub fn verify_for(&self, provider: &dyn IdentityVerifier) -> Result<VerifiedPackage<'_>> {
        self.manifest.validate()?;
        if !provider.verify(
            &self.manifest.developer,
            &self.manifest.signing_bytes(),
            &self.sig,
        ) {
            return Err(PackageError::BadSignature);
        }
        Ok(VerifiedPackage { inner: self })
    }

    /// [`Self::verify`] plus a full content check against `src`.
    pub fn verify_with_contents<S: FileSource + ?Sized>(
        &self,
        src: &S,
    ) -> Result<VerifiedPackage<'_>> {
        let v = self.verify()?;
        v.verify_contents(src)?;
        Ok(v)
    }

    /// Canonical CBOR encoding of the signed package — the on-disk / on-wire
    /// form. `decode(encode(p)) == p` and the bytes are byte-identical across
    /// runs and platforms.
    pub fn to_canonical_cbor(&self) -> Vec<u8> {
        cbor::encode(&Cv::Map(vec![
            (KS_MANIFEST, self.manifest.to_cv()),
            (KS_SIG, Cv::Bytes(self.sig.0.to_vec())),
        ]))
    }

    /// Parse a signed package from canonical CBOR. Does **not** verify — call
    /// [`Self::verify`] on the result.
    pub fn from_canonical_cbor(bytes: &[u8]) -> Result<Self> {
        let mut r = MapReader::new(cbor::decode(bytes)?)?;
        let manifest = PackageManifest::from_cv(r.require(KS_MANIFEST)?)?;
        let sig = r.bytes_exact(KS_SIG, 64)?;
        r.finish()?;
        Ok(Self {
            manifest,
            sig: Sig(<[u8; 64]>::try_from(sig.as_slice()).unwrap()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev_key() -> RawKeypairAuth {
        RawKeypairAuth::from_seed([0xD1; 32])
    }

    fn dev_split(pk: PubKey) -> SplitPlan {
        SplitPlan::all_to(pk, Role::Developer)
    }

    /// A four-file synthetic "three.js" bundle.
    fn web_bundle() -> (MemoryFiles, Vec<FileEntry>) {
        let files: Vec<(&str, &[u8])> = vec![
            ("index.html", b"<!doctype html><script src=main.js>"),
            ("main.js", b"import * as THREE from './three.module.js'"),
            ("assets/level.glb", b"glTF\x02\x00\x00\x00fake"),
            ("three.module.js", b"export const REVISION='fake'"),
        ];
        let mut mem = MemoryFiles::new();
        let mut entries = Vec::new();
        for (p, b) in files {
            mem.insert(p, b.to_vec());
            entries.push(FileEntry {
                path: p.to_string(),
                hash: Hash::of(b),
                size: b.len() as u64,
            });
        }
        (mem, entries)
    }

    // -- round trips --------------------------------------------------------

    #[test]
    fn web_package_round_trips_and_verifies() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (mem, entries) = web_bundle();
        let pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::fixed(500, "USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();

        let v = pkg.verify_with_contents(&mem).unwrap();
        assert_eq!(v.manifest().kind, PackageKind::Web);
        assert_eq!(v.manifest().entry(), "index.html");
        assert_eq!(v.manifest().files.len(), 4);

        let bytes = pkg.to_canonical_cbor();
        let back = Package::from_canonical_cbor(&bytes).unwrap();
        assert_eq!(back, pkg, "signed package survives a CBOR round trip");
        assert_eq!(
            back.to_canonical_cbor(),
            bytes,
            "re-encoding is byte-identical"
        );
        assert_eq!(back.id(), pkg.id());
        back.verify_with_contents(&mem).unwrap();
    }

    #[test]
    fn wasm_package_round_trips_and_verifies() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let wasm = b"\x00asm\x01\x00\x00\x00 fake authority module".to_vec();
        let pkg = PackageManifest::wasm_only(
            "game.wasm",
            &wasm,
            PackagePrice::free("USDC"),
            dev_split(pk),
            DeterminismClass::Deterministic,
            pk,
        )
        .sign(&key)
        .unwrap();

        let mem = MemoryFiles::new().with("game.wasm", wasm.clone());
        let v = pkg.verify_with_contents(&mem).unwrap();
        assert_eq!(v.manifest().entry(), "game.wasm");
        assert!(v.is_replay_verifiable());

        let back = Package::from_canonical_cbor(&pkg.to_canonical_cbor()).unwrap();
        assert_eq!(back, pkg);
    }

    #[test]
    fn web_and_wasm_package_round_trips() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let wasm = b"\x00asm authority".to_vec();
        let (mut mem, mut entries) = web_bundle();
        mem.insert("rules.wasm", wasm.clone());
        entries.push(FileEntry {
            path: "rules.wasm".into(),
            hash: Hash::of(&wasm),
            size: wasm.len() as u64,
        });

        let pkg = PackageManifest::web_and_wasm(
            "index.html",
            "rules.wasm",
            entries,
            PackagePrice::pwyw(0, 300, "USDC"),
            dev_split(pk),
            DeterminismClass::Deterministic,
            pk,
        )
        .sign(&key)
        .unwrap();

        let v = pkg.verify_with_contents(&mem).unwrap();
        assert_eq!(v.manifest().kind, PackageKind::WebAndWasm);
        // The browser entry wins; the authority is separately addressable.
        assert_eq!(v.manifest().entry(), "index.html");
        assert_eq!(v.manifest().wasm_entry.as_deref(), Some("rules.wasm"));
        assert!(v.is_replay_verifiable(), "rung 1 keeps the guarantee");
        assert!(v.manifest().legacy_game_id().is_none());
    }

    // -- the determinism boundary -------------------------------------------

    #[test]
    fn web_package_reports_non_replay_verifiable() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (mem, entries) = web_bundle();
        let pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();
        let v = pkg.verify_with_contents(&mem).unwrap();

        assert_eq!(
            v.manifest().determinism,
            DeterminismClass::NonDeterministic,
            "a web bundle is never labelled deterministic"
        );
        assert!(
            !v.is_replay_verifiable(),
            "rung 0 must not inherit rung 2's guarantee"
        );
        assert!(!v.manifest().determinism.is_replay_verifiable());
        assert_eq!(v.manifest().determinism.label(), "non-deterministic");
    }

    #[test]
    fn a_web_package_claiming_determinism_is_refused() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (_, entries) = web_bundle();
        let mut m = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        // Forge the claim the way a hostile publisher would.
        m.determinism = DeterminismClass::Deterministic;

        assert!(matches!(
            m.validate(),
            Err(PackageError::DeterminismClaimWithoutAuthority("web"))
        ));
        // ...and it cannot be signed into existence either.
        assert!(matches!(
            m.clone().sign(&key),
            Err(PackageError::DeterminismClaimWithoutAuthority("web"))
        ));

        // Even hand-encoded and re-signed by the developer, verify refuses it:
        // the rule is checked at verification, not only at construction.
        let sig = key.sign(&m.signing_bytes());
        let forged = Package { manifest: m, sig };
        assert!(matches!(
            forged.verify(),
            Err(PackageError::DeterminismClaimWithoutAuthority("web"))
        ));
        // The forgery survives a CBOR round trip and is still refused.
        let reparsed = Package::from_canonical_cbor(&forged.to_canonical_cbor()).unwrap();
        assert!(reparsed.verify().is_err());
    }

    // -- sorting is load-bearing --------------------------------------------

    #[test]
    fn a_reordered_file_list_produces_the_same_root() {
        let (_, entries) = web_bundle();
        let mut reversed = entries.clone();
        reversed.reverse();
        let mut shuffled = entries.clone();
        shuffled.swap(0, 2);
        shuffled.swap(1, 3);

        let a = PackageManifest::compute_root(&entries);
        let b = PackageManifest::compute_root(&reversed);
        let c = PackageManifest::compute_root(&shuffled);
        assert_eq!(a, b, "root is over the SORTED list");
        assert_eq!(a, c);

        // And a manifest built from a reordered list signs to identical bytes.
        let key = dev_key();
        let pk = key.node_pubkey();
        let p1 = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();
        let p2 = PackageManifest::web(
            "index.html",
            reversed,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();
        assert_eq!(p1.to_canonical_cbor(), p2.to_canonical_cbor());
        assert_eq!(p1.id(), p2.id());
        assert_eq!(p1.sig.0, p2.sig.0);
    }

    #[test]
    fn an_unsorted_file_list_on_the_wire_is_refused() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (_, entries) = web_bundle();
        let mut m = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        m.files.swap(0, 1); // root still matches (root sorts), order does not
        assert!(matches!(
            m.validate(),
            Err(PackageError::FileListNotSorted(_))
        ));
    }

    #[test]
    fn duplicate_paths_are_refused() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (_, mut entries) = web_bundle();
        entries.push(entries[0].clone());
        let m = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        assert!(matches!(
            m.validate(),
            Err(PackageError::FileListNotSorted(_))
        ));
        assert!(m.sign(&key).is_err());
    }

    // -- tampering ----------------------------------------------------------

    #[test]
    fn a_tampered_file_fails() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (mut mem, entries) = web_bundle();
        let pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();
        pkg.verify_with_contents(&mem).unwrap();

        // Swap in a same-length payload so only the hash can catch it.
        let original = mem.0.get("main.js").unwrap().clone();
        let mut evil = original.clone();
        *evil.last_mut().unwrap() ^= 0x01;
        assert_eq!(evil.len(), original.len());
        mem.insert("main.js", evil);

        match pkg.verify_with_contents(&mem) {
            Err(PackageError::FileHash { path, .. }) => assert_eq!(path, "main.js"),
            other => panic!("a tampered file must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_truncated_file_fails() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (mut mem, entries) = web_bundle();
        let pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();

        let mut short = mem.0.get("assets/level.glb").unwrap().clone();
        short.truncate(4);
        mem.insert("assets/level.glb", short);

        match pkg.verify_with_contents(&mem) {
            Err(PackageError::FileSize { path, want, got }) => {
                assert_eq!(path, "assets/level.glb");
                assert!(want > got);
            }
            other => panic!("truncation must be refused, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_truncation_to_zero_bytes_fails() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (mut mem, entries) = web_bundle();
        let pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();
        mem.insert("main.js", Vec::<u8>::new());
        assert!(matches!(
            pkg.verify_with_contents(&mem),
            Err(PackageError::FileSize { .. })
        ));
    }

    #[test]
    fn a_missing_file_fails() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (mut mem, entries) = web_bundle();
        let pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();
        mem.0.remove("three.module.js");
        match pkg.verify_with_contents(&mem) {
            Err(PackageError::FileMissing(p)) => assert_eq!(p, "three.module.js"),
            other => panic!("a missing file must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_bad_signature_fails() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (mem, entries) = web_bundle();
        let mut pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&key)
        .unwrap();
        pkg.verify().unwrap();

        // 1. Flip a signature bit.
        let good = pkg.sig;
        pkg.sig.0[10] ^= 0x40;
        assert!(matches!(pkg.verify(), Err(PackageError::BadSignature)));
        assert!(pkg.verify_with_contents(&mem).is_err());
        pkg.sig = good;

        // 2. Signed by the wrong key entirely.
        let attacker = RawKeypairAuth::from_seed([0xEE; 32]);
        let mut wrong = pkg.clone();
        wrong.sig = attacker.sign(&wrong.manifest.signing_bytes());
        assert!(matches!(wrong.verify(), Err(PackageError::BadSignature)));

        // 3. Attacker re-signs and swaps in their own key: the developer key is
        //    inside the signed bytes, so this is a different package, not a
        //    hijack of this one.
        let mut hijack = pkg.manifest.clone();
        hijack.developer = attacker.node_pubkey();
        let hijack = hijack.sign(&attacker).unwrap();
        assert_ne!(hijack.id(), pkg.id());
        assert_ne!(hijack.manifest.developer.0, pk.0);

        // 4. A manifest field changed after signing.
        let mut mutated = pkg.clone();
        mutated.manifest.price = PackagePrice::fixed(1, "USDC");
        assert!(matches!(mutated.verify(), Err(PackageError::BadSignature)));
    }

    #[test]
    fn signing_under_a_key_the_manifest_does_not_name_is_refused() {
        let key = dev_key();
        let other = RawKeypairAuth::from_seed([0x77; 32]);
        let (_, entries) = web_bundle();
        let m = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(key.node_pubkey()),
            key.node_pubkey(),
        );
        assert!(matches!(m.sign(&other), Err(PackageError::BadSignature)));
    }

    #[test]
    fn a_forged_root_fails() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (_, entries) = web_bundle();
        let mut m = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        m.root = Hash::of(b"not the root");
        assert!(matches!(
            m.validate(),
            Err(PackageError::RootMismatch { .. })
        ));

        // Adding a file without recomputing the root is the same failure.
        let mut m2 = PackageManifest::web(
            "index.html",
            web_bundle().1,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        m2.files.push(FileEntry {
            path: "zz-injected.js".into(),
            hash: Hash::of(b"evil"),
            size: 4,
        });
        assert!(matches!(
            m2.validate(),
            Err(PackageError::RootMismatch { .. })
        ));
    }

    // -- entries / paths ----------------------------------------------------

    #[test]
    fn an_entry_not_in_the_file_list_is_refused() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (_, entries) = web_bundle();
        let m = PackageManifest::web(
            "missing.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        assert!(matches!(m.validate(), Err(PackageError::EntryNotListed(_))));
        assert!(m.sign(&key).is_err());
    }

    #[test]
    fn kind_and_entries_must_agree() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (_, entries) = web_bundle();
        let mut m = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        m.kind = PackageKind::WebAndWasm; // claims an authority it has not got
        assert!(matches!(m.validate(), Err(PackageError::Entry(_))));
        let _ = key;
    }

    #[test]
    fn traversal_and_absolute_paths_are_refused() {
        for bad in [
            "../etc/passwd",
            "/etc/passwd",
            "a/../../b",
            "./a",
            "a//b",
            "a/",
            "",
            "c:/windows",
            "back\\slash",
            "nul\0byte",
        ] {
            assert!(
                validate_path(bad).is_err(),
                "{bad:?} must be refused as a bundle path"
            );
        }
        for ok in ["index.html", "assets/level.glb", "a/b/c/d.wasm.br", "x..y"] {
            validate_path(ok).unwrap_or_else(|e| panic!("{ok:?} should be valid: {e}"));
        }
    }

    #[test]
    fn a_manifest_with_a_traversal_path_is_refused_even_when_signed() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let mut m = PackageManifest::web(
            "index.html",
            web_bundle().1,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        m.files.push(FileEntry {
            path: "../../escape.js".into(),
            hash: Hash::of(b"x"),
            size: 1,
        });
        m.canonicalize(); // root now matches, so only path validation catches it
        let sig = key.sign(&m.signing_bytes());
        let pkg = Package { manifest: m, sig };
        assert!(matches!(
            pkg.verify(),
            Err(PackageError::InvalidPath { .. })
        ));
    }

    #[test]
    fn dir_files_refuses_to_read_a_traversal_path() {
        let src = DirFiles::new("/nonexistent-bundle-root");
        assert!(src.read("../../../etc/passwd").is_none());
    }

    #[test]
    fn manifest_file_lookup_only_returns_listed_paths() {
        let (_, entries) = web_bundle();
        let pk = dev_key().node_pubkey();
        let m = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::free("USDC"),
            dev_split(pk),
            pk,
        );
        assert!(m.file("index.html").is_some());
        assert!(m.file("assets/level.glb").is_some());
        assert!(m.file("secret.env").is_none());
        assert!(m.file("index.htm").is_none());
    }

    // -- price --------------------------------------------------------------

    #[test]
    fn price_models_round_trip_including_pwyw_with_a_minimum() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let cases = vec![
            PackagePrice::free("USDC"),
            PackagePrice::fixed(1, "USDC"),
            PackagePrice::fixed(u64::MAX, "USDC"),
            PackagePrice::pwyw(0, 0, "USDC"),
            PackagePrice::pwyw(0, 500, "USDC"),
            PackagePrice::pwyw(250, 900, "SOL"),
            PackagePrice::pwyw(250, 250, "USDC"),
        ];
        for price in cases {
            let wasm = b"\x00asm module".to_vec();
            let pkg = PackageManifest::wasm_only(
                "game.wasm",
                &wasm,
                price.clone(),
                dev_split(pk),
                DeterminismClass::Deterministic,
                pk,
            )
            .sign(&key)
            .unwrap();
            let back = Package::from_canonical_cbor(&pkg.to_canonical_cbor()).unwrap();
            back.verify().unwrap();
            assert_eq!(back.manifest.price, price, "price survives the round trip");
        }

        // The semantics, not just the bytes.
        let p = PackagePrice::pwyw(250, 900, "USDC");
        assert_eq!(p.min_units(), 250);
        assert_eq!(p.suggested_units(), 900);
        assert!(!p.admits(249), "below the pwyw minimum must not admit");
        assert!(p.admits(250));
        assert!(p.admits(10_000), "paying more is fine");

        let f = PackagePrice::free("USDC");
        assert_eq!(f.min_units(), 0);
        assert!(f.admits(0));

        let x = PackagePrice::fixed(500, "USDC");
        assert!(!x.admits(499));
        assert!(x.admits(500));
    }

    #[test]
    fn degenerate_prices_are_refused() {
        assert!(
            PackagePrice::fixed(0, "USDC").validate().is_err(),
            "fixed 0"
        );
        assert!(
            PackagePrice::pwyw(500, 100, "USDC").validate().is_err(),
            "suggested below min"
        );
        assert!(PackagePrice::free("").validate().is_err(), "empty currency");
        assert!(
            PackagePrice::free("US DC").validate().is_err(),
            "non-alphanumeric currency"
        );
        assert!(
            PackagePrice::free("A".repeat(MAX_CURRENCY_LEN + 1))
                .validate()
                .is_err(),
            "over-long currency"
        );
    }

    // -- split --------------------------------------------------------------

    #[test]
    fn split_legs_round_trip_and_resolve_sum_exact() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let split = SplitPlan {
            legs: vec![
                SplitLeg {
                    wallet: pk,
                    share_bps: 7_000,
                    role: Role::Developer,
                },
                SplitLeg {
                    wallet: PubKey([0x0B; 32]),
                    share_bps: 2_000,
                    role: Role::Operator,
                },
                SplitLeg {
                    wallet: PubKey([0x5E; 32]),
                    share_bps: 500,
                    role: Role::Stewards,
                },
                SplitLeg {
                    wallet: PubKey([0xA5; 32]),
                    share_bps: 500,
                    role: Role::Other("asset-pack".into()),
                },
            ],
        };
        let wasm = b"\x00asm".to_vec();
        let pkg = PackageManifest::wasm_only(
            "game.wasm",
            &wasm,
            PackagePrice::fixed(1_000, "USDC"),
            split.clone(),
            DeterminismClass::Deterministic,
            pk,
        )
        .sign(&key)
        .unwrap();
        let back = Package::from_canonical_cbor(&pkg.to_canonical_cbor()).unwrap();
        back.verify().unwrap();
        assert_eq!(back.manifest.split, split);
        assert_eq!(back.manifest.split.legs[3].role.tag(), "asset-pack");

        let legs = split.resolve(1_000);
        assert_eq!(
            legs.iter().map(|l| l.amount).collect::<Vec<_>>(),
            vec![700, 200, 50, 50]
        );

        // Sum-exact for every total, including ones that do not divide evenly.
        for total in [0u64, 1, 2, 3, 7, 99, 101, 9_999, 1_000_001, u32::MAX as u64] {
            let sum: u64 = split.resolve(total).iter().map(|l| l.amount).sum();
            assert_eq!(sum, total, "resolve({total}) must sum to exactly {total}");
        }
        // Deterministic.
        assert_eq!(split.resolve(7), split.resolve(7));
        // And the leftover unit goes by largest remainder, then declared order.
        assert_eq!(
            split
                .resolve(1)
                .iter()
                .map(|l| l.amount)
                .collect::<Vec<_>>(),
            vec![1, 0, 0, 0]
        );
    }

    #[test]
    fn malformed_splits_are_refused() {
        let pk = dev_key().node_pubkey();
        let cases: Vec<(&str, SplitPlan)> = vec![
            ("empty", SplitPlan { legs: vec![] }),
            (
                "does not sum to 10000",
                SplitPlan {
                    legs: vec![SplitLeg {
                        wallet: pk,
                        share_bps: 9_999,
                        role: Role::Developer,
                    }],
                },
            ),
            (
                "sums over 10000",
                SplitPlan {
                    legs: vec![
                        SplitLeg {
                            wallet: pk,
                            share_bps: 9_000,
                            role: Role::Developer,
                        },
                        SplitLeg {
                            wallet: PubKey([2; 32]),
                            share_bps: 2_000,
                            role: Role::Operator,
                        },
                    ],
                },
            ),
            (
                "zero-share leg",
                SplitPlan {
                    legs: vec![
                        SplitLeg {
                            wallet: pk,
                            share_bps: 10_000,
                            role: Role::Developer,
                        },
                        SplitLeg {
                            wallet: PubKey([2; 32]),
                            share_bps: 0,
                            role: Role::Stewards,
                        },
                    ],
                },
            ),
            (
                "duplicate wallet",
                SplitPlan {
                    legs: vec![
                        SplitLeg {
                            wallet: pk,
                            share_bps: 5_000,
                            role: Role::Developer,
                        },
                        SplitLeg {
                            wallet: pk,
                            share_bps: 5_000,
                            role: Role::Stewards,
                        },
                    ],
                },
            ),
            (
                "no developer leg",
                SplitPlan {
                    legs: vec![SplitLeg {
                        wallet: pk,
                        share_bps: 10_000,
                        role: Role::Operator,
                    }],
                },
            ),
            (
                "two developer legs",
                SplitPlan {
                    legs: vec![
                        SplitLeg {
                            wallet: pk,
                            share_bps: 5_000,
                            role: Role::Developer,
                        },
                        SplitLeg {
                            wallet: PubKey([2; 32]),
                            share_bps: 5_000,
                            role: Role::Developer,
                        },
                    ],
                },
            ),
            (
                "empty Other label",
                SplitPlan {
                    legs: vec![
                        SplitLeg {
                            wallet: pk,
                            share_bps: 9_000,
                            role: Role::Developer,
                        },
                        SplitLeg {
                            wallet: PubKey([2; 32]),
                            share_bps: 1_000,
                            role: Role::Other(String::new()),
                        },
                    ],
                },
            ),
        ];
        for (why, split) in cases {
            assert!(split.validate().is_err(), "must be refused: {why}");
        }
    }

    // -- format / encoding --------------------------------------------------

    #[test]
    fn an_unknown_format_version_is_refused() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let wasm = b"\x00asm".to_vec();
        let mut m = PackageManifest::wasm_only(
            "game.wasm",
            &wasm,
            PackagePrice::free("USDC"),
            dev_split(pk),
            DeterminismClass::Deterministic,
            pk,
        );
        m.format = 99;
        assert!(matches!(
            m.validate(),
            Err(PackageError::UnsupportedFormat(99))
        ));
        let sig = key.sign(&m.signing_bytes());
        let pkg = Package { manifest: m, sig };
        assert!(matches!(
            pkg.verify(),
            Err(PackageError::UnsupportedFormat(99))
        ));
        // ...and it is refused at parse time too, before any field is read.
        assert!(matches!(
            Package::from_canonical_cbor(&pkg.to_canonical_cbor()),
            Err(PackageError::UnsupportedFormat(99))
        ));
    }

    #[test]
    fn non_canonical_and_corrupt_bytes_are_refused() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let wasm = b"\x00asm".to_vec();
        let pkg = PackageManifest::wasm_only(
            "game.wasm",
            &wasm,
            PackagePrice::free("USDC"),
            dev_split(pk),
            DeterminismClass::Deterministic,
            pk,
        )
        .sign(&key)
        .unwrap();
        let good = pkg.to_canonical_cbor();

        assert!(Package::from_canonical_cbor(&[]).is_err());
        assert!(Package::from_canonical_cbor(&good[..good.len() - 1]).is_err());
        let mut trailing = good.clone();
        trailing.push(0x00);
        assert!(
            Package::from_canonical_cbor(&trailing).is_err(),
            "trailing bytes must be refused"
        );

        // An unknown top-level key in a signed envelope.
        let mut extra = cbor::decode(&good).unwrap();
        if let Cv::Map(m) = &mut extra {
            m.push((99, Cv::U64(1)));
        }
        assert!(matches!(
            Package::from_canonical_cbor(&cbor::encode(&extra)),
            Err(PackageError::Cbor(CborError::UnknownKey(99)))
        ));
    }

    #[test]
    fn encoding_is_stable_across_repeated_builds() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let (_, entries) = web_bundle();
        let build = || {
            PackageManifest::web(
                "index.html",
                entries.clone(),
                PackagePrice::pwyw(100, 400, "USDC"),
                dev_split(pk),
                pk,
            )
            .sign(&key)
            .unwrap()
        };
        let a = build();
        let b = build();
        assert_eq!(a.to_canonical_cbor(), b.to_canonical_cbor());
        assert_eq!(a.id(), b.id());
        assert_eq!(a.content_id()[0], 0x1e, "kotva multihash prefix");
        assert_eq!(a.content_id().len(), 33);
        assert_eq!(&a.content_id()[1..], &a.id().0[..]);
    }

    // -- backwards compatibility -------------------------------------------

    #[test]
    fn an_existing_wasm_only_game_id_is_unchanged_by_packaging() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let wasm = b"\x00asm\x01\x00\x00\x00 an already-published module".to_vec();

        // The pre-package id, exactly as `node::content_address` computes it.
        let legacy = Hash::of(&wasm);

        let pkg = PackageManifest::wasm_only(
            "game.wasm",
            &wasm,
            PackagePrice::free("USDC"),
            dev_split(pk),
            DeterminismClass::Deterministic,
            pk,
        )
        .sign(&key)
        .unwrap();
        let v = pkg.verify().unwrap();

        assert_eq!(
            v.manifest().legacy_game_id(),
            Some(legacy),
            "packaging must not change an existing wasm game's id"
        );
        assert_eq!(v.manifest().files[0].hash, legacy);
        assert_ne!(
            v.id(),
            legacy,
            "the package id is a NEW, additional id — not the module's address"
        );
        // The blob still resolves by its own content address.
        assert_eq!(Hash::of(&wasm), legacy);
    }

    // -- directory builder --------------------------------------------------

    #[test]
    fn from_dir_builds_a_deterministic_package_and_verifies_against_the_dir() {
        let key = dev_key();
        let pk = key.node_pubkey();
        let dir = std::env::temp_dir().join(format!(
            "magnetite-pkg-test-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets/sub")).unwrap();
        std::fs::write(dir.join("index.html"), b"<!doctype html>").unwrap();
        std::fs::write(dir.join("game.wasm"), b"\x00asm\x01\x00\x00\x00").unwrap();
        std::fs::write(dir.join("assets/a.png"), b"\x89PNG fake").unwrap();
        std::fs::write(dir.join("assets/sub/b.bin"), vec![0xAB; 4096]).unwrap();

        let terms = || PackageTerms {
            price: PackagePrice::pwyw(0, 250, "USDC"),
            split: dev_split(pk),
            determinism: DeterminismClass::Deterministic,
            developer: pk,
        };
        let m = PackageManifest::from_dir(
            &dir,
            PackageKind::WebAndWasm,
            Some("index.html".into()),
            Some("game.wasm".into()),
            terms(),
        )
        .unwrap();
        let pkg = m.clone().sign(&key).unwrap();

        assert_eq!(pkg.manifest.files.len(), 4);
        assert_eq!(
            pkg.manifest
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "assets/a.png",
                "assets/sub/b.bin",
                "game.wasm",
                "index.html"
            ],
            "paths are '/'-separated and sorted"
        );
        assert_eq!(pkg.manifest.total_size(), 15 + 8 + 9 + 4096);

        pkg.verify_with_contents(&DirFiles::new(&dir)).unwrap();

        // Rebuilding the same directory gives byte-identical output.
        let again = PackageManifest::from_dir(
            &dir,
            PackageKind::WebAndWasm,
            Some("index.html".into()),
            Some("game.wasm".into()),
            terms(),
        )
        .unwrap()
        .sign(&key)
        .unwrap();
        assert_eq!(again.to_canonical_cbor(), pkg.to_canonical_cbor());

        // Mutate one byte on disk: verification must refuse.
        std::fs::write(dir.join("assets/a.png"), b"\x89PNG FAKE").unwrap();
        assert!(matches!(
            pkg.verify_with_contents(&DirFiles::new(&dir)),
            Err(PackageError::FileHash { .. })
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn from_dir_refuses_a_symlink() {
        let dir = std::env::temp_dir().join(format!(
            "magnetite-pkg-symlink-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), b"hi").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", dir.join("leak")).unwrap();
            let pk = dev_key().node_pubkey();
            let r = PackageManifest::from_dir(
                &dir,
                PackageKind::Web,
                Some("index.html".into()),
                None,
                PackageTerms {
                    price: PackagePrice::free("USDC"),
                    split: dev_split(pk),
                    determinism: DeterminismClass::NonDeterministic,
                    developer: pk,
                },
            );
            assert!(matches!(r, Err(PackageError::Io(_))), "got {r:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- integration with the payment gate ---------------------------------

    #[test]
    fn package_price_feeds_the_existing_receipt_gate() {
        use crate::payment::{receipt_admits, MockPaymentRail, PaymentRail, PaymentSplit};

        let key = dev_key();
        let pk = key.node_pubkey();
        let wasm = b"\x00asm".to_vec();
        let pkg = PackageManifest::wasm_only(
            "game.wasm",
            &wasm,
            PackagePrice::pwyw(250, 900, "USDC"),
            dev_split(pk),
            DeterminismClass::Deterministic,
            pk,
        )
        .sign(&key)
        .unwrap();
        let price = &pkg.verify().unwrap().manifest().price;

        let rail = MockPaymentRail::new();
        let buyer = PubKey([0xB7; 32]);
        let pay = |amount: u64| {
            futures_lite_block_on(rail.checkout(&buyer, PaymentSplit::to_developer(pk, amount)))
        };

        let under = pay(249);
        assert!(
            !receipt_admits(&rail, &under, &buyer, price.min_units()),
            "underpaying a pwyw minimum must not admit"
        );
        let ok = pay(250);
        assert!(receipt_admits(&rail, &ok, &buyer, price.min_units()));
    }

    /// Minimal executor so this module's tests need no tokio dev-dependency
    /// wiring beyond what the crate already has.
    fn futures_lite_block_on<F: std::future::Future>(f: F) -> F::Output {
        // The mock rail's futures are ready immediately (no IO, no timers), so
        // a single poll with a no-op waker is sufficient and honest.
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn clone(p: *const ()) -> RawWaker {
            RawWaker::new(p, &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = Box::pin(f);
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("the mock rail must not yield"),
        }
    }

    // -- A7 finish: Package::sign / Package::verify_for are provider-generic --

    /// A second `Identity` provider, deliberately byte-incompatible with
    /// `RawKeypairAuth` (same trick as `magnetite_seams::identity`'s own A7
    /// test double): it signs `DOMAIN ‖ msg`, so its signature bytes are NEVER
    /// equal to `RawKeypairAuth`'s over the same key and message. Exists to
    /// prove `Package::sign`/`verify_for` genuinely dispatch per-provider,
    /// rather than merely happening to work because `RawKeypairAuth` was the
    /// only thing ever tried.
    struct DomainTaggedDeveloper(ed25519_dalek::SigningKey);

    const PKG_DOMAIN_TAG: &[u8] = b"magnetite-seams/package-domain-tagged-test/v1";

    impl DomainTaggedDeveloper {
        fn from_seed(seed: [u8; 32]) -> Self {
            DomainTaggedDeveloper(ed25519_dalek::SigningKey::from_bytes(&seed))
        }
        fn tagged(msg: &[u8]) -> Vec<u8> {
            let mut v = PKG_DOMAIN_TAG.to_vec();
            v.extend_from_slice(msg);
            v
        }
    }

    impl Identity for DomainTaggedDeveloper {
        fn pubkey(&self) -> PubKey {
            PubKey(self.0.verifying_key().to_bytes())
        }
        fn sign(&self, msg: &[u8]) -> Sig {
            use ed25519_dalek::Signer;
            Sig(self.0.sign(&Self::tagged(msg)).to_bytes())
        }
        fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
            use ed25519_dalek::Verifier;
            let vk = match ed25519_dalek::VerifyingKey::from_bytes(&pk.0) {
                Ok(vk) => vk,
                Err(_) => return false,
            };
            vk.verify(&Self::tagged(msg), &ed25519_dalek::Signature::from_bytes(&sig.0))
                .is_ok()
        }
    }

    #[test]
    fn sign_is_provider_generic_and_verify_for_follows_the_actual_signer() {
        // Before this A7 finish, `PackageManifest::sign` only ever accepted
        // `&RawKeypairAuth`, so `Package::verify`'s hard-coded check was
        // harmless in practice — nothing else could have signed a package.
        // Now that `sign` is generic over any `Identity`, a package CAN be
        // signed by a provider `verify()` cannot recognise, which is exactly
        // why `verify_for` exists.
        let dev = DomainTaggedDeveloper::from_seed([0xABu8; 32]);
        let pk = dev.pubkey();
        let (_mem, entries) = web_bundle();
        let pkg = PackageManifest::web(
            "index.html",
            entries,
            PackagePrice::fixed(500, "USDC"),
            dev_split(pk),
            pk,
        )
        .sign(&dev)
        .expect("signing under the manifest's own declared developer key must succeed");

        // 1. The legacy, RawKeypairAuth-hard-coded `verify()` cannot tell a
        //    domain-tagged signature apart from a bad one — it was never able
        //    to check any algorithm but RawKeypairAuth's.
        assert!(
            matches!(pkg.verify(), Err(PackageError::BadSignature)),
            "legacy hard-coded verify(): an honestly-signed package from a \
             non-default provider is wrongly rejected, silently rather than erring"
        );

        // 2. `verify_for`, given the ACTUAL signing provider, accepts it.
        assert!(
            pkg.verify_for(&dev).is_ok(),
            "verify_for must accept the package under its ACTUAL signing provider"
        );

        // 3. A structurally different algorithm over the SAME key must not
        //    verify a domain-tagged signature — proves this isn't a rubber stamp.
        let raw_same_key = RawKeypairAuth::from_seed([0xABu8; 32]);
        assert!(
            matches!(
                pkg.verify_for(&raw_same_key),
                Err(PackageError::BadSignature)
            ),
            "a different algorithm over the same key must not verify a \
             domain-tagged signature"
        );
    }
}
