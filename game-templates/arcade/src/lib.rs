//! # Magnetite Game Template — "Dot Collector"
//!
//! A minimal but real example game built on the Magnetite platform SDK and the
//! [Bevy](https://bevyengine.org) game engine.  It compiles to:
//!
//! * **Native** (Linux / macOS / Windows) — a windowed app with Bevy's full
//!   render stack (enable the `native` Cargo feature).
//! * **WASM** (`wasm32-unknown-unknown`) — a `cdylib` consumed by
//!   `wasm-bindgen`; the Magnetite platform host page drives the game loop.
//!
//! ## Game rules
//! Players move a dot around a bounded arena (±9.5 world units), collecting
//! randomly-placed coins.  Each collected coin scores one point stored in
//! `PlayerState::score`.  Health starts at 100 and slowly regenerates each
//! tick.  The server is authoritative: only the server calls `tick()` and
//! `handle_input()`; clients receive [`GameState`] snapshots and render them.
//!
//! ## SDK integration
//! This crate implements [`magnetite_sdk::GameLogic`] on [`DotCollector`],
//! which is the struct the platform server instantiates.  The Bevy
//! [`GamePlugin`] (enabled with `native`/`wasm` feature) mirrors the same
//! state for local rendering.

use std::collections::HashMap;

use magnetite_sdk::{
    Action, Direction, GameLogic, GameMetadata, GameState, Input, PlayerId, PlayerState, Position,
    Rotation, Snapshot,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Re-export a panic hook in wasm builds so browser consoles show Rust panics.
// ---------------------------------------------------------------------------
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub use console_error_panic_hook::set_once as set_panic_hook;

// ===========================================================================
// Coin — game-specific world object
// ===========================================================================

/// A collectible coin in the arena.  Serialised into `GameState::world` so
/// clients can render the coins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coin {
    pub x: f32,
    pub z: f32,
}

// ===========================================================================
// DotCollector — the authoritative game-logic struct
// ===========================================================================

/// The Magnetite-authoritative game instance.
///
/// Implements [`GameLogic`] so it can be dropped directly into the platform
/// server, or driven locally via the [`GameHandle`] wasm-bindgen API.
#[derive(Debug, Serialize, Deserialize)]
pub struct DotCollector {
    /// The canonical SDK state — players live here.
    state: GameState,
    /// Coins in the arena (serialised into `state.world` at snapshot time).
    coins: Vec<Coin>,
    /// Next player ID counter.
    next_player_id: u64,
    /// LCG seed for reproducible coin placement.
    rng_seed: u64,
}

impl Default for DotCollector {
    fn default() -> Self {
        Self {
            state: GameState::default(),
            coins: Vec::new(),
            next_player_id: 0,
            rng_seed: 0xdead_beef_cafe_babe,
        }
    }
}

impl DotCollector {
    /// Simple deterministic LCG for reproducible coin placement — no external
    /// `rand` dependency keeps the WASM bundle small.
    fn next_rand(&mut self) -> f32 {
        self.rng_seed = self
            .rng_seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Map the high 23 bits to [-9, 9]
        let bits = (self.rng_seed >> 41) as f32;
        (bits / (1u64 << 23) as f32) * 18.0 - 9.0
    }

    /// Spawn `n` coins at random positions within the arena.
    fn spawn_coins(&mut self, n: usize) {
        for _ in 0..n {
            let x = self.next_rand();
            let z = self.next_rand();
            self.coins.push(Coin { x, z });
        }
    }

    /// Bake `coins` into `GameState::world` so clients can read them.
    fn sync_world(&mut self) {
        self.state.world =
            serde_json::to_value(&self.coins).unwrap_or(serde_json::Value::Array(vec![]));
    }

    /// Add a new player; returns the assigned [`PlayerId`].
    pub fn add_player(&mut self) -> PlayerId {
        let id = PlayerId::new(self.next_player_id);
        self.next_player_id += 1;
        self.state.players.push(PlayerState {
            id,
            position: Position::default(),
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        });
        id
    }
}

// ---------------------------------------------------------------------------
// GameLogic implementation
// ---------------------------------------------------------------------------

impl GameLogic for DotCollector {
    fn new() -> Self {
        let mut game = DotCollector::default();
        // Start with one local player and eight coins on the field.
        game.add_player();
        game.spawn_coins(8);
        game.sync_world();
        game
    }

    /// Process one player's input frame and return the dominant [`Action`].
    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action {
        let Some(player) = self.state.players.iter_mut().find(|p| p.id == player_id) else {
            return Action::None;
        };

        const SPEED: f32 = 0.15;
        const ARENA: f32 = 9.5;

        let mut action = Action::None;
        let sprinting = input.keys.sprint;

        let speed = if sprinting { SPEED * 1.8 } else { SPEED };

        // Movement — WASD / arrow keys
        if input.keys.forward {
            player.position.z -= speed;
            action = Action::Move {
                direction: Direction::Forward,
                sprint: sprinting,
            };
        }
        if input.keys.backward {
            player.position.z += speed;
            action = Action::Move {
                direction: Direction::Backward,
                sprint: sprinting,
            };
        }
        if input.keys.left {
            player.position.x -= speed;
            action = Action::Move {
                direction: Direction::Left,
                sprint: sprinting,
            };
        }
        if input.keys.right {
            player.position.x += speed;
            action = Action::Move {
                direction: Direction::Right,
                sprint: sprinting,
            };
        }

        // Clamp to arena bounds
        player.position.x = player.position.x.clamp(-ARENA, ARENA);
        player.position.z = player.position.z.clamp(-ARENA, ARENA);

        // Look direction from mouse delta
        player.rotation.yaw += input.mouse.delta_x as f32 * 0.005;
        player.rotation.pitch =
            (player.rotation.pitch + input.mouse.delta_y as f32 * 0.005).clamp(-1.4, 1.4);

        // Jump / crouch (adjust Y; clamped to ground + modest ceiling)
        if input.keys.jump {
            player.position.y = (player.position.y + speed).min(3.0);
            action = Action::Jump;
        }
        if input.keys.crouch {
            player.position.y = (player.position.y - speed).max(0.0);
            action = Action::Crouch;
        }

        // Attack
        if input.keys.attack {
            action = Action::Attack;
        }
        if input.keys.secondary_attack {
            action = Action::SecondaryAttack;
        }

        action
    }

    /// Advance the simulation by one tick.
    fn tick(&mut self) {
        self.state.tick += 1;

        // Health regeneration: +1 HP per 60 ticks ≈ 1 HP/s at 60 Hz.
        for player in self.state.players.iter_mut() {
            if player.alive {
                player.health = (player.health + 1.0 / 60.0).min(player.max_health);
            }
        }

        // Coin collection — any alive player within 0.75 world units collects.
        const COLLECT_R: f32 = 0.75;
        let mut collected: Vec<usize> = Vec::new();

        'coins: for (ci, coin) in self.coins.iter().enumerate() {
            for player in self.state.players.iter_mut().filter(|p| p.alive) {
                let dx = player.position.x - coin.x;
                let dz = player.position.z - coin.z;
                if (dx * dx + dz * dz).sqrt() < COLLECT_R {
                    player.score += 1;
                    collected.push(ci);
                    continue 'coins;
                }
            }
        }

        // Remove collected coins (iterate in reverse to keep indices valid).
        for &ci in collected.iter().rev() {
            self.coins.swap_remove(ci);
        }

        // Maintain at least 8 coins on the field.
        let needed = 8usize.saturating_sub(self.coins.len());
        if needed > 0 {
            self.spawn_coins(needed);
        }

        // Bake coins into world payload so they reach clients.
        self.sync_world();
    }

    /// Immutable reference to the authoritative game state.
    fn state(&self) -> &GameState {
        &self.state
    }

    /// Return the ids of all currently-connected players.
    fn players(&self) -> Vec<PlayerId> {
        self.state.players.iter().map(|p| p.id).collect()
    }

    /// Static metadata about this game.
    fn metadata(&self) -> GameMetadata {
        GameMetadata {
            name: "Dot Collector".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            max_players: 8,
            min_players: 1,
            tick_rate: 60,
            description: "Collect glowing coins before your opponents. \
                          Build on the Magnetite SDK — game-jam to AAA scale."
                .to_string(),
        }
    }

    /// Capture a complete snapshot for save/replay/rollback.
    fn snapshot(&self) -> Snapshot {
        Snapshot::new(self.state.tick, self.state.clone())
    }

    /// Restore from a previously captured snapshot.
    fn restore(&mut self, snapshot: Snapshot) {
        // Re-derive coins from the world payload stored in the snapshot.
        self.coins = serde_json::from_value(snapshot.state.world.clone()).unwrap_or_default();
        self.state = snapshot.state;
    }

    /// Add a new player to the game.
    fn on_player_join(&mut self, player_id: PlayerId) {
        // Use the supplied ID if it doesn't clash; otherwise keep our counter.
        if !self.state.players.iter().any(|p| p.id == player_id) {
            // Ensure next_player_id stays ahead of any externally-assigned IDs.
            let raw = player_id.as_u64();
            if raw >= self.next_player_id {
                self.next_player_id = raw + 1;
            }
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
        }
    }

    /// Remove a player from the game.
    fn on_player_leave(&mut self, player_id: PlayerId) {
        self.state.remove_player(player_id);
    }
}

// ===========================================================================
// wasm-bindgen public API
//
// The Magnetite host page (index.html) calls these functions to drive the
// game loop at ~60 Hz using `requestAnimationFrame`.
// ===========================================================================

/// Opaque handle to a running [`DotCollector`] instance; owned by JavaScript.
#[wasm_bindgen]
pub struct GameHandle {
    game: DotCollector,
}

#[wasm_bindgen]
impl GameHandle {
    /// Create a new game instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> GameHandle {
        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        console_error_panic_hook::set_once();

        GameHandle {
            game: DotCollector::new(),
        }
    }

    /// Process an input frame from a player.
    ///
    /// `player_id`  — numeric player ID (u64 passed as f64 for JS interop).
    /// `input_json` — JSON-serialised [`magnetite_sdk::Input`].
    ///
    /// Returns a JSON string describing the resulting [`Action`].
    #[wasm_bindgen]
    pub fn handle_input(&mut self, player_id: f64, input_json: &str) -> String {
        let id = PlayerId::new(player_id as u64);
        match serde_json::from_str::<Input>(input_json) {
            Ok(input) => {
                let action = self.game.handle_input(id, input);
                serde_json::to_string(&action).unwrap_or_else(|_| "null".into())
            }
            Err(e) => format!("{{\"error\":\"{}\"}}", e),
        }
    }

    /// Advance the simulation by one tick.
    #[wasm_bindgen]
    pub fn tick(&mut self) {
        self.game.tick();
    }

    /// Return the current game state as a JSON string.
    ///
    /// The returned object matches [`magnetite_sdk::GameState`]:
    /// `{ tick, players: [...], world: [...coins] }`.
    #[wasm_bindgen]
    pub fn get_state(&self) -> String {
        serde_json::to_string(self.game.state()).unwrap_or_else(|_| "{}".into())
    }

    /// Return per-player scores as a JSON object `{ "0": 3, "1": 1, … }`.
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

    /// Add a new player; returns the assigned numeric player ID.
    #[wasm_bindgen]
    pub fn add_player(&mut self) -> f64 {
        self.game.add_player().as_u64() as f64
    }

    /// Current simulation tick count.
    #[wasm_bindgen]
    pub fn tick_count(&self) -> f64 {
        self.game.state().tick as f64
    }
}

/// `#[wasm_bindgen(start)]` entry point — called automatically when the Wasm
/// module is initialised.  Sets up the browser panic hook.
#[wasm_bindgen(start)]
pub fn wasm_main() {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    console_error_panic_hook::set_once();
}

// ===========================================================================
// Bevy integration — local rendering client (native + WASM canvas)
//
// Gate behind `native` or `wasm` features so that plain `cargo check` (no
// features) remains fast and does not pull in the render stack.
// ===========================================================================

#[cfg(any(feature = "native", feature = "wasm"))]
pub mod bevy_client {
    use super::*;
    use bevy::prelude::*;

    // ── ECS Resources ────────────────────────────────────────────────────────

    /// Latest game state received from the server (or driven locally).
    #[derive(Resource, Default)]
    pub struct LocalGameState {
        pub snapshot: Option<GameState>,
    }

    /// Pending input built from Bevy's input systems each frame.
    #[derive(Resource, Default)]
    pub struct PendingInput {
        pub keys: magnetite_sdk::KeyState,
        pub sequence: u64,
    }

    // ── ECS Components ───────────────────────────────────────────────────────

    /// Marker component on the entity that renders a player dot.
    #[derive(Component)]
    pub struct PlayerDot {
        pub id: PlayerId,
    }

    // ── Plugin ───────────────────────────────────────────────────────────────

    /// Add this plugin to your Bevy [`App`] to run Dot Collector locally.
    ///
    /// ```no_run
    /// # #[cfg(feature = "native")]
    /// # {
    /// use bevy::prelude::*;
    /// use magnetite_game_template::bevy_client::GamePlugin;
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
                .add_systems(Update, (collect_keyboard_input, sync_player_entities));
        }
    }

    // ── Systems ──────────────────────────────────────────────────────────────

    fn setup_scene(mut commands: Commands) {
        // Top-down camera for the 2-D arena.
        commands.spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 15.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
        ));

        commands.insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 600.0,
        });
    }

    /// Map Bevy keyboard input to `magnetite_sdk::KeyState` each frame.
    fn collect_keyboard_input(keys: Res<ButtonInput<KeyCode>>, mut pending: ResMut<PendingInput>) {
        pending.sequence += 1;
        pending.keys = magnetite_sdk::KeyState {
            forward: keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp),
            backward: keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown),
            left: keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft),
            right: keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight),
            jump: keys.pressed(KeyCode::Space),
            crouch: keys.pressed(KeyCode::ControlLeft),
            attack: keys.pressed(KeyCode::KeyZ),
            secondary_attack: keys.pressed(KeyCode::KeyX),
            interact: keys.pressed(KeyCode::KeyE),
            sprint: keys.pressed(KeyCode::ShiftLeft),
        };
    }

    /// Reconcile Bevy entities with the latest game snapshot.
    fn sync_player_entities(
        mut commands: Commands,
        state: Res<LocalGameState>,
        mut dots: Query<(&PlayerDot, &mut Transform)>,
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
            // Try to update an existing entity.
            let mut found = false;
            for (dot, mut transform) in dots.iter_mut() {
                if dot.id == player_state.id {
                    transform.translation = pos;
                    found = true;
                    break;
                }
            }
            // Spawn a new entity when there is none yet for this player.
            if !found {
                commands.spawn((
                    PlayerDot {
                        id: player_state.id,
                    },
                    Transform::from_translation(pos),
                    GlobalTransform::default(),
                ));
            }
        }
    }
}

// ===========================================================================
// Native binary entry point
// ===========================================================================

/// Run the game in a native desktop window.  Call from `src/main.rs`:
/// ```no_run
/// # #[cfg(feature = "native")]
/// magnetite_game_template::run_native();
/// ```
#[cfg(feature = "native")]
pub fn run_native() {
    use bevy::prelude::*;
    use bevy_client::GamePlugin;

    App::new().add_plugins((DefaultPlugins, GamePlugin)).run();
}

// ===========================================================================
// Tests — run on the host only (fast; no wasm target needed)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(forward: bool, backward: bool, left: bool, right: bool) -> Input {
        Input {
            keys: magnetite_sdk::KeyState {
                forward,
                backward,
                left,
                right,
                ..Default::default()
            },
            mouse: magnetite_sdk::MouseState::default(),
            sequence: 1,
            timestamp_ms: 0,
        }
    }

    #[test]
    fn new_game_has_one_player_and_coins() {
        let game = DotCollector::new();
        assert_eq!(
            game.players().len(),
            1,
            "should start with exactly one player"
        );
        assert!(
            !game.coins.is_empty(),
            "should start with coins on the field"
        );
    }

    #[test]
    fn handle_input_moves_player_forward() {
        let mut game = DotCollector::new();
        let player_id = game.players()[0];
        let initial_z = game.state().players[0].position.z;

        game.handle_input(player_id, make_input(true, false, false, false));

        let new_z = game.state().players[0].position.z;
        assert!(
            new_z < initial_z,
            "forward input should decrease Z (right-hand Y-up)"
        );
    }

    #[test]
    fn handle_input_returns_correct_action() {
        let mut game = DotCollector::new();
        let player_id = game.players()[0];

        let action = game.handle_input(player_id, make_input(true, false, false, false));
        assert!(
            matches!(
                action,
                Action::Move {
                    direction: Direction::Forward,
                    ..
                }
            ),
            "forward key must produce Move(Forward)"
        );
    }

    #[test]
    fn tick_advances_state_tick() {
        let mut game = DotCollector::new();
        game.tick();
        assert_eq!(game.state().tick, 1);
        game.tick();
        assert_eq!(game.state().tick, 2);
    }

    #[test]
    fn state_snapshot_contains_all_players() {
        let mut game = DotCollector::new();
        game.add_player();
        game.add_player();
        assert_eq!(
            game.state().players.len(),
            3,
            "snapshot should include all 3 players"
        );
    }

    #[test]
    fn arena_clamps_position() {
        let mut game = DotCollector::new();
        let pid = game.players()[0];

        // Drive the player 200 steps to the right.
        for _ in 0..200 {
            game.handle_input(pid, make_input(false, false, false, true));
        }

        let pos_x = game.state().players[0].position.x;
        assert!(
            pos_x <= 9.5 + f32::EPSILON,
            "X position {pos_x} must be clamped to arena bound 9.5"
        );
    }

    #[test]
    fn metadata_fields_are_valid() {
        let game = DotCollector::new();
        let meta = game.metadata();
        assert_eq!(meta.name, "Dot Collector");
        assert_eq!(meta.tick_rate, 60);
        assert!(meta.max_players >= meta.min_players);
        assert!(!meta.version.is_empty());
        assert!(!meta.description.is_empty());
    }

    #[test]
    fn snapshot_and_restore_rollback() {
        let mut game = DotCollector::new();
        let player_id = game.players()[0];

        // Advance one tick and capture a snapshot.
        game.tick();
        let snap = game.snapshot();
        assert_eq!(snap.tick, 1);
        assert!(snap.verify(), "freshly taken snapshot must verify");

        // Advance two more ticks.
        game.tick();
        game.tick();
        assert_eq!(game.state().tick, 3);

        // Restore to tick 1.
        game.restore(snap);
        assert_eq!(game.state().tick, 1, "state must be rolled back to tick 1");
    }

    #[test]
    fn on_player_join_and_leave() {
        let mut game = DotCollector::new();
        let pid = PlayerId::new(42);

        game.on_player_join(pid);
        assert_eq!(game.players().len(), 2, "joining adds a player");

        game.on_player_leave(pid);
        assert_eq!(game.players().len(), 1, "leaving removes the player");
    }

    #[test]
    fn health_regenerates_over_ticks() {
        let mut game = DotCollector::new();

        // Drain health first.
        game.state.players[0].health = 50.0;
        let before = game.state().players[0].health;

        for _ in 0..60 {
            game.tick();
        }

        let after = game.state().players[0].health;
        assert!(
            after > before,
            "health should regenerate: {before} → {after}"
        );
    }

    #[test]
    fn world_payload_contains_coins() {
        let game = DotCollector::new();
        // After new(), sync_world() has been called.
        let coins: Vec<Coin> = serde_json::from_value(game.state().world.clone())
            .expect("world should contain serialised coins");
        assert!(!coins.is_empty(), "world payload must include coins");
    }
}
