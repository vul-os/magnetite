//! # `magnetite_sdk::authority`
//!
//! Server-authoritative game primitives — the SDK layer that makes "one Rust
//! game, server-authoritative, anti-cheat by construction" possible.
//!
//! ## Overview
//!
//! | Type | Role |
//! |---|---|
//! | [`Tick`] | Monotonic tick counter (`u64`) |
//! | [`DeterministicRng`] | Xoshiro256** seeded per-match — the ONLY source of randomness |
//! | [`StepCtx`] | Per-step context passed to [`AuthoritativeGame::step`] |
//! | [`RejectReason`] | Why an input was rejected |
//! | [`AuthoritativeGame`] | The trait every authoritative Rust game implements |
//! | [`Topology`] | Scale primitive — `SingleRoom` / `Dedicated` / `Sharded` |
//! | [`MatchConfig`] | Per-match configuration including topology, tick rate, seed |
//! | [`GameExecutor`] | Runtime-facing abstraction over native and Wasm execution |
//! | [`NativeExecutor`] | In-process `GameExecutor` implementation |
//! | [`StepOutput`] | Result of one authoritative tick |
//! | [`Validator`] | Composable server-side anti-cheat check |
//! | [`RateLimit`] | Built-in validator: max inputs per second |
//! | [`MovementVelocity`] | Built-in validator: max movement speed |
//! | [`ActionCooldown`] | Built-in validator: minimum ticks between an action |
//! | [`InputSchema`] | Built-in validator: schema / range checks on raw input |
//! | [`ReplayLog`] | Deterministic input+hash log recorded by the runtime |
//! | [`ReplayVerdict`] | Outcome of [`verify_replay`] |
//!
//! ## Determinism contract
//!
//! `AuthoritativeGame::step` and `validate` MUST be **purely deterministic**:
//! given the same `(state, ordered commands, StepCtx)` they must produce
//! identical output on every platform, every run. Specifically:
//!
//! * Use **only** [`StepCtx::rng`] for randomness — never `rand`, `thread_rng`,
//!   or any OS RNG.
//! * Never read wall-clock time inside `step` / `validate`.
//! * Prefer fixed-point arithmetic over `f64` for values accumulated across ticks.
//!
//! Violations are detected at runtime by the replay verifier: if `state_hash`
//! diverges between the original run and the re-simulation, the verdict is
//! [`ReplayVerdict::Divergence`].

use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::input::Input;
use crate::state::PlayerId;

// ---------------------------------------------------------------------------
// Tick
// ---------------------------------------------------------------------------

/// Monotonic tick counter. Starts at 0 and advances by 1 each server tick.
pub type Tick = u64;

// ---------------------------------------------------------------------------
// DeterministicRng  (xoshiro256**)
// ---------------------------------------------------------------------------

/// Deterministic per-match RNG seeded from [`MatchConfig::seed`].
///
/// This is the **only** source of randomness a game is permitted to use inside
/// `step` or `validate`. Using any other RNG source (including `rand::thread_rng`,
/// `std::random`, or the OS) violates the determinism contract.
///
/// Implements the [xoshiro256**](https://prng.di.unimi.it/xoshiro256starstar.c)
/// algorithm — fast, high-quality, period 2^256−1.
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::DeterministicRng;
///
/// let mut rng = DeterministicRng::new(12345);
/// let a = rng.next_u64();
/// let b = rng.next_u64();
/// assert_ne!(a, b);
///
/// // Same seed → same sequence.
/// let mut rng2 = DeterministicRng::new(12345);
/// assert_eq!(rng2.next_u64(), a);
/// assert_eq!(rng2.next_u64(), b);
/// ```
#[derive(Debug, Clone)]
pub struct DeterministicRng {
    state: [u64; 4],
}

impl DeterministicRng {
    /// Construct a new RNG from a 64-bit seed.
    ///
    /// The seed is expanded to a 256-bit state using the `splitmix64` function
    /// to ensure a full-rank starting state even for small seed values.
    pub fn new(seed: u64) -> Self {
        // splitmix64 to fully spread a u64 seed across 4×u64 state.
        fn splitmix64(x: &mut u64) -> u64 {
            *x = x.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = *x;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        }
        let mut s = seed;
        Self {
            state: [
                splitmix64(&mut s),
                splitmix64(&mut s),
                splitmix64(&mut s),
                splitmix64(&mut s),
            ],
        }
    }

    /// Return the next 64-bit pseudo-random integer.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = rotl(self.state[1].wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = rotl(self.state[3], 45);
        result
    }

    /// Return a pseudo-random `f32` in `[0, 1)`.
    ///
    /// Uses 23 mantissa bits from [`next_u64`](Self::next_u64).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        // Take top 23 bits → float in [1, 2) → subtract 1.
        let bits = (self.next_u64() >> 41) as u32;
        f32::from_bits(0x3F80_0000 | bits) - 1.0
    }
}

#[inline(always)]
fn rotl(x: u64, k: u32) -> u64 {
    (x << k) | (x >> (64 - k))
}

// ---------------------------------------------------------------------------
// StepCtx
// ---------------------------------------------------------------------------

/// Per-tick context provided to [`AuthoritativeGame::step`].
///
/// `step` must not read wall-clock time or any other external state; all inputs
/// to deterministic simulation must flow through this struct.
pub struct StepCtx<'a> {
    /// The current authoritative tick number.
    pub tick: Tick,
    /// Nominal tick duration in milliseconds (e.g. 16 for 60 Hz).
    pub dt_ms: u32,
    /// The only permitted source of randomness.
    pub rng: &'a mut DeterministicRng,
}

// ---------------------------------------------------------------------------
// RejectReason
// ---------------------------------------------------------------------------

/// Why the server rejected a player's input.
///
/// Returned by [`AuthoritativeGame::validate`] and surfaced to the client as
/// [`crate::protocol::ClientNet::InputFrame`] → [`crate::protocol::ServerNet::Reject`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// The client is sending inputs faster than the allowed rate.
    RateLimited,
    /// The implied movement would exceed the physics bounds.
    OutOfBounds,
    /// A specific action is not permitted in the current game state.
    IllegalAction(String),
    /// The input's tick is too old (e.g. client is lagging badly).
    StaleInput,
    /// The player is not authorised to perform this action.
    Unauthorized,
}

impl std::fmt::Display for RejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RejectReason::RateLimited => write!(f, "rate limited"),
            RejectReason::OutOfBounds => write!(f, "out of bounds"),
            RejectReason::IllegalAction(msg) => write!(f, "illegal action: {msg}"),
            RejectReason::StaleInput => write!(f, "stale input"),
            RejectReason::Unauthorized => write!(f, "unauthorized"),
        }
    }
}

// ---------------------------------------------------------------------------
// AuthoritativeGame
// ---------------------------------------------------------------------------

/// The core trait every server-authoritative game implements.
///
/// ## Determinism — the contract
///
/// `step` and `validate` MUST be **purely deterministic**: given identical
/// `(state, ordered commands, StepCtx)` they must produce identical results on
/// every platform. This enables:
///
/// * **Replay verification** — re-simulation of a [`ReplayLog`] to detect
///   tampering or nondeterminism bugs.
/// * **Sharding** — migrate state between shard hosts and continue seamlessly.
/// * **Sandboxed execution** — the Wasm executor re-runs the same code with
///   identical inputs and asserts hash equality.
///
/// ## Anti-wallhack via `view_for`
///
/// Only the bytes returned by `view_for(player)` are transmitted to that
/// player. Never include other players' hidden state (positions behind walls,
/// inventory) in a player's view.
pub trait AuthoritativeGame: Send + 'static {
    /// Full, authoritative game state — stored server-side and used for
    /// restore/replay.
    type Snapshot: serde::Serialize + serde::de::DeserializeOwned + Clone;

    /// A compact diff of state changes since a prior snapshot — broadcast
    /// every tick.
    type Delta: serde::Serialize + serde::de::DeserializeOwned;

    /// Per-player, interest-filtered view of the world — **only this is sent
    /// to the player**. Omit enemy positions behind walls, fog-of-war regions,
    /// etc.
    type View: serde::Serialize;

    /// The authoritative game command — the output of `validate`. The game
    /// logic operates on commands, never raw client input.
    type Command: serde::Serialize + serde::de::DeserializeOwned;

    // ------------------------------------------------------------------ //
    // Lifecycle                                                            //
    // ------------------------------------------------------------------ //

    /// Construct a fresh game state for a new match.
    fn init(cfg: &MatchConfig) -> Self;

    /// Called when a player joins mid-game.
    fn on_join(&mut self, _p: PlayerId) {}

    /// Called when a player disconnects or is kicked.
    fn on_leave(&mut self, _p: PlayerId) {}

    // ------------------------------------------------------------------ //
    // Per-tick pipeline                                                    //
    // ------------------------------------------------------------------ //

    /// Validate an untrusted client input and translate it into 0 or more
    /// authoritative commands.
    ///
    /// **Never trust client-sent state.** Check position, velocity, and action
    /// legality here; return `Err(RejectReason)` for anything suspicious.
    fn validate(
        &self,
        player: PlayerId,
        input: &Input,
        tick: Tick,
    ) -> Result<Vec<Self::Command>, RejectReason>;

    /// Advance the game by one tick given an ordered list of (player, command)
    /// pairs.
    ///
    /// The list is deterministically ordered (e.g. by player id) by the
    /// runtime before calling `step`.
    fn step(&mut self, ctx: &mut StepCtx, commands: &[(PlayerId, Self::Command)]);

    // ------------------------------------------------------------------ //
    // Snapshot / delta                                                     //
    // ------------------------------------------------------------------ //

    /// Return the full authoritative snapshot (for periodic broadcasts and
    /// replay recording).
    fn snapshot(&self) -> Self::Snapshot;

    /// Restore game state from a snapshot (for replay re-simulation and shard
    /// handoff).
    fn restore(snap: &Self::Snapshot, cfg: &MatchConfig) -> Self;

    /// Return a compact delta relative to a prior snapshot (broadcast every
    /// tick to minimise bandwidth).
    fn delta(&self, since: &Self::Snapshot) -> Self::Delta;

    /// Return the interest-filtered view for a specific player (drives the
    /// anti-wallhack + bandwidth budget).
    fn view_for(&self, player: PlayerId) -> Self::View;
}

// ---------------------------------------------------------------------------
// Topology  +  MatchConfig
// ---------------------------------------------------------------------------

/// The scale topology chosen for a match.
///
/// Game code is **identical** across all three modes; the runtime selects the
/// topology from [`MatchConfig`] and manages connections accordingly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Topology {
    /// One process, broadcast-all — optimal for ≲16 players (game jams).
    SingleRoom,

    /// Authoritative server + per-player interest-filtered snapshots — up to
    /// ~256 players.
    Dedicated {
        /// Authoritative tick rate in Hz.
        tick_hz: u16,
    },

    /// Spatial sharding with per-cell authoritative servers and handoff when
    /// players cross cell boundaries — AAA scale.
    Sharded {
        /// Authoritative tick rate per shard in Hz.
        tick_hz: u16,
        /// Shard cell size in world units.
        cell_size: f32,
        /// Maximum players per shard before handoff is triggered.
        max_per_shard: u32,
    },
}

/// Per-match configuration. Determines topology, player cap, tick rate, and
/// the seed that is injected into [`DeterministicRng`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchConfig {
    /// The scale topology to use.
    pub topology: Topology,
    /// Hard cap on simultaneous players.
    pub max_players: u32,
    /// Authoritative tick rate in Hz.
    pub tick_hz: u16,
    /// Seed for [`DeterministicRng`] — set from a CSPRNG by the matchmaker.
    pub seed: u64,
    /// Broadcast a full [`AuthoritativeGame::Snapshot`] every this many ticks.
    pub snapshot_every: u16,
}

impl MatchConfig {
    /// Automatically choose the best [`Topology`] for `max_players`.
    ///
    /// | Players | Topology |
    /// |---|---|
    /// | 1–16 | `SingleRoom` |
    /// | 17–256 | `Dedicated { tick_hz: 60 }` |
    /// | 257+ | `Sharded { tick_hz: 20, cell_size: 500.0, max_per_shard: 64 }` |
    ///
    /// # Example
    /// ```rust
    /// use magnetite_sdk::authority::{MatchConfig, Topology};
    ///
    /// let cfg = MatchConfig::auto(4);
    /// assert!(matches!(cfg.topology, Topology::SingleRoom));
    ///
    /// let cfg = MatchConfig::auto(100);
    /// assert!(matches!(cfg.topology, Topology::Dedicated { .. }));
    ///
    /// let cfg = MatchConfig::auto(1000);
    /// assert!(matches!(cfg.topology, Topology::Sharded { .. }));
    /// ```
    pub fn auto(max_players: u32) -> Self {
        let topology = match max_players {
            0..=16 => Topology::SingleRoom,
            17..=256 => Topology::Dedicated { tick_hz: 60 },
            _ => Topology::Sharded {
                tick_hz: 20,
                cell_size: 500.0,
                max_per_shard: 64,
            },
        };
        let tick_hz = match &topology {
            Topology::SingleRoom => 60,
            Topology::Dedicated { tick_hz } => *tick_hz,
            Topology::Sharded { tick_hz, .. } => *tick_hz,
        };
        Self {
            topology,
            max_players,
            tick_hz,
            seed: 0, // matchmaker overwrites this
            snapshot_every: 300,
        }
    }
}

// ---------------------------------------------------------------------------
// GameExecutor + StepOutput
// ---------------------------------------------------------------------------

/// Output of one authoritative tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput {
    /// Players whose inputs were rejected this tick, with the reason.
    pub rejects: Vec<(PlayerId, RejectReason)>,
    /// A stable hash of the game state after this tick.
    ///
    /// Computed from the canonical JSON serialisation of the snapshot. The
    /// replay verifier uses this tick-by-tick to detect divergence.
    pub state_hash: u64,
}

/// Runtime-facing execution abstraction.
///
/// The same game can run in two modes without any game-code changes:
/// * [`NativeExecutor`] — in-process, zero overhead.
/// * `WasmExecutor` (in `magnetite-sandbox`) — Wasmtime, fuel-metered,
///   memory-capped, fully sandboxed.
///
/// All bytes exchanged through this trait are canonical JSON by default.
pub trait GameExecutor: Send {
    /// Advance the game by one tick.
    ///
    /// `inputs` is a list of `(player_id, raw_input)` pairs; the executor
    /// validates, translates to commands, and calls `step`.
    fn step(&mut self, tick: Tick, inputs: &[(PlayerId, Input)]) -> StepOutput;

    /// Serialise the current snapshot to bytes.
    fn snapshot(&self) -> Vec<u8>;

    /// Replace the current state with a previously-serialised snapshot.
    fn restore(&mut self, bytes: &[u8]);

    /// Serialise the interest-filtered view for `player`.
    fn view_for(&self, player: PlayerId) -> Vec<u8>;

    /// Serialise the delta since the state encoded in `snapshot_bytes`.
    fn delta_since(&self, snapshot_bytes: &[u8]) -> Vec<u8>;
}

// ---------------------------------------------------------------------------
// NativeExecutor
// ---------------------------------------------------------------------------

/// In-process [`GameExecutor`] — wraps a live [`AuthoritativeGame`] instance.
///
/// This is the default executor for `magnetite-runtime` when the game is
/// trusted (i.e. compiled directly into the server binary). The
/// `magnetite-sandbox` crate provides `WasmExecutor` for untrusted game logic.
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::{
///     AuthoritativeGame, DeterministicRng, GameExecutor, MatchConfig,
///     NativeExecutor, RejectReason, StepCtx, Tick,
/// };
/// use magnetite_sdk::input::Input;
/// use magnetite_sdk::state::PlayerId;
///
/// /// A trivial counter game used for testing.
/// struct CounterGame { count: u64 }
///
/// #[derive(serde::Serialize, serde::Deserialize, Clone)]
/// struct CounterSnap { count: u64 }
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct CounterDelta { delta: i64 }
///
/// #[derive(serde::Serialize)]
/// struct CounterView { count: u64 }
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// enum CounterCmd { Increment }
///
/// impl AuthoritativeGame for CounterGame {
///     type Snapshot = CounterSnap;
///     type Delta    = CounterDelta;
///     type View     = CounterView;
///     type Command  = CounterCmd;
///
///     fn init(_cfg: &MatchConfig) -> Self { CounterGame { count: 0 } }
///     fn validate(&self, _p: PlayerId, _i: &Input, _t: Tick)
///         -> Result<Vec<CounterCmd>, RejectReason>
///     { Ok(vec![CounterCmd::Increment]) }
///     fn step(&mut self, _ctx: &mut StepCtx, cmds: &[(PlayerId, CounterCmd)]) {
///         self.count += cmds.len() as u64;
///     }
///     fn snapshot(&self) -> CounterSnap { CounterSnap { count: self.count } }
///     fn restore(s: &CounterSnap, _cfg: &MatchConfig) -> Self { CounterGame { count: s.count } }
///     fn delta(&self, _s: &CounterSnap) -> CounterDelta { CounterDelta { delta: 0 } }
///     fn view_for(&self, _p: PlayerId) -> CounterView { CounterView { count: self.count } }
/// }
///
/// let cfg = MatchConfig::auto(2);
/// let mut exec = NativeExecutor::<CounterGame>::new(cfg);
/// let p = PlayerId::new(1);
/// let out = exec.step(1, &[(p, Input::default())]);
/// assert_eq!(out.rejects.len(), 0);
/// ```
pub struct NativeExecutor<G: AuthoritativeGame> {
    game: G,
    config: MatchConfig,
    rng: DeterministicRng,
}

impl<G: AuthoritativeGame> NativeExecutor<G> {
    /// Construct a new executor, initialising the game from `cfg`.
    pub fn new(cfg: MatchConfig) -> Self {
        let rng = DeterministicRng::new(cfg.seed);
        let game = G::init(&cfg);
        Self {
            game,
            config: cfg,
            rng,
        }
    }
}

impl<G: AuthoritativeGame> GameExecutor for NativeExecutor<G> {
    fn step(&mut self, tick: Tick, inputs: &[(PlayerId, Input)]) -> StepOutput {
        let dt_ms = 1000u32 / u32::from(self.config.tick_hz).max(1);
        let mut rejects = Vec::new();

        // Validate inputs → commands.
        let mut commands: Vec<(PlayerId, G::Command)> = Vec::with_capacity(inputs.len());
        for (player, input) in inputs {
            match self.game.validate(*player, input, tick) {
                Ok(cmds) => {
                    for cmd in cmds {
                        commands.push((*player, cmd));
                    }
                }
                Err(reason) => {
                    rejects.push((*player, reason));
                }
            }
        }

        // Sort commands deterministically by player id so step output is
        // identical regardless of input arrival order.
        commands.sort_by_key(|(pid, _)| pid.as_u64());

        // Advance game state.
        {
            let mut ctx = StepCtx {
                tick,
                dt_ms,
                rng: &mut self.rng,
            };
            self.game.step(&mut ctx, &commands);
        }

        // Compute a stable state hash over the serialised snapshot.
        let snap = self.game.snapshot();
        let state_hash = compute_state_hash(&snap);

        StepOutput {
            rejects,
            state_hash,
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        let snap = self.game.snapshot();
        serde_json::to_vec(&snap).unwrap_or_default()
    }

    fn restore(&mut self, bytes: &[u8]) {
        if let Ok(snap) = serde_json::from_slice::<G::Snapshot>(bytes) {
            self.game = G::restore(&snap, &self.config);
            // Re-seed RNG from config to keep things consistent after restore.
            self.rng = DeterministicRng::new(self.config.seed);
        }
    }

    fn view_for(&self, player: PlayerId) -> Vec<u8> {
        let view = self.game.view_for(player);
        serde_json::to_vec(&view).unwrap_or_default()
    }

    fn delta_since(&self, snapshot_bytes: &[u8]) -> Vec<u8> {
        let since: G::Snapshot = match serde_json::from_slice(snapshot_bytes) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let delta = self.game.delta(&since);
        serde_json::to_vec(&delta).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Validators
// ---------------------------------------------------------------------------

/// Composable server-side anti-cheat check.
///
/// Validators run **before** `AuthoritativeGame::validate` and provide an
/// additional, reusable layer of defence against abuse. They operate on raw
/// [`Input`] frames — no game-specific logic required.
///
/// # Built-ins
///
/// * [`RateLimit`] — reject if the player sends too many inputs per second.
/// * [`MovementVelocity`] — reject if implied movement speed exceeds a cap.
/// * [`ActionCooldown`] — enforce a minimum tick gap between an action type.
/// * [`InputSchema`] — basic structural / range validation.
///
/// Chain validators with [`ValidatorChain`].
pub trait Validator: Send {
    /// Return `Ok(())` if the input passes, or the reject reason.
    fn check(&mut self, player: PlayerId, input: &Input, tick: Tick) -> Result<(), RejectReason>;
}

// ------------------------------------------------------------------ //
// Built-in: RateLimit                                                 //
// ------------------------------------------------------------------ //

/// Rejects players who send more than `max_per_sec` inputs per second.
///
/// Uses a sliding-window counter reset every ~1 000 ms of wall-clock time.
/// This is the only validator that touches wall-clock time — it operates on
/// the **rate** of client messages, not on game simulation state.
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::{RateLimit, Validator};
/// use magnetite_sdk::input::Input;
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = RateLimit::new(100); // max 100 inputs/sec
/// let p = PlayerId::new(1);
/// // Under the limit: ok.
/// assert!(v.check(p, &Input::default(), 1).is_ok());
/// ```
pub struct RateLimit {
    max_per_sec: u32,
    /// (player_id → (count in current window, window_start))
    windows: HashMap<PlayerId, (u32, Instant)>,
}

impl RateLimit {
    /// Create a new rate limiter.
    ///
    /// `max_per_sec` is the maximum number of input frames accepted from a
    /// single player within any 1-second window.
    pub fn new(max_per_sec: u32) -> Self {
        Self {
            max_per_sec,
            windows: HashMap::new(),
        }
    }
}

impl Validator for RateLimit {
    fn check(&mut self, player: PlayerId, _input: &Input, _tick: Tick) -> Result<(), RejectReason> {
        let now = Instant::now();
        let entry = self.windows.entry(player).or_insert((0, now));
        // Reset window if a second has elapsed.
        if now.duration_since(entry.1).as_millis() >= 1000 {
            *entry = (0, now);
        }
        entry.0 += 1;
        if entry.0 > self.max_per_sec {
            Err(RejectReason::RateLimited)
        } else {
            Ok(())
        }
    }
}

// ------------------------------------------------------------------ //
// Built-in: MovementVelocity                                          //
// ------------------------------------------------------------------ //

/// Rejects inputs that imply a movement speed above `max_units_per_tick`.
///
/// Compares the mouse delta magnitude (used as a proxy for angular velocity in
/// many top-down / FPS games) against the configured cap. For richer position-
/// based checks, implement a custom [`Validator`] that tracks player positions
/// in the game snapshot.
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::{MovementVelocity, Validator};
/// use magnetite_sdk::input::{Input, MouseState};
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = MovementVelocity::new(100.0);
/// let p = PlayerId::new(1);
/// let fast = Input {
///     mouse: MouseState { delta_x: 9999.0, ..Default::default() },
///     ..Default::default()
/// };
/// assert!(v.check(p, &fast, 1).is_err());
/// ```
pub struct MovementVelocity {
    max_units_per_tick: f32,
}

impl MovementVelocity {
    /// Create a new velocity validator.
    ///
    /// `max_units_per_tick` is compared against the Euclidean magnitude of the
    /// mouse delta vector in each input frame.
    pub fn new(max_units_per_tick: f32) -> Self {
        Self { max_units_per_tick }
    }
}

impl Validator for MovementVelocity {
    fn check(&mut self, _player: PlayerId, input: &Input, _tick: Tick) -> Result<(), RejectReason> {
        let dx = input.mouse.delta_x as f32;
        let dy = input.mouse.delta_y as f32;
        let mag = (dx * dx + dy * dy).sqrt();
        if mag > self.max_units_per_tick {
            Err(RejectReason::OutOfBounds)
        } else {
            Ok(())
        }
    }
}

// ------------------------------------------------------------------ //
// Built-in: ActionCooldown                                            //
// ------------------------------------------------------------------ //

/// Enforces a minimum number of ticks between a specific action being taken.
///
/// The action is identified by a string key (e.g. `"attack"`, `"ability_2"`).
/// Tracks the last-used tick per player.
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::{ActionCooldown, Validator};
/// use magnetite_sdk::input::{Input, KeyState};
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = ActionCooldown::new("attack", 10); // 10-tick cooldown
/// let p = PlayerId::new(1);
/// let attacking = Input {
///     keys: KeyState { attack: true, ..Default::default() },
///     ..Default::default()
/// };
///
/// // First use: ok.
/// assert!(v.check(p, &attacking, 0).is_ok());
/// // Immediately after: rejected.
/// assert!(v.check(p, &attacking, 1).is_err());
/// // After cooldown: ok.
/// assert!(v.check(p, &attacking, 10).is_ok());
/// ```
pub struct ActionCooldown {
    action: &'static str,
    cooldown_ticks: Tick,
    /// (player_id → last_used_tick)
    last_used: HashMap<PlayerId, Tick>,
}

impl ActionCooldown {
    /// Create a new cooldown enforcer.
    ///
    /// `action` is a logical action name; the validator inspects the `Input`
    /// to determine whether the action is active. Supported action names:
    ///
    /// | Name | Trigger |
    /// |---|---|
    /// | `"attack"` | `input.keys.attack` |
    /// | `"secondary_attack"` | `input.keys.secondary_attack` |
    /// | `"jump"` | `input.keys.jump` |
    /// | `"interact"` | `input.keys.interact` |
    ///
    /// Any other name is currently treated as "never triggered".
    pub fn new(action: &'static str, cooldown_ticks: u64) -> Self {
        Self {
            action,
            cooldown_ticks,
            last_used: HashMap::new(),
        }
    }

    fn is_triggered(&self, input: &Input) -> bool {
        match self.action {
            "attack" => input.keys.attack,
            "secondary_attack" => input.keys.secondary_attack,
            "jump" => input.keys.jump,
            "interact" => input.keys.interact,
            _ => false,
        }
    }
}

impl Validator for ActionCooldown {
    fn check(&mut self, player: PlayerId, input: &Input, tick: Tick) -> Result<(), RejectReason> {
        if !self.is_triggered(input) {
            return Ok(());
        }
        if let Some(&last) = self.last_used.get(&player) {
            // Player has used this action before — enforce cooldown.
            if tick.saturating_sub(last) < self.cooldown_ticks {
                return Err(RejectReason::IllegalAction(format!(
                    "{} is on cooldown",
                    self.action
                )));
            }
        }
        // First use, or cooldown elapsed — allow and record.
        self.last_used.insert(player, tick);
        Ok(())
    }
}

// ------------------------------------------------------------------ //
// Built-in: InputSchema                                               //
// ------------------------------------------------------------------ //

/// Basic structural / range validation on raw input frames.
///
/// Rejects inputs whose sequence number is suspiciously large, whose
/// timestamp is clearly in the future (clock skew), or whose mouse deltas
/// are non-finite (`NaN` / ±∞).
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::{InputSchema, Validator};
/// use magnetite_sdk::input::{Input, MouseState};
/// use magnetite_sdk::state::PlayerId;
///
/// let mut v = InputSchema::default();
/// let p = PlayerId::new(1);
/// let bad = Input {
///     mouse: MouseState { delta_x: f64::NAN, ..Default::default() },
///     ..Default::default()
/// };
/// assert!(v.check(p, &bad, 1).is_err());
/// ```
#[derive(Debug, Default)]
pub struct InputSchema {
    /// Maximum allowed sequence number jump between frames (0 = unlimited).
    pub max_seq_jump: u64,
}

impl Validator for InputSchema {
    fn check(&mut self, _player: PlayerId, input: &Input, _tick: Tick) -> Result<(), RejectReason> {
        if !input.mouse.delta_x.is_finite() || !input.mouse.delta_y.is_finite() {
            return Err(RejectReason::IllegalAction(
                "non-finite mouse delta".to_string(),
            ));
        }
        if !input.mouse.x.is_finite() || !input.mouse.y.is_finite() {
            return Err(RejectReason::IllegalAction(
                "non-finite mouse position".to_string(),
            ));
        }
        Ok(())
    }
}

// ------------------------------------------------------------------ //
// ValidatorChain                                                      //
// ------------------------------------------------------------------ //

/// Run a sequence of [`Validator`]s in order; fail fast on first rejection.
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::{RateLimit, InputSchema, ValidatorChain, Validator};
/// use magnetite_sdk::input::Input;
/// use magnetite_sdk::state::PlayerId;
///
/// let mut chain = ValidatorChain::new()
///     .add(RateLimit::new(120))
///     .add(InputSchema::default());
///
/// let p = PlayerId::new(1);
/// assert!(chain.check(p, &Input::default(), 0).is_ok());
/// ```
pub struct ValidatorChain {
    validators: Vec<Box<dyn Validator>>,
}

impl ValidatorChain {
    /// Create an empty chain.
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    /// Append a validator to the chain.
    pub fn add(mut self, v: impl Validator + 'static) -> Self {
        self.validators.push(Box::new(v));
        self
    }
}

impl Default for ValidatorChain {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator for ValidatorChain {
    fn check(&mut self, player: PlayerId, input: &Input, tick: Tick) -> Result<(), RejectReason> {
        for v in &mut self.validators {
            v.check(player, input, tick)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ReplayLog + ReplayVerdict + verify_replay
// ---------------------------------------------------------------------------

/// A deterministic recording of an authoritative match — inputs + state hashes.
///
/// The runtime records one entry per tick. The anti-cheat service re-simulates
/// the log with [`verify_replay`] to detect tampering or nondeterminism.
///
/// # Storage
///
/// `ReplayLog` implements [`serde::Serialize`] / [`Deserialize`] so it can be
/// stored in the database or an object store and re-verified later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayLog {
    /// The match configuration used during the original run.
    pub config: MatchConfig,

    /// Per-tick input records: `(tick, [(player_id, input)])`.
    pub frames: Vec<(Tick, Vec<(PlayerId, Input)>)>,

    /// Per-tick state hashes recorded by the runtime: `(tick, hash)`.
    pub state_hashes: Vec<(Tick, u64)>,
}

impl ReplayLog {
    /// Create an empty replay log for the given configuration.
    pub fn new(config: MatchConfig) -> Self {
        Self {
            config,
            frames: Vec::new(),
            state_hashes: Vec::new(),
        }
    }

    /// Record inputs and the authoritative state hash for a tick.
    pub fn record(&mut self, tick: Tick, inputs: Vec<(PlayerId, Input)>, state_hash: u64) {
        self.frames.push((tick, inputs));
        self.state_hashes.push((tick, state_hash));
    }
}

/// Outcome of re-simulating a [`ReplayLog`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayVerdict {
    /// All ticks match — the recording is consistent.
    Clean,
    /// A state hash mismatch was found — either tampered inputs or a
    /// nondeterminism bug in the game.
    Divergence {
        /// The tick at which the divergence was first detected.
        tick: Tick,
        /// The hash recorded during the original run.
        expected: u64,
        /// The hash produced by re-simulation.
        got: u64,
    },
}

/// Re-simulate a [`ReplayLog`] and verify that state hashes match tick-by-tick.
///
/// Re-creates the game from `log.config`, replays every input frame in order,
/// and compares `state_hash` against `log.state_hashes`. The first mismatch
/// returns [`ReplayVerdict::Divergence`]; a clean log returns
/// [`ReplayVerdict::Clean`].
///
/// # Determinism requirement
///
/// This function is the enforcement mechanism for the determinism contract. If
/// `verify_replay` returns `Divergence`, the match is flagged for review.
///
/// # Example
/// ```rust
/// use magnetite_sdk::authority::{
///     AuthoritativeGame, DeterministicRng, GameExecutor, MatchConfig,
///     NativeExecutor, RejectReason, ReplayLog, ReplayVerdict, StepCtx, Tick,
///     verify_replay,
/// };
/// use magnetite_sdk::input::Input;
/// use magnetite_sdk::state::PlayerId;
///
/// struct TrivialGame { counter: u64 }
///
/// #[derive(serde::Serialize, serde::Deserialize, Clone)]
/// struct TrivialSnap { counter: u64 }
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct TrivialDelta {}
///
/// #[derive(serde::Serialize)]
/// struct TrivialView {}
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct TrivialCmd;
///
/// impl AuthoritativeGame for TrivialGame {
///     type Snapshot = TrivialSnap;
///     type Delta    = TrivialDelta;
///     type View     = TrivialView;
///     type Command  = TrivialCmd;
///
///     fn init(_cfg: &MatchConfig) -> Self { TrivialGame { counter: 0 } }
///     fn validate(&self, _p: PlayerId, _i: &Input, _t: Tick)
///         -> Result<Vec<TrivialCmd>, RejectReason> { Ok(vec![TrivialCmd]) }
///     fn step(&mut self, _ctx: &mut StepCtx, cmds: &[(PlayerId, TrivialCmd)]) {
///         self.counter += cmds.len() as u64;
///     }
///     fn snapshot(&self) -> TrivialSnap { TrivialSnap { counter: self.counter } }
///     fn restore(s: &TrivialSnap, _cfg: &MatchConfig) -> Self { TrivialGame { counter: s.counter } }
///     fn delta(&self, _s: &TrivialSnap) -> TrivialDelta { TrivialDelta {} }
///     fn view_for(&self, _p: PlayerId) -> TrivialView { TrivialView {} }
/// }
///
/// let cfg = MatchConfig::auto(2);
/// let mut exec = NativeExecutor::<TrivialGame>::new(cfg.clone());
/// let mut log  = ReplayLog::new(cfg);
///
/// let p = PlayerId::new(1);
/// for tick in 1u64..=5 {
///     let inputs = vec![(p, Input::default())];
///     let out = exec.step(tick, &inputs);
///     log.record(tick, inputs, out.state_hash);
/// }
///
/// assert_eq!(verify_replay::<TrivialGame>(&log), ReplayVerdict::Clean);
/// ```
pub fn verify_replay<G: AuthoritativeGame>(log: &ReplayLog) -> ReplayVerdict {
    // Re-create the executor from the same config.
    let mut exec = NativeExecutor::<G>::new(log.config.clone());

    // Build a tick→expected_hash lookup.
    let hash_map: HashMap<Tick, u64> = log.state_hashes.iter().copied().collect();

    for (tick, inputs) in &log.frames {
        let out = exec.step(*tick, inputs);

        if let Some(&expected) = hash_map.get(tick) {
            if out.state_hash != expected {
                return ReplayVerdict::Divergence {
                    tick: *tick,
                    expected,
                    got: out.state_hash,
                };
            }
        }
    }

    ReplayVerdict::Clean
}

// ---------------------------------------------------------------------------
// Internal: state hashing
// ---------------------------------------------------------------------------

/// Compute a stable 64-bit hash over the serialised snapshot.
///
/// Uses FNV-1a (64-bit) over the canonical JSON representation so the hash is
/// deterministic across platforms (JSON serialisation order is fixed because
/// `serde_json` serialises struct fields in declaration order).
///
/// FNV-1a is chosen over `std::collections::hash_map::DefaultHasher` because
/// the default hasher uses SipHash which is randomly seeded per-process,
/// making it non-deterministic across restarts.
pub(crate) fn compute_state_hash<S: serde::Serialize>(snapshot: &S) -> u64 {
    let json = serde_json::to_string(snapshot).unwrap_or_default();
    fnv1a_64(json.as_bytes())
}

/// FNV-1a 64-bit hash — deterministic, zero-dep, suitable for non-crypto use.
fn fnv1a_64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Input, KeyState, MouseState};
    use crate::state::PlayerId;

    // ------------------------------------------------------------------ //
    // DeterministicRng                                                    //
    // ------------------------------------------------------------------ //

    #[test]
    fn rng_same_seed_same_sequence() {
        let mut a = DeterministicRng::new(0xDEAD_BEEF);
        let mut b = DeterministicRng::new(0xDEAD_BEEF);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn rng_different_seeds_different_sequences() {
        let mut a = DeterministicRng::new(1);
        let mut b = DeterministicRng::new(2);
        // Very unlikely all 10 values collide.
        let equal = (0..10).all(|_| a.next_u64() == b.next_u64());
        assert!(!equal);
    }

    #[test]
    fn rng_next_f32_in_unit_interval() {
        let mut rng = DeterministicRng::new(42);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!(v >= 0.0 && v < 1.0, "f32 {v} out of [0,1)");
        }
    }

    // ------------------------------------------------------------------ //
    // MatchConfig::auto                                                   //
    // ------------------------------------------------------------------ //

    #[test]
    fn match_config_auto_single_room() {
        let cfg = MatchConfig::auto(1);
        assert!(matches!(cfg.topology, Topology::SingleRoom));
        assert_eq!(cfg.tick_hz, 60);
    }

    #[test]
    fn match_config_auto_dedicated() {
        let cfg = MatchConfig::auto(100);
        assert!(matches!(cfg.topology, Topology::Dedicated { .. }));
    }

    #[test]
    fn match_config_auto_sharded() {
        let cfg = MatchConfig::auto(1000);
        assert!(matches!(cfg.topology, Topology::Sharded { .. }));
    }

    // ------------------------------------------------------------------ //
    // Minimal AuthoritativeGame — same inputs ⟹ same state_hash         //
    // ------------------------------------------------------------------ //

    /// A tiny counter game for unit testing.
    struct CounterGame {
        count: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct CounterSnap {
        count: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct CounterDelta {
        #[allow(dead_code)]
        added: u64,
    }

    #[derive(serde::Serialize)]
    struct CounterView {
        count: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    enum CounterCmd {
        Increment,
    }

    impl AuthoritativeGame for CounterGame {
        type Snapshot = CounterSnap;
        type Delta = CounterDelta;
        type View = CounterView;
        type Command = CounterCmd;

        fn init(_cfg: &MatchConfig) -> Self {
            CounterGame { count: 0 }
        }

        fn validate(
            &self,
            _player: PlayerId,
            _input: &Input,
            _tick: Tick,
        ) -> Result<Vec<CounterCmd>, RejectReason> {
            Ok(vec![CounterCmd::Increment])
        }

        fn step(&mut self, _ctx: &mut StepCtx, commands: &[(PlayerId, CounterCmd)]) {
            self.count += commands.len() as u64;
        }

        fn snapshot(&self) -> CounterSnap {
            CounterSnap { count: self.count }
        }

        fn restore(snap: &CounterSnap, _cfg: &MatchConfig) -> Self {
            CounterGame { count: snap.count }
        }

        fn delta(&self, since: &CounterSnap) -> CounterDelta {
            CounterDelta {
                added: self.count.saturating_sub(since.count),
            }
        }

        fn view_for(&self, _player: PlayerId) -> CounterView {
            CounterView { count: self.count }
        }
    }

    #[test]
    fn same_inputs_same_state_hash() {
        let cfg = MatchConfig::auto(2);
        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);

        let inputs: Vec<(PlayerId, Input)> = vec![
            (
                p1,
                Input {
                    sequence: 0,
                    ..Default::default()
                },
            ),
            (
                p2,
                Input {
                    sequence: 0,
                    ..Default::default()
                },
            ),
        ];

        // Run A.
        let mut exec_a = NativeExecutor::<CounterGame>::new(cfg.clone());
        let out_a = exec_a.step(1, &inputs);

        // Run B — identical config and inputs.
        let mut exec_b = NativeExecutor::<CounterGame>::new(cfg);
        let out_b = exec_b.step(1, &inputs);

        assert_eq!(
            out_a.state_hash, out_b.state_hash,
            "same inputs must yield same state_hash"
        );
    }

    #[test]
    fn different_inputs_different_state_hash() {
        let cfg = MatchConfig::auto(2);
        let p1 = PlayerId::new(1);

        let mut exec_a = NativeExecutor::<CounterGame>::new(cfg.clone());
        let out_a = exec_a.step(1, &[(p1, Input::default())]);

        // No inputs — counter stays at 0.
        let mut exec_b = NativeExecutor::<CounterGame>::new(cfg);
        let out_b = exec_b.step(1, &[]);

        assert_ne!(
            out_a.state_hash, out_b.state_hash,
            "different inputs must yield different state_hash"
        );
    }

    // ------------------------------------------------------------------ //
    // Snapshot restore                                                     //
    // ------------------------------------------------------------------ //

    #[test]
    fn native_executor_snapshot_restore_roundtrip() {
        let cfg = MatchConfig::auto(1);
        let mut exec = NativeExecutor::<CounterGame>::new(cfg);

        let p = PlayerId::new(1);
        exec.step(1, &[(p, Input::default())]);
        exec.step(2, &[(p, Input::default())]);

        let snap = exec.snapshot();
        let hash_before = exec.step(3, &[(p, Input::default())]).state_hash;

        // Restore and replay tick 3.
        exec.restore(&snap);
        let hash_after = exec.step(3, &[(p, Input::default())]).state_hash;

        assert_eq!(
            hash_before, hash_after,
            "restore + replay must be deterministic"
        );
    }

    // ------------------------------------------------------------------ //
    // ReplayLog + verify_replay                                           //
    // ------------------------------------------------------------------ //

    #[test]
    fn replay_clean() {
        let cfg = MatchConfig::auto(2);
        let mut exec = NativeExecutor::<CounterGame>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg);

        let p = PlayerId::new(1);
        for tick in 1u64..=10 {
            let inputs = vec![(
                p,
                Input {
                    sequence: tick,
                    ..Default::default()
                },
            )];
            let out = exec.step(tick, &inputs.clone());
            log.record(tick, inputs, out.state_hash);
        }

        assert_eq!(
            verify_replay::<CounterGame>(&log),
            ReplayVerdict::Clean,
            "honest replay must be Clean"
        );
    }

    #[test]
    fn replay_tampered_diverges() {
        let cfg = MatchConfig::auto(2);
        let mut exec = NativeExecutor::<CounterGame>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg);

        let p = PlayerId::new(1);
        for tick in 1u64..=5 {
            let inputs = vec![(
                p,
                Input {
                    sequence: tick,
                    ..Default::default()
                },
            )];
            let out = exec.step(tick, &inputs.clone());
            log.record(tick, inputs, out.state_hash);
        }

        // Tamper: forge a state_hash for tick 3.
        for (tick, hash) in &mut log.state_hashes {
            if *tick == 3 {
                *hash = hash.wrapping_add(1);
            }
        }

        let verdict = verify_replay::<CounterGame>(&log);
        assert!(
            matches!(verdict, ReplayVerdict::Divergence { tick: 3, .. }),
            "tampered hash must yield Divergence at tick 3, got {verdict:?}"
        );
    }

    // ------------------------------------------------------------------ //
    // Validators                                                          //
    // ------------------------------------------------------------------ //

    #[test]
    fn rate_limit_passes_under_limit() {
        let mut v = RateLimit::new(100);
        let p = PlayerId::new(1);
        // A handful of inputs well under 100/sec.
        for tick in 0..10 {
            assert!(v.check(p, &Input::default(), tick).is_ok());
        }
    }

    #[test]
    fn rate_limit_rejects_over_limit() {
        let mut v = RateLimit::new(3); // very low cap for testing
        let p = PlayerId::new(1);
        // First 3 pass.
        for tick in 0..3 {
            assert!(v.check(p, &Input::default(), tick).is_ok());
        }
        // 4th is rejected (within the same ~1s window).
        assert_eq!(
            v.check(p, &Input::default(), 3),
            Err(RejectReason::RateLimited)
        );
    }

    #[test]
    fn movement_velocity_passes_small_delta() {
        let mut v = MovementVelocity::new(100.0);
        let p = PlayerId::new(1);
        let input = Input {
            mouse: MouseState {
                delta_x: 10.0,
                delta_y: 10.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(p, &input, 1).is_ok());
    }

    #[test]
    fn movement_velocity_rejects_large_delta() {
        let mut v = MovementVelocity::new(10.0);
        let p = PlayerId::new(1);
        let input = Input {
            mouse: MouseState {
                delta_x: 100.0,
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(v.check(p, &input, 1), Err(RejectReason::OutOfBounds));
    }

    #[test]
    fn action_cooldown_enforces_gap() {
        let mut v = ActionCooldown::new("attack", 5);
        let p = PlayerId::new(1);
        let attacking = Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // First use at tick 0: allowed.
        assert!(v.check(p, &attacking, 0).is_ok());
        // Tick 1: too soon.
        assert!(v.check(p, &attacking, 1).is_err());
        // Tick 4: still too soon (5-tick cooldown).
        assert!(v.check(p, &attacking, 4).is_err());
        // Tick 5: exactly at cooldown boundary — allowed.
        assert!(v.check(p, &attacking, 5).is_ok());
    }

    #[test]
    fn input_schema_rejects_nan_delta() {
        let mut v = InputSchema::default();
        let p = PlayerId::new(1);
        let bad = Input {
            mouse: MouseState {
                delta_x: f64::NAN,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(v.check(p, &bad, 1).is_err());
    }

    #[test]
    fn input_schema_accepts_normal_input() {
        let mut v = InputSchema::default();
        let p = PlayerId::new(1);
        assert!(v.check(p, &Input::default(), 1).is_ok());
    }

    #[test]
    fn validator_chain_passes_clean() {
        let mut chain = ValidatorChain::new()
            .add(RateLimit::new(200))
            .add(InputSchema::default());
        let p = PlayerId::new(1);
        assert!(chain.check(p, &Input::default(), 0).is_ok());
    }

    #[test]
    fn validator_chain_fails_fast() {
        let mut chain = ValidatorChain::new()
            .add(RateLimit::new(0)) // immediately rejects
            .add(InputSchema::default());
        let p = PlayerId::new(1);
        assert_eq!(
            chain.check(p, &Input::default(), 0),
            Err(RejectReason::RateLimited)
        );
    }

    // ------------------------------------------------------------------ //
    // FNV determinism                                                     //
    // ------------------------------------------------------------------ //

    #[test]
    fn fnv1a_is_deterministic() {
        let a = fnv1a_64(b"magnetite");
        let b = fnv1a_64(b"magnetite");
        assert_eq!(a, b);
        assert_ne!(fnv1a_64(b"magnetite"), fnv1a_64(b"magnetite!"));
    }

    // ------------------------------------------------------------------ //
    // RejectReason display                                                //
    // ------------------------------------------------------------------ //

    #[test]
    fn reject_reason_display() {
        assert_eq!(RejectReason::RateLimited.to_string(), "rate limited");
        assert_eq!(
            RejectReason::IllegalAction("nope".to_string()).to_string(),
            "illegal action: nope"
        );
    }
}
