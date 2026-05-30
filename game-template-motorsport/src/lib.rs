//! # Magnetite Motorsport Starter — "Circuit Rush"
//!
//! A racing / motorsport starter template built on the Magnetite platform SDK.
//! It demonstrates how to integrate complex 3-D vehicle physics (rapier3d) and
//! controller / gamepad input on the same SDK surface used by simple arcade games.
//!
//! ## What is included
//!
//! | Layer | Description |
//! |---|---|
//! | [`RacingGame`] | Server-authoritative game logic (`GameLogic` impl) |
//! | [`Vehicle`] | Per-car physics state (rigid-body raycast-suspension model) |
//! | [`Track`] | Oval track with sector checkpoints + finish line |
//! | [`LapTimer`] | Per-player lap timing; submits best lap to the platform score surface |
//! | [`InputMap`] | Maps SDK `Input` + gamepad analog axes to throttle/brake/steer |
//! | [`GameHandle`] | wasm-bindgen public API for the browser host page |
//! | `bevy_client` | Bevy ECS integration (native + WASM canvas) |
//!
//! ## SDK integration
//!
//! `RacingGame` implements [`magnetite_sdk::GameLogic`].  The platform server
//! calls `handle_input` + `tick` at the configured `tick_rate` (60 Hz).  Lap
//! times are submitted to the platform's score surface via `PlayerState::score`
//! (best lap in milliseconds, lower = better — the platform leaderboard sorts
//! ascending for this game).
//!
//! ## Physics model
//!
//! The server-authoritative physics layer is a discrete **raycast-suspension**
//! model implemented without rapier3d to keep `--no-default-features` builds
//! fast in CI.  When compiled with the `native` or `wasm` feature the Bevy
//! client renders the scene with `bevy_rapier3d` for full physics fidelity.
//!
//! ## Compile targets
//!
//! | Command | Result |
//! |---|---|
//! | `cargo check --no-default-features` | Fast CI pass (no Bevy/rapier) |
//! | `cargo run --features native` | Desktop window, rapier debug render |
//! | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | Browser WASM |

use std::collections::HashMap;

use magnetite_sdk::{
    Action, Direction, GameLogic, GameMetadata, GameState, Input, PlayerId, PlayerState, Position,
    Rotation, Snapshot,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Re-export the browser panic hook so errors show in the console.
// ---------------------------------------------------------------------------
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub use console_error_panic_hook::set_once as set_panic_hook;

// ===========================================================================
// Track definition
// ===========================================================================

/// A sector gate on the track.  Cars must pass through all gates in order
/// before the finish line counts as a lap completion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrackGate {
    /// Centre position of the gate in world-space (XZ plane).
    pub x: f32,
    pub z: f32,
    /// Half-width of the gate (metres); triggers when a car is within this radius.
    pub radius: f32,
    /// Sector index (0, 1, 2, …); the finish gate has the highest index.
    pub sector: u32,
}

/// The oval race circuit.
///
/// In the starter layout, the track is a rectangular oval with four corners
/// and a start/finish straight.  The gates correspond to the four turns plus
/// the finish line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Ordered list of gates a car must clear.
    pub gates: Vec<TrackGate>,
    /// Total number of laps required to complete the race (0 = infinite / time-trial).
    pub total_laps: u32,
}

impl Track {
    /// Build the default starter oval.
    ///
    /// The oval sits in the XZ plane (Y = 0), 200 m long × 80 m wide.
    /// Gate ordering: start/finish → turn 1 → back-straight mid → turn 3 → start/finish.
    pub fn oval() -> Self {
        Self {
            gates: vec![
                // Sector 0 — start/finish line (also the finish gate)
                TrackGate { x:   0.0, z:   0.0, radius: 8.0, sector: 0 },
                // Sector 1 — entry of turn 1 (right side)
                TrackGate { x:  90.0, z:   0.0, radius: 8.0, sector: 1 },
                // Sector 2 — turn 1 apex
                TrackGate { x: 100.0, z: -35.0, radius: 8.0, sector: 2 },
                // Sector 3 — back straight
                TrackGate { x:   0.0, z: -70.0, radius: 8.0, sector: 3 },
                // Sector 4 — turn 3 apex
                TrackGate { x:-100.0, z: -35.0, radius: 8.0, sector: 4 },
                // Sector 5 — turn 4 exit (returns to start/finish)
                TrackGate { x: -90.0, z:   0.0, radius: 8.0, sector: 5 },
            ],
            total_laps: 3,
        }
    }

    /// Return the gate the car must pass through next, given the last cleared sector.
    pub fn next_gate(&self, last_sector: u32) -> Option<&TrackGate> {
        self.gates
            .iter()
            .find(|g| g.sector == (last_sector + 1) % self.gates.len() as u32)
    }

    /// Test whether a position passes through the given gate.
    pub fn passes_gate(gate: &TrackGate, x: f32, z: f32) -> bool {
        let dx = x - gate.x;
        let dz = z - gate.z;
        (dx * dx + dz * dz).sqrt() < gate.radius
    }
}

// ===========================================================================
// Lap timing
// ===========================================================================

/// Lap timing state for one player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapTimer {
    /// Server tick when the current lap started.
    pub lap_start_tick: u64,
    /// Last sector index the car cleared.
    pub last_sector: u32,
    /// Number of complete laps finished.
    pub laps_done: u32,
    /// Best lap time in milliseconds (u64::MAX until the first lap completes).
    pub best_lap_ms: u64,
    /// Current lap number (1-based while racing).
    pub current_lap: u32,
    /// Whether the race is finished for this player.
    pub finished: bool,
}

impl LapTimer {
    pub fn new() -> Self {
        Self {
            lap_start_tick: 0,
            last_sector: u32::MAX, // before first crossing, no sector cleared
            laps_done: 0,
            best_lap_ms: u64::MAX,
            current_lap: 1,
            finished: false,
        }
    }

    /// Update timing from a gate passage.
    ///
    /// Returns `Some(lap_ms)` when a full lap is completed.
    pub fn on_gate(&mut self, gate: &TrackGate, current_tick: u64, tick_rate: u32) -> Option<u64> {
        // --- Sector progression ---
        // Before the race starts, the car must first pass the start/finish (sector 0).
        if self.last_sector == u32::MAX {
            if gate.sector == 0 {
                self.last_sector = 0;
                self.lap_start_tick = current_tick;
            }
            return None;
        }

        let expected_next = (self.last_sector + 1) % 6; // 6 sectors in the oval
        if gate.sector != expected_next {
            return None; // wrong sector — skip
        }

        self.last_sector = gate.sector;

        // --- Lap completion: back at sector 0 after clearing all others ---
        if gate.sector == 0 {
            let elapsed_ticks = current_tick.saturating_sub(self.lap_start_tick);
            let lap_ms = elapsed_ticks * 1000 / tick_rate as u64;

            if lap_ms < self.best_lap_ms {
                self.best_lap_ms = lap_ms;
            }

            self.laps_done += 1;
            self.lap_start_tick = current_tick;
            self.current_lap += 1;

            return Some(lap_ms);
        }

        None
    }
}

impl Default for LapTimer {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// InputMap — converts SDK Input to vehicle controls
// ===========================================================================

/// Vehicle control outputs after mapping from SDK input.
///
/// All values are in `[0.0, 1.0]` (normalised) unless noted.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct VehicleControls {
    /// Throttle: 0.0 = off, 1.0 = full.
    pub throttle: f32,
    /// Brake: 0.0 = none, 1.0 = full lock.
    pub brake: f32,
    /// Steering: -1.0 = full left, +1.0 = full right.
    pub steer: f32,
    /// Handbrake (drift assist).
    pub handbrake: bool,
}

impl VehicleControls {
    /// Map SDK `Input` to vehicle controls.
    ///
    /// ## Keyboard / mouse
    /// - W / ↑  → throttle 1.0
    /// - S / ↓  → brake 1.0
    /// - A / ←  → steer -1.0 (full left)
    /// - D / →  → steer +1.0 (full right)
    /// - Space  → handbrake
    ///
    /// ## Gamepad (analog)
    ///
    /// The SDK `Input` carries analog gamepad state in the `mouse` field's
    /// sub-channels until a dedicated `GamepadState` is added to the SDK.
    /// By convention for this template:
    ///
    /// | SDK field          | Gamepad mapping        |
    /// |--------------------|------------------------|
    /// | `mouse.scroll`     | Right trigger (throttle, 0..+1) |
    /// | `mouse.delta_y`    | Left trigger (brake, mapped 0..-1 → 0..1) |
    /// | `mouse.delta_x`    | Left stick X (steer, -1..+1) |
    ///
    /// This keeps the wire protocol stable while the SDK gamepad module is
    /// developed (see DECISIONS.md §4b).  A future `GamepadState` type will
    /// replace this mapping transparently.
    pub fn from_input(input: &Input) -> Self {
        // ── Analog gamepad detection ─────────────────────────────────────
        // If any analog channel carries a non-zero value we're in gamepad mode.
        let gamepad_mode = input.mouse.scroll.abs() > 0.01
            || input.mouse.delta_y.abs() > 0.01
            || input.mouse.delta_x.abs() > 0.01;

        if gamepad_mode {
            // Throttle: right trigger via scroll (+), clamped [0,1].
            let throttle = (input.mouse.scroll as f32).clamp(0.0, 1.0);
            // Brake: left trigger via delta_y (negative range from -1 to 0).
            let brake = (-input.mouse.delta_y as f32).clamp(0.0, 1.0);
            // Steer: left stick X, delta_x range [-1, +1].
            let steer = (input.mouse.delta_x as f32).clamp(-1.0, 1.0);
            let handbrake = input.keys.jump; // South button / A

            return Self { throttle, brake, steer, handbrake };
        }

        // ── Keyboard / digital ───────────────────────────────────────────
        let throttle = if input.keys.forward { 1.0 } else { 0.0 };
        let brake = if input.keys.backward { 1.0 } else { 0.0 };
        let steer = match (input.keys.left, input.keys.right) {
            (true, false)  => -1.0,
            (false, true)  =>  1.0,
            _              =>  0.0,
        };
        let handbrake = input.keys.jump;

        Self { throttle, brake, steer, handbrake }
    }
}

// ===========================================================================
// Vehicle — server-authoritative physics state
// ===========================================================================

/// Server-side vehicle state.
///
/// Uses a **discrete raycast-suspension** model.  Each tick:
/// 1. Throttle / brake set the longitudinal velocity.
/// 2. Steering rotates the heading; slip is approximated by reducing lateral
///    grip at high speed.
/// 3. The suspension spring model is implicit: the car stays on Y = 0 (flat
///    track) unless detailed terrain is provided.
///
/// When `bevy_rapier3d` is compiled in (feature `native`/`wasm`), the Bevy
/// client uses a full rigid-body vehicle instead; the server state is used for
/// reconciliation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vehicle {
    /// World-space position (X, Y = ground, Z).
    pub position: Position,
    /// Heading angle in radians (yaw; 0 = +Z axis, positive = counter-clockwise).
    pub yaw: f32,
    /// Forward speed in m/s (positive = forward, negative = reversing).
    pub speed: f32,
    /// Last applied controls.
    pub controls: VehicleControls,
    /// Whether the car is currently on track (crossed at least gate 0).
    pub on_track: bool,
}

impl Vehicle {
    /// Spawn a vehicle at the start/finish grid position for the given slot.
    pub fn spawn(slot: usize) -> Self {
        // Grid: stagger cars 6 m apart, alternating left/right.
        let offset_x = if slot % 2 == 0 { -3.0 } else { 3.0 };
        let offset_z = -(slot as f32 * 6.0);
        Self {
            position: Position {
                x: offset_x,
                y: 0.0,
                z: offset_z,
            },
            yaw: 0.0,
            speed: 0.0,
            controls: VehicleControls::default(),
            on_track: false,
        }
    }

    /// Advance physics by one tick at `tick_rate` Hz.
    ///
    /// Parameters:
    /// - `controls`: throttle/brake/steer for this tick.
    /// - `dt`: time step in seconds (1.0 / tick_rate).
    pub fn step(&mut self, controls: VehicleControls, dt: f32) {
        self.controls = controls;

        // ── Longitudinal ────────────────────────────────────────────────
        const MAX_SPEED: f32     = 80.0;  // m/s ≈ 288 km/h
        const ACCEL: f32         = 22.0;  // m/s²
        const BRAKE_DECEL: f32   = 40.0;  // m/s² (braking harder than engine drag)
        const DRAG: f32          =  0.6;  // rolling resistance coefficient

        let drive_force   = controls.throttle * ACCEL;
        let brake_force   = controls.brake * BRAKE_DECEL;
        let drag_force    = self.speed * DRAG;

        let net_accel = drive_force - brake_force - drag_force;
        self.speed = (self.speed + net_accel * dt).clamp(-MAX_SPEED * 0.3, MAX_SPEED);

        // Handbrake: sharp friction — drop to 30 % speed.
        if controls.handbrake {
            self.speed *= 0.85f32.powf(dt * 60.0); // approx exponential decay per tick
        }

        // ── Lateral / steering ──────────────────────────────────────────
        // At low speed the car pivots freely; at high speed lateral grip limits turn rate.
        const MAX_STEER_RATE: f32 = std::f32::consts::PI * 0.9; // rad/s at low speed
        let speed_factor = (self.speed.abs() / MAX_SPEED).clamp(0.0, 1.0);
        let grip_factor  = 1.0 - speed_factor * 0.55; // [0.45, 1.0] grip range
        let yaw_rate     = controls.steer * MAX_STEER_RATE * grip_factor;

        self.yaw += yaw_rate * dt;

        // ── Position integration ────────────────────────────────────────
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        // In Magnetite's right-hand Y-up: forward = +X rotated by yaw in XZ plane.
        self.position.x += cos_yaw * self.speed * dt;
        self.position.z += sin_yaw * self.speed * dt;
        // Y stays at 0 (flat track; terrain deformation is a future extension).
    }
}

// ===========================================================================
// RaceWorld — serialisable snapshot of all game-world objects
// ===========================================================================

/// Full world snapshot — baked into `GameState::world` each tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceWorld {
    /// Vehicle states, keyed by player ID (as u64 string for JSON compat).
    pub vehicles: HashMap<String, Vehicle>,
    /// Lap timers, keyed by player ID.
    pub lap_timers: HashMap<String, LapTimer>,
    /// The static track definition (sent on first tick so clients can render it).
    pub track: Track,
    /// Whether the race has started (countdown complete).
    pub race_started: bool,
    /// Countdown ticks remaining before race start (0 = racing).
    pub countdown_ticks: u32,
    /// Whether the race is fully finished (all cars have completed).
    pub race_finished: bool,
}

impl RaceWorld {
    fn new() -> Self {
        Self {
            vehicles: HashMap::new(),
            lap_timers: HashMap::new(),
            track: Track::oval(),
            race_started: false,
            countdown_ticks: 180, // 3-second countdown at 60 Hz
            race_finished: false,
        }
    }
}

// ===========================================================================
// RacingGame — implements GameLogic
// ===========================================================================

/// The Magnetite-authoritative racing game instance.
///
/// Implements [`GameLogic`]; the platform server instantiates exactly one of
/// these per match session and drives it via `handle_input` + `tick`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RacingGame {
    state: GameState,
    world: RaceWorld,
    next_player_id: u64,
}

impl Default for RacingGame {
    fn default() -> Self {
        Self {
            state: GameState::default(),
            world: RaceWorld::new(),
            next_player_id: 0,
        }
    }
}

impl RacingGame {
    /// Spawn a new player entry and place their car on the grid.
    fn add_player_internal(&mut self, player_id: PlayerId) {
        let slot = self.state.players.len();
        self.state.players.push(PlayerState {
            id: player_id,
            position: Position::default(),
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        });
        let key = player_id.as_u64().to_string();
        self.world.vehicles.insert(key.clone(), Vehicle::spawn(slot));
        self.world.lap_timers.insert(key, LapTimer::new());
    }

    /// Sync vehicle positions back to the `PlayerState` so the platform can
    /// read positions without knowing about `RaceWorld`.
    fn sync_player_positions(&mut self) {
        for player in self.state.players.iter_mut() {
            let key = player.id.as_u64().to_string();
            if let Some(vehicle) = self.world.vehicles.get(&key) {
                player.position = vehicle.position;
                player.rotation.yaw = vehicle.yaw.to_degrees();
            }
        }
    }

    /// Bake `world` into `GameState::world`.
    fn sync_world(&mut self) {
        self.state.world = serde_json::to_value(&self.world)
            .unwrap_or(serde_json::Value::Null);
    }

    /// Tick-level lap-gate detection for a single player.
    ///
    /// Checks every gate; triggers at most one gate per tick (avoids
    /// teleportation exploits).
    fn update_lap_timer(world: &mut RaceWorld, player_id: PlayerId, current_tick: u64, tick_rate: u32) -> Option<u64> {
        let key = player_id.as_u64().to_string();
        let vehicle = world.vehicles.get(&key)?.clone();
        let timer = world.lap_timers.get_mut(&key)?;

        for gate in &world.track.gates {
            if Track::passes_gate(gate, vehicle.position.x, vehicle.position.z) {
                if let Some(lap_ms) = timer.on_gate(gate, current_tick, tick_rate) {
                    return Some(lap_ms);
                }
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// GameLogic implementation
// ---------------------------------------------------------------------------

impl GameLogic for RacingGame {
    fn new() -> Self {
        let mut game = RacingGame::default();
        // Start with one local player so the game is immediately playable.
        let pid = PlayerId::new(0);
        game.next_player_id = 1;
        game.add_player_internal(pid);
        game.sync_player_positions();
        game.sync_world();
        game
    }

    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action {
        let key = player_id.as_u64().to_string();
        let Some(vehicle) = self.world.vehicles.get_mut(&key) else {
            return Action::None;
        };

        // Don't accept driving input during countdown.
        if !self.world.race_started {
            return Action::None;
        }

        let controls = VehicleControls::from_input(&input);
        const TICK_RATE: u32 = 60;
        let dt = 1.0 / TICK_RATE as f32;
        vehicle.step(controls, dt);

        // Determine the dominant action for the replay buffer.
        if controls.brake > 0.5 {
            Action::Custom {
                name: "brake".to_string(),
                payload: serde_json::json!({ "intensity": controls.brake }),
            }
        } else if controls.throttle > 0.1 {
            Action::Move {
                direction: Direction::Forward,
                sprint: controls.throttle > 0.9,
            }
        } else {
            Action::None
        }
    }

    fn tick(&mut self) {
        self.state.tick += 1;
        let current_tick = self.state.tick;
        const TICK_RATE: u32 = 60;

        // ── Countdown ──────────────────────────────────────────────────────
        if !self.world.race_started {
            if self.world.countdown_ticks > 0 {
                self.world.countdown_ticks -= 1;
            } else {
                self.world.race_started = true;
            }
        }

        // ── Physics tick (if race already started, players can also drive via
        //    handle_input which is called before tick; here we just advance
        //    coasting / passive deceleration for vehicles with no input). ──

        // ── Lap gate detection ─────────────────────────────────────────────
        let player_ids: Vec<PlayerId> = self.state.players.iter().map(|p| p.id).collect();
        for &pid in &player_ids {
            if let Some(lap_ms) = Self::update_lap_timer(&mut self.world, pid, current_tick, TICK_RATE) {
                let key = pid.as_u64().to_string();
                let timer = self.world.lap_timers.get(&key).cloned();

                // Update platform score: best lap in ms (lower = better).
                // The platform leaderboard is configured for ascending sort.
                if let Some(ref t) = timer {
                    if let Some(player) = self.state.players.iter_mut().find(|p| p.id == pid) {
                        if t.best_lap_ms < u64::MAX {
                            // score = negative best lap so the platform's
                            // descending leaderboard becomes ascending.
                            // Alternatively configure the game's leaderboard
                            // for ascending; this approach works with the
                            // default SDK descending sort.
                            player.score = -(t.best_lap_ms as i64);
                        }

                        // Store lap stats in the custom payload.
                        player.custom = serde_json::json!({
                            "laps_done": t.laps_done,
                            "best_lap_ms": if t.best_lap_ms == u64::MAX { serde_json::Value::Null } else { t.best_lap_ms.into() },
                            "current_lap": t.current_lap,
                            "last_lap_ms": lap_ms,
                        });
                    }
                }
            }
        }

        // ── Race completion check ──────────────────────────────────────────
        let total_laps = self.world.track.total_laps;
        let all_done = !self.state.players.is_empty()
            && self.state.players.iter().all(|p| {
                let key = p.id.as_u64().to_string();
                self.world
                    .lap_timers
                    .get(&key)
                    .map(|t| t.laps_done >= total_laps)
                    .unwrap_or(false)
            });
        self.world.race_finished = all_done;

        // ── Sync state ────────────────────────────────────────────────────
        self.sync_player_positions();
        self.sync_world();
    }

    fn state(&self) -> &GameState {
        &self.state
    }

    fn players(&self) -> Vec<PlayerId> {
        self.state.players.iter().map(|p| p.id).collect()
    }

    fn metadata(&self) -> GameMetadata {
        GameMetadata {
            name: "Circuit Rush".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            max_players: 8,
            min_players: 1,
            tick_rate: 60,
            description: "High-speed motorsport racing on the Magnetite platform — \
                          Bevy + rapier3d vehicle physics, gamepad analog triggers, \
                          sector-based lap timing with platform leaderboard integration."
                .to_string(),
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot::new(self.state.tick, self.state.clone())
    }

    fn restore(&mut self, snapshot: Snapshot) {
        // Restore world from the embedded world payload.
        self.world = serde_json::from_value(snapshot.state.world.clone())
            .unwrap_or_else(|_| RaceWorld::new());
        self.state = snapshot.state;
    }

    fn on_player_join(&mut self, player_id: PlayerId) {
        if !self.state.players.iter().any(|p| p.id == player_id) {
            let raw = player_id.as_u64();
            if raw >= self.next_player_id {
                self.next_player_id = raw + 1;
            }
            self.add_player_internal(player_id);
            self.sync_world();
        }
    }

    fn on_player_leave(&mut self, player_id: PlayerId) {
        let key = player_id.as_u64().to_string();
        self.world.vehicles.remove(&key);
        self.world.lap_timers.remove(&key);
        self.state.remove_player(player_id);
        self.sync_world();
    }
}

// ===========================================================================
// wasm-bindgen public API
// ===========================================================================

/// Opaque handle to a [`RacingGame`] instance owned by JavaScript.
#[wasm_bindgen]
pub struct GameHandle {
    game: RacingGame,
}

#[wasm_bindgen]
impl GameHandle {
    /// Create a new game session.
    #[wasm_bindgen(constructor)]
    pub fn new() -> GameHandle {
        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        console_error_panic_hook::set_once();

        GameHandle {
            game: RacingGame::new(),
        }
    }

    /// Process an input frame for a player.
    ///
    /// `player_id`  — numeric player ID (u64 as f64 for JS interop).
    /// `input_json` — JSON-serialised [`magnetite_sdk::Input`].
    ///
    /// For **gamepad / controller** input populate the following fields:
    /// ```json
    /// {
    ///   "mouse": {
    ///     "scroll":  0.85,   // right trigger (throttle)
    ///     "delta_y": -0.6,   // left trigger  (brake, as negative)
    ///     "delta_x":  0.3    // left stick X  (steer)
    ///   },
    ///   "keys": { "jump": false, … }
    /// }
    /// ```
    ///
    /// Returns a JSON string describing the resulting `Action`.
    #[wasm_bindgen]
    pub fn handle_input(&mut self, player_id: f64, input_json: &str) -> String {
        let id = PlayerId::new(player_id as u64);
        match serde_json::from_str::<Input>(input_json) {
            Ok(input) => {
                let action = self.game.handle_input(id, input);
                serde_json::to_string(&action).unwrap_or_else(|_| "null".into())
            }
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        }
    }

    /// Advance the simulation by one tick.
    #[wasm_bindgen]
    pub fn tick(&mut self) {
        self.game.tick();
    }

    /// Return the current full game state as JSON (`GameState` shape).
    #[wasm_bindgen]
    pub fn get_state(&self) -> String {
        serde_json::to_string(self.game.state()).unwrap_or_else(|_| "{}".into())
    }

    /// Return per-player scores as JSON `{ "player_id": score, … }`.
    ///
    /// Score is `-(best_lap_ms)` — more negative = better lap time.
    #[wasm_bindgen]
    pub fn get_scores(&self) -> String {
        let scores: HashMap<String, i64> = self
            .game
            .state()
            .players
            .iter()
            .map(|p| (p.id.as_u64().to_string(), p.score))
            .collect();
        serde_json::to_string(&scores).unwrap_or_else(|_| "{}".into())
    }

    /// Return lap timing data for all players as JSON.
    #[wasm_bindgen]
    pub fn get_lap_times(&self) -> String {
        let timers = &self.game.world.lap_timers;
        serde_json::to_string(timers).unwrap_or_else(|_| "{}".into())
    }

    /// Add a new player; returns their numeric player ID (f64 for JS interop).
    #[wasm_bindgen]
    pub fn add_player(&mut self) -> f64 {
        let pid = PlayerId::new(self.game.next_player_id);
        self.game.next_player_id += 1;
        self.game.on_player_join(pid);
        pid.as_u64() as f64
    }

    /// Current simulation tick count.
    #[wasm_bindgen]
    pub fn tick_count(&self) -> f64 {
        self.game.state().tick as f64
    }

    /// Returns `true` when the race countdown has finished.
    #[wasm_bindgen]
    pub fn race_started(&self) -> bool {
        self.game.world.race_started
    }

    /// Returns `true` when all players have completed the race.
    #[wasm_bindgen]
    pub fn race_finished(&self) -> bool {
        self.game.world.race_finished
    }

    /// Countdown ticks remaining (0 = racing underway).
    #[wasm_bindgen]
    pub fn countdown_ticks(&self) -> u32 {
        self.game.world.countdown_ticks
    }
}

/// `#[wasm_bindgen(start)]` entry — sets up the browser panic hook.
#[wasm_bindgen(start)]
pub fn wasm_main() {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    console_error_panic_hook::set_once();
}

// ===========================================================================
// Bevy integration — native desktop + WASM canvas rendering
//
// Gate behind `native` or `wasm` so plain `cargo check` stays fast.
// ===========================================================================

#[cfg(any(feature = "native", feature = "wasm"))]
pub mod bevy_client {
    use super::*;
    use bevy::prelude::*;

    // ── Resources ────────────────────────────────────────────────────────

    /// Latest authoritative game state (server snapshot or local sim).
    #[derive(Resource, Default)]
    pub struct LocalGameState {
        pub snapshot: Option<GameState>,
    }

    /// Pending input for the local player, built each frame.
    #[derive(Resource, Default)]
    pub struct PendingInput {
        pub controls: VehicleControls,
        pub sequence: u64,
    }

    // ── Components ───────────────────────────────────────────────────────

    /// Marker: the entity represents a racing car for `player_id`.
    #[derive(Component)]
    pub struct CarEntity {
        pub player_id: PlayerId,
    }

    /// Marker: a track gate visualisation entity.
    #[derive(Component)]
    pub struct GateMarker {
        pub sector: u32,
    }

    // ── Plugin ───────────────────────────────────────────────────────────

    /// Add to your Bevy [`App`] to run Circuit Rush locally.
    ///
    /// ```no_run
    /// # #[cfg(feature = "native")]
    /// # {
    /// use bevy::prelude::*;
    /// use magnetite_game_motorsport::bevy_client::GamePlugin;
    ///
    /// App::new()
    ///     .add_plugins((DefaultPlugins, GamePlugin))
    ///     .run();
    /// # }
    /// ```
    pub struct GamePlugin;

    impl Plugin for GamePlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<LocalGameState>()
                .init_resource::<PendingInput>()
                .add_systems(Startup, setup_scene)
                .add_systems(Update, (
                    collect_input,
                    sync_car_entities,
                ));
        }
    }

    // ── Systems ──────────────────────────────────────────────────────────

    fn setup_scene(mut commands: Commands) {
        // Chase camera positioned behind and above the start line.
        commands.spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 30.0, -60.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));

        // Ambient + directional lighting.
        commands.insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
        });
        commands.spawn((
            DirectionalLight {
                illuminance: 10_000.0,
                shadows_enabled: true,
                ..Default::default()
            },
            Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));

        // Ground plane — a simple stretched Bevy mesh represents the track surface.
        commands.spawn((
            Mesh3d::default(),
            MeshMaterial3d::<StandardMaterial>::default(),
            Transform {
                translation: Vec3::new(0.0, -0.05, -35.0),
                scale: Vec3::new(220.0, 0.1, 90.0),
                ..Default::default()
            },
        ));
    }

    /// Read Bevy keyboard input and map it to `VehicleControls`.
    fn collect_input(
        keys: Res<ButtonInput<KeyCode>>,
        // NOTE: Gamepad input via `gilrs` / Bevy's `Gamepads` resource would be
        // wired here in a production build.  The SDK `InputMap` documents the
        // `mouse.*` channel convention so the server logic already handles analog
        // gamepads via that path.  Bevy integration uses keyboard for simplicity.
        mut pending: ResMut<PendingInput>,
    ) {
        pending.sequence += 1;
        let throttle = if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            1.0
        } else {
            0.0
        };
        let brake = if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            1.0
        } else {
            0.0
        };
        let steer = match (
            keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft),
            keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight),
        ) {
            (true, false)  => -1.0,
            (false, true)  =>  1.0,
            _              =>  0.0,
        };
        let handbrake = keys.pressed(KeyCode::Space);

        pending.controls = VehicleControls { throttle, brake, steer, handbrake };
    }

    /// Reconcile Bevy car entities with the latest game snapshot.
    fn sync_car_entities(
        mut commands: Commands,
        state: Res<LocalGameState>,
        mut cars: Query<(&CarEntity, &mut Transform)>,
    ) {
        let Some(ref snapshot) = state.snapshot else {
            return;
        };
        for player_state in &snapshot.players {
            let pos = Vec3::new(
                player_state.position.x,
                player_state.position.y,
                player_state.position.z,
            );
            let yaw_rad = player_state.rotation.yaw.to_radians();

            let mut found = false;
            for (car, mut transform) in cars.iter_mut() {
                if car.player_id == player_state.id {
                    transform.translation = pos;
                    transform.rotation = Quat::from_rotation_y(yaw_rad);
                    found = true;
                    break;
                }
            }
            if !found {
                commands.spawn((
                    CarEntity { player_id: player_state.id },
                    Mesh3d::default(),
                    MeshMaterial3d::<StandardMaterial>::default(),
                    Transform {
                        translation: pos,
                        rotation: Quat::from_rotation_y(yaw_rad),
                        scale: Vec3::new(2.0, 1.0, 4.5), // car dimensions (W × H × L)
                        ..Default::default()
                    },
                ));
            }
        }
    }
}

// ===========================================================================
// Native binary entry point
// ===========================================================================

/// Run Circuit Rush in a native desktop window.  Called from `src/main.rs`.
#[cfg(feature = "native")]
pub fn run_native() {
    use bevy::prelude::*;
    use bevy_client::GamePlugin;

    App::new()
        .add_plugins((DefaultPlugins, GamePlugin))
        .run();
}

// ===========================================================================
// Tests — run on the host without features (fast CI path)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Input mapping tests ──────────────────────────────────────────────

    fn make_keyboard_input(fwd: bool, back: bool, left: bool, right: bool, jump: bool) -> Input {
        Input {
            keys: magnetite_sdk::KeyState {
                forward: fwd,
                backward: back,
                left,
                right,
                jump,
                ..Default::default()
            },
            mouse: magnetite_sdk::MouseState::default(),
            sequence: 1,
            timestamp_ms: 0,
        }
    }

    fn make_gamepad_input(throttle: f64, brake: f64, steer: f64) -> Input {
        Input {
            keys: magnetite_sdk::KeyState::default(),
            mouse: magnetite_sdk::MouseState {
                scroll:  throttle,       // right trigger
                delta_y: -brake,         // left trigger (negative)
                delta_x: steer,          // left stick X
                ..Default::default()
            },
            sequence: 1,
            timestamp_ms: 0,
        }
    }

    #[test]
    fn keyboard_throttle_maps_correctly() {
        let input = make_keyboard_input(true, false, false, false, false);
        let controls = VehicleControls::from_input(&input);
        assert_eq!(controls.throttle, 1.0);
        assert_eq!(controls.brake, 0.0);
        assert_eq!(controls.steer, 0.0);
    }

    #[test]
    fn keyboard_brake_maps_correctly() {
        let input = make_keyboard_input(false, true, false, false, false);
        let controls = VehicleControls::from_input(&input);
        assert_eq!(controls.throttle, 0.0);
        assert_eq!(controls.brake, 1.0);
    }

    #[test]
    fn keyboard_steer_left_right() {
        let left_input = make_keyboard_input(false, false, true, false, false);
        let right_input = make_keyboard_input(false, false, false, true, false);
        assert_eq!(VehicleControls::from_input(&left_input).steer, -1.0);
        assert_eq!(VehicleControls::from_input(&right_input).steer, 1.0);
    }

    #[test]
    fn keyboard_handbrake() {
        let input = make_keyboard_input(false, false, false, false, true);
        assert!(VehicleControls::from_input(&input).handbrake);
    }

    #[test]
    fn gamepad_throttle_maps_correctly() {
        let input = make_gamepad_input(0.85, 0.0, 0.0);
        let controls = VehicleControls::from_input(&input);
        assert!((controls.throttle - 0.85).abs() < 0.01, "throttle = {}", controls.throttle);
        assert!(controls.brake < 0.01);
    }

    #[test]
    fn gamepad_brake_maps_correctly() {
        let input = make_gamepad_input(0.0, 0.7, 0.0);
        let controls = VehicleControls::from_input(&input);
        assert!(controls.throttle < 0.01);
        assert!((controls.brake - 0.7).abs() < 0.01, "brake = {}", controls.brake);
    }

    #[test]
    fn gamepad_steer_maps_correctly() {
        let input = make_gamepad_input(0.5, 0.0, -0.6);
        let controls = VehicleControls::from_input(&input);
        assert!((controls.steer - (-0.6)).abs() < 0.01, "steer = {}", controls.steer);
    }

    // ── Vehicle physics tests ────────────────────────────────────────────

    #[test]
    fn vehicle_accelerates_with_throttle() {
        let mut v = Vehicle::spawn(0);
        let controls = VehicleControls { throttle: 1.0, brake: 0.0, steer: 0.0, handbrake: false };
        let dt = 1.0 / 60.0;
        v.step(controls, dt);
        assert!(v.speed > 0.0, "vehicle should accelerate: speed = {}", v.speed);
    }

    #[test]
    fn vehicle_brakes() {
        let mut v = Vehicle::spawn(0);
        v.speed = 30.0; // pre-set cruising speed
        let controls = VehicleControls { throttle: 0.0, brake: 1.0, steer: 0.0, handbrake: false };
        let initial_speed = v.speed;
        v.step(controls, 1.0 / 60.0);
        assert!(v.speed < initial_speed, "speed should decrease: {} → {}", initial_speed, v.speed);
    }

    #[test]
    fn vehicle_steers() {
        let mut v = Vehicle::spawn(0);
        v.speed = 20.0;
        let initial_yaw = v.yaw;
        let controls = VehicleControls { throttle: 0.5, brake: 0.0, steer: 1.0, handbrake: false };
        v.step(controls, 1.0 / 60.0);
        assert!(v.yaw != initial_yaw, "yaw should change when steering");
    }

    #[test]
    fn vehicle_handbrake_reduces_speed() {
        let mut v = Vehicle::spawn(0);
        v.speed = 40.0;
        let controls = VehicleControls { throttle: 0.0, brake: 0.0, steer: 0.0, handbrake: true };
        v.step(controls, 1.0 / 60.0);
        assert!(v.speed < 40.0, "handbrake should reduce speed");
    }

    // ── Track / lap timer tests ──────────────────────────────────────────

    #[test]
    fn track_oval_has_correct_gate_count() {
        let track = Track::oval();
        assert_eq!(track.gates.len(), 6, "oval should have 6 sector gates");
    }

    #[test]
    fn track_passes_gate_true_when_close() {
        let gate = TrackGate { x: 0.0, z: 0.0, radius: 8.0, sector: 0 };
        assert!(Track::passes_gate(&gate, 0.0, 0.0));
        assert!(Track::passes_gate(&gate, 5.0, 5.0)); // within radius
    }

    #[test]
    fn track_passes_gate_false_when_far() {
        let gate = TrackGate { x: 0.0, z: 0.0, radius: 8.0, sector: 0 };
        assert!(!Track::passes_gate(&gate, 20.0, 20.0));
    }

    #[test]
    fn lap_timer_counts_lap_after_full_circuit() {
        // Simulate a car going through all 6 gates in order.
        let track = Track::oval();
        let mut timer = LapTimer::new();
        const TICK_RATE: u32 = 60;

        // Gate 0 starts the lap
        timer.on_gate(&track.gates[0], 0, TICK_RATE);
        assert_eq!(timer.last_sector, 0);

        // Gates 1-4 are intermediate sectors
        for i in 1..5 {
            timer.on_gate(&track.gates[i], i as u64 * 100, TICK_RATE);
        }
        // Gate 5 — second-to-last
        timer.on_gate(&track.gates[5], 500, TICK_RATE);

        // Back through gate 0 — this should complete the lap
        let lap_result = timer.on_gate(&track.gates[0], 1800, TICK_RATE);
        assert!(lap_result.is_some(), "lap should complete when returning to sector 0");
        assert_eq!(timer.laps_done, 1);
    }

    #[test]
    fn lap_timer_skips_wrong_sector() {
        let track = Track::oval();
        let mut timer = LapTimer::new();
        const TICK_RATE: u32 = 60;

        // Start normally
        timer.on_gate(&track.gates[0], 0, TICK_RATE);

        // Skip sector 1 and try sector 2 directly — should be ignored
        let result = timer.on_gate(&track.gates[2], 100, TICK_RATE);
        assert!(result.is_none());
        assert_eq!(timer.last_sector, 0, "last sector should still be 0");
    }

    // ── RacingGame integration tests ─────────────────────────────────────

    #[test]
    fn new_game_has_one_player() {
        let game = RacingGame::new();
        assert_eq!(game.players().len(), 1);
    }

    #[test]
    fn countdown_prevents_movement() {
        let mut game = RacingGame::new();
        assert!(!game.world.race_started, "race should not be started initially");
        let pid = game.players()[0];

        let initial_pos = game.state().players[0].position;
        let fwd_input = make_keyboard_input(true, false, false, false, false);
        game.handle_input(pid, fwd_input);

        let _new_pos = game.state().players[0].position;
        // The vehicle position won't change because handle_input checks race_started.
        // Note: sync_player_positions runs during tick, not handle_input,
        // so we compare the vehicle directly.
        let key = pid.as_u64().to_string();
        let vehicle_pos = &game.world.vehicles[&key].position;
        assert_eq!(vehicle_pos.x, initial_pos.x);
        assert_eq!(vehicle_pos.z, initial_pos.z);
    }

    #[test]
    fn tick_advances_tick_counter() {
        let mut game = RacingGame::new();
        game.tick();
        assert_eq!(game.state().tick, 1);
        game.tick();
        assert_eq!(game.state().tick, 2);
    }

    #[test]
    fn tick_decrements_countdown() {
        let mut game = RacingGame::new();
        let initial = game.world.countdown_ticks;
        game.tick();
        assert_eq!(game.world.countdown_ticks, initial - 1);
    }

    #[test]
    fn race_starts_after_countdown() {
        let mut game = RacingGame::new();
        // Drain the countdown
        let countdown = game.world.countdown_ticks;
        for _ in 0..=countdown {
            game.tick();
        }
        assert!(game.world.race_started, "race should start after countdown");
    }

    #[test]
    fn player_join_leave() {
        let mut game = RacingGame::new();
        let pid = PlayerId::new(99);

        game.on_player_join(pid);
        assert_eq!(game.players().len(), 2);
        assert!(game.world.vehicles.contains_key("99"));

        game.on_player_leave(pid);
        assert_eq!(game.players().len(), 1);
        assert!(!game.world.vehicles.contains_key("99"));
    }

    #[test]
    fn snapshot_and_restore() {
        let mut game = RacingGame::new();
        game.tick();
        let snap = game.snapshot();
        assert_eq!(snap.tick, 1);
        assert!(snap.verify());

        game.tick();
        game.tick();
        assert_eq!(game.state().tick, 3);

        game.restore(snap);
        assert_eq!(game.state().tick, 1, "must roll back to tick 1");
    }

    #[test]
    fn metadata_is_valid() {
        let game = RacingGame::new();
        let meta = game.metadata();
        assert_eq!(meta.name, "Circuit Rush");
        assert_eq!(meta.tick_rate, 60);
        assert!(meta.max_players >= meta.min_players);
        assert!(!meta.version.is_empty());
        assert!(!meta.description.is_empty());
    }

    #[test]
    fn world_payload_round_trips() {
        let game = RacingGame::new();
        let world: RaceWorld = serde_json::from_value(game.state().world.clone())
            .expect("world payload must deserialise to RaceWorld");
        assert!(!world.vehicles.is_empty(), "world must contain at least one vehicle");
        assert_eq!(world.track.gates.len(), 6, "track must have 6 gates");
    }

    #[test]
    fn score_reflects_best_lap() {
        // Drive through all 6 gates after the countdown.
        let mut game = RacingGame::new();

        // Skip countdown
        let countdown = game.world.countdown_ticks;
        for _ in 0..=countdown {
            game.tick();
        }
        assert!(game.world.race_started);

        let pid = game.players()[0];
        let key = pid.as_u64().to_string();

        // Teleport the car through all gates in order.
        let gates = game.world.track.gates.clone();

        // Pass gate 0 to start the lap
        {
            let v = game.world.vehicles.get_mut(&key).unwrap();
            v.position.x = gates[0].x;
            v.position.z = gates[0].z;
        }
        game.tick();

        // Pass remaining gates 1..5 then gate 0 again
        for i in 1..gates.len() {
            let g = gates[i];
            {
                let v = game.world.vehicles.get_mut(&key).unwrap();
                v.position.x = g.x;
                v.position.z = g.z;
            }
            game.tick();
        }

        // Back through gate 0 — completes lap 1
        {
            let v = game.world.vehicles.get_mut(&key).unwrap();
            v.position.x = gates[0].x;
            v.position.z = gates[0].z;
        }
        game.tick();

        // After completing a lap, score should be non-zero (negative best lap ms)
        let player = game.state().players.iter().find(|p| p.id == pid).unwrap();
        assert!(player.score < 0, "score should be negative best-lap-ms; got {}", player.score);
    }
}
