//! Shard manager seam.
//!
//! For [`Topology::SingleRoom`] and [`Topology::Dedicated`] there is always
//! exactly one shard — the local process.  This module provides the
//! [`ShardManager`] abstraction so that `magnetite-runtime` can be extended in
//! N2 to a multi-shard, spatially-partitioned topology without changing the
//! tick loop or connection manager APIs.
//!
//! ## N1 (this implementation)
//!
//! A single-node, single-shard implementation.  All players live in `shard 0`.
//! The [`ShardManager`] tracks which shard each player belongs to and provides
//! a hook point for future cross-shard handoff logic.
//!
//! ## N2 (future)
//!
//! When [`Topology::Sharded`] is selected and a player crosses a cell boundary,
//! `handoff` should:
//! 1. Serialize the player's local state.
//! 2. Connect to the target shard's runtime instance.
//! 3. Transfer the serialized state and the player's WS connection.
//! 4. Update the routing table here.

use std::collections::HashMap;

use tracing::debug;

use magnetite_sdk::authority::Topology;
use magnetite_sdk::state::PlayerId;

/// Identifies a shard.  In N1 there is always one shard: `ShardId(0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShardId(pub u32);

impl ShardId {
    /// The single local shard used in N1 (SingleRoom + Dedicated topologies).
    pub const LOCAL: Self = ShardId(0);
}

impl std::fmt::Display for ShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shard({})", self.0)
    }
}

/// Manages shard membership and provides a handoff seam for N2.
///
/// In N1, all players are assigned to [`ShardId::LOCAL`] and no actual
/// migration happens.  The interface is designed so that N2 can replace the
/// internals without changing callers.
pub struct ShardManager {
    topology: Topology,
    /// Maps each player to the shard they currently reside in.
    player_shard: HashMap<PlayerId, ShardId>,
}

impl ShardManager {
    /// Create a new shard manager for the given topology.
    pub fn new(topology: Topology) -> Self {
        Self {
            topology,
            player_shard: HashMap::new(),
        }
    }

    /// Assign a newly-joined player to the appropriate shard.
    ///
    /// In N1, this always assigns [`ShardId::LOCAL`].
    pub fn assign(&mut self, player: PlayerId) -> ShardId {
        let shard = match &self.topology {
            Topology::SingleRoom | Topology::Dedicated { .. } => ShardId::LOCAL,
            Topology::Sharded { .. } => {
                // N2: choose shard based on player's initial position / load.
                // N1: always local.
                ShardId::LOCAL
            }
        };
        self.player_shard.insert(player, shard);
        debug!(%player, %shard, "player assigned to shard");
        shard
    }

    /// Return the shard a player is currently on, if known.
    pub fn shard_of(&self, player: PlayerId) -> Option<ShardId> {
        self.player_shard.get(&player).copied()
    }

    /// Remove a player from the routing table (on disconnect).
    pub fn remove(&mut self, player: PlayerId) {
        self.player_shard.remove(&player);
    }

    /// Attempt to hand off a player to a different shard.
    ///
    /// In N1 this is a no-op (returns the same shard).  In N2, this should
    /// serialise the player's state, migrate the WS connection, and update the
    /// routing table.
    ///
    /// Returns the shard the player ends up on after the handoff attempt.
    pub fn handoff(&mut self, player: PlayerId, target: ShardId) -> ShardId {
        match &self.topology {
            Topology::Sharded { .. } => {
                // N2: migrate player state to `target` shard.
                // N1: update table only (all shards are local no-ops).
                self.player_shard.insert(player, target);
                debug!(%player, %target, "shard handoff (N1: no-op migration)");
                target
            }
            _ => {
                // SingleRoom / Dedicated: handoff is meaningless.
                self.shard_of(player).unwrap_or(ShardId::LOCAL)
            }
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

    /// Return a list of all shards that have at least one player.
    pub fn active_shards(&self) -> Vec<ShardId> {
        let mut shards: Vec<ShardId> = self
            .player_shard
            .values()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        shards.sort_by_key(|s| s.0);
        shards
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::authority::Topology;

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
    fn sharded_assigns_local_in_n1() {
        let mut mgr = ShardManager::new(Topology::Sharded {
            tick_hz: 20,
            cell_size: 500.0,
            max_per_shard: 64,
        });
        let player = PlayerId::new(3);
        let shard = mgr.assign(player);
        assert_eq!(shard, ShardId::LOCAL);
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
        // Handoff on SingleRoom returns the same shard.
        let result = mgr.handoff(player, ShardId(1));
        assert_eq!(result, ShardId::LOCAL);
    }

    #[test]
    fn handoff_updates_table_for_sharded() {
        let mut mgr = ShardManager::new(Topology::Sharded {
            tick_hz: 20,
            cell_size: 500.0,
            max_per_shard: 64,
        });
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
}
