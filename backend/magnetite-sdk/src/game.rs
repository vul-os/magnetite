use crate::input::{Action, Input};
use crate::state::PlayerId;
use crate::GameState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameMetadata {
    pub name: String,
    pub max_players: usize,
    pub tick_rate: u32,
}

pub trait GameLogic {
    fn new() -> Self;
    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action;
    fn tick(&mut self);
    fn state(&self) -> GameState;
    fn players(&self) -> Vec<PlayerId>;
    fn metadata(&self) -> GameMetadata;
}
