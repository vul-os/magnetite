//! `mag_*` sandbox ABI exports — only compiled when `--features wasm`.
//!
//! The Magnetite sandbox (`magnetite-sandbox`) loads a `wasm32-wasip1` game
//! module and calls these exports to drive the authoritative game loop:
//!
//! ```text
//! mag_alloc(len: u32) -> u32          // allocate `len` bytes, return ptr
//! mag_free(ptr: u32, len: u32)        // deallocate (bump alloc = no-op)
//! mag_init(cfg_ptr: u32, cfg_len: u32)// initialise game from JSON MatchConfig
//! mag_step(in_ptr: u32, in_len: u32) -> u32  // step; return ptr to StepOutput JSON
//! mag_snapshot() -> u32               // return ptr to Snapshot JSON
//! mag_restore(ptr: u32, len: u32)     // restore game from Snapshot JSON
//! mag_view(player_id: u64) -> u32     // return ptr to View JSON
//! ```
//!
//! ## Wire format
//!
//! Every `-> u32` return is a pointer into linear memory at which a
//! **length-prefixed JSON blob** lives:
//!
//! ```text
//! [length: u32 little-endian][JSON bytes...]
//! ```
//!
//! The host reads `length` bytes of JSON after the 4-byte header. The memory
//! is owned by the module (static bump allocator) until the next call to the
//! same export; the host must copy the bytes immediately.
//!
//! ## Bump allocator
//!
//! We use a trivial monotone bump allocator backed by a 4 MiB static buffer.
//! `mag_free` is a no-op — the host never needs to free individually; the
//! entire bump arena is reset at the start of each `mag_step` call.

#[cfg(feature = "wasm")]
mod inner {
    use crate::game::ArenaShooter;
    use magnetite_sdk::authority::{AuthoritativeGame, GameExecutor, MatchConfig, NativeExecutor};
    use magnetite_sdk::input::Input;
    use magnetite_sdk::state::PlayerId;

    // ---------------------------------------------------------------------------
    // Bump allocator (static, 4 MiB)
    // ---------------------------------------------------------------------------

    const BUMP_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

    static mut BUMP_BUF: [u8; BUMP_SIZE] = [0u8; BUMP_SIZE];
    static mut BUMP_PTR: usize = 0;

    /// Reset the bump arena. Called at the start of each `mag_step`.
    unsafe fn bump_reset() {
        BUMP_PTR = 0;
    }

    /// Allocate `len` bytes from the bump arena. Panics if out of space.
    unsafe fn bump_alloc(len: usize) -> *mut u8 {
        let aligned = (BUMP_PTR + 7) & !7; // 8-byte alignment
        let next = aligned + len;
        assert!(next <= BUMP_SIZE, "mag bump allocator OOM");
        BUMP_PTR = next;
        BUMP_BUF.as_mut_ptr().add(aligned)
    }

    // ---------------------------------------------------------------------------
    // Game singleton
    // ---------------------------------------------------------------------------

    static mut EXECUTOR: Option<NativeExecutor<ArenaShooter>> = None;
    static mut CFG: Option<MatchConfig> = None;

    /// Write a length-prefixed JSON payload into the bump arena and return its pointer.
    unsafe fn write_json(bytes: &[u8]) -> u32 {
        let len = bytes.len() as u32;
        let total = 4 + bytes.len();
        let ptr = bump_alloc(total);
        // Write 4-byte little-endian length prefix.
        ptr.copy_from_nonoverlapping(len.to_le_bytes().as_ptr(), 4);
        // Write JSON payload.
        ptr.add(4)
            .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
        ptr as u32
    }

    // ---------------------------------------------------------------------------
    // ABI exports
    // ---------------------------------------------------------------------------

    /// Allocate `len` bytes from the bump arena. Returns the pointer.
    ///
    /// # Safety
    /// Called by the Wasmtime host to obtain writable memory for inputs.
    #[no_mangle]
    pub unsafe extern "C" fn mag_alloc(len: u32) -> u32 {
        bump_alloc(len as usize) as u32
    }

    /// Free a previously allocated pointer. No-op for the bump allocator.
    ///
    /// # Safety
    /// The host calls this after reading output; safe to ignore with bump alloc.
    #[no_mangle]
    pub unsafe extern "C" fn mag_free(_ptr: u32, _len: u32) {}

    /// Initialise the game from a JSON-encoded [`MatchConfig`].
    ///
    /// # Safety
    /// `cfg_ptr` must point to `cfg_len` readable bytes of valid JSON.
    #[no_mangle]
    pub unsafe extern "C" fn mag_init(cfg_ptr: u32, cfg_len: u32) {
        let bytes = core::slice::from_raw_parts(cfg_ptr as *const u8, cfg_len as usize);
        let cfg: MatchConfig =
            serde_json::from_slice(bytes).expect("mag_init: invalid MatchConfig JSON");
        EXECUTOR = Some(NativeExecutor::<ArenaShooter>::new(cfg.clone()));
        CFG = Some(cfg);
        bump_reset();
    }

    /// Advance one tick given a JSON-encoded `Vec<(PlayerId, Input)>`.
    ///
    /// Returns a pointer to a length-prefixed JSON-encoded [`StepOutput`].
    ///
    /// # Safety
    /// `inputs_ptr` must point to `inputs_len` readable bytes of valid JSON.
    #[no_mangle]
    pub unsafe extern "C" fn mag_step(inputs_ptr: u32, inputs_len: u32) -> u32 {
        bump_reset();
        let bytes = core::slice::from_raw_parts(inputs_ptr as *const u8, inputs_len as usize);
        let inputs: Vec<(PlayerId, Input)> = serde_json::from_slice(bytes).unwrap_or_default();

        // Derive tick from inputs length (the host passes tick in the outer
        // protocol; for the ABI we use the state_hash output, and the host
        // tracks the tick). Use a sentinel tick of 0 if we have no way to
        // know — in practice the runtime wraps this and tracks tick externally.
        // A production host would pass the tick as a separate argument; this
        // reference implementation derives it by counting step calls.
        static mut CURRENT_TICK: u64 = 0;
        CURRENT_TICK += 1;

        let exec = EXECUTOR.as_mut().expect("mag_step: mag_init not called");
        let out = exec.step(CURRENT_TICK, &inputs);
        let json = serde_json::to_vec(&out).unwrap_or_default();
        write_json(&json)
    }

    /// Serialise the current game state.
    ///
    /// Returns a pointer to a length-prefixed JSON-encoded `ArenaSnapshot`.
    ///
    /// # Safety
    /// Must be called after `mag_init`.
    #[no_mangle]
    pub unsafe extern "C" fn mag_snapshot() -> u32 {
        bump_reset();
        let exec = EXECUTOR
            .as_ref()
            .expect("mag_snapshot: mag_init not called");
        let bytes = exec.snapshot();
        write_json(&bytes)
    }

    /// Restore game state from a JSON-encoded `ArenaSnapshot`.
    ///
    /// # Safety
    /// `ptr` must point to `len` readable bytes of valid JSON.
    #[no_mangle]
    pub unsafe extern "C" fn mag_restore(ptr: u32, len: u32) {
        let bytes = core::slice::from_raw_parts(ptr as *const u8, len as usize);
        let exec = EXECUTOR.as_mut().expect("mag_restore: mag_init not called");
        exec.restore(bytes);
    }

    /// Serialise the interest-filtered view for `player_id`.
    ///
    /// Returns a pointer to a length-prefixed JSON-encoded `ArenaView`.
    ///
    /// # Safety
    /// Must be called after `mag_init`.
    #[no_mangle]
    pub unsafe extern "C" fn mag_view(player_id: u64) -> u32 {
        bump_reset();
        let exec = EXECUTOR.as_ref().expect("mag_view: mag_init not called");
        let player = PlayerId::new(player_id);
        let bytes = exec.view_for(player);
        write_json(&bytes)
    }
}
