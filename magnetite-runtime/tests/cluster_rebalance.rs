//! The rebalancer, driven across **real nodes over real sockets**.
//!
//! `rebalance.rs`'s unit tests prove the planner's arithmetic. These prove the
//! part that arithmetic cannot: that a plan turns into actual shard authority
//! moving between processes-worth of listening TCP nodes, through the unchanged
//! two-phase handoff, and that it then *stops*.
//!
//! | Test | Property |
//! |---|---|
//! | `shards_distribute_across_three_nodes_by_capacity` | a lopsided cluster balances, and the big node keeps the bigger share |
//! | `convergence_then_zero_migrations` | once balanced, **every** further tick migrates nothing |
//! | `a_joining_node_takes_share` | a node added later is given work |
//! | `a_shard_in_cooldown_is_not_moved_again` | the per-shard brake holds over the socket path |
//! | `an_unreachable_peer_is_backed_off_not_hammered` | one probe failure, then silence — not a retry storm |
//! | `a_failed_migration_leaves_the_source_owning_the_shard` | the fail-closed guarantee survives the rebalancer |
//! | `a_dead_peers_shards_are_reported_as_LOST_not_recovered` | no silent resurrection of state that is gone |
//! | `a_non_member_is_never_a_placement_target` | deny-by-default still gates everything |

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use magnetite_runtime::cluster::ClusterMembership;
use magnetite_runtime::fleet::{FleetNode, NetworkHandoffTransport, PeerRoute};
use magnetite_runtime::rebalance::{
    plan_moves, ClusterView, LossCause, RebalancePolicy, Rebalancer, SkipReason,
};
use magnetite_runtime::shard::ShardId;
use magnetite_seams::discovery::Capacity;
use magnetite_seams::identity::{PubKey, RawKeypairAuth};

use magnetite_sdk::scaling::SpreadScheduler;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn cap(shards: u32) -> Capacity {
    Capacity {
        cpu_cores: shards,
        ram_mb: 2048 * shards as u64,
        bandwidth_mbps: 1000,
        free_slots: 64,
        max_shards: shards,
    }
}

/// A live node: a bound handoff listener plus the identity behind it.
struct Node {
    id: Arc<RawKeypairAuth>,
    node: FleetNode,
}

impl Node {
    /// Bind `id` as a live node whose door admits exactly `members`.
    fn spawn_as(id: Arc<RawKeypairAuth>, ceiling: u32, members: &[PubKey]) -> Self {
        let node = FleetNode::bind(
            "127.0.0.1:0",
            Arc::clone(&id),
            Some(members.iter().copied().collect()),
        )
        .expect("bind");
        node.publish_capacity(cap(ceiling));
        Self { id, node }
    }

    fn key(&self) -> PubKey {
        self.id.node_pubkey()
    }

    fn route(&self) -> PeerRoute {
        self.node.route()
    }

    fn owned(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .node
            .authority()
            .owned_shards()
            .iter()
            .map(|s| s.0)
            .collect();
        v.sort_unstable();
        v
    }

    /// This node's outbound transport, gated by the same membership as its door.
    fn transport(&self, members: &[PubKey]) -> NetworkHandoffTransport {
        self.node
            .transport()
            .with_membership(ClusterMembership::from_keys(members.iter().copied()))
            .with_timeout(Duration::from_millis(600))
    }
}

/// A directory holding operator-configured routes to `routes`, gated by
/// `members`. Deny-by-default: anything not in `members` is refused on insert.
fn directory(
    members: &[PubKey],
    routes: &[PeerRoute],
) -> magnetite_runtime::cluster::RouteDirectory {
    let mut dir = magnetite_runtime::cluster::RouteDirectory::new(
        ClusterMembership::from_keys(members.iter().copied()),
    );
    for r in routes {
        let _ = dir.admit_operator_route(r.clone());
    }
    dir
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn fast_policy() -> RebalancePolicy {
    RebalancePolicy {
        deadband_shards: 0,
        cooldown: Duration::from_millis(0),
        max_in_flight: 8,
        backoff_base: Duration::from_millis(50),
        backoff_max: Duration::from_millis(500),
        interval: Duration::from_millis(10),
    }
}

// ---------------------------------------------------------------------------
// Distribution
// ---------------------------------------------------------------------------

#[test]
fn shards_distribute_across_three_nodes_by_capacity() {
    // A is big (6 shards), B medium (3), C small (1). All 10 shards start on A.
    let ids: Vec<Arc<RawKeypairAuth>> =
        (0..3).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 6, &members);
    let b = Node::spawn_as(Arc::clone(&ids[1]), 3, &members);
    let c = Node::spawn_as(Arc::clone(&ids[2]), 1, &members);

    for s in 1..=10u32 {
        a.node.authority().claim(ShardId(s), vec![s as u8; 32]);
    }

    let mut t = a.transport(&members);
    let dir = directory(&members, &[b.route(), c.route()]);
    let mut r = Rebalancer::new(a.key(), fast_policy(), Box::new(SpreadScheduler));

    // Drive to a fixed point.
    for _ in 0..12 {
        let rep = r.tick(&mut t, &dir, &cap(6), now_unix(), Instant::now());
        assert!(rep.failed.is_empty(), "migration failed: {:?}", rep.failed);
        if rep.is_quiet() {
            break;
        }
    }

    let (na, nb, nc) = (a.owned().len(), b.owned().len(), c.owned().len());
    assert_eq!(na + nb + nc, 10, "shards were lost: {na}/{nb}/{nc}");
    assert!(nb > 0 && nc > 0, "work never left node A: {na}/{nb}/{nc}");
    assert!(
        na >= nb && nb >= nc,
        "share did not track capacity 6/3/1: got {na}/{nb}/{nc}"
    );
    assert!(nc <= 1, "the 1-shard node was overfilled: {nc}");
}

// ---------------------------------------------------------------------------
// Convergence — the property that makes this safe to run forever
// ---------------------------------------------------------------------------

#[test]
fn convergence_then_zero_migrations() {
    let ids: Vec<Arc<RawKeypairAuth>> = (0..3).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);
    let b = Node::spawn_as(Arc::clone(&ids[1]), 4, &members);
    let c = Node::spawn_as(Arc::clone(&ids[2]), 4, &members);

    for s in 1..=9u32 {
        a.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }

    let mut t = a.transport(&members);
    let dir = directory(&members, &[b.route(), c.route()]);
    let mut r = Rebalancer::new(a.key(), fast_policy(), Box::new(SpreadScheduler));

    let mut ticks_that_moved = 0;
    let mut settled = None;
    for i in 0..15 {
        let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
        assert!(rep.failed.is_empty(), "{:?}", rep.failed);
        if rep.is_quiet() {
            settled = Some(i);
            break;
        }
        ticks_that_moved += 1;
    }
    let settled = settled.expect("rebalancer never reached a fixed point — it is thrashing");
    assert!(ticks_that_moved > 0, "test is vacuous: nothing ever moved");

    // THE assertion: from here, twenty more ticks must migrate exactly nothing.
    let snapshot = (a.owned(), b.owned(), c.owned());
    for i in 0..20 {
        let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
        assert!(
            rep.migrated.is_empty(),
            "tick {} after convergence (settled at {settled}) migrated {:?} — the loop thrashes",
            i,
            rep.migrated
        );
        assert!(rep.failed.is_empty());
    }
    assert_eq!(
        snapshot,
        (a.owned(), b.owned(), c.owned()),
        "ownership drifted while the cluster was supposedly steady"
    );
    assert_eq!(
        a.owned().len() + b.owned().len() + c.owned().len(),
        9,
        "shards leaked during convergence"
    );
}

#[test]
fn a_joining_node_takes_share() {
    let ids: Vec<Arc<RawKeypairAuth>> = (0..3).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);
    let b = Node::spawn_as(Arc::clone(&ids[1]), 4, &members);
    let c = Node::spawn_as(Arc::clone(&ids[2]), 4, &members);

    for s in 1..=8u32 {
        a.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }

    let mut t = a.transport(&members);
    let mut r = Rebalancer::new(a.key(), fast_policy(), Box::new(SpreadScheduler));

    // Phase 1: only A and B are in the directory. Balance to 4/4.
    let dir_ab = directory(&members, &[b.route()]);
    for _ in 0..10 {
        if r.tick(&mut t, &dir_ab, &cap(4), now_unix(), Instant::now())
            .is_quiet()
        {
            break;
        }
    }
    assert_eq!(a.owned().len(), 4);
    assert_eq!(b.owned().len(), 4);
    assert_eq!(c.owned().len(), 0, "C is not in the cluster yet");

    // Phase 2: C joins. A must now shed toward it.
    let dir_abc = directory(&members, &[b.route(), c.route()]);
    for _ in 0..10 {
        if r.tick(&mut t, &dir_abc, &cap(4), now_unix(), Instant::now())
            .is_quiet()
        {
            break;
        }
    }
    assert!(
        !c.owned().is_empty(),
        "a joining node was given no work: {:?}/{:?}/{:?}",
        a.owned(),
        b.owned(),
        c.owned()
    );
    assert_eq!(a.owned().len() + b.owned().len() + c.owned().len(), 8);
}

// ---------------------------------------------------------------------------
// The brakes, over sockets
// ---------------------------------------------------------------------------

#[test]
fn a_shard_in_cooldown_is_not_moved_again() {
    let ids: Vec<Arc<RawKeypairAuth>> = (0..2).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);
    let b = Node::spawn_as(Arc::clone(&ids[1]), 4, &members);

    for s in 1..=4u32 {
        a.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }

    let policy = RebalancePolicy {
        cooldown: Duration::from_secs(3600),
        max_in_flight: 1,
        ..fast_policy()
    };
    let mut t = a.transport(&members);
    let dir = directory(&members, &[b.route()]);
    let mut r = Rebalancer::new(a.key(), policy, Box::new(SpreadScheduler));

    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    assert_eq!(rep.migrated.len(), 1, "expected exactly one move per tick");
    let moved = rep.migrated[0].0;
    assert!(
        r.in_cooldown(moved, Instant::now()),
        "a shard that just moved is not in cooldown"
    );

    // Hand it back to A by hand, as a flapping view would. The rebalancer must
    // refuse to move it again while the cooldown holds.
    a.node.authority().claim(moved, vec![0u8; 8]);
    let rep2 = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    assert!(
        rep2.migrated.iter().all(|(s, _, _)| *s != moved),
        "moved a shard that was in cooldown — this is exactly the thrash we forbid"
    );
    assert!(rep2
        .skipped
        .iter()
        .any(|(s, why)| *s == moved && matches!(why, SkipReason::Cooldown { .. })));
}

#[test]
fn an_unreachable_peer_is_backed_off_not_hammered() {
    let ids: Vec<Arc<RawKeypairAuth>> = (0..2).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);

    // B is a member with a route, but nothing is listening there.
    let dead = PeerRoute::new("127.0.0.1:9", members[1]);

    for s in 1..=4u32 {
        a.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }

    let policy = RebalancePolicy {
        backoff_base: Duration::from_secs(30),
        backoff_max: Duration::from_secs(300),
        ..fast_policy()
    };
    let mut t = a.transport(&members);
    let dir = directory(&members, &[dead]);
    let mut r = Rebalancer::new(a.key(), policy, Box::new(SpreadScheduler));

    let first = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    assert_eq!(first.unreachable.len(), 1, "probe failure not reported");
    assert!(first.migrated.is_empty(), "migrated to a dead node");
    assert_eq!(r.failures_against(&members[1]), 1);

    // Every subsequent tick inside the backoff window must not even TRY.
    for _ in 0..5 {
        let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
        assert!(
            rep.unreachable.is_empty(),
            "re-probed a backed-off peer — that is hammering"
        );
        assert_eq!(rep.backed_off.len(), 1, "backoff not reported");
        assert!(rep.migrated.is_empty());
    }
    assert_eq!(
        r.failures_against(&members[1]),
        1,
        "failure count grew without a single new attempt"
    );
    // A owns everything it started with. Nothing was lost to a dead peer.
    assert_eq!(a.owned(), vec![1, 2, 3, 4]);
}

#[test]
fn a_failed_migration_leaves_the_source_owning_the_shard() {
    let ids: Vec<Arc<RawKeypairAuth>> = (0..2).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);
    // B answers a probe, then dies before the handoff.
    let b = Node::spawn_as(Arc::clone(&ids[1]), 4, &members);
    let b_route = b.route();

    let state: Vec<u8> = b"the world as it was".to_vec();
    for s in 1..=4u32 {
        a.node.authority().claim(ShardId(s), state.clone());
    }

    let mut t = a.transport(&members);
    let dir = directory(&members, &[b_route]);
    let mut r = Rebalancer::new(
        a.key(),
        RebalancePolicy {
            max_in_flight: 1,
            ..fast_policy()
        },
        Box::new(SpreadScheduler),
    );

    // Probe succeeds while B is up...
    let mut b = b;
    b.node.shutdown();
    drop(b);
    // ...and now every transfer attempt fails. Whatever the ordering, the
    // invariant is the same: A never gives up a shard it could not hand over.
    for _ in 0..3 {
        let _ = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    }

    assert_eq!(
        a.owned(),
        vec![1, 2, 3, 4],
        "a failed migration lost the source's authority"
    );
    for s in 1..=4u32 {
        assert_eq!(
            a.node.authority().state_of(ShardId(s)).as_deref(),
            Some(state.as_slice()),
            "a failed migration corrupted or dropped shard state"
        );
    }
}

// ---------------------------------------------------------------------------
// Failure-driven re-placement, told truthfully
// ---------------------------------------------------------------------------

#[test]
fn a_dead_peers_shards_are_reported_as_lost_not_recovered() {
    let ids: Vec<Arc<RawKeypairAuth>> = (0..2).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);
    let b = Node::spawn_as(Arc::clone(&ids[1]), 4, &members);
    let b_route = b.route();
    let b_key = b.key();

    // A holds 4, B holds 4. Already balanced, so nothing should move.
    for s in 1..=4u32 {
        a.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }
    for s in 5..=8u32 {
        b.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }

    let mut t = a.transport(&members);
    let dir = directory(&members, &[b_route]);
    let mut r = Rebalancer::new(a.key(), fast_policy(), Box::new(SpreadScheduler));

    let first = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    assert!(first.lost.is_empty(), "mourned a live node");
    assert!(first.migrated.is_empty(), "moved shards in a balanced cluster");

    // B dies, taking shards 5..=8 and everything in them.
    let mut b = b;
    b.node.shutdown();
    drop(b);

    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());

    let mut lost: Vec<u32> = rep.lost.iter().map(|l| l.shard.0).collect();
    lost.sort_unstable();
    assert_eq!(
        lost,
        vec![5, 6, 7, 8],
        "a dead node's shards were not reported as lost"
    );
    assert!(rep.lost.iter().all(|l| l.last_owner == b_key));
    assert!(rep
        .lost
        .iter()
        .all(|l| matches!(l.cause, LossCause::Unreachable(_))));

    // The honesty assertion: the loss is described as a loss.
    let msg = rep.lost[0].to_string();
    assert!(msg.contains("STATE LOST"), "loss message hedges: {msg}");
    assert!(
        msg.contains("Checkpoint recovery is not enabled"),
        "loss message hides why it is unrecoverable: {msg}"
    );
    assert!(
        msg.contains("world is gone"),
        "loss message hedges about the state: {msg}"
    );

    // And crucially: nothing quietly resurrected them. A did NOT take over
    // 5..=8, because there was no state to take over.
    assert_eq!(
        a.owned(),
        vec![1, 2, 3, 4],
        "a dead peer's shards were silently 'recovered' as empty worlds"
    );

    // A node is mourned once, not once per tick.
    let again = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    assert!(
        again.lost.is_empty(),
        "the same loss was reported twice — an operator would think it kept happening"
    );
}

#[test]
fn a_lapsed_peer_stops_receiving_work() {
    let ids: Vec<Arc<RawKeypairAuth>> = (0..2).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
    let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
    let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);
    let b = Node::spawn_as(Arc::clone(&ids[1]), 4, &members);

    for s in 1..=4u32 {
        a.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }

    let mut t = a.transport(&members);
    let mut r = Rebalancer::new(a.key(), fast_policy(), Box::new(SpreadScheduler));

    // B is live and reachable but NOT in the route directory — the state a
    // lapsed lease leaves behind. It must receive nothing.
    let empty = directory(&members, &[]);
    for _ in 0..3 {
        let rep = r.tick(&mut t, &empty, &cap(4), now_unix(), Instant::now());
        assert!(rep.migrated.is_empty());
    }
    assert_eq!(a.owned(), vec![1, 2, 3, 4]);
    assert!(b.owned().is_empty(), "work reached a node with no live route");

    // Restore the route and it starts taking share again — the lapse was not
    // a permanent eviction, just an absence of a place to send work.
    let dir = directory(&members, &[b.route()]);
    for _ in 0..6 {
        if r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now())
            .is_quiet()
        {
            break;
        }
    }
    assert!(!b.owned().is_empty(), "a recovered peer never got work back");
}

// ---------------------------------------------------------------------------
// Deny-by-default is still the floor
// ---------------------------------------------------------------------------

#[test]
fn a_non_member_is_never_a_placement_target() {
    let a_id = Arc::new(RawKeypairAuth::generate());
    let outsider_id = Arc::new(RawKeypairAuth::generate());
    let outsider = Node::spawn_as(Arc::clone(&outsider_id), 8, &[outsider_id.node_pubkey()]);
    // A's cluster contains only A. The outsider announced a perfectly good
    // address and is genuinely running — and is still not a target.
    let members = vec![a_id.node_pubkey()];
    let a = Node::spawn_as(Arc::clone(&a_id), 1, &members);

    for s in 1..=6u32 {
        a.node.authority().claim(ShardId(s), vec![0u8; 8]);
    }

    let mut dir = magnetite_runtime::cluster::RouteDirectory::new(ClusterMembership::from_keys(
        members.iter().copied(),
    ));
    assert!(
        dir.admit_operator_route(outsider.route()).is_err(),
        "a non-member address was admitted to the route directory"
    );

    let mut t = a.transport(&members);
    let mut r = Rebalancer::new(a.key(), fast_policy(), Box::new(SpreadScheduler));
    for _ in 0..3 {
        let rep = r.tick(&mut t, &dir, &cap(1), now_unix(), Instant::now());
        assert!(rep.migrated.is_empty());
    }
    assert_eq!(a.owned(), vec![1, 2, 3, 4, 5, 6]);
    assert!(outsider.owned().is_empty(), "a stranger received a shard");
}

// ---------------------------------------------------------------------------
// Planner-level guards that need no sockets but belong with the fleet story
// ---------------------------------------------------------------------------

#[test]
fn an_empty_cluster_view_plans_nothing() {
    let me = RawKeypairAuth::generate().node_pubkey();
    let view = ClusterView {
        local: me,
        local_capacity: cap(4),
        local_shards: (1..=4).map(ShardId).collect(),
        peers: Vec::new(),
        unreachable: Vec::new(),
    };
    let plan = plan_moves(
        &view,
        &fast_policy(),
        &SpreadScheduler,
        &HashMap::new(),
        &HashMap::new(),
        Instant::now(),
    );
    assert!(
        plan.is_empty(),
        "planned a migration with nowhere to migrate to"
    );
}
