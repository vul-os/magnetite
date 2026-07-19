//! Keep the JS client's cross-language fixture honest.
//!
//! `magnetite-web-client/src/__fixtures__/redirect.json` holds a real signed
//! redirect that the web client verifies with WebCrypto. If this crate's
//! canonical signing bytes ever change without the fixture (and the JS
//! encoder) being regenerated, the JS client would start refusing legitimate
//! redirects in production — and its own tests would happily keep passing on
//! stale data. This test fails first.

use magnetite_runtime::cluster::SignedRedirect;
use magnetite_seams::identity::{Identity, PubKey, RawKeypairAuth};

const FIXTURE: &str = include_str!("../../magnetite-web-client/src/__fixtures__/redirect.json");

#[test]
fn the_js_client_fixture_still_verifies_under_this_crate() {
    let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture is valid JSON");
    let r: SignedRedirect =
        serde_json::from_value(v["redirect"].clone()).expect("fixture parses as a SignedRedirect");
    let issuer = PubKey::from_hex(v["issuer_key"].as_str().unwrap()).unwrap();

    // Verified at a moment inside its validity window.
    let now = r.issued_at + 1;
    let route = r
        .verify_for(&issuer, r.player, now)
        .expect("the committed fixture must still verify — regenerate it if the wire format moved");
    assert_eq!(route.pubkey.to_hex(), v["target_key"].as_str().unwrap());
    assert_eq!(route.addr, "10.0.0.11:7100");

    // And it is genuinely time-bound, which is what the JS `expired` test leans on.
    assert!(r.verify_for(&issuer, r.player, r.expires_at + 1).is_err());
}

#[test]
fn the_fixture_node_hello_proof_still_verifies() {
    let v: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let hello = &v["hello"];
    let key = PubKey::from_hex(hello["node_key"].as_str().unwrap()).unwrap();
    let raw = (0..128)
        .step_by(2)
        .map(|i| u8::from_str_radix(&hello["sig"].as_str().unwrap()[i..i + 2], 16).unwrap())
        .collect::<Vec<u8>>();
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&raw);

    assert!(<RawKeypairAuth as Identity>::verify(
        &key,
        &magnetite_runtime::follow::node_hello_bytes(hello["nonce"].as_str().unwrap(), &key),
        &magnetite_seams::identity::Sig(sig),
    ));
}
