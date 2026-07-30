//! Seam §3.1 — `Identity` / `AuthProvider`.
//!
//! Identity is a keypair. The default [`RawKeypairAuth`] implements raw Ed25519
//! challenge/response login with **no external dependency**, and doubles as the
//! node's own [`Identity`]. It can also act as a lightweight IdP: it mints
//! short-lived, audience- and scope-bound [`Token`]s so external comms systems
//! (Matrix / Jitsi / LiveKit) can be entered from a single keypair login.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::sync::Mutex;

use crate::error::{Result, SeamError};
use crate::now_unix;

/// Ed25519 public key — the substrate identity for a player or node.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PubKey(pub [u8; 32]);

/// Ed25519 signature.
#[derive(Clone, Copy)]
pub struct Sig(pub [u8; 64]);

impl std::fmt::Debug for PubKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PubKey({})", hex::encode(self.0))
    }
}

impl std::fmt::Debug for Sig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sig({}…)", hex::encode(&self.0[..8]))
    }
}

impl PubKey {
    /// Full lowercase-hex encoding — the canonical, name-less address.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse a 64-char hex pubkey.
    pub fn from_hex(s: &str) -> Result<Self> {
        let raw =
            hex::decode(s).map_err(|e| SeamError::MalformedKey(format!("pubkey hex: {e}")))?;
        let arr: [u8; 32] = raw
            .try_into()
            .map_err(|_| SeamError::MalformedKey("pubkey must be 32 bytes".into()))?;
        Ok(PubKey(arr))
    }
}

// Hex (de)serialization keeps JSON payloads compact and human-diffable, and
// sidesteps serde's lack of a built-in impl for `[u8; 64]`.
impl Serialize for PubKey {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(self.0))
    }
}
impl<'de> Deserialize<'de> for PubKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        PubKey::from_hex(&s).map_err(serde::de::Error::custom)
    }
}
impl Serialize for Sig {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(self.0))
    }
}
impl<'de> Deserialize<'de> for Sig {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let raw = hex::decode(&s).map_err(serde::de::Error::custom)?;
        let arr: [u8; 64] = raw
            .try_into()
            .map_err(|_| serde::de::Error::custom("signature must be 64 bytes"))?;
        Ok(Sig(arr))
    }
}

/// The substrate identity trait: a keypair that can sign and verify.
///
/// **A7 (fixed).** `verify` was originally an associated function — "it only
/// needs the public key, never the secret half" — so every caller had to name
/// a *concrete type* to verify anything (`<RawKeypairAuth as
/// Identity>::verify(...)`), and the crate's `Token` type used to expose an
/// `is_valid_at` method that hard-coded `RawKeypairAuth` specifically. That
/// was a silent trap: a provider whose signature bytes differ (a domain tag,
/// a different curve) would not error, it would just make verification
/// return `false` for a token that is actually valid, or — worse, in
/// principle — `true` for one that is not, if the two algorithms ever agreed
/// on the wrong pair. `is_valid_at` has since been deleted (see
/// [`Token::is_valid_for`]'s docs) because auditing every caller in the tree
/// found none in production code — only tests, which have all been migrated.
///
/// `verify` itself is STILL an associated function, not a method: eight call
/// sites elsewhere in this tree (`magnetite-runtime`'s `fleet`/`cluster`/
/// `follow` modules and `magnetite-solana-rail`'s handshake path) invoke it by
/// that exact name via UFCS, and Rust has no method overloading — turning
/// `verify` itself into a `&self` method would require an edit at every one of
/// those call sites in the same breaking change. Those eight sites were
/// individually audited for the A7 finish: every one of them is part of a
/// **closed** subsystem where the signing side is *also* hard-typed to
/// `RawKeypairAuth` (e.g. `FollowToken::mint(id: &RawKeypairAuth, ...)`), so
/// there is no live cross-provider mismatch there today — unlike `Token`,
/// which is generically issued by any [`AuthProvider`]. `verify` staying an
/// associated function is therefore tracked as a design wart, not a live bug.
///
/// [`IdentityVerifier`] below is the actual fix for the cases that ARE
/// pluggable: a **method**, blanket-implemented for every `Identity`, so any
/// concrete provider — the one actually "in play" at a call site — is
/// reachable as `&dyn IdentityVerifier` and dispatches through its own
/// algorithm rather than a hard-coded type. See [`Token::is_valid_for`],
/// [`crate::package::Package::verify_for`], and the
/// `is_valid_for_follows_the_provider_*` tests below, which prove the
/// dispatch actually follows the instance passed in rather than
/// rubber-stamping.
pub trait Identity {
    /// This identity's Ed25519 public key.
    fn pubkey(&self) -> PubKey;
    /// Sign a message with this identity's secret key.
    fn sign(&self, msg: &[u8]) -> Sig;
    /// Verify a detached signature against a public key and message.
    ///
    /// Associated function, not a method — see the trait docs on why this
    /// stays as-is and [`IdentityVerifier`] for the method-based fix.
    fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool;
}

/// **A7 fix.** A live, per-instance, dynamically-dispatchable verifier — the
/// "provider in play" that [`Identity::verify`] could not be, because it
/// takes no `self`.
///
/// Blanket-implemented for every [`Identity`], so nothing that already
/// implements `Identity` (in this crate, in `magnetite-kotva`, or anywhere
/// else) has to change to gain it: `RawKeypairAuth`, `KotvaIdentity` and any
/// test double are all `IdentityVerifier`s automatically. Holding `&dyn IdentityVerifier`
/// lets a caller verify against *whichever concrete provider is actually
/// running* instead of a type named at compile time.
pub trait IdentityVerifier {
    /// Verify `sig` over `msg` under `pk`, using this instance's algorithm.
    fn verify(&self, pk: &PubKey, msg: &[u8], sig: &Sig) -> bool;
}

impl<T: Identity> IdentityVerifier for T {
    fn verify(&self, pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
        <T as Identity>::verify(pk, msg, sig)
    }
}

/// A login challenge. Self-contained and MAC'd by the authority so login can be
/// verified statelessly: the authority proves it issued the challenge via
/// `server_sig`, and the client proves key control by signing [`Challenge::signing_bytes`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Challenge {
    /// The key the challenge is addressed to.
    pub subject: PubKey,
    /// Random single-use nonce.
    pub nonce: [u8; 32],
    /// Unix seconds after which the challenge is dead.
    pub expires_at: u64,
    /// Authority's signature over `signing_bytes` (proves we issued it).
    pub server_sig: Sig,
}

impl Challenge {
    /// Canonical bytes both sides sign: `subject || nonce || expires_at`.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(32 + 32 + 8);
        b.extend_from_slice(&self.subject.0);
        b.extend_from_slice(&self.nonce);
        b.extend_from_slice(&self.expires_at.to_le_bytes());
        b
    }
}

/// A client's answer to a [`Challenge`]: the challenge plus a signature over its
/// `signing_bytes` produced by the subject's secret key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    /// The challenge being answered.
    pub challenge: Challenge,
    /// Subject's signature over `challenge.signing_bytes()`.
    pub client_sig: Sig,
}

/// A verified login — the authenticated key plus a freshly minted session token.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    /// The authenticated public key.
    pub subject: PubKey,
    /// Session token (audience `"session"`).
    pub token: Token,
}

/// Intended recipient system of a scoped token (e.g. `"matrix"`, `"jitsi"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Audience(pub String);

/// A set of granted permission strings (e.g. `["room:join", "voice"]`).
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Scope(pub Vec<String>);

/// The signed claim body of a [`Token`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    /// The authority (node) that minted the token.
    pub issuer: PubKey,
    /// The player key the token speaks for.
    pub subject: PubKey,
    /// Which external system may accept it.
    pub audience: Audience,
    /// Granted permissions.
    pub scope: Scope,
    /// Issued-at, unix seconds.
    pub issued_at: u64,
    /// Expiry, unix seconds.
    pub expires_at: u64,
    /// Anti-replay nonce.
    pub nonce: [u8; 16],
}

impl TokenClaims {
    /// Deterministic canonical encoding used as the signing message.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.issuer.0);
        b.extend_from_slice(&self.subject.0);
        b.extend_from_slice(&(self.audience.0.len() as u32).to_le_bytes());
        b.extend_from_slice(self.audience.0.as_bytes());
        b.extend_from_slice(&(self.scope.0.len() as u32).to_le_bytes());
        for s in &self.scope.0 {
            b.extend_from_slice(&(s.len() as u32).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
        }
        b.extend_from_slice(&self.issued_at.to_le_bytes());
        b.extend_from_slice(&self.expires_at.to_le_bytes());
        b.extend_from_slice(&self.nonce);
        b
    }
}

/// A short-lived, audience+scope-bound credential signed by the authority's key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Token {
    /// The claims.
    pub claims: TokenClaims,
    /// Authority signature over `claims.signing_bytes()`.
    pub sig: Sig,
}

impl Token {
    /// Verify the token against `provider` — the auth provider actually in
    /// play — instead of assuming `RawKeypairAuth`.
    ///
    /// **A7 fix, now the ONLY verification path.** A previous, hard-coded
    /// `is_valid_at(&self, now: u64) -> bool` used to check every token's
    /// signature under `RawKeypairAuth`'s algorithm specifically, regardless
    /// of which provider actually issued it — silently wrong (not erroring)
    /// for any provider whose signature bytes were not byte-identical to
    /// `RawKeypairAuth`'s. It was kept temporarily rather than fixed in place
    /// because callers throughout the tree depended on its exact signature.
    /// Auditing every one of those callers (A7's remaining scope) found that
    /// **all of them were test code** — zero production call sites anywhere
    /// in the 16-crate tree called it — so it has been deleted outright
    /// rather than deprecated: there is no external consumer for a
    /// workspace-internal crate to break, and a `#[deprecated]` attribute
    /// would have left the trap callable.
    ///
    /// Pass the live [`Identity`] instance whose algorithm actually produced
    /// (or should have produced) `self.sig`. Because [`IdentityVerifier`] is
    /// blanket-implemented for every `Identity`, any provider —
    /// `RawKeypairAuth`, `magnetite-kotva`'s `KotvaIdentity`, or a foreign one
    /// this crate has never heard of — works here with no special-casing.
    ///
    /// `now` is unix seconds; pass the current time in production.
    pub fn is_valid_for(&self, provider: &dyn IdentityVerifier, now: u64) -> bool {
        if now >= self.claims.expires_at {
            return false;
        }
        let msg = self.claims.signing_bytes();
        provider.verify(&self.claims.issuer, &msg, &self.sig)
    }
}

/// Challenge-response login provider (§3.1). The default is [`RawKeypairAuth`].
#[async_trait::async_trait]
pub trait AuthProvider {
    /// Issue a fresh login challenge for a public key.
    async fn challenge(&self, pk: &PubKey) -> Challenge;
    /// Verify a signed challenge response and open a session.
    async fn verify_login(&self, resp: LoginResponse) -> Result<Session>;
    /// Act as an IdP: mint a scoped, short-lived credential for an external system.
    async fn mint_scoped_token(&self, pk: &PubKey, aud: Audience, scope: Scope) -> Token;
}

/// Default auth provider: raw Ed25519 challenge/response, no external services.
///
/// Holds the node's own signing key, so it is simultaneously the node
/// [`Identity`] and the authority that MAC's challenges and mints tokens.
pub struct RawKeypairAuth {
    signing_key: SigningKey,
    /// Seconds a challenge stays valid.
    pub challenge_ttl: u64,
    /// Seconds a minted token stays valid.
    pub token_ttl: u64,
    spent_nonces: Mutex<HashSet<[u8; 32]>>,
}

impl RawKeypairAuth {
    /// Build from an explicit 32-byte seed (deterministic — handy for tests).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&seed),
            challenge_ttl: 300,
            token_ttl: 900,
            spent_nonces: Mutex::new(HashSet::new()),
        }
    }

    /// Generate a fresh random node keypair from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        Self::from_seed(seed)
    }

    /// The authority/node public key.
    pub fn node_pubkey(&self) -> PubKey {
        PubKey(self.signing_key.verifying_key().to_bytes())
    }

    /// The 32-byte secret seed backing this keypair.
    ///
    /// This is **private key material**: it exists so a node can persist its
    /// identity across restarts (see `magnetite node --node-key-file`). Write it
    /// only to owner-readable storage and never to a log, an ad, or the wire.
    pub fn seed(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    fn sign_bytes(&self, msg: &[u8]) -> Sig {
        Sig(self.signing_key.sign(msg).to_bytes())
    }
}

impl Identity for RawKeypairAuth {
    fn pubkey(&self) -> PubKey {
        self.node_pubkey()
    }
    fn sign(&self, msg: &[u8]) -> Sig {
        self.sign_bytes(msg)
    }
    fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
        let vk = match VerifyingKey::from_bytes(&pk.0) {
            Ok(vk) => vk,
            Err(_) => return false,
        };
        vk.verify(msg, &Signature::from_bytes(&sig.0)).is_ok()
    }
}

#[async_trait::async_trait]
impl AuthProvider for RawKeypairAuth {
    async fn challenge(&self, pk: &PubKey) -> Challenge {
        let mut nonce = [0u8; 32];
        OsRng.fill_bytes(&mut nonce);
        let mut ch = Challenge {
            subject: *pk,
            nonce,
            expires_at: now_unix() + self.challenge_ttl,
            server_sig: Sig([0u8; 64]),
        };
        ch.server_sig = self.sign_bytes(&ch.signing_bytes());
        ch
    }

    async fn verify_login(&self, resp: LoginResponse) -> Result<Session> {
        let ch = &resp.challenge;
        let bytes = ch.signing_bytes();

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
        let mut nonce = [0u8; 16];
        OsRng.fill_bytes(&mut nonce);
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
        let sig = self.sign_bytes(&claims.signing_bytes());
        Token { claims, sig }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in "player" keypair used to answer challenges in tests.
    struct Player(SigningKey);
    impl Player {
        fn new(seed: [u8; 32]) -> Self {
            Player(SigningKey::from_bytes(&seed))
        }
        fn pubkey(&self) -> PubKey {
            PubKey(self.0.verifying_key().to_bytes())
        }
        fn sign(&self, msg: &[u8]) -> Sig {
            Sig(self.0.sign(msg).to_bytes())
        }
    }

    #[test]
    fn seed_round_trips_so_a_node_identity_can_be_persisted() {
        // A node writes `seed()` to its key file and reloads it on restart; the
        // reloaded key must be the SAME identity, or every peer that pinned it
        // and every tracker slot bound to it is orphaned.
        let node = RawKeypairAuth::generate();
        let reloaded = RawKeypairAuth::from_seed(node.seed());
        assert_eq!(node.node_pubkey().0, reloaded.node_pubkey().0);
        assert_eq!(node.seed(), reloaded.seed());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let node = RawKeypairAuth::from_seed([1u8; 32]);
        let msg = b"authoritative tick 42";
        let sig = node.sign(msg);
        assert!(<RawKeypairAuth as Identity>::verify(
            &node.pubkey(),
            msg,
            &sig
        ));
        // Tampered message fails.
        assert!(!<RawKeypairAuth as Identity>::verify(
            &node.pubkey(),
            b"authoritative tick 43",
            &sig
        ));
    }

    #[tokio::test]
    async fn challenge_response_login_and_token() {
        let node = RawKeypairAuth::from_seed([2u8; 32]);
        let player = Player::new([9u8; 32]);

        let ch = node.challenge(&player.pubkey()).await;
        let client_sig = player.sign(&ch.signing_bytes());
        let resp = LoginResponse {
            challenge: ch,
            client_sig,
        };

        let session = node.verify_login(resp).await.expect("login ok");
        assert_eq!(session.subject, player.pubkey());
        assert!(session.token.is_valid_for(&node, now_unix()));
        assert_eq!(session.token.claims.issuer, node.node_pubkey());
        // Token expired in the future -> invalid once past expiry.
        assert!(!session
            .token
            .is_valid_for(&node, session.token.claims.expires_at));
    }

    #[tokio::test]
    async fn replayed_challenge_is_rejected() {
        let node = RawKeypairAuth::from_seed([3u8; 32]);
        let player = Player::new([8u8; 32]);
        let ch = node.challenge(&player.pubkey()).await;
        let client_sig = player.sign(&ch.signing_bytes());
        let resp = LoginResponse {
            challenge: ch,
            client_sig,
        };
        assert!(node.verify_login(resp.clone()).await.is_ok());
        assert!(matches!(
            node.verify_login(resp).await,
            Err(SeamError::ChallengeReplayed)
        ));
    }

    #[tokio::test]
    async fn forged_challenge_is_rejected() {
        let node = RawKeypairAuth::from_seed([4u8; 32]);
        let attacker = RawKeypairAuth::from_seed([5u8; 32]);
        let player = Player::new([7u8; 32]);
        // Challenge minted by a DIFFERENT authority.
        let ch = attacker.challenge(&player.pubkey()).await;
        let client_sig = player.sign(&ch.signing_bytes());
        let resp = LoginResponse {
            challenge: ch,
            client_sig,
        };
        assert!(matches!(
            node.verify_login(resp).await,
            Err(SeamError::UntrustedChallenge)
        ));
    }

    /// A SECOND, deliberately different `Identity` provider for the A7 proof
    /// below. Unlike `magnetite-kotva`'s `KotvaIdentity` (which chose an
    /// **empty** domain specifically so its bytes match `RawKeypairAuth`'s),
    /// this one signs `DOMAIN_TAG ‖ msg` — its signature bytes are NEVER
    /// equal to `RawKeypairAuth`'s over the same key and message. A test that
    /// only ever exercises `RawKeypairAuth` (or a provider engineered to be
    /// byte-identical to it) cannot distinguish "verification follows the
    /// provider" from "every provider happens to agree" — this one can.
    struct DomainTaggedAuth(SigningKey);

    const DOMAIN_TAG: &[u8] = b"magnetite-seams/domain-tagged-test/v1";

    impl DomainTaggedAuth {
        fn from_seed(seed: [u8; 32]) -> Self {
            DomainTaggedAuth(SigningKey::from_bytes(&seed))
        }
        fn tagged(msg: &[u8]) -> Vec<u8> {
            let mut v = DOMAIN_TAG.to_vec();
            v.extend_from_slice(msg);
            v
        }
    }

    impl Identity for DomainTaggedAuth {
        fn pubkey(&self) -> PubKey {
            PubKey(self.0.verifying_key().to_bytes())
        }
        fn sign(&self, msg: &[u8]) -> Sig {
            Sig(self.0.sign(&Self::tagged(msg)).to_bytes())
        }
        fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
            let vk = match VerifyingKey::from_bytes(&pk.0) {
                Ok(vk) => vk,
                Err(_) => return false,
            };
            vk.verify(&Self::tagged(msg), &Signature::from_bytes(&sig.0))
                .is_ok()
        }
    }

    #[test]
    fn domain_tagged_signatures_really_do_differ_from_raw_keypair_auths() {
        // Sanity check on the test double itself: if this ever failed, the
        // "SECOND, deliberately different" premise below would be false.
        let seed = [77u8; 32];
        let tagged = DomainTaggedAuth::from_seed(seed);
        let raw = RawKeypairAuth::from_seed(seed);
        let msg = b"a message both providers sign";
        assert_eq!(tagged.pubkey(), raw.pubkey(), "same seed, same key");
        assert_ne!(
            tagged.sign(msg).0,
            raw.sign(msg).0,
            "different algorithms MUST NOT produce the same signature bytes"
        );
    }

    #[test]
    fn hard_coding_the_wrong_provider_silently_mis_verifies_an_honest_token() {
        // Reproduces the A7 defect's SHAPE directly against `Identity::verify`,
        // rather than assuming it: a token honestly issued by a provider that
        // is NOT byte-compatible with RawKeypairAuth is REJECTED when checked
        // under a hard-coded `RawKeypairAuth` verify call — not with an error,
        // just silently `false` — even though the signature is perfectly valid
        // under the provider that actually issued it. This is the exact defect
        // the deleted `Token::is_valid_at` used to exhibit (see `is_valid_for`'s
        // docs); it is kept here as a standing regression check because the
        // underlying pattern — verifying via a named concrete type instead of
        // the live instance — still exists at the handful of call sites this
        // A7 pass found to be architecturally closed (see the `Identity` trait
        // docs) and would reappear instantly if anyone reintroduced it into a
        // pluggable path like `Token`.
        let issuer = DomainTaggedAuth::from_seed([88u8; 32]);
        let now = now_unix();
        let claims = TokenClaims {
            issuer: issuer.pubkey(),
            subject: PubKey([9u8; 32]),
            audience: Audience("session".into()),
            scope: Scope(vec!["session".into()]),
            issued_at: now,
            expires_at: now + 900,
            nonce: [3u8; 16],
        };
        let sig = issuer.sign(&claims.signing_bytes());
        let tok = Token { claims, sig };

        assert!(
            now < tok.claims.expires_at,
            "test bug: token must still be fresh for this to be a signature-only check"
        );
        assert!(
            !<RawKeypairAuth as Identity>::verify(
                &tok.claims.issuer,
                &tok.claims.signing_bytes(),
                &tok.sig
            ),
            "hard-coding RawKeypairAuth: a HONESTLY-ISSUED token from a real (non-default) \
             provider is wrongly rejected, and does so silently rather than erring"
        );
        // The corrected form, given the ACTUAL issuing provider, agrees the
        // token is good — proving the mismatch above is purely a provider
        // problem, not a malformed token.
        assert!(tok.is_valid_for(&issuer, now));
    }

    #[tokio::test]
    async fn is_valid_for_follows_the_provider_rather_than_the_hard_coded_one() {
        // The A7 fix, proved with the second, byte-different provider above:
        // `is_valid_for` verifies correctly when given the ACTUAL issuing
        // provider, and correctly REJECTS when given a different one — so
        // this is not a rubber stamp, it genuinely dispatches per-instance.
        let issuer = DomainTaggedAuth::from_seed([99u8; 32]);
        let now = now_unix();
        let claims = TokenClaims {
            issuer: issuer.pubkey(),
            subject: PubKey([1u8; 32]),
            audience: Audience("matrix".into()),
            scope: Scope(vec!["room:join".into()]),
            issued_at: now,
            expires_at: now + 900,
            nonce: [4u8; 16],
        };
        let sig = issuer.sign(&claims.signing_bytes());
        let tok = Token { claims, sig };

        // 1. The provider that actually issued it: verifies.
        assert!(
            tok.is_valid_for(&issuer, now),
            "is_valid_for must accept the token under its ACTUAL issuing provider"
        );

        // 2. A different provider algorithm, same key (same seed): must
        //    reject. If verification were secretly still hard-coded to one
        //    algorithm, swapping the passed-in provider would change nothing
        //    and this would spuriously agree with (1) for the wrong reason.
        let raw_same_key = RawKeypairAuth::from_seed([99u8; 32]);
        assert!(
            !tok.is_valid_for(&raw_same_key, now),
            "a structurally different algorithm over the SAME key must not verify \
             a domain-tagged signature — proves this isn't a rubber stamp"
        );

        // 3. Expiry is still enforced regardless of provider.
        assert!(!tok.is_valid_for(&issuer, tok.claims.expires_at));
    }

    #[tokio::test]
    async fn scoped_token_is_audience_bound_and_signed() {
        let node = RawKeypairAuth::from_seed([6u8; 32]);
        let player = Player::new([6u8; 32]);
        let tok = node
            .mint_scoped_token(
                &player.pubkey(),
                Audience("matrix".into()),
                Scope(vec!["room:join".into()]),
            )
            .await;
        assert_eq!(tok.claims.audience, Audience("matrix".into()));
        assert_eq!(tok.claims.subject, player.pubkey());
        assert!(tok.is_valid_for(&node, now_unix()));

        // Any bit-flip in claims breaks the signature.
        let mut bad = tok.clone();
        bad.claims.audience = Audience("jitsi".into());
        assert!(!bad.is_valid_for(&node, now_unix()));
    }
}
