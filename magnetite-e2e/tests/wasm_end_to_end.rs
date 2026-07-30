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
//! ## Why the inputs are not empty
//!
//! They used to be, and that is precisely why a guest bug survived for months:
//! the reference module discarded every input frame it was handed, and a parity
//! test that steps both sides with `vec![]` agrees perfectly — on nothing
//! happening. Parity over the empty input list only proves the two executors run
//! the same idle physics loop.
//!
//! These tests therefore drive **non-empty, varying** inputs from several
//! players. The sandbox ABI has no `on_join`, so first sight of a player id in a
//! `mag_step` payload is that player's join (see
//! `AuthoritativeGame::on_join`) — which means real inputs also exercise
//! spawning, validation, rejection and RNG-minted projectile ids on both sides.
//!
//! ## How to run
//!
//! ```sh
//! # 1. Build the wasm (one-time):
//! cd game-templates/authoritative
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
use magnetite_sdk::input::{Input, KeyState, MouseState};
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
/// from `game-templates/authoritative/`.
fn wasm_path() -> PathBuf {
    // The canonical release output path relative to the workspace root.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("e2e crate must have a parent")
        .to_path_buf();

    workspace_root
        .join("game-templates")
        .join("authoritative")
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

/// Number of players whose inputs are submitted each tick.
const PLAYERS: u64 = 4;

/// Deterministic, non-empty input frames for `tick`.
///
/// Varies per (tick, player) so movement, aim and shooting all fire across the
/// run — including the RNG draw that mints a projectile id, which is the part
/// that makes snapshot/restore faithfulness observable.
fn inputs_for_tick(tick: u64) -> Vec<(PlayerId, Input)> {
    (1..=PLAYERS)
        .map(|p| {
            let phase = (tick + p) % 4;
            (
                PlayerId::new(p),
                Input {
                    keys: KeyState {
                        forward: phase == 0,
                        right: phase == 1,
                        attack: phase == 2,
                        ..Default::default()
                    },
                    mouse: MouseState {
                        delta_x: if phase == 3 { 1.0 } else { 0.0 },
                        delta_y: if phase == 3 { 0.5 } else { 0.0 },
                        ..Default::default()
                    },
                    sequence: tick,
                    // Client-supplied and untrusted: a conforming module must
                    // never read this as a clock. Non-zero on purpose.
                    timestamp_ms: 1_000 + tick * 16,
                },
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Test 1: sandbox parity with native (headline proof)
// ---------------------------------------------------------------------------

/// **Headline proof:** `WasmExecutor` and `NativeExecutor` produce the same
/// `state_hash` on every tick given identical inputs and seed.
///
/// This test requires the wasm artifact to be present at:
///   `game-templates/authoritative/target/wasm32-wasip1/release/game_template_authoritative.wasm`
///
/// If the file is absent, the test is skipped with a clear message.
#[test]
fn wasm_sandbox_parity_with_native() {
    let wasm = wasm_path();

    if !wasm.exists() {
        println!("[SKIP] wasm artifact not found at {}", wasm.display());
        println!("Build it first:");
        println!("  cd game-templates/authoritative");
        println!("  cargo build --release --target wasm32-wasip1 --features wasm");
        // Hard fail so the CI script can distinguish missing-artifact from pass.
        panic!(
            "wasm artifact missing — run `cargo build --release --target wasm32-wasip1 --features wasm` in game-templates/authoritative/ first"
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

    // ── Drive K ticks with real, varying inputs ──────────────────────────
    let mut distinct_hashes = std::collections::BTreeSet::new();
    for tick in 1..=K_TICKS {
        let inputs = inputs_for_tick(tick);

        let native_out = native.step(tick, &inputs);
        let wasm_out = wasm_exec.step(tick, &inputs);

        native_hashes.push((tick, native_out.state_hash));
        wasm_hashes.push((tick, wasm_out.state_hash));
        distinct_hashes.insert(native_out.state_hash);

        // Record for verify_replay.
        log.record(tick, inputs.clone(), native_out.state_hash);

        assert_eq!(
            native_out.state_hash, wasm_out.state_hash,
            "state_hash MISMATCH at tick {tick}: native={} wasm={}",
            native_out.state_hash, wasm_out.state_hash
        );
        assert_eq!(
            native_out.rejects.len(),
            wasm_out.rejects.len(),
            "reject count MISMATCH at tick {tick}: native={:?} wasm={:?}",
            native_out.rejects,
            wasm_out.rejects
        );
    }

    // The whole point of using non-empty inputs: prove the world actually moved.
    // A module that discards its inputs produces an idle physics loop whose hash
    // sequence is short and repetitive, and both executors would still "agree".
    assert!(
        distinct_hashes.len() > K_TICKS as usize / 2,
        "expected the state to keep changing under real inputs, saw only {} distinct hashes over {K_TICKS} ticks — are inputs reaching the simulation?",
        distinct_hashes.len()
    );

    println!("[PASS] sandbox parity confirmed over {K_TICKS} ticks (seed=0x{SEED:016X})");
    println!(
        "       {} distinct state hashes — inputs are reaching the simulation",
        distinct_hashes.len()
    );
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
/// Snapshot/restore is covered by
/// [`wasm_snapshot_restore_resumes_the_same_trajectory`] below: the guest now
/// strips the 4-byte length prefix `WasmExecutor` writes and re-seeds its tick
/// counter from `ArenaSnapshot::tick`, so a restored module resumes correctly.
///
/// This test requires the wasm artifact.
#[test]
fn wasm_state_hash_is_reproducible_across_instances() {
    let wasm = wasm_path();
    if !wasm.exists() {
        panic!(
            "wasm artifact missing — run `cargo build --release --target wasm32-wasip1 --features wasm` in game-templates/authoritative/ first"
        );
    }

    let cfg = match_config();
    let limits = LimitsConfig {
        fuel_per_step: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_epochs_per_step: 10,
        epoch_tick_ms: 50,
    };

    // Instance A — run K ticks, record hashes.
    let mut exec_a = WasmExecutor::from_file(&wasm, cfg.clone(), limits.clone())
        .expect("WasmExecutor A must load successfully");
    let mut hashes_a: Vec<u64> = Vec::new();
    for tick in 1..=K_TICKS {
        let out = exec_a.step(tick, &inputs_for_tick(tick));
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
        let out = exec_b.step(tick, &inputs_for_tick(tick));
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

    for tick in 1..=K_TICKS {
        let inputs = inputs_for_tick(tick);
        let out = exec.step(tick, &inputs);
        log.record(tick, inputs, out.state_hash);
    }

    let verdict = verify_replay::<ArenaShooter>(&log);
    assert_eq!(
        verdict,
        ReplayVerdict::Clean,
        "native ArenaShooter must produce a Clean replay over {K_TICKS} ticks"
    );
    println!("[PASS] native verify_replay Clean ({K_TICKS} ticks, non-empty inputs)");
}

// ---------------------------------------------------------------------------
// Test 4: snapshot / restore resumes the same trajectory (wasm)
// ---------------------------------------------------------------------------

/// The property shard handoff and replay-from-checkpoint actually depend on:
/// take a snapshot mid-run, install it in a **fresh** executor, and the two must
/// then step forward identically.
///
/// This is strictly stronger than "restore(snapshot()) is idempotent", which also
/// passes on a module that ignores `mag_restore` altogether — the shape of the bug
/// this test was written to catch.
#[test]
fn wasm_snapshot_restore_resumes_the_same_trajectory() {
    let wasm = wasm_path();
    if !wasm.exists() {
        panic!(
            "wasm artifact missing — run `cargo build --release --target wasm32-wasip1 --features wasm` in game-templates/authoritative/ first"
        );
    }

    let cfg = match_config();
    let limits = LimitsConfig {
        fuel_per_step: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_epochs_per_step: 10,
        epoch_tick_ms: 50,
    };
    let split = K_TICKS / 2;

    // A runs the whole match, snapshotting at the halfway point.
    let mut a = WasmExecutor::from_file(&wasm, cfg.clone(), limits.clone())
        .expect("WasmExecutor A must load");
    for tick in 1..=split {
        a.step(tick, &inputs_for_tick(tick));
    }
    let snap = a.snapshot();
    assert!(!snap.is_empty(), "snapshot must not be empty");

    let continued: Vec<u64> = ((split + 1)..=K_TICKS)
        .map(|tick| a.step(tick, &inputs_for_tick(tick)).state_hash)
        .collect();

    // B starts fresh, adopts the snapshot, and runs only the second half.
    let mut b =
        WasmExecutor::from_file(&wasm, cfg.clone(), limits).expect("WasmExecutor B must load");
    b.restore(&snap);
    assert_eq!(
        b.snapshot(),
        snap,
        "restore must install the snapshot it was given"
    );

    let resumed: Vec<u64> = ((split + 1)..=K_TICKS)
        .map(|tick| b.step(tick, &inputs_for_tick(tick)).state_hash)
        .collect();

    assert_eq!(
        continued, resumed,
        "a wasm executor restored from a tick-{split} snapshot must resume the same trajectory"
    );
    assert!(
        !continued.contains(&0),
        "state_hash 0 means a guest call failed: {continued:?}"
    );
    println!(
        "[PASS] wasm snapshot at tick {split} resumes identically for {} ticks",
        continued.len()
    );
}
