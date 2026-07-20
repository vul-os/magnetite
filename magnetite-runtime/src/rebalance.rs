//! The reconciler that actually moves shards — and the reasons it refuses to.
//!
//! [`crate::fleet`] can move a shard safely. [`crate::cluster`] can say who is
//! allowed to receive one. Neither of them ever *decides* to move anything, so a
//! configured three-node cluster would sit there with every shard on whichever
//! node happened to create it. This module closes that gap: a periodic
//! reconciler that compares where shards **are** against where a
//! [`ShardScheduler`] says they **should be**, and walks the difference down
//! using the existing two-phase handoff.
//!
//! Nothing here is a new authority path. A rebalancer decision is only ever a
//! *proposal*; the move itself still goes through
//! [`NetworkHandoffTransport::migrate_shard`], which still checks membership,
//! still pins the target's key, still runs prepare/commit, and still leaves the
//! source owning the shard on **every** failure.
//!
//! # Why a rebalancer needs a brake before it needs a brain
//!
//! A reconciler that reacts to every measurement is worse than no reconciler:
//! shards ping-pong, every move costs a pause and a client reconnect, and the
//! cluster spends its capacity migrating instead of simulating. Four brakes,
//! all mandatory:
//!
//! | Brake | What it stops |
//! |---|---|
//! | **Deadband** ([`RebalancePolicy::deadband_shards`]) | Moving for an imbalance of one shard, which is inside the rounding error of any bin-pack. |
//! | **Cooldown** ([`RebalancePolicy::cooldown`]) | A shard that just moved being moved straight back by the next tick's slightly different view. |
//! | **Concurrency cap** ([`RebalancePolicy::max_in_flight`]) | Draining a node in one tick — a thundering herd of reconnects. |
//! | **Per-peer backoff** ([`RebalancePolicy::backoff_base`]) | Hammering a node that is down. Failures back off exponentially, and a backed-off peer is not even probed. |
//!
//! # Why it converges
//!
//! The desired placement is a **pure function of the shard set and the node
//! set** — it does not read current ownership. So moving a shard does not change
//! where anything is supposed to be. Each successful move strictly reduces
//! `Σ|actual_i − desired_i|`, and that sum is a non-negative integer, so the
//! process terminates. At the fixed point every node is within `deadband_shards`
//! of its desired count and [`Rebalancer::tick`] plans **zero** moves, tick after
//! tick, forever. `steady_state_issues_no_migrations` in this module's tests and
//! the multi-node socket test `convergence_then_zero_migrations` both assert
//! exactly that.
//!
//! One deliberate consequence: we balance **counts, not identities**. If this
//! node holds three shards and the scheduler wants it to hold three *different*
//! shards, that is a swap with no load benefit and we do nothing. Identity-based
//! diffing is the classic source of endless shuffling.
//!
//! # What happens when a node dies — read this before trusting it
//!
//! **Magnetite still has no live replication. A node that dies takes with it
//! everything it simulated since its last checkpoint, and that is gone.**
//!
//! What it now has is [`crate::checkpoint`]: shard state written to the
//! [`magnetite_seams::blobstore`] seam on a cadence. When the rebalancer detects
//! a death (the peer stops answering, or its discovery lease lapses, and shards
//! it last reported are now reported by nobody), it takes one of exactly two
//! outcomes per shard:
//!
//! - [`ShardRecovery`] — a survivor rebuilt the shard from the newest checkpoint
//!   it cached while the owner was alive, at a **strictly higher epoch** so the
//!   existing fence settles any race with a returning zombie. This is a
//!   **rollback**: up to one cadence of simulation is gone, and
//!   [`ShardRecovery::loss_window_secs`] reports how much.
//! - [`ShardLoss`] — the honest fallback, unchanged, taken whenever a restore
//!   cannot be made safe: no checkpoint known, blob missing, content hash or
//!   shard binding mismatch, epoch that would not out-rank the old owner, or an
//!   owner that is merely slow rather than dead ([`RecoveryPolicy`]).
//!
//! It still **never** silently starts an *empty* replacement shard: an empty
//! shard with the same id is a new world, not a recovered one, and quietly
//! producing one would look like a successful recovery while every player in it
//! lost their session. Callers that want a fresh shard must ask for one knowing
//! it is fresh.
//!
//! Surviving nodes do stop routing *new* work to a lapsed peer, because
//! [`RouteDirectory`] filters on the lease and the peer drops out of the node
//! set on the next tick. That is re-placement of capacity, not recovery of state.
//!
//! TODO(warm-standby): checkpointing bounds the loss to one cadence; it does not
//! remove it. Removing it needs a warm standby fed continuously (and the quorum
//! question that comes with it). Not attempted here.
//!
//! # Still not solved here
//!
//! NAT traversal. Peers must be directly reachable, exactly as in
//! [`crate::fleet`]. A rebalancer cannot route around an unroutable node; it
//! backs off from it and places work elsewhere.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use tracing::{info, warn};

use magnetite_seams::discovery::Capacity;
use magnetite_seams::identity::PubKey;

use magnetite_sdk::scaling::{NodeCapacity, NodeId, Placement, ShardKey};

/// Re-exported so a caller can build a [`Rebalancer`] without depending on the
/// SDK directly — the schedulers are part of this module's public interface.
pub use magnetite_sdk::scaling::{LocalScheduler, ShardScheduler, SpreadScheduler};

use crate::checkpoint::{
    restore_shard, CheckpointRef, CheckpointStore, RecoveryPolicy, RestoreError, ShardRecovery,
};
use crate::cluster::RouteDirectory;
use crate::fleet::{NetworkHandoffTransport, PeerRoute, PeerStatus};
use crate::shard::ShardId;

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// The four brakes. Defaults are tuned for a real cluster; tests shrink them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebalancePolicy {
    /// How many shards a node may be over its desired count before anything
    /// moves. `0` disables hysteresis (not recommended: a greedy bin-pack
    /// routinely differs by one shard for reasons that are pure tie-breaking).
    pub deadband_shards: u32,
    /// How long after a shard moves before it may move again. Absorbs the
    /// window where two nodes hold slightly different views of the cluster.
    pub cooldown: Duration,
    /// Most migrations started by one [`Rebalancer::tick`].
    pub max_in_flight: usize,
    /// First backoff after a peer fails. Doubles per consecutive failure.
    pub backoff_base: Duration,
    /// Ceiling on the backoff, so a peer that comes back is retried eventually.
    pub backoff_max: Duration,
    /// How often [`Rebalancer::tick`] is expected to be called. Advisory — used
    /// by the CLI to pace the loop, not enforced here.
    pub interval: Duration,
}

impl Default for RebalancePolicy {
    fn default() -> Self {
        Self {
            // One shard of imbalance is inside the noise of a greedy bin-pack.
            deadband_shards: 1,
            // Comfortably longer than one tick, so a shard cannot be re-judged
            // by the very next (possibly stale) measurement.
            cooldown: Duration::from_secs(120),
            // Two at a time: enough to drain a lopsided node in a few ticks,
            // few enough that reconnect storms stay small.
            max_in_flight: 2,
            backoff_base: Duration::from_secs(5),
            backoff_max: Duration::from_secs(300),
            interval: Duration::from_secs(30),
        }
    }
}

impl RebalancePolicy {
    /// The backoff after `failures` consecutive failures: `base * 2^(n-1)`,
    /// clamped to [`Self::backoff_max`]. Saturating, so a long-dead peer is
    /// retried every `backoff_max` rather than overflowing into never.
    pub fn backoff_for(&self, failures: u32) -> Duration {
        if failures == 0 {
            return Duration::ZERO;
        }
        let shift = (failures - 1).min(32);
        let mult = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
        let secs = self.backoff_base.as_secs().saturating_mul(mult);
        Duration::from_secs(secs).min(self.backoff_max)
    }
}

// ---------------------------------------------------------------------------
// The view a plan is computed from
// ---------------------------------------------------------------------------

/// Everything the planner is allowed to look at.
///
/// Built by [`Rebalancer::tick`] from live, membership-filtered routes and real
/// probes; constructible directly so the planner can be unit-tested without
/// sockets.
#[derive(Debug, Clone)]
pub struct ClusterView {
    /// This node's key.
    pub local: PubKey,
    /// This node's own capacity.
    pub local_capacity: Capacity,
    /// Shards this node is authoritative for right now.
    pub local_shards: Vec<ShardId>,
    /// Members that answered a probe this tick. **Only these are placement
    /// targets** — an unanswered peer is not a place to put work.
    pub peers: Vec<PeerStatus>,
    /// Members we know of but could not see this tick (no route, lease lapsed,
    /// backed off, or the probe failed). Placement-invisible; retained so
    /// [`Rebalancer`] can tell "gone" from "never existed".
    pub unreachable: Vec<PubKey>,
}

impl ClusterView {
    /// Nodes offered to the scheduler, sorted by key hex.
    ///
    /// Sorting matters: [`magnetite_sdk::scaling::SpreadScheduler`] is a greedy
    /// pass over this list, so a stable order is what makes the desired
    /// placement deterministic — and a deterministic desired placement is what
    /// makes the loop converge instead of oscillate.
    pub fn nodes(&self) -> Vec<NodeCapacity> {
        let mut v: Vec<NodeCapacity> = std::iter::once(NodeCapacity {
            node: NodeId(self.local.to_hex()),
            capacity: self.local_capacity.clone(),
        })
        .chain(self.peers.iter().map(|p| NodeCapacity {
            node: NodeId(p.node.to_hex()),
            capacity: p.capacity.clone(),
        }))
        .collect();
        v.sort_by(|a, b| a.node.0.cmp(&b.node.0));
        v.dedup_by(|a, b| a.node.0 == b.node.0);
        v
    }

    /// Every shard known to exist anywhere in the visible cluster, sorted and
    /// de-duplicated. A shard briefly claimed by two nodes (a view captured
    /// mid-migration) is counted once — double-counting it would invent load.
    pub fn all_shards(&self) -> Vec<ShardKey> {
        let mut v: Vec<u32> = self
            .local_shards
            .iter()
            .map(|s| s.0)
            .chain(self.peers.iter().flat_map(|p| p.shards.iter().map(|s| s.0)))
            .collect();
        v.sort_unstable();
        v.dedup();
        v.into_iter().map(ShardKey).collect()
    }
}

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

/// One migration the planner wants to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMove {
    /// Shard to hand over.
    pub shard: ShardId,
    /// Member key the scheduler chose. Still re-checked against membership by
    /// the transport before a byte moves.
    pub target: PubKey,
}

/// Why a shard that *looked* misplaced was left where it is. Every variant is a
/// deliberate refusal to move, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The whole node is within [`RebalancePolicy::deadband_shards`] of its
    /// desired share, so nothing on it is worth moving.
    WithinDeadband {
        /// Shards actually held.
        actual: usize,
        /// Shards the scheduler wants here.
        desired: usize,
    },
    /// This shard moved too recently.
    Cooldown {
        /// How much of the cooldown is left.
        remaining: Duration,
    },
    /// The chosen target is in exponential backoff after failing.
    PeerBackoff {
        /// The peer being spared.
        peer: String,
        /// How much longer it is spared for.
        remaining: Duration,
    },
    /// [`RebalancePolicy::max_in_flight`] was reached this tick.
    ConcurrencyCap,
    /// No live, membership-approved route to the chosen target.
    NoRoute(String),
}

/// A shard whose state is **gone**, because the node holding it disappeared.
///
/// This is what is reported when a restore from a checkpoint was **not
/// possible or not safe** — see [`Self::restore_refused`]. The shard's world —
/// entity positions, scores, whatever the game had in memory — died with the
/// node. A caller may choose to start a *fresh* shard with the same id; that is
/// a new world and should be described to players as one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardLoss {
    /// The shard whose state is unrecoverable.
    pub shard: ShardId,
    /// The node that was last seen holding it.
    pub last_owner: PubKey,
    /// Why we concluded it is gone.
    pub cause: LossCause,
    /// Why a checkpoint restore did not happen: `None` when recovery is switched
    /// off entirely, otherwise the [`RestoreError`] that made it unsafe. This is
    /// the field to read before believing any story about durability — a
    /// tampered or missing checkpoint lands here, not in a fake recovery.
    pub restore_refused: Option<String>,
}

/// How a shard came to be lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LossCause {
    /// The node's discovery lease lapsed, or it dropped out of membership.
    LeaseLapsed,
    /// The node has a route but stopped answering probes.
    Unreachable(String),
}

impl std::fmt::Display for ShardLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cause = match &self.cause {
            LossCause::LeaseLapsed => "its lease lapsed".to_string(),
            LossCause::Unreachable(e) => format!("it stopped answering: {e}"),
        };
        let why = match &self.restore_refused {
            Some(e) => format!(" NO CHECKPOINT RESTORE WAS POSSIBLE: {e}."),
            None => " Checkpoint recovery is not enabled on this node.".to_string(),
        };
        write!(
            f,
            "shard {} STATE LOST: node {} held it and {cause}.{why} This shard's \
             in-memory world is gone — a replacement shard would be a NEW world, \
             not a recovered one",
            self.shard,
            self.last_owner.to_hex()
        )
    }
}

/// What one planning pass decided.
#[derive(Debug, Clone, Default)]
pub struct MigrationPlan {
    /// Migrations to attempt, in shard-id order.
    pub moves: Vec<PlannedMove>,
    /// Shards considered and deliberately left alone.
    pub skipped: Vec<(ShardId, SkipReason)>,
    /// Shards the scheduler could place nowhere (no node had headroom).
    pub unplaced: Vec<ShardId>,
}

impl MigrationPlan {
    /// Whether this plan would move anything. The convergence assertion.
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
}

/// Compute a plan. **Pure**: no sockets, no clock of its own, no interior
/// mutation — every time-dependent input is passed in, so convergence and the
/// brakes are directly testable.
///
/// `cooldowns` maps shard id to the instant its cooldown expires.
/// `peer_backoff` maps a peer key hex to the instant its backoff expires.
pub fn plan_moves(
    view: &ClusterView,
    policy: &RebalancePolicy,
    scheduler: &dyn ShardScheduler,
    cooldowns: &HashMap<u32, Instant>,
    peer_backoff: &HashMap<String, Instant>,
    now: Instant,
) -> MigrationPlan {
    let mut plan = MigrationPlan::default();

    let nodes = view.nodes();
    let shards = view.all_shards();
    if nodes.len() < 2 || shards.is_empty() {
        // Nobody to move to, or nothing to move. Not an error — this is the
        // ordinary single-box case and the deny-by-default case.
        return plan;
    }

    let desired: Placement = scheduler.place(&shards, &nodes);
    let me = NodeId(view.local.to_hex());

    plan.unplaced = desired.unplaced.iter().map(|s| ShardId(s.0)).collect();

    // The scheduler is consulted for **how many** shards each node should hold,
    // not for which specific shard goes where. Two reasons:
    //
    // * A bin-pack is ownership-blind, so taking its per-shard assignment
    //   literally would reshuffle shards that are already on a correctly-sized
    //   node — churn with no load benefit.
    // * Load is a count. Swapping shard 3 for shard 7 between two nodes changes
    //   nothing anybody can measure, but it costs two migrations and a
    //   reconnect for every player in both.
    let desired_count: HashMap<&str, usize> = nodes
        .iter()
        .map(|n| (n.node.0.as_str(), desired.shards_on(&n.node).len()))
        .collect();

    let actual = view.local_shards.len();
    let want = desired_count.get(me.0.as_str()).copied().unwrap_or(0);

    // ---- Brake 1: deadband. Balance counts, not identities. ----
    let excess = actual.saturating_sub(want);
    if excess as u32 <= policy.deadband_shards {
        for s in &view.local_shards {
            plan.skipped.push((
                *s,
                SkipReason::WithinDeadband {
                    actual,
                    desired: want,
                },
            ));
        }
        return plan;
    }

    // Receivers: probed peers that are BELOW their desired count, most starved
    // first. Ties broken on key hex so every node in the cluster computes the
    // same order from the same facts — the other half of determinism.
    let mut receivers: Vec<(PubKey, String, usize)> = view
        .peers
        .iter()
        .filter_map(|p| {
            let hex = p.node.to_hex();
            let want = desired_count.get(hex.as_str()).copied().unwrap_or(0);
            let deficit = want.saturating_sub(p.shards.len());
            (deficit > 0).then_some((p.node, hex, deficit))
        })
        .collect();
    receivers.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));

    // Candidates: our own shards, lowest id first. Which one leaves does not
    // matter for load, so pick deterministically.
    let mut candidates: Vec<ShardId> = view.local_shards.clone();
    candidates.sort_by_key(|s| s.0);

    let mut moved = 0usize;
    let mut cursor = 0usize;
    for shard in candidates {
        // Shed only down to the desired count; the rest are already fine.
        if moved >= excess || cursor >= receivers.len() {
            break;
        }
        // ---- Brake 3: concurrency cap. ----
        if moved >= policy.max_in_flight {
            plan.skipped.push((shard, SkipReason::ConcurrencyCap));
            continue;
        }
        // ---- Brake 2: per-shard cooldown. ----
        if let Some(until) = cooldowns.get(&shard.0) {
            if *until > now {
                plan.skipped.push((
                    shard,
                    SkipReason::Cooldown {
                        remaining: until.saturating_duration_since(now),
                    },
                ));
                continue;
            }
        }
        // Walk to the next receiver that still has room and is not backed off.
        let mut placed = false;
        while cursor < receivers.len() {
            let (target, hex, deficit) = receivers[cursor].clone();
            if deficit == 0 {
                cursor += 1;
                continue;
            }
            // ---- Brake 4: peer backoff. ----
            if let Some(until) = peer_backoff.get(&hex) {
                if *until > now {
                    plan.skipped.push((
                        shard,
                        SkipReason::PeerBackoff {
                            peer: hex.clone(),
                            remaining: until.saturating_duration_since(now),
                        },
                    ));
                    cursor += 1;
                    continue;
                }
            }
            receivers[cursor].2 -= 1;
            plan.moves.push(PlannedMove { shard, target });
            moved += 1;
            placed = true;
            break;
        }
        if !placed && cursor >= receivers.len() {
            break;
        }
    }

    plan
}

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

/// What one [`Rebalancer::tick`] actually did.
#[derive(Debug, Clone, Default)]
pub struct RebalanceReport {
    /// Successful migrations: `(shard, target, epoch established)`.
    pub migrated: Vec<(ShardId, PubKey, u64)>,
    /// Attempted and failed. **The source still owns every one of these** — see
    /// [`crate::shard::HandoffError`].
    pub failed: Vec<(ShardId, PubKey, String)>,
    /// Deliberate non-moves and why.
    pub skipped: Vec<(ShardId, SkipReason)>,
    /// Peers that did not answer a probe this tick.
    pub unreachable: Vec<(PubKey, String)>,
    /// Peers skipped without even probing, because they are backed off.
    pub backed_off: Vec<(PubKey, Duration)>,
    /// **Unrecoverable**: shards whose owning node vanished and which could not
    /// be safely restored from a checkpoint. Never empty silently — the caller
    /// is expected to surface these.
    pub lost: Vec<ShardLoss>,
    /// Shards rebuilt on this node from a checkpoint after their owner died.
    /// **Rollbacks, not resurrections** — each carries the seconds of simulation
    /// that were lost and the tick it went back to.
    pub recovered: Vec<ShardRecovery>,
    /// Zombie claims actively fenced this tick: `(shard, peer, peer's stale
    /// epoch, whether the peer confirmed it dropped the shard)`. A peer that
    /// comes back believing it still owns a shard we restored is evicted here.
    pub fenced: Vec<(ShardId, PubKey, u64, bool)>,
    /// Shards no visible node had headroom for.
    pub unplaced: Vec<ShardId>,
}

impl RebalanceReport {
    /// Whether this tick moved nothing at all — the convergence predicate.
    pub fn is_quiet(&self) -> bool {
        self.migrated.is_empty() && self.failed.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The reconciler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PeerHealth {
    failures: u32,
    blocked_until: Option<Instant>,
    /// When this peer's *current* run of failures started. Cleared on any
    /// success. This is what makes [`RecoveryPolicy::grace`] a wall-clock
    /// guarantee rather than a count of however many ticks fit in a second.
    failing_since: Option<Instant>,
}

/// The periodic reconciler.
///
/// Owns exactly the state the brakes need — per-shard cooldowns, per-peer
/// failure counts, and the last observed ownership map used to tell a *loss*
/// from a *move*. It owns no authority: every actual transfer is delegated to
/// [`NetworkHandoffTransport`].
pub struct Rebalancer {
    local: PubKey,
    policy: RebalancePolicy,
    scheduler: Box<dyn ShardScheduler + Send>,
    /// shard id -> instant the cooldown expires.
    cooldowns: HashMap<u32, Instant>,
    /// peer key hex -> health.
    health: HashMap<String, PeerHealth>,
    /// peer key hex -> shards it was last seen holding. The basis for loss
    /// detection: a shard here, held by a node that is now gone, is lost.
    last_seen: HashMap<String, (PubKey, Vec<ShardId>)>,
    /// Losses already reported, so a dead node is mourned once, not every tick.
    mourned: HashSet<(u32, String)>,
    /// Where durable copies are, learned from peers' authenticated status
    /// replies **while they were alive**. A dead peer cannot tell us where its
    /// checkpoint is, so this cache is the only way a restore is ever possible.
    /// Every entry is re-fetched, re-hashed and re-bound to its shard before it
    /// is acted on — this map is a hint, never a credential.
    known_checkpoints: HashMap<u32, CheckpointRef>,
    /// Durability: the blob store to restore from, and when a restore is
    /// allowed. `None` ⇒ the death path behaves exactly as it did before
    /// [`crate::checkpoint`] existed: honest [`ShardLoss`], no replacement.
    recovery: Option<(CheckpointStore, RecoveryPolicy)>,
}

impl std::fmt::Debug for Rebalancer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rebalancer")
            .field("local", &self.local.to_hex())
            .field("policy", &self.policy)
            .field("cooldowns", &self.cooldowns.len())
            .field("peers_tracked", &self.health.len())
            .finish()
    }
}

impl Rebalancer {
    /// Build a reconciler for the node identified by `local`.
    pub fn new(
        local: PubKey,
        policy: RebalancePolicy,
        scheduler: Box<dyn ShardScheduler + Send>,
    ) -> Self {
        Self {
            local,
            policy,
            scheduler,
            cooldowns: HashMap::new(),
            health: HashMap::new(),
            last_seen: HashMap::new(),
            mourned: HashSet::new(),
            known_checkpoints: HashMap::new(),
            recovery: None,
        }
    }

    /// Enable **epoch-safe restore** on the death path, reading checkpoints from
    /// `store`.
    ///
    /// Without this a dead node's shards are reported as [`ShardLoss`] and
    /// nothing is started, which is the pre-durability behaviour and remains the
    /// default. With it, a death becomes a restore *only* when every one of the
    /// checks in [`crate::checkpoint`] passes; any doubt at all falls back to
    /// [`ShardLoss`].
    ///
    /// The store must be one **this** node can read. A node-local store cannot
    /// hold another node's checkpoints, so recovery across machines needs a
    /// shared blob store; with a local one this degrades to the honest fallback
    /// rather than to anything false.
    pub fn with_recovery(mut self, store: CheckpointStore, policy: RecoveryPolicy) -> Self {
        self.recovery = if policy.enabled {
            Some((store, policy))
        } else {
            None
        };
        self
    }

    /// Whether the death path will attempt restores.
    pub fn recovery_enabled(&self) -> bool {
        self.recovery.is_some()
    }

    /// The newest checkpoint this node has heard announced for `shard`.
    pub fn known_checkpoint(&self, shard: ShardId) -> Option<CheckpointRef> {
        self.known_checkpoints.get(&shard.0).copied()
    }

    /// Record a checkpoint announcement. Newest wins, ordered on `(epoch, tick)`
    /// so a peer replaying an old announcement cannot roll the cache backwards.
    fn learn_checkpoints(&mut self, refs: &[CheckpointRef]) {
        for r in refs {
            match self.known_checkpoints.get(&r.shard) {
                Some(cur) if (cur.epoch, cur.tick) >= (r.epoch, r.tick) => {}
                _ => {
                    self.known_checkpoints.insert(r.shard, *r);
                }
            }
        }
    }

    /// The policy in force.
    pub fn policy(&self) -> &RebalancePolicy {
        &self.policy
    }

    /// Whether `shard` is currently in cooldown.
    pub fn in_cooldown(&self, shard: ShardId, now: Instant) -> bool {
        self.cooldowns.get(&shard.0).is_some_and(|u| *u > now)
    }

    /// Consecutive probe/migration failures recorded against a peer.
    pub fn failures_against(&self, peer: &PubKey) -> u32 {
        self.health
            .get(&peer.to_hex())
            .map(|h| h.failures)
            .unwrap_or(0)
    }

    /// Whether a peer is currently in backoff (and so will not even be probed).
    pub fn is_backed_off(&self, peer: &PubKey, now: Instant) -> bool {
        self.health
            .get(&peer.to_hex())
            .and_then(|h| h.blocked_until)
            .is_some_and(|u| u > now)
    }

    fn record_failure(&mut self, peer: &PubKey, now: Instant) -> Duration {
        let h = self.health.entry(peer.to_hex()).or_insert(PeerHealth {
            failures: 0,
            blocked_until: None,
            failing_since: None,
        });
        h.failures = h.failures.saturating_add(1);
        h.failing_since.get_or_insert(now);
        let wait = self.policy.backoff_for(h.failures);
        h.blocked_until = Some(now + wait);
        wait
    }

    fn record_success(&mut self, peer: &PubKey) {
        if let Some(h) = self.health.get_mut(&peer.to_hex()) {
            h.failures = 0;
            h.blocked_until = None;
            h.failing_since = None;
        }
    }

    /// How long this peer has been continuously failing, and how many times.
    fn failure_run(&self, peer_hex: &str, now: Instant) -> (u32, Duration) {
        match self.health.get(peer_hex) {
            Some(h) => (
                h.failures,
                h.failing_since
                    .map(|t| now.saturating_duration_since(t))
                    .unwrap_or_default(),
            ),
            None => (0, Duration::ZERO),
        }
    }

    fn backoff_map(&self) -> HashMap<String, Instant> {
        self.health
            .iter()
            .filter_map(|(k, h)| h.blocked_until.map(|u| (k.clone(), u)))
            .collect()
    }

    /// Attempt an epoch-safe restore of `shard` onto this node.
    ///
    /// `None` means recovery is switched off. `Some(Err(_))` means it was
    /// attempted and **refused**; the caller must fall back to [`ShardLoss`].
    ///
    /// The liveness gate is deliberately the strict one:
    ///
    /// - a **lapsed lease** is a time-based, cluster-wide signal that the node
    ///   stopped renewing its discovery record for a full TTL — that is dead;
    /// - **unreachable** is only a local observation, so it additionally
    ///   requires [`RecoveryPolicy::min_failures`] consecutive failures *and*
    ///   [`RecoveryPolicy::grace`] of continuous failing. A peer in its first
    ///   backoff is slow, not dead, and nothing is restored from under it.
    #[allow(clippy::too_many_arguments)]
    fn try_restore(
        &self,
        transport: &NetworkHandoffTransport,
        shard: ShardId,
        owner: PubKey,
        owner_hex: &str,
        lease_lapsed: bool,
        now_unix: u64,
        now: Instant,
    ) -> Option<Result<ShardRecovery, RestoreError>> {
        let (store, policy) = self.recovery.as_ref()?;

        if !lease_lapsed {
            let (failures, failing_for) = self.failure_run(owner_hex, now);
            if failures < policy.min_failures || failing_for < policy.grace {
                return Some(Err(RestoreError::OwnerNotDead {
                    failures,
                    grace_remaining: policy.grace.saturating_sub(failing_for),
                }));
            }
        }

        let cp_ref = match self.known_checkpoints.get(&shard.0) {
            Some(r) => *r,
            None => return Some(Err(RestoreError::NoCheckpoint(shard))),
        };
        Some(restore_shard(
            &transport.authority(),
            store,
            &cp_ref,
            shard,
            owner,
            now_unix,
        ))
    }

    /// One reconciliation pass.
    ///
    /// 1. Take live routes from `dir` — already signature-verified, lease-checked
    ///    and **membership-filtered**, so a stranger never enters the node set.
    /// 2. Probe every member that is not in backoff.
    /// 3. Plan against the probed cluster.
    /// 4. Execute, through the ordinary two-phase handoff.
    /// 5. Report anything that was lost with a dead node, loudly.
    ///
    /// Returns what happened. A tick that changes nothing returns a report with
    /// [`RebalanceReport::is_quiet`] true.
    pub fn tick(
        &mut self,
        transport: &mut NetworkHandoffTransport,
        dir: &RouteDirectory,
        local_capacity: &Capacity,
        now_unix: u64,
        now: Instant,
    ) -> RebalanceReport {
        let mut report = RebalanceReport::default();

        // 1. Live, membership-approved routes only. Never widen this.
        let routes: Vec<PeerRoute> = dir
            .live_routes(now_unix)
            .into_iter()
            .filter(|r| r.pubkey != self.local)
            .collect();
        let live_keys: HashSet<String> = routes.iter().map(|r| r.pubkey.to_hex()).collect();

        // 2. Probe. A backed-off peer is not contacted at all — that is the
        //    difference between backing off and merely retrying more slowly.
        let mut peers: Vec<PeerStatus> = Vec::new();
        let mut unreachable: Vec<PubKey> = Vec::new();
        for route in &routes {
            if let Some(until) = self
                .health
                .get(&route.pubkey.to_hex())
                .and_then(|h| h.blocked_until)
            {
                if until > now {
                    report
                        .backed_off
                        .push((route.pubkey, until.saturating_duration_since(now)));
                    unreachable.push(route.pubkey);
                    continue;
                }
            }
            match transport.probe_peer(route) {
                Ok(status) => {
                    self.record_success(&route.pubkey);
                    self.last_seen.insert(
                        route.pubkey.to_hex(),
                        (route.pubkey, status.shards.clone()),
                    );
                    // A peer that answered is alive: nothing it holds is lost,
                    // so clear any mourning we recorded for it earlier.
                    self.mourned.retain(|(_, k)| k != &route.pubkey.to_hex());
                    // Cache where its durable copies are, while we still can.
                    self.learn_checkpoints(&status.checkpoints);
                    peers.push(status);
                }
                Err(e) => {
                    let wait = self.record_failure(&route.pubkey, now);
                    warn!(
                        peer = %route.pubkey.to_hex(),
                        error = %e,
                        backoff_secs = wait.as_secs(),
                        "cluster peer did not answer a status probe — backing off"
                    );
                    report.unreachable.push((route.pubkey, e.to_string()));
                    unreachable.push(route.pubkey);
                }
            }
        }

        // 5a. Loss accounting. A peer we have seen before, which is now either
        //     out of the live route set (lease lapsed / revoked) or unreachable,
        //     took its shards' state with it. This is reported as LOSS. It is
        //     never quietly "recovered": see the module docs.
        let visible_shards: HashSet<u32> = peers
            .iter()
            .flat_map(|p| p.shards.iter().map(|s| s.0))
            .chain(transport.authority().owned_shards().iter().map(|s| s.0))
            .collect();
        let unreachable_hex: HashSet<String> = unreachable.iter().map(|k| k.to_hex()).collect();
        let mut losses: Vec<ShardLoss> = Vec::new();
        let mut recovered: Vec<ShardRecovery> = Vec::new();
        // Snapshot so the restore below can borrow `self` freely; a cluster's
        // peer set is small and this runs once per rebalance interval.
        let seen: Vec<(String, PubKey, Vec<ShardId>)> = self
            .last_seen
            .iter()
            .map(|(hex, (k, s))| (hex.clone(), *k, s.clone()))
            .collect();
        for (hex, key, shards) in &seen {
            let lapsed = !live_keys.contains(hex);
            let dead = unreachable_hex.contains(hex);
            if !lapsed && !dead {
                continue;
            }
            for shard in shards {
                if visible_shards.contains(&shard.0) {
                    // Someone else reports holding it — it MOVED, it is not lost.
                    continue;
                }
                if !self.mourned.insert((shard.0, hex.clone())) {
                    continue;
                }
                let cause = if lapsed {
                    LossCause::LeaseLapsed
                } else {
                    LossCause::Unreachable(
                        report
                            .unreachable
                            .iter()
                            .find(|(k, _)| &k.to_hex() == hex)
                            .map(|(_, e)| e.clone())
                            .unwrap_or_else(|| "peer stopped answering".into()),
                    )
                };

                // 5b. Try to make this a bounded rollback instead of a total
                //     loss. Every failure path below ends in ShardLoss; none of
                //     them ever starts an empty shard under a real shard id.
                let refused = match self.try_restore(
                    transport, *shard, *key, hex, lapsed, now_unix, now,
                ) {
                    Some(Ok(rec)) => {
                        // It is ours now, at a strictly higher epoch. Do not
                        // mourn it, and do not let a later tick mourn it either.
                        recovered.push(rec);
                        continue;
                    }
                    Some(Err(e)) => Some(e.to_string()),
                    None => None,
                };
                losses.push(ShardLoss {
                    shard: *shard,
                    last_owner: *key,
                    cause,
                    restore_refused: refused,
                });
            }
        }
        for loss in &losses {
            warn!(
                shard = loss.shard.0,
                node = %loss.last_owner.to_hex(),
                "{loss}"
            );
        }
        report.lost = losses;
        report.recovered = recovered;

        // 5c. The active half of the fence. Any peer that answered while
        //     claiming a shard we own at a LOWER epoch is a zombie — typically
        //     one that was restored out from under and has just come back. It is
        //     told, over the authenticated channel, to drop it. `fence_peer`
        //     refuses to send unless we genuinely out-rank it, so this is an
        //     assertion of fact and the comparison is total-ordered: two nodes
        //     can never both decide they win.
        for status in &peers {
            for (shard, peer_epoch) in &status.epochs {
                let ours = match transport.authority().epoch_of(*shard) {
                    Some(e) => e,
                    None => continue,
                };
                if ours <= *peer_epoch {
                    continue;
                }
                let route = match routes.iter().find(|r| r.pubkey == status.node) {
                    Some(r) => r,
                    None => continue,
                };
                match transport.fence_peer(route, *shard, *peer_epoch) {
                    Ok(dropped) => {
                        warn!(
                            shard = shard.0,
                            peer = %status.node.to_hex(),
                            peer_epoch = *peer_epoch,
                            our_epoch = ours,
                            dropped,
                            "fenced a stale shard claim — no split-brain: the higher epoch wins"
                        );
                        report.fenced.push((*shard, status.node, *peer_epoch, dropped));
                    }
                    Err(e) => warn!(
                        shard = shard.0,
                        peer = %status.node.to_hex(),
                        error = %e,
                        "could not deliver a fence to a stale claimant"
                    ),
                }
            }
        }
        // Forget lapsed peers so we do not re-mourn them forever.
        self.last_seen
            .retain(|hex, _| live_keys.contains(hex) || !unreachable_hex.contains(hex));

        // 3. Plan.
        let local_shards = transport.authority().owned_shards();
        let view = ClusterView {
            local: self.local,
            local_capacity: local_capacity.clone(),
            local_shards,
            peers,
            unreachable,
        };
        let backoff = self.backoff_map();
        let plan = plan_moves(
            &view,
            &self.policy,
            self.scheduler.as_ref(),
            &self.cooldowns,
            &backoff,
            now,
        );
        report.skipped = plan.skipped;
        report.unplaced = plan.unplaced;

        // 4. Execute — through the unchanged, fail-closed handoff.
        for mv in plan.moves {
            let route = match dir.route_for(&mv.target, now_unix) {
                Ok(r) => r,
                Err(e) => {
                    report
                        .skipped
                        .push((mv.shard, SkipReason::NoRoute(e.to_string())));
                    continue;
                }
            };
            if let Err(e) = transport.route_from_directory(mv.shard, &mv.target, dir, now_unix) {
                report
                    .skipped
                    .push((mv.shard, SkipReason::NoRoute(e.to_string())));
                continue;
            }
            debug_assert_eq!(transport.route(mv.shard).map(|r| &r.addr), Some(&route.addr));
            match transport.migrate_shard(mv.shard) {
                Ok(epoch) => {
                    // Cooldown starts on SUCCESS. A shard that failed to move is
                    // still here and still misplaced; the peer backoff is what
                    // stops us retrying it too fast.
                    self.cooldowns
                        .insert(mv.shard.0, now + self.policy.cooldown);
                    self.record_success(&mv.target);
                    info!(
                        shard = mv.shard.0,
                        epoch,
                        target = %mv.target.to_hex(),
                        "rebalancer migrated shard"
                    );
                    report.migrated.push((mv.shard, mv.target, epoch));
                }
                Err(e) => {
                    let wait = self.record_failure(&mv.target, now);
                    warn!(
                        shard = mv.shard.0,
                        target = %mv.target.to_hex(),
                        error = %e,
                        backoff_secs = wait.as_secs(),
                        "rebalancer migration failed — SOURCE RETAINS the shard and its state"
                    );
                    report.failed.push((mv.shard, mv.target, e.to_string()));
                }
            }
        }

        report
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::scaling::SpreadScheduler;
    use magnetite_seams::identity::RawKeypairAuth;

    fn key(n: u8) -> PubKey {
        // Deterministic distinct keys; only their bytes matter here.
        let mut b = [0u8; 32];
        b[0] = n;
        PubKey(b)
    }

    fn cap(shards: u32) -> Capacity {
        Capacity {
            cpu_cores: shards,
            ram_mb: 1024 * shards as u64,
            bandwidth_mbps: 100,
            free_slots: 100,
            max_shards: shards,
        }
    }

    fn status(k: PubKey, shards: &[u32], ceiling: u32) -> PeerStatus {
        PeerStatus {
            node: k,
            shards: shards.iter().copied().map(ShardId).collect(),
            capacity: cap(ceiling),
            epochs: Vec::new(),
            checkpoints: Vec::new(),
        }
    }

    fn view(local: PubKey, local_cap: u32, mine: &[u32], peers: Vec<PeerStatus>) -> ClusterView {
        ClusterView {
            local,
            local_capacity: cap(local_cap),
            local_shards: mine.iter().copied().map(ShardId).collect(),
            peers,
            unreachable: Vec::new(),
        }
    }

    fn plan(v: &ClusterView, p: &RebalancePolicy) -> MigrationPlan {
        plan_moves(
            v,
            p,
            &SpreadScheduler,
            &HashMap::new(),
            &HashMap::new(),
            Instant::now(),
        )
    }

    fn loose() -> RebalancePolicy {
        RebalancePolicy {
            deadband_shards: 0,
            max_in_flight: 64,
            ..Default::default()
        }
    }

    #[test]
    fn a_lone_node_with_no_peers_plans_nothing() {
        let v = view(key(1), 4, &[1, 2, 3, 4, 5, 6], vec![]);
        assert!(plan(&v, &loose()).is_empty());
    }

    #[test]
    fn an_overloaded_node_sheds_to_an_empty_peer() {
        let v = view(key(1), 4, &[1, 2, 3, 4], vec![status(key(2), &[], 4)]);
        let p = plan(&v, &loose());
        assert!(!p.is_empty(), "four shards, two equal nodes: must shed");
        assert!(p.moves.iter().all(|m| m.target == key(2)));
    }

    /// The headline property. Apply the plan, re-plan, and the second pass must
    /// be empty — otherwise the loop moves shards forever.
    #[test]
    fn steady_state_issues_no_migrations() {
        let policy = loose();
        let mut mine: Vec<u32> = (1..=8).collect();
        let mut theirs: Vec<u32> = Vec::new();

        // Converge by repeatedly planning and applying.
        let mut passes = 0;
        loop {
            let v = view(
                key(1),
                4,
                &mine,
                vec![status(key(2), &theirs, 4)],
            );
            let p = plan(&v, &policy);
            if p.is_empty() {
                break;
            }
            for m in &p.moves {
                mine.retain(|s| *s != m.shard.0);
                theirs.push(m.shard.0);
            }
            theirs.sort_unstable();
            passes += 1;
            assert!(passes < 20, "rebalancer failed to converge — it is thrashing");
        }
        assert!(passes > 0, "test is vacuous: nothing ever moved");

        // And it STAYS quiet: ten more passes, zero migrations.
        for _ in 0..10 {
            let v = view(key(1), 4, &mine, vec![status(key(2), &theirs, 4)]);
            assert!(
                plan(&v, &policy).is_empty(),
                "converged cluster planned a migration: {mine:?} / {theirs:?}"
            );
        }
        // Two equal nodes, eight shards: four each.
        assert_eq!(mine.len(), 4);
        assert_eq!(theirs.len(), 4);
    }

    #[test]
    fn capacity_decides_the_share_not_node_count() {
        // Node A can hold 6, node B can hold 2. Eight shards, all on A.
        let mine: Vec<u32> = (1..=8).collect();
        let v = view(key(1), 6, &mine, vec![status(key(2), &[], 2)]);
        let p = plan(&v, &loose());
        // A keeps the larger share; B is offered at most its ceiling.
        assert!(p.moves.len() <= 2, "small node must not be overfilled: {p:?}");
        assert!(!p.moves.is_empty());
    }

    #[test]
    fn deadband_suppresses_a_trivial_imbalance() {
        // Five shards over two equal nodes: 3/2 is as balanced as it gets.
        let v = view(key(1), 4, &[1, 2, 3], vec![status(key(2), &[4, 5], 4)]);
        let p = plan(
            &v,
            &RebalancePolicy {
                deadband_shards: 1,
                ..Default::default()
            },
        );
        assert!(p.is_empty(), "moved for a one-shard imbalance: {p:?}");
        assert!(matches!(
            p.skipped.first(),
            Some((_, SkipReason::WithinDeadband { .. }))
        ));
    }

    #[test]
    fn a_shard_in_cooldown_is_not_moved_again() {
        let v = view(key(1), 4, &[1, 2, 3, 4], vec![status(key(2), &[], 4)]);
        let now = Instant::now();
        let mut cd = HashMap::new();
        // Shard 1 just moved (and came back somehow); it must be left alone.
        cd.insert(1u32, now + Duration::from_secs(60));
        let p = plan_moves(&v, &loose(), &SpreadScheduler, &cd, &HashMap::new(), now);
        assert!(
            p.moves.iter().all(|m| m.shard.0 != 1),
            "moved a shard that is in cooldown"
        );
        assert!(p
            .skipped
            .iter()
            .any(|(s, r)| s.0 == 1 && matches!(r, SkipReason::Cooldown { .. })));
    }

    #[test]
    fn concurrency_cap_limits_one_pass() {
        let mine: Vec<u32> = (1..=10).collect();
        let v = view(key(1), 8, &mine, vec![status(key(2), &[], 8)]);
        let p = plan(
            &v,
            &RebalancePolicy {
                deadband_shards: 0,
                max_in_flight: 2,
                ..Default::default()
            },
        );
        assert_eq!(p.moves.len(), 2, "concurrency cap not enforced");
        assert!(p
            .skipped
            .iter()
            .any(|(_, r)| matches!(r, SkipReason::ConcurrencyCap)));
    }

    #[test]
    fn a_backed_off_peer_receives_nothing() {
        let v = view(key(1), 4, &[1, 2, 3, 4], vec![status(key(2), &[], 4)]);
        let now = Instant::now();
        let mut bo = HashMap::new();
        bo.insert(key(2).to_hex(), now + Duration::from_secs(30));
        let p = plan_moves(&v, &loose(), &SpreadScheduler, &HashMap::new(), &bo, now);
        assert!(p.is_empty(), "planned a move onto a backed-off peer");
        assert!(p
            .skipped
            .iter()
            .any(|(_, r)| matches!(r, SkipReason::PeerBackoff { .. })));
    }

    #[test]
    fn backoff_grows_exponentially_and_is_capped() {
        let p = RebalancePolicy {
            backoff_base: Duration::from_secs(5),
            backoff_max: Duration::from_secs(60),
            ..Default::default()
        };
        assert_eq!(p.backoff_for(0), Duration::ZERO);
        assert_eq!(p.backoff_for(1), Duration::from_secs(5));
        assert_eq!(p.backoff_for(2), Duration::from_secs(10));
        assert_eq!(p.backoff_for(3), Duration::from_secs(20));
        assert_eq!(p.backoff_for(10), Duration::from_secs(60), "not capped");
        assert_eq!(p.backoff_for(u32::MAX), Duration::from_secs(60));
    }

    #[test]
    fn repeated_failures_against_a_peer_back_off_further_each_time() {
        let id = RawKeypairAuth::generate();
        let mut r = Rebalancer::new(
            id.node_pubkey(),
            RebalancePolicy {
                backoff_base: Duration::from_secs(4),
                backoff_max: Duration::from_secs(64),
                ..Default::default()
            },
            Box::new(SpreadScheduler),
        );
        let peer = key(9);
        let now = Instant::now();
        assert_eq!(r.record_failure(&peer, now), Duration::from_secs(4));
        assert_eq!(r.record_failure(&peer, now), Duration::from_secs(8));
        assert_eq!(r.record_failure(&peer, now), Duration::from_secs(16));
        assert_eq!(r.failures_against(&peer), 3);
        assert!(r.is_backed_off(&peer, now));
        assert!(!r.is_backed_off(&peer, now + Duration::from_secs(17)));
        // A success wipes the slate — a peer that comes back is not punished.
        r.record_success(&peer);
        assert_eq!(r.failures_against(&peer), 0);
        assert!(!r.is_backed_off(&peer, now));
    }

    #[test]
    fn a_shard_reported_by_two_nodes_is_counted_once() {
        // Mid-migration views can double-report. Counting it twice would invent
        // load and provoke a spurious move.
        let v = view(key(1), 4, &[1, 2], vec![status(key(2), &[2, 3], 4)]);
        assert_eq!(v.all_shards().len(), 3);
    }

    #[test]
    fn node_ordering_is_stable_so_placement_is_deterministic() {
        let a = view(
            key(1),
            4,
            &[1, 2, 3, 4],
            vec![status(key(2), &[], 4), status(key(3), &[], 4)],
        );
        let b = view(
            key(1),
            4,
            &[1, 2, 3, 4],
            vec![status(key(3), &[], 4), status(key(2), &[], 4)],
        );
        assert_eq!(a.nodes().len(), 3);
        let ids_a: Vec<_> = a.nodes().into_iter().map(|n| n.node.0).collect();
        let ids_b: Vec<_> = b.nodes().into_iter().map(|n| n.node.0).collect();
        assert_eq!(ids_a, ids_b, "peer order must not change placement");
        assert_eq!(plan(&a, &loose()).moves, plan(&b, &loose()).moves);
    }

    #[test]
    fn a_loss_message_says_lost_not_recovered() {
        let l = ShardLoss {
            shard: ShardId(7),
            last_owner: key(3),
            cause: LossCause::LeaseLapsed,
            restore_refused: Some("no checkpoint is known for shard 7".into()),
        };
        let msg = l.to_string();
        assert!(msg.contains("STATE LOST"));
        assert!(msg.contains("NO CHECKPOINT RESTORE WAS POSSIBLE"));
        assert!(msg.contains("world is gone"));
        assert!(
            !msg.to_lowercase().contains("recovered from"),
            "loss must never read as a recovery"
        );
    }

    #[test]
    fn adding_a_node_makes_the_existing_ones_shed() {
        // Two nodes, balanced 4/4. A third joins with equal capacity.
        let before = view(key(1), 4, &[1, 2, 3, 4], vec![status(key(2), &[5, 6, 7, 8], 4)]);
        assert!(plan(&before, &loose()).is_empty(), "already balanced");

        let after = view(
            key(1),
            4,
            &[1, 2, 3, 4],
            vec![status(key(2), &[5, 6, 7, 8], 4), status(key(3), &[], 4)],
        );
        let p = plan(&after, &loose());
        assert!(!p.is_empty(), "a joining node must take share");
        assert!(p.moves.iter().all(|m| m.target == key(3)));
    }
}
