//! Unit tests for the kotva binding. Fully offline — `kotva-core` is a library
//! that starts no service, opens no socket and needs no operator, so there is
//! nothing to stub out and no network to avoid.
//!
//! What these cover:
//!  * **§3.1** — a challenge/response round-trips; the four login checks each
//!    fail closed on their own failure mode; a kotva-minted `Token` verifies
//!    correctly through `Token::is_valid_for(&node, ...)` (the empty-domain
//!    decision that keeps `KotvaIdentity` byte-compatible with
//!    `RawKeypairAuth`, asserted rather than assumed);
//!  * **A7 cross-check** — `Token::is_valid_for` (magnetite-seams's fix: verify
//!    against whichever provider is actually passed in) genuinely follows a
//!    REAL kotva domain-separated signature, not just the synthetic double
//!    `magnetite-seams`'s own tests use — and still rejects it under a
//!    different provider over the same key, proving it is not a rubber stamp.
//!    (The crate used to also show `Token::is_valid_at` — a hard-coded,
//!    RawKeypairAuth-only path — silently rejecting the same honest token;
//!    that method has since been deleted, having had zero production callers
//!    anywhere in the tree, so this crate's docs no longer reference it.)
//!  * **key-material interchange** — one seed produces the same public key and
//!    byte-identical signatures in both providers, which is the concrete content
//!    of `ALIGNMENT.md` §3's "the identity curve already matches";
//!  * **§3.2** — key-names are deterministic and correctly shaped, and the
//!    checksum word rejects a corrupted or truncated name. **No golden name
//!    vectors**: kotva's `main` has already changed the derivation preimage after
//!    `core-v0.2.0`, so a hard-coded name would encode a version, not a fact;
//!  * **content addressing** — kotva's `ContentId` really is magnetite's
//!    `Hash` plus a `0x1e` agility prefix, and demotion fails closed on any other
//!    prefix.

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use magnetite_seams::{HashNaming, RawKeypairAuth};

/// A stand-in "player" that answers challenges with plain Ed25519 — i.e. a
/// client that knows nothing about kotva. It must still be able to log in.
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

// --- §3.1 ------------------------------------------------------------------------------------

#[test]
fn the_same_seed_is_the_same_identity_in_both_providers() {
    // The claim in ALIGNMENT.md §3 that "the identity curve already matches"
    // means exactly this: no key conversion, no re-derivation.
    let seed = [13u8; 32];
    let kotva = KotvaIdentity::from_seed(seed);
    let raw = RawKeypairAuth::from_seed(seed);
    assert_eq!(kotva.node_pubkey(), raw.node_pubkey());
    assert_eq!(kotva.seed(), raw.seed());
}

#[test]
fn seed_round_trips_so_a_node_identity_can_be_persisted() {
    let node = KotvaIdentity::generate();
    let reloaded = KotvaIdentity::from_seed(node.seed());
    assert_eq!(node.node_pubkey(), reloaded.node_pubkey());
}

#[test]
fn sign_and_verify_roundtrip() {
    let node = KotvaIdentity::from_seed([1u8; 32]);
    let msg = b"authoritative tick 42";
    let sig = node.sign(msg);
    assert!(<KotvaIdentity as Identity>::verify(
        &node.pubkey(),
        msg,
        &sig
    ));
    assert!(!<KotvaIdentity as Identity>::verify(
        &node.pubkey(),
        b"authoritative tick 43",
        &sig
    ));
}

#[test]
fn signatures_are_byte_identical_across_the_two_providers() {
    // The load-bearing compatibility fact. If this ever fails, every shared
    // signed type (`Challenge`, `TokenClaims`, `SignedAd`) forks in two.
    let seed = [21u8; 32];
    let kotva = KotvaIdentity::from_seed(seed);
    let raw = RawKeypairAuth::from_seed(seed);
    let msg = b"the bytes both providers must agree on";

    assert_eq!(kotva.sign(msg).0, raw.sign(msg).0);
    // ...and each verifies the other's signature.
    assert!(<RawKeypairAuth as Identity>::verify(
        &raw.pubkey(),
        msg,
        &kotva.sign(msg)
    ));
    assert!(<KotvaIdentity as Identity>::verify(
        &kotva.pubkey(),
        msg,
        &raw.sign(msg)
    ));
}

#[test]
fn a_domain_tag_actually_separates() {
    let node = KotvaIdentity::from_seed([22u8; 32]);
    let msg = b"receipt body";
    let sig = node.sign_domain(b"magnetite/receipt/v1", msg).unwrap();

    assert!(verify_domain(
        &node.pubkey(),
        b"magnetite/receipt/v1",
        msg,
        &sig
    ));
    // Wrong domain, and the undomained trait path, must both reject.
    assert!(!verify_domain(
        &node.pubkey(),
        b"magnetite/wager/v1",
        msg,
        &sig
    ));
    assert!(!<KotvaIdentity as Identity>::verify(
        &node.pubkey(),
        msg,
        &sig
    ));
}

/// A test-only [`Identity`] view of [`KotvaIdentity`] that uses a REAL,
/// non-empty kotva domain tag — the opposite choice from `KotvaIdentity`'s
/// own `Identity` impl, which deliberately signs with an EMPTY domain so its
/// bytes match `RawKeypairAuth`'s (see the crate docs). This type exists
/// purely to prove `magnetite_seams`'s A7 fix
/// (`IdentityVerifier`/`Token::is_valid_for`) against genuine kotva
/// domain-separated crypto, not just the synthetic double
/// `magnetite-seams`'s own tests use.
struct DomainedKotvaForTokens<'a>(&'a KotvaIdentity);

const TOKEN_TEST_DOMAIN: &[u8] = b"magnetite-kotva/token-cross-check/v1";

impl Identity for DomainedKotvaForTokens<'_> {
    fn pubkey(&self) -> PubKey {
        self.0.node_pubkey()
    }
    fn sign(&self, msg: &[u8]) -> Sig {
        self.0
            .sign_domain(TOKEN_TEST_DOMAIN, msg)
            .expect("64-byte ed25519 signature")
    }
    fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
        verify_domain(pk, TOKEN_TEST_DOMAIN, msg, sig)
    }
}

#[test]
fn is_valid_for_follows_a_genuinely_domain_separated_kotva_signature() {
    // Builds a token "issued" via real kotva domain-separated signing (NOT
    // KotvaIdentity's own empty-domain `Identity` impl), and shows:
    //  1. `is_valid_for` accepts it when given the matching domained
    //     provider — magnetite-seams's A7 fix, exercised here against real
    //     kotva crypto rather than a synthetic double;
    //  2. `is_valid_for` still rejects it under a DIFFERENT provider over
    //     the same key (the undomained `KotvaIdentity` itself), proving
    //     this is genuine per-instance dispatch, not a rubber stamp.
    // (This test used to also show the legacy `Token::is_valid_at` — hard-coded
    // to RawKeypairAuth's plain Ed25519 — silently rejecting this same honest
    // token. That method has been deleted; it had zero production callers
    // anywhere in the tree, only tests, all of which have been migrated to
    // `is_valid_for`.)
    let node = KotvaIdentity::from_seed([55u8; 32]);
    let domained = DomainedKotvaForTokens(&node);
    let now = now_unix();
    let claims = TokenClaims {
        issuer: domained.pubkey(),
        subject: PubKey([1u8; 32]),
        audience: Audience("matrix".into()),
        scope: Scope(vec!["room:join".into()]),
        issued_at: now,
        expires_at: now + 900,
        nonce: [5u8; 16],
    };
    let sig = domained.sign(&claims.signing_bytes());
    let tok = Token { claims, sig };

    assert!(
        tok.is_valid_for(&domained, now),
        "is_valid_for must accept the token under its ACTUAL issuing provider"
    );
    assert!(
        !tok.is_valid_for(&node, now),
        "the undomained KotvaIdentity is a DIFFERENT algorithm over the same \
         key and must not verify a domain-separated signature"
    );
}

#[tokio::test]
async fn challenge_response_login_round_trips() {
    let node = KotvaIdentity::from_seed([2u8; 32]);
    let player = Player::new([9u8; 32]);

    let ch = node.challenge(&player.pubkey()).await;
    let resp = LoginResponse {
        challenge: ch.clone(),
        client_sig: player.sign(&ch.signing_bytes()),
    };

    let session = node.verify_login(resp).await.expect("login ok");
    assert_eq!(session.subject, player.pubkey());
    assert_eq!(session.token.claims.issuer, node.node_pubkey());
    // `is_valid_for` dispatches through whichever provider is actually
    // passed in — here, the kotva node itself.
    assert!(session.token.is_valid_for(&node, now_unix()));
    assert!(!session
        .token
        .is_valid_for(&node, session.token.claims.expires_at));
}

#[tokio::test]
async fn replayed_challenge_is_rejected() {
    let node = KotvaIdentity::from_seed([3u8; 32]);
    let player = Player::new([8u8; 32]);
    let ch = node.challenge(&player.pubkey()).await;
    let resp = LoginResponse {
        client_sig: player.sign(&ch.signing_bytes()),
        challenge: ch,
    };
    assert!(node.verify_login(resp.clone()).await.is_ok());
    assert!(matches!(
        node.verify_login(resp).await,
        Err(SeamError::ChallengeReplayed)
    ));
}

#[tokio::test]
async fn a_challenge_from_another_authority_is_rejected() {
    let node = KotvaIdentity::from_seed([4u8; 32]);
    let attacker = KotvaIdentity::from_seed([5u8; 32]);
    let player = Player::new([7u8; 32]);
    let ch = attacker.challenge(&player.pubkey()).await;
    let resp = LoginResponse {
        client_sig: player.sign(&ch.signing_bytes()),
        challenge: ch,
    };
    assert!(matches!(
        node.verify_login(resp).await,
        Err(SeamError::UntrustedChallenge)
    ));
}

#[tokio::test]
async fn a_wrong_client_signature_is_rejected() {
    let node = KotvaIdentity::from_seed([44u8; 32]);
    let player = Player::new([45u8; 32]);
    let impostor = Player::new([46u8; 32]);
    let ch = node.challenge(&player.pubkey()).await;
    // Challenge addressed to `player`, answered by `impostor`.
    let resp = LoginResponse {
        client_sig: impostor.sign(&ch.signing_bytes()),
        challenge: ch,
    };
    assert!(matches!(
        node.verify_login(resp).await,
        Err(SeamError::InvalidSignature)
    ));
}

#[tokio::test]
async fn a_kotva_minted_token_is_audience_bound_and_tamper_evident() {
    let node = KotvaIdentity::from_seed([6u8; 32]);
    let player = Player::new([6u8; 32]);
    let tok = node
        .mint_scoped_token(
            &player.pubkey(),
            Audience("matrix".into()),
            Scope(vec!["room:join".into()]),
        )
        .await;
    assert_eq!(tok.claims.audience, Audience("matrix".into()));
    assert!(tok.is_valid_for(&node, now_unix()));

    let mut bad = tok.clone();
    bad.claims.audience = Audience("jitsi".into());
    assert!(!bad.is_valid_for(&node, now_unix()));
}

// --- §3.2 ------------------------------------------------------------------------------------

#[test]
fn a_key_name_is_deterministic_and_shaped_as_kotva_specifies() {
    let pk = PubKey([7u8; 32]);
    let a = key_name_of(&pk);
    let b = key_name_of(&pk);
    assert_eq!(a, b, "key-name derivation must be deterministic");
    // 8 data words + 1 checksum word.
    assert_eq!(a.split('-').count(), keyname::DATA_WORDS + 1);
    // Distinct keys, distinct names.
    assert_ne!(a, key_name_of(&PubKey([8u8; 32])));
    // No golden vector asserted on purpose — see the crate docs on the
    // unreleased derivation change on kotva's `main`.
}

#[test]
fn the_checksum_word_rejects_a_corrupted_name() {
    let pk = PubKey([42u8; 32]);
    let name = key_name_of(&pk);
    assert!(key_name_is_well_formed(&name));
    assert!(key_name_matches(&name, &pk));

    // Swap one data word for a different valid word: the name is still made of
    // real words and still the right length, so ONLY the checksum can catch it.
    let mut words: Vec<String> = name.split('-').map(str::to_owned).collect();
    let other = key_name_of(&PubKey([43u8; 32]));
    let replacement = other.split('-').next().unwrap().to_string();
    assert_ne!(
        words[0], replacement,
        "test needs two different first words"
    );
    words[0] = replacement;
    let typo = words.join("-");

    assert_ne!(typo, name);
    assert!(
        !key_name_is_well_formed(&typo),
        "a corrupted data word must fail the checksum"
    );
    assert!(!key_name_matches(&typo, &pk));

    // Truncation fails closed too.
    let truncated = words[..keyname::DATA_WORDS].join("-");
    assert!(!key_name_is_well_formed(&truncated));
}

#[tokio::test]
async fn resolution_is_fail_closed_and_a_learned_name_resolves() {
    let naming = KotvaNaming::new();
    let pk = PubKey([0x5Au8; 32]);
    let name = naming.display(&pk);

    // Not learned yet -> no resolution, even though the name is valid.
    assert_eq!(naming.resolve(&name).await, None);
    naming.learn(pk);
    assert_eq!(naming.resolve(&name).await, Some(pk));

    // A corrupted name never resolves to a DIFFERENT key.
    let mut words: Vec<String> = name.split('-').map(str::to_owned).collect();
    let other = key_name_of(&PubKey([0x5Bu8; 32]));
    words[0] = other.split('-').next().unwrap().to_string();
    let typo = words.join("-");
    if typo != name {
        assert_eq!(naming.resolve(&typo).await, None);
    }

    // The substrate form still addresses, and junk still does not.
    assert_eq!(naming.resolve(&naming.canonical(&pk)).await, Some(pk));
    assert_eq!(naming.resolve("not-a-key").await, None);

    // Local aliases behave like HashNaming's.
    naming.register("alice", pk);
    assert_eq!(naming.resolve("alice").await, Some(pk));
}

#[tokio::test]
async fn kotva_and_magnetite_naming_disagree_on_display_and_that_is_the_point() {
    // Both are valid `Naming` impls over the same key; swapping the provider
    // changes only the display layer, never the substrate address.
    let pk = PubKey([0x33u8; 32]);
    let kotva = KotvaNaming::new();
    let hash = HashNaming::new();
    assert_ne!(kotva.display(&pk), hash.display(&pk));
    assert_eq!(kotva.canonical(&pk), hash.canonical(&pk));
}

// --- Content addressing ----------------------------------------------------------------------

#[test]
fn kotva_content_id_is_magnetites_blake3_plus_an_agility_prefix() {
    // This is the ALIGNMENT.md §3 claim, asserted rather than assumed:
    // "game id = BLAKE3 of (wasm + manifest) — same hash, no agility prefix".
    let bytes = b"a game package: wasm + manifest";

    let mag = Hash::of(bytes); // magnetite: BLAKE3-256(bytes)
    let kot = ContentId::of(bytes); // kotva:     [0x1e] || BLAKE3-256(bytes)

    assert_eq!(kot.algorithm(), Some(kotva_core::id::MH_BLAKE3_256));
    assert_eq!(kot.digest(), &mag.0[..], "the digest is the SAME 32 bytes");
    assert_eq!(kot.as_bytes().len(), 33);
    assert_eq!(kot.as_bytes()[0], 0x1e);
    assert_eq!(&kot.as_bytes()[1..], &mag.0[..]);

    // Conversion both ways, no rehash.
    assert_eq!(content_id_of_hash(&mag), kot);
    assert_eq!(hash_from_content_id(&kot).unwrap(), mag);

    // And kotva's own verifier accepts a ContentId built from a magnetite Hash.
    assert!(content_id_of_hash(&mag).verify(bytes));
}

#[test]
fn demoting_a_non_blake3_content_id_fails_closed() {
    let mut foreign = ContentId::of(b"x");
    foreign.0[0] = 0x12; // sha2-256 in the multiformats table
    assert!(matches!(
        hash_from_content_id(&foreign),
        Err(SeamError::MalformedKey(_))
    ));
    assert!(matches!(
        hash_from_content_id(&ContentId(Vec::new())),
        Err(SeamError::MalformedKey(_))
    ));
    // Right prefix, wrong digest length.
    assert!(matches!(
        hash_from_content_id(&ContentId(vec![0x1e, 0x00, 0x01])),
        Err(SeamError::MalformedKey(_))
    ));
}
