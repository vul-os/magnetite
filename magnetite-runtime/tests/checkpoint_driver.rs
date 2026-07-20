//! Does a node that was merely *configured* for durability actually write
//! anything?
//!
//! # Why this file exists, and why it is not another `fleet_durability.rs`
//!
//! `fleet_durability.rs` proves the restore path is epoch-safe. Every one of its
//! tests builds a `Checkpointer` by hand, calls `checkpoint_now` by hand, and
//! calls `attach_checkpointer` by hand. That is the right way to test a *restore*
//! — you need a checkpoint to exist before you can restore it — but it means the
//! whole suite stayed green while the production node wired **none** of that up:
//! `attach_checkpointer` had exactly one caller in the repo and it was that test.
//! The feature was inert in every real node and nothing failed.
//!
//! So the rule for this file is:
//!
//! > **No test here may call `Checkpointer::checkpoint_now`, and none may seed
//! > `ShardAuthority` by calling `claim`/`update_state` itself.**
//!
//! A checkpoint may only come into existence the way it does on a live node: the
//! game is stepped, [`ShardStateExecutor`] publishes the world, and
//! [`spawn_checkpoint_loop`] notices the cadence has elapsed and writes. Anything
//! that lets the test do the node's job for it would re-create the exact blind
//! spot this file exists to close — if the drive path is deleted, these tests go
//! red, which is the only property that matters here.
//!
//! The last test then takes the bytes that landed on disk and restores them
//! through a *separate* store handle, which is what a survivor on another
//! machine actually has: a directory, and no access to the dead node's memory.

use std::sync::Arc;
use std::time::{Duration, Instant};

use magnetite_runtime::checkpoint::{
    restore_shard, CheckpointPolicy, CheckpointStore, Checkpointer, ShardStateExecutor,
    ShardStateSink, spawn_checkpoint_loop,
};
use magnetite_runtime::fleet::ShardAuthority;
use magnetite_runtime::shard::ShardId;
use magnetite_seams::blobstore::FsBlobStore;
use magnetite_sdk::authority::{
    AuthoritativeGame, GameExecutor, MatchConfig, NativeExecutor, RejectReason, StepCtx, Tick,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

// ---------------------------------------------------------------------------
// A toy world with state that visibly changes, so a rollback is detectable
// ---------------------------------------------------------------------------

/// Counts ticks. Trivial, but the point is that the number is *different* at
/// different moments, so a restored world can be checked against the moment it
/// was actually captured rather than just "some bytes came back".
struct Counter {
    ticks: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CounterSnap {
    ticks: u64,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct CounterDelta;
#[derive(serde::Serialize)]
struct CounterView;
#[derive(serde::Serialize, serde::Deserialize)]
struct CounterCmd;

impl AuthoritativeGame for Counter {
    type Snapshot = CounterSnap;
    type Delta = CounterDelta;
    type View = CounterView;
    type Command = CounterCmd;

    fn init(_cfg: &MatchConfig) -> Self {
        Counter { ticks: 0 }
    }
    fn validate(&self, _p: PlayerId, _i: &Input, _t: Tick) -> Result<Vec<CounterCmd>, RejectReason> {
        Ok(vec![])
    }
    fn step(&mut self, _ctx: &mut StepCtx, _cmds: &[(PlayerId, CounterCmd)]) {
        self.ticks += 1;
    }
    fn snapshot(&self) -> CounterSnap {
        CounterSnap { ticks: self.ticks }
    }
    fn restore(s: &CounterSnap, _cfg: &MatchConfig) -> Self {
        Counter { ticks: s.ticks }
    }
    fn delta(&self, _s: &CounterSnap) -> CounterDelta {
        CounterDelta
    }
    fn view_for(&self, _p: PlayerId) -> CounterView {
        CounterView
    }
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "magnetite-cpdrv-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Blob files, excluding the atomic-write temporaries.
fn blob_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| !n.starts_with(".tmp-"))
                .unwrap_or(false)
        })
        .collect()
}

/// Build the durability wiring the way a node does: a store over a directory, a
/// checkpointer over that store, a sink over the node's authority, and the loop
/// that drives writes. Returns the executor the "game" is stepped through.
///
/// Note what is NOT here: no `checkpoint_now`, no `claim`. The only way state
/// reaches the authority is by stepping the returned executor.
struct Node {
    exec: Box<dyn GameExecutor>,
    authority: ShardAuthority,
    dir: std::path::PathBuf,
    _loop: std::thread::JoinHandle<()>,
}

fn boot_node(tag: &str, cadence: Duration) -> Node {
    let dir = temp_dir(tag);
    let store = CheckpointStore::new(Arc::new(FsBlobStore::new(&dir).expect("blob dir")));
    let checkpointer = Checkpointer::new(
        store,
        CheckpointPolicy {
            enabled: true,
            cadence,
        },
    );
    let authority = ShardAuthority::new();
    let sink = ShardStateSink::new(authority.clone(), ShardId::LOCAL);
    let inner = NativeExecutor::<Counter>::new(MatchConfig::auto(16));
    let exec = Box::new(ShardStateExecutor::new(Box::new(inner), sink.clone()));
    let handle = spawn_checkpoint_loop(authority.clone(), checkpointer, sink.tick_source());
    Node {
        exec,
        authority,
        dir,
        _loop: handle,
    }
}

/// Step the world, giving the checkpoint thread real time to run.
fn simulate(node: &mut Node, ticks: u64, from: u64) {
    for t in from..from + ticks {
        node.exec.step(t, &[]);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Poll until the node has written at least one blob, or give up.
fn wait_for_blob(dir: &std::path::Path, within: Duration) -> Vec<std::path::PathBuf> {
    let deadline = Instant::now() + within;
    loop {
        let f = blob_files(dir);
        if !f.is_empty() || Instant::now() > deadline {
            return f;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// **The test that would have caught the inert feature.**
///
/// Nothing in this test writes a checkpoint. It boots the durability wiring,
/// steps the game, and asserts that a file appeared on disk anyway. If the
/// checkpoint loop is removed, or the executor stops publishing into the
/// authority, or the node never claims its shard, no file appears and this
/// fails — which is exactly what the previous suite could not detect.
#[test]
fn a_running_node_writes_a_checkpoint_without_anyone_asking_it_to() {
    let mut node = boot_node("writes", Duration::from_millis(300));
    assert!(
        blob_files(&node.dir).is_empty(),
        "nothing should be written before the world is stepped"
    );

    simulate(&mut node, 60, 1);

    let files = wait_for_blob(&node.dir, Duration::from_secs(5));
    assert!(
        !files.is_empty(),
        "a running node with checkpointing configured wrote NOTHING to {} — \
         the checkpointer is attached but nothing drives it",
        node.dir.display()
    );

    // And it is a real checkpoint of a real world, not an empty placeholder:
    // the node genuinely owns its shard and the stored state is the game's.
    assert!(
        node.authority.owns(ShardId::LOCAL),
        "a node that checkpoints a shard must actually own it"
    );
    let state = node.authority.state_of(ShardId::LOCAL).expect("owned state");
    let snap: CounterSnap = serde_json::from_slice(&state).expect("state is the game's snapshot");
    assert!(
        snap.ticks > 0,
        "the checkpointed world must be one that was actually simulated, not an empty shard"
    );

    let _ = std::fs::remove_dir_all(&node.dir);
}

/// The cadence is the loss window, so it has to be real in both directions: a
/// node must not write on every tick, and it must write again once the cadence
/// has genuinely elapsed.
#[test]
fn checkpoints_follow_the_configured_cadence() {
    let mut node = boot_node("cadence", Duration::from_millis(400));
    simulate(&mut node, 40, 1);
    let first = wait_for_blob(&node.dir, Duration::from_secs(5));
    assert!(!first.is_empty(), "expected an initial checkpoint");
    assert!(
        first.len() < 20,
        "cadence ignored: {} blobs for ~40 ticks — a checkpoint per tick is not a cadence",
        first.len()
    );

    // Keep simulating past several more cadences; the world keeps changing, so
    // each new checkpoint is new content and therefore a new blob.
    simulate(&mut node, 100, 41);
    let later = blob_files(&node.dir);
    assert!(
        later.len() > first.len(),
        "checkpointing stopped after the first write ({} then {})",
        first.len(),
        later.len()
    );

    let _ = std::fs::remove_dir_all(&node.dir);
}

/// The full loop, end to end: a node runs, writes to disk, dies, and a survivor
/// that has nothing but the directory rebuilds the shard at a strictly higher
/// epoch and reports how much was lost.
///
/// The survivor's store is a *separate* `FsBlobStore` over the same path, built
/// after the node is gone. That is the real situation: the dead node's memory,
/// its `Checkpointer` and its authority table are all unavailable, and the only
/// thing that survived is bytes in a directory.
#[test]
fn a_node_dies_and_a_survivor_restores_it_at_a_higher_epoch_with_a_loss_window() {
    let mut node = boot_node("restore", Duration::from_millis(300));
    simulate(&mut node, 60, 1);
    assert!(
        !wait_for_blob(&node.dir, Duration::from_secs(5)).is_empty(),
        "no checkpoint was written, so there is nothing to restore"
    );

    // What the world looked like when it died, and at what epoch.
    let dead_epoch = node.authority.epoch_of(ShardId::LOCAL).expect("owned");
    let dir = node.dir.clone();

    // The node dies. Everything it held in memory goes with it.
    let Node { authority, .. } = node;
    drop(authority);

    // --- survivor side: a directory and nothing else --------------------
    let survivor_store =
        CheckpointStore::new(Arc::new(FsBlobStore::new(&dir).expect("reopen blob dir")));
    // The survivor learns the ref the way it does on the wire — but we must not
    // hand it one the dead node's in-memory Checkpointer produced, because that
    // object no longer exists. Rebuild it from what is on disk.
    let cp_ref = newest_ref_on_disk(&dir, &survivor_store);

    let survivor = ShardAuthority::new();
    let previous_owner = magnetite_seams::identity::RawKeypairAuth::generate().node_pubkey();
    let rec = restore_shard(
        &survivor,
        &survivor_store,
        &cp_ref,
        ShardId::LOCAL,
        previous_owner,
        now_unix() + 7,
    )
    .expect("a checkpoint that is on disk and intact must restore");

    // The epoch fence: strictly above the dead owner's epoch.
    assert!(
        rec.epoch > rec.checkpoint_epoch,
        "restore must out-rank the dead owner: {} vs {}",
        rec.epoch,
        rec.checkpoint_epoch
    );
    assert!(
        rec.epoch > dead_epoch,
        "restore at epoch {} does not fence the dead owner at epoch {dead_epoch}",
        rec.epoch
    );
    assert!(survivor.owns(ShardId::LOCAL));

    // The loss window is real and reported, and the restored world is the one
    // that was captured — not an empty shard wearing the right id.
    assert!(
        rec.loss_window_secs >= 7,
        "the reported loss window ({}s) must cover the time since the checkpoint",
        rec.loss_window_secs
    );
    let restored = survivor.state_of(ShardId::LOCAL).expect("restored state");
    let snap: CounterSnap = serde_json::from_slice(&restored).expect("restored a real world");
    assert!(
        snap.ticks > 0,
        "restored an EMPTY world — an empty shard under a real shard id is the one \
         outcome that looks like success in every log line and is not"
    );

    // And it never claims to be loss-free.
    let msg = rec.to_string();
    assert!(msg.contains("WAS LOST"), "{msg}");
    assert!(!msg.to_lowercase().contains("no data loss"), "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Reconstruct the newest checkpoint ref from the blobs a dead node left behind.
///
/// A survivor normally receives this over the authenticated status exchange;
/// here the announcing node is gone, so we read the directory. Each blob is
/// fetched and verified through the ordinary `get_verified` path, so a corrupt
/// file cannot become a ref.
fn newest_ref_on_disk(
    dir: &std::path::Path,
    store: &CheckpointStore,
) -> magnetite_runtime::checkpoint::CheckpointRef {
    let mut best: Option<magnetite_runtime::checkpoint::CheckpointRef> = None;
    for path in blob_files(dir) {
        let Some(hex) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Ok(hash) = magnetite_seams::blobstore::Hash::from_hex(hex) else {
            continue;
        };
        let id = magnetite_runtime::checkpoint::CheckpointId(hash);
        let Ok(cp) = store.get_verified(id, ShardId::LOCAL) else {
            continue;
        };
        let r = cp.to_ref();
        if best.map(|b| (r.epoch, r.tick) > (b.epoch, b.tick)).unwrap_or(true) {
            best = Some(r);
        }
    }
    best.expect("the dead node left at least one verifiable checkpoint on disk")
}
