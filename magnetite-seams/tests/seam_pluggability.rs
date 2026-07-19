//! Proof that the §3.1 Identity/Auth and §3.2 Naming seams are genuinely
//! pluggable — i.e. that consuming code is written against the *trait* and not
//! accidentally hardwired to the default implementation.
//!
//! The Comms, Discovery and BlobStore seams already have several shipped
//! implementations each, so their pluggability is demonstrated by the tree
//! itself. Naming and Identity each had exactly one, which proves nothing. This
//! file closes that gap:
//!
//! * **Naming** — the same unmodified consumer function is driven by
//!   [`HashNaming`] (default) and by [`KeyNameNaming`] (optional, feature-gated),
//!   and passes identically against both.
//! * **Identity/Auth** — a *test-double* `AuthProvider` living only in this file
//!   (see `foreign_auth`) is accepted everywhere `RawKeypairAuth` is, including
//!   by the generic `BuiltinProvider<A: AuthProvider>` comms adapter. It is a
//!   test double on purpose: no honest *shipped* second auth provider exists
//!   without an external service, so none is claimed.

#![cfg(feature = "keyname")]

use magnetite_seams::keyname::{full_name_of, short_name_of, KeyNameNaming};
use magnetite_seams::{HashNaming, Naming, PubKey};

/// The consumer under test. It is written **only** against the `Naming` trait —
/// it never names `HashNaming`, `KeyNameNaming`, hex, or words. Every provider
/// must satisfy it byte-for-byte the same way.
async fn address_book_roundtrip(naming: &dyn Naming, pk: &PubKey, resolvable_name: &str) -> String {
    // 1. A display string exists and is deterministic.
    let shown = naming.display(pk);
    assert_eq!(shown, naming.display(pk), "display must be deterministic");
    assert!(!shown.is_empty(), "display must not be empty");

    // 2. A name the provider says it can resolve maps back to the same key.
    assert_eq!(
        naming.resolve(resolvable_name).await.as_ref(),
        Some(pk),
        "provider must resolve the name it published"
    );

    // 3. Junk fails closed — no panic, no guess, no partial match.
    for junk in ["", "   ", "definitely-not-a-name", "@", "a@b@c"] {
        assert_eq!(naming.resolve(junk).await, None, "junk resolved: {junk:?}");
    }

    // 4. Distinct keys never share a display string.
    let other = PubKey([0xA5; 32]);
    assert_ne!(naming.display(pk), naming.display(&other));

    shown
}

#[tokio::test]
async fn naming_seam_accepts_the_default_provider() {
    let pk = PubKey([7u8; 32]);
    let naming = HashNaming::new();
    naming.register("alice", pk);
    let shown = address_book_roundtrip(&naming, &pk, "alice").await;
    assert!(shown.starts_with("mag_"));
}

#[tokio::test]
async fn naming_seam_accepts_the_keyname_provider_unchanged() {
    let pk = PubKey([7u8; 32]);
    let naming = KeyNameNaming::new();
    naming.learn(pk);
    // Exactly the same consumer, exactly the same assertions, different provider.
    let shown = address_book_roundtrip(&naming, &pk, &short_name_of(&pk)).await;
    assert_eq!(shown.split('-').count(), 8);
    // …and the lossless form resolves through the trait with no directory at all.
    let fresh = KeyNameNaming::new();
    assert_eq!(fresh.resolve(&full_name_of(&pk)).await, Some(pk));
}

#[tokio::test]
async fn a_provider_set_can_swap_its_naming_provider() {
    // Stand-in for a wired provider bundle: the field is `Box<dyn Naming>`, so
    // swapping the implementation is a one-line change with no consumer edits.
    struct Seams {
        naming: Box<dyn Naming + Send + Sync>,
    }
    let pk = PubKey([0x33; 32]);

    let default_set = Seams {
        naming: Box::new({
            let n = HashNaming::new();
            n.register("bob", pk);
            n
        }),
    };
    let keyname_set = Seams {
        naming: Box::new({
            let n = KeyNameNaming::new();
            n.learn(pk);
            n
        }),
    };

    for (set, name) in [
        (&default_set, "bob".to_string()),
        (&keyname_set, short_name_of(&pk)),
    ] {
        assert_eq!(set.naming.resolve(&name).await, Some(pk));
        assert!(!set.naming.display(&pk).is_empty());
    }

    // The two providers really are different code, not the same thing twice.
    assert_ne!(default_set.naming.display(&pk), keyname_set.naming.display(&pk));

    // Both keep the substrate reachable: raw hex resolves through either.
    assert_eq!(default_set.naming.resolve(&pk.to_hex()).await, Some(pk));
    assert_eq!(keyname_set.naming.resolve(&pk.to_hex()).await, Some(pk));
}

/// Identity/Auth seam: a foreign `AuthProvider` implementation that is NOT
/// `RawKeypairAuth`, defined here to show the seam accepts one.
///
/// It is deliberately a **test double**, not a shipped provider: it issues
/// non-expiring challenges from a counter and mints tokens signed by its own
/// key. Real alternatives (e.g. an OIDC bridge) need code or services
/// that are not present, and inventing one would be fiction.
mod foreign_auth {
    use magnetite_seams::{
        Audience, AuthProvider, Challenge, Identity, LoginResponse, PubKey, RawKeypairAuth, Scope,
        SeamError, Session, Sig, Token,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    pub struct CounterAuth {
        inner: RawKeypairAuth,
        counter: AtomicU64,
    }

    impl CounterAuth {
        pub fn new(seed: [u8; 32]) -> Self {
            Self {
                inner: RawKeypairAuth::from_seed(seed),
                counter: AtomicU64::new(0),
            }
        }
        pub fn issued(&self) -> u64 {
            self.counter.load(Ordering::SeqCst)
        }
        pub fn node_pubkey(&self) -> PubKey {
            self.inner.node_pubkey()
        }
    }

    impl Identity for CounterAuth {
        fn pubkey(&self) -> PubKey {
            self.inner.pubkey()
        }
        fn sign(&self, msg: &[u8]) -> Sig {
            self.inner.sign(msg)
        }
        fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
            <RawKeypairAuth as Identity>::verify(pk, msg, sig)
        }
    }

    #[async_trait::async_trait]
    impl AuthProvider for CounterAuth {
        async fn challenge(&self, pk: &PubKey) -> Challenge {
            self.counter.fetch_add(1, Ordering::SeqCst);
            self.inner.challenge(pk).await
        }
        async fn verify_login(&self, resp: LoginResponse) -> Result<Session, SeamError> {
            self.inner.verify_login(resp).await
        }
        async fn mint_scoped_token(&self, pk: &PubKey, aud: Audience, scope: Scope) -> Token {
            self.inner.mint_scoped_token(pk, aud, scope).await
        }
    }
}

#[tokio::test]
async fn identity_seam_accepts_a_foreign_auth_provider() {
    use foreign_auth::CounterAuth;
    use magnetite_seams::{
        AuthProvider, BuiltinProvider, CommsProvider, PubKey, RoomScope,
    };

    async fn login_flow(auth: &dyn AuthProvider, player: &PubKey) -> magnetite_seams::Challenge {
        auth.challenge(player).await
    }

    let auth = CounterAuth::new([42u8; 32]);
    let player = PubKey([9u8; 32]);
    let ch = login_flow(&auth, &player).await;
    assert_eq!(ch.subject, player);
    assert_eq!(auth.issued(), 1, "the foreign provider really served the call");

    // The generic comms adapter is parameterised over `A: AuthProvider`, so it
    // accepts the foreign provider with no change to its own code.
    let comms = BuiltinProvider::new(CounterAuth::new([42u8; 32]));
    let room = comms.create_room(RoomScope::Lobby).await;
    let cred = comms.issue_join_credential(&player, &room).await;
    assert_eq!(cred.token.claims.issuer, auth.node_pubkey());
    assert_eq!(cred.token.claims.subject, player);
}
