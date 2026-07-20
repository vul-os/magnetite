//! # magnetite-seams
//!
//! The pluggable **seams** the decentralized Magnetite games platform programs
//! against (see `DECENTRALIZATION.md` §3). Nothing in the game runtime,
//! scheduler, or payment path may name a provider-specific type — everyone sees
//! only these traits:
//!
//! | § | Seam | Trait(s) | Offline default |
//! |---|------|----------|-------------------|
//! | 3.1 | Identity / Auth | [`Identity`], [`AuthProvider`] | [`RawKeypairAuth`] |
//! | 3.2 | Naming | [`Naming`] | [`HashNaming`] |
//! | 3.3 | BlobStore | [`BlobStore`] | [`LocalBlobStore`], [`HttpBlobStore`] |
//! | 3.4 | Discovery | [`Discovery`] | [`LanDiscovery`], [`TrackerDiscovery`] |
//! | 3.5 | CommsProvider | [`CommsProvider`] | [`BuiltinProvider`] |
//! | 3.6 | PaymentRail | [`PaymentRail`] | [`MockPaymentRail`] |
//! | 3.7 | InputProvider | [`InputProvider`] | [`LocalDeviceInput`] |
//!
//! **§3.7 carries a caveat the others do not.** [`InputClass`] splits input into
//! deterministic (replay-verifiable, the platform's core guarantee) and attested
//! (sensor-derived, *not* verifiable at any point). Read [`input`] before
//! consuming attested events — treating them as if they carried the replay
//! guarantee would silently hollow out `verify_replay`.
//!
//! **Every default works with zero external services** — no network, no chain,
//! no homeserver — so CI runs fully offline. Provider-specific adapters live
//! behind their own feature-gated modules and are never referenced by
//! non-provider code.
//!
//! The one optional provider that exists today is [`keyname::KeyNameNaming`]
//! (`--features keyname`), a second `Naming` implementation using word-based
//! key-names. It adds no dependencies and exists to prove the `Naming` seam is
//! genuinely swappable rather than hardwired to its default.
//!
//! The [`defaults`] module wires one working provider set for `magnetite dev`.

pub mod blobstore;
pub mod comms;
pub mod discovery;
mod error;
pub mod identity;
pub mod input;
#[cfg(feature = "keyname")]
pub mod keyname;
pub mod naming;
pub mod payment;

pub use error::{Result, SeamError};

// Seam §3.1 — Identity / Auth
pub use identity::{
    AuthProvider, Audience, Challenge, Identity, LoginResponse, PubKey, RawKeypairAuth, Scope,
    Session, Sig, Token, TokenClaims,
};

// Seam §3.2 — Naming
pub use naming::{HashNaming, Naming};
/// Optional second Naming provider (`--features keyname`) — see [`keyname`].
#[cfg(feature = "keyname")]
pub use keyname::KeyNameNaming;

// Seam §3.3 — BlobStore
pub use blobstore::{BlobFetcher, BlobStore, FsBlobStore, Hash, HttpBlobStore, LocalBlobStore};

// Seam §3.4 — Discovery
pub use discovery::{
    Capacity, Discovery, FanoutDiscovery, Filter, LanDiscovery, NodeAddr, Price, SessionAd,
    SignedAd, SignedWithdraw,
    TrackerClient, TrackerDiscovery, MAX_AD_TTL_SECS,
};

// Seam §3.5 — CommsProvider
pub use comms::{BuiltinProvider, CommsProvider, JoinCred, RoomAddr, RoomScope};

// Seam §3.6 — PaymentRail
pub use payment::{
    Channel, Escrow, MockPaymentRail, PayOut, PaymentRail, PaymentSplit, Receipt, Split, WagerTerms,
};

// Seam §3.7 — InputProvider
//
// `InputClass` is exported alongside the trait on purpose: consuming code that
// touches input should have to name the guarantee class it is relying on.
pub use input::{
    AttestedEvent, AttestedEventInput, DeterministicInput, Implausible, InputClass, InputEvent,
    InputProvider, LocalDeviceInput, PlausibilityGate, PlausibilityLimits, SignedAttestedEvent,
    ATTESTED_DOMAIN,
};

/// Current unix time in whole seconds. Used for token/challenge expiry.
pub(crate) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The default, fully-offline provider set that wires every seam together.
///
/// This is the provider bundle `magnetite dev` uses: raw-keypair identity, hash
/// naming, local blob storage, LAN discovery, the builtin comms shim, the
/// deterministic mock payment rail, and a deterministic local input queue — none
/// of which touch an external service.
///
/// The input slot is deliberately the **deterministic** provider: the default
/// bundle stays fully replay-verifiable, and nothing here depends on a camera or
/// a pose model. Swapping in an attested provider is an explicit choice a host
/// makes, and it trades that guarantee away (see [`crate::input`]).
pub mod defaults {
    use crate::comms::BuiltinProvider;
    use crate::identity::RawKeypairAuth;
    use crate::naming::HashNaming;
    use crate::{LanDiscovery, LocalBlobStore, LocalDeviceInput, MockPaymentRail};

    /// A wired-together default provider set for a single node.
    ///
    /// Construct with [`DefaultSeams::generate`] (random node key) or
    /// [`DefaultSeams::from_seed`] (deterministic, for tests/reproducible demos).
    pub struct DefaultSeams {
        /// Identity + auth (also the node's IdP for comms tokens).
        pub auth: RawKeypairAuth,
        /// Human-name / short-hash display over raw keys.
        pub naming: HashNaming,
        /// Content-addressed local blob store.
        pub blobs: LocalBlobStore,
        /// LAN/in-proc discovery phonebook.
        pub discovery: LanDiscovery,
        /// Builtin comms shim (mints join creds from the node key).
        pub comms: BuiltinProvider<RawKeypairAuth>,
        /// Deterministic offline payment rail.
        pub payments: MockPaymentRail,
        /// Deterministic (replay-verifiable) local input queue.
        pub input: LocalDeviceInput,
    }

    impl DefaultSeams {
        /// Build the default set from an explicit node-key seed (deterministic).
        pub fn from_seed(seed: [u8; 32]) -> Self {
            Self {
                auth: RawKeypairAuth::from_seed(seed),
                naming: HashNaming::new(),
                blobs: LocalBlobStore::new(),
                discovery: LanDiscovery::new(),
                // The comms provider needs its own IdP handle to the same key.
                comms: BuiltinProvider::new(RawKeypairAuth::from_seed(seed)),
                payments: MockPaymentRail::new(),
                input: LocalDeviceInput::new(),
            }
        }

        /// Build the default set with a fresh random node key.
        pub fn generate() -> Self {
            let seed = {
                use rand::RngCore;
                let mut s = [0u8; 32];
                rand::rngs::OsRng.fill_bytes(&mut s);
                s
            };
            Self::from_seed(seed)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            BlobStore, CommsProvider, Discovery, InputClass, InputProvider, Naming, PaymentRail,
            RoomScope,
        };

        #[tokio::test]
        async fn default_set_wires_every_seam_offline() {
            let seams = DefaultSeams::from_seed([77u8; 32]);

            // Identity/auth.
            let pk = seams.auth.node_pubkey();
            // Naming.
            assert!(seams.naming.display(&pk).starts_with("mag_"));
            // Blobs.
            let h = seams.blobs.put(b"game.wasm").await;
            assert!(seams.blobs.has(&h).await);
            // Discovery is empty but live.
            assert!(seams.discovery.find(h, Default::default()).await.is_empty());
            // Comms mints a join cred from the node key.
            let room = seams.comms.create_room(RoomScope::Lobby).await;
            let cred = seams.comms.issue_join_credential(&pk, &room).await;
            assert_eq!(cred.token.claims.issuer, pk);
            // Payments produce a verifiable receipt.
            let split = crate::PaymentSplit {
                developer: crate::Split {
                    wallet: pk,
                    amount: 500,
                },
                operator: None,
                protocol_fee_bps: 0,
            };
            let r = seams.payments.checkout(&pk, split).await;
            assert!(seams.payments.verify_receipt(&r));
            // Input: the default slot is deterministic, so the default bundle
            // keeps the replay guarantee intact.
            assert_eq!(seams.input.class(), InputClass::Deterministic);
            assert!(seams.input.class().is_replay_verifiable());
            seams.input.press(pk, 0, 1, b"start".to_vec());
            assert_eq!(seams.input.drain(0).await.len(), 1);
        }
    }
}
