//! # game-template-authoritative
//!
//! **Reference authoritative arena shooter** for the Magnetite platform.
//!
//! This crate implements [`AuthoritativeGame`] for a small top-down arena
//! shooter. It is the canonical example of how to write a game that:
//!
//! * Is **100% server-authoritative** — clients send inputs, the server runs
//!   the simulation, clients receive views.
//! * Is **deterministic** — same `(state, commands, tick, seed)` → identical
//!   result on every platform and every run.
//! * Supports **replay verification** — state hashes match on re-simulation.
//! * Compiles to **`wasm32-wasip1`** (with `--features wasm`) and exports the
//!   `mag_*` ABI so the Magnetite sandbox can load and fuel-meter it.
//!
//! ## Game rules
//!
//! * Players spawn at deterministic positions (seed-derived, arena corners).
//! * Each tick a player may **move** (up to [`MAX_SPEED`] units/tick) and/or
//!   **shoot** (one projectile per [`SHOOT_COOLDOWN_TICKS`] ticks).
//! * Projectiles travel [`PROJECTILE_SPEED`] units/tick and expire after
//!   [`PROJECTILE_LIFETIME_TICKS`] ticks.
//! * A hit deals [`HIT_DAMAGE`] HP. Players start with [`MAX_HP`] HP. Death
//!   is permanent for the match (no respawn in this reference).
//! * Match ends when at most one player is alive, but continues until all
//!   inputs stop (no forced termination — caller decides).
//!
//! ## Determinism contract
//!
//! * Only [`StepCtx::rng`] is used for randomness (projectile IDs).
//! * No wall clock, no `std::thread`, no I/O inside `step` / `validate`.
//! * All positions are `f32` accumulated from fixed-magnitude per-tick deltas —
//!   no cross-tick `f64` accumulation.
//!
//! ## WASM ABI (`--features wasm`)
//!
//! When built with `--features wasm --target wasm32-wasip1`, the crate exports:
//!
//! ```text
//! mag_alloc(len: u32) -> u32          // bump allocator pointer
//! mag_free(ptr: u32, len: u32)        // no-op (bump allocator)
//! mag_init(cfg_ptr: u32, cfg_len: u32)
//! mag_step(inputs_ptr: u32, inputs_len: u32) -> u32  // packed StepOutput ptr
//! mag_snapshot() -> u32
//! mag_restore(ptr: u32, len: u32)
//! mag_view(player_id: u64) -> u32
//! ```
//!
//! Each `-> u32` return is a pointer into the module's linear memory where a
//! length-prefixed JSON blob lives (4-byte little-endian length + payload).

pub mod game;
pub mod types;

#[cfg(feature = "wasm")]
pub mod wasm_abi;

pub use game::ArenaShooter;
pub use types::{
    ArenaCommand, ArenaDelta, ArenaSnapshot, ArenaView, Projectile, ShooterPlayer, ARENA_HEIGHT,
    ARENA_WIDTH, HIT_DAMAGE, MAX_HP, MAX_SPEED, PROJECTILE_LIFETIME_TICKS, PROJECTILE_SPEED,
    SHOOT_COOLDOWN_TICKS,
};
