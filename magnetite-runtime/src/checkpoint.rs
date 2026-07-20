//! Durable shard state — checkpoints to the [`BlobStore`] seam, and the
//! **epoch-safe** restore that turns a dead node from a total loss into a
//! bounded one.
//!
//! # What this buys you, stated honestly
//!
//! Before this module, [`crate::rebalance`] could only report [`ShardLoss`]: a
//! node died, its shards' in-memory worlds died with it, and starting an empty
//! shard under the same id would have been a lie dressed as a recovery.
//!
//! This module does **not** make that go away. It makes it *bounded*:
//!
//! > A shard is checkpointed every [`CheckpointPolicy::cadence`]. When its owner
//! > dies, a survivor can rebuild it from the newest checkpoint it knows about.
//! > **Everything simulated between that checkpoint and the death is gone.**
//!
//! That is up to one full cadence of lost world — 15 seconds at the default.
//! Players are rolled back, not preserved. Nothing in this crate says "no data
//! loss", because it would not be true; every recovery reports the actual
//! window ([`ShardRecovery::loss_window_secs`]) and the tick it rolled back to.
//!
//! ## Why 15 seconds is the default cadence
//!
//! The cadence is a straight trade of *bytes written* against *seconds lost*.
//! A shard's serialized state is the whole simulated world for that cell, so a
//! checkpoint is not free: at 1 s you pay a serialization + a blob write sixty
//! times a minute per shard, and the blob store becomes the bottleneck long
//! before the tick loop does. At 60 s you lose a minute of play, which for most
//! games is past the point where "restored" and "restarted" feel different to a
//! player.
//!
//! 15 s sits where the cost is negligible (four writes per shard per minute)
//! and the loss is still recognisably a *rollback* — a player loses position and
//! recent score, not their session. Change it with `--checkpoint-interval`;
//! the honest framing does not change, only the number.
//!
//! # The format
//!
//! One blob per checkpoint, and the blob's BLAKE3 content address **is** the
//! checkpoint id. Crucially the blob contains the binding metadata as well as
//! the bytes:
//!
//! ```text
//! Checkpoint {
//!   version, shard, epoch, tick, taken_at_unix,
//!   state_hash,      // BLAKE3 of `state`
//!   state,           // the serialized shard world
//! }
//! ```
//!
//! Putting `shard` and `epoch` *inside* the hashed content is what makes
//! "a checkpoint for shard S restores shard T" impossible rather than merely
//! checked: to retarget a checkpoint you would have to change the content,
//! which changes the address, which no longer matches what was announced.
//! `state_hash` is a second, redundant binding so that state corruption is
//! distinguishable from metadata corruption in the logs.
//!
//! # Split-brain: the part that must not be got wrong
//!
//! A restore creates a second node claiming a shard. If the original owner was
//! merely slow or partitioned — not dead — that is split-brain, and split-brain
//! is strictly worse than the loss this module exists to fix. Two mechanisms,
//! and neither is new machinery:
//!
//! **1. Do not restore something that is not dead.** [`RecoveryPolicy`] gates a
//! restore behind the *existing* liveness signals: the owner's discovery lease
//! must have lapsed, or it must have failed [`RecoveryPolicy::min_failures`]
//! consecutive probes **and** been failing for at least
//! [`RecoveryPolicy::grace`]. A peer in its first backoff is not dead, it is
//! backed off, and nothing is restored from under it.
//!
//! **2. When it comes back, the epoch fence settles it — deterministically.**
//! A restore claims the shard at a **strictly higher epoch** than both the
//! checkpoint and the restorer's own high-water mark
//! ([`ShardAuthority::claim_at_least`]). The returning owner still believes it
//! owns the shard at its old, lower epoch. That belief cannot survive contact:
//!
//! - The zombie tries to hand the shard anywhere: the target's high-water mark
//!   is already at or above the restored epoch, and [`ShardAuthority::stage`]
//!   refuses it as stale. Authority does not move.
//! - The restored owner sees, in the ordinary authenticated
//!   [`crate::fleet::Frame::Status`] reply, that a peer claims a shard it owns
//!   at a **lower** epoch, and sends a signed [`crate::fleet::Frame::Fence`].
//!   The zombie drops the shard. Same rule, applied actively rather than waiting
//!   for the zombie to try something.
//! - The rule is symmetric and total-ordered on `(shard, epoch)`, so if the
//!   *restorer* is the one holding the lower epoch, the restorer yields. Two
//!   nodes can never both decide they win, because they are comparing the same
//!   two numbers.
//!
//! There is no second notion of authority anywhere in here. `(shard, epoch)` is
//! it, exactly as in [`crate::fleet`].
//!
//! # What is still unrecoverable
//!
//! - **The last cadence of simulation.** By construction. See above.
//! - **A shard nobody ever checkpointed**, or whose checkpoint the survivor
//!   never heard about (it never successfully probed the owner). Falls back to
//!   [`ShardLoss`], unchanged.
//! - **Anything, if the blob store is not reachable by the survivor.**
//!   [`LocalBlobStore`] is in-memory and dies with its node.
//!   [`FsBlobStore`] — what `--checkpoint-dir` builds — outlives the *process*,
//!   but a node-local directory does not outlive the *machine*, so a survivor on
//!   another box still cannot read it. Cross-machine restore needs
//!   `--checkpoint-dir` pointed at storage both nodes can reach (a shared
//!   mount). Every one of these cases degrades to exactly today's behaviour:
//!   honest [`ShardLoss`].
//! - **Player connections.** A restored shard is a restored *world*; clients
//!   still reconnect.
//!
//! [`BlobStore`]: magnetite_seams::blobstore::BlobStore
//! [`ShardLoss`]: crate::rebalance::ShardLoss
//! [`LocalBlobStore`]: magnetite_seams::blobstore::LocalBlobStore
//! [`FsBlobStore`]: magnetite_seams::blobstore::FsBlobStore

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use magnetite_seams::blobstore::{BlobStore, Hash};

use crate::fleet::ShardAuthority;
use crate::shard::ShardId;

/// Checkpoint envelope version. Bumped if the layout ever changes; an unknown
/// version is **refused**, never guessed at.
pub const CHECKPOINT_VERSION: u16 = 1;

/// Default seconds between checkpoints. See the module docs for why 15.
pub const DEFAULT_CADENCE_SECS: u64 = 15;

// ---------------------------------------------------------------------------
// The record
// ---------------------------------------------------------------------------

/// One durable snapshot of a shard, exactly as it is stored in the blob store.
///
/// The content address of this record's serialized bytes is the checkpoint id.
/// Every field that a restore must trust — which shard, which epoch, which tick
/// — is inside the hashed content, so none of them can be swapped without
/// invalidating the address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Envelope version ([`CHECKPOINT_VERSION`]).
    pub version: u16,
    /// The shard this state belongs to. A restore of any other shard is refused.
    pub shard: u32,
    /// The epoch the owner held when it took this checkpoint. A restore always
    /// claims **above** this.
    pub epoch: u64,
    /// Simulation tick at which the state was captured — the tick a restore
    /// rolls the world back to.
    pub tick: u64,
    /// Wall-clock seconds since the Unix epoch when it was taken. Advisory: it
    /// is how the loss window is reported, not how anything is authorized.
    pub taken_at_unix: u64,
    /// BLAKE3 of [`Self::state`]. Redundant with the content address by design.
    pub state_hash: Hash,
    /// The serialized shard world.
    #[serde(with = "hex_bytes")]
    pub state: Vec<u8>,
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(v))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

impl Checkpoint {
    /// Build a checkpoint record over some shard state.
    pub fn new(shard: ShardId, epoch: u64, tick: u64, taken_at_unix: u64, state: Vec<u8>) -> Self {
        Self {
            version: CHECKPOINT_VERSION,
            shard: shard.0,
            epoch,
            tick,
            taken_at_unix,
            state_hash: Hash::of(&state),
            state,
        }
    }

    /// Serialize to the exact bytes that get content-addressed.
    pub fn to_bytes(&self) -> Vec<u8> {
        // `serde_json` cannot fail on this shape (no maps with non-string keys,
        // no non-finite floats), but we never `unwrap` on a fallible encode in a
        // durability path — an empty vec would hash to a stable, obviously-wrong
        // id that the verify step below rejects.
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The content address these bytes will have.
    pub fn id(&self) -> CheckpointId {
        CheckpointId(Hash::of(&self.to_bytes()))
    }

    /// The announcement a peer needs to find and trust this checkpoint later.
    pub fn to_ref(&self) -> CheckpointRef {
        CheckpointRef {
            shard: self.shard,
            epoch: self.epoch,
            tick: self.tick,
            taken_at_unix: self.taken_at_unix,
            id: self.id(),
        }
    }

    /// Verify this record against the id it was fetched under, and against the
    /// shard it is being restored into. **Fails closed on every mismatch.**
    ///
    /// This is deliberately paranoid about the blob store: [`BlobStore`] is a
    /// seam, and a seam is somebody else's code. We re-hash what came back
    /// rather than trusting the store to have honoured content addressing.
    pub fn verify(&self, want_id: CheckpointId, want_shard: ShardId) -> Result<(), RestoreError> {
        if self.version != CHECKPOINT_VERSION {
            return Err(RestoreError::Version(self.version));
        }
        let got = self.id();
        if got != want_id {
            return Err(RestoreError::Tampered {
                want: want_id,
                got,
            });
        }
        if Hash::of(&self.state) != self.state_hash {
            return Err(RestoreError::StateHashMismatch {
                declared: self.state_hash,
                computed: Hash::of(&self.state),
            });
        }
        if self.shard != want_shard.0 {
            return Err(RestoreError::WrongShard {
                want: want_shard,
                got: ShardId(self.shard),
            });
        }
        Ok(())
    }
}

/// The content address of a serialized [`Checkpoint`] — its identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub Hash);

impl CheckpointId {
    /// Lowercase hex.
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_hex())
    }
}

/// A peer's advertisement that a checkpoint exists.
///
/// Travels inside the already-authenticated status exchange, so its authorship
/// is as proven as any other post-handshake frame. It is nonetheless only a
/// *pointer*: every field here is re-derived from the fetched content before a
/// restore acts on it, so a peer that lies in this struct can at worst make a
/// survivor fetch a blob that then fails [`Checkpoint::verify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRef {
    /// Shard the checkpoint covers.
    pub shard: u32,
    /// Epoch at which it was taken.
    pub epoch: u64,
    /// Tick at which it was taken.
    pub tick: u64,
    /// When it was taken (Unix seconds) — used to report the loss window.
    pub taken_at_unix: u64,
    /// Content address of the [`Checkpoint`] record.
    pub id: CheckpointId,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a restore was refused. **Every variant means "fall back to
/// [`ShardLoss`]"** — none of them ever results in an empty shard being started
/// under a real shard id.
///
/// [`ShardLoss`]: crate::rebalance::ShardLoss
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreError {
    /// No checkpoint is known for this shard at all.
    NoCheckpoint(ShardId),
    /// The blob store does not have (or would not return) the checkpoint.
    Missing(CheckpointId),
    /// The bytes did not decode as a checkpoint record.
    Undecodable(String),
    /// Unknown envelope version — refused rather than guessed at.
    Version(u16),
    /// **Content hash mismatch**: the bytes are not the checkpoint that was
    /// announced. Corruption or tampering; either way, not restorable.
    Tampered {
        /// Address requested.
        want: CheckpointId,
        /// Address the returned bytes actually have.
        got: CheckpointId,
    },
    /// The record's own `state_hash` does not match its state bytes.
    StateHashMismatch {
        /// What the record claims.
        declared: Hash,
        /// What the bytes hash to.
        computed: Hash,
    },
    /// A checkpoint for one shard was offered as another's. Refused.
    WrongShard {
        /// Shard being restored.
        want: ShardId,
        /// Shard the checkpoint actually covers.
        got: ShardId,
    },
    /// The restore would not have taken a strictly higher epoch, so it could
    /// not fence the old owner. Refused — this is the split-brain guard.
    NotHigherEpoch {
        /// Epoch the restore would have claimed.
        proposed: u64,
        /// The local high-water mark it failed to beat.
        high_water: u64,
    },
    /// The owner is not dead enough yet: still inside grace / under the failure
    /// threshold. Not an error condition, a deliberate refusal.
    OwnerNotDead {
        /// Consecutive probe failures so far.
        failures: u32,
        /// How much of the grace period remains.
        grace_remaining: Duration,
    },
}

impl std::fmt::Display for RestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCheckpoint(s) => write!(f, "no checkpoint is known for shard {s}"),
            Self::Missing(id) => write!(f, "blob store has no checkpoint {id}"),
            Self::Undecodable(e) => write!(f, "checkpoint did not decode: {e}"),
            Self::Version(v) => write!(
                f,
                "checkpoint envelope version {v} is not {CHECKPOINT_VERSION} — refusing to guess"
            ),
            Self::Tampered { want, got } => write!(
                f,
                "CHECKPOINT REJECTED: content hash mismatch — asked for {want}, bytes hash to {got}"
            ),
            Self::StateHashMismatch { declared, computed } => write!(
                f,
                "CHECKPOINT REJECTED: state hash mismatch — record declares {}, bytes hash to {}",
                declared.to_hex(),
                computed.to_hex()
            ),
            Self::WrongShard { want, got } => write!(
                f,
                "CHECKPOINT REJECTED: this is shard {got}'s checkpoint, not shard {want}'s"
            ),
            Self::NotHigherEpoch {
                proposed,
                high_water,
            } => write!(
                f,
                "restore at epoch {proposed} would not exceed high-water {high_water}; \
                 refusing, because a restore that cannot out-rank the old owner is split-brain"
            ),
            Self::OwnerNotDead {
                failures,
                grace_remaining,
            } => write!(
                f,
                "owner is not gone: {failures} consecutive failures, {}s of grace remaining — \
                 a slow or backed-off owner is NOT restored from under",
                grace_remaining.as_secs()
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// How often state is made durable, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointPolicy {
    /// Off by default at the type level? No — the *node* defaults to a
    /// node-local store, which is safe and useless across machines. This flag
    /// is the operator's explicit on/off.
    pub enabled: bool,
    /// Seconds between checkpoints of a given shard. This is the **loss
    /// window**: up to one cadence of simulation is lost on a death.
    pub cadence: Duration,
}

impl Default for CheckpointPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            cadence: Duration::from_secs(DEFAULT_CADENCE_SECS),
        }
    }
}

/// When a survivor is allowed to conclude an owner is dead and rebuild its
/// shards. Every knob here exists to stop a *slow* owner being treated as a
/// *dead* one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPolicy {
    /// Master switch. Off ⇒ the death path behaves exactly as it did before
    /// this module existed: honest [`ShardLoss`], no replacement shard.
    ///
    /// [`ShardLoss`]: crate::rebalance::ShardLoss
    pub enabled: bool,
    /// Consecutive failed probes before an owner may be declared dead. One
    /// failure is a packet loss; three across three ticks is a pattern.
    pub min_failures: u32,
    /// Minimum wall-clock time the owner must have been continuously failing.
    /// Guards the case where a fast rebalance interval racks up `min_failures`
    /// inside a few seconds of ordinary network trouble.
    pub grace: Duration,
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            min_failures: 3,
            // Comfortably longer than one default rebalance interval (30s) is
            // overkill; 60s is two intervals of *continuous* failure, which no
            // transient reaches.
            grace: Duration::from_secs(60),
        }
    }
}

// ---------------------------------------------------------------------------
// The store adapter
// ---------------------------------------------------------------------------

/// A blocking view of the [`BlobStore`] seam, for the synchronous fleet paths.
///
/// This is **not** a parallel storage abstraction — it stores nothing itself and
/// owns no format. It exists only because [`BlobStore`] is `async` while
/// [`crate::fleet`] and [`crate::rebalance`] are deliberately synchronous
/// (`std::net::TcpStream`, a plain reconcile thread). Each call runs the future
/// to completion on a scoped thread with a private current-thread runtime, so it
/// is safe to call from inside or outside an async context. Checkpoints happen
/// on a cadence measured in seconds, so a thread per operation is not a cost
/// worth engineering away.
pub struct CheckpointStore {
    inner: Arc<dyn BlobStore + Send + Sync>,
}

impl std::fmt::Debug for CheckpointStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckpointStore").finish_non_exhaustive()
    }
}

impl Clone for CheckpointStore {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl CheckpointStore {
    /// Wrap any [`BlobStore`].
    pub fn new(store: Arc<dyn BlobStore + Send + Sync>) -> Self {
        Self { inner: store }
    }

    fn block_on<T: Send>(f: impl std::future::Future<Output = T> + Send) -> T {
        std::thread::scope(|s| {
            s.spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("current-thread runtime")
                    .block_on(f)
            })
            .join()
            .expect("blobstore worker thread panicked")
        })
    }

    /// Store a checkpoint; the returned id is its content address.
    pub fn put(&self, cp: &Checkpoint) -> CheckpointId {
        let bytes = cp.to_bytes();
        let id = CheckpointId(Self::block_on(self.inner.put(&bytes)));
        debug_assert_eq!(id, cp.id(), "blob store returned a non-content address");
        id
    }

    /// Fetch and **fully verify** a checkpoint for `shard`.
    ///
    /// Returns `Err` on anything at all: missing, undecodable, wrong version,
    /// wrong content hash, wrong internal state hash, or a checkpoint belonging
    /// to a different shard.
    pub fn get_verified(
        &self,
        id: CheckpointId,
        shard: ShardId,
    ) -> Result<Checkpoint, RestoreError> {
        let bytes = Self::block_on(self.inner.get(&id.0)).ok_or(RestoreError::Missing(id))?;
        // Verify the address BEFORE parsing: a hash check over raw bytes cannot
        // be confused by a hostile parse.
        let got = CheckpointId(Hash::of(&bytes));
        if got != id {
            return Err(RestoreError::Tampered { want: id, got });
        }
        let cp: Checkpoint = serde_json::from_slice(&bytes)
            .map_err(|e| RestoreError::Undecodable(e.to_string()))?;
        cp.verify(id, shard)?;
        Ok(cp)
    }
}

// ---------------------------------------------------------------------------
// Taking checkpoints
// ---------------------------------------------------------------------------

/// Drives periodic checkpointing for the shards a node owns, and publishes the
/// resulting [`CheckpointRef`]s so peers learn where the durable copies are.
///
/// Cloneable: the tick loop takes checkpoints through one handle while the
/// fleet listener reads [`Self::refs`] to answer status queries.
#[derive(Clone)]
pub struct Checkpointer {
    policy: CheckpointPolicy,
    store: CheckpointStore,
    /// shard -> newest ref we have written.
    latest: Arc<Mutex<HashMap<u32, CheckpointRef>>>,
    /// shard -> when we last wrote one.
    last_run: Arc<Mutex<HashMap<u32, Instant>>>,
}

impl std::fmt::Debug for Checkpointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkpointer")
            .field("policy", &self.policy)
            .field("shards", &self.latest.lock().map(|l| l.len()).unwrap_or(0))
            .finish()
    }
}

impl Checkpointer {
    /// Build a checkpointer over a blob store.
    pub fn new(store: CheckpointStore, policy: CheckpointPolicy) -> Self {
        Self {
            policy,
            store,
            latest: Arc::new(Mutex::new(HashMap::new())),
            last_run: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The policy in force.
    pub fn policy(&self) -> &CheckpointPolicy {
        &self.policy
    }

    /// The underlying store.
    pub fn store(&self) -> &CheckpointStore {
        &self.store
    }

    /// Newest checkpoint refs for every shard this node has checkpointed —
    /// exactly what gets announced to peers.
    pub fn refs(&self) -> Vec<CheckpointRef> {
        let g = self.latest.lock().unwrap_or_else(|p| p.into_inner());
        let mut v: Vec<CheckpointRef> = g.values().copied().collect();
        v.sort_by_key(|r| (r.shard, r.epoch));
        v
    }

    /// The newest ref for one shard, if any.
    pub fn latest_for(&self, shard: ShardId) -> Option<CheckpointRef> {
        self.latest
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&shard.0)
            .copied()
    }

    /// Whether `shard` is due a checkpoint at `now`.
    pub fn is_due(&self, shard: ShardId, now: Instant) -> bool {
        if !self.policy.enabled {
            return false;
        }
        match self
            .last_run
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&shard.0)
        {
            Some(t) => now.duration_since(*t) >= self.policy.cadence,
            None => true,
        }
    }

    /// Checkpoint one shard unconditionally (ignoring cadence).
    ///
    /// Returns `None` when this node does not own the shard — a non-owner must
    /// not be able to publish a "durable" copy of state it has no authority
    /// over, or a restore could rebuild the wrong world.
    pub fn checkpoint_now(
        &self,
        authority: &ShardAuthority,
        shard: ShardId,
        tick: u64,
        now_unix: u64,
        now: Instant,
    ) -> Option<CheckpointRef> {
        let epoch = authority.epoch_of(shard)?;
        let state = authority.state_of(shard)?;
        let cp = Checkpoint::new(shard, epoch, tick, now_unix, state);
        let id = self.store.put(&cp);
        let r = CheckpointRef {
            shard: shard.0,
            epoch,
            tick,
            taken_at_unix: now_unix,
            id,
        };
        self.latest
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(shard.0, r);
        self.last_run
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(shard.0, now);
        info!(
            shard = shard.0,
            epoch,
            tick,
            bytes = cp.state.len(),
            id = %id,
            "shard checkpointed"
        );
        Some(r)
    }

    /// Checkpoint every owned shard whose cadence has elapsed. The ordinary
    /// entry point for a tick loop or a background thread.
    pub fn checkpoint_due(
        &self,
        authority: &ShardAuthority,
        tick: u64,
        now_unix: u64,
        now: Instant,
    ) -> Vec<CheckpointRef> {
        if !self.policy.enabled {
            return Vec::new();
        }
        authority
            .owned_shards()
            .into_iter()
            .filter(|s| self.is_due(*s, now))
            .filter_map(|s| self.checkpoint_now(authority, s, tick, now_unix, now))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Driving checkpoints from a live node
// ---------------------------------------------------------------------------

/// Seconds since the Unix epoch, or 0 if the clock is before it.
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Publishes a live shard's state and tick into [`ShardAuthority`] so that a
/// [`Checkpointer`] has something real to checkpoint.
///
/// # Why this type has to exist
///
/// [`Checkpointer::checkpoint_due`] can only checkpoint shards the node
/// *owns* — it reads [`ShardAuthority::owned_shards`] and
/// [`ShardAuthority::state_of`]. A serving node whose game state lives only
/// inside its executor owns nothing as far as the fleet layer is concerned, so
/// a checkpointer attached to it writes **nothing**, forever, and reports no
/// error while doing so. This sink is the connection between "the game is
/// simulating a world" and "the fleet layer knows this node holds that world".
///
/// # The claim happens exactly once
///
/// The first publish [`ShardAuthority::claim`]s the shard, taking epoch 1. Every
/// publish after that is an [`ShardAuthority::update_state`], which is a no-op
/// unless this node still owns the shard.
///
/// It **never re-claims**, and that is deliberate rather than incidental. Losing
/// ownership means one of two things: the shard was handed to a peer, or this
/// node was fenced because a survivor restored the shard at a higher epoch.
/// Re-claiming would bump the epoch and resurrect this node as a competing owner
/// — precisely the split-brain the epoch fence exists to prevent. A sink that
/// has lost its shard goes quiet and stays quiet.
#[derive(Clone)]
pub struct ShardStateSink {
    authority: ShardAuthority,
    shard: ShardId,
    /// Latest simulation tick, read by the checkpoint loop so a checkpoint
    /// records the tick it was actually taken at.
    tick: Arc<std::sync::atomic::AtomicU64>,
    claimed: Arc<std::sync::atomic::AtomicBool>,
    lost_warned: Arc<std::sync::atomic::AtomicBool>,
}

impl std::fmt::Debug for ShardStateSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShardStateSink")
            .field("shard", &self.shard)
            .field("tick", &self.tick())
            .finish_non_exhaustive()
    }
}

impl ShardStateSink {
    /// Publish `shard`'s state into `authority`.
    pub fn new(authority: ShardAuthority, shard: ShardId) -> Self {
        Self {
            authority,
            shard,
            tick: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            claimed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            lost_warned: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// The shard being published.
    pub fn shard(&self) -> ShardId {
        self.shard
    }

    /// The authority being published into.
    pub fn authority(&self) -> &ShardAuthority {
        &self.authority
    }

    /// The most recent tick published.
    pub fn tick(&self) -> u64 {
        self.tick.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// A shared handle to the live tick counter, for the checkpoint loop.
    pub fn tick_source(&self) -> Arc<std::sync::atomic::AtomicU64> {
        Arc::clone(&self.tick)
    }

    /// Record the world as of `tick`.
    ///
    /// Returns `true` while this node still owns the shard. Once it does not,
    /// this returns `false` forever: see the type docs on why re-claiming is
    /// not an option.
    pub fn publish(&self, tick: u64, state: Vec<u8>) -> bool {
        use std::sync::atomic::Ordering;
        self.tick.store(tick, Ordering::Relaxed);
        if !self.claimed.load(Ordering::Acquire) {
            self.authority.claim(self.shard, state);
            self.claimed.store(true, Ordering::Release);
            info!(
                shard = self.shard.0,
                epoch = self.authority.epoch_of(self.shard).unwrap_or(0),
                tick,
                "node claimed its local shard — state is now checkpointable"
            );
            return true;
        }
        if self.authority.update_state(self.shard, state) {
            return true;
        }
        if !self.lost_warned.swap(true, Ordering::Relaxed) {
            warn!(
                shard = self.shard.0,
                tick,
                "this node no longer owns shard {} (handed off, or fenced by a higher epoch); \
                 it will NOT re-claim it — re-claiming would make this node a second owner",
                self.shard.0
            );
        }
        false
    }
}

/// Wraps a [`GameExecutor`] so every simulated tick is published to a
/// [`ShardStateSink`].
///
/// This is the only place a real node's world becomes visible to the durability
/// machinery. It is a pass-through in every other respect: it changes no
/// simulation behaviour, and the state it publishes is the executor's own
/// `snapshot()` bytes, so a restore rebuilds byte-identical state.
///
/// [`GameExecutor`]: magnetite_sdk::authority::GameExecutor
pub struct ShardStateExecutor {
    inner: Box<dyn magnetite_sdk::authority::GameExecutor>,
    sink: ShardStateSink,
}

impl ShardStateExecutor {
    /// Wrap `inner`, publishing each tick's snapshot into `sink`.
    pub fn new(
        inner: Box<dyn magnetite_sdk::authority::GameExecutor>,
        sink: ShardStateSink,
    ) -> Self {
        Self { inner, sink }
    }
}

impl magnetite_sdk::authority::GameExecutor for ShardStateExecutor {
    fn step(
        &mut self,
        tick: magnetite_sdk::authority::Tick,
        inputs: &[(magnetite_sdk::state::PlayerId, magnetite_sdk::input::Input)],
    ) -> magnetite_sdk::authority::StepOutput {
        let out = self.inner.step(tick, inputs);
        // Publish AFTER the step, so the state and the tick describe the same
        // moment. A checkpoint taken from this is a world that was really
        // simulated, not one mid-step.
        self.sink.publish(tick, self.inner.snapshot());
        out
    }
    fn snapshot(&self) -> Vec<u8> {
        self.inner.snapshot()
    }
    fn restore(&mut self, bytes: &[u8]) {
        self.inner.restore(bytes)
    }
    fn view_for(&self, player: magnetite_sdk::state::PlayerId) -> Vec<u8> {
        self.inner.view_for(player)
    }
    fn delta_since(&self, snapshot_bytes: &[u8]) -> Vec<u8> {
        self.inner.delta_since(snapshot_bytes)
    }
}

/// Longest a checkpoint loop sleeps between cadence checks.
///
/// The loop polls rather than sleeping a whole cadence so that the checkpoint it
/// writes reflects a tick from the last fraction of a second, not one from a
/// full cadence ago. Polling is what keeps the *content* fresh; the cadence in
/// [`CheckpointPolicy`] is still what decides when a write happens.
const CHECKPOINT_POLL: Duration = Duration::from_millis(250);

/// Start the background thread that actually writes checkpoints.
///
/// **This is the production driver.** A [`Checkpointer`] attached to a node does
/// nothing on its own — it exposes `checkpoint_due` and waits to be called. If
/// nothing calls it, no checkpoint is ever written and the entire restore path
/// downstream is unreachable, silently, with every test still green. This
/// function is what calls it.
///
/// The thread runs for the life of the process, like the rebalance loop.
pub fn spawn_checkpoint_loop(
    authority: ShardAuthority,
    checkpointer: Checkpointer,
    tick: Arc<std::sync::atomic::AtomicU64>,
) -> std::thread::JoinHandle<()> {
    let poll = checkpointer.policy().cadence.min(CHECKPOINT_POLL);
    std::thread::spawn(move || loop {
        std::thread::sleep(poll);
        checkpointer.checkpoint_due(
            &authority,
            tick.load(std::sync::atomic::Ordering::Relaxed),
            now_unix(),
            Instant::now(),
        );
    })
}

// ---------------------------------------------------------------------------
// Restoring
// ---------------------------------------------------------------------------

/// A shard rebuilt from a checkpoint. **Not** a loss-free recovery — read
/// [`Self::loss_window_secs`] before describing it to anyone as recovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardRecovery {
    /// The shard now running again on this node.
    pub shard: ShardId,
    /// The epoch this node claimed — strictly above both the checkpoint's epoch
    /// and this node's previous high-water mark, so the old owner is fenced.
    pub epoch: u64,
    /// The epoch the checkpoint was taken at (what the dead owner held).
    pub checkpoint_epoch: u64,
    /// The tick the world has been rolled back to.
    pub checkpoint_tick: u64,
    /// **Seconds of simulation lost**: from the checkpoint to the moment of
    /// recovery. Everything in this window is gone.
    pub loss_window_secs: u64,
    /// BLAKE3 of the restored state — the continuity assertion. A shard stepped
    /// on from here produces the same results as one stepped on from the
    /// original, because it is byte-identical state.
    pub state_hash: Hash,
    /// Content address of the checkpoint used.
    pub checkpoint: CheckpointId,
    /// The node that used to own it.
    pub previous_owner: magnetite_seams::identity::PubKey,
}

impl std::fmt::Display for ShardRecovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "shard {} RESTORED from checkpoint {} at tick {} (epoch {} -> {}, previous owner {}): \
             UP TO {}s OF SIMULATION WAS LOST — this is a rollback to the last checkpoint, \
             not a loss-free recovery",
            self.shard,
            self.checkpoint,
            self.checkpoint_tick,
            self.checkpoint_epoch,
            self.epoch,
            self.previous_owner.to_hex(),
            self.loss_window_secs,
        )
    }
}

/// Rebuild `shard` on this node from `cp_ref`, taking a strictly higher epoch.
///
/// Fails closed on every problem, and specifically refuses to claim an epoch
/// that would not out-rank this node's own high-water mark — a restore that
/// cannot fence the old owner is split-brain and is not worth having.
///
/// `previous_owner` is recorded for the report only; it grants nothing.
#[allow(clippy::too_many_arguments)]
pub fn restore_shard(
    authority: &ShardAuthority,
    store: &CheckpointStore,
    cp_ref: &CheckpointRef,
    shard: ShardId,
    previous_owner: magnetite_seams::identity::PubKey,
    now_unix: u64,
) -> Result<ShardRecovery, RestoreError> {
    if cp_ref.shard != shard.0 {
        return Err(RestoreError::WrongShard {
            want: shard,
            got: ShardId(cp_ref.shard),
        });
    }
    // Fetch + verify. Nothing below this line runs on unverified bytes.
    let cp = store.get_verified(cp_ref.id, shard)?;

    // The epoch floor is the max of what the checkpoint saw and what we have
    // seen, so the claim is strictly above BOTH. Claiming above only the
    // checkpoint could land us at or below an epoch this node already fenced.
    let hw = authority.high_water(shard);
    let floor = hw.max(cp.epoch);
    let epoch = authority.claim_at_least(shard, floor, cp.state.clone());
    if epoch <= hw || epoch <= cp.epoch {
        // Defensive: `claim_at_least` guarantees this cannot happen. If it ever
        // did, we would rather have no shard than an unfenced one.
        return Err(RestoreError::NotHigherEpoch {
            proposed: epoch,
            high_water: floor,
        });
    }

    let rec = ShardRecovery {
        shard,
        epoch,
        checkpoint_epoch: cp.epoch,
        checkpoint_tick: cp.tick,
        loss_window_secs: now_unix.saturating_sub(cp.taken_at_unix),
        state_hash: cp.state_hash,
        checkpoint: cp_ref.id,
        previous_owner,
    };
    warn!(
        shard = shard.0,
        epoch,
        checkpoint_tick = cp.tick,
        loss_window_secs = rec.loss_window_secs,
        "{rec}"
    );
    Ok(rec)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::blobstore::LocalBlobStore;
    use magnetite_seams::identity::{PubKey, RawKeypairAuth};

    fn store() -> CheckpointStore {
        CheckpointStore::new(Arc::new(LocalBlobStore::new()))
    }

    fn owner() -> PubKey {
        RawKeypairAuth::generate().node_pubkey()
    }

    #[test]
    fn a_checkpoints_id_is_the_hash_of_its_bytes() {
        let cp = Checkpoint::new(ShardId(4), 7, 900, 1_700_000_000, b"world".to_vec());
        assert_eq!(cp.id().0, Hash::of(&cp.to_bytes()));
        assert_eq!(cp.state_hash, Hash::of(b"world"));
    }

    #[test]
    fn roundtrip_through_the_blobstore_seam_verifies() {
        let s = store();
        let cp = Checkpoint::new(ShardId(1), 2, 42, 1_700_000_000, b"state-bytes".to_vec());
        let id = s.put(&cp);
        assert_eq!(id, cp.id());
        let back = s.get_verified(id, ShardId(1)).expect("verified");
        assert_eq!(back, cp);
    }

    #[test]
    fn a_checkpoint_for_shard_s_never_restores_shard_t() {
        let s = store();
        let cp = Checkpoint::new(ShardId(5), 1, 10, 0, b"five".to_vec());
        let id = s.put(&cp);
        // Fetching it *as shard 9* is refused even though the bytes are intact.
        assert!(matches!(
            s.get_verified(id, ShardId(9)),
            Err(RestoreError::WrongShard { .. })
        ));
        // And so is a restore attempt with a mismatched ref.
        let auth = ShardAuthority::new();
        let bad_ref = CheckpointRef {
            shard: 9,
            ..cp.to_ref()
        };
        assert!(matches!(
            restore_shard(&auth, &s, &bad_ref, ShardId(9), owner(), 0),
            Err(RestoreError::WrongShard { .. })
        ));
        assert!(!auth.owns(ShardId(9)), "a refused restore must claim nothing");
    }

    #[test]
    fn a_tampered_checkpoint_is_rejected_not_restored() {
        // Store honest bytes, then announce them under a DIFFERENT id.
        let s = store();
        let cp = Checkpoint::new(ShardId(2), 3, 100, 0, b"honest".to_vec());
        s.put(&cp);
        let lying = CheckpointRef {
            id: CheckpointId(Hash::of(b"some-other-content")),
            ..cp.to_ref()
        };
        let auth = ShardAuthority::new();
        let err = restore_shard(&auth, &s, &lying, ShardId(2), owner(), 0).unwrap_err();
        assert!(matches!(err, RestoreError::Missing(_)));
        assert!(!auth.owns(ShardId(2)));

        // And a store that returns the wrong bytes for a hash is caught.
        struct Liar;
        #[async_trait::async_trait]
        impl BlobStore for Liar {
            async fn put(&self, b: &[u8]) -> Hash {
                Hash::of(b)
            }
            async fn get(&self, _h: &Hash) -> Option<Vec<u8>> {
                Some(b"garbage".to_vec())
            }
            async fn has(&self, _h: &Hash) -> bool {
                true
            }
        }
        let liar = CheckpointStore::new(Arc::new(Liar));
        let err = restore_shard(&auth, &liar, &cp.to_ref(), ShardId(2), owner(), 0).unwrap_err();
        assert!(
            matches!(err, RestoreError::Tampered { .. }),
            "a store that substitutes content must be caught: {err}"
        );
        assert!(!auth.owns(ShardId(2)), "garbage must never become a shard");
    }

    #[test]
    fn a_corrupt_state_hash_inside_the_record_is_rejected() {
        // A record whose internal state_hash does not match its state. It is
        // self-consistently addressed (the id covers the lie), so only the
        // second, redundant check catches it — which is why it exists.
        let s = store();
        let mut cp = Checkpoint::new(ShardId(3), 1, 5, 0, b"real".to_vec());
        cp.state_hash = Hash::of(b"not-the-real-state");
        let id = s.put(&cp);
        let err = s.get_verified(id, ShardId(3)).unwrap_err();
        assert!(matches!(err, RestoreError::StateHashMismatch { .. }), "{err}");
    }

    #[test]
    fn an_unknown_envelope_version_is_refused() {
        let s = store();
        let mut cp = Checkpoint::new(ShardId(1), 1, 1, 0, b"x".to_vec());
        cp.version = 99;
        let id = s.put(&cp);
        assert!(matches!(
            s.get_verified(id, ShardId(1)),
            Err(RestoreError::Version(99))
        ));
    }

    #[test]
    fn restore_takes_a_strictly_higher_epoch_than_the_checkpoint() {
        let s = store();
        let cp = Checkpoint::new(ShardId(1), 9, 500, 1_000, b"w".to_vec());
        s.put(&cp);
        let auth = ShardAuthority::new();
        let rec = restore_shard(&auth, &s, &cp.to_ref(), ShardId(1), owner(), 1_012).unwrap();
        assert!(
            rec.epoch > cp.epoch,
            "restore must out-rank the dead owner: {} vs {}",
            rec.epoch,
            cp.epoch
        );
        assert_eq!(auth.epoch_of(ShardId(1)), Some(rec.epoch));
        assert_eq!(auth.state_of(ShardId(1)).as_deref(), Some(&b"w"[..]));
        assert_eq!(rec.loss_window_secs, 12, "loss window must be reported");
    }

    #[test]
    fn restore_also_out_ranks_the_restorers_own_high_water() {
        // This node already fenced shard 1 up to epoch 40 (it saw a later
        // migration). A checkpoint from epoch 9 must not claim epoch 10.
        let s = store();
        let cp = Checkpoint::new(ShardId(1), 9, 500, 0, b"w".to_vec());
        s.put(&cp);
        let auth = ShardAuthority::new();
        auth.claim(ShardId(1), b"other".to_vec());
        for _ in 0..40 {
            auth.claim(ShardId(1), b"other".to_vec());
        }
        let hw = auth.high_water(ShardId(1));
        let rec = restore_shard(&auth, &s, &cp.to_ref(), ShardId(1), owner(), 0).unwrap();
        assert!(rec.epoch > hw, "restore must beat local high-water too");
    }

    #[test]
    fn continuity_the_restored_state_is_byte_identical() {
        let s = store();
        let world = b"entity=1,x=33.5,score=9001".to_vec();
        let cp = Checkpoint::new(ShardId(6), 2, 77, 0, world.clone());
        s.put(&cp);
        let auth = ShardAuthority::new();
        let rec = restore_shard(&auth, &s, &cp.to_ref(), ShardId(6), owner(), 0).unwrap();
        assert_eq!(auth.state_of(ShardId(6)), Some(world.clone()));
        assert_eq!(rec.state_hash, Hash::of(&world), "state hash continuity");
    }

    #[test]
    fn a_recovery_message_never_claims_zero_loss() {
        let rec = ShardRecovery {
            shard: ShardId(1),
            epoch: 10,
            checkpoint_epoch: 9,
            checkpoint_tick: 400,
            loss_window_secs: 14,
            state_hash: Hash::of(b"s"),
            checkpoint: CheckpointId(Hash::of(b"c")),
            previous_owner: owner(),
        };
        let msg = rec.to_string();
        assert!(msg.contains("WAS LOST"));
        assert!(msg.contains("rollback"));
        assert!(!msg.to_lowercase().contains("no data loss"));
        assert!(!msg.to_lowercase().contains("without loss"));
    }

    #[test]
    fn checkpointer_respects_cadence_and_ownership() {
        let auth = ShardAuthority::new();
        auth.claim(ShardId(1), b"v1".to_vec());
        let cpr = Checkpointer::new(
            store(),
            CheckpointPolicy {
                enabled: true,
                cadence: Duration::from_secs(10),
            },
        );
        let t0 = Instant::now();
        let made = cpr.checkpoint_due(&auth, 1, 0, t0);
        assert_eq!(made.len(), 1, "first pass always checkpoints");
        // Not due yet.
        assert!(cpr.checkpoint_due(&auth, 2, 0, t0 + Duration::from_secs(3)).is_empty());
        // Due.
        assert_eq!(
            cpr.checkpoint_due(&auth, 3, 0, t0 + Duration::from_secs(11))
                .len(),
            1
        );
        // A shard we do not own is never checkpointed — a non-owner must not
        // publish "durable" state it has no authority over.
        assert!(cpr
            .checkpoint_now(&auth, ShardId(99), 4, 0, t0)
            .is_none());
    }

    #[test]
    fn a_disabled_checkpointer_writes_nothing() {
        let auth = ShardAuthority::new();
        auth.claim(ShardId(1), b"v".to_vec());
        let cpr = Checkpointer::new(
            store(),
            CheckpointPolicy {
                enabled: false,
                ..Default::default()
            },
        );
        assert!(cpr.checkpoint_due(&auth, 1, 0, Instant::now()).is_empty());
        assert!(cpr.refs().is_empty());
    }

    #[test]
    fn the_newest_checkpoint_wins_and_is_announced() {
        let auth = ShardAuthority::new();
        auth.claim(ShardId(2), b"early".to_vec());
        let cpr = Checkpointer::new(store(), CheckpointPolicy::default());
        let t0 = Instant::now();
        cpr.checkpoint_now(&auth, ShardId(2), 1, 100, t0).unwrap();
        auth.update_state(ShardId(2), b"later".to_vec());
        let r2 = cpr.checkpoint_now(&auth, ShardId(2), 2, 200, t0).unwrap();
        assert_eq!(cpr.latest_for(ShardId(2)), Some(r2));
        assert_eq!(cpr.refs(), vec![r2]);
        // And the announced ref really resolves to the later state.
        let cp = cpr.store().get_verified(r2.id, ShardId(2)).unwrap();
        assert_eq!(cp.state, b"later".to_vec());
    }

    #[test]
    fn the_sink_claims_once_and_then_only_updates() {
        let auth = ShardAuthority::new();
        let sink = ShardStateSink::new(auth.clone(), ShardId(1));
        assert!(sink.publish(1, b"t1".to_vec()));
        let epoch = auth.epoch_of(ShardId(1)).expect("claimed");
        for t in 2..10 {
            assert!(sink.publish(t, format!("t{t}").into_bytes()));
        }
        assert_eq!(
            auth.epoch_of(ShardId(1)),
            Some(epoch),
            "publishing state must not bump the epoch — only a claim does that"
        );
        assert_eq!(auth.state_of(ShardId(1)).as_deref(), Some(&b"t9"[..]));
        assert_eq!(sink.tick(), 9, "the live tick must be readable by the loop");
    }

    /// The split-brain guard. Once a survivor has restored this shard at a
    /// higher epoch, the fenced node's game keeps ticking and keeps publishing.
    /// If any of those publishes re-claimed the shard, this node would come back
    /// as a second owner of a shard someone else legitimately holds.
    #[test]
    fn a_fenced_sink_never_re_claims_its_shard() {
        let auth = ShardAuthority::new();
        let sink = ShardStateSink::new(auth.clone(), ShardId(1));
        sink.publish(1, b"alive".to_vec());
        let mine = auth.epoch_of(ShardId(1)).expect("claimed");

        // A survivor restored it at a strictly higher epoch and fenced us.
        assert!(auth.fence(ShardId(1), mine + 5), "fence must strip ownership");
        assert!(!auth.owns(ShardId(1)));

        // The game does not know it is dead and keeps simulating.
        for t in 2..20 {
            assert!(
                !sink.publish(t, b"zombie".to_vec()),
                "a fenced sink must report that it no longer owns the shard"
            );
        }
        assert!(
            !auth.owns(ShardId(1)),
            "a fenced node re-claimed its shard — this is split-brain: two nodes \
             now believe they own the same shard"
        );
        assert_eq!(
            auth.high_water(ShardId(1)),
            mine + 5,
            "a fenced sink must not move the epoch fence at all"
        );
    }

    #[test]
    fn recovery_policy_defaults_are_conservative() {
        let p = RecoveryPolicy::default();
        assert!(p.min_failures >= 3, "one lost packet must not kill a node");
        assert!(p.grace >= Duration::from_secs(30));
    }
}
