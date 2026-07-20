//! Authenticated node-to-node shard handoff — the cross-node "Bucket D" wire.
//!
//! This is the transport that turns "many shards on one box" into "one world
//! across many boxes". It is deliberately **self-contained**: plain blocking TCP
//! plus an Ed25519 mutual handshake built on the node keypair
//! ([`magnetite_seams::identity`]). It does **not** depend on libp2p or any
//! external service — cross-node handoff is core game functionality, so it must
//! never sit on top of an optional dependency. (A QUIC or NAT-traversing
//! transport could one day be offered as an *optional* alternative behind the
//! same [`HandoffTransport`] seam; none is implemented.)
//!
//! ## Peer authentication
//!
//! Connecting to the right *address* is not proof of the right *node*. Both
//! sides therefore prove key control over fresh random nonces:
//!
//! ```text
//! client → Hello{ client_pub, client_nonce, proto }
//! server → ServerHello{ server_pub, server_nonce, sig_S }   sig over the transcript
//! client   verifies sig_S AND that server_pub == the PINNED expected peer key
//! client → ClientAuth{ sig_C }                              sig over the transcript
//! server   verifies sig_C AND that client_pub is in its allowlist (if set)
//! server → AuthOk
//! ```
//!
//! Both nonces enter the signed transcript, so a recorded handshake cannot be
//! replayed against a new session, and every later frame is signed over
//! `transcript = BLAKE3(client_nonce || server_nonce)` so frames cannot be
//! lifted from one connection into another.
//!
//! ## Two-phase handoff (the correctness core)
//!
//! Authority over a shard is the pair `(shard, epoch)`. Migration is 2PC with an
//! explicit epoch fence:
//!
//! ```text
//! SOURCE                                        TARGET
//! Owned(e)
//!   ── Offer{shard, e+1, state, hash} ────────►  validate: authed peer,
//!                                                e+1 > highest epoch seen,
//!                                                hash matches bytes
//!   ◄──────────────── Ack{shard, e+1, hash} ──  STAGED(e+1)   (not authoritative)
//! release authority locally (fenced at e)
//!   ── Commit{shard, e+1} ───────────────────►  promote STAGED → Owned(e+1)
//!   ◄────────────── CommitAck{shard, e+1} ────
//! Released
//! ```
//!
//! **Every partial failure resolves to "source still owns it".**
//!
//! | Failure | Resolution |
//! |---|---|
//! | connection/handshake fails | source `Owned(e)`, target never saw it |
//! | target rejects the offer | source `Owned(e)` |
//! | ack times out | source `Owned(e)`; target's stage is *not* authoritative |
//! | connection drops mid-transfer | source `Owned(e)`; stage discarded |
//! | commit-ack times out | source **reclaims** `Owned(e)`, keeps its state |
//!
//! The source never discards its serialized state until it has a `CommitAck`, so
//! "nobody owns the shard" is unreachable. The one genuinely ambiguous window in
//! any 2PC — commit delivered, commit-ack lost — is resolved by the **epoch
//! fence**: the target holds `e+1`, the source holds the strictly lower `e`, and
//! every peer rejects offers/commits at an epoch it has already surpassed. So a
//! stale owner cannot resurrect itself or hand the shard on; the higher epoch
//! deterministically wins. Duplicate and replayed handoffs are rejected by the
//! same monotonic epoch check.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use magnetite_seams::blobstore::Hash;
use magnetite_seams::discovery::Capacity;
use magnetite_seams::identity::{Identity, PubKey, RawKeypairAuth, Sig};

use crate::cluster::{ClusterMembership, RouteDirectory, RouteRejection, Redirector, SignedRedirect};
use crate::shard::{HandoffError, HandoffEvent, HandoffTransport, ShardId};

/// Wall-clock unix seconds, used to stamp redirect/token lifetimes.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Wire constants
// ---------------------------------------------------------------------------

/// Protocol identifier carried in the handshake and bound into every signature.
pub const PROTO: &str = "magnetite-handoff/1";

/// Hard cap on a single frame (state blobs included). Refused, not truncated.
pub const MAX_FRAME: usize = 64 * 1024 * 1024;

/// Default per-read/write socket timeout for a handoff exchange.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a target keeps a STAGED (accepted but uncommitted) shard before
/// discarding it. A stage is never authoritative, so expiry can only ever lose
/// a not-yet-committed copy — the source still owns the shard.
pub const STAGE_TTL: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

/// One protocol frame. Length-prefixed JSON on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Frame {
    /// Client opens: who it claims to be + its nonce.
    Hello {
        /// Claimed client node key (proven in `ClientAuth`).
        peer: PubKey,
        /// Client's random 32-byte nonce (hex).
        nonce: String,
        /// Protocol identifier; must equal [`PROTO`].
        proto: String,
    },
    /// Server answers, proving control of its node key over the transcript.
    ServerHello {
        /// Server's node key.
        peer: PubKey,
        /// Server's random 32-byte nonce (hex).
        nonce: String,
        /// Signature over the server-side transcript bytes.
        sig: Sig,
    },
    /// Client proves control of the key it claimed in `Hello`.
    ClientAuth {
        /// Signature over the client-side transcript bytes.
        sig: Sig,
    },
    /// Handshake complete; the channel is mutually authenticated.
    AuthOk,
    /// Phase 1 — transfer serialized shard state at a new epoch.
    Offer {
        /// Shard being migrated.
        shard: u32,
        /// The **new** epoch this migration establishes (strictly increasing).
        epoch: u64,
        /// Serialized shard state, hex-encoded.
        state: String,
        /// BLAKE3 of the raw state bytes (hex) — checked against `state`.
        hash: String,
        /// Source signature binding `(transcript, shard, epoch, hash)`.
        sig: Sig,
    },
    /// Phase 1 ack — state validated and STAGED (not yet authoritative).
    Ack {
        /// Shard.
        shard: u32,
        /// Epoch acked.
        epoch: u64,
        /// The hash the target actually computed over the bytes it stored.
        hash: String,
        /// Target signature binding `(transcript, shard, epoch, hash)`.
        sig: Sig,
    },
    /// Any refusal. The source treats this as "I still own the shard".
    Reject {
        /// Human-readable reason (also logged).
        reason: String,
    },
    /// Phase 2 — promote the staged state to authoritative on the target.
    Commit {
        /// Shard.
        shard: u32,
        /// Epoch being committed.
        epoch: u64,
        /// Source signature binding `(transcript, shard, epoch)`.
        sig: Sig,
    },
    /// Phase 2 ack — the target is now the authoritative owner at `epoch`.
    CommitAck {
        /// Shard.
        shard: u32,
        /// Epoch now owned by the target.
        epoch: u64,
        /// Target signature binding `(transcript, shard, epoch)`.
        sig: Sig,
    },
    /// "I hold `shard` at `epoch`; if you hold it lower, you are stale."
    ///
    /// The active half of the epoch fence, used after a checkpoint restore to
    /// evict a zombie owner that came back believing it still owns the shard.
    /// It grants nothing and moves no state: the receiver either finds its own
    /// high-water mark is already ≥ `epoch` and ignores it, or drops its stale
    /// claim. Signed over the session transcript, so it cannot be replayed onto
    /// another connection, and only ever accepted from an authenticated peer
    /// that passed the inbound allowlist.
    Fence {
        /// Shard being fenced.
        shard: u32,
        /// The strictly-higher epoch the sender holds.
        epoch: u64,
        /// Sender signature binding `(transcript, shard, epoch)`.
        sig: Sig,
    },
    /// The answer to [`Frame::Fence`].
    FenceAck {
        /// Shard.
        shard: u32,
        /// Epoch that was asserted.
        epoch: u64,
        /// Whether the receiver actually dropped a stale claim. `false` means
        /// it was not stale (it holds an equal or higher epoch) — which is
        /// itself informative: the *sender* is the one who should stand down.
        dropped: bool,
    },
    /// **Read-only** query: "what do you own, and how much can you hold?"
    ///
    /// Answered only on an already mutually-authenticated channel, and only to a
    /// peer that passed the inbound allowlist — i.e. to a cluster member. It
    /// changes no authority, stages nothing, and can never be a step in a
    /// handoff: it exists so [`crate::rebalance`] can see the real cluster
    /// instead of guessing.
    StatusRequest,
    /// The answer to [`Frame::StatusRequest`].
    ///
    /// Deliberately **unsigned**: the whole frame already travels inside the
    /// authenticated channel, so its authorship is exactly as proven as every
    /// other post-handshake frame. It is also advisory — a peer that lies about
    /// its capacity can only attract or repel shards it is already authorized to
    /// hold, and every actual handoff to it is still epoch-fenced, two-phase and
    /// membership-checked.
    Status {
        /// Shard ids this node is authoritative for right now.
        shards: Vec<u32>,
        /// `(shard, epoch)` for each owned shard — the authority claim made
        /// explicit, so a peer can tell a *current* owner from a *stale* one
        /// without attempting a handoff to find out.
        ///
        /// `#[serde(default)]`: a peer built before checkpointing existed simply
        /// omits it, and is then treated as claiming no epochs — which disables
        /// fencing against that peer rather than mis-fencing it.
        #[serde(default)]
        epochs: Vec<(u32, u64)>,
        /// Newest durable checkpoint this node has written per shard.
        ///
        /// A **pointer**, not evidence: a survivor re-fetches and fully
        /// re-verifies the content before restoring anything, so a peer that
        /// lies here can only cause a failed fetch. `#[serde(default)]` for the
        /// same forward/backward-compatibility reason as `epochs`.
        #[serde(default)]
        checkpoints: Vec<crate::checkpoint::CheckpointRef>,
        /// Self-declared logical cores.
        cpu_cores: u32,
        /// Self-declared RAM in megabytes.
        ram_mb: u64,
        /// Self-declared shard ceiling (`0` ⇒ derive from hardware).
        max_shards: u32,
        /// Self-declared free player slots.
        free_slots: u32,
    },
}

// ---------------------------------------------------------------------------
// Framing
// ---------------------------------------------------------------------------

fn write_frame(sock: &mut TcpStream, frame: &Frame) -> Result<(), HandoffError> {
    let body = serde_json::to_vec(frame)
        .map_err(|e| HandoffError::Transport(format!("encode frame: {e}")))?;
    if body.len() > MAX_FRAME {
        return Err(HandoffError::Transport(format!(
            "frame of {} bytes exceeds the {MAX_FRAME}-byte limit",
            body.len()
        )));
    }
    let len = (body.len() as u32).to_be_bytes();
    sock.write_all(&len).map_err(io_err)?;
    sock.write_all(&body).map_err(io_err)?;
    sock.flush().map_err(io_err)?;
    Ok(())
}

fn read_frame(sock: &mut TcpStream) -> Result<Frame, HandoffError> {
    let mut len = [0u8; 4];
    sock.read_exact(&mut len).map_err(io_err)?;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(HandoffError::Transport(format!(
            "peer announced a {n}-byte frame, over the {MAX_FRAME}-byte limit"
        )));
    }
    let mut body = vec![0u8; n];
    sock.read_exact(&mut body).map_err(io_err)?;
    serde_json::from_slice(&body).map_err(|e| HandoffError::Transport(format!("decode frame: {e}")))
}

/// Map an I/O error to the right handoff failure, keeping timeouts distinct so
/// the caller can report "ack timed out — authority retained" precisely.
fn io_err(e: std::io::Error) -> HandoffError {
    match e.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
            HandoffError::Timeout(e.to_string())
        }
        _ => HandoffError::Transport(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Transcript + signing bytes
// ---------------------------------------------------------------------------

fn transcript(client_nonce: &[u8; 32], server_nonce: &[u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(client_nonce);
    buf.extend_from_slice(server_nonce);
    Hash::of(&buf).0
}

fn hs_bytes(
    tag: &[u8],
    client_nonce: &[u8; 32],
    server_nonce: &[u8; 32],
    client_pub: &PubKey,
    server_pub: &PubKey,
) -> Vec<u8> {
    let mut b = Vec::with_capacity(tag.len() + PROTO.len() + 32 * 4);
    b.extend_from_slice(tag);
    b.extend_from_slice(PROTO.as_bytes());
    b.extend_from_slice(client_nonce);
    b.extend_from_slice(server_nonce);
    b.extend_from_slice(&client_pub.0);
    b.extend_from_slice(&server_pub.0);
    b
}

fn msg_bytes(tag: &[u8], transcript: &[u8; 32], shard: u32, epoch: u64, hash: &str) -> Vec<u8> {
    let mut b = Vec::with_capacity(tag.len() + 32 + 12 + hash.len());
    b.extend_from_slice(tag);
    b.extend_from_slice(transcript);
    b.extend_from_slice(&shard.to_le_bytes());
    b.extend_from_slice(&epoch.to_le_bytes());
    b.extend_from_slice(hash.as_bytes());
    b
}

fn verify(pk: &PubKey, msg: &[u8], sig: &Sig) -> bool {
    <RawKeypairAuth as Identity>::verify(pk, msg, sig)
}

fn random_nonce() -> [u8; 32] {
    // The node keypair crate already pulls in `rand`; derive a nonce from the OS
    // CSPRNG via a throwaway key so this module needs no extra dependency.
    // (`RawKeypairAuth::generate` seeds itself from `OsRng`.)
    RawKeypairAuth::generate().node_pubkey().0
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn hex_decode(s: &str) -> Result<Vec<u8>, HandoffError> {
    if !s.len().is_multiple_of(2) {
        return Err(HandoffError::Transport("odd-length hex".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| HandoffError::Transport(format!("bad hex: {e}")))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Authority table
// ---------------------------------------------------------------------------

/// A shard this node holds authority over, at a specific epoch.
#[derive(Debug, Clone)]
pub struct OwnedShard {
    /// The epoch at which authority is held. Higher always wins.
    pub epoch: u64,
    /// The last serialized state for this shard.
    pub state: Vec<u8>,
}

#[derive(Debug, Clone)]
struct StagedShard {
    epoch: u64,
    state: Vec<u8>,
    staged_at: std::time::Instant,
}

#[derive(Debug, Default)]
struct AuthorityInner {
    owned: HashMap<u32, OwnedShard>,
    staged: HashMap<u32, StagedShard>,
    /// Highest epoch ever *seen* for a shard, whether owned, staged, or handed
    /// away. This is the replay/stale fence and is never decreased.
    high_water: HashMap<u32, u64>,
}

/// Per-node record of which shards it owns, at which epoch — the fence that
/// makes duplicate and stale handoffs impossible to apply.
///
/// Cloneable and thread-safe: the listener thread and the transport share one.
#[derive(Debug, Clone, Default)]
pub struct ShardAuthority {
    inner: Arc<Mutex<AuthorityInner>>,
}

impl ShardAuthority {
    /// An empty authority table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Take initial authority over a shard locally (bootstrap / world creation).
    pub fn claim(&self, shard: ShardId, state: Vec<u8>) {
        let mut g = self.inner.lock().unwrap();
        let epoch = g.high_water.get(&shard.0).copied().unwrap_or(0) + 1;
        g.high_water.insert(shard.0, epoch);
        g.owned.insert(shard.0, OwnedShard { epoch, state });
    }

    /// Take authority over a shard at an epoch **strictly above** both this
    /// node's high-water mark and `floor`, returning the epoch claimed.
    ///
    /// This is [`Self::claim`] with an externally-supplied floor, and it exists
    /// for exactly one caller: [`crate::checkpoint::restore_shard`]. A survivor
    /// rebuilding a dead node's shard has usually never seen that shard, so its
    /// own high-water mark is 0 and a plain `claim` would take epoch 1 — which
    /// the returning owner, sitting at epoch 7, would out-rank. Passing the
    /// checkpoint's epoch as the floor is what makes the restore *win* the
    /// existing fence instead of losing to it.
    ///
    /// It cannot be used to weaken the fence: the result is always strictly
    /// greater than the current high-water mark, so it can only ever move
    /// authority forward.
    pub fn claim_at_least(&self, shard: ShardId, floor: u64, state: Vec<u8>) -> u64 {
        let mut g = self.inner.lock().unwrap();
        let hw = g.high_water.get(&shard.0).copied().unwrap_or(0);
        let epoch = hw.max(floor).saturating_add(1);
        g.high_water.insert(shard.0, epoch);
        // A stage for an older epoch can never be committed now; drop it so it
        // cannot sit around holding memory until its TTL.
        g.staged.remove(&shard.0);
        g.owned.insert(shard.0, OwnedShard { epoch, state });
        epoch
    }

    /// Be told, by a peer that proved it holds `shard` at `epoch`, that our
    /// claim is stale — and give it up if it is.
    ///
    /// Returns `true` if authority was actually dropped. This is the *active*
    /// half of the epoch fence: normally a stale owner discovers it is stale by
    /// trying to hand the shard on and being refused, but a zombie that returns
    /// after a restore may never try anything, and meanwhile two nodes claim the
    /// same shard. A fence resolves that immediately and deterministically.
    ///
    /// It is not a new authority path — it can only ever *reduce* what this node
    /// claims, and only in favour of a strictly higher epoch. A peer that sends
    /// a lower or equal epoch is ignored, so this cannot be used to strip a
    /// current owner. The caller ([`serve_conn`]) has already verified the
    /// sender is an authenticated cluster member and signed the frame.
    pub fn fence(&self, shard: ShardId, epoch: u64) -> bool {
        let mut g = self.inner.lock().unwrap();
        let hw = g.high_water.get(&shard.0).copied().unwrap_or(0);
        if epoch <= hw {
            // We are at or ahead of the claimed epoch: not stale, nothing to do.
            return false;
        }
        g.high_water.insert(shard.0, epoch);
        g.staged.remove(&shard.0);
        g.owned.remove(&shard.0).is_some()
    }

    /// Every shard this node owns, with the epoch it owns it at. Sorted by
    /// shard id. Used to announce authority so a stale claim can be detected.
    pub fn owned_epochs(&self) -> Vec<(ShardId, u64)> {
        let g = self.inner.lock().unwrap();
        let mut v: Vec<(ShardId, u64)> =
            g.owned.iter().map(|(s, o)| (ShardId(*s), o.epoch)).collect();
        v.sort_by_key(|(s, _)| s.0);
        v
    }

    /// Update the stored state of a shard we already own (e.g. after ticking).
    ///
    /// Ignored when this node is not the owner — a non-owner must not be able to
    /// stash state that a later handoff would ship as authoritative.
    pub fn update_state(&self, shard: ShardId, state: Vec<u8>) -> bool {
        let mut g = self.inner.lock().unwrap();
        match g.owned.get_mut(&shard.0) {
            Some(o) => {
                o.state = state;
                true
            }
            None => false,
        }
    }

    /// The epoch at which this node owns `shard`, or `None` if it does not.
    pub fn epoch_of(&self, shard: ShardId) -> Option<u64> {
        self.inner.lock().unwrap().owned.get(&shard.0).map(|o| o.epoch)
    }

    /// Whether this node currently holds authority over `shard`.
    pub fn owns(&self, shard: ShardId) -> bool {
        self.epoch_of(shard).is_some()
    }

    /// The authoritative state for a shard this node owns.
    pub fn state_of(&self, shard: ShardId) -> Option<Vec<u8>> {
        self.inner
            .lock()
            .unwrap()
            .owned
            .get(&shard.0)
            .map(|o| o.state.clone())
    }

    /// Every shard owned by this node, sorted.
    pub fn owned_shards(&self) -> Vec<ShardId> {
        let g = self.inner.lock().unwrap();
        let mut v: Vec<ShardId> = g.owned.keys().copied().map(ShardId).collect();
        v.sort();
        v
    }

    /// The highest epoch ever observed for a shard (the anti-replay fence).
    pub fn high_water(&self, shard: ShardId) -> u64 {
        self.inner
            .lock()
            .unwrap()
            .high_water
            .get(&shard.0)
            .copied()
            .unwrap_or(0)
    }

    // ---- target side -----------------------------------------------------

    /// Phase 1: validate and STAGE an incoming migration.
    ///
    /// Rejects any epoch that is not strictly above the high-water mark — that
    /// single check kills duplicates, replays, and late-arriving stale handoffs.
    /// A staged shard is **not** authoritative.
    fn stage(&self, shard: u32, epoch: u64, state: Vec<u8>) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        let hw = g.high_water.get(&shard).copied().unwrap_or(0);
        if epoch <= hw {
            return Err(format!(
                "stale or replayed handoff: epoch {epoch} is not above high-water {hw} for shard {shard}"
            ));
        }
        // Drop an expired stage before overwriting so TTL is honoured.
        if let Some(s) = g.staged.get(&shard) {
            if s.staged_at.elapsed() < STAGE_TTL && s.epoch >= epoch {
                return Err(format!(
                    "shard {shard} already has a live stage at epoch {}",
                    s.epoch
                ));
            }
        }
        g.staged.insert(
            shard,
            StagedShard {
                epoch,
                state,
                staged_at: std::time::Instant::now(),
            },
        );
        Ok(())
    }

    /// Phase 2: promote a stage to authoritative. Fails closed if the stage is
    /// missing, expired, or at a different epoch.
    fn commit(&self, shard: u32, epoch: u64) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        let staged = match g.staged.get(&shard) {
            Some(s) if s.epoch == epoch => s.clone(),
            Some(s) => {
                return Err(format!(
                    "commit for epoch {epoch} does not match staged epoch {}",
                    s.epoch
                ))
            }
            None => return Err(format!("no staged state for shard {shard}")),
        };
        if staged.staged_at.elapsed() >= STAGE_TTL {
            g.staged.remove(&shard);
            return Err(format!("stage for shard {shard} expired before commit"));
        }
        let hw = g.high_water.get(&shard).copied().unwrap_or(0);
        if epoch <= hw {
            g.staged.remove(&shard);
            return Err(format!(
                "commit at epoch {epoch} is fenced out by high-water {hw}"
            ));
        }
        g.staged.remove(&shard);
        g.high_water.insert(shard, epoch);
        g.owned.insert(
            shard,
            OwnedShard {
                epoch,
                state: staged.state,
            },
        );
        Ok(())
    }

    // ---- source side -----------------------------------------------------

    /// Reserve the next epoch for an outgoing migration without giving up
    /// authority. Returns `(new_epoch, state)`; `None` if we are not the owner.
    fn begin_offer(&self, shard: u32) -> Option<(u64, Vec<u8>)> {
        let mut g = self.inner.lock().unwrap();
        let owned = g.owned.get(&shard)?.clone();
        let hw = g.high_water.get(&shard).copied().unwrap_or(owned.epoch);
        let next = hw.max(owned.epoch) + 1;
        // Reserve it so a concurrent migration of the same shard cannot pick the
        // same epoch, but do NOT touch `owned` — authority is still ours.
        g.high_water.insert(shard, next);
        Some((next, owned.state))
    }

    /// Final step: authority is released to the peer that now holds `epoch`.
    /// Only ever called after a verified `CommitAck`.
    fn release(&self, shard: u32, epoch: u64) {
        let mut g = self.inner.lock().unwrap();
        g.owned.remove(&shard);
        let hw = g.high_water.get(&shard).copied().unwrap_or(0);
        g.high_water.insert(shard, hw.max(epoch));
    }
}

// ---------------------------------------------------------------------------
// Handshake
// ---------------------------------------------------------------------------

/// A completed, mutually-authenticated handshake.
pub struct AuthedChannel {
    /// The peer's proven node key.
    pub peer: PubKey,
    /// Transcript hash binding every later frame to this exact session.
    pub transcript: [u8; 32],
}

/// Server side of the mutual handshake.
///
/// `allowed` is the peer allowlist: `None` accepts any key that proves itself,
/// `Some(set)` accepts only listed keys. Anything that fails is refused with a
/// `Reject` frame and an error — never silently downgraded.
pub fn server_handshake(
    sock: &mut TcpStream,
    identity: &RawKeypairAuth,
    allowed: Option<&HashSet<PubKey>>,
) -> Result<AuthedChannel, HandoffError> {
    let (client_pub, client_nonce) = match read_frame(sock)? {
        Frame::Hello { peer, nonce, proto } => {
            if proto != PROTO {
                let _ = write_frame(
                    sock,
                    &Frame::Reject {
                        reason: format!("unsupported protocol {proto}"),
                    },
                );
                return Err(HandoffError::Auth(format!("unsupported protocol {proto}")));
            }
            let n = hex_decode(&nonce)?;
            let n: [u8; 32] = n
                .try_into()
                .map_err(|_| HandoffError::Auth("client nonce must be 32 bytes".into()))?;
            (peer, n)
        }
        other => {
            return Err(HandoffError::Auth(format!(
                "expected Hello, got {}",
                frame_name(&other)
            )))
        }
    };

    // Refuse an unexpected peer *before* spending a signature on it.
    if let Some(set) = allowed {
        if !set.contains(&client_pub) {
            let _ = write_frame(
                sock,
                &Frame::Reject {
                    reason: "peer not authorized".into(),
                },
            );
            return Err(HandoffError::Auth(format!(
                "peer {} is not in the allowlist",
                client_pub.to_hex()
            )));
        }
    }

    let server_nonce = random_nonce();
    let server_pub = identity.node_pubkey();
    let s_bytes = hs_bytes(
        b"mg-hs-server",
        &client_nonce,
        &server_nonce,
        &client_pub,
        &server_pub,
    );
    write_frame(
        sock,
        &Frame::ServerHello {
            peer: server_pub,
            nonce: hex_encode(&server_nonce),
            sig: identity.sign(&s_bytes),
        },
    )?;

    let c_bytes = hs_bytes(
        b"mg-hs-client",
        &client_nonce,
        &server_nonce,
        &client_pub,
        &server_pub,
    );
    match read_frame(sock)? {
        Frame::ClientAuth { sig } => {
            if !verify(&client_pub, &c_bytes, &sig) {
                let _ = write_frame(
                    sock,
                    &Frame::Reject {
                        reason: "bad client signature".into(),
                    },
                );
                return Err(HandoffError::Auth(
                    "client failed to prove control of its node key".into(),
                ));
            }
        }
        other => {
            return Err(HandoffError::Auth(format!(
                "expected ClientAuth, got {}",
                frame_name(&other)
            )))
        }
    }

    write_frame(sock, &Frame::AuthOk)?;
    Ok(AuthedChannel {
        peer: client_pub,
        transcript: transcript(&client_nonce, &server_nonce),
    })
}

/// Client side of the mutual handshake.
///
/// `expect_peer` is the node key we intend to talk to. Reaching the right
/// address proves nothing; if the far side presents any other key — or cannot
/// sign for the one it presents — the connection is refused.
pub fn client_handshake(
    sock: &mut TcpStream,
    identity: &RawKeypairAuth,
    expect_peer: &PubKey,
) -> Result<AuthedChannel, HandoffError> {
    let client_nonce = random_nonce();
    let client_pub = identity.node_pubkey();
    write_frame(
        sock,
        &Frame::Hello {
            peer: client_pub,
            nonce: hex_encode(&client_nonce),
            proto: PROTO.to_string(),
        },
    )?;

    let (server_pub, server_nonce) = match read_frame(sock)? {
        Frame::ServerHello { peer, nonce, sig } => {
            if peer != *expect_peer {
                return Err(HandoffError::Auth(format!(
                    "peer key mismatch: expected {}, the far side presented {}",
                    expect_peer.to_hex(),
                    peer.to_hex()
                )));
            }
            let n = hex_decode(&nonce)?;
            let n: [u8; 32] = n
                .try_into()
                .map_err(|_| HandoffError::Auth("server nonce must be 32 bytes".into()))?;
            let s_bytes = hs_bytes(b"mg-hs-server", &client_nonce, &n, &client_pub, &peer);
            if !verify(&peer, &s_bytes, &sig) {
                return Err(HandoffError::Auth(
                    "server failed to prove control of its node key".into(),
                ));
            }
            (peer, n)
        }
        Frame::Reject { reason } => return Err(HandoffError::Auth(reason)),
        other => {
            return Err(HandoffError::Auth(format!(
                "expected ServerHello, got {}",
                frame_name(&other)
            )))
        }
    };

    let c_bytes = hs_bytes(
        b"mg-hs-client",
        &client_nonce,
        &server_nonce,
        &client_pub,
        &server_pub,
    );
    write_frame(
        sock,
        &Frame::ClientAuth {
            sig: identity.sign(&c_bytes),
        },
    )?;

    match read_frame(sock)? {
        Frame::AuthOk => {}
        Frame::Reject { reason } => return Err(HandoffError::Auth(reason)),
        other => {
            return Err(HandoffError::Auth(format!(
                "expected AuthOk, got {}",
                frame_name(&other)
            )))
        }
    }

    Ok(AuthedChannel {
        peer: server_pub,
        transcript: transcript(&client_nonce, &server_nonce),
    })
}

fn frame_name(f: &Frame) -> &'static str {
    match f {
        Frame::Hello { .. } => "Hello",
        Frame::ServerHello { .. } => "ServerHello",
        Frame::ClientAuth { .. } => "ClientAuth",
        Frame::AuthOk => "AuthOk",
        Frame::Offer { .. } => "Offer",
        Frame::Ack { .. } => "Ack",
        Frame::Reject { .. } => "Reject",
        Frame::Commit { .. } => "Commit",
        Frame::CommitAck { .. } => "CommitAck",
        Frame::StatusRequest => "StatusRequest",
        Frame::Status { .. } => "Status",
        Frame::Fence { .. } => "Fence",
        Frame::FenceAck { .. } => "FenceAck",
    }
}

/// A capacity that claims nothing. Used before a host publishes real numbers.
pub(crate) fn unmeasured_capacity() -> Capacity {
    Capacity {
        cpu_cores: 0,
        ram_mb: 0,
        bandwidth_mbps: 0,
        free_slots: 0,
        max_shards: 0,
    }
}

/// A cloneable handle that updates what a [`FleetNode`] reports to peers.
///
/// Purely advisory data — it cannot grant, revoke or move authority.
#[derive(Debug, Clone)]
pub struct CapacityPublisher {
    inner: Arc<Mutex<Capacity>>,
}

impl CapacityPublisher {
    /// Replace the advertised capacity. Takes effect on the next status query.
    pub fn publish(&self, capacity: Capacity) {
        match self.inner.lock() {
            Ok(mut c) => *c = capacity,
            Err(p) => *p.into_inner() = capacity,
        }
    }
}

/// What a peer answered to a [`Frame::StatusRequest`].
///
/// Everything in here is the peer's **own claim**, made over an authenticated
/// channel. It is used for placement arithmetic only; it is never a substitute
/// for membership, and acting on it still goes through the ordinary two-phase,
/// epoch-fenced, membership-checked handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerStatus {
    /// The peer's node key, as proven by the handshake (not as claimed in the
    /// frame — the frame carries no key at all).
    pub node: PubKey,
    /// Shards the peer says it is authoritative for.
    pub shards: Vec<ShardId>,
    /// The peer's self-declared capacity.
    pub capacity: Capacity,
    /// `(shard, epoch)` the peer claims. Compared against local epochs to spot
    /// a stale (zombie) claim; a shard missing from here is simply not fenced.
    pub epochs: Vec<(ShardId, u64)>,
    /// Checkpoints the peer says it has written. Cached by [`crate::rebalance`]
    /// while the peer is alive, so that when it dies a survivor still knows
    /// where the durable copy is. Always re-verified before use.
    pub checkpoints: Vec<crate::checkpoint::CheckpointRef>,
}

impl PeerStatus {
    /// The epoch this peer claims for `shard`, if it claims one.
    pub fn epoch_of(&self, shard: ShardId) -> Option<u64> {
        self.epochs
            .iter()
            .find(|(s, _)| *s == shard)
            .map(|(_, e)| *e)
    }
}

// ---------------------------------------------------------------------------
// FleetNode — the listener half
// ---------------------------------------------------------------------------

/// A node's handoff endpoint: the authenticated listener plus its shard
/// authority table.
///
/// Bind it once per node; hand the [`ShardAuthority`] to whatever drives the
/// simulation so it can see which shards this box owns.
pub struct FleetNode {
    identity: Arc<RawKeypairAuth>,
    authority: ShardAuthority,
    allowed: Option<HashSet<PubKey>>,
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
    timeout: Duration,
    /// What this node reports to peers that ask for its status. Shared with the
    /// listener threads so [`Self::publish_capacity`] takes effect live.
    capacity: Arc<Mutex<Capacity>>,
    /// The checkpointer whose refs are announced in status replies. `None` (the
    /// default) announces no checkpoints at all, so a node with durability
    /// switched off tells peers exactly that and is never restored from.
    checkpointer: Arc<Mutex<Option<crate::checkpoint::Checkpointer>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FleetNode {
    /// Bind a handoff listener.
    ///
    /// Pass `"127.0.0.1:0"` for an ephemeral port (tests use this to stand up
    /// several real nodes in one process). `allowed`, when `Some`, is the set of
    /// peer node keys permitted to hand shards to this node.
    pub fn bind(
        addr: impl ToSocketAddrs,
        identity: Arc<RawKeypairAuth>,
        allowed: Option<HashSet<PubKey>>,
    ) -> Result<Self, HandoffError> {
        let listener = TcpListener::bind(addr)
            .map_err(|e| HandoffError::Transport(format!("bind failed: {e}")))?;
        let local = listener
            .local_addr()
            .map_err(|e| HandoffError::Transport(format!("local_addr failed: {e}")))?;
        let authority = ShardAuthority::new();
        let shutdown = Arc::new(AtomicBool::new(false));
        // Until the host publishes its measured capacity, this node advertises
        // ZERO headroom. That is the conservative direction: a node that has not
        // said how big it is attracts no shards, rather than attracting a share
        // it may not be able to hold.
        let capacity = Arc::new(Mutex::new(unmeasured_capacity()));
        let checkpointer: Arc<Mutex<Option<crate::checkpoint::Checkpointer>>> =
            Arc::new(Mutex::new(None));

        let t_checkpointer = Arc::clone(&checkpointer);
        let t_capacity = Arc::clone(&capacity);
        let t_identity = Arc::clone(&identity);
        let t_authority = authority.clone();
        let t_allowed = allowed.clone();
        let t_shutdown = Arc::clone(&shutdown);
        let timeout = DEFAULT_TIMEOUT;

        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if t_shutdown.load(Ordering::SeqCst) {
                    break;
                }
                let mut sock = match stream {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("handoff listener accept failed: {e}");
                        continue;
                    }
                };
                let _ = sock.set_read_timeout(Some(timeout));
                let _ = sock.set_write_timeout(Some(timeout));
                let c_identity = Arc::clone(&t_identity);
                let c_authority = t_authority.clone();
                let c_allowed = t_allowed.clone();
                let c_capacity = Arc::clone(&t_capacity);
                let c_checkpointer = Arc::clone(&t_checkpointer);
                std::thread::spawn(move || {
                    if let Err(e) = serve_conn(
                        &mut sock,
                        &c_identity,
                        &c_authority,
                        c_allowed.as_ref(),
                        &c_capacity,
                        &c_checkpointer,
                    ) {
                        // Refusals are expected traffic (that is the point of a
                        // fail-closed door); log and drop the connection.
                        debug!("handoff connection ended: {e}");
                    }
                });
            }
        });

        Ok(Self {
            identity,
            authority,
            allowed,
            addr: local,
            shutdown,
            timeout,
            capacity,
            checkpointer,
            handle: Some(handle),
        })
    }

    /// Announce this node's durable checkpoints to peers that query its status.
    ///
    /// Advisory in the same sense as capacity: it tells the cluster *where the
    /// durable copy is*, and nothing more. A survivor that acts on it still
    /// fetches the content, re-hashes it, checks the shard binding, and claims a
    /// strictly higher epoch. Until this is called the node announces nothing,
    /// which means no peer will ever try to restore its shards.
    pub fn attach_checkpointer(&self, checkpointer: crate::checkpoint::Checkpointer) {
        match self.checkpointer.lock() {
            Ok(mut c) => *c = Some(checkpointer),
            Err(p) => *p.into_inner() = Some(checkpointer),
        }
    }

    /// Set what this node reports to peers that send a [`Frame::StatusRequest`].
    ///
    /// Advisory only, in both directions: nothing here grants or withholds
    /// authority, it just lets the cluster's rebalancers size this box. Takes
    /// effect on the next status query.
    pub fn publish_capacity(&self, capacity: Capacity) {
        match self.capacity.lock() {
            Ok(mut c) => *c = capacity,
            Err(p) => *p.into_inner() = capacity,
        }
    }

    /// A detachable handle for [`Self::publish_capacity`], so a host that
    /// measures its hardware somewhere else (or later) can update what peers see
    /// without holding the node itself.
    pub fn capacity_publisher(&self) -> CapacityPublisher {
        CapacityPublisher {
            inner: Arc::clone(&self.capacity),
        }
    }

    /// The capacity currently advertised to peers.
    pub fn published_capacity(&self) -> Capacity {
        self.capacity
            .lock()
            .map(|c| c.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    /// The address this node is listening on (resolved, so `:0` is usable).
    pub fn addr(&self) -> std::net::SocketAddr {
        self.addr
    }

    /// This node's public key — the identity a peer must pin to reach it.
    pub fn pubkey(&self) -> PubKey {
        self.identity.node_pubkey()
    }

    /// The route another node needs to hand a shard here.
    pub fn route(&self) -> PeerRoute {
        PeerRoute {
            addr: self.addr.to_string(),
            pubkey: self.pubkey(),
        }
    }

    /// This node's shard authority table.
    pub fn authority(&self) -> ShardAuthority {
        self.authority.clone()
    }

    /// The peer allowlist in force, if any.
    pub fn allowed(&self) -> Option<&HashSet<PubKey>> {
        self.allowed.as_ref()
    }

    /// Per-exchange socket timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Build an outbound transport that hands shards *away* from this node,
    /// sharing this node's authority table so releases are recorded.
    pub fn transport(&self) -> NetworkHandoffTransport {
        NetworkHandoffTransport::new(Arc::clone(&self.identity), self.authority.clone())
    }

    /// Stop accepting new connections. In-flight exchanges finish on their own.
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Unblock `incoming()` with a self-connect.
        let _ = TcpStream::connect(self.addr);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for FleetNode {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.shutdown();
        }
    }
}

/// Serve one authenticated connection: handshake, then handoff frames.
fn serve_conn(
    sock: &mut TcpStream,
    identity: &RawKeypairAuth,
    authority: &ShardAuthority,
    allowed: Option<&HashSet<PubKey>>,
    capacity: &Arc<Mutex<Capacity>>,
    checkpointer: &Arc<Mutex<Option<crate::checkpoint::Checkpointer>>>,
) -> Result<(), HandoffError> {
    let chan = server_handshake(sock, identity, allowed)?;
    debug!(peer = %chan.peer.to_hex(), "handoff peer authenticated");

    loop {
        let frame = match read_frame(sock) {
            Ok(f) => f,
            // A closed connection is the normal end of an exchange.
            Err(_) => return Ok(()),
        };
        match frame {
            Frame::Offer {
                shard,
                epoch,
                state,
                hash,
                sig,
            } => {
                let signed = msg_bytes(b"mg-offer", &chan.transcript, shard, epoch, &hash);
                if !verify(&chan.peer, &signed, &sig) {
                    write_frame(
                        sock,
                        &Frame::Reject {
                            reason: "offer signature does not verify".into(),
                        },
                    )?;
                    continue;
                }
                let bytes = match hex_decode(&state) {
                    Ok(b) => b,
                    Err(e) => {
                        write_frame(
                            sock,
                            &Frame::Reject {
                                reason: format!("undecodable state: {e}"),
                            },
                        )?;
                        continue;
                    }
                };
                let got = Hash::of(&bytes).to_hex();
                if got != hash {
                    write_frame(
                        sock,
                        &Frame::Reject {
                            reason: format!("state hash mismatch: declared {hash}, computed {got}"),
                        },
                    )?;
                    continue;
                }
                match authority.stage(shard, epoch, bytes) {
                    Ok(()) => {
                        let ack = msg_bytes(b"mg-ack", &chan.transcript, shard, epoch, &got);
                        write_frame(
                            sock,
                            &Frame::Ack {
                                shard,
                                epoch,
                                hash: got,
                                sig: identity.sign(&ack),
                            },
                        )?;
                    }
                    Err(reason) => {
                        warn!(shard, epoch, %reason, "handoff offer refused");
                        write_frame(sock, &Frame::Reject { reason })?;
                    }
                }
            }
            Frame::Commit { shard, epoch, sig } => {
                let signed = msg_bytes(b"mg-commit", &chan.transcript, shard, epoch, "");
                if !verify(&chan.peer, &signed, &sig) {
                    write_frame(
                        sock,
                        &Frame::Reject {
                            reason: "commit signature does not verify".into(),
                        },
                    )?;
                    continue;
                }
                match authority.commit(shard, epoch) {
                    Ok(()) => {
                        info!(shard, epoch, "shard authority adopted from peer");
                        let ackb =
                            msg_bytes(b"mg-commit-ack", &chan.transcript, shard, epoch, "");
                        write_frame(
                            sock,
                            &Frame::CommitAck {
                                shard,
                                epoch,
                                sig: identity.sign(&ackb),
                            },
                        )?;
                    }
                    Err(reason) => {
                        warn!(shard, epoch, %reason, "handoff commit refused");
                        write_frame(sock, &Frame::Reject { reason })?;
                    }
                }
            }
            // Read-only cluster status. No authority is read out that a peer
            // could not already infer by trying a handoff, and nothing is
            // mutated. Only reachable past `server_handshake`, which has
            // already enforced the inbound allowlist.
            Frame::StatusRequest => {
                let cap = capacity
                    .lock()
                    .map(|c| c.clone())
                    .unwrap_or_else(|p| p.into_inner().clone());
                let owned = authority.owned_epochs();
                let shards: Vec<u32> = owned.iter().map(|(s, _)| s.0).collect();
                let epochs: Vec<(u32, u64)> = owned.iter().map(|(s, e)| (s.0, *e)).collect();
                let checkpoints = checkpointer
                    .lock()
                    .map(|c| c.as_ref().map(|c| c.refs()).unwrap_or_default())
                    .unwrap_or_default();
                write_frame(
                    sock,
                    &Frame::Status {
                        shards,
                        epochs,
                        checkpoints,
                        cpu_cores: cap.cpu_cores,
                        ram_mb: cap.ram_mb,
                        max_shards: cap.max_shards,
                        free_slots: cap.free_slots,
                    },
                )?;
            }
            // The active epoch fence. Accepted only from an authenticated,
            // allowlisted peer, and only with a signature over THIS session's
            // transcript. It can never raise this node's authority — only drop
            // a claim that a strictly higher epoch has already superseded.
            Frame::Fence { shard, epoch, sig } => {
                let signed = msg_bytes(b"mg-fence", &chan.transcript, shard, epoch, "");
                if !verify(&chan.peer, &signed, &sig) {
                    write_frame(
                        sock,
                        &Frame::Reject {
                            reason: "fence signature does not verify".into(),
                        },
                    )?;
                    continue;
                }
                let dropped = authority.fence(ShardId(shard), epoch);
                if dropped {
                    warn!(
                        shard,
                        epoch,
                        peer = %chan.peer.to_hex(),
                        "FENCED OUT: this node's claim on the shard was stale and has been \
                         dropped — a higher epoch owns it elsewhere"
                    );
                }
                write_frame(sock, &Frame::FenceAck { shard, epoch, dropped })?;
            }
            other => {
                write_frame(
                    sock,
                    &Frame::Reject {
                        reason: format!("unexpected frame {} after handshake", frame_name(&other)),
                    },
                )?;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Routing + the outbound transport
// ---------------------------------------------------------------------------

/// Where a shard lives and **which node key must answer there**.
///
/// The pubkey is the load-bearing half: an attacker who steals the address
/// still cannot receive a shard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRoute {
    /// `host:port` of the peer's handoff listener.
    pub addr: String,
    /// The peer's node key, pinned. A different key aborts the handoff.
    pub pubkey: PubKey,
}

impl PeerRoute {
    /// Build a route.
    pub fn new(addr: impl Into<String>, pubkey: PubKey) -> Self {
        Self {
            addr: addr.into(),
            pubkey,
        }
    }
}

/// The real cross-node handoff transport.
///
/// Replaces the old fail-closed stub: it opens an authenticated TCP channel to
/// the node that should own the target shard and runs the two-phase migration
/// described at the top of this module. With no route configured for a shard it
/// still fails closed ([`HandoffError::NoRoute`]) — it never pretends a
/// cross-node move happened.
pub struct NetworkHandoffTransport {
    identity: Arc<RawKeypairAuth>,
    authority: ShardAuthority,
    routes: HashMap<u32, PeerRoute>,
    timeout: Duration,
    /// Operator-authorized cluster membership. `None` keeps the pre-membership
    /// behaviour (any pinned route may be used); `Some` refuses to hand a shard
    /// to any key the operator did not authorize — including a hand-registered
    /// route. See [`crate::cluster`].
    membership: Option<ClusterMembership>,
    /// Redirect minting, enabled by [`Self::with_redirects`]. Redirects are
    /// produced **only** in the success path of a migration.
    redirector: Option<Redirector>,
    /// Players believed to be connected here, per shard — the set that gets
    /// redirected when that shard moves.
    shard_players: HashMap<u32, Vec<u64>>,
    /// Redirects minted by committed migrations, awaiting delivery to clients.
    pending_redirects: Vec<SignedRedirect>,
    /// Audit log of completed migrations: `(shard, epoch, bytes, peer hex)`.
    pub migrations: Vec<(ShardId, u64, usize, String)>,
}

impl NetworkHandoffTransport {
    /// Build a transport for a node with the given identity and authority table.
    pub fn new(identity: Arc<RawKeypairAuth>, authority: ShardAuthority) -> Self {
        Self {
            identity,
            authority,
            routes: HashMap::new(),
            timeout: DEFAULT_TIMEOUT,
            membership: None,
            redirector: None,
            shard_players: HashMap::new(),
            pending_redirects: Vec::new(),
            migrations: Vec::new(),
        }
    }

    /// Register (or replace) the route for a shard.
    ///
    /// Registering a route is **not** authorization: with a membership attached
    /// ([`Self::with_membership`]) a migration to a non-member key is still
    /// refused at send time.
    pub fn add_route(&mut self, shard: ShardId, route: PeerRoute) -> &mut Self {
        self.routes.insert(shard.0, route);
        self
    }

    /// Gate every outbound migration on operator-authorized cluster membership.
    ///
    /// This is the enforcement point for the rule that discovery may supply
    /// addresses but never membership: even a route derived from a perfectly
    /// valid signed ad — or hand-registered — is refused unless the target's
    /// **public key** is in this set.
    pub fn with_membership(mut self, membership: ClusterMembership) -> Self {
        self.membership = Some(membership);
        self
    }

    /// The membership gating this transport, if any.
    pub fn membership(&self) -> Option<&ClusterMembership> {
        self.membership.as_ref()
    }

    /// Point `shard` at a cluster member using a route learned from **signed
    /// discovery ads**, instead of hand-registering an address.
    ///
    /// Fails closed: a non-member, an unknown node, or a node whose lease has
    /// lapsed yields a [`RouteRejection`] and leaves the routing table untouched.
    pub fn route_from_directory(
        &mut self,
        shard: ShardId,
        node_key: &PubKey,
        dir: &RouteDirectory,
        now: u64,
    ) -> Result<PeerRoute, RouteRejection> {
        let route = dir.route_for(node_key, now)?;
        self.routes.insert(shard.0, route.clone());
        Ok(route)
    }

    /// Emit signed client redirects when a migration commits.
    pub fn with_redirects(mut self, redirector: Redirector) -> Self {
        self.redirector = Some(redirector);
        self
    }

    /// Record which players are connected here for `shard`. These are the
    /// sessions that get redirected when the shard moves.
    pub fn set_shard_players(&mut self, shard: ShardId, players: Vec<u64>) -> &mut Self {
        self.shard_players.insert(shard.0, players);
        self
    }

    /// Note that `player` is connected here on `shard`.
    pub fn track_player(&mut self, shard: ShardId, player: u64) -> &mut Self {
        let v = self.shard_players.entry(shard.0).or_default();
        if !v.contains(&player) {
            v.push(player);
        }
        self
    }

    /// Forget a player on `shard` — they disconnected, or they already followed
    /// the shard elsewhere. A player we no longer serve must not be minted a
    /// redirect: that would hand a live credential to a session that is gone.
    pub fn untrack_player(&mut self, shard: ShardId, player: u64) -> &mut Self {
        if let Some(v) = self.shard_players.get_mut(&shard.0) {
            v.retain(|p| *p != player);
        }
        self
    }

    /// The players currently tracked on `shard`.
    pub fn tracked_players(&self, shard: ShardId) -> Vec<u64> {
        self.shard_players.get(&shard.0).cloned().unwrap_or_default()
    }

    /// Take the redirects minted by committed migrations, clearing the queue.
    /// The caller ships each one down the affected player's existing (already
    /// authenticated) connection.
    pub fn take_redirects(&mut self) -> Vec<SignedRedirect> {
        std::mem::take(&mut self.pending_redirects)
    }

    /// Redirects awaiting delivery, without consuming them.
    pub fn pending_redirects(&self) -> &[SignedRedirect] {
        &self.pending_redirects
    }

    /// Override the per-exchange socket timeout (tests use a short one).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The authority table this transport releases from.
    pub fn authority(&self) -> ShardAuthority {
        self.authority.clone()
    }

    /// The route registered for a shard, if any.
    pub fn route(&self, shard: ShardId) -> Option<&PeerRoute> {
        self.routes.get(&shard.0)
    }

    /// Ask a member peer what it owns and how big it is.
    ///
    /// Fails closed exactly like a migration would: membership is checked before
    /// a socket is opened, the peer's key is **pinned** by the handshake, and any
    /// error yields `Err` rather than a half-known peer. A probe never moves a
    /// shard and never touches authority, so a failed probe costs only a stale
    /// view — the caller ([`crate::rebalance`]) treats that as "do not place
    /// work there".
    pub fn probe_peer(&self, route: &PeerRoute) -> Result<PeerStatus, HandoffError> {
        if let Some(m) = &self.membership {
            if !m.contains(&route.pubkey) {
                return Err(HandoffError::Auth(format!(
                    "node {} is not an authorized member of this cluster",
                    route.pubkey.to_hex()
                )));
            }
        }
        let addr = route
            .addr
            .to_socket_addrs()
            .map_err(|e| HandoffError::Transport(format!("bad peer address: {e}")))?
            .next()
            .ok_or_else(|| HandoffError::Transport("peer address resolved to nothing".into()))?;
        let mut sock = TcpStream::connect_timeout(&addr, self.timeout)
            .map_err(|e| HandoffError::Transport(format!("connect to {addr}: {e}")))?;
        sock.set_read_timeout(Some(self.timeout)).map_err(io_err)?;
        sock.set_write_timeout(Some(self.timeout)).map_err(io_err)?;
        sock.set_nodelay(true).ok();

        // Same mutual handshake as a handoff: the peer must prove the PINNED key.
        let _chan = client_handshake(&mut sock, &self.identity, &route.pubkey)?;
        write_frame(&mut sock, &Frame::StatusRequest)?;
        match read_frame(&mut sock)? {
            Frame::Status {
                shards,
                epochs,
                checkpoints,
                cpu_cores,
                ram_mb,
                max_shards,
                free_slots,
            } => Ok(PeerStatus {
                // Keep only refs the peer is entitled to speak about: a
                // checkpoint ref for a shard it does not claim tells us nothing
                // we should act on, and dropping it here stops a peer seeding a
                // survivor's cache with pointers to arbitrary blobs.
                checkpoints: checkpoints
                    .into_iter()
                    .filter(|c| shards.contains(&c.shard))
                    .collect(),
                node: route.pubkey,
                shards: shards.iter().copied().map(ShardId).collect(),
                epochs: epochs.into_iter().map(|(s, e)| (ShardId(s), e)).collect(),
                capacity: Capacity {
                    cpu_cores,
                    ram_mb,
                    bandwidth_mbps: 0,
                    free_slots,
                    max_shards,
                },
            }),
            Frame::Reject { reason } => Err(HandoffError::Rejected(reason)),
            other => Err(HandoffError::Rejected(format!(
                "expected Status, got {}",
                frame_name(&other)
            ))),
        }
    }

    /// Tell a peer that this node owns `shard` at a strictly higher epoch, so
    /// it should drop a stale claim. Returns whether the peer actually did.
    ///
    /// Used after a checkpoint restore to evict a returning zombie owner. It is
    /// refused before a socket opens unless we genuinely own the shard at an
    /// epoch above the peer's — the fence is an assertion of fact, and a node
    /// that cannot make it truthfully must not make it at all.
    pub fn fence_peer(
        &self,
        route: &PeerRoute,
        shard: ShardId,
        peer_epoch: u64,
    ) -> Result<bool, HandoffError> {
        if let Some(m) = &self.membership {
            if !m.contains(&route.pubkey) {
                return Err(HandoffError::Auth(format!(
                    "node {} is not an authorized member of this cluster",
                    route.pubkey.to_hex()
                )));
            }
        }
        let epoch = self.authority.epoch_of(shard).ok_or(HandoffError::NotOwner(shard))?;
        if epoch <= peer_epoch {
            return Err(HandoffError::Rejected(format!(
                "refusing to fence shard {shard}: we hold epoch {epoch}, peer holds {peer_epoch} \
                 — we are the stale one"
            )));
        }

        let addr = route
            .addr
            .to_socket_addrs()
            .map_err(|e| HandoffError::Transport(format!("bad peer address: {e}")))?
            .next()
            .ok_or_else(|| HandoffError::Transport("peer address resolved to nothing".into()))?;
        let mut sock = TcpStream::connect_timeout(&addr, self.timeout)
            .map_err(|e| HandoffError::Transport(format!("connect to {addr}: {e}")))?;
        sock.set_read_timeout(Some(self.timeout)).map_err(io_err)?;
        sock.set_write_timeout(Some(self.timeout)).map_err(io_err)?;
        sock.set_nodelay(true).ok();

        let chan = client_handshake(&mut sock, &self.identity, &route.pubkey)?;
        let signed = msg_bytes(b"mg-fence", &chan.transcript, shard.0, epoch, "");
        write_frame(
            &mut sock,
            &Frame::Fence {
                shard: shard.0,
                epoch,
                sig: self.identity.sign(&signed),
            },
        )?;
        match read_frame(&mut sock)? {
            Frame::FenceAck {
                shard: s,
                epoch: e,
                dropped,
            } if s == shard.0 && e == epoch => Ok(dropped),
            Frame::Reject { reason } => Err(HandoffError::Rejected(reason)),
            other => Err(HandoffError::Rejected(format!(
                "expected FenceAck, got {}",
                frame_name(&other)
            ))),
        }
    }

    /// Migrate a shard this node owns to the node registered for it.
    ///
    /// On `Ok(epoch)` the target is the authoritative owner at `epoch` and this
    /// node has released the shard. On **every** `Err` this node still owns the
    /// shard and still holds its state — see the failure table at the top of the
    /// module.
    pub fn migrate_shard(&mut self, shard: ShardId) -> Result<u64, HandoffError> {
        let route = self
            .routes
            .get(&shard.0)
            .cloned()
            .ok_or(HandoffError::NoRoute(shard))?;

        // Membership is checked BEFORE we connect or serialize anything. A node
        // that merely announced itself in the open discovery phonebook is not a
        // cluster member, and is never handed a shard — no matter how the route
        // got into the table.
        if let Some(m) = &self.membership {
            if !m.contains(&route.pubkey) {
                warn!(
                    shard = shard.0,
                    peer = %route.pubkey.to_hex(),
                    "refusing handoff: target is not an authorized cluster member"
                );
                return Err(HandoffError::Auth(format!(
                    "node {} is not an authorized member of this cluster",
                    route.pubkey.to_hex()
                )));
            }
        }

        if !self.authority.owns(shard) {
            return Err(HandoffError::NotOwner(shard));
        }
        let (epoch, state) = self
            .authority
            .begin_offer(shard.0)
            .ok_or(HandoffError::NotOwner(shard))?;

        match self.run_migration(shard, epoch, &state, &route) {
            Ok(()) => {
                // Only here — after a verified CommitAck — is authority given up.
                self.authority.release(shard.0, epoch);
                self.migrations
                    .push((shard, epoch, state.len(), route.pubkey.to_hex()));
                // The session follows the shard — but ONLY now, past the verified
                // CommitAck. Nothing below this line runs on a failed or
                // rolled-back migration, so a redirect can never point players at
                // a node that did not actually take the shard.
                self.emit_redirects(shard, epoch, &route);
                info!(
                    shard = shard.0,
                    epoch,
                    peer = %route.pubkey.to_hex(),
                    "shard migrated to peer node"
                );
                Ok(epoch)
            }
            Err(e) => {
                // Fail closed: authority is retained at the ORIGINAL epoch, with
                // state intact. The reserved epoch is burnt (high-water already
                // moved), which is exactly what stops a retry from colliding
                // with an offer the peer may yet have staged.
                warn!(
                    shard = shard.0,
                    epoch,
                    error = %e,
                    "cross-node handoff failed — source RETAINS authority"
                );
                Err(e)
            }
        }
    }

    /// Mint one redirect per player known to be connected here for `shard`.
    ///
    /// Called from exactly one place: the success arm of [`Self::migrate_shard`],
    /// after the target has acknowledged the commit. The players are removed from
    /// this node's tracking table, because they now belong to the target.
    fn emit_redirects(&mut self, shard: ShardId, epoch: u64, route: &PeerRoute) {
        let Some(redirector) = self.redirector.clone() else {
            self.shard_players.remove(&shard.0);
            return;
        };
        let players = self.shard_players.remove(&shard.0).unwrap_or_default();
        if players.is_empty() {
            return;
        }
        let now = now_secs();
        let minted = redirector.redirects_for(&self.identity, &players, shard, epoch, route, now);
        info!(
            shard = shard.0,
            epoch,
            players = minted.len(),
            peer = %route.pubkey.to_hex(),
            "issuing signed session redirects — players follow the shard"
        );
        self.pending_redirects.extend(minted);
    }

    /// The wire half of a migration. Any error leaves authority untouched.
    fn run_migration(
        &self,
        shard: ShardId,
        epoch: u64,
        state: &[u8],
        route: &PeerRoute,
    ) -> Result<(), HandoffError> {
        let addr = route
            .addr
            .to_socket_addrs()
            .map_err(|e| HandoffError::Transport(format!("bad peer address: {e}")))?
            .next()
            .ok_or_else(|| HandoffError::Transport("peer address resolved to nothing".into()))?;

        let mut sock = TcpStream::connect_timeout(&addr, self.timeout)
            .map_err(|e| HandoffError::Transport(format!("connect to {addr}: {e}")))?;
        sock.set_read_timeout(Some(self.timeout)).map_err(io_err)?;
        sock.set_write_timeout(Some(self.timeout)).map_err(io_err)?;
        sock.set_nodelay(true).ok();

        // 0. Prove who we are; require the peer to prove it is the PINNED key.
        let chan = client_handshake(&mut sock, &self.identity, &route.pubkey)?;

        // 1. PREPARE.
        let hash = Hash::of(state).to_hex();
        let offer = msg_bytes(b"mg-offer", &chan.transcript, shard.0, epoch, &hash);
        write_frame(
            &mut sock,
            &Frame::Offer {
                shard: shard.0,
                epoch,
                state: hex_encode(state),
                hash: hash.clone(),
                sig: self.identity.sign(&offer),
            },
        )?;

        // 2. Wait for a valid, correctly-signed ack for THIS shard+epoch+hash.
        match read_frame(&mut sock)? {
            Frame::Ack {
                shard: s,
                epoch: e,
                hash: h,
                sig,
            } => {
                if s != shard.0 || e != epoch || h != hash {
                    return Err(HandoffError::Rejected(format!(
                        "ack does not match the offer (got shard {s} epoch {e})"
                    )));
                }
                let signed = msg_bytes(b"mg-ack", &chan.transcript, s, e, &h);
                if !verify(&route.pubkey, &signed, &sig) {
                    return Err(HandoffError::Auth("ack signature does not verify".into()));
                }
            }
            Frame::Reject { reason } => return Err(HandoffError::Rejected(reason)),
            other => {
                return Err(HandoffError::Rejected(format!(
                    "expected Ack, got {}",
                    frame_name(&other)
                )))
            }
        }

        // 3. COMMIT — the target may now become authoritative.
        let commit = msg_bytes(b"mg-commit", &chan.transcript, shard.0, epoch, "");
        write_frame(
            &mut sock,
            &Frame::Commit {
                shard: shard.0,
                epoch,
                sig: self.identity.sign(&commit),
            },
        )?;

        // 4. Only a verified CommitAck lets the caller release authority.
        match read_frame(&mut sock)? {
            Frame::CommitAck {
                shard: s,
                epoch: e,
                sig,
            } => {
                if s != shard.0 || e != epoch {
                    return Err(HandoffError::Rejected(format!(
                        "commit-ack does not match (got shard {s} epoch {e})"
                    )));
                }
                let signed = msg_bytes(b"mg-commit-ack", &chan.transcript, s, e, "");
                if !verify(&route.pubkey, &signed, &sig) {
                    return Err(HandoffError::Auth(
                        "commit-ack signature does not verify".into(),
                    ));
                }
                Ok(())
            }
            Frame::Reject { reason } => Err(HandoffError::Rejected(reason)),
            other => Err(HandoffError::Rejected(format!(
                "expected CommitAck, got {}",
                frame_name(&other)
            ))),
        }
    }
}

impl HandoffTransport for NetworkHandoffTransport {
    fn transfer(
        &mut self,
        event: &HandoffEvent,
        player_state_blob: &[u8],
    ) -> Result<(), HandoffError> {
        // A player crossing into a shard hosted elsewhere migrates that shard's
        // state as the unit of authority. If we do not own the target shard's
        // source state there is nothing to hand over locally, so refresh the
        // stored state and run the two-phase move.
        let shard = event.to_shard;
        if !self.routes.contains_key(&shard.0) {
            return Err(HandoffError::NoRoute(shard));
        }
        if !self.authority.owns(shard) {
            self.authority.claim(shard, player_state_blob.to_vec());
        } else {
            self.authority.update_state(shard, player_state_blob.to_vec());
        }
        // The migrating player is by definition affected: if the move commits,
        // they get a signed redirect to the new owner. If it fails, `migrate_shard`
        // returns Err and no redirect is minted.
        self.track_player(shard, event.player.as_u64());
        self.migrate_shard(shard).map(|_| ())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::authority::{
        AuthoritativeGame, GameExecutor, MatchConfig, NativeExecutor, RejectReason, StepCtx, Tick,
        Topology,
    };
    use magnetite_sdk::input::{Input, MouseState};
    use magnetite_sdk::scaling::{
        Capacity, NodeCapacity, NodeId, ShardKey, ShardScheduler, SpreadScheduler,
    };
    use magnetite_sdk::state::PlayerId;

    fn ident(seed: u8) -> Arc<RawKeypairAuth> {
        Arc::new(RawKeypairAuth::from_seed([seed; 32]))
    }

    fn short() -> Duration {
        Duration::from_millis(600)
    }

    // ── A deterministic toy world used to prove determinism across migration ──

    struct Counter {
        acc: u64,
        steps: u64,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct Snap {
        acc: u64,
        steps: u64,
    }
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Delta {
        acc: u64,
    }
    #[derive(serde::Serialize)]
    struct View {
        acc: u64,
    }
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Cmd(u64);

    impl AuthoritativeGame for Counter {
        type Snapshot = Snap;
        type Delta = Delta;
        type View = View;
        type Command = Cmd;
        fn init(_c: &MatchConfig) -> Self {
            Counter { acc: 0, steps: 0 }
        }
        fn validate(&self, p: PlayerId, i: &Input, t: Tick) -> Result<Vec<Cmd>, RejectReason> {
            // Mix player, input, and tick so any lost/duplicated step shows up.
            Ok(vec![Cmd(
                p.as_u64() ^ (i.mouse.delta_x as i64 as u64).rotate_left(7) ^ t.rotate_left(19),
            )])
        }
        fn step(&mut self, ctx: &mut StepCtx, cmds: &[(PlayerId, Cmd)]) {
            for (_, c) in cmds {
                self.acc = self
                    .acc
                    .wrapping_mul(0x100_0000_01b3)
                    .wrapping_add(c.0 ^ ctx.tick);
            }
            self.steps += 1;
        }
        fn snapshot(&self) -> Snap {
            Snap {
                acc: self.acc,
                steps: self.steps,
            }
        }
        fn restore(s: &Snap, _c: &MatchConfig) -> Self {
            Counter {
                acc: s.acc,
                steps: s.steps,
            }
        }
        fn delta(&self, since: &Snap) -> Delta {
            Delta {
                acc: self.acc ^ since.acc,
            }
        }
        fn view_for(&self, _p: PlayerId) -> View {
            View { acc: self.acc }
        }
    }

    fn cfg() -> MatchConfig {
        MatchConfig {
            topology: Topology::Sharded {
                tick_hz: 20,
                cell_size: 100.0,
                max_per_shard: 64,
            },
            max_players: 256,
            tick_hz: 20,
            seed: 0x5EED,
            snapshot_every: 300,
        }
    }

    fn exec() -> NativeExecutor<Counter> {
        NativeExecutor::<Counter>::new(cfg())
    }

    fn input(dx: f64) -> Input {
        Input {
            mouse: MouseState {
                delta_x: dx,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    // ── 1. Peer authentication ────────────────────────────────────────────

    #[test]
    fn authenticated_handshake_succeeds_between_known_peers() {
        let a = ident(1);
        let b = ident(2);
        let allowed: HashSet<PubKey> = [a.node_pubkey()].into_iter().collect();
        let node_b = FleetNode::bind("127.0.0.1:0", Arc::clone(&b), Some(allowed)).unwrap();

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short());
        t.add_route(ShardId(7), node_b.route());
        t.authority().claim(ShardId(7), b"hello fleet".to_vec());

        let epoch = t.migrate_shard(ShardId(7)).expect("migration succeeds");
        assert_eq!(epoch, 2, "claim took epoch 1, the migration establishes 2");
        assert!(
            node_b.authority().owns(ShardId(7)),
            "target now holds authority"
        );
        assert_eq!(
            node_b.authority().state_of(ShardId(7)).unwrap(),
            b"hello fleet".to_vec(),
            "state arrived intact"
        );
        assert!(
            !t.authority().owns(ShardId(7)),
            "source released authority after the commit-ack"
        );
    }

    #[test]
    fn unauthorized_peer_is_rejected() {
        let good = ident(10);
        let stranger = ident(11);
        let server = ident(12);
        // Only `good` may hand shards to this node.
        let allowed: HashSet<PubKey> = [good.node_pubkey()].into_iter().collect();
        let node = FleetNode::bind("127.0.0.1:0", Arc::clone(&server), Some(allowed)).unwrap();

        let mut t = NetworkHandoffTransport::new(Arc::clone(&stranger), ShardAuthority::new())
            .with_timeout(short());
        t.add_route(ShardId(1), node.route());
        t.authority().claim(ShardId(1), b"state".to_vec());

        let err = t.migrate_shard(ShardId(1)).unwrap_err();
        assert!(
            matches!(err, HandoffError::Auth(_)),
            "an unknown key must be refused, got {err:?}"
        );
        assert!(
            t.authority().owns(ShardId(1)),
            "a rejected handoff leaves authority with the source"
        );
        assert!(!node.authority().owns(ShardId(1)));
    }

    #[test]
    fn wrong_peer_key_at_the_right_address_is_rejected() {
        let client = ident(20);
        let server = ident(21);
        let impostor_key = ident(22).node_pubkey();
        let node = FleetNode::bind("127.0.0.1:0", Arc::clone(&server), None).unwrap();

        let mut t = NetworkHandoffTransport::new(Arc::clone(&client), ShardAuthority::new())
            .with_timeout(short());
        // Right address, WRONG expected node key.
        t.add_route(
            ShardId(3),
            PeerRoute::new(node.addr().to_string(), impostor_key),
        );
        t.authority().claim(ShardId(3), b"secret world".to_vec());

        let err = t.migrate_shard(ShardId(3)).unwrap_err();
        assert!(
            matches!(err, HandoffError::Auth(ref m) if m.contains("mismatch")),
            "reaching the right address is not proof of the right node, got {err:?}"
        );
        assert!(t.authority().owns(ShardId(3)), "source keeps the shard");
        assert!(!node.authority().owns(ShardId(3)));
    }

    #[test]
    fn forged_client_signature_is_rejected() {
        // A client that presents someone else's pubkey but signs with its own.
        let victim = ident(30).node_pubkey();
        let attacker = ident(31);
        let server = ident(32);
        let node = FleetNode::bind("127.0.0.1:0", Arc::clone(&server), None).unwrap();

        let mut sock = TcpStream::connect(node.addr()).unwrap();
        sock.set_read_timeout(Some(short())).unwrap();
        sock.set_write_timeout(Some(short())).unwrap();
        let nonce = random_nonce();
        write_frame(
            &mut sock,
            &Frame::Hello {
                peer: victim, // claims the victim's identity
                nonce: hex_encode(&nonce),
                proto: PROTO.into(),
            },
        )
        .unwrap();
        let (spub, snonce) = match read_frame(&mut sock).unwrap() {
            Frame::ServerHello { peer, nonce, .. } => {
                let n: [u8; 32] = hex_decode(&nonce).unwrap().try_into().unwrap();
                (peer, n)
            }
            other => panic!("expected ServerHello, got {}", frame_name(&other)),
        };
        // Sign the transcript with the ATTACKER's key while claiming `victim`.
        let bytes = hs_bytes(b"mg-hs-client", &nonce, &snonce, &victim, &spub);
        write_frame(
            &mut sock,
            &Frame::ClientAuth {
                sig: attacker.sign(&bytes),
            },
        )
        .unwrap();
        match read_frame(&mut sock).unwrap() {
            Frame::Reject { reason } => assert!(reason.contains("signature")),
            other => panic!("forged auth must be rejected, got {}", frame_name(&other)),
        }
    }

    // ── 2. Two-phase safety / negatives ───────────────────────────────────

    /// A listener that authenticates honestly and then goes SILENT — the ack
    /// never arrives.
    fn silent_after_handshake(identity: Arc<RawKeypairAuth>) -> (std::net::SocketAddr, PubKey) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let pk = identity.node_pubkey();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = sock.set_write_timeout(Some(Duration::from_secs(5)));
                if server_handshake(&mut sock, &identity, None).is_ok() {
                    // Read the offer, then never answer.
                    let _ = read_frame(&mut sock);
                    std::thread::sleep(Duration::from_secs(3));
                }
            }
        });
        (addr, pk)
    }

    #[test]
    fn ack_timeout_leaves_authority_with_the_source() {
        let client = ident(40);
        let (addr, pk) = silent_after_handshake(ident(41));

        let mut t = NetworkHandoffTransport::new(Arc::clone(&client), ShardAuthority::new())
            .with_timeout(short());
        t.add_route(ShardId(5), PeerRoute::new(addr.to_string(), pk));
        t.authority().claim(ShardId(5), b"world state v1".to_vec());

        let err = t.migrate_shard(ShardId(5)).unwrap_err();
        assert!(
            matches!(err, HandoffError::Timeout(_)),
            "a missing ack must surface as a timeout, got {err:?}"
        );
        assert!(
            t.authority().owns(ShardId(5)),
            "ack timeout ⇒ source RETAINS authority (no split-brain, no orphan)"
        );
        assert_eq!(
            t.authority().state_of(ShardId(5)).unwrap(),
            b"world state v1".to_vec(),
            "and the state is not lost"
        );
    }

    /// A listener that authenticates and then hangs up immediately — the target
    /// "crashes" mid-handoff.
    fn crash_after_handshake(identity: Arc<RawKeypairAuth>) -> (std::net::SocketAddr, PubKey) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let pk = identity.node_pubkey();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = sock.set_write_timeout(Some(Duration::from_secs(5)));
                let _ = server_handshake(&mut sock, &identity, None);
                let _ = sock.shutdown(std::net::Shutdown::Both);
            }
        });
        (addr, pk)
    }

    #[test]
    fn target_crash_mid_handoff_leaves_the_shard_with_the_source() {
        let client = ident(50);
        let (addr, pk) = crash_after_handshake(ident(51));

        let mut t = NetworkHandoffTransport::new(Arc::clone(&client), ShardAuthority::new())
            .with_timeout(short());
        t.add_route(ShardId(9), PeerRoute::new(addr.to_string(), pk));
        t.authority().claim(ShardId(9), b"still mine".to_vec());

        assert!(t.migrate_shard(ShardId(9)).is_err());
        assert!(
            t.authority().owns(ShardId(9)),
            "a target that dies mid-handoff must not orphan the shard"
        );
        assert_eq!(
            t.authority().state_of(ShardId(9)).unwrap(),
            b"still mine".to_vec()
        );
    }

    #[test]
    fn no_route_fails_closed() {
        let t_ident = ident(60);
        let mut t = NetworkHandoffTransport::new(t_ident, ShardAuthority::new());
        t.authority().claim(ShardId(2), b"x".to_vec());
        assert!(matches!(
            t.migrate_shard(ShardId(2)),
            Err(HandoffError::NoRoute(_))
        ));
        assert!(t.authority().owns(ShardId(2)), "still ours");
    }

    #[test]
    fn migrating_a_shard_we_do_not_own_is_refused() {
        let client = ident(65);
        let node = FleetNode::bind("127.0.0.1:0", ident(66), None).unwrap();
        let mut t = NetworkHandoffTransport::new(Arc::clone(&client), ShardAuthority::new())
            .with_timeout(short());
        t.add_route(ShardId(4), node.route());
        assert!(matches!(
            t.migrate_shard(ShardId(4)),
            Err(HandoffError::NotOwner(_))
        ));
    }

    // ── 3. Replay / stale / duplicate ─────────────────────────────────────

    #[test]
    fn replayed_and_stale_handoffs_are_rejected_by_the_epoch_fence() {
        let auth = ShardAuthority::new();
        // A real migration lands epoch 5.
        auth.stage(1, 5, b"v5".to_vec()).unwrap();
        auth.commit(1, 5).unwrap();
        assert_eq!(auth.epoch_of(ShardId(1)), Some(5));

        // Exactly the same offer again (duplicate) — refused.
        let dup = auth.stage(1, 5, b"v5".to_vec()).unwrap_err();
        assert!(dup.contains("stale or replayed"), "{dup}");

        // A late-arriving OLDER handoff (replay of a stale owner) — refused.
        let stale = auth.stage(1, 3, b"v3".to_vec()).unwrap_err();
        assert!(stale.contains("stale or replayed"), "{stale}");

        // A commit for an epoch that was never staged — refused.
        assert!(auth.commit(1, 6).is_err());

        // The shard is still owned at the epoch that legitimately won.
        assert_eq!(auth.epoch_of(ShardId(1)), Some(5));
        assert_eq!(
            auth.state_of(ShardId(1)).unwrap(),
            b"v5".to_vec(),
            "a stale replay never overwrites live state"
        );
    }

    #[test]
    fn a_replayed_offer_over_the_wire_is_rejected() {
        let a = ident(70);
        let node_b = FleetNode::bind("127.0.0.1:0", ident(71), None).unwrap();

        let mut t =
            NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new()).with_timeout(short());
        t.add_route(ShardId(8), node_b.route());
        t.authority().claim(ShardId(8), b"first".to_vec());
        let epoch = t.migrate_shard(ShardId(8)).unwrap();

        // Now replay the SAME shard+epoch over a fresh authenticated channel.
        let route = node_b.route();
        let addr: std::net::SocketAddr = route.addr.parse().unwrap();
        let mut sock = TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(short())).unwrap();
        sock.set_write_timeout(Some(short())).unwrap();
        let chan = client_handshake(&mut sock, &a, &route.pubkey).unwrap();
        let state = b"replayed payload".to_vec();
        let hash = Hash::of(&state).to_hex();
        let signed = msg_bytes(b"mg-offer", &chan.transcript, 8, epoch, &hash);
        write_frame(
            &mut sock,
            &Frame::Offer {
                shard: 8,
                epoch,
                state: hex_encode(&state),
                hash,
                sig: a.sign(&signed),
            },
        )
        .unwrap();
        match read_frame(&mut sock).unwrap() {
            Frame::Reject { reason } => assert!(reason.contains("stale or replayed"), "{reason}"),
            other => panic!("replay must be rejected, got {}", frame_name(&other)),
        }
        assert_eq!(
            node_b.authority().state_of(ShardId(8)).unwrap(),
            b"first".to_vec(),
            "the replay did not overwrite the committed state"
        );
    }

    #[test]
    fn a_signature_from_another_session_does_not_transfer() {
        // Capture a valid offer signature from session 1 and try to use it in
        // session 2 — the transcript binding must break it.
        let a = ident(80);
        let node_b = FleetNode::bind("127.0.0.1:0", ident(81), None).unwrap();
        let route = node_b.route();
        let addr: std::net::SocketAddr = route.addr.parse().unwrap();

        let mut s1 = TcpStream::connect(addr).unwrap();
        s1.set_read_timeout(Some(short())).unwrap();
        s1.set_write_timeout(Some(short())).unwrap();
        let c1 = client_handshake(&mut s1, &a, &route.pubkey).unwrap();
        let state = b"cross-session".to_vec();
        let hash = Hash::of(&state).to_hex();
        let sig1 = a.sign(&msg_bytes(b"mg-offer", &c1.transcript, 12, 1, &hash));

        let mut s2 = TcpStream::connect(addr).unwrap();
        s2.set_read_timeout(Some(short())).unwrap();
        s2.set_write_timeout(Some(short())).unwrap();
        let _c2 = client_handshake(&mut s2, &a, &route.pubkey).unwrap();
        write_frame(
            &mut s2,
            &Frame::Offer {
                shard: 12,
                epoch: 1,
                state: hex_encode(&state),
                hash,
                sig: sig1, // signature from the OTHER session
            },
        )
        .unwrap();
        match read_frame(&mut s2).unwrap() {
            Frame::Reject { reason } => assert!(reason.contains("signature"), "{reason}"),
            other => panic!("cross-session replay must fail, got {}", frame_name(&other)),
        }
    }

    #[test]
    fn corrupted_state_is_rejected_before_it_is_staged() {
        let a = ident(85);
        let node_b = FleetNode::bind("127.0.0.1:0", ident(86), None).unwrap();
        let route = node_b.route();
        let addr: std::net::SocketAddr = route.addr.parse().unwrap();
        let mut sock = TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(short())).unwrap();
        sock.set_write_timeout(Some(short())).unwrap();
        let chan = client_handshake(&mut sock, &a, &route.pubkey).unwrap();

        // Declare the hash of one payload, ship a different one.
        let claimed = Hash::of(b"honest state").to_hex();
        let signed = msg_bytes(b"mg-offer", &chan.transcript, 13, 1, &claimed);
        write_frame(
            &mut sock,
            &Frame::Offer {
                shard: 13,
                epoch: 1,
                state: hex_encode(b"TAMPERED"),
                hash: claimed,
                sig: a.sign(&signed),
            },
        )
        .unwrap();
        match read_frame(&mut sock).unwrap() {
            Frame::Reject { reason } => assert!(reason.contains("hash mismatch"), "{reason}"),
            other => panic!("tampered state must be rejected, got {}", frame_name(&other)),
        }
        assert!(!node_b.authority().owns(ShardId(13)));
    }

    // ── 4. Determinism across the migration boundary ──────────────────────

    /// Drive `ticks` of the toy world through an executor, returning its final
    /// snapshot bytes. `start` lets a run resume from transferred state.
    fn run_ticks(exec: &mut dyn GameExecutor, from: Tick, to: Tick, players: &[PlayerId]) {
        for tick in from..=to {
            let inputs: Vec<(PlayerId, Input)> = players
                .iter()
                .enumerate()
                .map(|(i, p)| (*p, input(3.0 * (i as f64 + 1.0) + tick as f64)))
                .collect();
            exec.step(tick, &inputs);
        }
    }

    #[test]
    fn determinism_holds_across_a_real_cross_node_migration() {
        let players: Vec<PlayerId> = (1..=4).map(PlayerId::new).collect();

        // CONTROL: the whole run on one node, never migrated.
        let mut control = exec();
        run_ticks(&mut control, 1, 20, &players);
        let control_snap = control.snapshot();

        // MIGRATED: ticks 1..=10 on node A, hand the shard to node B, 11..=20 on B.
        let a_ident = ident(90);
        let node_b = FleetNode::bind("127.0.0.1:0", ident(91), None).unwrap();

        let mut src = exec();
        run_ticks(&mut src, 1, 10, &players);
        let mid = src.snapshot();

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a_ident), ShardAuthority::new())
            .with_timeout(short());
        t.add_route(ShardId(0), node_b.route());
        t.authority().claim(ShardId(0), mid.clone());
        t.migrate_shard(ShardId(0)).expect("migration succeeds");

        // Node B materializes the shard from the bytes it received.
        let received = node_b
            .authority()
            .state_of(ShardId(0))
            .expect("target owns the shard");
        assert_eq!(
            Hash::of(&received),
            Hash::of(&mid),
            "state hash is continuous across the migration boundary"
        );
        let mut dst = exec();
        dst.restore(&received);
        assert_eq!(
            Hash::of(&dst.snapshot()),
            Hash::of(&mid),
            "restore reproduces the source's exact state"
        );

        run_ticks(&mut dst, 11, 20, &players);
        assert_eq!(
            Hash::of(&dst.snapshot()),
            Hash::of(&control_snap),
            "a shard that migrated mid-run produces IDENTICAL results to one that never moved"
        );
    }

    #[test]
    fn a_failed_migration_does_not_disturb_determinism_either() {
        let players: Vec<PlayerId> = (1..=3).map(PlayerId::new).collect();
        let mut control = exec();
        run_ticks(&mut control, 1, 12, &players);

        // Same run, but a doomed migration attempt happens at tick 6.
        let mut src = exec();
        run_ticks(&mut src, 1, 6, &players);
        let (addr, pk) = crash_after_handshake(ident(96));
        let mut t = NetworkHandoffTransport::new(ident(95), ShardAuthority::new())
            .with_timeout(short());
        t.add_route(ShardId(0), PeerRoute::new(addr.to_string(), pk));
        t.authority().claim(ShardId(0), src.snapshot());
        assert!(t.migrate_shard(ShardId(0)).is_err());
        assert!(t.authority().owns(ShardId(0)), "we kept the shard");

        // The source keeps simulating; the world is unchanged by the failure.
        run_ticks(&mut src, 7, 12, &players);
        assert_eq!(
            Hash::of(&src.snapshot()),
            Hash::of(&control.snapshot()),
            "a failed handoff is invisible to the simulation"
        );
    }

    // ── 5. Multi-node placement + cross-shard traffic ─────────────────────

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
    fn a_world_spans_two_real_nodes_placed_by_capacity() {
        // Two real listeners on ephemeral ports.
        let a_ident = ident(100);
        let node_a = FleetNode::bind("127.0.0.1:0", Arc::clone(&a_ident), None).unwrap();
        let node_b = FleetNode::bind("127.0.0.1:0", ident(101), None).unwrap();

        // A BIGGER box must take MORE shards.
        let nodes = vec![
            NodeCapacity::new(node_a.pubkey().to_hex(), cap(6, 65536, 6)),
            NodeCapacity::new(node_b.pubkey().to_hex(), cap(2, 65536, 2)),
        ];
        let shards: Vec<ShardKey> = (0..8).map(ShardKey).collect();
        let placement = SpreadScheduler.place(&shards, &nodes);
        assert!(placement.unplaced.is_empty(), "all shards placed");

        let on_a = placement.shards_on(&NodeId(node_a.pubkey().to_hex()));
        let on_b = placement.shards_on(&NodeId(node_b.pubkey().to_hex()));
        assert_eq!(on_a.len(), 6);
        assert_eq!(on_b.len(), 2);
        assert!(
            on_a.len() > on_b.len(),
            "capacity-aware placement: the bigger box takes more shards"
        );

        // Node A starts owning everything, then ships B's share over the wire.
        let mut t = NetworkHandoffTransport::new(Arc::clone(&a_ident), ShardAuthority::new())
            .with_timeout(short());
        for key in &shards {
            let mut e = exec();
            run_ticks(&mut e, 1, 3, &[PlayerId::new(key.0 as u64 + 1)]);
            t.authority().claim(ShardId(key.0), e.snapshot());
        }
        for key in &on_b {
            t.add_route(ShardId(key.0), node_b.route());
            t.migrate_shard(ShardId(key.0))
                .unwrap_or_else(|e| panic!("shard {} must migrate: {e}", key.0));
        }

        // The world is genuinely split across two processes' authority tables.
        assert_eq!(t.authority().owned_shards().len(), 6);
        assert_eq!(node_b.authority().owned_shards().len(), 2);
        for key in &on_b {
            assert!(node_b.authority().owns(ShardId(key.0)));
            assert!(
                !t.authority().owns(ShardId(key.0)),
                "no shard is owned twice"
            );
        }
        for key in &on_a {
            assert!(t.authority().owns(ShardId(key.0)));
            assert!(!node_b.authority().owns(ShardId(key.0)));
        }
        assert_eq!(t.migrations.len(), 2, "two real cross-node transfers");
    }

    #[test]
    fn a_shard_can_bounce_back_and_forth_without_ambiguity() {
        // A → B → A, proving epochs keep increasing and ownership is exclusive.
        let a_ident = ident(110);
        let b_ident = ident(111);
        let node_a = FleetNode::bind("127.0.0.1:0", Arc::clone(&a_ident), None).unwrap();
        let node_b = FleetNode::bind("127.0.0.1:0", Arc::clone(&b_ident), None).unwrap();

        let mut a_tx =
            NetworkHandoffTransport::new(Arc::clone(&a_ident), node_a.authority()).with_timeout(short());
        a_tx.add_route(ShardId(1), node_b.route());
        let mut b_tx =
            NetworkHandoffTransport::new(Arc::clone(&b_ident), node_b.authority()).with_timeout(short());
        b_tx.add_route(ShardId(1), node_a.route());

        node_a.authority().claim(ShardId(1), b"round trip".to_vec());
        let e1 = a_tx.migrate_shard(ShardId(1)).unwrap();
        assert!(node_b.authority().owns(ShardId(1)));
        assert!(!node_a.authority().owns(ShardId(1)));

        let e2 = b_tx.migrate_shard(ShardId(1)).unwrap();
        assert!(e2 > e1, "epochs are strictly monotonic across the fleet");
        assert!(node_a.authority().owns(ShardId(1)));
        assert!(!node_b.authority().owns(ShardId(1)));
        assert_eq!(
            node_a.authority().state_of(ShardId(1)).unwrap(),
            b"round trip".to_vec(),
            "state survived the round trip byte-for-byte"
        );

        // The stale owner cannot hand it on again: it no longer owns it.
        assert!(matches!(
            b_tx.migrate_shard(ShardId(1)),
            Err(HandoffError::NotOwner(_))
        ));
    }

    #[test]
    fn handoff_transport_seam_drives_a_cross_node_move() {
        // Exercise the `HandoffTransport` trait itself (the seam the runtime
        // calls), not just `migrate_shard`.
        let a = ident(120);
        let node_b = FleetNode::bind("127.0.0.1:0", ident(121), None).unwrap();
        let mut t =
            NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new()).with_timeout(short());
        t.add_route(ShardId(2), node_b.route());

        let ev = HandoffEvent {
            player: PlayerId::new(1),
            from_shard: ShardId(1),
            to_shard: ShardId(2),
            target_addr: Some(node_b.addr().to_string()),
        };
        t.transfer(&ev, b"player+world blob").unwrap();
        assert!(node_b.authority().owns(ShardId(2)));

        // Without a route, the seam still fails closed.
        let bad = HandoffEvent {
            to_shard: ShardId(999),
            ..ev
        };
        assert!(matches!(
            t.transfer(&bad, b"blob"),
            Err(HandoffError::NoRoute(_))
        ));
    }

    #[test]
    fn oversized_frames_are_refused_not_truncated() {
        let a = ident(130);
        let node_b = FleetNode::bind("127.0.0.1:0", ident(131), None).unwrap();
        let route = node_b.route();
        let addr: std::net::SocketAddr = route.addr.parse().unwrap();
        let mut sock = TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(short())).unwrap();
        sock.set_write_timeout(Some(short())).unwrap();
        client_handshake(&mut sock, &a, &route.pubkey).unwrap();
        // Announce an absurd length; the peer must not allocate it.
        sock.write_all(&u32::MAX.to_be_bytes()).unwrap();
        // The connection is dropped; nothing was staged.
        std::thread::sleep(Duration::from_millis(100));
        assert!(!node_b.authority().owns(ShardId(0)));
    }

    // ── 6. Cluster membership + discovery-driven routes ───────────────────

    use crate::cluster::{
        AdmitError, ClusterMembership, FollowAdmission, RouteDirectory, RouteRejection, Redirector,
    };
    use magnetite_seams::discovery::{
        Capacity as AdCapacity, NodeAddr, SessionAd, SignedAd,
    };

    fn ad_at(id: &RawKeypairAuth, addr: &str, now: u64, ttl: u64) -> SignedAd {
        SignedAd::sign(
            id,
            SessionAd {
                game: Hash::of(b"snake"),
                node: NodeAddr(addr.into()),
                operator: None,
                region: None,
                capacity: AdCapacity {
                    cpu_cores: 8,
                    ram_mb: 16384,
                    bandwidth_mbps: 1000,
                    free_slots: 4,
                    max_shards: 32,
                },
                ping_hint: 20,
                price: None,
                chat_room: None,
                voice_room: None,
            },
            now,
            ttl,
        )
    }
    #[test]
    fn a_route_derived_from_a_signed_ad_drives_a_real_migration() {
        // No hand-registered addresses: the cluster configures itself from the
        // phonebook, gated by operator-authorized membership.
        let a = ident(140);
        let b = ident(141);
        let node_b = FleetNode::bind("127.0.0.1:0", Arc::clone(&b), None).unwrap();

        let membership =
            ClusterMembership::from_keys([a.node_pubkey(), node_b.pubkey()]);
        let mut dir = RouteDirectory::new(membership.clone());
        // B announces itself on the open phonebook, signing with its node key.
        dir.observe(&ad_at(&b, &node_b.addr().to_string(), 1_000, 60), 1_000)
            .expect("a member's ad becomes a route");

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short())
            .with_membership(membership);
        t.route_from_directory(ShardId(21), &node_b.pubkey(), &dir, 1_000)
            .expect("route derived from discovery");
        t.authority().claim(ShardId(21), b"self-configured".to_vec());

        t.migrate_shard(ShardId(21)).expect("migration succeeds");
        assert!(node_b.authority().owns(ShardId(21)));
        assert_eq!(
            node_b.authority().state_of(ShardId(21)).unwrap(),
            b"self-configured".to_vec()
        );
    }

    #[test]
    fn an_unauthorized_node_that_announces_is_never_handed_a_shard() {
        // The volunteer runs a REAL, willing listener and announces a REAL,
        // correctly-signed ad. It must still receive nothing.
        let a = ident(150);
        let volunteer = ident(151);
        let volunteer_node =
            FleetNode::bind("127.0.0.1:0", Arc::clone(&volunteer), None).unwrap();

        let membership = ClusterMembership::from_keys([a.node_pubkey()]);
        let mut dir = RouteDirectory::new(membership.clone());
        let err = dir
            .observe(
                &ad_at(&volunteer, &volunteer_node.addr().to_string(), 1_000, 60),
                1_000,
            )
            .unwrap_err();
        assert!(matches!(err, RouteRejection::NotAMember(_)));

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short())
            .with_membership(membership);
        // No route can be derived…
        assert!(t
            .route_from_directory(ShardId(22), &volunteer_node.pubkey(), &dir, 1_000)
            .is_err());
        // …and even hand-registering one does not help: membership is re-checked
        // at migration time, before a single byte of state leaves this box.
        t.add_route(ShardId(22), volunteer_node.route());
        t.authority().claim(ShardId(22), b"do not exfiltrate".to_vec());
        let err = t.migrate_shard(ShardId(22)).unwrap_err();
        assert!(
            matches!(err, HandoffError::Auth(ref m) if m.contains("not an authorized member")),
            "volunteering must not make you eligible, got {err:?}"
        );
        assert!(t.authority().owns(ShardId(22)), "state stayed put");
        assert!(!volunteer_node.authority().owns(ShardId(22)));
    }

    #[test]
    fn an_ad_whose_key_differs_from_the_key_on_the_wire_is_rejected() {
        // The ad is signed by a member, but the box actually listening at that
        // address holds a different key. Key pinning must abort the handoff.
        let a = ident(160);
        let member = ident(161);
        let impostor = ident(162);
        let impostor_node = FleetNode::bind("127.0.0.1:0", Arc::clone(&impostor), None).unwrap();

        let membership = ClusterMembership::from_keys([a.node_pubkey(), member.node_pubkey()]);
        let mut dir = RouteDirectory::new(membership.clone());
        // The member announces the impostor's ADDRESS (or the address was taken
        // over). The pinned key is still the member's, from the signed ad.
        let route = dir
            .observe(
                &ad_at(&member, &impostor_node.addr().to_string(), 1_000, 60),
                1_000,
            )
            .unwrap();
        assert_eq!(route.pubkey, member.node_pubkey());

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short())
            .with_membership(membership);
        t.route_from_directory(ShardId(23), &member.node_pubkey(), &dir, 1_000)
            .unwrap();
        t.authority().claim(ShardId(23), b"pinned".to_vec());
        let err = t.migrate_shard(ShardId(23)).unwrap_err();
        assert!(
            matches!(err, HandoffError::Auth(ref m) if m.contains("mismatch")),
            "the address is not the identity, got {err:?}"
        );
        assert!(t.authority().owns(ShardId(23)));
        assert!(!impostor_node.authority().owns(ShardId(23)));
    }

    #[test]
    fn a_lapsed_ad_is_not_routed_to() {
        let a = ident(170);
        let b = ident(171);
        let node_b = FleetNode::bind("127.0.0.1:0", Arc::clone(&b), None).unwrap();
        let membership = ClusterMembership::from_keys([a.node_pubkey(), node_b.pubkey()]);
        let mut dir = RouteDirectory::new(membership.clone());
        dir.observe(&ad_at(&b, &node_b.addr().to_string(), 1_000, 60), 1_000)
            .unwrap();

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short())
            .with_membership(membership);
        assert!(matches!(
            t.route_from_directory(ShardId(24), &node_b.pubkey(), &dir, 5_000),
            Err(RouteRejection::Expired)
        ));
        t.authority().claim(ShardId(24), b"x".to_vec());
        assert!(matches!(
            t.migrate_shard(ShardId(24)),
            Err(HandoffError::NoRoute(_))
        ));
    }

    // ── 7. The player's session follows the shard ─────────────────────────

    #[test]
    fn a_committed_migration_redirects_the_connected_players_to_the_new_owner() {
        let a = ident(180);
        let b = ident(181);
        let node_b = FleetNode::bind("127.0.0.1:0", Arc::clone(&b), None).unwrap();
        let membership = ClusterMembership::from_keys([a.node_pubkey(), node_b.pubkey()]);

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short())
            .with_membership(membership.clone())
            .with_redirects(Redirector::new());
        t.add_route(ShardId(30), node_b.route());
        t.set_shard_players(ShardId(30), vec![7, 8]);
        t.authority().claim(ShardId(30), b"world".to_vec());

        let epoch = t.migrate_shard(ShardId(30)).unwrap();
        let redirects = t.take_redirects();
        assert_eq!(redirects.len(), 2, "both connected players are redirected");
        assert!(t.take_redirects().is_empty(), "the queue drains");

        let now = now_secs();
        let mut door = FollowAdmission::new(node_b.pubkey(), membership);
        for r in &redirects {
            // Client side: the redirect came from the node it is talking to, and
            // pins B's key.
            let route = r.verify_for(&a.node_pubkey(), r.player, now).unwrap();
            assert_eq!(route.pubkey, node_b.pubkey());
            assert_eq!(route.addr, node_b.addr().to_string());
            // Target side: B genuinely owns the shard at this epoch.
            door.admit(
                &r.token,
                r.player,
                ShardId(30),
                node_b.authority().epoch_of(ShardId(30)),
                now,
            )
            .unwrap();
        }
        assert_eq!(node_b.authority().epoch_of(ShardId(30)), Some(epoch));
    }

    #[test]
    fn a_failed_migration_emits_no_redirect_at_all() {
        // The rollback path must not leak a redirect pointing players at a node
        // that never took the shard.
        let a = ident(190);
        let (addr, pk) = crash_after_handshake(ident(191));
        let membership = ClusterMembership::from_keys([a.node_pubkey(), pk]);

        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short())
            .with_membership(membership)
            .with_redirects(Redirector::new());
        t.add_route(ShardId(31), PeerRoute::new(addr.to_string(), pk));
        t.set_shard_players(ShardId(31), vec![7]);
        t.authority().claim(ShardId(31), b"still here".to_vec());

        assert!(t.migrate_shard(ShardId(31)).is_err());
        assert!(
            t.take_redirects().is_empty(),
            "a rolled-back migration must never redirect anyone"
        );
        assert!(t.authority().owns(ShardId(31)), "and we keep the shard");
    }

    #[test]
    fn a_redirect_from_a_superseded_migration_is_refused_by_the_new_owner() {
        // A → B (epoch e1) issues a redirect; the shard then moves B → A (e2).
        // A player who follows the stale redirect must be refused by B.
        let a_ident = ident(200);
        let b_ident = ident(201);
        let node_a = FleetNode::bind("127.0.0.1:0", Arc::clone(&a_ident), None).unwrap();
        let node_b = FleetNode::bind("127.0.0.1:0", Arc::clone(&b_ident), None).unwrap();
        let membership = ClusterMembership::from_keys([node_a.pubkey(), node_b.pubkey()]);

        let mut a_tx = NetworkHandoffTransport::new(Arc::clone(&a_ident), node_a.authority())
            .with_timeout(short())
            .with_membership(membership.clone())
            .with_redirects(Redirector::new());
        a_tx.add_route(ShardId(32), node_b.route());
        a_tx.set_shard_players(ShardId(32), vec![5]);
        node_a.authority().claim(ShardId(32), b"bouncing".to_vec());
        a_tx.migrate_shard(ShardId(32)).unwrap();
        let stale = a_tx.take_redirects().pop().unwrap();

        // Now it bounces back to A, so B's ownership at the redirect's epoch ends.
        let mut b_tx = NetworkHandoffTransport::new(Arc::clone(&b_ident), node_b.authority())
            .with_timeout(short())
            .with_membership(membership.clone());
        b_tx.add_route(ShardId(32), node_a.route());
        b_tx.migrate_shard(ShardId(32)).unwrap();

        let mut door = FollowAdmission::new(node_b.pubkey(), membership);
        let err = door
            .admit(
                &stale.token,
                5,
                ShardId(32),
                node_b.authority().epoch_of(ShardId(32)),
                now_secs(),
            )
            .unwrap_err();
        assert!(
            matches!(err, AdmitError::StaleEpoch { .. }),
            "the epoch fence covers session-follow too, got {err:?}"
        );
    }

    #[test]
    fn the_seam_path_redirects_the_migrating_player() {
        let a = ident(210);
        let node_b = FleetNode::bind("127.0.0.1:0", ident(211), None).unwrap();
        let membership = ClusterMembership::from_keys([a.node_pubkey(), node_b.pubkey()]);
        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), ShardAuthority::new())
            .with_timeout(short())
            .with_membership(membership)
            .with_redirects(Redirector::new());
        t.add_route(ShardId(33), node_b.route());

        let ev = HandoffEvent {
            player: PlayerId::new(9),
            from_shard: ShardId(1),
            to_shard: ShardId(33),
            target_addr: None,
        };
        t.transfer(&ev, b"blob").unwrap();
        let rs = t.take_redirects();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].player, 9, "the player that crossed follows the shard");
        assert_eq!(rs[0].target_key, node_b.pubkey());
    }

    #[test]
    fn hex_roundtrips() {
        let data = b"\x00\xff\x10 magnetite";
        assert_eq!(hex_decode(&hex_encode(data)).unwrap(), data.to_vec());
        assert!(hex_decode("abc").is_err());
        assert!(hex_decode("zz").is_err());
    }

    #[test]
    fn nonces_are_unique() {
        let a = random_nonce();
        let b = random_nonce();
        assert_ne!(a, b, "handshake nonces must not repeat");
    }

    #[test]
    fn update_state_requires_ownership() {
        let auth = ShardAuthority::new();
        assert!(!auth.update_state(ShardId(1), b"x".to_vec()));
        auth.claim(ShardId(1), b"a".to_vec());
        assert!(auth.update_state(ShardId(1), b"b".to_vec()));
        assert_eq!(auth.state_of(ShardId(1)).unwrap(), b"b".to_vec());
    }
}
