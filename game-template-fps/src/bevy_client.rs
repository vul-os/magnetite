//! # Bevy + rapier3d Rendering Client
//!
//! Gates behind the `native` or `wasm` feature flags. This module wires the
//! authoritative [`crate::FpsGame`] into a Bevy [`App`] with:
//!
//! - Bevy 0.14 `DefaultPlugins` (window, renderer, audio, input)
//! - `bevy_rapier3d` for physics (character controller, level colliders)
//! - `gilrs` gamepad integration (native only)
//! - First-person camera controlled by mouse + right thumbstick
//! - Level geometry spawned from [`crate::level::LevelDescriptor`]
//! - Per-player entity sync driven by [`LocalGameState`]
//!
//! ## Architecture
//!
//! ```text
//!     Bevy Update frame
//!         │
//!         ├─ collect_keyboard_input   → PendingInput (KeyState + mouse delta)
//!         ├─ collect_gamepad_input    → PendingInput (gilrs axes/buttons injected)
//!         ├─ run_local_game_tick      → drives FpsGame (handle_input + tick)
//!         └─ sync_player_entities     → creates/moves player capsules in ECS
//! ```

use bevy::prelude::*;

#[cfg(feature = "native")]
use bevy_rapier3d::prelude::*;

use magnetite_sdk::{
    state::{GameState, PlayerId},
    Input, KeyState, MouseState,
};

use crate::{level, FpsGame, FpsPlayerCustom};
use magnetite_sdk::GameLogic;

// ===========================================================================
// Resources
// ===========================================================================

/// Shared authoritative game state driven locally (single-player / host).
#[derive(Resource)]
pub struct LocalGameState {
    pub game: FpsGame,
    pub local_player_id: PlayerId,
}

impl Default for LocalGameState {
    fn default() -> Self {
        let mut game = FpsGame::new();
        let local_player_id = PlayerId::new(0);
        game.on_player_join(local_player_id);
        Self {
            game,
            local_player_id,
        }
    }
}

/// Input frame being assembled this update tick.
#[derive(Resource, Default)]
pub struct PendingInput {
    pub keys: KeyState,
    pub mouse: MouseState,
    pub sequence: u64,
}

// ===========================================================================
// Components
// ===========================================================================

/// Marker for the entity representing the local player's first-person camera.
#[derive(Component)]
pub struct FpsCamera;

/// Marker linking a Bevy entity to a Magnetite [`PlayerId`].
#[derive(Component)]
pub struct PlayerEntity {
    pub id: PlayerId,
}

/// Marker for spawned level collider entities.
#[derive(Component)]
pub struct LevelGeometry;

// ===========================================================================
// Plugin
// ===========================================================================

/// Add this plugin to your Bevy [`App`] to run the FPS starter locally.
///
/// ```no_run
/// # #[cfg(feature = "native")]
/// # {
/// use bevy::prelude::*;
/// use magnetite_fps_starter::bevy_client::FpsPlugin;
///
/// App::new()
///     .add_plugins((DefaultPlugins, FpsPlugin))
///     .run();
/// # }
/// ```
pub struct FpsPlugin;

impl Plugin for FpsPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "native")]
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .add_plugins(RapierDebugRenderPlugin::default());

        app.init_resource::<LocalGameState>()
            .init_resource::<PendingInput>()
            .add_systems(Startup, (setup_camera, spawn_level))
            .add_systems(
                Update,
                (
                    collect_keyboard_input,
                    #[cfg(feature = "native")]
                    collect_gamepad_input,
                    run_local_game_tick,
                    sync_player_entities,
                )
                    .chain(),
            );
    }
}

// ===========================================================================
// Startup systems
// ===========================================================================

/// Spawn the first-person camera attached to the local player.
fn setup_camera(mut commands: Commands) {
    // First-person camera — positioned at eye-height (1.7m above ground).
    commands.spawn((
        FpsCamera,
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.7, 0.0),
    ));

    // Ambient light.
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 250.0,
    });

    // Directional (sun) light.
    commands.spawn(DirectionalLight {
        illuminance: 10_000.0,
        shadows_enabled: true,
        ..default()
    });
}

/// Spawn static level geometry (floor, walls, cover boxes).
fn spawn_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let desc = level::level_descriptor();

    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.25, 0.28),
        perceptual_roughness: 0.9,
        ..default()
    });
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.18, 0.20),
        perceptual_roughness: 1.0,
        ..default()
    });

    for col in &desc.colliders {
        let [hx, hy, hz] = col.half_extents;
        let mesh = meshes.add(Cuboid::new(hx * 2.0, hy * 2.0, hz * 2.0));
        let mat = if col.is_wall {
            wall_mat.clone()
        } else {
            floor_mat.clone()
        };
        let transform = Transform::from_xyz(col.center.x, col.center.y, col.center.z);

        let entity = commands
            .spawn((
                LevelGeometry,
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                transform,
                GlobalTransform::default(),
            ))
            .id();

        // Add rapier3d collider in native builds.
        #[cfg(feature = "native")]
        commands.entity(entity).insert(Collider::cuboid(hx, hy, hz));
    }
}

// ===========================================================================
// Input systems
// ===========================================================================

/// Collect keyboard + mouse input into [`PendingInput`].
fn collect_keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    mut pending: ResMut<PendingInput>,
) {
    pending.sequence += 1;

    pending.keys = KeyState {
        forward: keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp),
        backward: keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown),
        left: keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft),
        right: keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight),
        jump: keys.pressed(KeyCode::Space),
        crouch: keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::KeyC),
        attack: keys.pressed(KeyCode::KeyZ)
            || keys.pressed(bevy::input::mouse::MouseButton::Left as _),
        secondary_attack: keys.pressed(KeyCode::KeyX),
        interact: keys.pressed(KeyCode::KeyE),
        sprint: keys.pressed(KeyCode::ShiftLeft),
    };

    // Accumulate mouse motion.
    let mut dx = 0.0f64;
    let mut dy = 0.0f64;
    for ev in mouse_motion.read() {
        dx += ev.delta.x as f64;
        dy += ev.delta.y as f64;
    }
    pending.mouse.delta_x = dx;
    pending.mouse.delta_y = dy;
}

/// Collect gamepad (controller) input via Bevy's `Gamepads` resource.
///
/// Maps controller axes and buttons into the same [`PendingInput`] as keyboard.
/// Native only — WASM uses the JS Gamepad API.
#[cfg(feature = "native")]
fn collect_gamepad_input(
    gamepads: Res<Gamepads>,
    axes: Res<Axis<GamepadAxis>>,
    buttons: Res<ButtonInput<GamepadButton>>,
    mut pending: ResMut<PendingInput>,
) {
    for gamepad in gamepads.iter() {
        // ── Left stick — movement ────────────────────────────────────────────
        let lx = axes
            .get(GamepadAxis {
                gamepad,
                axis_type: GamepadAxisType::LeftStickX,
            })
            .unwrap_or(0.0);
        let ly = axes
            .get(GamepadAxis {
                gamepad,
                axis_type: GamepadAxisType::LeftStickY,
            })
            .unwrap_or(0.0);

        // Dead-zone 0.15.
        if lx.abs() > 0.15 || ly.abs() > 0.15 {
            // Encode left-stick in mouse.delta_x/y with a scale flag
            // (32768 means "gamepad analog" in InputMap::analog_axis).
            pending.mouse.delta_x = (lx as f64) * 32768.0;
            pending.mouse.delta_y = (-ly as f64) * 32768.0; // Y-flip: stick up = forward
        }

        // ── Right stick — look ───────────────────────────────────────────────
        let rx = axes
            .get(GamepadAxis {
                gamepad,
                axis_type: GamepadAxisType::RightStickX,
            })
            .unwrap_or(0.0);
        let ry = axes
            .get(GamepadAxis {
                gamepad,
                axis_type: GamepadAxisType::RightStickY,
            })
            .unwrap_or(0.0);

        const LOOK_SENSITIVITY: f64 = 120.0; // pixels/s equivalent
                                             // Accumulate into mouse delta (FpsGame::handle_input reads these).
        if rx.abs() > 0.15 {
            pending.mouse.delta_x += rx as f64 * LOOK_SENSITIVITY;
        }
        if ry.abs() > 0.15 {
            pending.mouse.delta_y += ry as f64 * LOOK_SENSITIVITY;
        }

        // ── Buttons ──────────────────────────────────────────────────────────
        if buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::South,
        }) {
            pending.keys.jump = true;
        }
        if buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::East,
        }) {
            pending.keys.crouch = true;
        }
        if buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::RightTrigger2,
        }) {
            pending.keys.attack = true; // fire
        }
        if buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::LeftTrigger2,
        }) {
            pending.keys.secondary_attack = true; // aim
        }
        if buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::LeftThumb,
        }) {
            pending.keys.sprint = true;
        }
        if buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::West,
        }) {
            // X/West = interact (also reload in some games).
            pending.keys.interact = true;
        }
    }
}

// ===========================================================================
// Game-tick system
// ===========================================================================

/// Drive the local [`FpsGame`] one tick, feeding the assembled [`PendingInput`].
fn run_local_game_tick(mut gs: ResMut<LocalGameState>, pending: Res<PendingInput>) {
    let input = Input {
        keys: pending.keys,
        mouse: pending.mouse,
        sequence: pending.sequence,
        timestamp_ms: 0,
    };
    let pid = gs.local_player_id;
    gs.game.handle_input(pid, input);
    gs.game.tick();
}

// ===========================================================================
// Entity-sync system
// ===========================================================================

/// Reconcile Bevy entities with the latest game snapshot.
///
/// Creates a capsule entity for each player that doesn't have one, and
/// teleports existing ones to the authoritative position.
fn sync_player_entities(
    mut commands: Commands,
    gs: Res<LocalGameState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut player_q: Query<(&PlayerEntity, &mut Transform)>,
    mut camera_q: Query<(&FpsCamera, &mut Transform), Without<PlayerEntity>>,
) {
    let state: &GameState = gs.game.state();

    for ps in &state.players {
        let pos = Vec3::new(ps.position.x, ps.position.y + 1.7, ps.position.z);

        // Update FPS camera for the local player.
        if ps.id == gs.local_player_id {
            for (_cam, mut cam_tx) in camera_q.iter_mut() {
                cam_tx.translation = pos;
                // Rotation is applied in look system; here we just set position.
            }
        }

        // Try to move an existing entity.
        let mut found = false;
        for (pe, mut tx) in player_q.iter_mut() {
            if pe.id == ps.id {
                tx.translation = Vec3::new(ps.position.x, ps.position.y + 0.9, ps.position.z);
                found = true;
                break;
            }
        }

        if !found && ps.id != gs.local_player_id {
            // Spawn a capsule mesh for remote players.
            let mat_color = if ps.alive {
                Color::srgb(0.2, 0.6, 1.0)
            } else {
                Color::srgba(0.5, 0.5, 0.5, 0.3)
            };
            let mesh = meshes.add(Capsule3d::new(0.4, 1.8));
            let mat = materials.add(StandardMaterial {
                base_color: mat_color,
                ..default()
            });

            let entity = commands
                .spawn((
                    PlayerEntity { id: ps.id },
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(ps.position.x, ps.position.y + 0.9, ps.position.z),
                    GlobalTransform::default(),
                ))
                .id();

            // Add rapier3d character controller in native builds.
            #[cfg(feature = "native")]
            commands
                .entity(entity)
                .insert(Collider::capsule_y(0.9, 0.4))
                .insert(KinematicCharacterController {
                    up: Vec3::Y,
                    slide: true,
                    ..default()
                });
        }
    }
}

// ===========================================================================
// HUD helpers (placeholder — a real game would use bevy_egui or a UI plugin)
// ===========================================================================

/// Return a string summary of the local player's state for an in-game HUD.
///
/// In a full implementation this feeds into a `bevy_egui` HUD system.
#[allow(dead_code)]
pub fn hud_text(state: &GameState, local_player_id: PlayerId) -> String {
    if let Some(ps) = state.player(local_player_id) {
        let c = FpsPlayerCustom::from_player(ps);
        format!(
            "HP: {:.0}/{:.0}  Ammo: {}/{}  Kills: {}  Deaths: {}",
            ps.health, ps.max_health, c.ammo, c.ammo_reserve, c.kills, c.deaths
        )
    } else {
        "Spectating".to_string()
    }
}
