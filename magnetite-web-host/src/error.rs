//! Errors raised while building or validating a bundle.
//!
//! Note what is *not* here: nothing about serving. A refused or failed request
//! is a [`crate::respond::Response`] with a status code, not an `Err` — the
//! caller must always have something to hand the browser, and an error type
//! that could be `?`-propagated past the response builder is how a 500 becomes
//! a hang.

/// Result alias for manifest construction and validation.
pub type Result<T> = std::result::Result<T, Error>;

/// Why a manifest could not be accepted.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// A path could not be served safely. `why` is the specific rule.
    #[error("unsafe bundle path {path:?}: {why}")]
    BadPath {
        /// The offending path, as supplied.
        path: String,
        /// Which rule it broke.
        why: String,
    },
    /// A stored path is not in normalized form (only possible on a manifest
    /// that bypassed the constructor).
    #[error("bundle path {0:?} is not normalized")]
    UnnormalizedPath(String),
    /// Two different files claim one path. Unresolvable, so refused.
    #[error("duplicate bundle path {0:?} with differing content")]
    DuplicatePath(String),
    /// `files` is not sorted by path, which breaks both the binary-search
    /// lookup and the root hash's canonical order.
    #[error("bundle files are not sorted by path")]
    UnsortedFiles,
    /// `entry` names a file the manifest does not contain.
    #[error("entry {0:?} is not in the bundle")]
    EntryMissing(String),
    /// A bundle with no files has nothing to serve.
    #[error("bundle contains no files")]
    EmptyBundle,
}
