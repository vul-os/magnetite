//! `mag_*` sandbox ABI exports — only compiled when `--features wasm`.
//!
//! The Magnetite sandbox (`magnetite-sandbox`) loads a `wasm32-wasip1` game
//! module and calls these exports to drive the authoritative game loop:
//!
//! ```text
//! mag_abi_version() -> u32            // declared ABI version; host refuses a mismatch
//! mag_alloc(len: u32) -> u32          // allocate `len` bytes, return ptr
//! mag_free(ptr: u32, len: u32)        // deallocate (bump alloc = no-op)
//! mag_init(cfg_ptr: u32, cfg_len: u32)// initialise game from JSON MatchConfig
//! mag_step(p_ptr: u32, p_len: u32) -> u32  // step; return ptr to StepOutput JSON
//! mag_snapshot() -> u32               // return ptr to Snapshot JSON
//! mag_restore(ptr: u32, len: u32)     // restore game from bare Snapshot JSON
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
//! The host reads `length` bytes of JSON after the 4-byte header. The memory is
//! owned by the module (static bump allocator) and is valid only until the next
//! call to **any** `mag_*` export; the host must copy the bytes immediately.
//!
//! Buffers the host passes *in* are never prefixed — `len` is already a
//! parameter. That includes `mag_restore`.
//!
//! `mag_step`'s payload is `{"tick": N, "inputs": [...]}` and its reply echoes
//! the tick, so the host can check that both sides mean the same moment.
//!
//! The normative statement of all of this is `site/docs/sandbox-abi.md`.
//!
//! ## Bump allocator
//!
//! We use a trivial monotone bump allocator backed by a 4 MiB static buffer.
//! `mag_free` is a no-op — the host never needs to free individually; the
//! entire bump arena is reset inside each `mag_step` call, *after* that tick's
//! inputs have been decoded and before the output is allocated.

#[cfg(feature = "wasm")]
mod inner {
    use crate::game::ArenaShooter;
    use crate::types::ArenaSnapshot;
    use magnetite_sdk::authority::{GameExecutor, MatchConfig, NativeExecutor};
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

    /// Ticks stepped since the last `mag_init` or `mag_restore`.
    ///
    /// The host tells us the tick in every `mag_step` payload, so this is not a
    /// counter — it is the last tick we were given. It is kept so that
    /// `mag_step` can refuse a mis-sequenced step, and it is re-seeded from
    /// `ArenaSnapshot::tick` by `mag_restore`.
    static mut CURRENT_TICK: u64 = 0;

    /// One `(player, input)` pair as the host encodes it.
    ///
    /// A derived struct `Deserialize` accepts a JSON **object** (`{"player_id":
    /// 1, "input": {…}}`, which is what the host writes) *and* a JSON **array**
    /// (`[1, {…}]`, the shape a serialised `(PlayerId, Input)` tuple produces).
    /// Both are therefore decoded here, which is why this type is a struct rather
    /// than a tuple.
    #[derive(serde::Deserialize)]
    struct InputFrame {
        player_id: u64,
        input: Input,
    }

    /// The whole `mag_step` payload: `{"tick": N, "inputs": [...]}`.
    #[derive(serde::Deserialize)]
    struct StepPayload {
        tick: u64,
        inputs: Vec<InputFrame>,
    }

    /// What `mag_step` returns, including the tick we actually simulated so the
    /// host can check that we and it agree about which moment this is.
    #[derive(serde::Serialize)]
    struct StepReply {
        rejects: Vec<GuestReject>,
        state_hash: u64,
        tick: u64,
    }

    #[derive(serde::Serialize)]
    struct GuestReject {
        player_id: u64,
        reason: magnetite_sdk::authority::RejectReason,
    }

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

    /// The ABI version this module implements.
    ///
    /// The host calls this before exchanging any payload and refuses the module
    /// unless it matches `magnetite_sandbox::abi::MAG_ABI_VERSION`. Keep it a
    /// literal: it describes the shape of the calls below, so it must change in
    /// the same commit they do.
    #[no_mangle]
    pub extern "C" fn mag_abi_version() -> u32 {
        1
    }

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

    /// Advance one tick given the host's JSON step payload.
    ///
    /// Returns a pointer to a length-prefixed JSON `StepOutput`, including the
    /// tick that was simulated so the host can verify we agree about it.
    ///
    /// # Payload shape
    ///
    /// `{"tick": N, "inputs": [{"player_id": N, "input": {…}}]}`, matching
    /// `magnetite_sandbox::abi::StepPayload`. Each frame decodes from either the
    /// object form above or the two-element array form `[1, {…}]` — see
    /// [`InputFrame`].
    ///
    /// # Safety
    /// `payload_ptr` must point to `payload_len` readable bytes of valid JSON.
    #[no_mangle]
    pub unsafe extern "C" fn mag_step(payload_ptr: u32, payload_len: u32) -> u32 {
        // Consume the host's buffer BEFORE resetting the arena: the reset makes
        // the region reusable, and the output allocated below may overlap it.
        let bytes = core::slice::from_raw_parts(payload_ptr as *const u8, payload_len as usize);
        let payload: StepPayload = match serde_json::from_slice(bytes) {
            Ok(payload) => payload,
            // Trapping is the loud failure the host can classify. Defaulting to
            // an empty list here is what hid this bug: a module that drops every
            // input is indistinguishable from a match where nobody pressed a key.
            Err(e) => panic!("mag_step: undecodable step payload ({e})"),
        };

        // The tick comes from the host now, so a mis-sequenced step is something
        // this module can see and refuse rather than silently absorb. Tick 0 is
        // the initial state; the first step is tick 1.
        assert!(
            payload.tick > CURRENT_TICK,
            "mag_step: tick {} is not ahead of the last tick simulated ({})",
            payload.tick,
            CURRENT_TICK
        );
        CURRENT_TICK = payload.tick;

        let inputs: Vec<(PlayerId, Input)> = payload
            .inputs
            .into_iter()
            .map(|f| (PlayerId::new(f.player_id), f.input))
            .collect();

        bump_reset();

        let exec = EXECUTOR.as_mut().expect("mag_step: mag_init not called");
        let out = exec.step(CURRENT_TICK, &inputs);
        let reply = StepReply {
            rejects: out
                .rejects
                .into_iter()
                .map(|(pid, reason)| GuestReject {
                    player_id: pid.as_u64(),
                    reason,
                })
                .collect(),
            state_hash: out.state_hash,
            tick: CURRENT_TICK,
        };
        let json = serde_json::to_vec(&reply).unwrap_or_default();
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

    /// Restore game state from a bare JSON-encoded `ArenaSnapshot`.
    ///
    /// # Framing
    ///
    /// The payload is **not** length-prefixed — `len` already carries the length,
    /// so a prefix would only duplicate it. This matches `mag_init` and
    /// `mag_step`: no inbound call is prefixed, only the buffers this module
    /// returns are.
    ///
    /// Two earlier revisions got this wrong in opposite directions. The first
    /// handed the whole buffer, prefix bytes and all, to a JSON parser and
    /// discarded the error, so a restore silently did nothing. The second
    /// accepted either framing, because the host and wibbly's browser driver
    /// disagreed about which one applied. The redundancy was the root cause and
    /// it is now gone.
    ///
    /// # Safety
    /// `ptr` must point to `len` readable bytes.
    #[no_mangle]
    pub unsafe extern "C" fn mag_restore(ptr: u32, len: u32) {
        let payload = core::slice::from_raw_parts(ptr as *const u8, len as usize);

        // The tick is authoritative state and lives in the snapshot. Read it
        // first so the counter is right even though the executor owns the game.
        // (Two parses of a small payload; clarity beats a byte-saving here.)
        let snap: ArenaSnapshot =
            serde_json::from_slice(payload).expect("mag_restore: payload is not an ArenaSnapshot");

        let exec = EXECUTOR.as_mut().expect("mag_restore: mag_init not called");
        exec.try_restore(payload)
            .expect("mag_restore: executor rejected the snapshot");

        // Adopting the snapshot's tick is what makes the next mag_step's
        // monotonicity check meaningful and what lets the host verify, via the
        // echoed tick, that this module really resumed where the snapshot left off.
        CURRENT_TICK = snap.tick;
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
