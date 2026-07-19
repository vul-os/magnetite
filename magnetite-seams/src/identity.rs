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
/// `verify` is an associated function because verification only needs the
/// public key, never the secret half.
pub trait Identity {
    /// This identity's Ed25519 public key.
    fn pubkey(&self) -> PubKey;
    /// Sign a message with this identity's secret key.
    fn sign(&self, msg: &[u8]) -> Sig;
    /// Verify a detached signature against a public key and message.
    fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool;
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
    /// Verify the token is well-formed, unexpired, and signed by `issuer`.
    ///
    /// `now` is unix seconds; pass the current time in production.
    pub fn is_valid_at(&self, now: u64) -> bool {
        if now >= self.claims.expires_at {
            return false;
        }
        let msg = self.claims.signing_bytes();
        <RawKeypairAuth as Identity>::verify(&self.claims.issuer, &msg, &self.sig)
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
        assert!(session.token.is_valid_at(now_unix()));
        assert_eq!(session.token.claims.issuer, node.node_pubkey());
        // Token expired in the future -> invalid once past expiry.
        assert!(!session.token.is_valid_at(session.token.claims.expires_at));
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
        assert!(tok.is_valid_at(now_unix()));

        // Any bit-flip in claims breaks the signature.
        let mut bad = tok.clone();
        bad.claims.audience = Audience("jitsi".into());
        assert!(!bad.is_valid_at(now_unix()));
    }
}
