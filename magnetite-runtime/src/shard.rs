//! Shard manager — single-process multi-shard topology.
//!
//! ## N2 — `Topology::Sharded` (single-node, multi-shard)
//!
//! When `Topology::Sharded { cell_size, .. }` is active the world is divided into
//! a 2-D grid of square *cells*.  Each cell maps to one [`ShardId`].  The
//! runtime runs **all** shards in the same process; there is no inter-process
//! communication.
//!
//! ### Spatial assignment
//!
//! A player's (x, y) position is derived from their [`Input`] mouse-delta
//! accumulator on the server (a simplified proxy until the game exposes an
//! explicit position signal).  The cell coordinates are:
//!
//! ```text
//! cell_x = floor(pos_x / cell_size)  (clamped to [0, 65535])
//! cell_y = floor(pos_y / cell_size)
//! ShardId = cell_y * 256 + cell_x   (compact but unlimited in principle)
//! ```
//!
//! ### HANDOFF
//!
//! When [`ShardManager::update_position`] detects that a player has crossed a
//! cell boundary it:
//!
//! 1. Records the old shard.
//! 2. Computes the new cell / shard.
//! 3. Updates the routing table.
//! 4. Returns a [`HandoffEvent`] so the tick loop can serialise the player's
//!    state, re-subscribe them in the new shard's executor, and publish a fresh
//!    `ServerNet::Snapshot` to the client.
//!
//! The player's WS connection remains on the same Tokio task; only the logical
//! shard routing changes.  This is intentionally single-process for N2.
//!
//! ### Multi-node seam
//!
//! The `handoff` method accepts a `&[u8]` player-state blob (serialised by the
//! caller via `GameExecutor::snapshot`).  In a future multi-node deployment:
//! - Replace the in-process routing table with a distributed coordination layer
//!   (e.g. etcd).
//! - The `target_addr` field in [`HandoffEvent`] would become a remote WS URL.
//! - The caller forwards the player's WS connection to that URL.
//!
//! Everything above that seam is already in place: the state blob is always
//! serialised before handoff, and the connection manager decouples WS I/O from
//! shard identity.

use std::collections::HashMap;

use tracing::{debug, info};

use magnetite_sdk::authority::Topology;
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

/// Identifies a shard.
///
/// For `SingleRoom` and `Dedicated` there is always one shard: [`ShardId::LOCAL`].
/// For `Sharded`, the id encodes the (cell_x, cell_y) grid coordinates compactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShardId(pub u32);

impl ShardId {
    /// The single local shard used by `SingleRoom` and `Dedicated` topologies.
    pub const LOCAL: Self = ShardId(0);

    /// Build a shard id from spatial cell coordinates.
    pub fn from_cell(cell_x: u16, cell_y: u16) -> Self {
        ShardId((cell_y as u32) * 65536 + cell_x as u32)
    }

    /// Decode back to (cell_x, cell_y).
    pub fn to_cell(self) -> (u16, u16) {
        let x = (self.0 & 0xFFFF) as u16;
        let y = (self.0 >> 16) as u16;
        (x, y)
    }
}

impl std::fmt::Display for ShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shard({})", self.0)
    }
}

// ---------------------------------------------------------------------------
// Per-player spatial state (for Sharded topology)
// ---------------------------------------------------------------------------

/// Server-side proxy position accumulated from input mouse deltas.
///
/// This is a *coarse* position estimate used purely for shard assignment.
/// The authoritative game state inside the executor owns the canonical position.
/// Uses `f64` to match `MouseState::delta_x/delta_y`.
#[derive(Debug, Clone, Default)]
struct PlayerPos {
    x: f64,
    y: f64,
}

// ---------------------------------------------------------------------------
// HandoffEvent
// ---------------------------------------------------------------------------

/// Emitted by [`ShardManager::update_position`] when a player crosses a cell
/// boundary in a `Topology::Sharded` deployment.
///
/// The runtime should:
/// 1. Serialise the player's state via `GameExecutor::snapshot` (or a
///    player-scoped sub-snapshot when the game supports it).
/// 2. Pass the bytes to [`ShardManager::apply_handoff`] (a no-op in N2 since
///    we are single-process, but the seam is preserved for multi-node N3+).
/// 3. Send `ServerNet::Snapshot` to the player so their client re-bootstraps.
#[derive(Debug, Clone)]
pub struct HandoffEvent {
    /// The player being migrated.
    pub player: PlayerId,
    /// The shard the player is leaving.
    pub from_shard: ShardId,
    /// The shard the player is joining.
    pub to_shard: ShardId,
    /// In a multi-node deployment this would be the target node's address.
    ///
    /// In N2 (single-process) this is always `None`; the caller just updates
    /// its in-process routing table.
    pub target_addr: Option<String>,
}

// ---------------------------------------------------------------------------
// ShardManager
// ---------------------------------------------------------------------------

/// Manages shard membership and spatial routing.
///
/// For `SingleRoom` / `Dedicated` this is a trivial passthrough (all players
/// in [`ShardId::LOCAL`]).  For `Sharded`, it tracks each player's proxy
/// position and emits [`HandoffEvent`]s on cell crossings.
pub struct ShardManager {
    topology: Topology,
    /// Maps each player to their current shard.
    player_shard: HashMap<PlayerId, ShardId>,
    /// Per-player proxy positions (only populated for `Sharded` topology).
    positions: HashMap<PlayerId, PlayerPos>,
}

impl ShardManager {
    /// Create a new shard manager for the given topology.
    pub fn new(topology: Topology) -> Self {
        Self {
            topology,
            player_shard: HashMap::new(),
            positions: HashMap::new(),
        }
    }

    /// Assign a newly-joined player to the appropriate shard.
    ///
    /// For `Sharded` topology, new players start in shard 0 (cell (0,0)) until
    /// their first position update arrives.
    pub fn assign(&mut self, player: PlayerId) -> ShardId {
        let shard = match &self.topology {
            Topology::SingleRoom | Topology::Dedicated { .. } => ShardId::LOCAL,
            Topology::Sharded { .. } => ShardId::LOCAL, // origin cell until first position
        };
        self.player_shard.insert(player, shard);
        if matches!(self.topology, Topology::Sharded { .. }) {
            self.positions.insert(player, PlayerPos::default());
        }
        debug!(%player, %shard, "player assigned to shard");
        shard
    }

    /// Return the shard a player is currently on, if known.
    pub fn shard_of(&self, player: PlayerId) -> Option<ShardId> {
        self.player_shard.get(&player).copied()
    }

    /// Remove a player from the routing table on disconnect.
    pub fn remove(&mut self, player: PlayerId) {
        self.player_shard.remove(&player);
        self.positions.remove(&player);
    }

    /// Update a player's proxy position from an input frame and return a
    /// [`HandoffEvent`] if they crossed into a new cell.
    ///
    /// This is a no-op for `SingleRoom` / `Dedicated` topologies and returns
    /// `None`.
    ///
    /// For `Sharded` topology:
    /// - Accumulates `input.mouse.delta_x` / `delta_y` into the proxy position.
    /// - Computes the new cell.
    /// - If the cell changed, updates the routing table and returns a
    ///   [`HandoffEvent`].
    pub fn update_position(&mut self, player: PlayerId, input: &Input) -> Option<HandoffEvent> {
        let cell_size = match &self.topology {
            Topology::Sharded { cell_size, .. } => *cell_size as f64,
            _ => return None,
        };

        let pos = self.positions.get_mut(&player)?;
        let old_shard = *self.player_shard.get(&player)?;

        // Accumulate mouse deltas as a proxy for position.
        pos.x += input.mouse.delta_x;
        pos.y += input.mouse.delta_y;

        let new_shard = pos_to_shard(pos.x, pos.y, cell_size);
        if new_shard == old_shard {
            return None;
        }

        // Cell crossing detected → handoff.
        self.player_shard.insert(player, new_shard);
        let event = HandoffEvent {
            player,
            from_shard: old_shard,
            to_shard: new_shard,
            target_addr: None, // multi-node seam: fill with remote URL in N3+
        };
        info!(
            %player,
            from = %old_shard,
            to   = %new_shard,
            "shard handoff (single-process N2)"
        );
        Some(event)
    }

    /// Apply a completed handoff.
    ///
    /// In N2 (single-process) the routing table has already been updated by
    /// [`update_position`], so this is a no-op that accepts the state blob for
    /// future use.  In a multi-node deployment, this would forward the blob to
    /// the target shard's runtime over the network.
    ///
    /// # Multi-node seam
    ///
    /// Replace the body of this function with a call to the target shard's
    /// provisioning API (e.g. POST `/restore` with the player state blob).
    /// The `target_addr` in the [`HandoffEvent`] provides the remote URL.
    #[allow(unused_variables)]
    pub fn apply_handoff(&self, event: &HandoffEvent, player_state_blob: &[u8]) {
        // N2: single-process — routing table already updated; nothing to do.
        // N3+ multi-node: send `player_state_blob` to `event.target_addr`.
        debug!(
            player = %event.player,
            to = %event.to_shard,
            blob_len = player_state_blob.len(),
            "handoff state transfer (N2: no-op — single process)"
        );
    }

    /// Explicit shard handoff (used when the caller controls the target shard
    /// directly, e.g. forced migration or rebalancing).
    ///
    /// For `SingleRoom` / `Dedicated`: returns the current shard unchanged.
    /// For `Sharded`: updates the routing table to `target` and returns it.
    pub fn handoff(&mut self, player: PlayerId, target: ShardId) -> ShardId {
        match &self.topology {
            Topology::Sharded { .. } => {
                self.player_shard.insert(player, target);
                debug!(%player, %target, "explicit shard handoff");
                target
            }
            _ => self.shard_of(player).unwrap_or(ShardId::LOCAL),
        }
    }

    /// Number of players currently tracked.
    pub fn len(&self) -> usize {
        self.player_shard.len()
    }

    /// Returns `true` when no players are tracked.
    pub fn is_empty(&self) -> bool {
        self.player_shard.is_empty()
    }

    /// Return a sorted list of all shards that have at least one player.
    pub fn active_shards(&self) -> Vec<ShardId> {
        let mut shards: Vec<ShardId> = self
            .player_shard
            .values()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        shards.sort();
        shards
    }

    /// Return all players assigned to a given shard.
    pub fn players_in_shard(&self, shard: ShardId) -> Vec<PlayerId> {
        self.player_shard
            .iter()
            .filter_map(|(pid, &sid)| if sid == shard { Some(*pid) } else { None })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a (x, y) position to the enclosing cell's [`ShardId`].
fn pos_to_shard(x: f64, y: f64, cell_size: f64) -> ShardId {
    let cell_x = if cell_size > 0.0 {
        ((x / cell_size).floor() as i64).clamp(0, 0xFFFF) as u16
    } else {
        0
    };
    let cell_y = if cell_size > 0.0 {
        ((y / cell_size).floor() as i64).clamp(0, 0xFFFF) as u16
    } else {
        0
    };
    ShardId::from_cell(cell_x, cell_y)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::authority::Topology;
    use magnetite_sdk::input::{Input, MouseState};

    fn sharded() -> ShardManager {
        ShardManager::new(Topology::Sharded {
            tick_hz: 20,
            cell_size: 100.0,
            max_per_shard: 64,
        })
    }

    #[test]
    fn single_room_assigns_local() {
        let mut mgr = ShardManager::new(Topology::SingleRoom);
        let player = PlayerId::new(1);
        let shard = mgr.assign(player);
        assert_eq!(shard, ShardId::LOCAL);
        assert_eq!(mgr.shard_of(player), Some(ShardId::LOCAL));
    }

    #[test]
    fn dedicated_assigns_local() {
        let mut mgr = ShardManager::new(Topology::Dedicated { tick_hz: 60 });
        let player = PlayerId::new(2);
        let shard = mgr.assign(player);
        assert_eq!(shard, ShardId::LOCAL);
    }

    #[test]
    fn sharded_assigns_origin_cell() {
        let mut mgr = sharded();
        let player = PlayerId::new(3);
        let shard = mgr.assign(player);
        assert_eq!(shard, ShardId::LOCAL); // origin cell (0,0) = ShardId(0)
    }

    #[test]
    fn remove_cleans_up() {
        let mut mgr = ShardManager::new(Topology::SingleRoom);
        let player = PlayerId::new(4);
        mgr.assign(player);
        assert_eq!(mgr.len(), 1);
        mgr.remove(player);
        assert!(mgr.is_empty());
    }

    #[test]
    fn handoff_noop_for_single_room() {
        let mut mgr = ShardManager::new(Topology::SingleRoom);
        let player = PlayerId::new(5);
        mgr.assign(player);
        let result = mgr.handoff(player, ShardId(1));
        assert_eq!(result, ShardId::LOCAL);
    }

    #[test]
    fn handoff_updates_table_for_sharded() {
        let mut mgr = sharded();
        let player = PlayerId::new(6);
        mgr.assign(player);
        let new_shard = mgr.handoff(player, ShardId(1));
        assert_eq!(new_shard, ShardId(1));
        assert_eq!(mgr.shard_of(player), Some(ShardId(1)));
    }

    #[test]
    fn active_shards_deduplicated() {
        let mut mgr = ShardManager::new(Topology::SingleRoom);
        for i in 0..5 {
            let player = PlayerId::new(10 + i);
            mgr.assign(player);
        }
        let shards = mgr.active_shards();
        assert_eq!(shards.len(), 1);
        assert_eq!(shards[0], ShardId::LOCAL);
    }

    #[test]
    fn shard_id_cell_roundtrip() {
        let sid = ShardId::from_cell(7, 3);
        assert_eq!(sid.to_cell(), (7, 3));
    }

    #[test]
    fn pos_to_shard_origin() {
        let sid = pos_to_shard(0.0, 0.0, 100.0);
        assert_eq!(sid, ShardId::from_cell(0, 0));
    }

    #[test]
    fn pos_to_shard_different_cells() {
        let s1 = pos_to_shard(50.0, 50.0, 100.0); // cell (0,0)
        let s2 = pos_to_shard(150.0, 50.0, 100.0); // cell (1,0)
        let s3 = pos_to_shard(50.0, 150.0, 100.0); // cell (0,1)
        assert_ne!(s1, s2);
        assert_ne!(s1, s3);
        assert_ne!(s2, s3);
    }

    #[test]
    fn no_handoff_within_same_cell() {
        let mut mgr = sharded();
        let player = PlayerId::new(20);
        mgr.assign(player);

        // Small movement — stays in cell (0,0).
        let input = Input {
            mouse: MouseState {
                delta_x: 10.0,
                delta_y: 10.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let event = mgr.update_position(player, &input);
        assert!(event.is_none(), "no handoff expected within same cell");
    }

    #[test]
    fn handoff_emitted_on_cell_crossing() {
        let mut mgr = sharded();
        let player = PlayerId::new(21);
        mgr.assign(player);

        // Large movement — crosses from cell (0,0) to cell (1,0).
        let input = Input {
            mouse: MouseState {
                delta_x: 150.0,
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let event = mgr.update_position(player, &input);
        assert!(event.is_some(), "handoff expected on cell crossing");
        let ev = event.unwrap();
        assert_eq!(ev.from_shard, ShardId::LOCAL);
        assert_eq!(ev.to_shard, ShardId::from_cell(1, 0));
        assert!(
            ev.target_addr.is_none(),
            "single-process: no remote address"
        );

        // Routing table updated.
        assert_eq!(mgr.shard_of(player), Some(ShardId::from_cell(1, 0)));
    }

    #[test]
    fn multiple_crossings_track_correctly() {
        let mut mgr = sharded();
        let player = PlayerId::new(22);
        mgr.assign(player);

        // Cross x=100 boundary.
        let i1 = Input {
            mouse: MouseState {
                delta_x: 110.0,
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let ev1 = mgr.update_position(player, &i1);
        assert!(ev1.is_some());

        // Cross y=100 boundary from current position.
        let i2 = Input {
            mouse: MouseState {
                delta_x: 0.0,
                delta_y: 110.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let ev2 = mgr.update_position(player, &i2);
        assert!(ev2.is_some());
        assert_eq!(mgr.shard_of(player), Some(ShardId::from_cell(1, 1)));
    }

    #[test]
    fn update_position_noop_for_single_room() {
        let mut mgr = ShardManager::new(Topology::SingleRoom);
        let player = PlayerId::new(30);
        mgr.assign(player);
        let input = Input {
            mouse: MouseState {
                delta_x: 9999.0,
                delta_y: 9999.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let ev = mgr.update_position(player, &input);
        assert!(ev.is_none());
    }

    #[test]
    fn players_in_shard_filters_correctly() {
        let mut mgr = sharded();
        let p1 = PlayerId::new(40);
        let p2 = PlayerId::new(41);
        mgr.assign(p1);
        mgr.assign(p2);

        // Force p2 into a different shard.
        mgr.handoff(p2, ShardId(1));

        let in_local = mgr.players_in_shard(ShardId::LOCAL);
        let in_1 = mgr.players_in_shard(ShardId(1));

        assert!(in_local.contains(&p1));
        assert!(!in_local.contains(&p2));
        assert!(in_1.contains(&p2));
    }

    #[test]
    fn apply_handoff_is_stable_noop() {
        let mgr = sharded();
        let ev = HandoffEvent {
            player: PlayerId::new(99),
            from_shard: ShardId::LOCAL,
            to_shard: ShardId(1),
            target_addr: None,
        };
        // Should not panic.
        mgr.apply_handoff(&ev, b"some_state_blob");
    }
}
