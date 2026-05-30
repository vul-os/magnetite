//! # Magnetite SDK
//!
//! **Magnetite SDK** is the Rust library for building multiplayer games on the
//! [Magnetite platform](https://magnetite.gg) — games that scale from a
//! weekend game jam to a COD-size AAA title.
//!
//! ## Core concepts
//!
//! | Module | Purpose |
//! |---|---|
//! | [`game`] | [`game::GameLogic`] trait — the entry point for every game |
//! | [`input`] | Strongly-typed input frames and actions |
//! | [`state`] | Game state, snapshots, player state |
//! | [`protocol`] | Versioned wire protocol (client ↔ server) |
//! | [`networking`] | Server config, tick loop, prediction, interest management |
//!
//! ## Quick start
//!
//! 1. Implement [`game::GameLogic`] for your game struct.
//! 2. Register it with [`export_game!`].
//! 3. `cargo build --target wasm32-unknown-unknown` for the browser client or
//!    `cargo build` for the native server binary.
//!
//! ```rust
//! use magnetite_sdk::{
//!     export_game,
//!     game::{GameLogic, GameMetadata},
//!     input::{Action, Input},
//!     state::{GameState, PlayerId, Snapshot},
//! };
//!
//! struct MyGame {
//!     state: GameState,
//! }
//!
//! impl GameLogic for MyGame {
//!     fn new() -> Self { MyGame { state: GameState::default() } }
//!     fn handle_input(&mut self, _pid: PlayerId, _input: Input) -> Action { Action::None }
//!     fn tick(&mut self) { self.state.tick += 1; }
//!     fn state(&self) -> &GameState { &self.state }
//!     fn players(&self) -> Vec<PlayerId> { vec![] }
//!     fn metadata(&self) -> GameMetadata { GameMetadata::default() }
//!     fn snapshot(&self) -> Snapshot { Snapshot::new(self.state.tick, self.state.clone()) }
//!     fn restore(&mut self, snap: Snapshot) { self.state = snap.state; }
//! }
//!
//! export_game!(MyGame);
//! ```
//!
//! ## Platform services
//!
//! The [`platform`] module exposes shared platform services that in-game code
//! can call directly:
//!
//! | Service | Path |
//! |---|---|
//! | Chat + presence | [`platform::comms`] |
//! | Voice signaling | [`platform::comms::VoiceSignal`] |
//!
//! ## Feature flags
//!
//! *(Planned — not yet implemented.)*
//!
//! | Flag | Description |
//! |---|---|
//! | `binary` | Switch the wire protocol codec from JSON to MessagePack |
//! | `bevy` | Enable Bevy integration helpers |
//!
//! ## License
//!
//! MIT — see the repository root.

pub mod game;
pub mod input;
pub mod networking;
pub mod platform;
pub mod protocol;
pub mod state;

// Convenience re-exports of the most commonly used types.
pub use game::{GameLogic, GameMetadata};
pub use input::{Action, Direction, Input, InputEvent, KeyCode, KeyState, MouseState};
pub use networking::{
    FullInterest, InterestManager, NetworkManager, PredictionBuffer, RadiusInterest, ServerConfig,
    ServerNetworkManager, StateSyncProtocol, TickLoop,
};
pub use platform::comms::{
    ChatMessage, ClientCommsMessage, CommsClient, CommsConfig, CommsConnectionState,
    CommsErrorCode, CommsEvent, PresenceStatus, PresenceUpdate, ServerCommsMessage, VoiceSignal,
};
pub use protocol::{ClientMessage, Envelope, ErrorCode, ServerMessage, PROTOCOL_VERSION};
pub use state::{GameState, PlayerId, PlayerState, Position, Rotation, Snapshot};

// ---------------------------------------------------------------------------
// export_game! macro
// ---------------------------------------------------------------------------

/// Register a [`GameLogic`] implementation with the Magnetite runtime.
///
/// This macro emits the FFI glue that the Magnetite server and WASM host use
/// to discover, instantiate, and drive your game. You **must** call it exactly
/// once per crate, at the crate root.
///
/// # What it generates
///
/// - `magnetite_game_new() -> *mut GameBox` — factory function called by the
///   runtime to create a fresh game instance.
/// - `magnetite_game_metadata() -> *mut u8` (JSON) — returns
///   [`GameMetadata`](game::GameMetadata) as a heap-allocated JSON string.
/// - A type alias `GameBox` for `Box<dyn std::any::Any>` for internal use.
///
/// # Example
///
/// ```rust
/// use magnetite_sdk::{
///     export_game,
///     game::{GameLogic, GameMetadata},
///     input::{Action, Input},
///     state::{GameState, PlayerId, Snapshot},
/// };
///
/// struct Pong { state: GameState }
///
/// impl GameLogic for Pong {
///     fn new() -> Self { Pong { state: GameState::default() } }
///     fn handle_input(&mut self, _p: PlayerId, _i: Input) -> Action { Action::None }
///     fn tick(&mut self) { self.state.tick += 1; }
///     fn state(&self) -> &GameState { &self.state }
///     fn players(&self) -> Vec<PlayerId> { vec![] }
///     fn metadata(&self) -> GameMetadata { GameMetadata::default() }
///     fn snapshot(&self) -> Snapshot { Snapshot::new(self.state.tick, self.state.clone()) }
///     fn restore(&mut self, s: Snapshot) { self.state = s.state; }
/// }
///
/// export_game!(Pong);
/// ```
#[macro_export]
macro_rules! export_game {
    ($game_type:ty) => {
        /// Opaque box used by the Magnetite runtime.
        #[doc(hidden)]
        pub type MagnetiteGameBox = Box<$game_type>;

        /// Called by the Magnetite runtime to create a new game instance.
        ///
        /// # Safety
        ///
        /// The returned pointer is heap-allocated and must be freed by calling
        /// `magnetite_game_destroy`.
        #[no_mangle]
        pub unsafe extern "C" fn magnetite_game_new() -> *mut MagnetiteGameBox {
            use $crate::game::GameLogic;
            Box::into_raw(Box::new(Box::new(<$game_type>::new())))
        }

        /// Destroy a game instance previously created by `magnetite_game_new`.
        ///
        /// # Safety
        ///
        /// `ptr` must be a valid pointer returned by `magnetite_game_new` and
        /// must not have already been destroyed.
        #[no_mangle]
        pub unsafe extern "C" fn magnetite_game_destroy(ptr: *mut MagnetiteGameBox) {
            if !ptr.is_null() {
                drop(Box::from_raw(ptr));
            }
        }

        /// Return game metadata as a null-terminated JSON string.
        ///
        /// The caller is responsible for freeing the returned pointer with
        /// `magnetite_free_string`.
        ///
        /// # Safety
        ///
        /// `ptr` must be a valid pointer returned by `magnetite_game_new`.
        #[no_mangle]
        pub unsafe extern "C" fn magnetite_game_metadata(
            ptr: *mut MagnetiteGameBox,
        ) -> *mut std::os::raw::c_char {
            use $crate::game::GameLogic;
            let game = &**ptr;
            let meta = game.metadata();
            let json = $crate::serde_json::to_string(&meta).unwrap_or_default();
            let cstring = std::ffi::CString::new(json).unwrap_or_default();
            cstring.into_raw()
        }

        /// Free a string allocated by the Magnetite SDK.
        ///
        /// # Safety
        ///
        /// `ptr` must have been returned by a Magnetite SDK function that
        /// documents this as the freeing mechanism.
        #[no_mangle]
        pub unsafe extern "C" fn magnetite_free_string(ptr: *mut std::os::raw::c_char) {
            if !ptr.is_null() {
                drop(std::ffi::CString::from_raw(ptr));
            }
        }
    };
}

// Make serde_json available inside the macro expansion without callers needing
// to import it.
#[doc(hidden)]
pub use serde_json;

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal game to exercise all trait methods in the integration tests.
    struct TestGame {
        state: GameState,
    }

    impl GameLogic for TestGame {
        fn new() -> Self {
            TestGame {
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
            GameMetadata {
                name: "Test Game".to_string(),
                version: "0.0.1".to_string(),
                max_players: 4,
                min_players: 1,
                tick_rate: 60,
                description: "Integration test game.".to_string(),
            }
        }

        fn snapshot(&self) -> Snapshot {
            Snapshot::new(self.state.tick, self.state.clone())
        }

        fn restore(&mut self, snap: Snapshot) {
            self.state = snap.state;
        }
    }

    #[test]
    fn game_logic_full_cycle() {
        let mut game = TestGame::new();

        // Tick a few times.
        game.tick();
        game.tick();
        assert_eq!(game.state().tick, 2);

        // Snapshot and restore.
        let snap = game.snapshot();
        assert!(snap.verify());
        game.tick();
        assert_eq!(game.state().tick, 3);
        game.restore(snap);
        assert_eq!(game.state().tick, 2);

        // Metadata is sound.
        let meta = game.metadata();
        assert_eq!(meta.tick_rate, 60);
        assert!(meta.max_players >= meta.min_players);
    }

    #[test]
    fn snapshot_serialises_and_restores() {
        let mut game = TestGame::new();
        game.tick();
        let snap = game.snapshot();

        let bytes = serde_json::to_vec(&snap).unwrap();
        let snap2: Snapshot = serde_json::from_slice(&bytes).unwrap();

        assert!(snap2.verify());
        game.restore(snap2);
        assert_eq!(game.state().tick, 1);
    }

    #[test]
    fn protocol_version_constant() {
        assert!(PROTOCOL_VERSION > 0, "PROTOCOL_VERSION must be ≥ 1");
    }

    #[test]
    fn envelope_new_sets_version() {
        let env = Envelope::new(ClientMessage::Disconnect);
        assert_eq!(env.version, PROTOCOL_VERSION);
    }

    #[test]
    fn server_config_defaults_sane() {
        let cfg = ServerConfig::default();
        assert!(cfg.tick_rate > 0);
        assert!(cfg.max_players > 0);
    }

    #[test]
    fn prediction_buffer_roundtrip() {
        let mut buf = PredictionBuffer::new(16);
        for seq in 0..5u64 {
            buf.push(Input {
                sequence: seq,
                ..Default::default()
            });
        }
        assert_eq!(buf.len(), 5);
        buf.acknowledge(3);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.pending()[0].sequence, 4);
    }
}
