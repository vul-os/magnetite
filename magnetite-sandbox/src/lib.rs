//! # `magnetite-sandbox`
//!
//! Wasmtime sandbox host for Magnetite authoritative game modules.
//!
//! Provides [`WasmExecutor`], which implements [`magnetite_sdk::authority::GameExecutor`]
//! by running a game compiled to `wasm32-wasip1` inside a `wasmtime::Store` with:
//!
//! - **Fuel metering** (`Config::consume_fuel(true)`) — deterministic CPU budgets per step.
//! - **Epoch interruption** (`Config::epoch_interruption(true)`) — wall-clock timeout per step.
//! - **Memory limits** ([`StoreLimits`]) — guest heap capped at a configurable byte ceiling.
//! - **Deterministic environment** — no `wasi` clock or random imports exposed; the guest
//!   receives tick + seed via the ABI, not OS calls.
//!
//! ## Sandbox ABI
//!
//! The guest module must export:
//!
//! ```text
//! mag_alloc(len: i32) -> i32          bump allocator — host writes inputs here
//! mag_free(ptr: i32, len: i32)        release a previously allocated region
//! mag_init(cfg_ptr: i32, cfg_len: i32)          initialise from JSON MatchConfig
//! mag_step(inputs_ptr: i32, inputs_len: i32) -> i32   run one tick; returns packed StepOutput ptr
//! mag_snapshot() -> i32               serialise state; returns ptr to length-prefixed bytes
//! mag_restore(ptr: i32, len: i32)     replace state from length-prefixed bytes
//! mag_view(player_id: i64) -> i32     per-player view; returns ptr to length-prefixed bytes
//! ```
//!
//! All guest-owned buffers are length-prefixed: a 4-byte little-endian `u32` followed by the
//! payload bytes.  The host reads the length, copies the payload, then calls `mag_free` on the
//! returned pointer (including the 4-byte prefix).
//!
//! ## Determinism constraints
//!
//! Games running inside the sandbox MUST obey the same rules as native games:
//!
//! 1. **No OS randomness** — `wasi_snapshot_preview1::random_get` is not linked; the
//!    guest must derive randomness from data passed via `mag_init` / `mag_step`.
//! 2. **No wall clock** — `wasi_snapshot_preview1::clock_time_get` is not linked.
//! 3. **Fuel budget** — each `mag_step` call is given [`LimitsConfig::fuel_per_step`] units of
//!    fuel; exceeding it returns [`SandboxError::FuelExhausted`].
//! 4. **Memory cap** — the guest's linear memory cannot grow beyond
//!    [`LimitsConfig::max_memory_bytes`]; attempts to grow past the cap trap and return
//!    [`SandboxError::MemoryLimitExceeded`].
//! 5. **Epoch timeout** — a background thread increments the engine epoch every
//!    [`LimitsConfig::epoch_tick_ms`] milliseconds; the store is configured to trap after
//!    [`LimitsConfig::max_epochs_per_step`] epochs, bounding wall time.
//!
//! ## Example (conceptual — requires a real `.wasm` at runtime)
//!
//! ```rust,no_run
//! use magnetite_sandbox::{WasmExecutor, LimitsConfig};
//! use magnetite_sdk::authority::{GameExecutor, MatchConfig};
//!
//! let cfg = MatchConfig::auto(2);
//! let limits = LimitsConfig::default();
//! let mut exec = WasmExecutor::from_file("game.wasm", cfg, limits).unwrap();
//!
//! // Drive the executor exactly like a NativeExecutor:
//! use magnetite_sdk::state::PlayerId;
//! use magnetite_sdk::input::Input;
//! let out = exec.step(1, &[(PlayerId::new(1), Input::default())]);
//! println!("tick 1 state_hash = {}", out.state_hash);
//! ```

pub mod abi;
pub mod executor;
pub mod limits;

pub use executor::WasmExecutor;
pub use limits::LimitsConfig;

/// Errors that can arise in the sandbox layer.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    /// The `.wasm` file could not be read from disk.
    #[error("wasm file I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The `.wasm` file could not be compiled or the module could not be instantiated.
    #[error("wasm engine error: {0}")]
    Load(#[from] wasmtime::Error),

    /// A required guest export (`mag_init`, `mag_step`, etc.) was not found.
    #[error("missing guest export `{name}`")]
    MissingExport { name: &'static str },

    /// The guest exhausted its per-step fuel budget.
    #[error("guest exhausted fuel budget per step")]
    FuelExhausted,

    /// The guest exceeded the memory limit.
    #[error("guest exceeded memory limit")]
    MemoryLimitExceeded,

    /// The guest exceeded the epoch / wall-clock deadline.
    #[error("guest exceeded epoch timeout per step")]
    EpochTimeout,

    /// The host could not serialise data to send to the guest.
    #[error("serialisation error: {0}")]
    Serialise(#[from] serde_json::Error),

    /// The guest returned a pointer that could not be read.
    #[error("guest returned invalid pointer or length-prefix: {0}")]
    InvalidGuestPointer(String),

    /// The Wasm trap indicated a memory limit was exceeded (via `StoreLimits`).
    #[error("wasm trap: {0}")]
    Trap(String),
}
