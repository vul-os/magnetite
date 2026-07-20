//! Bevy application — 2-D arena renderer + game loop.
//!
//! This module requires the `render` feature flag (which pulls in the full
//! Bevy default feature set). It is excluded from `--no-default-features` builds
//! so that `cargo check --no-default-features` stays fast for CI.
//!
//! # Architecture
//!
//! ```text
//! Bevy App
//!   ├── NetPlugin         — drives the tokio WS task via channel resources
//!   ├── PredictionPlugin  — wraps ClientPredictor as a Bevy Resource
//!   └── RenderPlugin      — 2-D sprites for players + projectiles
//!
//! Each frame (FixedUpdate at tick_hz):
//!   1. InputSystem        — sample Bevy Input<KeyCode> + cursor position
//!   2. PredictSystem      — ClientPredictor::predict(input) → send ClientNet
//!   3. NetReceiveSystem   — drain rx_from_server → reconcile_ack / reconcile_snapshot
//!   4. RenderSystem       — spawn/despawn/move Sprite entities to match PredictedState
//! ```

use bevy::prelude::*;
use tokio::sync::mpsc;

use magnetite_sdk::{
    input::{Input as GameInput, KeyState, MouseState},
    protocol::{ClientNet, ServerNet},
    state::PlayerId,
    MatchConfig,
};

use crate::{
    net::{NetChannels, NetConfig},
    prediction::{ClientPredictor, PredictedState},
};

// ─────────────────────────────────────────────────────────────────────────────
// Resources
// ─────────────────────────────────────────────────────────────────────────────

/// Bevy resource wrapping the network channels.
#[derive(Resource)]
pub struct NetResource {
    pub channels: NetChannels,
}

/// Bevy resource wrapping the prediction engine.
#[derive(Resource)]
pub struct PredictorResource {
    pub predictor: ClientPredictor,
}

/// Bevy resource holding the current match configuration.
#[derive(Resource, Default)]
pub struct MatchConfigResource {
    pub config: Option<MatchConfig>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Components
// ─────────────────────────────────────────────────────────────────────────────

/// Marks an entity as representing a shooter player.
#[derive(Component)]
pub struct PlayerSprite {
    pub player_id: PlayerId,
}

/// Marks an entity as representing an in-flight projectile.
#[derive(Component)]
pub struct ProjectileSprite {
    pub projectile_id: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin: Net
// ─────────────────────────────────────────────────────────────────────────────

/// Bevy plugin that manages the WebSocket network task.
///
/// Call `NetPlugin::new(config)` and add it to the Bevy App.
pub struct NetPlugin {
    pub config: NetConfig,
}

impl NetPlugin {
    pub fn new(config: NetConfig) -> Self {
        Self { config }
    }
}

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        let channels = crate::net::spawn_net_task(self.config.clone());
        app.insert_resource(NetResource { channels });
        app.add_systems(Update, process_server_messages);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin: Prediction
// ─────────────────────────────────────────────────────────────────────────────

/// Bevy plugin that registers the prediction engine and drives the game loop.
pub struct PredictionPlugin {
    pub player_id: PlayerId,
    pub buffer_capacity: usize,
}

impl PredictionPlugin {
    pub fn new(player_id: PlayerId) -> Self {
        Self {
            player_id,
            buffer_capacity: 128,
        }
    }
}

impl Plugin for PredictionPlugin {
    fn build(&self, app: &mut App) {
        let predictor = ClientPredictor::new(self.player_id, self.buffer_capacity);
        app.insert_resource(PredictorResource { predictor });
        app.insert_resource(MatchConfigResource::default());
        app.add_systems(FixedUpdate, client_tick_system);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin: Render
// ─────────────────────────────────────────────────────────────────────────────

/// Bevy plugin that renders the arena as simple 2-D coloured rectangles.
pub struct ArenaRenderPlugin;

impl Plugin for ArenaRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_arena_background);
        app.add_systems(Update, render_players_and_projectiles);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Systems
// ─────────────────────────────────────────────────────────────────────────────

/// Drain all pending server messages and apply them to the predictor.
fn process_server_messages(
    mut net: ResMut<NetResource>,
    mut predictor_res: ResMut<PredictorResource>,
    mut config_res: ResMut<MatchConfigResource>,
) {
    let predictor = &mut predictor_res.predictor;
    // Drain up to 64 messages per frame to avoid frame budget overruns.
    for _ in 0..64 {
        match net.channels.rx_from_server.try_recv() {
            Ok(msg) => handle_server_message(msg, predictor, &mut config_res),
            Err(_) => break,
        }
    }
}

fn handle_server_message(
    msg: ServerNet,
    predictor: &mut ClientPredictor,
    config_res: &mut MatchConfigResource,
) {
    match msg {
        ServerNet::Welcome {
            player_id: _,
            config,
        } => {
            predictor.on_welcome(&config);
            config_res.config = Some(config);
        }

        ServerNet::Snapshot { tick: _, full } => {
            // Deserialise the snapshot and reconcile.
            if let Err(e) = predictor.reconcile_snapshot(&full) {
                eprintln!("[client] failed to deserialise snapshot: {e}");
            }
        }

        ServerNet::Delta {
            tick: _,
            since_tick,
            diff,
        } => {
            // Apply the delta.
            if let Err(e) = predictor.apply_delta(since_tick, &diff) {
                eprintln!("[client] failed to apply delta: {e}");
            }
        }

        ServerNet::Ack { seq, tick: _ } => {
            // We need the server view to reconcile properly. In a real client
            // the server would also include the player's authoritative view in
            // the Ack (or we derive it from the last Delta). For the reference
            // client we use the last predicted authoritative state.
            let auth_view =
                build_view_from_predicted(&predictor.authoritative, predictor.player_id);
            predictor.reconcile_ack(seq, auth_view);
        }

        ServerNet::Reject { seq, reason } => {
            eprintln!("[client] input seq={seq} rejected: {reason}");
            // Force reconcile from the current authoritative state.
            let auth_view =
                build_view_from_predicted(&predictor.authoritative, predictor.player_id);
            predictor.reconcile_ack(seq, auth_view);
        }

        // Fleet session-follow frames. This reference client is single-node and
        // does not follow migrated shards, so it must NOT act on them — a
        // client that half-follows a redirect (reconnecting without verifying
        // the issuer signature and pinning `target_key`) is exactly the hijack
        // the protocol exists to prevent. See `magnetite-web-client` for a
        // client that implements the follow properly.
        ServerNet::NodeIdentity { .. } => {}
        ServerNet::Redirect { .. } => {
            eprintln!(
                "[client] server sent a session redirect; this client does not \
                 implement session follow — disconnecting rather than following \
                 it unverified"
            );
        }

        // Attested-input answers (seam §3.7). This reference client is a
        // keyboard/gamepad client: its input is `InputClass::Deterministic` and
        // it never sends a `ClientNet::AttestedEvent`, so an answer to one is
        // not addressed to it.
        //
        // Critically these must NOT be routed into `predictor.reconcile_ack`
        // like `ServerNet::Ack`/`Reject` are. Those carry the client-local input
        // sequence; an attested `seq` is an unrelated per-player counter, so
        // feeding one to the `PredictionBuffer` would discard correct prediction
        // frames. The class boundary that makes `verify_replay` meaningful is
        // the same boundary here.
        ServerNet::AttestedAck { .. } | ServerNet::AttestedReject { .. } => {}
    }
}

/// Build an [`ArenaView`] from a [`PredictedState`] for reconciliation.
fn build_view_from_predicted(
    state: &PredictedState,
    player_id: PlayerId,
) -> game_template_authoritative::types::ArenaView {
    game_template_authoritative::types::ArenaView {
        self_state: state.self_player.clone(),
        other_players: state.other_players.clone(),
        projectiles: state.projectiles.clone(),
        tick: state.authoritative_tick,
    }
}

/// One authoritative-rate tick: sample input → predict → enqueue to server.
fn client_tick_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut predictor_res: ResMut<PredictorResource>,
    net: Res<NetResource>,
) {
    let predictor = &mut predictor_res.predictor;

    // ── Sample keyboard ───────────────────────────────────────────────────
    let keys = KeyState {
        forward: keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp),
        backward: keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown),
        left: keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft),
        right: keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight),
        attack: keyboard.pressed(KeyCode::Space) || keyboard.pressed(KeyCode::KeyZ),
        ..Default::default()
    };

    // ── Sample mouse cursor in world space ────────────────────────────────
    let mut mouse = MouseState::default();
    if let Ok(window) = windows.get_single() {
        if let Some(cursor) = window.cursor_position() {
            mouse.x = cursor.x as f64;
            mouse.y = cursor.y as f64;

            // Optional: convert to world coords via camera ray.
            if let Ok((camera, cam_transform)) = camera.get_single() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor) {
                    mouse.x = world_pos.x as f64;
                    mouse.y = world_pos.y as f64;
                }
            }
        }
    }

    let input = GameInput {
        keys,
        mouse,
        sequence: 0, // overwritten by predict()
        timestamp_ms: 0,
    };

    // ── Predict + send ────────────────────────────────────────────────────
    let frame = predictor.predict(input);
    let _ = net.channels.tx_to_server.try_send(frame);
}

/// Spawn the arena boundary as a hollow rectangle.
fn spawn_arena_background(mut commands: Commands) {
    use game_template_authoritative::types::{ARENA_HEIGHT, ARENA_WIDTH};

    // Arena border — a large slightly lighter sprite with a smaller darker
    // sprite on top to create the illusion of walls.
    commands.spawn(Sprite {
        color: Color::srgb(0.15, 0.15, 0.2),
        custom_size: Some(Vec2::new(ARENA_WIDTH + 10.0, ARENA_HEIGHT + 10.0)),
        ..default()
    });
    commands.spawn(Sprite {
        color: Color::srgb(0.05, 0.05, 0.08),
        custom_size: Some(Vec2::new(ARENA_WIDTH, ARENA_HEIGHT)),
        ..default()
    });
}

/// Sync player and projectile sprites with the current predicted state.
fn render_players_and_projectiles(
    mut commands: Commands,
    predictor_res: Res<PredictorResource>,
    mut player_sprites: Query<(Entity, &PlayerSprite, &mut Transform)>,
    mut proj_sprites: Query<(Entity, &ProjectileSprite, &mut Transform)>,
) {
    let state = &predictor_res.predictor.predicted;

    // ── Players ────────────────────────────────────────────────────────────
    // Collect all known player ids in the predicted state.
    let mut known_player_ids: std::collections::HashSet<PlayerId> =
        std::collections::HashSet::new();
    if let Some(ref p) = state.self_player {
        known_player_ids.insert(p.id);
    }
    for p in &state.other_players {
        known_player_ids.insert(p.id);
    }

    // Update or despawn existing sprites.
    for (entity, sprite, mut transform) in player_sprites.iter_mut() {
        let player = state
            .self_player
            .iter()
            .find(|p| p.id == sprite.player_id)
            .or_else(|| {
                state
                    .other_players
                    .iter()
                    .find(|p| p.id == sprite.player_id)
            });

        match player {
            Some(p) if p.alive => {
                transform.translation.x = p.x;
                transform.translation.y = p.y;
                transform.rotation = Quat::from_rotation_z(p.angle);
            }
            _ => {
                commands.entity(entity).despawn();
            }
        }
    }

    // Spawn sprites for new players.
    let existing_ids: std::collections::HashSet<PlayerId> =
        player_sprites.iter().map(|(_, s, _)| s.player_id).collect();

    let self_id = predictor_res.predictor.player_id;

    let all_players: Vec<_> = state
        .self_player
        .iter()
        .chain(state.other_players.iter())
        .filter(|p| p.alive && !existing_ids.contains(&p.id))
        .collect();

    for player in all_players {
        // Self = bright green; others = red.
        let color = if player.id == self_id {
            Color::srgb(0.0, 0.9, 0.3)
        } else {
            Color::srgb(0.9, 0.2, 0.2)
        };

        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::splat(12.0)),
                ..default()
            },
            Transform::from_xyz(player.x, player.y, 1.0)
                .with_rotation(Quat::from_rotation_z(player.angle)),
            PlayerSprite {
                player_id: player.id,
            },
        ));
    }

    // ── Projectiles ────────────────────────────────────────────────────────
    let known_proj_ids: std::collections::HashSet<u64> =
        state.projectiles.iter().map(|p| p.id).collect();

    // Despawn removed projectiles.
    for (entity, sprite, _) in proj_sprites.iter() {
        if !known_proj_ids.contains(&sprite.projectile_id) {
            commands.entity(entity).despawn();
        }
    }

    // Update positions of existing.
    for (_, sprite, mut transform) in proj_sprites.iter_mut() {
        if let Some(proj) = state
            .projectiles
            .iter()
            .find(|p| p.id == sprite.projectile_id)
        {
            transform.translation.x = proj.x;
            transform.translation.y = proj.y;
        }
    }

    // Spawn new projectiles.
    let existing_proj_ids: std::collections::HashSet<u64> = proj_sprites
        .iter()
        .map(|(_, s, _)| s.projectile_id)
        .collect();

    for proj in &state.projectiles {
        if existing_proj_ids.contains(&proj.id) {
            continue;
        }
        // Own projectiles = yellow; enemy = orange.
        let color = if proj.owner == self_id {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgb(1.0, 0.5, 0.0)
        };
        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::splat(4.0)),
                ..default()
            },
            Transform::from_xyz(proj.x, proj.y, 2.0),
            ProjectileSprite {
                projectile_id: proj.id,
            },
        ));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// App builder — public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Build and return the Bevy [`App`] for the arena shooter client.
///
/// Call `build_app(player_id, net_config).run()` from `main`.
pub fn build_app(player_id: PlayerId, net_config: NetConfig) -> App {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Magnetite Arena Shooter — Client".to_string(),
            resolution: (800., 600.).into(),
            ..default()
        }),
        ..default()
    }));

    app.add_plugins((
        NetPlugin::new(net_config),
        PredictionPlugin::new(player_id),
        ArenaRenderPlugin,
    ));

    // 2-D camera.
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    });

    // Fix update rate to the server tick rate (60 Hz by default).
    app.insert_resource(Time::<Fixed>::from_hz(60.0));

    app
}
