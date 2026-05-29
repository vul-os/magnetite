use bevy::prelude::*;
use magnetite_sdk::{
    GameLogic, GameState, Input, KeyCode, KeyState, MouseState, PlayerId, PlayerState, Position, Rotation,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Resource, Default)]
pub struct GamePluginState {
    pub players: HashMap<PlayerId, PlayerState>,
    pub next_player_id: u64,
}

impl GamePluginState {
    pub fn add_player(&mut self) -> PlayerId {
        let id = PlayerId(self.next_player_id);
        self.next_player_id += 1;
        let player_state = PlayerState {
            id,
            position: Position { x: 0.0, y: 0.0, z: 0.0 },
            rotation: Rotation { pitch: 0.0, yaw: 0.0 },
            health: 100.0,
        };
        self.players.insert(id, player_state);
        id
    }
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePluginState>()
            .add_systems(Update, (handle_input_system, tick_system));
    }
}

fn handle_input_system(
    mut state: ResMut<GamePluginState>,
    input_events: Res<InputEvents>,
) {
    for event in input_events.events.iter() {
        match event {
            InputEvent::Press(key) => {
                let player_id = PlayerId(0);
                if let Some(player) = state.players.get_mut(&player_id) {
                    match key {
                        KeyCode::Forward => player.position.z -= 0.1,
                        KeyCode::Backward => player.position.z += 0.1,
                        KeyCode::Left => player.position.x -= 0.1,
                        KeyCode::Right => player.position.x += 0.1,
                        KeyCode::Jump => player.position.y += 0.1,
                        KeyCode::Crouch => player.position.y -= 0.1,
                    }
                }
            }
            InputEvent::Release(_) => {}
            InputEvent::MouseMove { x, y } => {
                let player_id = PlayerId(0);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.rotation.yaw = *x as f32 * 0.01;
                    player.rotation.pitch = *y as f32 * 0.01;
                }
            }
            InputEvent::MouseDelta { dx, dy } => {
                let player_id = PlayerId(0);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.rotation.yaw = *dx as f32 * 0.01;
                    player.rotation.pitch = *dy as f32 * 0.01;
                }
            }
        }
    }
}

fn tick_system(mut state: ResMut<GamePluginState>) {
    for player in state.players.values_mut() {
        player.health = player.health.min(100.0).max(0.0);
    }
}

#[derive(Default)]
pub struct InputEvents {
    pub events: Vec<InputEvent>,
}

#[derive(Clone, Debug)]
pub enum InputEvent {
    Press(KeyCode),
    Release(KeyCode),
    MouseMove { x: f64, y: f64 },
    MouseDelta { dx: f64, dy: f64 },
}

impl GameLogic for GamePluginState {
    fn new() -> Self {
        let mut state = GamePluginState::default();
        state.add_player();
        state
    }

    fn handle_input(&mut self, player: PlayerId, input: Input) {
        if let Some(player_state) = self.players.get_mut(&player) {
            if input.keys.forward {
                player_state.position.z -= 0.1;
            }
            if input.keys.backward {
                player_state.position.z += 0.1;
            }
            if input.keys.left {
                player_state.position.x -= 0.1;
            }
            if input.keys.right {
                player_state.position.x += 0.1;
            }
            if input.keys.jump {
                player_state.position.y += 0.1;
            }
            if input.keys.crouch {
                player_state.position.y -= 0.1;
            }
            player_state.rotation.yaw = input.mouse.delta_x as f32 * 0.01;
            player_state.rotation.pitch = input.mouse.delta_y as f32 * 0.01;
        }
    }

    fn tick(&mut self) {
        tick_system(self.reborrow());
    }

    fn state(&self) -> GameState {
        GameState {
            players: self.players.values().cloned().collect(),
        }
    }

    fn players(&self) -> Vec<PlayerId> {
        self.players.keys().cloned().collect()
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    App::new()
        .add_plugins((MinimalPlugins, GamePlugin))
        .run();
}

#[cfg(target_arch = "wasm32")]
pub fn run_game() {
    App::new()
        .add_plugins((MinimalPlugins, GamePlugin))
        .run();
}

mod reborrow {
    use super::*;

    pub trait Reborrow {
        fn reborrow(&mut self) -> GamePluginState;
    }

    impl Reborrow for GamePluginState {
        fn reborrow(&mut self) -> GamePluginState {
            GamePluginState {
                players: self.players.clone(),
                next_player_id: self.next_player_id,
            }
        }
    }
}
