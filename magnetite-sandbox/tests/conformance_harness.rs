//! Conformance-harness integration tests.
//!
//! Two modules are checked here, and the pair is the point:
//!
//! * `conformance/reference.wat` — hand-written WebAssembly, no Rust anywhere in
//!   it, no imports at all. It must pass **every** check. This is the tree's
//!   evidence that the `mag_*` ABI is satisfiable without the Rust SDK.
//! * The Rust arena-shooter template, when it has been built. Only its
//!   *framing and shape* obligations are asserted, because the shipping module
//!   has known behavioural defects (documented in `site/docs/sandbox-abi.md` §8)
//!   that this harness is what found. Asserting conformance would fail; not
//!   running it at all would lose the regression value.

use std::path::PathBuf;

use magnetite_sandbox::conformance::{self, ConformanceOptions, Status};

/// Checks that follow directly from the ABI's shape and framing. Every module
/// the sandbox will load must pass all of these regardless of its game logic.
const SHAPE_AND_FRAMING: &[&str] = &[
    "module.compiles",
    "imports.within_host_surface",
    "exports.memory",
    "exports.mag_alloc",
    "exports.mag_free",
    "exports.mag_init",
    "exports.mag_step",
    "exports.mag_snapshot",
    "exports.mag_restore",
    "exports.mag_view",
    "instance.instantiates",
    "mag_alloc.writable_in_bounds",
    "mag_init.accepts_match_config",
    "mag_step.length_prefix",
    "mag_step.payload_is_json",
    "mag_step.payload_is_step_output",
    "mag_free.accepts_returned_buffer",
    "mag_snapshot.length_prefix",
    "mag_view.length_prefix",
    "mag_step.deterministic",
    "limits.fuel_binds_module",
    "limits.epoch_binds_module",
];

fn assert_shape_and_framing(report: &conformance::Report, module: &str) {
    for id in SHAPE_AND_FRAMING {
        let check = report
            .checks
            .iter()
            .find(|c| c.id == *id)
            .unwrap_or_else(|| panic!("{module}: harness produced no `{id}` check"));
        assert_eq!(
            check.status,
            Status::Pass,
            "{module}: `{id}` is {} — {}",
            check.status.label(),
            check.detail
        );
    }
}

/// The non-Rust reference module must conform completely.
#[test]
fn hand_written_wat_module_fully_conforms() {
    let wat = include_bytes!("../conformance/reference.wat");
    let report = conformance::run(wat, &ConformanceOptions::default());
    println!("{}", report.render());
    assert_shape_and_framing(&report, "reference.wat");
    assert_eq!(
        report.count(Status::Fail),
        0,
        "reference.wat must have zero failures:\n{}",
        report.render()
    );
    assert_eq!(
        report.count(Status::Warn),
        0,
        "reference.wat must have zero warnings:\n{}",
        report.render()
    );
}

/// The Rust template, if built. Shape and framing are asserted; behaviour is
/// only reported, because it is currently defective.
#[test]
fn rust_arena_shooter_satisfies_the_framing_contract() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has a parent directory")
        .join("game-templates/authoritative/target/wasm32-wasip1/release")
        .join("game_template_authoritative.wasm");

    let Ok(bytes) = std::fs::read(&path) else {
        println!(
            "skipping: {} not built. Build it with:\n  \
             cd game-templates/authoritative && \
             cargo build --release --target wasm32-wasip1 --no-default-features --features wasm",
            path.display()
        );
        return;
    };

    let report = conformance::run(&bytes, &ConformanceOptions::default());
    println!("{}", report.render());
    assert_shape_and_framing(&report, "game_template_authoritative.wasm");
}
