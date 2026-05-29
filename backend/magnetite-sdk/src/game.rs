//! The core [`GameLogic`] trait and supporting metadata types.
//!
//! Every Magnetite game implements [`GameLogic`]. The trait covers the full
//! lifecycle needed for both *simple* (game-jam size) and *large* (AAA-scale
//! server-authoritative) games:
//!
//! | Method | Purpose |
//! |---|---|
//! | [`GameLogic::new`] | Construct the initial game state |
//! | [`GameLogic::handle_input`] | Deterministically interpret one input frame → action |
//! | [`GameLogic::tick`] | Advance physics / AI / timers by one server tick |
//! | [`GameLogic::state`] | Return the current authoritative [`GameState`] |
//! | [`GameLogic::players`] | Enumerate connected [`PlayerId`]s |
//! | [`GameLogic::metadata`] | Return static game metadata |
//! | [`GameLogic::snapshot`] | Capture a complete [`Snapshot`] (save / replay) |
//! | [`GameLogic::restore`] | Roll back to a previously captured [`Snapshot`] |
//! | [`GameLogic::on_player_join`] | A new player connected — add to state |
//! | [`GameLogic::on_player_leave`] | A player disconnected — clean up state |
//!
//! # Snapshot / Restore (save & replay)
//!
//! The platform calls [`GameLogic::snapshot`] periodically and stores the
//! result in the replay buffer. [`GameLogic::restore`] re-winds the game to an
//! earlier snapshot, enabling:
//!
//! - **Replays:** replay the recorded input log from a snapshot.
//! - **Client-side prediction:** the client runs `tick` + `restore` in a tight
//!   loop to reconcile against the server's authoritative state.
//! - **Save / load:** persist a snapshot to disk between sessions.
//!
//! # Example — minimal implementation
//!
//! ```rust
//! use magnetite_sdk::{
//!     game::{GameLogic, GameMetadata},
//!     input::{Action, Input},
//!     state::{GameState, PlayerId, PlayerState, Position, Rotation, Snapshot},
//! };
//!
//! struct PingPong {
//!     state: GameState,
//! }
//!
//! impl GameLogic for PingPong {
//!     fn new() -> Self {
//!         PingPong { state: GameState::default() }
//!     }
//!
//!     fn handle_input(&mut self, _player_id: PlayerId, _input: Input) -> Action {
//!         Action::None
//!     }
//!
//!     fn tick(&mut self) {
//!         self.state.tick += 1;
//!     }
//!
//!     fn state(&self) -> &GameState {
//!         &self.state
//!     }
//!
//!     fn players(&self) -> Vec<PlayerId> {
//!         self.state.players.iter().map(|p| p.id).collect()
//!     }
//!
//!     fn metadata(&self) -> GameMetadata {
//!         GameMetadata {
//!             name: "PingPong".to_string(),
//!             version: "0.1.0".to_string(),
//!             max_players: 2,
//!             min_players: 2,
//!             tick_rate: 60,
//!             description: "Classic table-tennis.".to_string(),
//!         }
//!     }
//!
//!     fn snapshot(&self) -> Snapshot {
//!         Snapshot::new(self.state.tick, self.state.clone())
//!     }
//!
//!     fn restore(&mut self, snapshot: Snapshot) {
//!         self.state = snapshot.state;
//!     }
//!
//!     fn on_player_join(&mut self, player_id: PlayerId) {
//!         self.state.players.push(PlayerState {
//!             id: player_id,
//!             position: Position::default(),
//!             rotation: Rotation::default(),
//!             health: 100.0,
//!             max_health: 100.0,
//!             alive: true,
//!             score: 0,
//!             custom: serde_json::Value::Null,
//!         });
//!     }
//!
//!     fn on_player_leave(&mut self, player_id: PlayerId) {
//!         self.state.remove_player(player_id);
//!     }
//! }
//!
//! let mut game = PingPong::new();
//! game.on_player_join(PlayerId::new(1));
//! game.tick();
//! let snap = game.snapshot();
//! game.tick();
//! game.restore(snap);           // rolled back one tick
//! assert_eq!(game.state().tick, 1);
//! ```

use crate::input::{Action, Input};
use crate::state::{GameState, PlayerId, Snapshot};

/// Static metadata that describes a game to the Magnetite platform.
///
/// Returned by [`GameLogic::metadata`]; used by matchmaking, the storefront,
/// and the server runtime.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameMetadata {
    /// Human-readable display name (e.g. `"Stellar Siege"`).
    pub name: String,
    /// Semantic version string (e.g. `"1.2.3"`).
    pub version: String,
    /// Maximum number of simultaneous players.
    pub max_players: usize,
    /// Minimum players required to start a match.
    pub min_players: usize,
    /// Authoritative tick rate in Hz (frames per second on the server).
    ///
    /// Common values: `20` (low-bandwidth turn-based), `60` (action),
    /// `128` (competitive FPS).
    pub tick_rate: u32,
    /// Short description shown in the storefront (plain text, ≤ 200 chars).
    pub description: String,
}

impl Default for GameMetadata {
    fn default() -> Self {
        Self {
            name: "Unnamed Game".to_string(),
            version: "0.1.0".to_string(),
            max_players: 16,
            min_players: 1,
            tick_rate: 60,
            description: String::new(),
        }
    }
}

/// The core trait every Magnetite game must implement.
///
/// See the [module documentation](crate::game) for a complete example.
///
/// ## Object safety
///
/// [`GameLogic`] is *not* object-safe because of the `fn new() -> Self`
/// associated function. Use the [`export_game!`](crate::export_game) macro
/// (which wraps `new` in a `Box<dyn GameLogic>` factory) for dynamic dispatch.
pub trait GameLogic {
    /// Construct the game in its initial state (no players connected yet).
    fn new() -> Self
    where
        Self: Sized;

    /// Deterministically translate one player's input frame into an action.
    ///
    /// This function **must be pure** with respect to the game state visible
    /// through `&mut self`: it may mutate state *only* as a result of the
    /// input (i.e. moving a player). The platform may call this repeatedly
    /// with the same arguments during client prediction.
    fn handle_input(&mut self, player_id: PlayerId, input: Input) -> Action;

    /// Advance the simulation by exactly one tick.
    ///
    /// The server calls this once per tick interval (see
    /// [`GameMetadata::tick_rate`]). Physics, AI, and timer logic live here.
    fn tick(&mut self);

    /// Return an immutable reference to the current authoritative [`GameState`].
    fn state(&self) -> &GameState;

    /// Return the ids of all currently connected players.
    fn players(&self) -> Vec<PlayerId>;

    /// Return static metadata about this game.
    fn metadata(&self) -> GameMetadata;

    /// Capture the complete game state as a [`Snapshot`].
    ///
    /// The platform calls this periodically (configurable interval). Store the
    /// full state so that [`restore`](GameLogic::restore) can reconstruct it
    /// exactly.
    fn snapshot(&self) -> Snapshot;

    /// Roll back the game to a previously captured [`Snapshot`].
    ///
    /// After this call `self.state()` must equal the state embedded in
    /// `snapshot`. Used for replays and client-side prediction rollback.
    fn restore(&mut self, snapshot: Snapshot);

    /// Called when a new player joins the session.
    ///
    /// Default implementation is a no-op; override to add the player to your
    /// state (spawn a character, assign a team, etc.).
    fn on_player_join(&mut self, _player_id: PlayerId) {}

    /// Called when a player leaves the session (disconnect or quit).
    ///
    /// Default implementation is a no-op; override to clean up (despawn
    /// character, transfer ownership, etc.).
    fn on_player_leave(&mut self, _player_id: PlayerId) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{PlayerState, Position, Rotation};

    struct CounterGame {
        state: GameState,
    }

    impl GameLogic for CounterGame {
        fn new() -> Self {
            CounterGame {
                state: GameState::default(),
            }
        }

        fn handle_input(&mut self, _pid: PlayerId, _input: Input) -> Action {
            Action::None
        }

        fn tick(&mut self) {
            self.state.tick += 1;
        }

        fn state(&self) -> &GameState {
            &self.state
        }

        fn players(&self) -> Vec<PlayerId> {
            self.state.players.iter().map(|p| p.id).collect()
        }

        fn metadata(&self) -> GameMetadata {
            GameMetadata::default()
        }

        fn snapshot(&self) -> Snapshot {
            Snapshot::new(self.state.tick, self.state.clone())
        }

        fn restore(&mut self, snap: Snapshot) {
            self.state = snap.state;
        }

        fn on_player_join(&mut self, player_id: PlayerId) {
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

        fn on_player_leave(&mut self, player_id: PlayerId) {
            self.state.remove_player(player_id);
        }
    }

    #[test]
    fn tick_increments_state_tick() {
        let mut game = CounterGame::new();
        assert_eq!(game.state().tick, 0);
        game.tick();
        game.tick();
        assert_eq!(game.state().tick, 2);
    }

    #[test]
    fn snapshot_restore_rollback() {
        let mut game = CounterGame::new();
        game.tick();
        let snap = game.snapshot();
        assert_eq!(snap.tick, 1);

        game.tick();
        game.tick();
        assert_eq!(game.state().tick, 3);

        game.restore(snap);
        assert_eq!(game.state().tick, 1);
    }

    #[test]
    fn player_join_leave() {
        let mut game = CounterGame::new();
        game.on_player_join(PlayerId::new(10));
        game.on_player_join(PlayerId::new(11));
        assert_eq!(game.players().len(), 2);
        game.on_player_leave(PlayerId::new(10));
        assert_eq!(game.players().len(), 1);
    }

    #[test]
    fn metadata_defaults_sane() {
        let game = CounterGame::new();
        let meta = game.metadata();
        assert!(meta.tick_rate > 0);
        assert!(meta.max_players >= meta.min_players);
    }
}
