//! Conformance harness for the public `mag_*` sandbox ABI.
//!
//! Give it any `.wasm` (or `.wat`) module and it reports, check by check,
//! whether that module satisfies the contract in `site/docs/sandbox-abi.md`.
//! It knows nothing about the source language: it only calls exports and reads
//! linear memory, exactly as the real host does.
//!
//! The harness deliberately uses the **same** engine configuration, the same
//! WASI stub linker and the same resource limiter as [`crate::WasmExecutor`],
//! so a pass here means "this module works in the real sandbox", not "this
//! module works in a test rig".
//!
//! ## What it can and cannot decide
//!
//! Some contract clauses are decidable by observation ("the length prefix is
//! honoured", "two fresh instances agree tick for tick"). Others are
//! properties of the module's source that no black-box run can prove — a
//! module could be deterministic for the harness's inputs and not for yours.
//! Checks are therefore reported at four strengths:
//!
//! | Status | Meaning |
//! |---|---|
//! | [`Status::Pass`] | The contract clause was observed to hold. |
//! | [`Status::Fail`] | The clause was observed to be violated. Non-conforming. |
//! | [`Status::Warn`] | Legal, but almost certainly a bug in the module. |
//! | [`Status::Info`] | Measured, not judged. |
//!
//! [`Report::is_conforming`] is true when no check failed. It is evidence, not
//! a proof.
//!
//! ## Example
//!
//! ```rust,no_run
//! use magnetite_sandbox::conformance::{self, ConformanceOptions};
//!
//! let wasm = std::fs::read("game.wasm").unwrap();
//! let report = conformance::run(&wasm, &ConformanceOptions::default());
//! println!("{}", report.render());
//! assert!(report.is_conforming());
//! ```

use std::fmt::Write as _;

use wasmtime::{Engine, Instance, Module, Store, Val, ValType};

use magnetite_sdk::authority::{MatchConfig, Tick, Topology};
use magnetite_sdk::input::{Input, KeyState};
use magnetite_sdk::state::PlayerId;

use crate::abi;
use crate::executor::{build_linker, spawn_epoch_thread, StoreState};
use crate::limits::{LimitsConfig, StoreLimits};

// ---------------------------------------------------------------------------
// The export table the contract defines
// ---------------------------------------------------------------------------

/// Every required export, with its exact signature.
///
/// `params` / `results` are Wasm value types. `i32` carries both pointers and
/// lengths; `mag_view` takes the player id as an `i64` (the contract's only
/// 64-bit parameter).
pub const REQUIRED_EXPORTS: &[(&str, &[ValType], &[ValType])] = &[
    ("mag_abi_version", &[], &[ValType::I32]),
    ("mag_alloc", &[ValType::I32], &[ValType::I32]),
    ("mag_free", &[ValType::I32, ValType::I32], &[]),
    ("mag_init", &[ValType::I32, ValType::I32], &[]),
    ("mag_step", &[ValType::I32, ValType::I32], &[ValType::I32]),
    ("mag_snapshot", &[], &[ValType::I32]),
    ("mag_restore", &[ValType::I32, ValType::I32], &[]),
    ("mag_view", &[ValType::I64], &[ValType::I32]),
];

/// The complete set of host imports a conforming module may declare.
///
/// This is the surface `build_linker` provides. A module importing anything
/// else cannot be instantiated by the sandbox at all. Note that
/// `clock_time_get`, `clock_res_get` and `random_get` are *present* but return
/// `ENOSYS` (38) — importing them is legal, depending on their success is not.
pub const ALLOWED_IMPORTS: &[(&str, &str)] = &[
    ("wasi_snapshot_preview1", "clock_time_get"),
    ("wasi_snapshot_preview1", "clock_res_get"),
    ("wasi_snapshot_preview1", "random_get"),
    ("wasi_snapshot_preview1", "fd_write"),
    ("wasi_snapshot_preview1", "fd_read"),
    ("wasi_snapshot_preview1", "proc_exit"),
    ("wasi_snapshot_preview1", "environ_get"),
    ("wasi_snapshot_preview1", "environ_sizes_get"),
    ("wasi_snapshot_preview1", "args_get"),
    ("wasi_snapshot_preview1", "args_sizes_get"),
];

/// Imports that are linked but non-functional. Declaring them is a smell: the
/// module was compiled against a runtime that expects a clock or an OS RNG.
pub const ENOSYS_IMPORTS: &[&str] = &["clock_time_get", "clock_res_get", "random_get"];

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Strength of a single conformance finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The clause was observed to hold.
    Pass,
    /// The clause was observed to be violated — the module is non-conforming.
    Fail,
    /// Permitted by the contract, but near-certainly a defect in the module.
    Warn,
    /// A measurement, not a judgement.
    Info,
    /// Not checked, because an earlier check made it impossible.
    Skip,
}

impl Status {
    /// Fixed-width label used by [`Report::render`].
    pub fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Warn => "WARN",
            Status::Info => "INFO",
            Status::Skip => "SKIP",
        }
    }
}

/// One conformance finding.
#[derive(Debug, Clone)]
pub struct Check {
    /// Stable dotted identifier, e.g. `mag_step.length_prefix`.
    pub id: String,
    /// Outcome.
    pub status: Status,
    /// Human-readable evidence — what was observed, not what was expected.
    pub detail: String,
}

/// The full result of a conformance run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Findings, in the order they were produced.
    pub checks: Vec<Check>,
    /// Size of the module under test, in bytes.
    pub module_bytes: usize,
}

impl Report {
    fn push(&mut self, id: impl Into<String>, status: Status, detail: impl Into<String>) {
        self.checks.push(Check {
            id: id.into(),
            status,
            detail: detail.into(),
        });
    }

    fn ok(&mut self, id: impl Into<String>, detail: impl Into<String>) {
        self.push(id, Status::Pass, detail);
    }

    fn bad(&mut self, id: impl Into<String>, detail: impl Into<String>) {
        self.push(id, Status::Fail, detail);
    }

    /// Number of checks with a given status.
    pub fn count(&self, status: Status) -> usize {
        self.checks.iter().filter(|c| c.status == status).count()
    }

    /// True when no check failed.
    ///
    /// This is evidence of conformance over the inputs the harness tried, not a
    /// proof of conformance over all inputs.
    pub fn is_conforming(&self) -> bool {
        self.count(Status::Fail) == 0
    }

    /// Render the report as aligned plain text, one line per check.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let width = self
            .checks
            .iter()
            .map(|c| c.id.len())
            .max()
            .unwrap_or(0)
            .max(4);
        for c in &self.checks {
            let _ = writeln!(
                out,
                "{:<5} {:<width$}  {}",
                c.status.label(),
                c.id,
                c.detail,
                width = width
            );
        }
        let _ = writeln!(
            out,
            "\n{} bytes · {} pass · {} fail · {} warn · {} info · {} skip · verdict: {}",
            self.module_bytes,
            self.count(Status::Pass),
            self.count(Status::Fail),
            self.count(Status::Warn),
            self.count(Status::Info),
            self.count(Status::Skip),
            if self.is_conforming() {
                "CONFORMING"
            } else {
                "NON-CONFORMING"
            }
        );
        out
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Knobs for a conformance run.
#[derive(Debug, Clone)]
pub struct ConformanceOptions {
    /// How many ticks to step when checking determinism and snapshot replay.
    pub ticks: u64,
    /// The `MatchConfig::seed` handed to the module under test.
    pub seed: u64,
    /// A second, different seed, used to see whether the module consumes it.
    pub alt_seed: u64,
    /// Player cap in the `MatchConfig`, and the number of synthetic players
    /// whose inputs are submitted each tick.
    pub players: u32,
    /// Resource limits — the same type the real host uses.
    pub limits: LimitsConfig,
}

impl Default for ConformanceOptions {
    fn default() -> Self {
        Self {
            ticks: 24,
            seed: 0xDEAD_CAFE_1337_BABE,
            alt_seed: 0x0BAD_F00D_0000_0001,
            players: 4,
            // Epoch budget widened relative to the production default: the
            // harness cares whether the mechanism reaches the guest, and a
            // 10 ms ceiling makes functional checks flaky on a loaded machine.
            limits: LimitsConfig {
                max_epochs_per_step: 200,
                ..LimitsConfig::default()
            },
        }
    }
}

impl ConformanceOptions {
    fn config(&self, seed: u64) -> MatchConfig {
        MatchConfig {
            topology: Topology::SingleRoom,
            max_players: self.players,
            tick_hz: 60,
            seed,
            snapshot_every: 300,
        }
    }
}

// ---------------------------------------------------------------------------
// Probe — a fallible, one-call-at-a-time view of a guest instance
// ---------------------------------------------------------------------------

/// Errors a probe surfaces. Distinct from [`crate::SandboxError`] because the
/// harness must distinguish "the module misbehaved" from "the host failed".
type ProbeResult<T> = Result<T, String>;

struct Probe {
    store: Store<StoreState>,
    instance: Instance,
    fuel_per_step: u64,
    epochs_per_step: u64,
}

impl Probe {
    fn new(engine: &Engine, module: &Module, limits: &LimitsConfig) -> ProbeResult<Self> {
        let linker = build_linker(engine).map_err(|e| e.to_string())?;
        let state = StoreState {
            limits: StoreLimits {
                max_memory_bytes: limits.max_memory_bytes,
            },
        };
        let mut store = Store::new(engine, state);
        store.limiter(|s| &mut s.limits);
        store
            .set_fuel(limits.fuel_per_step)
            .map_err(|e| e.to_string())?;
        store.set_epoch_deadline(limits.max_epochs_per_step);
        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|e| format!("instantiate failed: {}", one_line(&e.to_string())))?;
        Ok(Self {
            store,
            instance,
            fuel_per_step: limits.fuel_per_step,
            epochs_per_step: limits.max_epochs_per_step,
        })
    }

    /// Replenish the per-call budgets, exactly as the host does before each step.
    fn refuel(&mut self) -> ProbeResult<()> {
        self.store
            .set_fuel(self.fuel_per_step)
            .map_err(|e| e.to_string())?;
        self.store.set_epoch_deadline(self.epochs_per_step);
        Ok(())
    }

    fn call(&mut self, name: &str, args: &[Val]) -> ProbeResult<Vec<Val>> {
        let func = self
            .instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| format!("missing export `{name}`"))?;
        let n_results = func.ty(&self.store).results().len();
        let mut results = vec![Val::I32(0); n_results];
        func.call(&mut self.store, args, &mut results)
            .map_err(|e| format!("`{name}` trapped: {}", trap_reason(&e)))?;
        Ok(results)
    }

    fn call_i32(&mut self, name: &str, args: &[Val]) -> ProbeResult<i32> {
        match self.call(name, args)?.first() {
            Some(Val::I32(v)) => Ok(*v),
            other => Err(format!("`{name}` returned {other:?}, expected one i32")),
        }
    }

    fn memory_len(&mut self) -> ProbeResult<usize> {
        let mem = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| "missing export `memory`".to_string())?;
        Ok(mem.data(&self.store).len())
    }

    /// `mag_alloc(len)` then write `bytes` at the returned pointer.
    fn alloc_write(&mut self, bytes: &[u8]) -> ProbeResult<i32> {
        let ptr = self.call_i32("mag_alloc", &[Val::I32(bytes.len() as i32)])?;
        if ptr < 0 {
            return Err(format!("mag_alloc returned negative pointer {ptr}"));
        }
        let mem = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| "missing export `memory`".to_string())?;
        let data = mem.data_mut(&mut self.store);
        let start = ptr as usize;
        let end = start
            .checked_add(bytes.len())
            .ok_or_else(|| "alloc pointer + len overflows".to_string())?;
        if end > data.len() {
            return Err(format!(
                "mag_alloc({}) returned ptr={ptr}; [{start},{end}) is outside the {}-byte memory",
                bytes.len(),
                data.len()
            ));
        }
        data[start..end].copy_from_slice(bytes);
        Ok(ptr)
    }

    /// Read a `[u32 LE length][payload]` buffer at `ptr` without freeing it.
    ///
    /// Returns the payload and the total framed size, or a precise description
    /// of how the framing was violated.
    fn read_prefixed(&mut self, ptr: i32) -> ProbeResult<(Vec<u8>, usize)> {
        let mem = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| "missing export `memory`".to_string())?;
        let data = mem.data(&self.store);
        if ptr < 0 {
            return Err(format!("returned pointer is negative ({ptr})"));
        }
        let base = ptr as usize;
        if base + 4 > data.len() {
            return Err(format!(
                "ptr={ptr} leaves no room for the 4-byte prefix in {} bytes of memory",
                data.len()
            ));
        }
        let len = u32::from_le_bytes([data[base], data[base + 1], data[base + 2], data[base + 3]])
            as usize;
        if base + 4 + len > data.len() {
            return Err(format!(
                "ptr={ptr} prefix claims {len} payload bytes, which runs past the {}-byte memory",
                data.len()
            ));
        }
        Ok((data[base + 4..base + 4 + len].to_vec(), 4 + len))
    }

    fn free(&mut self, ptr: i32, len: usize) -> ProbeResult<()> {
        self.call("mag_free", &[Val::I32(ptr), Val::I32(len as i32)])?;
        Ok(())
    }

    fn abi_version(&mut self) -> ProbeResult<u32> {
        Ok(self.call_i32("mag_abi_version", &[])? as u32)
    }

    fn init(&mut self, cfg: &MatchConfig) -> ProbeResult<()> {
        let bytes = abi::encode_config(cfg).map_err(|e| e.to_string())?;
        let ptr = self.alloc_write(&bytes)?;
        self.refuel()?;
        self.call("mag_init", &[Val::I32(ptr), Val::I32(bytes.len() as i32)])?;
        self.free(ptr, bytes.len())?;
        Ok(())
    }

    /// One `mag_step`, returning the raw pointer and the framed payload.
    fn step(
        &mut self,
        tick: Tick,
        inputs: &[(PlayerId, Input)],
    ) -> ProbeResult<(i32, Vec<u8>, usize)> {
        let bytes = abi::encode_step_payload(tick, inputs).map_err(|e| e.to_string())?;
        let ptr = self.alloc_write(&bytes)?;
        self.refuel()?;
        let out = self.call_i32("mag_step", &[Val::I32(ptr), Val::I32(bytes.len() as i32)])?;
        self.free(ptr, bytes.len())?;
        let (payload, framed) = self.read_prefixed(out)?;
        Ok((out, payload, framed))
    }

    /// One `mag_step` whose output is decoded as a `state_hash`.
    fn step_hash(&mut self, tick: Tick, inputs: &[(PlayerId, Input)]) -> ProbeResult<u64> {
        self.step_output(tick, inputs).map(|(hash, _)| hash)
    }

    /// One `mag_step`, decoded to `(state_hash, reject count)`.
    ///
    /// The tick the guest echoes is checked here rather than reported separately:
    /// a guest that disagrees about which tick it just ran has not produced a
    /// usable result, so this is an error and not a measurement.
    fn step_output(
        &mut self,
        tick: Tick,
        inputs: &[(PlayerId, Input)],
    ) -> ProbeResult<(u64, usize)> {
        let (ptr, payload, framed) = self.step(tick, inputs)?;
        let (out, guest_tick) = abi::decode_step_output(&payload).map_err(|e| {
            format!(
                "mag_step payload is not a StepOutput ({e}): {}",
                preview(&payload)
            )
        })?;
        if guest_tick != tick {
            return Err(format!(
                "host asked for tick {tick}, guest reported simulating tick {guest_tick}"
            ));
        }
        self.free(ptr, framed)?;
        Ok((out.state_hash, out.rejects.len()))
    }

    fn snapshot(&mut self) -> ProbeResult<Vec<u8>> {
        self.refuel()?;
        let ptr = self.call_i32("mag_snapshot", &[])?;
        let (payload, framed) = self.read_prefixed(ptr)?;
        self.free(ptr, framed)?;
        Ok(payload)
    }

    /// `mag_restore` with the snapshot bytes exactly as the guest emitted them —
    /// no length prefix, matching every other inbound call.
    fn restore(&mut self, snapshot: &[u8]) -> ProbeResult<()> {
        let ptr = self.alloc_write(snapshot)?;
        self.refuel()?;
        self.call(
            "mag_restore",
            &[Val::I32(ptr), Val::I32(snapshot.len() as i32)],
        )?;
        self.free(ptr, snapshot.len())?;
        Ok(())
    }

    fn view(&mut self, player: PlayerId) -> ProbeResult<Vec<u8>> {
        self.refuel()?;
        let ptr = self.call_i32("mag_view", &[Val::I64(player.as_u64() as i64)])?;
        let (payload, framed) = self.read_prefixed(ptr)?;
        self.free(ptr, framed)?;
        Ok(payload)
    }
}

// ---------------------------------------------------------------------------
// Synthetic inputs
// ---------------------------------------------------------------------------

/// Deterministic synthetic input frames — no RNG, no clock, so the harness is
/// itself reproducible.
fn inputs_for_tick(tick: u64, players: u32) -> Vec<(PlayerId, Input)> {
    (1..=u64::from(players.max(1)))
        .map(|p| {
            let phase = (tick + p) % 4;
            (
                PlayerId::new(p),
                Input {
                    keys: KeyState {
                        forward: phase == 0,
                        left: phase == 1,
                        attack: phase == 2,
                        ..Default::default()
                    },
                    mouse: Default::default(),
                    sequence: tick,
                    // Deliberately non-zero: a conforming module must never
                    // treat a client-supplied timestamp as a clock.
                    timestamp_ms: 1_000 + tick * 16,
                },
            )
        })
        .collect()
}

/// Extract the *reason* a guest call failed.
///
/// `wasmtime::Error` is an `anyhow::Error` whose `Display` is the outermost
/// context — "error while executing at wasm backtrace: …" — not the trap kind.
/// Formatting it directly therefore loses exactly the information a caller needs
/// (this is why [`crate::WasmExecutor`]'s own `classify_trap`, which matches on
/// `err.to_string()`, cannot distinguish fuel from epoch from a plain trap).
/// Downcast to [`wasmtime::Trap`] first, and fall back to the whole error chain.
fn trap_reason(err: &wasmtime::Error) -> String {
    if let Some(trap) = err.downcast_ref::<wasmtime::Trap>() {
        return one_line(&trap.to_string());
    }
    one_line(
        &err.chain()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" / "),
    )
}

/// Collapse a multi-line diagnostic into one report-safe line.
fn one_line(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() > 160 {
        format!("{}…", joined.chars().take(160).collect::<String>())
    } else {
        joined
    }
}

fn preview(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let cut: String = s.chars().take(120).collect();
    if s.chars().count() > 120 {
        format!("{cut}…")
    } else {
        cut
    }
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

/// Run every conformance check against `wasm_bytes`.
///
/// `wasm_bytes` may be a binary `.wasm` module or WebAssembly text — wasmtime
/// accepts both, so a hand-written `.wat` needs no separate assembler.
///
/// Never panics and never returns early: a failure in one check is recorded and
/// dependent checks are marked [`Status::Skip`].
pub fn run(wasm_bytes: &[u8], opts: &ConformanceOptions) -> Report {
    let mut r = Report {
        checks: Vec::new(),
        module_bytes: wasm_bytes.len(),
    };

    // ---- engine: identical configuration to the production host -----------
    let mut engine_cfg = wasmtime::Config::new();
    engine_cfg.consume_fuel(true);
    engine_cfg.epoch_interruption(true);
    let engine = match Engine::new(&engine_cfg) {
        Ok(e) => e,
        Err(e) => {
            r.bad("host.engine", format!("could not build the engine: {e}"));
            return r;
        }
    };

    // ---- module.compiles --------------------------------------------------
    let module = match Module::new(&engine, wasm_bytes) {
        Ok(m) => {
            r.ok(
                "module.compiles",
                "compiles under fuel metering + epoch interruption",
            );
            m
        }
        Err(e) => {
            r.bad("module.compiles", format!("{e}"));
            return r;
        }
    };

    // ---- imports.within_host_surface --------------------------------------
    {
        let mut unknown = Vec::new();
        let mut enosys = Vec::new();
        let mut count = 0;
        for imp in module.imports() {
            count += 1;
            let key = (imp.module(), imp.name());
            if !ALLOWED_IMPORTS
                .iter()
                .any(|(m, n)| *m == key.0 && *n == key.1)
            {
                unknown.push(format!("{}::{}", key.0, key.1));
            } else if ENOSYS_IMPORTS.contains(&key.1) {
                enosys.push(key.1.to_string());
            }
        }
        if unknown.is_empty() {
            r.ok(
                "imports.within_host_surface",
                format!("{count} import(s), all provided by the sandbox linker"),
            );
        } else {
            r.bad(
                "imports.within_host_surface",
                format!(
                    "imports the sandbox does not provide: {}",
                    unknown.join(", ")
                ),
            );
        }
        if enosys.is_empty() {
            r.push(
                "imports.no_clock_or_entropy",
                Status::Pass,
                "declares no clock or OS-entropy import at all",
            );
        } else {
            r.push(
                "imports.no_clock_or_entropy",
                Status::Warn,
                format!(
                    "declares {} — linked but always ENOSYS; any code path that reaches them will fail",
                    enosys.join(", ")
                ),
            );
        }
    }

    // ---- exports.* --------------------------------------------------------
    let mut exports_ok = true;
    {
        let mut found_memory = false;
        for exp in module.exports() {
            if exp.name() == "memory" && exp.ty().memory().is_some() {
                found_memory = true;
            }
        }
        if found_memory {
            r.ok("exports.memory", "linear memory exported as `memory`");
        } else {
            r.bad(
                "exports.memory",
                "no linear memory exported under the name `memory`",
            );
            exports_ok = false;
        }

        for (name, params, results) in REQUIRED_EXPORTS {
            let id = format!("exports.{name}");
            let Some(exp) = module.exports().find(|e| e.name() == *name) else {
                r.bad(&id, "not exported");
                exports_ok = false;
                continue;
            };
            let Some(ft) = exp.ty().func().cloned() else {
                r.bad(&id, "exported, but not a function");
                exports_ok = false;
                continue;
            };
            let got_p: Vec<ValType> = ft.params().collect();
            let got_r: Vec<ValType> = ft.results().collect();
            // `wasmtime::ValType` has no `PartialEq`; its `Display` form is the
            // canonical spec name (`i32`, `i64`, …), which is what we compare.
            let same_types = |got: &[ValType], want: &[ValType]| {
                got.len() == want.len()
                    && got
                        .iter()
                        .zip(want.iter())
                        .all(|(a, b)| a.to_string() == b.to_string())
            };
            let same = same_types(&got_p, params) && same_types(&got_r, results);
            if same {
                r.ok(&id, sig(&got_p, &got_r));
            } else {
                r.bad(
                    &id,
                    format!(
                        "signature is {} but the contract requires {}",
                        sig(&got_p, &got_r),
                        sig(params, results)
                    ),
                );
                exports_ok = false;
            }
        }
    }

    if !exports_ok {
        r.push(
            "runtime.*",
            Status::Skip,
            "runtime checks need the full export table",
        );
        return r;
    }

    // The epoch thread must exist for the epoch checks to mean anything.
    spawn_epoch_thread(engine.clone(), opts.limits.epoch_tick_ms);

    // ---- instantiate + mag_alloc -----------------------------------------
    let mut probe = match Probe::new(&engine, &module, &opts.limits) {
        Ok(p) => {
            r.ok(
                "instance.instantiates",
                format!(
                    "instantiated with a {} MiB memory cap",
                    opts.limits.max_memory_bytes / (1024 * 1024)
                ),
            );
            p
        }
        Err(e) => {
            r.bad("instance.instantiates", e);
            return r;
        }
    };

    match probe.memory_len() {
        Ok(len) => r.push(
            "instance.memory_size",
            Status::Info,
            format!("{len} bytes ({} pages) at instantiation", len / 65_536),
        ),
        Err(e) => r.bad("instance.memory_size", e),
    }

    match probe.abi_version() {
        Ok(v) if v == abi::MAG_ABI_VERSION => r.ok(
            "abi.version_matches_host",
            format!("mag_abi_version() = {v}"),
        ),
        Ok(v) => r.bad(
            "abi.version_matches_host",
            format!(
                "mag_abi_version() = {v}, but this host speaks {}. The sandbox refuses this module at load.",
                abi::MAG_ABI_VERSION
            ),
        ),
        Err(e) => r.bad("abi.version_matches_host", e),
    }

    match probe.alloc_write(&[0xA5; 4096]) {
        Ok(ptr) => r.ok(
            "mag_alloc.writable_in_bounds",
            format!("4096 bytes at ptr={ptr}, host wrote them without trapping"),
        ),
        Err(e) => r.bad("mag_alloc.writable_in_bounds", e),
    }

    // ---- mag_init ---------------------------------------------------------
    let cfg = opts.config(opts.seed);
    match probe.init(&cfg) {
        Ok(()) => r.ok(
            "mag_init.accepts_match_config",
            format!(
                "accepted {} bytes of MatchConfig JSON (seed={:#x})",
                abi::encode_config(&cfg).map(|b| b.len()).unwrap_or(0),
                opts.seed
            ),
        ),
        Err(e) => {
            r.bad("mag_init.accepts_match_config", e);
            return r;
        }
    }

    // ---- mag_step framing + payload --------------------------------------
    // Tick 1: the first step of a match. Tick 0 is the state mag_init produced.
    let first_inputs = inputs_for_tick(1, opts.players);
    match probe.step(1, &first_inputs) {
        Ok((ptr, payload, framed)) => {
            r.ok(
                "mag_step.length_prefix",
                format!(
                    "ptr={ptr}, prefix declares {} payload bytes, all inside linear memory",
                    payload.len()
                ),
            );
            match serde_json::from_slice::<serde_json::Value>(&payload) {
                Ok(_) => r.ok(
                    "mag_step.payload_is_json",
                    format!("{} bytes of valid JSON", payload.len()),
                ),
                Err(e) => r.bad(
                    "mag_step.payload_is_json",
                    format!("{e}: {}", preview(&payload)),
                ),
            }
            match abi::decode_step_output(&payload) {
                Ok((out, guest_tick)) => {
                    r.ok(
                        "mag_step.payload_is_step_output",
                        format!(
                            "state_hash={}, {} reject(s)",
                            out.state_hash,
                            out.rejects.len()
                        ),
                    );
                    if guest_tick == 1 {
                        r.ok(
                            "mag_step.echoes_the_requested_tick",
                            "host asked for tick 1, guest reported tick 1",
                        );
                    } else {
                        r.bad(
                            "mag_step.echoes_the_requested_tick",
                            format!(
                                "host asked for tick 1, guest reported tick {guest_tick} — the two sides disagree about which moment this is"
                            ),
                        );
                    }
                }
                Err(e) => {
                    r.bad(
                        "mag_step.payload_is_step_output",
                        format!("{e}: {}", preview(&payload)),
                    );
                    r.push(
                        "mag_step.echoes_the_requested_tick",
                        Status::Skip,
                        "StepOutput did not decode",
                    );
                }
            }
            match probe.free(ptr, framed) {
                Ok(()) => r.ok(
                    "mag_free.accepts_returned_buffer",
                    format!("mag_free(ptr={ptr}, len={framed}) did not trap"),
                ),
                Err(e) => r.bad("mag_free.accepts_returned_buffer", e),
            }
        }
        Err(e) => {
            r.bad("mag_step.length_prefix", e);
            for id in [
                "mag_step.payload_is_json",
                "mag_step.payload_is_step_output",
                "mag_step.echoes_the_requested_tick",
                "mag_free.accepts_returned_buffer",
            ] {
                r.push(
                    id,
                    Status::Skip,
                    "mag_step did not produce a readable buffer",
                );
            }
        }
    }
    // ---- mag_snapshot / mag_view framing ---------------------------------
    match probe.snapshot() {
        Ok(payload) => {
            r.ok(
                "mag_snapshot.length_prefix",
                format!("{} payload bytes inside linear memory", payload.len()),
            );
            match serde_json::from_slice::<serde_json::Value>(&payload) {
                Ok(_) => r.ok(
                    "mag_snapshot.payload_is_json",
                    format!("valid JSON: {}", preview(&payload)),
                ),
                // The host treats snapshot bytes as opaque, so a non-JSON
                // encoding is legal — but it forfeits host-side tooling.
                Err(e) => r.push(
                    "mag_snapshot.payload_is_json",
                    Status::Warn,
                    format!("not JSON ({e}) — legal, but host-side replay inspection and delta computation cannot read it: {}", preview(&payload)),
                ),
            }
        }
        Err(e) => {
            r.bad("mag_snapshot.length_prefix", e);
            r.push(
                "mag_snapshot.payload_is_json",
                Status::Skip,
                "no readable snapshot buffer",
            );
        }
    }

    match probe.view(PlayerId::new(1)) {
        Ok(payload) => {
            r.ok(
                "mag_view.length_prefix",
                format!("{} payload bytes inside linear memory", payload.len()),
            );
            match serde_json::from_slice::<serde_json::Value>(&payload) {
                Ok(_) => r.ok(
                    "mag_view.payload_is_json",
                    format!("valid JSON: {}", preview(&payload)),
                ),
                Err(e) => r.push(
                    "mag_view.payload_is_json",
                    Status::Warn,
                    format!("not JSON ({e}) — legal, but only a client that shares the module's encoding can read it: {}", preview(&payload)),
                ),
            }
        }
        Err(e) => {
            r.bad("mag_view.length_prefix", e);
            r.push(
                "mag_view.payload_is_json",
                Status::Skip,
                "no readable view buffer",
            );
        }
    }

    drop(probe);

    // ---- determinism across two fresh instances --------------------------
    let run_hashes = |seed: u64, ticks: u64, with_inputs: bool| -> ProbeResult<Vec<u64>> {
        let mut p = Probe::new(&engine, &module, &opts.limits)?;
        p.init(&opts.config(seed))?;
        let mut out = Vec::with_capacity(ticks as usize);
        for t in 1..=ticks {
            let inputs = if with_inputs {
                inputs_for_tick(t, opts.players)
            } else {
                Vec::new()
            };
            out.push(p.step_hash(t, &inputs)?);
        }
        Ok(out)
    };

    let a = run_hashes(opts.seed, opts.ticks, true);
    let b = run_hashes(opts.seed, opts.ticks, true);
    match (&a, &b) {
        (Ok(a), Ok(b)) if a == b => r.ok(
            "mag_step.deterministic",
            format!(
                "{} tick(s), two fresh instances, identical state_hash sequence (last={})",
                opts.ticks,
                a.last().copied().unwrap_or(0)
            ),
        ),
        (Ok(a), Ok(b)) => {
            let at = a.iter().zip(b.iter()).position(|(x, y)| x != y);
            r.bad(
                "mag_step.deterministic",
                format!(
                    "state_hash sequences diverge at tick {} ({:?} vs {:?})",
                    at.map(|i| i + 1).unwrap_or(0),
                    at.and_then(|i| a.get(i)),
                    at.and_then(|i| b.get(i))
                ),
            );
        }
        (Err(e), _) | (_, Err(e)) => r.bad("mag_step.deterministic", e.clone()),
    }

    // ---- the module actually advances, consumes the seed, sees inputs ----
    if let Ok(hs) = &a {
        let distinct = {
            let mut v = hs.clone();
            v.sort_unstable();
            v.dedup();
            v.len()
        };
        r.push(
            "mag_step.state_advances",
            if distinct > 1 {
                Status::Pass
            } else {
                Status::Warn
            },
            format!(
                "{distinct} distinct state_hash value(s) over {} tick(s)",
                opts.ticks
            ),
        );

        match run_hashes(opts.alt_seed, opts.ticks, true) {
            Ok(alt) if &alt != hs => r.push(
                "determinism.seed_is_consumed",
                Status::Pass,
                format!("a different MatchConfig.seed ({:#x}) yields a different hash sequence", opts.alt_seed),
            ),
            Ok(_) => r.push(
                "determinism.seed_is_consumed",
                Status::Warn,
                "changing MatchConfig.seed changed nothing — the module ignores the only entropy it is given",
            ),
            Err(e) => r.push("determinism.seed_is_consumed", Status::Skip, e),
        }

        // A module that reads its inputs either acts on them or refuses them.
        // Requiring the state to change would fail a module whose whole answer to
        // these inputs is a legitimate rejection, so a reject counts as evidence
        // too. Only a module that does *neither* is ignoring the payload.
        match (
            run_hashes(opts.seed, opts.ticks, false),
            rejects_seen(&engine, &module, opts),
        ) {
            (Ok(empty), rejected) if &empty != hs => r.push(
                "mag_step.inputs_are_read",
                Status::Pass,
                format!(
                    "submitting player inputs changes the resulting state ({rejected} reject(s) reported)"
                ),
            ),
            (Ok(_), rejected) if rejected > 0 => r.push(
                "mag_step.inputs_are_read",
                Status::Pass,
                format!(
                    "state is unchanged, but the module reported {rejected} reject(s) — it read the payload and refused it"
                ),
            ),
            (Ok(_), _) => r.push(
                "mag_step.inputs_are_read",
                Status::Warn,
                format!(
                    "{} tick(s) with {} players' inputs produce byte-identical hashes to {} empty ticks and report no rejects — the module is not reading the mag_step payload",
                    opts.ticks, opts.players, opts.ticks
                ),
            ),
            (Err(e), _) => r.push("mag_step.inputs_are_read", Status::Skip, e),
        }
    }

    // ---- snapshot / restore round-trip -----------------------------------
    snapshot_restore_checks(&mut r, &engine, &module, opts);

    // ---- resource limits reach the guest ---------------------------------
    limit_checks(&mut r, &engine, &module, opts);

    r
}

/// Total rejects the module reports across a fresh run with inputs.
///
/// Used only as corroborating evidence that the `mag_step` payload was read;
/// zero is not itself a failure.
fn rejects_seen(engine: &Engine, module: &Module, opts: &ConformanceOptions) -> usize {
    let mut total = 0;
    let Ok(mut p) = Probe::new(engine, module, &opts.limits) else {
        return 0;
    };
    if p.init(&opts.config(opts.seed)).is_err() {
        return 0;
    }
    for t in 1..=opts.ticks {
        match p.step_output(t, &inputs_for_tick(t, opts.players)) {
            Ok((_, rejects)) => total += rejects,
            Err(_) => return total,
        }
    }
    total
}

/// Two round-trip properties, checked separately because they fail separately:
///
/// 1. `mag_restore(mag_snapshot())` is a no-op on state — snapshot again and
///    the bytes match.
/// 2. A snapshot restored into a *fresh* instance resumes the same trajectory:
///    stepping both forward with identical inputs yields identical hashes.
///
/// (2) is the property replay verification and shard migration actually depend
/// on; (1) alone can pass on a module that ignores `mag_restore` entirely.
fn snapshot_restore_checks(
    r: &mut Report,
    engine: &Engine,
    module: &Module,
    opts: &ConformanceOptions,
) {
    let half = (opts.ticks / 2).max(1);

    let mut a = match Probe::new(engine, module, &opts.limits) {
        Ok(p) => p,
        Err(e) => {
            r.push("mag_restore.*", Status::Skip, e);
            return;
        }
    };
    if let Err(e) = a.init(&opts.config(opts.seed)) {
        r.push("mag_restore.*", Status::Skip, e);
        return;
    }
    for t in 1..=half {
        if let Err(e) = a.step_hash(t, &inputs_for_tick(t, opts.players)) {
            r.push("mag_restore.*", Status::Skip, e);
            return;
        }
    }
    let snap = match a.snapshot() {
        Ok(s) => s,
        Err(e) => {
            r.push("mag_restore.*", Status::Skip, e);
            return;
        }
    };

    // (1) idempotent restore of the module's own snapshot.
    match a.restore(&snap).and_then(|()| a.snapshot()) {
        Ok(again) if again == snap => r.ok(
            "mag_restore.snapshot_is_idempotent",
            format!(
                "restore(snapshot()) then snapshot() reproduces the same {} bytes",
                snap.len()
            ),
        ),
        Ok(again) => r.bad(
            "mag_restore.snapshot_is_idempotent",
            format!(
                "snapshot changed across a restore of itself: {} -> {}",
                preview(&snap),
                preview(&again)
            ),
        ),
        Err(e) => r.bad("mag_restore.snapshot_is_idempotent", e),
    }

    // (2) a fresh instance restored from the snapshot resumes the trajectory.
    //
    // Ticks continue from where the snapshot left off, which is also what makes
    // this check able to catch a module that restored the state but not the tick:
    // it will report the wrong tick back and `step_output` will refuse it.
    let tail: Vec<(Tick, Vec<(PlayerId, Input)>)> = ((half + 1)..=opts.ticks)
        .map(|t| (t, inputs_for_tick(t, opts.players)))
        .collect();

    let continued: ProbeResult<Vec<u64>> = tail.iter().map(|(t, i)| a.step_hash(*t, i)).collect();

    let resumed: ProbeResult<Vec<u64>> = (|| {
        let mut b = Probe::new(engine, module, &opts.limits)?;
        b.init(&opts.config(opts.seed))?;
        b.restore(&snap)?;
        tail.iter().map(|(t, i)| b.step_hash(*t, i)).collect()
    })();
    match (continued, resumed) {
        (Ok(x), Ok(y)) if x == y && !x.is_empty() => r.ok(
            "mag_restore.resumes_trajectory",
            format!(
                "snapshot at tick {half} restored into a fresh instance; the next {} tick(s) hash identically",
                x.len()
            ),
        ),
        (Ok(x), Ok(y)) => r.bad(
            "mag_restore.resumes_trajectory",
            format!(
                "after restoring the tick-{half} snapshot into a fresh instance the trajectories differ: {x:?} vs {y:?}"
            ),
        ),
        (Err(e), _) | (_, Err(e)) => r.bad("mag_restore.resumes_trajectory", e),
    }
}

/// Fuel, memory and epoch are host-side mechanisms, but whether they *bind this
/// module* is a property of the module. All three are checked by observing that
/// the module is actually stopped.
fn limit_checks(r: &mut Report, engine: &Engine, module: &Module, opts: &ConformanceOptions) {
    // ---- fuel ------------------------------------------------------------
    let starved = LimitsConfig {
        fuel_per_step: 1,
        ..opts.limits.clone()
    };
    match Probe::new(engine, module, &starved) {
        Ok(mut p) => {
            // mag_init may itself run out of fuel; either way the module must
            // be stopped rather than allowed to complete a step.
            let outcome = p
                .init(&opts.config(opts.seed))
                .and_then(|()| p.step_hash(1, &[]));
            match outcome {
                Err(e) if e.contains("fuel") => r.ok(
                    "limits.fuel_binds_module",
                    "a 1-unit fuel budget stops the module: all fuel consumed",
                ),
                Err(e) => r.push(
                    "limits.fuel_binds_module",
                    Status::Pass,
                    format!("a 1-unit fuel budget stops the module: {e}"),
                ),
                Ok(h) => r.bad(
                    "limits.fuel_binds_module",
                    format!("completed mag_init + mag_step on 1 fuel unit (state_hash={h}) — the module is not fuel-metered"),
                ),
            }
        }
        Err(e) => r.push("limits.fuel_binds_module", Status::Skip, e),
    }

    // ---- memory ----------------------------------------------------------
    // The limiter is consulted on the module's *initial* memory as well as on
    // growth, so a cap below what the module declares must refuse instantiation.
    let declared_pages = module
        .exports()
        .find_map(|e| e.ty().memory().map(|m| m.minimum()))
        .unwrap_or(0);
    let tiny = LimitsConfig {
        max_memory_bytes: 64 * 1024, // one page
        ..opts.limits.clone()
    };
    if declared_pages <= 1 {
        r.push(
            "limits.memory_binds_module",
            Status::Skip,
            format!("module declares {declared_pages} page(s); no cap below that to test with"),
        );
    } else {
        match Probe::new(engine, module, &tiny) {
            Err(_) => r.ok(
                "limits.memory_binds_module",
                format!(
                    "module declares {declared_pages} pages; a 1-page cap refuses instantiation"
                ),
            ),
            Ok(_) => r.bad(
                "limits.memory_binds_module",
                format!("module declares {declared_pages} pages yet instantiated under a 1-page cap — the memory cap does not bind it"),
            ),
        }
    }

    // ---- epoch -----------------------------------------------------------
    // A deadline of zero epochs is already expired, so the very first
    // epoch check inside the guest must trap. This proves the interrupt
    // reaches this module's code without needing a module that spins.
    match Probe::new(engine, module, &opts.limits) {
        Ok(mut p) => {
            let init = p.init(&opts.config(opts.seed));
            p.store.set_epoch_deadline(0);
            p.epochs_per_step = 0;
            let outcome = init.and_then(|()| p.step_hash(1, &[]));
            match outcome {
                Err(e) if e.contains("epoch") || e.contains("interrupt") => r.ok(
                    "limits.epoch_binds_module",
                    "an already-expired epoch deadline interrupts the module",
                ),
                Err(e) => r.push(
                    "limits.epoch_binds_module",
                    Status::Info,
                    format!("stopped, but not by the epoch deadline: {e}"),
                ),
                Ok(h) => r.push(
                    "limits.epoch_binds_module",
                    Status::Warn,
                    format!("mag_step completed (state_hash={h}) with an expired epoch deadline — this module reaches no epoch check inside a step, so wall-clock interruption cannot stop it mid-step"),
                ),
            }
        }
        Err(e) => r.push("limits.epoch_binds_module", Status::Skip, e),
    }
}

fn sig(params: &[ValType], results: &[ValType]) -> String {
    let p: Vec<String> = params.iter().map(|t| t.to_string()).collect();
    let rr: Vec<String> = results.iter().map(|t| t.to_string()).collect();
    if rr.is_empty() {
        format!("({})", p.join(", "))
    } else {
        format!("({}) -> {}", p.join(", "), rr.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A stub with every required export, so a test can break exactly one thing.
    /// `$version` and `$view_param` are the two knobs the tests below turn.
    macro_rules! stub {
        (version: $version:expr, view_param: $view_param:expr, step_ptr: $step_ptr:expr) => {
            format!(
                r#"(module
                    (memory (export "memory") 1)
                    (func (export "mag_abi_version") (result i32) (i32.const {v}))
                    (func (export "mag_alloc") (param i32) (result i32) (i32.const 0))
                    (func (export "mag_free") (param i32 i32))
                    (func (export "mag_init") (param i32 i32))
                    (func (export "mag_step") (param i32 i32) (result i32) (i32.const {s}))
                    (func (export "mag_snapshot") (result i32) (i32.const 0))
                    (func (export "mag_restore") (param i32 i32))
                    (func (export "mag_view") (param {p}) (result i32) (i32.const 0)))"#,
                v = $version,
                p = $view_param,
                s = $step_ptr,
            )
            .into_bytes()
        };
    }

    #[test]
    fn required_exports_table_matches_the_contract() {
        // Eight: the seven call exports plus the version declaration.
        assert_eq!(REQUIRED_EXPORTS.len(), 8);
        assert!(REQUIRED_EXPORTS
            .iter()
            .any(|(n, ..)| *n == "mag_abi_version"));
    }

    #[test]
    fn garbage_is_reported_as_a_compile_failure_not_a_panic() {
        let r = run(b"definitely not wasm", &ConformanceOptions::default());
        assert!(!r.is_conforming());
        assert_eq!(r.checks[0].id, "module.compiles");
        assert_eq!(r.checks[0].status, Status::Fail);
    }

    #[test]
    fn a_module_with_no_mag_exports_fails_every_export_check() {
        let r = run(
            br#"(module (memory (export "memory") 1))"#,
            &ConformanceOptions::default(),
        );
        assert!(!r.is_conforming());
        // memory passes, all eight functions fail.
        assert_eq!(r.count(Status::Fail), 8);
        assert!(r
            .checks
            .iter()
            .any(|c| c.id == "exports.memory" && c.status == Status::Pass));
    }

    #[test]
    fn a_wrong_signature_is_reported_as_a_signature_mismatch() {
        // mag_view taking i32 instead of i64.
        let wat = stub!(version: 1, view_param: "i32", step_ptr: 0);
        let r = run(&wat, &ConformanceOptions::default());
        let c = r
            .checks
            .iter()
            .find(|c| c.id == "exports.mag_view")
            .expect("mag_view check present");
        assert_eq!(c.status, Status::Fail);
        assert!(c.detail.contains("i64"), "detail was: {}", c.detail);
    }

    #[test]
    fn a_mismatched_abi_version_is_reported_as_a_failure() {
        let wat = stub!(version: 99, view_param: "i64", step_ptr: 0);
        let r = run(&wat, &ConformanceOptions::default());
        let c = r
            .checks
            .iter()
            .find(|c| c.id == "abi.version_matches_host")
            .expect("version check present");
        assert_eq!(c.status, Status::Fail, "detail: {}", c.detail);
        assert!(c.detail.contains("99"), "detail was: {}", c.detail);
    }

    #[test]
    fn the_declared_version_is_reported_when_it_matches() {
        let wat = stub!(version: 1, view_param: "i64", step_ptr: 0);
        let r = run(&wat, &ConformanceOptions::default());
        let c = r
            .checks
            .iter()
            .find(|c| c.id == "abi.version_matches_host")
            .expect("version check present");
        assert_eq!(c.status, Status::Pass, "detail: {}", c.detail);
    }

    #[test]
    fn an_out_of_bounds_length_prefix_is_caught() {
        // mag_step returns a pointer one byte short of a readable prefix.
        let wat = stub!(version: 1, view_param: "i64", step_ptr: 65533);
        let r = run(&wat, &ConformanceOptions::default());
        let c = r
            .checks
            .iter()
            .find(|c| c.id == "mag_step.length_prefix")
            .expect("length_prefix check present");
        assert_eq!(c.status, Status::Fail, "detail: {}", c.detail);
    }

    #[test]
    fn the_wat_reference_module_conforms() {
        let wat = include_bytes!("../conformance/reference.wat");
        let r = run(wat, &ConformanceOptions::default());
        println!("{}", r.render());
        assert!(
            r.is_conforming(),
            "reference.wat must conform:\n{}",
            r.render()
        );
    }
}
