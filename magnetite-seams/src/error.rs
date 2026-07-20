//! Shared error type for every seam.

/// Errors surfaced by seam operations.
///
/// Kept intentionally small: the defaults never touch the network or an
/// external service, so most variants describe crypto / lookup failures.
#[derive(Debug, thiserror::Error)]
pub enum SeamError {
    /// A login challenge is past its expiry.
    #[error("login challenge expired")]
    ChallengeExpired,

    /// The presented challenge was not signed by this authority (forged/foreign).
    #[error("challenge was not issued by this authority")]
    UntrustedChallenge,

    /// The challenge nonce was already spent (replay attempt).
    #[error("login challenge already used")]
    ChallengeReplayed,

    /// A signature failed to verify against the given key and message.
    #[error("invalid signature")]
    InvalidSignature,

    /// A malformed key/signature/hash byte string.
    #[error("malformed key material: {0}")]
    MalformedKey(String),

    /// Content-addressed blob not present in the store.
    #[error("blob not found")]
    BlobNotFound,

    /// A transport error from a pluggable client (tracker / http fetch).
    #[error("transport error: {0}")]
    Transport(String),

    /// An attested (sensor-derived) input event failed plausibility screening.
    ///
    /// This means "not physically reachable", **never** "cheating proven" — see
    /// [`crate::input::Implausible`]. Only the [`crate::InputClass::Attested`]
    /// path produces it; deterministic input is checked by replay instead.
    #[error("input event is not plausible: {0}")]
    Implausible(String),

    /// Generic invalid-input guard.
    #[error("invalid input: {0}")]
    Invalid(String),
}

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, SeamError>;
