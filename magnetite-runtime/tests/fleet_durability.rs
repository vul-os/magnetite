//! Durability driven across **real nodes over real sockets** — and, mostly, the
//! cases where it must refuse to help.
//!
//! `checkpoint.rs`'s unit tests prove the record format and the verify rules.
//! These prove the part that a pure function cannot: that a *death observed
//! over a socket* turns into a restore with the same bytes and a stated loss
//! window, that a returning owner is fenced rather than tolerated, and that
//! every doubtful case lands back on the honest [`ShardLoss`] instead of an
//! empty shard wearing a real shard id.
//!
//! | Test | Property |
//! |---|---|
//! | `a_dead_nodes_shard_is_restored_with_state_hash_continuity` | the restored world is **byte-identical** to the checkpointed one, and the loss window is reported |
//! | `a_returning_zombie_owner_is_fenced_out` | no split-brain: the lower epoch loses, and loses actively |
//! | `a_slow_owner_is_never_restored_from_under` | backed-off ≠ dead |
//! | `a_corrupt_checkpoint_falls_back_to_shard_loss` | unverifiable state is a loss, not a restore |
//! | `a_lying_peer_cannot_retarget_a_checkpoint_at_another_shard` | shard binding is inside the hashed content |
//! | `recovery_disabled_behaves_exactly_as_before` | the durability-off path is unchanged |

use std::sync::Arc;
use std::time::{Duration, Instant};

use magnetite_runtime::checkpoint::{
    CheckpointPolicy, CheckpointRef, CheckpointStore, Checkpointer, RecoveryPolicy,
};
use magnetite_runtime::cluster::{ClusterMembership, RouteDirectory};
use magnetite_runtime::fleet::{FleetNode, NetworkHandoffTransport, PeerRoute};
use magnetite_runtime::rebalance::{RebalancePolicy, Rebalancer};
use magnetite_runtime::shard::ShardId;
use magnetite_sdk::scaling::SpreadScheduler;
use magnetite_seams::blobstore::{BlobStore, Hash, LocalBlobStore};
use magnetite_seams::discovery::Capacity;
use magnetite_seams::identity::{PubKey, RawKeypairAuth};

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

struct Node {
    id: Arc<RawKeypairAuth>,
    node: FleetNode,
}

impl Node {
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

    fn transport(&self, members: &[PubKey]) -> NetworkHandoffTransport {
        self.node
            .transport()
            .with_membership(ClusterMembership::from_keys(members.iter().copied()))
            .with_timeout(Duration::from_millis(600))
    }
}

fn directory(members: &[PubKey], routes: &[PeerRoute]) -> RouteDirectory {
    let mut dir = RouteDirectory::new(ClusterMembership::from_keys(members.iter().copied()));
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

/// A recovery policy that treats the very first failed probe as death, so a
/// test does not have to wait out a real grace period. Production defaults are
/// asserted separately in `checkpoint.rs`.
fn eager_recovery() -> RecoveryPolicy {
    RecoveryPolicy {
        enabled: true,
        min_failures: 1,
        grace: Duration::ZERO,
    }
}

/// Two nodes and a blob store both of them can read — the cross-machine setup
/// that makes recovery possible at all.
struct Cluster {
    a: Node,
    b: Option<Node>,
    members: Vec<PubKey>,
    blobs: Arc<LocalBlobStore>,
    b_key: PubKey,
    b_route: PeerRoute,
}

impl Cluster {
    /// B owns `shard` holding `state`, and has checkpointed it.
    fn with_checkpointed_shard(shard: u32, state: &[u8]) -> (Self, CheckpointRef) {
        let ids: Vec<Arc<RawKeypairAuth>> =
            (0..2).map(|_| Arc::new(RawKeypairAuth::generate())).collect();
        let members: Vec<PubKey> = ids.iter().map(|i| i.node_pubkey()).collect();
        let a = Node::spawn_as(Arc::clone(&ids[0]), 4, &members);
        let b = Node::spawn_as(Arc::clone(&ids[1]), 4, &members);

        let blobs = Arc::new(LocalBlobStore::new());
        let store = CheckpointStore::new(blobs.clone());
        let cp = Checkpointer::new(store, CheckpointPolicy::default());

        b.node.authority().claim(ShardId(shard), state.to_vec());
        let r = cp
            .checkpoint_now(
                &b.node.authority(),
                ShardId(shard),
                1_234,
                now_unix() - 5,
                Instant::now(),
            )
            .expect("owner checkpoints its own shard");
        b.node.attach_checkpointer(cp);

        let b_key = b.key();
        let b_route = b.route();
        (
            Self {
                a,
                b: Some(b),
                members,
                blobs,
                b_key,
                b_route,
            },
            r,
        )
    }

    fn store(&self) -> CheckpointStore {
        CheckpointStore::new(self.blobs.clone())
    }

    /// Kill B's listener. Its shard's world is now only in the blob store.
    fn kill_b(&mut self) {
        if let Some(mut b) = self.b.take() {
            b.node.shutdown();
        }
    }
}

// ---------------------------------------------------------------------------
// The happy path — stated honestly
// ---------------------------------------------------------------------------

#[test]
fn a_dead_nodes_shard_is_restored_with_state_hash_continuity() {
    let state = b"world-at-tick-1234".to_vec();
    let (mut c, cp_ref) = Cluster::with_checkpointed_shard(5, &state);

    let mut t = c.a.transport(&c.members);
    let dir = directory(&c.members, &[c.b_route.clone()]);
    let mut r = Rebalancer::new(c.a.key(), fast_policy(), Box::new(SpreadScheduler))
        .with_recovery(c.store(), eager_recovery());
    assert!(r.recovery_enabled());

    // While B is alive, A learns where the durable copy is — a dead peer cannot
    // tell anyone, so this cache is the whole reason recovery is possible.
    let first = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    assert!(first.lost.is_empty() && first.recovered.is_empty());
    assert_eq!(
        r.known_checkpoint(ShardId(5)).map(|k| k.id),
        Some(cp_ref.id),
        "A never cached B's checkpoint announcement"
    );

    c.kill_b();

    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    assert_eq!(rep.recovered.len(), 1, "shard 5 was not restored: {rep:?}");
    assert!(
        rep.lost.is_empty(),
        "a shard was both restored and mourned: {:?}",
        rep.lost
    );

    let rec = &rep.recovered[0];
    assert_eq!(rec.shard, ShardId(5));
    assert_eq!(rec.previous_owner, c.b_key);
    assert_eq!(rec.checkpoint, cp_ref.id);
    assert_eq!(rec.checkpoint_tick, 1_234, "did not report the rolled-back tick");

    // Continuity: the restored world is the checkpointed world, byte for byte.
    assert_eq!(rec.state_hash, Hash::of(&state));
    assert_eq!(
        c.a.node.authority().state_of(ShardId(5)).as_deref(),
        Some(state.as_slice()),
        "the restored shard does not hold the checkpointed state"
    );
    assert!(c.a.node.authority().owns(ShardId(5)));

    // The epoch strictly out-ranks the dead owner's, which is what makes the
    // fence able to settle a race at all.
    assert!(
        rec.epoch > rec.checkpoint_epoch,
        "restore did not take a higher epoch: {} <= {}",
        rec.epoch,
        rec.checkpoint_epoch
    );

    // And it is never sold as loss-free.
    assert!(rec.loss_window_secs >= 5, "loss window under-reported");
    let msg = rec.to_string();
    assert!(msg.contains("SIMULATION WAS LOST"), "recovery hedges: {msg}");
    assert!(msg.contains("rollback"), "recovery reads as resurrection: {msg}");
}

// ---------------------------------------------------------------------------
// Split-brain
// ---------------------------------------------------------------------------

#[test]
fn a_returning_zombie_owner_is_fenced_out() {
    let state = b"contested-world".to_vec();
    let (mut c, _) = Cluster::with_checkpointed_shard(5, &state);

    let mut t = c.a.transport(&c.members);
    let mut r = Rebalancer::new(c.a.key(), fast_policy(), Box::new(SpreadScheduler))
        .with_recovery(c.store(), eager_recovery());

    let dir = directory(&c.members, &[c.b_route.clone()]);
    r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    c.kill_b();
    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    let restored_epoch = rep.recovered[0].epoch;
    assert!(c.a.node.authority().owns(ShardId(5)));

    // B comes back believing it still owns shard 5, at the epoch it died with.
    let zombie = Node::spawn_as(
        Arc::new(RawKeypairAuth::generate()),
        4,
        &[c.a.key(), c.b_key],
    );
    // A zombie with a *different* key is a different node; what we need is the
    // stale CLAIM, so give the returning node the old, lower epoch on shard 5.
    zombie.node.authority().claim(ShardId(5), state.clone());
    let zombie_epoch = zombie
        .node
        .authority()
        .epoch_of(ShardId(5))
        .expect("zombie claims the shard");
    assert!(
        zombie_epoch < restored_epoch,
        "test setup: the zombie must hold the LOWER epoch ({zombie_epoch} vs {restored_epoch})"
    );

    let members = vec![c.a.key(), zombie.key()];
    let mut t = c.a.transport(&members);
    let dir = directory(&members, &[zombie.route()]);
    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());

    assert!(
        rep.fenced.iter().any(|(s, _, e, dropped)| s.0 == 5
            && *e == zombie_epoch
            && *dropped),
        "the stale claimant was not fenced: {:?}",
        rep.fenced
    );
    assert!(
        !zombie.node.authority().owns(ShardId(5)),
        "SPLIT BRAIN: the returning owner still holds shard 5"
    );
    assert!(
        c.a.node.authority().owns(ShardId(5)),
        "the restorer lost the shard it legitimately holds at the higher epoch"
    );
}

// ---------------------------------------------------------------------------
// The refusals — the point of the whole exercise
// ---------------------------------------------------------------------------

#[test]
fn a_slow_owner_is_never_restored_from_under() {
    let state = b"slow-but-alive".to_vec();
    let (mut c, _) = Cluster::with_checkpointed_shard(5, &state);

    let mut t = c.a.transport(&c.members);
    let dir = directory(&c.members, &[c.b_route.clone()]);
    // Production-shaped: three consecutive failures AND a minute of them.
    let mut r = Rebalancer::new(c.a.key(), fast_policy(), Box::new(SpreadScheduler))
        .with_recovery(c.store(), RecoveryPolicy::default());

    r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    c.kill_b();

    // Several ticks of unreachability — still inside the grace window.
    for _ in 0..4 {
        let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
        assert!(
            rep.recovered.is_empty(),
            "restored a shard from under an owner that is merely unreachable"
        );
        for l in &rep.lost {
            let why = l.restore_refused.clone().unwrap_or_default();
            assert!(
                why.contains("owner is not gone"),
                "the refusal was not the liveness gate: {why}"
            );
        }
    }
    assert!(
        !c.a.node.authority().owns(ShardId(5)),
        "a slow owner's shard was taken over anyway"
    );
}

/// A blob store that hands back bytes other than the ones stored. Stands in for
/// corruption, a hostile store, and a bit-flip alike — the restore path may not
/// tell them apart, it may only refuse.
struct LyingStore(Arc<LocalBlobStore>);

#[async_trait::async_trait]
impl BlobStore for LyingStore {
    async fn put(&self, bytes: &[u8]) -> Hash {
        self.0.put(bytes).await
    }
    async fn get(&self, hash: &Hash) -> Option<Vec<u8>> {
        let mut b = self.0.get(hash).await?;
        if let Some(x) = b.last_mut() {
            *x ^= 0xff;
        }
        Some(b)
    }
    async fn has(&self, hash: &Hash) -> bool {
        self.0.has(hash).await
    }
}

#[test]
fn a_corrupt_checkpoint_falls_back_to_shard_loss() {
    let state = b"world-that-will-not-verify".to_vec();
    let (mut c, _) = Cluster::with_checkpointed_shard(5, &state);

    let lying = CheckpointStore::new(Arc::new(LyingStore(c.blobs.clone())));
    let mut t = c.a.transport(&c.members);
    let dir = directory(&c.members, &[c.b_route.clone()]);
    let mut r = Rebalancer::new(c.a.key(), fast_policy(), Box::new(SpreadScheduler))
        .with_recovery(lying, eager_recovery());

    r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    c.kill_b();
    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());

    assert!(
        rep.recovered.is_empty(),
        "restored a shard from a checkpoint that does not verify"
    );
    let loss = rep
        .lost
        .iter()
        .find(|l| l.shard.0 == 5)
        .expect("shard 5 must be reported LOST when its checkpoint is unusable");
    let why = loss.restore_refused.clone().unwrap_or_default();
    assert!(
        why.contains("CHECKPOINT REJECTED") || why.contains("did not decode"),
        "the loss does not say the checkpoint was rejected: {why}"
    );
    assert!(
        !c.a.node.authority().owns(ShardId(5)),
        "an EMPTY shard 5 was started under a real shard id — the exact lie this must never tell"
    );
}

#[test]
fn a_lying_peer_cannot_retarget_a_checkpoint_at_another_shard() {
    // B checkpoints shard 7. A is told (falsely) that this blob is shard 5's.
    let state = b"shard-sevens-world".to_vec();
    let (mut c, cp7) = Cluster::with_checkpointed_shard(7, &state);

    let mut t = c.a.transport(&c.members);
    let dir = directory(&c.members, &[c.b_route.clone()]);
    let mut r = Rebalancer::new(c.a.key(), fast_policy(), Box::new(SpreadScheduler))
        .with_recovery(c.store(), eager_recovery());
    r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());

    // The lie: same content address, different claimed shard. It is only a
    // pointer; the binding that matters is inside the hashed content.
    let store = c.store();
    let cp = store
        .get_verified(cp7.id, ShardId(7))
        .expect("shard 7's checkpoint is genuine");
    assert_eq!(cp.shard, 7);
    let err = store
        .get_verified(cp7.id, ShardId(5))
        .expect_err("a shard-7 checkpoint must never verify as shard 5");
    assert!(
        err.to_string().contains("CHECKPOINT REJECTED") && err.to_string().contains("not shard"),
        "wrong refusal for a retargeted checkpoint: {err}"
    );

    c.kill_b();
    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    // Shard 7 itself restores normally; nothing invented a shard 5.
    assert!(rep.recovered.iter().all(|x| x.shard.0 == 7));
    assert!(!c.a.node.authority().owns(ShardId(5)));
}

#[test]
fn recovery_disabled_behaves_exactly_as_before() {
    let state = b"undurable".to_vec();
    let (mut c, _) = Cluster::with_checkpointed_shard(5, &state);

    let mut t = c.a.transport(&c.members);
    let dir = directory(&c.members, &[c.b_route.clone()]);
    // Explicitly off — and off must mean off, even though a perfectly good
    // checkpoint is sitting in a store this node can read.
    let mut r = Rebalancer::new(c.a.key(), fast_policy(), Box::new(SpreadScheduler)).with_recovery(
        c.store(),
        RecoveryPolicy {
            enabled: false,
            ..RecoveryPolicy::default()
        },
    );
    assert!(!r.recovery_enabled());

    r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());
    c.kill_b();
    let rep = r.tick(&mut t, &dir, &cap(4), now_unix(), Instant::now());

    assert!(rep.recovered.is_empty());
    let loss = rep.lost.iter().find(|l| l.shard.0 == 5).expect("lost");
    assert_eq!(loss.restore_refused, None);
    assert!(loss.to_string().contains("STATE LOST"));
    assert!(!c.a.node.authority().owns(ShardId(5)));
}
