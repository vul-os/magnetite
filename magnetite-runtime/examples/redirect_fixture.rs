//! Emit a real signed redirect as JSON, for the JS client's cross-language
//! tests.
//!
//! The web client verifies redirect signatures itself, which is only worth
//! anything if its canonical-bytes encoding matches this crate's byte for byte.
//! Rather than assert that by eye, we generate a genuine redirect here with
//! fixed seeds and let `magnetite-web-client/src/follow.test.js` verify it.
//!
//! ```sh
//! cargo run --example redirect_fixture > ../magnetite-web-client/src/__fixtures__/redirect.json
//! ```
//!
//! `magnetite-runtime/tests/redirect_fixture.rs` keeps the committed file
//! honest: it fails if the fixture stops verifying under this crate's own
//! verifier.

use std::sync::Arc;

use magnetite_runtime::cluster::SignedRedirect;
use magnetite_runtime::fleet::PeerRoute;
use magnetite_runtime::shard::ShardId;
use magnetite_seams::identity::RawKeypairAuth;

/// Fixed so the fixture is reproducible.
pub const ISSUER_SEED: u8 = 11;
pub const TARGET_SEED: u8 = 22;
pub const PLAYER: u64 = 7;
pub const SHARD: u32 = 3;
pub const EPOCH: u64 = 5;
pub const ISSUED_AT: u64 = 1_800_000_000;
pub const TTL: u64 = 30;
pub const ADDR: &str = "10.0.0.11:7100";

fn main() {
    let issuer = Arc::new(RawKeypairAuth::from_seed([ISSUER_SEED; 32]));
    let target = RawKeypairAuth::from_seed([TARGET_SEED; 32]);
    let route = PeerRoute::new(ADDR, target.node_pubkey());
    let r = SignedRedirect::mint(
        &issuer,
        PLAYER,
        ShardId(SHARD),
        EPOCH,
        &route,
        ISSUED_AT,
        TTL,
    );
    let hello_sig = <RawKeypairAuth as magnetite_seams::identity::Identity>::sign(
        &target,
        &magnetite_runtime::follow::node_hello_bytes("fixture-nonce", &target.node_pubkey()),
    );
    let hello_sig_hex = hello_sig
        .0
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    let out = serde_json::json!({
        "issuer_key": issuer.node_pubkey().to_hex(),
        "target_key": target.node_pubkey().to_hex(),
        "player": PLAYER,
        "issued_at": ISSUED_AT,
        "expires_at": ISSUED_AT + TTL,
        "redirect": r,
        // A node-identity proof over a fixed nonce, for the key-pinning half of
        // the client's checks.
        "hello": {
            "nonce": "fixture-nonce",
            "node_key": target.node_pubkey().to_hex(),
            "sig": hello_sig_hex,
        },
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
