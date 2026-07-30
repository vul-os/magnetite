//! Seams §3.1 / §3.2 bound to the **kotva substrate**.
//!
//! # Status: a working binding, narrow in scope
//!
//! A standalone crate (not a `magnetite-seams` feature — see this crate's
//! `Cargo.toml` for the measurement that forced that) binding two seams —
//! [`Identity`]/[`AuthProvider`] and [`Naming`] — to [`kotva_core`] v0.2.0 (tag
//! `core-v0.2.0`), per magnetite's `ALIGNMENT.md` §3.
//!
//! It is not a migration.
//! [`RawKeypairAuth`](magnetite_seams::RawKeypairAuth) and
//! [`HashNaming`](magnetite_seams::HashNaming) are untouched and remain
//! `magnetite-seams`'s zero-dependency offline defaults; nothing in
//! `magnetite-seams` or `backend`'s default build depends on this crate
//! existing. Nothing in the tree selects these types yet either — they exist so
//! the seams are demonstrably bindable and so the byte-level compatibility
//! questions get answered in code rather than in prose.
//!
//! What is **not** here, and is not claimed: kotva's `Identity` object
//! (multi-suite key set), `DeviceCert`, `MoveRecord`, `CapabilityToken`,
//! `pubobj`/`pubsub`, and the canonical CBOR codec. Those still have no seam
//! to bind to today — see "What does not fit" below. `RecoveryPolicy` and
//! `KeyRotation` **do** now have somewhere to land, in reduced form:
//! `magnetite_seams::rotation` (A8, landed after this crate's own docs named
//! the gap — see that module and "What does not fit" below for exactly what
//! carried over and what deliberately did not).
//!
//! # Seam §3.1 — [`KotvaIdentity`]
//!
//! Wraps [`kotva_core::identity::IdentityKey`] (Ed25519, kotva suite `0x01`).
//! Because magnetite and kotva both use Ed25519 for identity, the *key material*
//! is interchangeable: a 32-byte seed loads into either type and yields the same
//! public key. Two byte-level details had to be decided:
//!
//! **1. Domain separation.** kotva signs via
//! [`IdentityKey::sign_domain(domain, msg)`](kotva_core::identity::IdentityKey::sign_domain),
//! which signs `domain ‖ msg`. Magnetite's [`Identity::sign`] takes no domain
//! parameter, so [`KotvaIdentity`]'s [`Identity`] impl uses an **empty
//! domain**, making its signatures byte-identical to
//! [`RawKeypairAuth`](magnetite_seams::RawKeypairAuth)'s raw Ed25519. That is
//! what keeps the existing [`Challenge`]/[`Token`] types working unchanged
//! across both providers, and it is verified by test — including under
//! [`Token::is_valid_for`], which dispatches through whichever provider is
//! actually passed to it rather than assuming one.
//!
//! Callers who want kotva's domain separation use
//! [`KotvaIdentity::sign_domain`] / [`verify_domain`] directly. Those signatures
//! deliberately do **not** verify under `<KotvaIdentity as Identity>::verify`,
//! which is the entire point of a domain tag.
//!
//! **2. Strict verification.** kotva's
//! [`verify_domain`](kotva_core::identity::verify_domain) uses Ed25519
//! `verify_strict` (RFC 8032 §5.1.7 cofactorless verification, rejecting
//! non-canonical and small-order `A`); magnetite's
//! [`RawKeypairAuth::verify`](magnetite_seams::RawKeypairAuth) uses plain
//! `verify`. So `<KotvaIdentity as Identity>::verify` accepts a **strict subset**
//! of what `<RawKeypairAuth as Identity>::verify` accepts. For every
//! honestly-generated keypair the two agree, and the round-trip tests cover
//! that; the divergence is confined to malleable/degenerate keys, where kotva is
//! the stricter and better-behaved of the two. This crate does not construct
//! such a key to prove the divergence — that is untested here and stated as a
//! property of the upstream code, not as something measured.
//!
//! # Seam §3.2 — [`KotvaNaming`]
//!
//! Wraps [`kotva_core::keyname`]: 8 data words (10 bits each, from a 1024-word
//! list) + 1 checksum word, hyphen-joined, derived from `BLAKE3` of the pubkey.
//! Unlike `magnetite_seams::keyname::KeyNameNaming` — which is magnetite's own
//! 2048-word/11-bit encoding and explicitly claims compatibility with nothing —
//! this provider produces the name a kotva node produces for the same key. That
//! interoperability is the reason it exists.
//!
//! **Name derivation is pinned to the tag, and the tag is not the last word.**
//! At `core-v0.2.0` the preimage is `BLAKE3(pubkey)`. kotva's `main` has since
//! changed it to `BLAKE3(0x01 ‖ 0x1e ‖ pubkey)` (its §18.9.17 derivation-version
//! binding), which yields *different names for the same key*. Nothing here is
//! wrong, but no name produced by this crate should be treated as a stable
//! long-lived identifier until kotva cuts a tag with that change in it. The
//! tests therefore assert *properties* (determinism, checksum fail-closed, key
//! binding) and deliberately contain **no golden name vectors**.
//!
//! 80 bits of name cannot invert a 256-bit key, so [`KotvaNaming::resolve`]
//! resolves a word-name only for keys the node has [`learn`](KotvaNaming::learn)ed
//! — the same hint-cache, never-an-authority shape as
//! [`HashNaming`](magnetite_seams::HashNaming)'s alias table.
//!
//! # Content addressing — the cleanest part of the overlap
//!
//! magnetite's [`Hash`](struct@Hash) is `BLAKE3-256(bytes)`. kotva's
//! [`ContentId`] is `[0x1e] ‖ BLAKE3-256(bytes)`, where `0x1e` is the
//! multiformats code for `blake3`. They are the *same digest*, so conversion is a
//! prefix byte and needs no rehash: [`content_id_of_hash`] and
//! [`hash_from_content_id`]. This is exactly what `ALIGNMENT.md` §3 claims, and
//! it is asserted directly rather than assumed — see the test
//! `kotva_content_id_is_magnetites_blake3_plus_an_agility_prefix`.
//!
//! # What does not fit magnetite's seams
//!
//! Recorded because it is the useful output of trying. These are **flagged, not
//! fixed** — changing the seam traits is separate work, tracked in
//! `ALIGNMENT.md`, and this crate deliberately does not touch them:
//!
//! * **`Identity::verify` was an associated (static) function — FIXED (A7).**
//!   `magnetite_seams::identity::IdentityVerifier` is now a method, blanket-
//!   implemented for every `Identity` (this crate's `KotvaIdentity` included,
//!   automatically — nothing here changed to gain it). `Token::is_valid_for`
//!   verifies against whichever provider is actually passed in, so it is not
//!   hard-coded to `RawKeypairAuth`. The old hard-coded `Token::is_valid_at`
//!   has since been deleted outright: auditing every caller across all 16
//!   magnetite crates (the second half of A7) found zero in production code
//!   — only tests, all migrated to `is_valid_for` — so there was no
//!   compatibility reason left to keep it, and keeping it would have left a
//!   silent-mis-verification trap callable for no benefit.
//!   `magnetite-runtime`'s `fleet`/`cluster`/`follow` protocol and
//!   `magnetite-solana-rail`'s handshake path still call `Identity::verify`
//!   directly by concrete type via UFCS (the reason `verify` itself stays an
//!   associated function rather than becoming a method) — those were
//!   individually audited and found to be a **closed** system (the signing
//!   side is equally hard-typed to `RawKeypairAuth`), not a live A7-class bug.
//! * **No seam expressed key lifecycle — PARTIALLY LANDED (A8).**
//!   `magnetite_seams::rotation` now carries a minimum key-rotation chain
//!   (`RotationRecord` + `verify_chain`), adopted from kotva's own
//!   `KeyRotation` shape (continuity signature by the retiring key,
//!   hash-chained) and evermesh's decided fork-resolution rule (finalized
//!   signing beats everything; short of finality, recovery beats a
//!   provisional signing rotation; same-class forks resolve by lowest record
//!   id, merge-order independent). A node or player that rotates its key
//!   through that seam keeps the SAME `identity_id` (the genesis record's
//!   content hash) — it no longer becomes a different identity. What did
//!   NOT land: kotva's multi-guardian `RecoveryPolicy` quorum
//!   (`GuardianApproval`, `rotate_threshold`) is reduced to evermesh's
//!   simpler "any one declared recovery key suffices" shape, with no quorum
//!   counting; `DeviceCert` (a delegation fact layered on top of a resolved
//!   chain head, not about whether the root identity persists) and
//!   `MoveRecord` (a name rebinding — the `Naming` seam's concern, not
//!   `Identity`'s) still have nowhere to land and are not claimed. This
//!   crate does not yet bridge `KotvaIdentity` to `magnetite_seams::rotation`
//!   — that would need kotva's own `KeyRotation`/`RecoveryPolicy` CBOR shapes
//!   translated the way `content_id_of_hash` bridges `ContentId`, and is
//!   future work, not assumed here.
//! * **The signing codec differs and no seam mediates it.** Every signed kotva
//!   object is canonical integer-keyed deterministic CBOR; every signed
//!   magnetite object (`Challenge`, `TokenClaims`, `SignedAd`) is a hand-rolled
//!   little-endian byte concatenation. Neither is wrong, but they are not
//!   interchangeable, and nothing in this crate bridges them — that is the
//!   package-format work in `ALIGNMENT.md` §7 phase 1 item 2, not this.
//! * **`ContentId` is a `Vec<u8>`, magnetite's `Hash` is `[u8; 32]`**, so
//!   `hash_from_content_id` is fallible where `content_id_of_hash` is not.
//! * **kotva-core is a heavy leaf for two seams.** It pulls `hpke`, `x-wing`,
//!   `ml-dsa`, `chacha20poly1305`, `ciborium`, `hkdf`, `sha2` and
//!   `unicode-normalization` — none of which `identity`, `id` or `keyname` need.
//!   There is no finer-grained feature on kotva-core to ask for. `Cargo.toml`
//!   records what that costs.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use kotva_core::identity::{verify_domain as kotva_verify_domain, IdentityKey};
use kotva_core::keyname;
use kotva_core::ContentId;

use magnetite_seams::{
    Audience, AuthProvider, Challenge, Hash, Identity, LoginResponse, Naming, PubKey, Result,
    Scope, SeamError, Session, Sig, Token, TokenClaims,
};

/// The empty domain label. See the crate docs: magnetite's [`Identity`] trait has
/// no domain parameter, and using an empty label is what makes
/// [`KotvaIdentity`]'s signatures byte-identical to
/// [`RawKeypairAuth`](magnetite_seams::RawKeypairAuth)'s, so the shared
/// [`Challenge`]/[`Token`] types verify under either provider.
const NO_DOMAIN: &[u8] = b"";

/// Current unix time in whole seconds, for token/challenge expiry.
///
/// Inlined rather than exported from `magnetite-seams`, whose `now_unix` is
/// `pub(crate)`: widening a helper's visibility across a crate boundary to serve
/// one adapter is a larger change than four lines, and it would make an internal
/// detail part of the seam crate's public API.
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// --- Content addressing bridge ---------------------------------------------------------------

/// Promote a magnetite [`Hash`](struct@Hash) to a kotva [`ContentId`] — prefix
/// only, no rehash.
///
/// Infallible: magnetite's `Hash` is by construction a 32-byte BLAKE3-256
/// digest, which is precisely what a `0x1e`-prefixed `ContentId` holds.
pub fn content_id_of_hash(h: &Hash) -> ContentId {
    let mut v = Vec::with_capacity(33);
    v.push(kotva_core::id::MH_BLAKE3_256);
    v.extend_from_slice(&h.0);
    ContentId(v)
}

/// Demote a kotva [`ContentId`] to a magnetite [`Hash`](struct@Hash).
///
/// **Fails closed** on any algorithm prefix other than BLAKE3-256 (`0x1e`) or a
/// digest that is not 32 bytes: magnetite's `Hash` cannot represent another hash
/// function, so guessing would be worse than refusing. A kotva peer that
/// migrates its agility prefix becomes unaddressable here, loudly.
pub fn hash_from_content_id(id: &ContentId) -> Result<Hash> {
    match id.algorithm() {
        Some(kotva_core::id::MH_BLAKE3_256) => {
            let digest: [u8; 32] = id.digest().try_into().map_err(|_| {
                SeamError::MalformedKey(format!(
                    "kotva ContentId digest must be 32 bytes, got {}",
                    id.digest().len()
                ))
            })?;
            Ok(Hash(digest))
        }
        Some(other) => Err(SeamError::MalformedKey(format!(
            "kotva ContentId uses hash algorithm {other:#04x}; magnetite Hash is BLAKE3-256 \
             ({:#04x}) only",
            kotva_core::id::MH_BLAKE3_256
        ))),
        None => Err(SeamError::MalformedKey("empty kotva ContentId".into())),
    }
}

// --- Seam §3.1 — Identity / AuthProvider -----------------------------------------------------

/// [`Identity`] + [`AuthProvider`] over [`kotva_core::identity::IdentityKey`].
///
/// A drop-in alternative to [`RawKeypairAuth`](magnetite_seams::RawKeypairAuth)
/// with the same challenge/response protocol and the same wire types. The
/// difference is *where the key lives*: the signing key is kotva's `IdentityKey`,
/// so the same key can be handed to kotva code (`DeviceCert::issue`,
/// `RecoveryPolicy::sign`, `pubobj` signing) without re-deriving anything.
pub struct KotvaIdentity {
    ik: IdentityKey,
    /// The 32-byte seed, retained so [`seed`](Self::seed) can persist the node
    /// identity across restarts. `IdentityKey` exposes no secret-key accessor,
    /// so this is the only way to keep parity with `RawKeypairAuth::seed`.
    seed: [u8; 32],
    /// Seconds a challenge stays valid.
    pub challenge_ttl: u64,
    /// Seconds a minted token stays valid.
    pub token_ttl: u64,
    spent_nonces: Mutex<HashSet<[u8; 32]>>,
}

impl KotvaIdentity {
    /// Build from an explicit 32-byte Ed25519 seed (deterministic — for tests
    /// and for reloading a persisted node key).
    ///
    /// The same seed passed to `RawKeypairAuth::from_seed` yields the same public
    /// key; both are Ed25519 over the same seed.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            ik: IdentityKey::from_seed(&seed),
            seed,
            challenge_ttl: 300,
            token_ttl: 900,
            spent_nonces: Mutex::new(HashSet::new()),
        }
    }

    /// Generate a fresh random identity from the OS CSPRNG.
    ///
    /// Seeded locally rather than through `IdentityKey::generate()` so
    /// [`seed`](Self::seed) can return the secret half — see that field's note.
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        Self::from_seed(seed)
    }

    /// The authority/node public key.
    pub fn node_pubkey(&self) -> PubKey {
        PubKey(self.ik.public_array())
    }

    /// The 32-byte secret seed backing this keypair.
    ///
    /// **Private key material.** Same contract as `RawKeypairAuth::seed`: write
    /// it only to owner-readable storage, never to a log, an ad, or the wire.
    pub fn seed(&self) -> [u8; 32] {
        self.seed
    }

    /// Borrow the underlying kotva key, for kotva-native operations this seam
    /// has no vocabulary for (`DeviceCert::issue`, `RecoveryPolicy::sign`, …).
    pub fn identity_key(&self) -> &IdentityKey {
        &self.ik
    }

    /// Sign with kotva's explicit domain separation: the signature covers
    /// `domain ‖ msg`.
    ///
    /// The result does **not** verify under `<KotvaIdentity as Identity>::verify`
    /// (which uses the empty domain) unless `domain` is empty — that separation
    /// is the purpose of the label. Verify with [`verify_domain`].
    pub fn sign_domain(&self, domain: &[u8], msg: &[u8]) -> Result<Sig> {
        sig_from_vec(self.ik.sign_domain(domain, msg))
    }
}

/// Verify a domain-separated kotva signature (the inverse of
/// [`KotvaIdentity::sign_domain`]).
///
/// Uses kotva's `verify_strict` path, so it also rejects non-canonical and
/// small-order public keys.
pub fn verify_domain(pk: &PubKey, domain: &[u8], msg: &[u8], sig: &Sig) -> bool {
    kotva_verify_domain(&pk.0, domain, msg, &sig.0).is_ok()
}

/// kotva returns signatures as `Vec<u8>`; magnetite's [`Sig`] is `[u8; 64]`.
fn sig_from_vec(v: Vec<u8>) -> Result<Sig> {
    let arr: [u8; 64] = v.try_into().map_err(|v: Vec<u8>| {
        SeamError::MalformedKey(format!("kotva signature must be 64 bytes, got {}", v.len()))
    })?;
    Ok(Sig(arr))
}

impl Identity for KotvaIdentity {
    fn pubkey(&self) -> PubKey {
        self.node_pubkey()
    }

    /// Signs the bare message (empty kotva domain) so the bytes match
    /// [`RawKeypairAuth`](magnetite_seams::RawKeypairAuth) exactly.
    ///
    /// Ed25519 signatures are always 64 bytes, so the length conversion cannot
    /// fail in practice; the trait returns `Sig` rather than `Result<Sig>`, so a
    /// hypothetical short signature is turned into an all-zero `Sig`, which
    /// fails every verification. Fail-closed rather than panic.
    fn sign(&self, msg: &[u8]) -> Sig {
        sig_from_vec(self.ik.sign_domain(NO_DOMAIN, msg)).unwrap_or(Sig([0u8; 64]))
    }

    /// Verifies via kotva's strict path — see the crate docs on strictness.
    fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
        kotva_verify_domain(&pk.0, NO_DOMAIN, msg, &sig.0).is_ok()
    }
}

#[async_trait::async_trait]
impl AuthProvider for KotvaIdentity {
    async fn challenge(&self, pk: &PubKey) -> Challenge {
        use rand::RngCore;
        let mut nonce = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let mut ch = Challenge {
            subject: *pk,
            nonce,
            expires_at: now_unix() + self.challenge_ttl,
            server_sig: Sig([0u8; 64]),
        };
        ch.server_sig = <Self as Identity>::sign(self, &ch.signing_bytes());
        ch
    }

    async fn verify_login(&self, resp: LoginResponse) -> Result<Session> {
        let ch = &resp.challenge;
        let bytes = ch.signing_bytes();

        // Same four checks, same order, as `RawKeypairAuth::verify_login`.
        // 1. We must have issued this challenge (stateless MAC check).
        if !<Self as Identity>::verify(&self.node_pubkey(), &bytes, &ch.server_sig) {
            return Err(SeamError::UntrustedChallenge);
        }
        // 2. Freshness.
        if now_unix() >= ch.expires_at {
            return Err(SeamError::ChallengeExpired);
        }
        // 3. The subject proves key control.
        if !<Self as Identity>::verify(&ch.subject, &bytes, &resp.client_sig) {
            return Err(SeamError::InvalidSignature);
        }
        // 4. Single use.
        {
            let mut spent = self.spent_nonces.lock().unwrap();
            if !spent.insert(ch.nonce) {
                return Err(SeamError::ChallengeReplayed);
            }
        }

        let token = self
            .mint_scoped_token(
                &ch.subject,
                Audience("session".into()),
                Scope(vec!["session".into()]),
            )
            .await;
        Ok(Session {
            subject: ch.subject,
            token,
        })
    }

    async fn mint_scoped_token(&self, pk: &PubKey, aud: Audience, scope: Scope) -> Token {
        use rand::RngCore;
        let mut nonce = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let now = now_unix();
        let claims = TokenClaims {
            issuer: self.node_pubkey(),
            subject: *pk,
            audience: aud,
            scope,
            issued_at: now,
            expires_at: now + self.token_ttl,
            nonce,
        };
        let sig = <Self as Identity>::sign(self, &claims.signing_bytes());
        Token { claims, sig }
    }
}

// --- Seam §3.2 — Naming ----------------------------------------------------------------------

/// Render `pk` as its kotva key-name: 8 data words + 1 checksum word, hyphenated.
///
/// Pinned to `core-v0.2.0`'s derivation (`BLAKE3(pubkey)`); see the crate docs
/// for why that is not yet a stable identifier.
pub fn key_name_of(pk: &PubKey) -> String {
    keyname::encode(&pk.0)
}

/// Whether `name` passes kotva's internal checksum word.
///
/// This proves the name was not mistyped or truncated. It does **not** prove the
/// name belongs to any particular key — use [`key_name_matches`] for that.
pub fn key_name_is_well_formed(name: &str) -> bool {
    keyname::verify(name)
}

/// Whether `name` is the kotva key-name of `pk` (checksum **and** key binding).
pub fn key_name_matches(name: &str, pk: &PubKey) -> bool {
    keyname::matches(name, &pk.0)
}

/// [`Naming`] over [`kotva_core::keyname`] — the zero-authority, checksummed
/// word-name a kotva node shows for the same key.
///
/// Resolution order, all fail-closed (`None`, never a guess):
/// 1. a 64-char hex key — the substrate form stays addressable, matching
///    [`HashNaming`](magnetite_seams::HashNaming);
/// 2. a kotva key-name that passes the checksum **and** names a key this node
///    has [`learn`](Self::learn)ed. A name that fails the checksum is rejected
///    before the directory is consulted;
/// 3. a locally registered alias (never authoritative, same as `HashNaming`).
#[derive(Default)]
pub struct KotvaNaming {
    /// kotva key-name → key, for keys this node has seen. A hint cache.
    directory: Mutex<HashMap<String, PubKey>>,
    /// Free-form local aliases → key. Never authoritative.
    aliases: Mutex<HashMap<String, PubKey>>,
}

impl KotvaNaming {
    /// Fresh provider with nothing learned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a key so its kotva key-name resolves. Idempotent.
    ///
    /// Learning a key grants it nothing — 80 bits of name cannot invert a
    /// 256-bit key, so a directory is the only way a word-name can resolve at
    /// all, and it is a cache of keys already obtained by some trusted path
    /// (a signed ad, a contact exchange).
    pub fn learn(&self, pk: PubKey) {
        self.directory.lock().unwrap().insert(key_name_of(&pk), pk);
    }

    /// Register a local human alias → key hint (never authoritative).
    pub fn register(&self, name: &str, pk: PubKey) {
        self.aliases.lock().unwrap().insert(name.to_string(), pk);
    }

    /// The canonical, name-less address for a key (full hex) — identical to
    /// [`HashNaming::canonical`](magnetite_seams::HashNaming::canonical), because
    /// the substrate address is the key in both.
    pub fn canonical(&self, pk: &PubKey) -> String {
        pk.to_hex()
    }
}

#[async_trait::async_trait]
impl Naming for KotvaNaming {
    async fn resolve(&self, name: &str) -> Option<PubKey> {
        // 1. Raw hex — the substrate form is always addressable.
        if let Ok(pk) = PubKey::from_hex(name) {
            return Some(pk);
        }
        // 2. A kotva key-name. Checksum FIRST: a mistyped name must fail closed
        //    rather than get a directory lookup it might satisfy by accident.
        if key_name_is_well_formed(name) {
            return self.directory.lock().unwrap().get(name).copied();
        }
        // 3. Local alias hint.
        self.aliases.lock().unwrap().get(name).copied()
    }

    fn display(&self, pk: &PubKey) -> String {
        key_name_of(pk)
    }
}

#[cfg(test)]
mod tests;
