//! Cross-crate tests for the package format (`ALIGNMENT.md` §7 Phase 1.2).
//!
//! These run **outside** `magnetite-seams`, so they exercise only the public
//! API — the same surface `magnetite-cli`, a node, and a storefront see. Two
//! things are being pinned here that the in-module unit tests cannot pin:
//!
//! 1. **The API is usable from outside.** Building, signing and verifying a
//!    package must be possible without touching a private item.
//! 2. **The wire bytes are frozen.** The golden vector below is the literal
//!    canonical CBOR of a known package. A refactor that changes the encoding —
//!    a field renumbered, a map reordered, an optional encoded as `null`, a
//!    length written non-shortest-form — breaks this test rather than silently
//!    invalidating every signature ever issued.
//!
//! Fully offline: no network, no chain, no temp files, no clock.

use magnetite_seams::identity::{Identity, RawKeypairAuth};
use magnetite_seams::package::{
    DeterminismClass, FileEntry, MemoryFiles, Package, PackageError, PackageKind, PackageManifest,
    PackagePrice, PriceModel, SplitLeg, SplitPlan,
};
// `Role` is the payment seam's type, shared by both split layers — see
// `ALIGNMENT.md` §4 and the note at the top of `package.rs`.
use magnetite_seams::{Hash, PubKey, Role};

/// A fixed developer key, so every byte below is reproducible.
fn dev() -> RawKeypairAuth {
    RawKeypairAuth::from_seed([0x11; 32])
}

/// A three-file synthetic web bundle plus its file entries.
fn bundle() -> (MemoryFiles, Vec<FileEntry>) {
    let files: Vec<(&str, &[u8])> = vec![
        ("index.html", b"<!doctype html><canvas id=c>"),
        ("main.js", b"const c=document.getElementById('c')"),
        ("assets/atlas.png", b"\x89PNG\r\n\x1a\n"),
    ];
    let mut mem = MemoryFiles::new();
    let mut entries = Vec::new();
    for (p, b) in files {
        mem.insert(p, b.to_vec());
        entries.push(FileEntry {
            path: p.to_string(),
            hash: Hash::of(b),
            size: b.len() as u64,
        });
    }
    (mem, entries)
}

// ---------------------------------------------------------------------------
// The frozen wire format
// ---------------------------------------------------------------------------

/// The literal canonical CBOR of a `kind: wasm` package built from the fixed
/// key above, priced `pwyw:250:900` in `USDC`, with the whole split going to
/// the developer.
///
/// **Do not "fix" this by regenerating it.** If the assertion below fails, the
/// encoding changed — and every package id and signature ever issued under the
/// old encoding just became unverifiable. Either revert the change, or bump
/// `PACKAGE_FORMAT_V1` and add a second vector alongside this one.
///
/// Annotated (`‖` = concatenation, indentation = nesting):
///
/// ```text
/// a2                              map(2) — the signed envelope
///   01 a9                           key 1 = manifest, map(9)
///     01 01                           format = 1
///     02 02                           kind = 2 (wasm)
///     04 69 "game.wasm"               wasm_entry (key 3, web_entry, is ABSENT — not null)
///     05 81 83                        files = array(1) of array(3)
///          69 "game.wasm"               path
///          5820 0d66d4…                 BLAKE3(module) — the module's own content address
///          08                           size = 8
///     06 5820 86b46a…                 root over the sorted (path, hash) list
///     07 a4                           price, map(4)
///       01 03                           model = 3 (pwyw)
///       02 18 fa                        min = 250
///       03 19 0384                      suggested = 900
///       04 64 "USDC"                    currency
///     08 81 a3                        split = array(1) of map(3)
///       01 5820 d04ab2…                 wallet
///       02 19 2710                      share_bps = 10000
///       03 01                           role = 1 (developer)
///     09 02                           determinism = 2 (deterministic)
///     0a 5820 d04ab2…                 developer pubkey — bound INSIDE the signed bytes
///   02 5840 81a8da…                  key 2 = Ed25519 signature (64 bytes)
/// ```
///
/// Note what is *not* there: no text keys, no floats, no `null` for the absent
/// `web_entry`, no non-shortest-form lengths, and keys in ascending order.
const GOLDEN_WASM_PACKAGE_CBOR: &str = concat!(
    "a201a901010202046967616d652e7761736d0581836967616d652e7761736d5820",
    "0d66d411a21e80d93afa1487b002a1867432c04ba1d08dc790d259639f6a69c408",
    "06582086b46a08ae8026126f304e25f5d631b04e3ef6eba42941ec7735d3ca039f",
    "1cd507a401030218fa031903840464555344430881a3015820d04ab232742bb4ab",
    "3a1368bd4615e4e6d0224ab71a016baf8520a332c977873702192710030109020a",
    "5820d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c97787",
    "3702584081a8da036f61cc41fe9c8e1f0056b07ea433e6ed878e9aec22bb985145",
    "74e2ea4ba64c050a9cfca4e598e70064a74da9d9392e03fa17871fd1f4d543119b",
    "7b0d",
);

/// The root hash of that package: `BLAKE3(ROOT_DOMAIN ‖ cbor([[path, hash]]))`.
const GOLDEN_ROOT: &str = "86b46a08ae8026126f304e25f5d631b04e3ef6eba42941ec7735d3ca039f1cd5";
/// Its package id: `BLAKE3(ID_DOMAIN ‖ developer ‖ manifest signing bytes)`.
const GOLDEN_ID: &str = "cd767defdb77a600cde5aad24d3707369e9adbacb0cf419a36ccd071a550ca01";

fn golden_package() -> Package {
    let key = dev();
    let pk = key.node_pubkey();
    let wasm = b"\x00asm\x01\x00\x00\x00".to_vec();
    PackageManifest::wasm_only(
        "game.wasm",
        &wasm,
        PackagePrice::pwyw(250, 900, "USDC"),
        SplitPlan::all_to(pk, Role::Developer),
        DeterminismClass::Deterministic,
        pk,
    )
    .sign(&key)
    .unwrap()
}

#[test]
fn the_wire_encoding_is_frozen() {
    let pkg = golden_package();
    assert_eq!(
        hex_of(&pkg.to_canonical_cbor()),
        GOLDEN_WASM_PACKAGE_CBOR,
        "the canonical encoding changed — read the note on GOLDEN_WASM_PACKAGE_CBOR \
         before touching this assertion"
    );
    assert_eq!(pkg.manifest.root.to_hex(), GOLDEN_ROOT);
    assert_eq!(pkg.id().to_hex(), GOLDEN_ID);
    // The kotva-style content address is the same digest behind the multihash
    // BLAKE3 agility prefix — a re-wrapping, not a re-hashing.
    assert_eq!(pkg.content_id()[0], 0x1e);
    assert_eq!(&pkg.content_id()[1..], &pkg.id().0[..]);
}

#[test]
fn the_frozen_bytes_still_parse_and_verify() {
    let bytes = unhex(GOLDEN_WASM_PACKAGE_CBOR);
    let pkg = Package::from_canonical_cbor(&bytes).expect("the frozen vector must parse");
    let v = pkg.verify().expect("the frozen vector must verify");
    assert_eq!(v.id().to_hex(), GOLDEN_ID);
    assert_eq!(pkg, golden_package());
    // Re-encoding an accepted package reproduces the input byte-for-byte. This
    // is the non-malleability property the signature depends on.
    assert_eq!(pkg.to_canonical_cbor(), bytes);
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Public-API round trips
// ---------------------------------------------------------------------------

#[test]
fn a_web_package_is_buildable_signable_and_verifiable_from_outside_the_crate() {
    let key = dev();
    let pk = key.node_pubkey();
    let (mem, entries) = bundle();

    let pkg = PackageManifest::web(
        "index.html",
        entries,
        PackagePrice::pwyw(0, 500, "USDC"),
        SplitPlan::all_to(pk, Role::Developer),
        pk,
    )
    .sign(&key)
    .expect("a well-formed web package signs");

    let v = pkg
        .verify_with_contents(&mem)
        .expect("a freshly built package verifies");

    assert_eq!(v.manifest().kind, PackageKind::Web);
    assert_eq!(v.manifest().entry(), "index.html");
    assert_eq!(v.manifest().files.len(), 3);
    assert_eq!(
        v.manifest().determinism,
        DeterminismClass::NonDeterministic,
        "a web bundle is labelled non-deterministic"
    );
    assert!(
        !v.is_replay_verifiable(),
        "rung 0 does not inherit rung 2's replay guarantee"
    );

    // The bytes survive a round trip through the canonical encoding unchanged.
    let bytes = pkg.to_canonical_cbor();
    let back = Package::from_canonical_cbor(&bytes).unwrap();
    assert_eq!(back.to_canonical_cbor(), bytes);
    assert_eq!(back.id(), pkg.id());
    back.verify_with_contents(&mem).unwrap();
}

#[test]
fn a_wasm_package_keeps_the_modules_existing_content_address() {
    let key = dev();
    let pk = key.node_pubkey();
    let wasm = b"\x00asm\x01\x00\x00\x00 an already-published module".to_vec();

    // What `magnetite_runtime::node::content_address` computes today.
    let existing_game_id = Hash::of(&wasm);

    let pkg = PackageManifest::wasm_only(
        "game.wasm",
        &wasm,
        PackagePrice::free("USDC"),
        SplitPlan::all_to(pk, Role::Developer),
        DeterminismClass::Deterministic,
        pk,
    )
    .sign(&key)
    .unwrap();

    let v = pkg.verify().unwrap();
    assert_eq!(
        v.manifest().legacy_game_id(),
        Some(existing_game_id),
        "wrapping an existing wasm game in a package must not change its id"
    );
    assert!(
        v.is_replay_verifiable(),
        "a wasm authority may claim determinism"
    );
    assert_ne!(
        v.id(),
        existing_game_id,
        "the package id is an additional identifier, not a replacement"
    );
}

// ---------------------------------------------------------------------------
// Fail-closed behaviour, from outside
// ---------------------------------------------------------------------------

#[test]
fn every_tamper_is_refused() {
    let key = dev();
    let pk = key.node_pubkey();
    let (mem, entries) = bundle();
    let pkg = PackageManifest::web(
        "index.html",
        entries,
        PackagePrice::fixed(500, "USDC"),
        SplitPlan::all_to(pk, Role::Developer),
        pk,
    )
    .sign(&key)
    .unwrap();
    pkg.verify_with_contents(&mem).unwrap();

    // 1. A same-length content change.
    let mut tampered = mem.clone();
    tampered.insert("main.js", b"const c=document.getElementById('X')".to_vec());
    assert!(matches!(
        pkg.verify_with_contents(&tampered),
        Err(PackageError::FileHash { .. })
    ));

    // 2. Truncation.
    let mut truncated = mem.clone();
    truncated.insert("assets/atlas.png", b"\x89PNG".to_vec());
    assert!(matches!(
        pkg.verify_with_contents(&truncated),
        Err(PackageError::FileSize { .. })
    ));

    // 3. A missing file.
    let mut missing = mem.clone();
    missing.0.remove("index.html");
    assert!(matches!(
        pkg.verify_with_contents(&missing),
        Err(PackageError::FileMissing(_))
    ));

    // 4. A flipped signature bit.
    let mut bad_sig = pkg.clone();
    bad_sig.sig.0[0] ^= 0x01;
    assert!(matches!(bad_sig.verify(), Err(PackageError::BadSignature)));

    // 5. A price changed after signing.
    let mut repriced = pkg.clone();
    repriced.manifest.price = PackagePrice::fixed(1, "USDC");
    assert!(matches!(repriced.verify(), Err(PackageError::BadSignature)));

    // 6. Truncated package bytes.
    let bytes = pkg.to_canonical_cbor();
    assert!(Package::from_canonical_cbor(&bytes[..bytes.len() / 2]).is_err());
}

#[test]
fn a_web_package_cannot_claim_to_be_replay_verifiable() {
    let key = dev();
    let pk = key.node_pubkey();
    let (_, entries) = bundle();
    let mut m = PackageManifest::web(
        "index.html",
        entries,
        PackagePrice::free("USDC"),
        SplitPlan::all_to(pk, Role::Developer),
        pk,
    );
    m.determinism = DeterminismClass::Deterministic;

    // Refused at validation, at signing, and at verification of a hand-signed
    // package — three independent places, so there is no route in.
    assert!(matches!(
        m.validate(),
        Err(PackageError::DeterminismClaimWithoutAuthority("web"))
    ));
    let sig = key.sign(&m.signing_bytes());
    let forged = Package {
        manifest: m.clone(),
        sig,
    };
    assert!(matches!(
        forged.verify(),
        Err(PackageError::DeterminismClaimWithoutAuthority("web"))
    ));
    assert!(m.sign(&key).is_err());
}

#[test]
fn sorting_is_load_bearing_but_not_order_sensitive() {
    let (_, entries) = bundle();
    let mut reversed = entries.clone();
    reversed.reverse();

    assert_eq!(
        PackageManifest::compute_root(&entries),
        PackageManifest::compute_root(&reversed),
        "the root addresses a SET of files: reordering must not change it"
    );

    let key = dev();
    let pk = key.node_pubkey();
    let a = PackageManifest::web(
        "index.html",
        entries,
        PackagePrice::free("USDC"),
        SplitPlan::all_to(pk, Role::Developer),
        pk,
    )
    .sign(&key)
    .unwrap();
    let b = PackageManifest::web(
        "index.html",
        reversed,
        PackagePrice::free("USDC"),
        SplitPlan::all_to(pk, Role::Developer),
        pk,
    )
    .sign(&key)
    .unwrap();
    assert_eq!(a.id(), b.id());
    assert_eq!(a.to_canonical_cbor(), b.to_canonical_cbor());
}

#[test]
fn price_models_and_split_legs_round_trip_through_the_wire() {
    let key = dev();
    let pk = key.node_pubkey();
    let split = SplitPlan {
        legs: vec![
            SplitLeg {
                wallet: pk,
                share_bps: 8_000,
                role: Role::Developer,
            },
            SplitLeg {
                wallet: PubKey([0x0B; 32]),
                share_bps: 1_500,
                role: Role::Operator,
            },
            SplitLeg {
                wallet: PubKey([0x5E; 32]),
                share_bps: 500,
                role: Role::Other("charity".into()),
            },
        ],
    };

    for price in [
        PackagePrice::free("USDC"),
        PackagePrice::fixed(1_999, "USDC"),
        PackagePrice::pwyw(0, 500, "USDC"),
        PackagePrice::pwyw(250, 900, "SOL"),
    ] {
        let wasm = b"\x00asm\x01\x00\x00\x00".to_vec();
        let pkg = PackageManifest::wasm_only(
            "game.wasm",
            &wasm,
            price.clone(),
            split.clone(),
            DeterminismClass::Deterministic,
            pk,
        )
        .sign(&key)
        .unwrap();
        let back = Package::from_canonical_cbor(&pkg.to_canonical_cbor()).unwrap();
        back.verify().unwrap();
        assert_eq!(back.manifest.price, price);
        assert_eq!(back.manifest.split, split);
    }

    // A pwyw minimum is enforced, and the split resolves sum-exact.
    let p = PackagePrice::pwyw(250, 900, "USDC");
    assert!(!p.admits(249));
    assert!(p.admits(250));
    assert!(matches!(p.model, PriceModel::Pwyw { min: 250, .. }));
    for total in [0u64, 1, 3, 999, 1_000, 1_000_003] {
        let sum: u64 = split.resolve(total).iter().map(|l| l.amount).sum();
        assert_eq!(sum, total);
    }
}
