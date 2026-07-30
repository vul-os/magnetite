//! Ed25519 signing key + StrKey address encoding for the Stellar wallet this
//! rail can spend from.
//!
//! Design copied by hand from `patala/patala-stellar/src/keys.rs` (see this
//! crate's `Cargo.toml` for why that is a design copy, not a dependency).
//! Stellar is Ed25519-native and its account addresses (`G...`) / secret seeds
//! (`S...`) are StrKey encodings (version byte + payload + CRC16-XMODEM
//! checksum, base32) of a raw 32-byte Ed25519 public key / seed — StrKey
//! encode/decode itself is delegated to `stellar-strkey`, the Stellar
//! Development Foundation's own crate for exactly this, rather than
//! reimplemented here.
//!
//! Wallet addresses in this crate are `magnetite_seams::identity::PubKey`
//! (already a raw 32-byte Ed25519 key — the same type every `Leg::wallet`
//! uses) converted to/from StrKey with [`to_strkey`]/[`from_strkey`]; there is
//! no separate wallet-key type here the way `patala-stellar` needed one.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use magnetite_seams::identity::PubKey;
use stellar_strkey::ed25519::{PrivateKey as StrkeySeed, PublicKey as StrkeyPub};

use crate::StellarError;

/// Encode a wallet's raw Ed25519 public key as a Stellar account address (`G...`).
pub fn to_strkey(pk: &PubKey) -> String {
    StrkeyPub(pk.0).to_string()
}

/// Decode a Stellar account address (`G...`). Anything that is not a valid
/// StrKey-encoded Ed25519 public key — bad checksum, wrong version byte, wrong
/// length, not StrKey at all — is rejected, never coerced.
pub fn from_strkey(s: &str) -> Result<PubKey, StellarError> {
    StrkeyPub::from_string(s)
        .map(|k| PubKey(k.0))
        .map_err(|_| StellarError::BadAddress(s.to_string()))
}

/// Ed25519 signature — 64 raw bytes.
#[derive(Clone, Copy)]
pub struct Sig(pub [u8; 64]);

/// The wallet signing key this rail can spend from — simultaneously the
/// signing identity and, since a Stellar address is just a StrKey-encoded
/// Ed25519 public key, the wallet the funds move from. No separate mapping
/// table (mirrors `patala-stellar`'s and `magnetite-solana-rail`'s identical
/// stance).
pub struct Keypair(SigningKey);

impl Keypair {
    /// Build from an explicit 32-byte seed (deterministic — for tests and for
    /// `STELLAR_SECRET_KEY` loading).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&seed))
    }

    /// This keypair's public key — also its Stellar wallet address.
    pub fn pubkey(&self) -> PubKey {
        PubKey(self.0.verifying_key().to_bytes())
    }

    /// Sign a message (here: always a 32-byte transaction hash).
    pub fn sign(&self, msg: &[u8]) -> Sig {
        Sig(self.0.sign(msg).to_bytes())
    }

    /// Verify a detached signature against a public key and message.
    pub fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
        let Ok(vk) = VerifyingKey::from_bytes(&pk.0) else {
            return false;
        };
        vk.verify(msg, &Signature::from_bytes(&sig.0)).is_ok()
    }

    /// Load a signing key from `STELLAR_SECRET_KEY` (a StrKey secret seed,
    /// `S...`). `Ok(None)` when unset — a verify-only rail, never logged, never
    /// serialized, never written anywhere by this crate.
    pub fn from_env() -> Result<Option<Self>, StellarError> {
        let Ok(s) = std::env::var("STELLAR_SECRET_KEY") else {
            return Ok(None);
        };
        let seed = StrkeySeed::from_string(s.trim())
            .map_err(|_| StellarError::Config("STELLAR_SECRET_KEY: not a valid S... seed".into()))?
            .0;
        Ok(Some(Self::from_seed(seed)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_roundtrip() {
        let k = Keypair::from_seed([1u8; 32]);
        let msg = b"magnetite-stellar-rail test message";
        let sig = k.sign(msg);
        assert!(Keypair::verify(&k.pubkey(), msg, &sig));
        assert!(!Keypair::verify(&k.pubkey(), b"different message", &sig));
    }

    #[test]
    fn strkey_address_round_trips() {
        let k = Keypair::from_seed([3u8; 32]);
        let addr = to_strkey(&k.pubkey());
        assert!(addr.starts_with('G'));
        let back = from_strkey(&addr).unwrap();
        assert_eq!(back.0, k.pubkey().0);
    }

    #[test]
    fn strkey_rejects_garbage() {
        assert!(from_strkey("not-a-strkey").is_err());
        assert!(from_strkey("").is_err());
        let seed_strkey = StrkeySeed([7u8; 32]).to_string();
        assert!(
            from_strkey(&seed_strkey).is_err(),
            "a seed is not an address"
        );
    }
}
