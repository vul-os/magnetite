//! Proof that the §3.1 Identity/Auth and §3.2 Naming seams accept the kotva
//! providers **through unmodified consumers** — i.e. that consuming code written
//! against the traits picks these up with no edit of its own.
//!
//! This is the kotva half of the same argument
//! `magnetite-seams/tests/seam_pluggability.rs` makes for `HashNaming` /
//! `KeyNameNaming` / the in-file auth test double. It lives here rather than
//! there because `magnetite-seams` must keep a crates.io-only dependency graph
//! (see this crate's `Cargo.toml`); that file is untouched.
//!
//! One thing this crate can claim that the seams crate could not: `KotvaIdentity`
//! is a **real** second `AuthProvider` that needs no external service, because
//! `kotva-core` is a library that starts nothing. The seams-crate file says "no
//! honest shipped second auth provider exists without an external service, so
//! none is claimed" and uses a test double. That statement was accurate when
//! written and is accurate for that crate's dependency set; it is this crate that
//! supplies the real one.

use magnetite_kotva::{key_name_of, KotvaIdentity, KotvaNaming};
use magnetite_seams::{
    AuthProvider, BuiltinProvider, Challenge, CommsProvider, HashNaming, Identity, LoginResponse,
    Naming, PubKey, RoomScope,
};

/// The consumer under test, copied verbatim from
/// `magnetite-seams/tests/seam_pluggability.rs`. It is written **only** against
/// the `Naming` trait — it never names a provider, hex, or words. Every provider
/// must satisfy it the same way.
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
async fn naming_seam_accepts_the_kotva_provider_unchanged() {
    let pk = PubKey([7u8; 32]);
    let naming = KotvaNaming::new();
    naming.learn(pk);
    // Exactly the same consumer, exactly the same assertions, different provider.
    let shown = address_book_roundtrip(&naming, &pk, &key_name_of(&pk)).await;
    // kotva's shape: 8 data words + 1 checksum word.
    assert_eq!(shown.split('-').count(), 9);
}

#[tokio::test]
async fn identity_seam_accepts_the_kotva_auth_provider() {
    // Driven only through `&dyn AuthProvider` — the consumer never names the type.
    async fn login_flow(auth: &dyn AuthProvider, player: &PubKey) -> Challenge {
        auth.challenge(player).await
    }

    let auth = KotvaIdentity::from_seed([55u8; 32]);
    let player_key = KotvaIdentity::from_seed([56u8; 32]);
    let player = player_key.pubkey();

    let ch = login_flow(&auth, &player).await;
    assert_eq!(ch.subject, player);

    let resp = LoginResponse {
        client_sig: player_key.sign(&ch.signing_bytes()),
        challenge: ch,
    };
    let session = auth.verify_login(resp).await.expect("kotva login succeeds");
    assert_eq!(session.subject, player);

    // The generic comms adapter is parameterised over `A: AuthProvider` and
    // accepts a kotva-backed provider with no change to its own code — the
    // strongest available proof that the seam is not hardwired to its default.
    let comms = BuiltinProvider::new(KotvaIdentity::from_seed([55u8; 32]));
    let room = comms.create_room(RoomScope::Lobby).await;
    let cred = comms.issue_join_credential(&player, &room).await;
    assert_eq!(cred.token.claims.issuer, auth.pubkey());
    assert_eq!(cred.token.claims.subject, player);
}

#[tokio::test]
async fn both_naming_providers_agree_on_the_substrate_and_differ_on_display() {
    // `KeyNameNaming` is behind `magnetite-seams`'s `keyname` feature, which this
    // crate does not enable — enabling a dependency's feature here would change
    // what the seams crate builds as, and the point of the split is that it
    // doesn't. So this compares the two providers reachable without that flag:
    // the default and kotva's.
    let pk = PubKey([0x44u8; 32]);
    let providers: Vec<Box<dyn Naming + Send + Sync>> =
        vec![Box::new(HashNaming::new()), Box::new(KotvaNaming::new())];

    // Display strings are distinct — genuinely different encodings, not the same
    // code reached twice.
    let shown: Vec<String> = providers.iter().map(|p| p.display(&pk)).collect();
    let mut uniq = shown.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        providers.len(),
        "providers must not share a display encoding"
    );

    // ...and every one keeps the substrate address reachable.
    for p in &providers {
        assert_eq!(p.resolve(&pk.to_hex()).await, Some(pk));
    }
}
