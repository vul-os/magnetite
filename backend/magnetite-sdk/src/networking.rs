//! Networking abstractions for Magnetite games.
//!
//! This module provides a layered set of types and traits that cover the full
//! range from a weekend-jam WebSocket game to a large server-authoritative AAA
//! title:
//!
//! | Layer | Types | Use case |
//! |---|---|---|
//! | Wire transport | [`FramedTransport`] | Byte-level framing (length-prefixed) |
//! | Protocol codec | [`Codec`] | `Envelope` encode/decode |
//! | Server loop | [`ServerConfig`], [`TickLoop`] | Tick rate, snapshot cadence |
//! | Prediction | [`PredictionBuffer`] | Client-side rollback / reconciliation |
//! | Interest management | [`InterestManager`] | AAA-scale relevance culling |
//!
//! # Design notes
//!
//! - **No async runtime is mandated.** The traits use plain `std::io` so they
//!   can be wrapped with `tokio`, `async-std`, or plain blocking threads at
//!   the integration layer. A future `async` feature flag will add `async fn`
//!   variants.
//! - **No heavy deps.** The only dependencies are `serde` + `serde_json`, which
//!   are already in scope. The TCP helpers use `std::net` exclusively.
//!
//! # Minimal server example
//!
//! ```rust,no_run
//! use magnetite_sdk::networking::{ServerConfig, TickLoop};
//!
//! let cfg = ServerConfig::builder()
//!     .tick_rate(60)
//!     .snapshot_interval(300)
//!     .max_players(64)
//!     .build();
//!
//! let _loop = TickLoop::from_config(&cfg);
//! // Drive the loop: call _loop.tick_duration() to sleep between ticks.
//! ```

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::input::Input;
use crate::protocol::{ClientMessage, Envelope, ServerMessage};
use crate::state::{GameState, PlayerId, Snapshot};

// ---------------------------------------------------------------------------
// Framed byte transport
// ---------------------------------------------------------------------------

/// Low-level framing helper: writes / reads length-prefixed byte frames over
/// any [`Read`] + [`Write`] stream.
///
/// Frame format (big-endian):
///
/// ```text
/// ┌─────────────────────────────┬──────────────────────────────┐
/// │  length (u32, 4 bytes, BE)  │  payload (length bytes)      │
/// └─────────────────────────────┴──────────────────────────────┘
/// ```
///
/// This is the same framing used by the legacy [`Connection`] type; it is now
/// factored out so both TCP and in-process test transports can share it.
pub struct FramedTransport<S> {
    stream: S,
}

impl<S: Read + Write> FramedTransport<S> {
    /// Wrap any `Read + Write` stream.
    pub fn new(stream: S) -> Self {
        Self { stream }
    }

    /// Send a raw byte frame.
    pub fn send_frame(&mut self, data: &[u8]) -> std::io::Result<()> {
        let len = data.len() as u32;
        self.stream.write_all(&len.to_be_bytes())?;
        self.stream.write_all(data)?;
        Ok(())
    }

    /// Receive a raw byte frame; blocks until a full frame arrives.
    pub fn recv_frame(&mut self) -> std::io::Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data)?;
        Ok(data)
    }

    /// Consume the transport and return the inner stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

// ---------------------------------------------------------------------------
// Protocol codec
// ---------------------------------------------------------------------------

/// Encode/decode [`Envelope`]-framed messages over a [`FramedTransport`].
///
/// Generic over the transport stream type so it works with TCP, UNIX sockets,
/// or in-process `Cursor<Vec<u8>>` for tests.
pub struct Codec<S> {
    transport: FramedTransport<S>,
}

impl<S: Read + Write> Codec<S> {
    /// Create a codec wrapping an existing framed transport.
    pub fn new(transport: FramedTransport<S>) -> Self {
        Self { transport }
    }

    /// Create a codec directly from a stream.
    pub fn from_stream(stream: S) -> Self {
        Self {
            transport: FramedTransport::new(stream),
        }
    }

    /// Send a [`ClientMessage`] envelope.
    pub fn send_client(&mut self, env: &Envelope<ClientMessage>) -> std::io::Result<()> {
        let bytes = env
            .encode()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        self.transport.send_frame(&bytes)
    }

    /// Send a [`ServerMessage`] envelope.
    pub fn send_server(&mut self, env: &Envelope<ServerMessage>) -> std::io::Result<()> {
        let bytes = env
            .encode()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        self.transport.send_frame(&bytes)
    }

    /// Receive and decode a [`ClientMessage`] envelope.
    pub fn recv_client(&mut self) -> std::io::Result<Envelope<ClientMessage>> {
        let bytes = self.transport.recv_frame()?;
        Envelope::decode(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }

    /// Receive and decode a [`ServerMessage`] envelope.
    pub fn recv_server(&mut self) -> std::io::Result<Envelope<ServerMessage>> {
        let bytes = self.transport.recv_frame()?;
        Envelope::decode(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Server configuration
// ---------------------------------------------------------------------------

/// Configuration for a Magnetite game server session.
///
/// Build with [`ServerConfig::builder()`] or use [`ServerConfig::default()`]
/// for sensible defaults.
///
/// ```rust
/// use magnetite_sdk::networking::ServerConfig;
///
/// let cfg = ServerConfig::builder()
///     .tick_rate(128)
///     .snapshot_interval(64)
///     .max_players(100)
///     .build();
/// assert_eq!(cfg.tick_rate, 128);
/// ```
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Authoritative tick rate in Hz.
    pub tick_rate: u32,
    /// How many ticks between full [`Snapshot`] broadcasts (0 = disabled).
    pub snapshot_interval: u32,
    /// Maximum number of simultaneous players.
    pub max_players: usize,
    /// Drop a client if no input arrives within this many ticks.
    pub timeout_ticks: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            tick_rate: 60,
            snapshot_interval: 300,
            max_players: 64,
            timeout_ticks: 120,
        }
    }
}

impl ServerConfig {
    /// Start building a [`ServerConfig`].
    pub fn builder() -> ServerConfigBuilder {
        ServerConfigBuilder(Self::default())
    }

    /// Returns the duration between ticks.
    #[inline]
    pub fn tick_duration(&self) -> Duration {
        Duration::from_secs(1) / self.tick_rate
    }
}

/// Fluent builder for [`ServerConfig`].
pub struct ServerConfigBuilder(ServerConfig);

impl ServerConfigBuilder {
    pub fn tick_rate(mut self, hz: u32) -> Self {
        self.0.tick_rate = hz;
        self
    }

    pub fn snapshot_interval(mut self, ticks: u32) -> Self {
        self.0.snapshot_interval = ticks;
        self
    }

    pub fn max_players(mut self, n: usize) -> Self {
        self.0.max_players = n;
        self
    }

    pub fn timeout_ticks(mut self, ticks: u32) -> Self {
        self.0.timeout_ticks = ticks;
        self
    }

    pub fn build(self) -> ServerConfig {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Tick loop helper
// ---------------------------------------------------------------------------

/// A simple tick-loop driver.
///
/// The server runtime calls [`TickLoop::should_snapshot`] each tick to decide
/// whether to broadcast a full [`Snapshot`].
///
/// ```rust
/// use magnetite_sdk::networking::{ServerConfig, TickLoop};
///
/// let cfg = ServerConfig::builder().tick_rate(60).snapshot_interval(60).build();
/// let mut tl = TickLoop::from_config(&cfg);
///
/// // Simulate 60 ticks.
/// for _ in 0..60 {
///     tl.advance();
/// }
/// // After 60 ticks with snapshot_interval=60, snapshot is due.
/// assert!(tl.should_snapshot());
/// ```
#[derive(Debug, Clone)]
pub struct TickLoop {
    config: ServerConfig,
    tick: u64,
}

impl TickLoop {
    /// Create from a [`ServerConfig`].
    pub fn from_config(cfg: &ServerConfig) -> Self {
        Self {
            config: cfg.clone(),
            tick: 0,
        }
    }

    /// Advance by one tick; returns the new tick counter.
    pub fn advance(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// Returns `true` when a snapshot should be broadcast this tick.
    pub fn should_snapshot(&self) -> bool {
        let interval = self.config.snapshot_interval;
        interval > 0 && self.tick > 0 && self.tick % u64::from(interval) == 0
    }

    /// The current tick counter.
    #[inline]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The configured tick duration.
    #[inline]
    pub fn tick_duration(&self) -> Duration {
        self.config.tick_duration()
    }
}

// ---------------------------------------------------------------------------
// Client-side prediction buffer
// ---------------------------------------------------------------------------

/// A ring buffer of unacknowledged input frames for client-side prediction.
///
/// The client pushes frames with [`PredictionBuffer::push`] as it sends them
/// to the server, then calls [`PredictionBuffer::acknowledge`] when the server
/// confirms a sequence number. Frames older than the acknowledged sequence can
/// be discarded; the remaining frames must be re-simulated on top of the
/// server's authoritative snapshot.
///
/// # Example
///
/// ```rust
/// use magnetite_sdk::input::Input;
/// use magnetite_sdk::networking::PredictionBuffer;
///
/// let mut buf = PredictionBuffer::new(128);
/// buf.push(Input { sequence: 1, ..Default::default() });
/// buf.push(Input { sequence: 2, ..Default::default() });
/// buf.acknowledge(1);
/// let pending = buf.pending();
/// assert_eq!(pending.len(), 1);
/// assert_eq!(pending[0].sequence, 2);
/// ```
pub struct PredictionBuffer {
    capacity: usize,
    frames: Vec<Input>,
}

impl PredictionBuffer {
    /// Create a buffer with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            frames: Vec::with_capacity(capacity),
        }
    }

    /// Append an input frame to the buffer.
    ///
    /// If the buffer is full the oldest frame is dropped (overrun).
    pub fn push(&mut self, frame: Input) {
        if self.frames.len() >= self.capacity {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }

    /// Discard all frames whose `sequence` is ≤ `acked_seq`.
    pub fn acknowledge(&mut self, acked_seq: u64) {
        self.frames.retain(|f| f.sequence > acked_seq);
    }

    /// Returns a slice of frames that have not yet been acknowledged.
    pub fn pending(&self) -> &[Input] {
        &self.frames
    }

    /// How many unacknowledged frames are buffered.
    #[inline]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns `true` when no unacknowledged frames remain.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Interest management trait (AAA scale)
// ---------------------------------------------------------------------------

/// A hook for server-side interest management (relevance / area-of-interest).
///
/// In large games it is wasteful — and a security risk — to broadcast the full
/// [`GameState`] to every client every tick. Instead, the server filters the
/// state through an `InterestManager` that returns only the subset each player
/// should see.
///
/// # Implementation guidance
///
/// - For a small game, use [`FullInterest`] which returns the full state to
///   everyone.
/// - For a battle-royale or MMO, implement radius-based or zone-based
///   filtering.
/// - The returned [`GameState`] must be valid and safe to send to the client;
///   in particular it must not contain other players' private data (e.g.
///   inventory not yet revealed).
///
/// ```rust
/// use magnetite_sdk::networking::{FullInterest, InterestManager};
/// use magnetite_sdk::state::{GameState, PlayerId};
///
/// let mgr = FullInterest;
/// let state = GameState::default();
/// let visible = mgr.visible_state(PlayerId::new(1), &state);
/// assert_eq!(visible.tick, state.tick);
/// ```
pub trait InterestManager {
    /// Return the portion of `full_state` that `player_id` should receive.
    fn visible_state(&self, player_id: PlayerId, full_state: &GameState) -> GameState;
}

/// [`InterestManager`] that sends the full state to every player.
///
/// Suitable for small/medium games where all state is safe to broadcast.
pub struct FullInterest;

impl InterestManager for FullInterest {
    fn visible_state(&self, _player_id: PlayerId, full_state: &GameState) -> GameState {
        full_state.clone()
    }
}

/// [`InterestManager`] that culls players outside a given radius.
///
/// Only the observing player and players within `radius` metres of them are
/// included in the returned state.
///
/// ```rust
/// use magnetite_sdk::networking::{InterestManager, RadiusInterest};
/// use magnetite_sdk::state::{GameState, PlayerId, PlayerState, Position, Rotation};
///
/// let mgr = RadiusInterest::new(100.0);
/// let mut state = GameState::default();
/// state.players.push(PlayerState {
///     id: PlayerId::new(1),
///     position: Position { x: 0.0, y: 0.0, z: 0.0 },
///     rotation: Rotation::default(),
///     health: 100.0, max_health: 100.0, alive: true, score: 0,
///     custom: serde_json::Value::Null,
/// });
/// state.players.push(PlayerState {
///     id: PlayerId::new(2),
///     position: Position { x: 500.0, y: 0.0, z: 0.0 },
///     rotation: Rotation::default(),
///     health: 100.0, max_health: 100.0, alive: true, score: 0,
///     custom: serde_json::Value::Null,
/// });
/// let visible = mgr.visible_state(PlayerId::new(1), &state);
/// // Player 2 is 500m away — outside the 100m radius, so not visible.
/// assert_eq!(visible.players.len(), 1);
/// ```
pub struct RadiusInterest {
    radius: f32,
}

impl RadiusInterest {
    /// Create a new radius-based interest manager.
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }
}

impl InterestManager for RadiusInterest {
    fn visible_state(&self, player_id: PlayerId, full_state: &GameState) -> GameState {
        let observer = match full_state.player(player_id) {
            Some(p) => p.position,
            None => {
                // Observer not found — return an empty state.
                return GameState {
                    tick: full_state.tick,
                    players: vec![],
                    world: full_state.world.clone(),
                };
            }
        };

        let visible_players = full_state
            .players
            .iter()
            .filter(|p| p.id == player_id || observer.distance_to(p.position) <= self.radius)
            .cloned()
            .collect();

        GameState {
            tick: full_state.tick,
            players: visible_players,
            world: full_state.world.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Legacy connection helper (kept for backwards compatibility)
// ---------------------------------------------------------------------------

/// A TCP connection from a game client to the server.
///
/// For new projects prefer using the typed [`Codec`] API directly. This type
/// is kept for backward compatibility.
pub struct Connection {
    inner: FramedTransport<TcpStream>,
    player_id: Option<PlayerId>,
}

impl Connection {
    /// Create from a connected [`TcpStream`].
    pub fn from_stream(stream: TcpStream) -> Self {
        Self {
            inner: FramedTransport::new(stream),
            player_id: None,
        }
    }

    /// Send a raw [`ServerMessage`] (serialised as JSON).
    pub fn send(&mut self, msg: &ServerMessage) -> std::io::Result<()> {
        let env = Envelope::new(msg.clone());
        let bytes = env
            .encode()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        self.inner.send_frame(&bytes)
    }

    /// Receive a raw [`ClientMessage`] (deserialised from JSON).
    pub fn receive(&mut self) -> std::io::Result<ClientMessage> {
        let bytes = self.inner.recv_frame()?;
        let env: Envelope<ClientMessage> = Envelope::decode(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(env.body)
    }

    /// Set the player id once the handshake is complete.
    pub fn set_player_id(&mut self, player_id: PlayerId) {
        self.player_id = Some(player_id);
    }

    /// Return the player id if the handshake is complete.
    pub fn player_id(&self) -> Option<PlayerId> {
        self.player_id
    }

    /// Send an input frame (convenience wrapper).
    pub fn send_input(&mut self, input: Input) -> std::io::Result<()> {
        let msg = ServerMessage::StateUpdate {
            tick: input.sequence,
            state: GameState::default(),
        };
        self.send(&msg)
    }
}

/// A server-side connection manager that tracks all connected players.
pub struct ServerNetworkManager {
    players: HashMap<PlayerId, FramedTransport<TcpStream>>,
    config: ServerConfig,
}

impl ServerNetworkManager {
    /// Create with default configuration.
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            config: ServerConfig::default(),
        }
    }

    /// Create with an explicit [`ServerConfig`].
    pub fn with_config(config: ServerConfig) -> Self {
        Self {
            players: HashMap::new(),
            config,
        }
    }

    /// Register a new player connection.
    pub fn add_player(&mut self, player_id: PlayerId, stream: TcpStream) {
        self.players.insert(player_id, FramedTransport::new(stream));
    }

    /// Deregister and return the underlying stream for a player.
    pub fn remove_player(&mut self, player_id: &PlayerId) -> Option<TcpStream> {
        self.players.remove(player_id).map(|t| t.into_inner())
    }

    /// Broadcast a [`ServerMessage`] to all connected players.
    pub fn broadcast(&mut self, msg: &ServerMessage) -> std::io::Result<()> {
        let env = Envelope::new(msg.clone());
        let bytes = env
            .encode()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        for transport in self.players.values_mut() {
            transport.send_frame(&bytes)?;
        }
        Ok(())
    }

    /// Send a [`ServerMessage`] to a single player.
    pub fn send_to(&mut self, player_id: &PlayerId, msg: &ServerMessage) -> std::io::Result<()> {
        if let Some(transport) = self.players.get_mut(player_id) {
            let env = Envelope::new(msg.clone());
            let bytes = env
                .encode()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            transport.send_frame(&bytes)?;
        }
        Ok(())
    }

    /// Number of currently connected players.
    #[inline]
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Returns `true` when the session is at capacity.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.players.len() >= self.config.max_players
    }

    /// The server configuration.
    #[inline]
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}

impl Default for ServerNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Legacy typed message alias (kept for existing users)
// ---------------------------------------------------------------------------

/// Wire messages — re-exported under the legacy name for backwards compat.
///
/// Prefer using [`ClientMessage`] / [`ServerMessage`] in new code.
pub use crate::protocol::{ClientMessage as Message, ServerMessage as StateMessage};

/// Legacy state-sync packet.
///
/// Kept for the old `StateSyncProtocol` users; prefer [`Snapshot`] in new code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncProtocol {
    pub tick: u64,
    pub state: GameState,
    pub checksum: u32,
}

impl StateSyncProtocol {
    pub fn new(tick: u64, state: GameState) -> Self {
        let snap = Snapshot::new(tick, state.clone());
        Self {
            tick,
            state,
            checksum: snap.checksum,
        }
    }

    pub fn is_valid(&self) -> bool {
        let snap = Snapshot::new(self.tick, self.state.clone());
        snap.checksum == self.checksum
    }
}

// Re-export NetworkManager as a convenience.
/// Legacy name for [`ServerNetworkManager`]. Prefer the new name.
pub use ServerNetworkManager as NetworkManager;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;

    #[test]
    fn server_config_builder() {
        let cfg = ServerConfig::builder()
            .tick_rate(128)
            .snapshot_interval(64)
            .max_players(100)
            .timeout_ticks(60)
            .build();
        assert_eq!(cfg.tick_rate, 128);
        assert_eq!(cfg.snapshot_interval, 64);
        assert_eq!(cfg.max_players, 100);
    }

    #[test]
    fn tick_duration_at_60hz() {
        let cfg = ServerConfig::default();
        // 1 second / 60 ticks = ~16.666 ms per tick.
        let d = cfg.tick_duration();
        let expected = Duration::from_secs(1) / 60;
        assert_eq!(d, expected);
        // Sanity: must be less than 17 ms and more than 16 ms.
        assert!(d < Duration::from_millis(17));
        assert!(d > Duration::from_millis(16));
    }

    #[test]
    fn tick_loop_snapshot_cadence() {
        let cfg = ServerConfig::builder()
            .tick_rate(60)
            .snapshot_interval(60)
            .build();
        let mut tl = TickLoop::from_config(&cfg);

        for _ in 0..59 {
            tl.advance();
            assert!(!tl.should_snapshot());
        }
        tl.advance(); // tick 60
        assert!(tl.should_snapshot());
        tl.advance(); // tick 61
        assert!(!tl.should_snapshot());
    }

    #[test]
    fn prediction_buffer_push_and_ack() {
        let mut buf = PredictionBuffer::new(10);
        buf.push(Input {
            sequence: 1,
            ..Default::default()
        });
        buf.push(Input {
            sequence: 2,
            ..Default::default()
        });
        buf.push(Input {
            sequence: 3,
            ..Default::default()
        });
        assert_eq!(buf.len(), 3);

        buf.acknowledge(2);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.pending()[0].sequence, 3);
    }

    #[test]
    fn prediction_buffer_overrun_drops_oldest() {
        let mut buf = PredictionBuffer::new(2);
        buf.push(Input {
            sequence: 1,
            ..Default::default()
        });
        buf.push(Input {
            sequence: 2,
            ..Default::default()
        });
        buf.push(Input {
            sequence: 3,
            ..Default::default()
        }); // evicts seq=1
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.pending()[0].sequence, 2);
    }

    #[test]
    fn full_interest_returns_all() {
        use crate::state::{PlayerState, Position, Rotation};
        let mgr = FullInterest;
        let mut state = GameState::default();
        state.players.push(PlayerState {
            id: PlayerId::new(1),
            position: Position::default(),
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        });
        let visible = mgr.visible_state(PlayerId::new(1), &state);
        assert_eq!(visible.players.len(), 1);
    }

    #[test]
    fn radius_interest_culls_distant_players() {
        use crate::state::{PlayerState, Position, Rotation};
        let mgr = RadiusInterest::new(50.0);
        let mut state = GameState::default();
        // Observer at origin.
        state.players.push(PlayerState {
            id: PlayerId::new(1),
            position: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        });
        // Nearby player (30m).
        state.players.push(PlayerState {
            id: PlayerId::new(2),
            position: Position {
                x: 30.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        });
        // Distant player (200m).
        state.players.push(PlayerState {
            id: PlayerId::new(3),
            position: Position {
                x: 200.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: Rotation::default(),
            health: 100.0,
            max_health: 100.0,
            alive: true,
            score: 0,
            custom: serde_json::Value::Null,
        });

        let visible = mgr.visible_state(PlayerId::new(1), &state);
        // Only self (id=1) and nearby (id=2) should be visible.
        assert_eq!(visible.players.len(), 2);
        assert!(visible.players.iter().any(|p| p.id == PlayerId::new(1)));
        assert!(visible.players.iter().any(|p| p.id == PlayerId::new(2)));
        assert!(!visible.players.iter().any(|p| p.id == PlayerId::new(3)));
    }

    #[test]
    fn state_sync_protocol_valid() {
        let state = GameState::default();
        let proto = StateSyncProtocol::new(5, state);
        assert!(proto.is_valid());
    }
}
