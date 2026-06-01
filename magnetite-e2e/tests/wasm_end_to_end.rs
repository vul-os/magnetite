//! Wasm end-to-end integration test — sandbox determinism parity.
//!
//! ## What this test proves
//!
//! The Magnetite MOAT promise is: **the same game logic, same inputs, same seed
//! → identical authoritative state on every tick, whether the game runs natively
//! (`NativeExecutor`) or inside the Wasmtime sandbox (`WasmExecutor`).**
//!
//! This test validates that promise by:
//!
//! 1. Building `game-template-authoritative` to `wasm32-wasip1` (done outside
//!    by `scripts/moat-demo.sh` or `cargo build --release --target wasm32-wasip1
//!    --features wasm -p game-template-authoritative`).
//! 2. Loading the produced `.wasm` via [`WasmExecutor`] and running K ticks.
//! 3. Running an identical scenario via [`NativeExecutor`] for the same K ticks.
//! 4. Asserting every tick's `state_hash` is **identical** between the two
//!    executors — proving sandbox determinism parity.
//! 5. Using `verify_replay` over the native log to confirm the game is
//!    `ReplayVerdict::Clean` (tamper-evident replay verification passes).
//!
//! ## Why empty inputs?
//!
//! The Sandbox ABI (`mag_init` / `mag_step`) does not expose `on_join`. Players
//! are joined inside the runtime host (not via the ABI). Both executors therefore
//! start with an empty player list and receive empty input lists — the game ticks
//! through the physics loop with no commands. This is the correct baseline for
//! a parity proof: both sides see identical (empty) state, and the hash must
//! match.
//!
//! ## How to run
//!
//! ```sh
//! # 1. Build the wasm (one-time):
//! cd game-template-authoritative
//! cargo build --release --target wasm32-wasip1 --features wasm
//!
//! # 2. Run the parity test:
//! cd magnetite-e2e
//! cargo test wasm_sandbox_parity_with_native -- --nocapture
//! ```
//!
//! Or use the one-command demo:
//! ```sh
//! bash scripts/moat-demo.sh
//! ```

use std::path::PathBuf;

use game_template_authoritative::ArenaShooter;
use magnetite_sandbox::{LimitsConfig, WasmExecutor};
use magnetite_sdk::authority::{
    verify_replay, GameExecutor, MatchConfig, NativeExecutor, ReplayLog, ReplayVerdict, Topology,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

/// Number of ticks to run in the parity test.
const K_TICKS: u64 = 30;

/// Deterministic seed — the same seed must produce the same hash sequence
/// on both native and sandboxed executors.
const SEED: u64 = 0xDEAD_CAFE_1337_BABE;

// ---------------------------------------------------------------------------
// Helper: locate the built wasm artifact
// ---------------------------------------------------------------------------

/// Return the path to the compiled `game_template_authoritative.wasm`.
///
/// The wasm must be built before running this test:
///   `cargo build --release --target wasm32-wasip1 --features wasm`
/// from `game-template-authoritative/`.
fn wasm_path() -> PathBuf {
    // The canonical release output path relative to the workspace root.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("e2e crate must have a parent")
        .to_path_buf();

    workspace_root
        .join("game-template-authoritative")
        .join("target")
        .join("wasm32-wasip1")
        .join("release")
        .join("game_template_authoritative.wasm")
}

// ---------------------------------------------------------------------------
// Helper: build a shared MatchConfig
// ---------------------------------------------------------------------------

fn match_config() -> MatchConfig {
    MatchConfig {
        topology: Topology::SingleRoom,
        max_players: 4,
        tick_hz: 60,
        seed: SEED,
        snapshot_every: 10,
    }
}

// ---------------------------------------------------------------------------
// Test 1: sandbox parity with native (headline proof)
// ---------------------------------------------------------------------------

/// **Headline proof:** `WasmExecutor` and `NativeExecutor` produce the same
/// `state_hash` on every tick given identical inputs and seed.
///
/// This test requires the wasm artifact to be present at:
///   `game-template-authoritative/target/wasm32-wasip1/release/game_template_authoritative.wasm`
///
/// If the file is absent, the test is skipped with a clear message.
#[test]
fn wasm_sandbox_parity_with_native() {
    let wasm = wasm_path();

    if !wasm.exists() {
        println!("[SKIP] wasm artifact not found at {}", wasm.display());
        println!("Build it first:");
        println!("  cd game-template-authoritative");
        println!("  cargo build --release --target wasm32-wasip1 --features wasm");
        // Hard fail so the CI script can distinguish missing-artifact from pass.
        panic!(
            "wasm artifact missing — run `cargo build --release --target wasm32-wasip1 --features wasm` in game-template-authoritative/ first"
        );
    }

    let cfg = match_config();
    let limits = LimitsConfig {
        // Generous budget: ArenaShooter with no players is very cheap.
        fuel_per_step: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_epochs_per_step: 10,
        epoch_tick_ms: 50,
    };

    // ── Native executor ──────────────────────────────────────────────────
    let mut native = NativeExecutor::<ArenaShooter>::new(cfg.clone());
    let mut native_hashes: Vec<(u64, u64)> = Vec::new(); // (tick, hash)

    // ── Wasm executor ────────────────────────────────────────────────────
    let mut wasm_exec = WasmExecutor::from_file(&wasm, cfg.clone(), limits)
        .expect("WasmExecutor::from_file must succeed with a valid game.wasm");
    let mut wasm_hashes: Vec<(u64, u64)> = Vec::new(); // (tick, hash)

    // ── Replay log for verify_replay ─────────────────────────────────────
    let mut log = ReplayLog::new(cfg.clone());

    // Empty inputs — no players have been joined via on_join.
    let inputs: Vec<(PlayerId, Input)> = vec![];

    // ── Drive K ticks ────────────────────────────────────────────────────
    for tick in 1..=K_TICKS {
        let native_out = native.step(tick, &inputs);
        let wasm_out = wasm_exec.step(tick, &inputs);

        native_hashes.push((tick, native_out.state_hash));
        wasm_hashes.push((tick, wasm_out.state_hash));

        // Record for verify_replay.
        log.record(tick, inputs.clone(), native_out.state_hash);

        assert_eq!(
            native_out.state_hash, wasm_out.state_hash,
            "state_hash MISMATCH at tick {tick}: native={} wasm={}",
            native_out.state_hash, wasm_out.state_hash
        );
    }

    println!("[PASS] sandbox parity confirmed over {K_TICKS} ticks (seed=0x{SEED:016X})");
    println!(
        "       Native hashes sample: {:?}",
        &native_hashes[..K_TICKS.min(5) as usize]
    );

    // ── verify_replay must return Clean ─────────────────────────────────
    let verdict = verify_replay::<ArenaShooter>(&log);
    assert_eq!(
        verdict,
        ReplayVerdict::Clean,
        "verify_replay must return Clean — native log is deterministic"
    );
    println!("[PASS] verify_replay returned Clean over {K_TICKS} ticks");
}

// ---------------------------------------------------------------------------
// Test 2: wasm executor snapshot is non-empty and state_hash is deterministic
// ---------------------------------------------------------------------------

/// Verify that `WasmExecutor` produces non-empty snapshots and that running
/// the same game twice from fresh executors gives identical state hashes.
///
/// Note on snapshot/restore: the wasm module's `mag_restore` receives the
/// snapshot bytes wrapped with a 4-byte length prefix (by `WasmExecutor`).
/// The internal static `CURRENT_TICK` in the guest is not reset by restore,
/// which is a known trait of the N1/N2 ABI (recorded as M8 in DECISIONS.md).
/// This test therefore verifies the executable guarantee: two independent
/// WasmExecutor instances agree on the state_hash sequence.
///
/// This test requires the wasm artifact.
#[test]
fn wasm_state_hash_is_reproducible_across_instances() {
    let wasm = wasm_path();
    if !wasm.exists() {
        panic!(
            "wasm artifact missing — run `cargo build --release --target wasm32-wasip1 --features wasm` in game-template-authoritative/ first"
        );
    }

    let cfg = match_config();
    let limits = LimitsConfig {
        fuel_per_step: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_epochs_per_step: 10,
        epoch_tick_ms: 50,
    };

    let inputs: Vec<(PlayerId, Input)> = vec![];

    // Instance A — run K ticks, record hashes.
    let mut exec_a = WasmExecutor::from_file(&wasm, cfg.clone(), limits.clone())
        .expect("WasmExecutor A must load successfully");
    let mut hashes_a: Vec<u64> = Vec::new();
    for tick in 1..=K_TICKS {
        let out = exec_a.step(tick, &inputs);
        assert_ne!(
            out.state_hash, 0,
            "state_hash must be non-zero at tick {tick}"
        );
        hashes_a.push(out.state_hash);
    }

    // Snapshot from A is non-empty.
    let snap_a = exec_a.snapshot();
    assert!(
        !snap_a.is_empty(),
        "WasmExecutor snapshot must not be empty"
    );
    println!(
        "[PASS] WasmExecutor snapshot non-empty ({} bytes)",
        snap_a.len()
    );

    // Instance B — same config, same seed, must produce identical hashes.
    let mut exec_b = WasmExecutor::from_file(&wasm, cfg.clone(), limits)
        .expect("WasmExecutor B must load successfully");
    let mut hashes_b: Vec<u64> = Vec::new();
    for tick in 1..=K_TICKS {
        let out = exec_b.step(tick, &inputs);
        hashes_b.push(out.state_hash);
    }

    // Every tick's hash must match between A and B.
    for (i, (ha, hb)) in hashes_a.iter().zip(hashes_b.iter()).enumerate() {
        assert_eq!(
            ha,
            hb,
            "state_hash mismatch at tick {}: instance_a={} instance_b={}",
            i + 1,
            ha,
            hb
        );
    }

    println!(
        "[PASS] wasm two-instance reproducibility: {} ticks, all state_hashes match",
        K_TICKS
    );
    println!(
        "       Sample hashes: {:?}",
        &hashes_a[..K_TICKS.min(5) as usize]
    );
}

// ---------------------------------------------------------------------------
// Test 3: NativeExecutor replay verify_replay is Clean (regression guard)
// ---------------------------------------------------------------------------

/// Regression guard: ensures the `verify_replay` baseline used in the parity
/// test is itself correct — the native game is deterministic over K ticks.
///
/// This test does NOT require the wasm artifact.
#[test]
fn native_verify_replay_clean_baseline() {
    let cfg = match_config();
    let mut exec = NativeExecutor::<ArenaShooter>::new(cfg.clone());
    let mut log = ReplayLog::new(cfg.clone());

    let inputs: Vec<(PlayerId, Input)> = vec![];

    for tick in 1..=K_TICKS {
        let out = exec.step(tick, &inputs);
        log.record(tick, inputs.clone(), out.state_hash);
    }

    let verdict = verify_replay::<ArenaShooter>(&log);
    assert_eq!(
        verdict,
        ReplayVerdict::Clean,
        "native ArenaShooter must produce a Clean replay over {K_TICKS} ticks"
    );
    println!("[PASS] native verify_replay Clean ({K_TICKS} ticks)");
}
