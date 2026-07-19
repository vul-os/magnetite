//! # `magnetite_sdk::scaling` — capacity-elastic scaling primitives
//!
//! *(Requires the `scaling` feature. Enabled by `magnetite-runtime`; off by
//! default so game crates compiled to `wasm32-wasip1` stay lean.)*
//!
//! This module implements the DECENTRALIZATION.md §4 property:
//!
//! > **A world is a set of shards. A node is generic compute that fills its own
//! > hardware. Player cap is emergent from capacity, never a config constant.**
//!
//! ## The three moving parts
//!
//! | Type | Role |
//! |---|---|
//! | [`Shardable`] | A world declares *how to partition its state into shards*. |
//! | [`ShardScheduler`] | Places a world's [`ShardKey`]s onto whatever node [`Capacity`] exists. |
//! | [`shards_for_capacity`] / [`player_capacity`] | Derive the shard/player budget from hardware. |
//!
//! The scheduler and the capacity helpers program **only** against
//! [`magnetite_seams::discovery::Capacity`] — no provider-specific type ever
//! leaks in (guardrail §6). The default [`LocalScheduler`] fills a single box's
//! cores; [`SpreadScheduler`] bin-packs shards across many nodes — the working
//! interface for real multi-node "Bucket D" placement.
//!
//! ## Determinism is untouched
//!
//! Sharding is purely a *placement* concern. Each shard still runs the same
//! deterministic [`AuthoritativeGame`](crate::authority::AuthoritativeGame)
//! `validate`/`step`, so [`verify_replay`](crate::authority::verify_replay)
//! keeps working per-shard. Nothing here weakens the anti-cheat moat.

use serde::{Deserialize, Serialize};

/// Re-export of the seam capacity descriptor a node self-measures and advertises.
///
/// Scheduling code programs against this and nothing provider-specific.
pub use magnetite_seams::discovery::Capacity;

use crate::authority::{MatchConfig, Topology};

// ---------------------------------------------------------------------------
// ShardKey
// ---------------------------------------------------------------------------

/// A transport-agnostic shard identifier at the SDK level.
///
/// The runtime maps this to its own routing `ShardId`; games never need to know
/// how a shard is physically hosted. A spatial world typically encodes cell
/// coordinates into the `u32`; a room-based world uses one key per room.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct ShardKey(pub u32);

impl ShardKey {
    /// Build a shard key from 2-D cell coordinates (spatial worlds).
    pub fn from_cell(cell_x: u16, cell_y: u16) -> Self {
        ShardKey((cell_y as u32) << 16 | cell_x as u32)
    }
    /// Decode back to `(cell_x, cell_y)`.
    pub fn to_cell(self) -> (u16, u16) {
        ((self.0 & 0xFFFF) as u16, (self.0 >> 16) as u16)
    }
}

impl std::fmt::Display for ShardKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shard#{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Shardable
// ---------------------------------------------------------------------------

/// A world declares *how to partition its state into shards*.
///
/// This is the only thing a game must provide for the runtime to scale it across
/// cores (single box) or nodes (a cluster). Everything above it — placement,
/// handoff, discovery — is generic by construction.
///
/// The runtime consults [`shard_of`](Shardable::shard_of) each tick to route an
/// entity's inputs to the right shard, and [`neighbors`](Shardable::neighbors)
/// to know which shards a boundary crossing may hand off to.
///
/// # Example — a spatial grid world
/// ```
/// # #[cfg(feature = "scaling")] {
/// use magnetite_sdk::scaling::{Shardable, ShardKey};
///
/// struct Grid { cell_size: f32, width_cells: u16, height_cells: u16 }
/// struct Entity { x: f32, y: f32 }
///
/// impl Shardable for Grid {
///     type Entity = Entity;
///     fn shard_of(&self, e: &Entity) -> ShardKey {
///         let cx = (e.x / self.cell_size) as u16;
///         let cy = (e.y / self.cell_size) as u16;
///         ShardKey::from_cell(cx, cy)
///     }
///     fn shards(&self) -> Vec<ShardKey> {
///         (0..self.height_cells).flat_map(|y| (0..self.width_cells)
///             .map(move |x| ShardKey::from_cell(x, y))).collect()
///     }
/// }
/// # }
/// ```
pub trait Shardable {
    /// The world entity whose position/identity decides its shard.
    type Entity;

    /// Which shard currently owns `entity`.
    fn shard_of(&self, entity: &Self::Entity) -> ShardKey;

    /// Enumerate all shard keys this world is currently partitioned into.
    fn shards(&self) -> Vec<ShardKey>;

    /// Shards adjacent to `shard` — the legal handoff targets when an entity
    /// crosses a boundary. Default: none (single-shard / non-spatial worlds).
    fn neighbors(&self, _shard: ShardKey) -> Vec<ShardKey> {
        Vec::new()
    }

    /// Hook invoked when `entity` crosses from shard `from` into shard `to`.
    /// A world can use this to update its own bookkeeping. Default: no-op.
    fn on_boundary(&mut self, _entity: &Self::Entity, _from: ShardKey, _to: ShardKey) {}
}

// ---------------------------------------------------------------------------
// Emergent capacity — the anti-"constant" helpers (§4)
// ---------------------------------------------------------------------------

/// Default players a single shard is sized for. This is a *shard* unit, not a
/// world cap — the world cap is `shards × this`, and the shard count itself is
/// emergent from hardware, so the world cap is never a fixed constant.
pub const DEFAULT_PLAYERS_PER_SHARD: u32 = 64;

/// Minimum RAM budgeted per shard. Used to derive a RAM-bound shard ceiling so a
/// many-core / low-RAM box does not over-shard itself.
pub const MIN_RAM_MB_PER_SHARD: u64 = 256;

/// How many shards a box of this [`Capacity`] should host — **emergent** from
/// cores *and* RAM, never a hardcoded constant.
///
/// The count is the minimum of the core budget (one shard per logical core) and
/// the RAM budget (`ram_mb / MIN_RAM_MB_PER_SHARD`), floored at 1.
pub fn shards_for_capacity(cap: &Capacity) -> u32 {
    let by_cores = cap.cpu_cores.max(1);
    let by_ram = (cap.ram_mb / MIN_RAM_MB_PER_SHARD).max(1);
    let by_ram = u32::try_from(by_ram).unwrap_or(u32::MAX);
    by_cores.min(by_ram).max(1)
}

/// Total **emergent** player capacity of a box: `shards × players_per_shard`.
pub fn player_capacity(cap: &Capacity, players_per_shard: u32) -> u64 {
    shards_for_capacity(cap) as u64 * players_per_shard as u64
}

// ---------------------------------------------------------------------------
// Capacity-elastic MatchConfig
// ---------------------------------------------------------------------------

impl MatchConfig {
    /// Build a **capacity-elastic** match config. The topology, shard count, and
    /// `max_players` are all *derived from `cap`* — the emergent-capacity model
    /// of §4 — rather than passed as a constant.
    ///
    /// - Tiny box (1 shard, ≤16 emergent players) → [`Topology::SingleRoom`].
    /// - One-shard box → [`Topology::Dedicated`].
    /// - Multi-shard box → [`Topology::Sharded`] whose `max_per_shard` is the
    ///   per-shard unit (players still scale with the shard count).
    ///
    /// # Example
    /// ```
    /// # #[cfg(feature = "scaling")] {
    /// use magnetite_sdk::authority::{MatchConfig, Topology};
    /// use magnetite_sdk::scaling::Capacity;
    ///
    /// let big = Capacity { cpu_cores: 32, ram_mb: 65536, bandwidth_mbps: 1000,
    ///                      free_slots: 0, max_shards: 0 };
    /// let cfg = MatchConfig::elastic(42, &big, 500.0);
    /// assert!(matches!(cfg.topology, Topology::Sharded { .. }));
    /// // Two identical boxes → identical emergent cap (deterministic).
    /// assert_eq!(cfg.max_players, MatchConfig::elastic(42, &big, 500.0).max_players);
    /// # }
    /// ```
    pub fn elastic(seed: u64, cap: &Capacity, cell_size: f32) -> Self {
        let players_per_shard = DEFAULT_PLAYERS_PER_SHARD;
        let shards = shards_for_capacity(cap);
        let max_players = player_capacity(cap, players_per_shard).min(u32::MAX as u64) as u32;

        let topology = if shards <= 1 && max_players <= 16 {
            Topology::SingleRoom
        } else if shards <= 1 {
            Topology::Dedicated { tick_hz: 60 }
        } else {
            Topology::Sharded {
                tick_hz: 20,
                cell_size,
                max_per_shard: players_per_shard,
            }
        };
        let tick_hz = match &topology {
            Topology::SingleRoom => 60,
            Topology::Dedicated { tick_hz } => *tick_hz,
            Topology::Sharded { tick_hz, .. } => *tick_hz,
        };
        Self {
            topology,
            max_players,
            tick_hz,
            seed,
            snapshot_every: 300,
        }
    }
}

// ---------------------------------------------------------------------------
// ShardScheduler
// ---------------------------------------------------------------------------

/// A node's identity for placement purposes (opaque; e.g. its pubkey or addr).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A node offered to the scheduler: who it is + what it can hold.
#[derive(Clone, Debug)]
pub struct NodeCapacity {
    /// Node identity.
    pub node: NodeId,
    /// Self-measured hardware capacity.
    pub capacity: Capacity,
}

impl NodeCapacity {
    /// Convenience constructor.
    pub fn new(node: impl Into<String>, capacity: Capacity) -> Self {
        Self {
            node: NodeId(node.into()),
            capacity,
        }
    }
    /// The emergent shard ceiling for this node.
    pub fn shard_ceiling(&self) -> u32 {
        // Prefer an explicit advertised `max_shards`, else derive from hardware.
        if self.capacity.max_shards > 0 {
            self.capacity.max_shards
        } else {
            shards_for_capacity(&self.capacity)
        }
    }
}

/// The result of a scheduling pass.
#[derive(Clone, Debug, Default)]
pub struct Placement {
    /// `(shard, node)` assignments.
    pub assignments: Vec<(ShardKey, NodeId)>,
    /// Shards that could not be placed (no node had headroom).
    pub unplaced: Vec<ShardKey>,
}

impl Placement {
    /// All shards assigned to a given node.
    pub fn shards_on(&self, node: &NodeId) -> Vec<ShardKey> {
        self.assignments
            .iter()
            .filter(|(_, n)| n == node)
            .map(|(s, _)| *s)
            .collect()
    }
}

/// Places a world's shards onto whatever node capacity exists (§4).
///
/// Implementations must never name a provider-specific type — they see only
/// [`ShardKey`]s and [`NodeCapacity`] (which wraps the seam [`Capacity`]).
pub trait ShardScheduler {
    /// Assign each shard in `shards` to one of `nodes`.
    fn place(&self, shards: &[ShardKey], nodes: &[NodeCapacity]) -> Placement;
}

/// Default single-box scheduler: **fill this box's cores** — all shards land on
/// the first (local) node. This is the `magnetite dev` / single-node path.
///
/// If more shards are supplied than the box's [`shard_ceiling`](NodeCapacity::shard_ceiling),
/// they are still placed on the local node (a single process happily
/// over-subscribes cores); the ceiling is advisory here and load-bearing only
/// for [`SpreadScheduler`].
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalScheduler;

impl ShardScheduler for LocalScheduler {
    fn place(&self, shards: &[ShardKey], nodes: &[NodeCapacity]) -> Placement {
        match nodes.first() {
            Some(local) => Placement {
                assignments: shards.iter().map(|s| (*s, local.node.clone())).collect(),
                unplaced: Vec::new(),
            },
            None => Placement {
                assignments: Vec::new(),
                unplaced: shards.to_vec(),
            },
        }
    }
}

/// Multi-node scheduler: greedily **bin-packs** shards across nodes, always
/// placing the next shard on the least-loaded node that still has headroom
/// (`shard_ceiling`). This is the working interface for federated "Bucket D"
/// placement; the actual cross-node *transport* is the runtime's handoff seam.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpreadScheduler;

impl ShardScheduler for SpreadScheduler {
    fn place(&self, shards: &[ShardKey], nodes: &[NodeCapacity]) -> Placement {
        // Track remaining headroom per node (index-aligned with `nodes`).
        let mut load: Vec<u32> = vec![0; nodes.len()];
        let mut placement = Placement::default();

        for &shard in shards {
            // Pick the node with the most remaining headroom (ceiling - load).
            let choice = nodes
                .iter()
                .enumerate()
                .filter(|(i, n)| load[*i] < n.shard_ceiling())
                .max_by_key(|(i, n)| n.shard_ceiling().saturating_sub(load[*i]));

            match choice {
                Some((i, n)) => {
                    load[i] += 1;
                    placement.assignments.push((shard, n.node.clone()));
                }
                None => placement.unplaced.push(shard),
            }
        }
        placement
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(cores: u32, ram_mb: u64, max_shards: u32) -> Capacity {
        Capacity {
            cpu_cores: cores,
            ram_mb,
            bandwidth_mbps: 1000,
            free_slots: 0,
            max_shards,
        }
    }

    #[test]
    fn shard_key_cell_roundtrip() {
        let k = ShardKey::from_cell(7, 3);
        assert_eq!(k.to_cell(), (7, 3));
    }

    #[test]
    fn shards_emergent_from_cores_and_ram() {
        // Core-bound: 8 cores, plenty of RAM → 8 shards.
        assert_eq!(shards_for_capacity(&cap(8, 65536, 0)), 8);
        // RAM-bound: 32 cores but only 1 GB → 4 shards (1024/256).
        assert_eq!(shards_for_capacity(&cap(32, 1024, 0)), 4);
        // Floor at 1 even for a tiny box.
        assert_eq!(shards_for_capacity(&cap(0, 0, 0)), 1);
    }

    #[test]
    fn player_capacity_scales_with_hardware() {
        let small = player_capacity(&cap(2, 8192, 0), DEFAULT_PLAYERS_PER_SHARD);
        let big = player_capacity(&cap(32, 65536, 0), DEFAULT_PLAYERS_PER_SHARD);
        assert!(big > small, "more hardware ⇒ more emergent player capacity");
        assert_eq!(small, 2 * DEFAULT_PLAYERS_PER_SHARD as u64);
    }

    #[test]
    fn elastic_config_is_not_a_constant() {
        // A 1-shard box collapses to a non-sharded topology (Single/Dedicated).
        let tiny = MatchConfig::elastic(1, &cap(1, 512, 0), 500.0);
        assert!(matches!(
            tiny.topology,
            Topology::SingleRoom | Topology::Dedicated { .. }
        ));

        let big = MatchConfig::elastic(1, &cap(32, 65536, 0), 500.0);
        assert!(matches!(big.topology, Topology::Sharded { .. }));
        assert!(
            big.max_players > tiny.max_players,
            "player cap must grow with hardware"
        );
    }

    #[test]
    fn local_scheduler_fills_first_node() {
        let shards: Vec<ShardKey> = (0..5).map(ShardKey).collect();
        let nodes = vec![
            NodeCapacity::new("local", cap(8, 65536, 8)),
            NodeCapacity::new("remote", cap(8, 65536, 8)),
        ];
        let p = LocalScheduler.place(&shards, &nodes);
        assert_eq!(p.shards_on(&NodeId("local".into())).len(), 5);
        assert!(p.shards_on(&NodeId("remote".into())).is_empty());
        assert!(p.unplaced.is_empty());
    }

    #[test]
    fn local_scheduler_no_nodes_leaves_unplaced() {
        let shards: Vec<ShardKey> = (0..3).map(ShardKey).collect();
        let p = LocalScheduler.place(&shards, &[]);
        assert_eq!(p.unplaced.len(), 3);
        assert!(p.assignments.is_empty());
    }

    #[test]
    fn spread_scheduler_balances_across_nodes() {
        let shards: Vec<ShardKey> = (0..8).map(ShardKey).collect();
        let nodes = vec![
            NodeCapacity::new("a", cap(4, 65536, 4)),
            NodeCapacity::new("b", cap(4, 65536, 4)),
        ];
        let p = SpreadScheduler.place(&shards, &nodes);
        assert!(p.unplaced.is_empty());
        assert_eq!(p.shards_on(&NodeId("a".into())).len(), 4);
        assert_eq!(p.shards_on(&NodeId("b".into())).len(), 4);
    }

    #[test]
    fn spread_scheduler_respects_ceilings_and_reports_overflow() {
        let shards: Vec<ShardKey> = (0..10).map(ShardKey).collect();
        let nodes = vec![NodeCapacity::new("only", cap(2, 65536, 3))]; // ceiling 3
        let p = SpreadScheduler.place(&shards, &nodes);
        assert_eq!(p.shards_on(&NodeId("only".into())).len(), 3);
        assert_eq!(p.unplaced.len(), 7, "shards past capacity are reported unplaced");
    }

    /// A tiny spatial world exercising the Shardable contract.
    struct World {
        cell_size: f32,
    }
    struct Ent {
        x: f32,
        y: f32,
    }
    impl Shardable for World {
        type Entity = Ent;
        fn shard_of(&self, e: &Ent) -> ShardKey {
            ShardKey::from_cell(
                (e.x / self.cell_size) as u16,
                (e.y / self.cell_size) as u16,
            )
        }
        fn shards(&self) -> Vec<ShardKey> {
            vec![ShardKey::from_cell(0, 0), ShardKey::from_cell(1, 0)]
        }
        fn neighbors(&self, shard: ShardKey) -> Vec<ShardKey> {
            let (x, _y) = shard.to_cell();
            vec![ShardKey::from_cell(x.wrapping_add(1), 0)]
        }
    }

    #[test]
    fn shardable_routes_entities_by_position() {
        let w = World { cell_size: 100.0 };
        assert_eq!(w.shard_of(&Ent { x: 50.0, y: 10.0 }), ShardKey::from_cell(0, 0));
        assert_eq!(w.shard_of(&Ent { x: 150.0, y: 10.0 }), ShardKey::from_cell(1, 0));
        assert_eq!(w.shards().len(), 2);
        assert_eq!(w.neighbors(ShardKey::from_cell(0, 0)), vec![ShardKey::from_cell(1, 0)]);
    }
}
