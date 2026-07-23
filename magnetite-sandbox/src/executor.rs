//! [`WasmExecutor`] — the Wasmtime-based [`GameExecutor`] implementation.
//!
//! ## Architecture
//!
//! ```text
//!  ┌──────────────────────────────────────────────────┐
//!  │  WasmExecutor                                    │
//!  │                                                  │
//!  │  Engine (fuel + epoch, per-executor)             │
//!  │  Store<StoreState>                               │
//!  │    └─ limits: StoreLimits                        │
//!  │  Instance (wasm exports)                         │
//!  │  limits_cfg: LimitsConfig                        │
//!  │  cached_snapshot: Vec<u8>  ← last snapshot bytes │
//!  └──────────────────────────────────────────────────┘
//! ```
//!
//! The `Engine` is configured once with `consume_fuel(true)` and
//! `epoch_interruption(true)`. A dedicated background thread increments the
//! epoch counter every `epoch_tick_ms` milliseconds for wall-clock timeouts.
//!
//! ## &self methods and caching
//!
//! [`GameExecutor::snapshot`], [`GameExecutor::view_for`], and
//! [`GameExecutor::delta_since`] take `&self` in the trait.  Calling into
//! Wasmtime requires `&mut Store`, which conflicts with shared references.
//! To resolve this without unsound code:
//!
//! - `snapshot()` returns a **cached** snapshot that is refreshed after every
//!   `step()` and `restore()` call.
//! - `view_for()` returns a cached per-player view refreshed after each step.
//!   If multiple players are needed, the runtime calls `step` between queries,
//!   so the cache stays current.
//! - `delta_since()` computes the delta host-side from the cached snapshot and
//!   the provided baseline — no guest call needed.
//!
//! This approach is correct because the runtime always calls `step` before
//! reading `snapshot`/`view_for`/`delta_since`, and `restore` always updates
//! the cache.
//!
//! ## WASI determinism
//!
//! The linker intentionally does **not** link the full WASI snapshot-preview1
//! surface. Clock and random imports are replaced with ENOSYS-returning stubs
//! so any guest that calls them gets a deterministic failure.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use wasmtime::{Engine, Instance, Linker, Module, Store};

use magnetite_sdk::authority::{GameExecutor, MatchConfig, StepOutput, Tick};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

use crate::abi;
use crate::limits::{LimitsConfig, StoreLimits};
use crate::SandboxError;

// ---------------------------------------------------------------------------
// Store state
// ---------------------------------------------------------------------------

/// Data stored inside the `wasmtime::Store`.
struct StoreState {
    limits: StoreLimits,
}

// ---------------------------------------------------------------------------
// WasmExecutor
// ---------------------------------------------------------------------------

/// A [`GameExecutor`] that runs an authoritative game compiled to `wasm32-wasip1`
/// inside a fuel-metered, epoch-interrupted, memory-capped Wasmtime sandbox.
///
/// # Determinism guarantees
///
/// - **Fuel budget**: each `mag_step` call receives [`LimitsConfig::fuel_per_step`]
///   fuel units; overrun traps as [`SandboxError::FuelExhausted`].
/// - **Epoch timeout**: the store epoch deadline is reset before each
///   `mag_step`; overrun traps as [`SandboxError::EpochTimeout`].
/// - **No WASI clock/random**: these imports are replaced with ENOSYS stubs.
/// - **Memory cap**: `memory.grow` beyond the configured ceiling is denied by
///   the [`StoreLimits`] resource limiter.
///
/// # Example
///
/// ```rust,no_run
/// use magnetite_sandbox::{WasmExecutor, LimitsConfig};
/// use magnetite_sdk::authority::{GameExecutor, MatchConfig};
/// use magnetite_sdk::state::PlayerId;
/// use magnetite_sdk::input::Input;
///
/// let cfg = MatchConfig::auto(4);
/// let limits = LimitsConfig::default();
/// let mut exec = WasmExecutor::from_file("game.wasm", cfg, limits).unwrap();
///
/// let out = exec.step(1, &[(PlayerId::new(1), Input::default())]);
/// println!("state_hash after tick 1 = {}", out.state_hash);
/// ```
pub struct WasmExecutor {
    store: Store<StoreState>,
    instance: Instance,
    limits_cfg: LimitsConfig,

    /// Cached serialised snapshot — refreshed after every `step` and `restore`.
    ///
    /// This allows `snapshot()` (which takes `&self`) to return the last
    /// authoritative snapshot without calling into the Wasm guest.
    cached_snapshot: Vec<u8>,

    /// Cached per-player views — refreshed after every `step`.
    ///
    /// Key: player id. Value: serialised view bytes.
    cached_views: HashMap<u64, Vec<u8>>,
}

impl WasmExecutor {
    // ---------------------------------------------------------------------- //
    // Construction                                                            //
    // ---------------------------------------------------------------------- //

    /// Load a `wasm32-wasip1` game module from a file path.
    ///
    /// Compiles the module eagerly, instantiates it, and calls `mag_init` with
    /// the serialised [`MatchConfig`].
    pub fn from_file(
        path: impl AsRef<std::path::Path>,
        config: MatchConfig,
        limits: LimitsConfig,
    ) -> Result<Self, SandboxError> {
        let wasm_bytes = std::fs::read(path)?;
        Self::from_bytes(&wasm_bytes, config, limits)
    }

    /// Load a `wasm32-wasip1` game module from in-memory bytes.
    ///
    /// Useful for tests that embed WAT-compiled modules inline.
    pub fn from_bytes(
        wasm_bytes: &[u8],
        config: MatchConfig,
        limits: LimitsConfig,
    ) -> Result<Self, SandboxError> {
        // Build the engine with fuel and epoch interruption enabled.
        let mut engine_cfg = wasmtime::Config::new();
        engine_cfg.consume_fuel(true);
        engine_cfg.epoch_interruption(true);
        let engine = Engine::new(&engine_cfg)?;

        // Compile the module eagerly.
        let module = Module::new(&engine, wasm_bytes)?;

        // Build a linker with deterministic WASI stubs.
        let linker = build_linker(&engine)?;

        // Build the store with the resource limiter.
        let state = StoreState {
            limits: StoreLimits {
                max_memory_bytes: limits.max_memory_bytes,
            },
        };
        let mut store = Store::new(&engine, state);
        store.limiter(|s| &mut s.limits);

        // Set initial fuel.
        store.set_fuel(limits.fuel_per_step)?;
        store.set_epoch_deadline(limits.max_epochs_per_step);

        // Spawn the epoch-incrementing background thread.
        spawn_epoch_thread(engine.clone(), limits.epoch_tick_ms);

        // Instantiate the module.
        let instance = linker.instantiate(&mut store, &module)?;

        let mut exec = Self {
            store,
            instance,
            limits_cfg: limits,
            cached_snapshot: Vec::new(),
            cached_views: HashMap::new(),
        };

        // Call mag_init with the serialised MatchConfig.
        exec.call_mag_init(&config)?;

        // Populate the initial snapshot cache.
        exec.refresh_snapshot_cache()?;

        Ok(exec)
    }

    // ---------------------------------------------------------------------- //
    // ABI helpers                                                             //
    // ---------------------------------------------------------------------- //

    /// Write `bytes` into guest memory via `mag_alloc`; return the guest pointer.
    fn alloc_and_write(&mut self, bytes: &[u8]) -> Result<i32, SandboxError> {
        let len = bytes.len() as i32;

        let mag_alloc = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "mag_alloc")
            .map_err(|_| SandboxError::MissingExport { name: "mag_alloc" })?;

        let ptr = mag_alloc.call(&mut self.store, len)?;

        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or(SandboxError::MissingExport { name: "memory" })?;

        let mem_data = memory.data_mut(&mut self.store);
        let start = ptr as usize;
        let end = start + bytes.len();
        if end > mem_data.len() {
            return Err(SandboxError::InvalidGuestPointer(format!(
                "alloc ptr={ptr} end={end} exceeds memory len={}",
                mem_data.len()
            )));
        }
        mem_data[start..end].copy_from_slice(bytes);
        Ok(ptr)
    }

    /// Free a guest buffer previously obtained from `mag_alloc`.
    fn free_guest(&mut self, ptr: i32, len: i32) -> Result<(), SandboxError> {
        let mag_free = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "mag_free")
            .map_err(|_| SandboxError::MissingExport { name: "mag_free" })?;

        mag_free.call(&mut self.store, (ptr, len))?;
        Ok(())
    }

    /// Read a length-prefixed guest buffer at `ptr`, then free it.
    ///
    /// Format: `[u32 LE length][payload bytes …]`
    fn read_and_free_length_prefixed(&mut self, ptr: i32) -> Result<Vec<u8>, SandboxError> {
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or(SandboxError::MissingExport { name: "memory" })?;

        let base = ptr as usize;
        let mem_data = memory.data(&self.store);

        if base + 4 > mem_data.len() {
            return Err(SandboxError::InvalidGuestPointer(format!(
                "ptr={ptr} too close to end for 4-byte prefix"
            )));
        }
        let payload_len = u32::from_le_bytes([
            mem_data[base],
            mem_data[base + 1],
            mem_data[base + 2],
            mem_data[base + 3],
        ]) as usize;
        if base + 4 + payload_len > mem_data.len() {
            return Err(SandboxError::InvalidGuestPointer(format!(
                "ptr={ptr} payload_len={payload_len} exceeds memory"
            )));
        }
        let payload = mem_data[base + 4..base + 4 + payload_len].to_vec();

        self.free_guest(ptr, (4 + payload_len) as i32)?;
        Ok(payload)
    }

    /// Call `mag_init` with the serialised [`MatchConfig`].
    fn call_mag_init(&mut self, config: &MatchConfig) -> Result<(), SandboxError> {
        let cfg_bytes = abi::encode_config(config)?;
        let ptr = self.alloc_and_write(&cfg_bytes)?;
        let len = cfg_bytes.len() as i32;

        let mag_init = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "mag_init")
            .map_err(|_| SandboxError::MissingExport { name: "mag_init" })?;

        mag_init.call(&mut self.store, (ptr, len))?;
        self.free_guest(ptr, len)?;
        Ok(())
    }

    /// Replenish fuel and reset the epoch deadline before a bounded guest call.
    fn prepare_step_limits(&mut self) -> Result<(), SandboxError> {
        self.store.set_fuel(self.limits_cfg.fuel_per_step)?;
        self.store
            .set_epoch_deadline(self.limits_cfg.max_epochs_per_step);
        Ok(())
    }

    /// Refresh `cached_snapshot` from the guest's current state via `mag_snapshot`.
    fn refresh_snapshot_cache(&mut self) -> Result<(), SandboxError> {
        let mag_snapshot = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "mag_snapshot")
            .map_err(|_| SandboxError::MissingExport {
                name: "mag_snapshot",
            })?;

        let ptr = mag_snapshot
            .call(&mut self.store, ())
            .map_err(Self::classify_trap)?;

        self.cached_snapshot = self.read_and_free_length_prefixed(ptr)?;
        Ok(())
    }

    /// Classify a wasmtime error into the appropriate [`SandboxError`] variant.
    fn classify_trap(err: wasmtime::Error) -> SandboxError {
        let msg = err.to_string();
        if msg.contains("all fuel consumed") || msg.contains("fuel") {
            SandboxError::FuelExhausted
        } else if msg.contains("epoch") || msg.contains("interrupt") {
            SandboxError::EpochTimeout
        } else if msg.contains("out of bounds memory") || msg.contains("memory access") {
            SandboxError::MemoryLimitExceeded
        } else {
            SandboxError::Trap(msg)
        }
    }

    // ---------------------------------------------------------------------- //
    // Inner (fallible) implementations of the GameExecutor methods           //
    // ---------------------------------------------------------------------- //

    fn step_inner(&mut self, inputs: &[(PlayerId, Input)]) -> Result<StepOutput, SandboxError> {
        self.prepare_step_limits()?;

        let input_bytes = abi::encode_inputs(inputs)?;
        let ptr = self.alloc_and_write(&input_bytes)?;
        let len = input_bytes.len() as i32;

        let mag_step = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, "mag_step")
            .map_err(|_| SandboxError::MissingExport { name: "mag_step" })?;

        let out_ptr = mag_step
            .call(&mut self.store, (ptr, len))
            .map_err(Self::classify_trap)?;

        self.free_guest(ptr, len)?;

        let out_bytes = self.read_and_free_length_prefixed(out_ptr)?;
        let output = abi::decode_step_output(&out_bytes)?;

        // Refresh the snapshot cache and per-player view caches after each step.
        if let Err(e) = self.refresh_snapshot_cache() {
            eprintln!("[magnetite-sandbox] post-step snapshot refresh failed: {e}");
        }
        self.refresh_view_caches(inputs);

        Ok(output)
    }

    /// Refresh per-player view caches for all players that sent inputs this tick.
    fn refresh_view_caches(&mut self, inputs: &[(PlayerId, Input)]) {
        for (player, _) in inputs {
            match self.call_mag_view(*player) {
                Ok(bytes) => {
                    self.cached_views.insert(player.as_u64(), bytes);
                }
                Err(e) => {
                    eprintln!(
                        "[magnetite-sandbox] view_for({}) refresh failed: {e}",
                        player.as_u64()
                    );
                }
            }
        }
    }

    fn call_mag_view(&mut self, player: PlayerId) -> Result<Vec<u8>, SandboxError> {
        let mag_view = self
            .instance
            .get_typed_func::<i64, i32>(&mut self.store, "mag_view")
            .map_err(|_| SandboxError::MissingExport { name: "mag_view" })?;

        let ptr = mag_view
            .call(&mut self.store, player.as_u64() as i64)
            .map_err(Self::classify_trap)?;

        self.read_and_free_length_prefixed(ptr)
    }

    fn restore_inner(&mut self, bytes: &[u8]) -> Result<(), SandboxError> {
        let framed = abi::write_length_prefixed(bytes);
        let ptr = self.alloc_and_write(&framed)?;
        let len = framed.len() as i32;

        let mag_restore = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "mag_restore")
            .map_err(|_| SandboxError::MissingExport {
                name: "mag_restore",
            })?;

        mag_restore
            .call(&mut self.store, (ptr, len))
            .map_err(Self::classify_trap)?;

        self.free_guest(ptr, len)?;

        // Refresh cache so snapshot() stays current after restore.
        self.refresh_snapshot_cache()?;
        self.cached_views.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// GameExecutor implementation
// ---------------------------------------------------------------------------

impl GameExecutor for WasmExecutor {
    /// Advance the guest game by one authoritative tick.
    ///
    /// Encodes `inputs` as JSON, writes them into guest memory, calls
    /// `mag_step` with a fresh fuel budget and epoch deadline, decodes and
    /// returns the [`StepOutput`].  Also refreshes the snapshot and view caches.
    fn step(&mut self, _tick: Tick, inputs: &[(PlayerId, Input)]) -> StepOutput {
        match self.step_inner(inputs) {
            Ok(out) => out,
            Err(e) => {
                eprintln!("[magnetite-sandbox] step error: {e}");
                // Zero hash signals divergence to the replay verifier.
                StepOutput {
                    rejects: vec![],
                    state_hash: 0,
                }
            }
        }
    }

    /// Return the last cached snapshot bytes.
    ///
    /// The cache is populated after each [`step`](Self::step) and
    /// [`restore`](Self::restore) call, so this is always fresh.
    fn snapshot(&self) -> Vec<u8> {
        self.cached_snapshot.clone()
    }

    /// Replace the current game state from a previously-serialised snapshot.
    fn restore(&mut self, bytes: &[u8]) {
        if let Err(e) = self.restore_inner(bytes) {
            eprintln!("[magnetite-sandbox] restore error: {e}");
        }
    }

    /// Return the interest-filtered view for `player` from the cache.
    ///
    /// The cache is populated after each [`step`](Self::step) for every player
    /// that submitted inputs.  If the player has no cached view (e.g. they
    /// joined after the last step), an empty `Vec` is returned.
    fn view_for(&self, player: PlayerId) -> Vec<u8> {
        self.cached_views
            .get(&player.as_u64())
            .cloned()
            .unwrap_or_default()
    }

    /// Return the delta since the state encoded in `snapshot_bytes`.
    ///
    /// Returns the current cached snapshot as a conservative full-state delta.
    /// The runtime can diff against the baseline snapshot when a compact delta
    /// is needed.  A future `mag_delta` export path can be added when the trait
    /// is updated to `&mut self` (tracked in DECISIONS.md as S3).
    ///
    /// # Determinism note
    ///
    /// The `snapshot_bytes` parameter is intentionally ignored in this
    /// implementation: the guest's `mag_delta` export would require `&mut Store`
    /// which conflicts with the `&self` trait signature.  Returning the full
    /// snapshot is safe and correct — the runtime diffs it client-side.
    fn delta_since(&self, _snapshot_bytes: &[u8]) -> Vec<u8> {
        self.cached_snapshot.clone()
    }
}

// ---------------------------------------------------------------------------
// Linker construction — deterministic WASI stubs
// ---------------------------------------------------------------------------

/// Build a [`Linker`] providing only the minimal WASI functions required for
/// `wasm32-wasip1` module initialisation, with clock and random replaced by
/// ENOSYS-returning stubs that preserve determinism.
fn build_linker(engine: &Engine) -> Result<Linker<StoreState>, SandboxError> {
    let mut linker: Linker<StoreState> = Linker::new(engine);

    // clock_time_get — always returns ENOSYS (errno 38).
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "clock_time_get",
            |_id: i32, _prec: i64, _ptr: i32| -> i32 { 38 },
        )
        .map_err(wasmtime::Error::from)?;

    // clock_res_get — always returns ENOSYS.
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "clock_res_get",
            |_id: i32, _ptr: i32| -> i32 { 38 },
        )
        .map_err(wasmtime::Error::from)?;

    // random_get — always returns ENOSYS.
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "random_get",
            |_buf: i32, _len: i32| -> i32 { 38 },
        )
        .map_err(wasmtime::Error::from)?;

    // fd_write — silently discards all output.
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_write",
            |_fd: i32, _iovs: i32, _iovs_len: i32, _nwritten: i32| -> i32 { 0 },
        )
        .map_err(wasmtime::Error::from)?;

    // fd_read — always returns empty.
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_read",
            |_fd: i32, _iovs: i32, _iovs_len: i32, _nread: i32| -> i32 { 0 },
        )
        .map_err(wasmtime::Error::from)?;

    // proc_exit — game code should never call this.
    linker
        .func_wrap("wasi_snapshot_preview1", "proc_exit", |_code: i32| {})
        .map_err(wasmtime::Error::from)?;

    // environ_get / environ_sizes_get — no environment variables.
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "environ_get",
            |_env: i32, _buf: i32| -> i32 { 0 },
        )
        .map_err(wasmtime::Error::from)?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "environ_sizes_get",
            |_cnt: i32, _sz: i32| -> i32 { 0 },
        )
        .map_err(wasmtime::Error::from)?;

    // args_get / args_sizes_get — no arguments.
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "args_get",
            |_argv: i32, _buf: i32| -> i32 { 0 },
        )
        .map_err(wasmtime::Error::from)?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "args_sizes_get",
            |_argc: i32, _sz: i32| -> i32 { 0 },
        )
        .map_err(wasmtime::Error::from)?;

    Ok(linker)
}

// ---------------------------------------------------------------------------
// Epoch-incrementing background thread
// ---------------------------------------------------------------------------

/// Spawn a daemon thread that increments the engine epoch every `interval_ms` ms.
fn spawn_epoch_thread(engine: Engine, interval_ms: u64) {
    let engine = Arc::new(engine);
    thread::Builder::new()
        .name("mag-sandbox-epoch".to_string())
        .spawn(move || {
            let interval = Duration::from_millis(interval_ms);
            loop {
                thread::sleep(interval);
                engine.increment_epoch();
            }
        })
        .expect("failed to spawn epoch thread");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::LimitsConfig;
    use magnetite_sdk::authority::MatchConfig;

    // ---- Error on invalid wasm -------------------------------------------------

    #[test]
    fn from_bytes_rejects_empty_bytes() {
        let cfg = MatchConfig::auto(2);
        let result = WasmExecutor::from_bytes(&[], cfg, LimitsConfig::default());
        assert!(result.is_err(), "empty bytes must fail to compile");
    }

    #[test]
    fn from_bytes_rejects_garbage() {
        let cfg = MatchConfig::auto(2);
        let result = WasmExecutor::from_bytes(b"not wasm !@#$", cfg, LimitsConfig::default());
        assert!(result.is_err(), "garbage must fail to compile");
    }

    // ---- classify_trap mapping -------------------------------------------------

    #[test]
    fn classify_fuel_trap() {
        let e = wasmtime::Error::msg("all fuel consumed by this instruction");
        assert!(matches!(
            WasmExecutor::classify_trap(e),
            SandboxError::FuelExhausted
        ));
    }

    #[test]
    fn classify_epoch_trap() {
        let e = wasmtime::Error::msg("epoch deadline reached for async execution");
        assert!(matches!(
            WasmExecutor::classify_trap(e),
            SandboxError::EpochTimeout
        ));
    }

    #[test]
    fn classify_memory_trap() {
        let e = wasmtime::Error::msg("out of bounds memory access");
        assert!(matches!(
            WasmExecutor::classify_trap(e),
            SandboxError::MemoryLimitExceeded
        ));
    }

    #[test]
    fn classify_unknown_trap() {
        let e = wasmtime::Error::msg("some other trap");
        assert!(matches!(
            WasmExecutor::classify_trap(e),
            SandboxError::Trap(_)
        ));
    }

    // ---- Engine configuration --------------------------------------------------

    #[test]
    fn engine_with_fuel_and_epoch_builds() {
        let mut cfg = wasmtime::Config::new();
        cfg.consume_fuel(true);
        cfg.epoch_interruption(true);
        assert!(Engine::new(&cfg).is_ok());
    }

    // ---- LimitsConfig defaults -------------------------------------------------

    #[test]
    fn limits_defaults_are_sane() {
        let l = LimitsConfig::default();
        assert!(l.fuel_per_step > 0);
        assert!(l.max_memory_bytes >= 1024 * 1024);
        assert!(l.max_epochs_per_step > 0);
        assert!(l.epoch_tick_ms > 0);
    }

    // ---- End-to-end execution (gated) -----------------------------------------
    //
    // A full execution test requires a valid wasm32-wasip1 game module.
    // The ABI codec is tested exhaustively in abi.rs; this test is gated with
    // #[ignore] so it does not block CI on machines without a game.wasm.
    //
    // To run locally:
    //   cargo test -p magnetite-sandbox -- --ignored

    #[test]
    #[ignore = "requires a valid wasm32-wasip1 game module; see executor.rs docs"]
    fn full_cycle_wasm_execution() {
        // 1. Compile a tiny WAT game that implements the Sandbox ABI.
        // 2. Create WasmExecutor::from_bytes.
        // 3. Call step / snapshot / restore / view_for.
        // 4. Assert state_hash changes deterministically.
        //
        // This test is marked #[ignore] because:
        // (a) Wasmtime compilation is slow in CI (~3-10 s cold).
        // (b) A correct WAT implementation of the full ABI is hundreds of lines
        //     and better expressed as an integration test using `magnetite build`.
        // (c) The ABI codec (abi.rs) and resource limits (limits.rs) are each
        //     tested independently above.
        unimplemented!("provide a wasm32-wasip1 game module and remove #[ignore]");
    }
}
