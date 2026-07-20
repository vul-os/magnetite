//! Session follow — the wiring that makes a player's *connection* follow a
//! migrated shard.
//!
//! The cryptographic mechanism lives in [`crate::cluster`] (`SignedRedirect`,
//! `FollowToken`, `FollowAdmission`) and the migration itself in
//! [`crate::fleet`]. This module is deliberately thin: it is the glue that lets
//! the node's own WebSocket listener ([`crate::server`]) drive that mechanism,
//! and it reimplements **none** of the checks.
//!
//! ```text
//!  player ── ws ──► node A                              node B
//!                    │  track_player(shard, player)
//!                    │
//!                    │  migrate_shard  ── two-phase ──►  stages, CommitAck
//!                    │  (success arm only)
//!                    │  take_redirects()
//!   ◄── ServerNet::Redirect{SignedRedirect} ──┘
//!   (client verifies issuer sig + expiry, pins target_key)
//!
//!  player ── ws ──────────────────────────────────────►  node B
//!   ── ClientNet::Hello{nonce} ─────────────────────────►
//!   ◄── ServerNet::NodeIdentity{node_key,nonce,sig} ─────  (client pins)
//!   ── ClientNet::Follow{SignedRedirect} ───────────────►  FollowAdmission::admit
//!   ◄── ServerNet::Welcome{same player_id} ──────────────  attached, continuous
//! ```
//!
//! ## What is enforced here
//!
//! Nothing new. [`FleetSession::admit_follow`] hands the token straight to
//! [`crate::cluster::FollowAdmission::admit`], which refuses — in order — a
//! token for another node, an issuer outside the operator-authorized
//! membership, a bad signature, an expired token, another player, another
//! shard, an epoch this node does not own, and a replayed nonce. Every one of
//! those is a refusal to attach the connection: the door is fail-closed and
//! this module never opens a side entrance.
//!
//! ## What is NOT covered
//!
//! - **NAT traversal.** The redirect carries an address the client must be able
//!   to reach directly. Nodes behind NAT are out of scope.
//! - **Channel binding.** [`ClientNet::Hello`](magnetite_sdk::protocol::ClientNet::Hello)
//!   proves the node holds its key, which is what makes redirect verification
//!   meaningful. It does not bind that proof to the transport, so a relay that
//!   can sit in the middle of a plaintext `ws://` connection is not defeated by
//!   it. Run behind TLS.

use std::sync::{Arc, Mutex};

use magnetite_seams::identity::{Identity, PubKey, RawKeypairAuth};

use crate::cluster::{AdmitError, ClusterMembership, FollowAdmission, SignedRedirect};
use crate::fleet::{NetworkHandoffTransport, ShardAuthority};
use crate::shard::ShardId;

/// Domain separator for the node-identity proof. Distinct from every other
/// signing domain in the protocol, so a `NodeIdentity` signature can never be
/// replayed as a redirect, a follow token, or a discovery ad.
pub const NODE_HELLO_DOMAIN: &[u8] = b"magnetite-node-hello-v1";

/// Bytes signed by a node answering [`ClientNet::Hello`](magnetite_sdk::protocol::ClientNet::Hello).
pub fn node_hello_bytes(nonce: &str, node_key: &PubKey) -> Vec<u8> {
    let mut b = Vec::with_capacity(NODE_HELLO_DOMAIN.len() + nonce.len() + 32);
    b.extend_from_slice(NODE_HELLO_DOMAIN);
    b.extend_from_slice(nonce.as_bytes());
    b.extend_from_slice(&node_key.0);
    b
}

/// Why a follow presented on a socket was refused.
///
/// Everything but [`FollowRefusal::Admission`] is malformed input; `Admission`
/// is the real gate and carries the [`AdmitError`] verbatim.
#[derive(Debug)]
pub enum FollowRefusal {
    /// The frame did not contain a well-formed `SignedRedirect`.
    Malformed(String),
    /// The redirect was refused by [`FollowAdmission::admit`].
    Admission(AdmitError),
}

impl std::fmt::Display for FollowRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FollowRefusal::Malformed(m) => write!(f, "malformed follow redirect: {m}"),
            FollowRefusal::Admission(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FollowRefusal {}

/// Per-node fleet state shared by the WebSocket listener and the migration path.
///
/// Cheap to clone — every field is shared. Attach one to
/// [`crate::GameServerConfig::fleet`] and the node's own listener will:
/// 1. track connected players per shard,
/// 2. deliver signed redirects on the live socket when a shard migrates away,
/// 3. admit incoming follows through [`FollowAdmission`] before attaching them.
#[derive(Clone)]
pub struct FleetSession {
    identity: Arc<RawKeypairAuth>,
    authority: ShardAuthority,
    /// Source side: holds `shard_players` and the redirect queue. Optional so a
    /// node can be follow-*target*-only.
    transport: Option<Arc<Mutex<NetworkHandoffTransport>>>,
    /// Target side: the admission door.
    admission: Arc<Mutex<FollowAdmission>>,
}

impl FleetSession {
    /// Build a session for a node with `identity`, owning shards tracked in
    /// `authority`, admitting follows from members of `membership`.
    pub fn new(
        identity: Arc<RawKeypairAuth>,
        authority: ShardAuthority,
        membership: ClusterMembership,
    ) -> Self {
        let node_key = identity.node_pubkey();
        Self {
            identity,
            authority,
            transport: None,
            admission: Arc::new(Mutex::new(FollowAdmission::new(node_key, membership))),
        }
    }

    /// Attach the outbound migration transport, making this node a redirect
    /// *source* as well as a follow target.
    ///
    /// The transport must be the same instance migrations are driven through —
    /// that is what couples "this player is connected here" to "this player gets
    /// a redirect when the shard leaves".
    pub fn with_transport(mut self, transport: Arc<Mutex<NetworkHandoffTransport>>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// This node's public key.
    pub fn node_key(&self) -> PubKey {
        self.identity.node_pubkey()
    }

    /// Sign a client's `Hello` nonce, proving possession of the node key.
    pub fn sign_hello(&self, nonce: &str) -> String {
        let key = self.node_key();
        let sig = self.identity.sign(&node_hello_bytes(nonce, &key));
        hex_of(&sig.0)
    }

    /// The shard authority table this node owns shards in.
    pub fn authority(&self) -> ShardAuthority {
        self.authority.clone()
    }

    /// The migration transport, if this node is a redirect source.
    pub fn transport(&self) -> Option<Arc<Mutex<NetworkHandoffTransport>>> {
        self.transport.clone()
    }

    /// Note that `player` is connected here on `shard` — so they are redirected
    /// when it moves.
    pub fn attach_player(&self, shard: ShardId, player: u64) {
        if let Some(t) = &self.transport {
            if let Ok(mut t) = t.lock() {
                t.track_player(shard, player);
            }
        }
    }

    /// Forget `player` on `shard` (disconnect, or a completed follow).
    pub fn detach_player(&self, shard: ShardId, player: u64) {
        if let Some(t) = &self.transport {
            if let Ok(mut t) = t.lock() {
                t.untrack_player(shard, player);
            }
        }
    }

    /// Players currently tracked on `shard`.
    pub fn tracked_players(&self, shard: ShardId) -> Vec<u64> {
        match &self.transport {
            Some(t) => t
                .lock()
                .map(|t| t.tracked_players(shard))
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Drain redirects minted by committed migrations, for delivery on the
    /// affected players' live sockets.
    ///
    /// These only ever exist past a verified `CommitAck` — see
    /// `NetworkHandoffTransport::migrate_shard`. A failed or rolled-back
    /// migration puts nothing in this queue, so this drain cannot deliver one.
    pub fn drain_redirects(&self) -> Vec<SignedRedirect> {
        match &self.transport {
            Some(t) => t.lock().map(|mut t| t.take_redirects()).unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// **Target side.** Admit a player presenting `redirect`.
    ///
    /// Delegates wholly to [`FollowAdmission::admit`] against the epoch this
    /// node actually owns for the shard. Returns the `(player, shard)` the
    /// caller may then attach — and only on success.
    pub fn admit_follow(
        &self,
        redirect: &SignedRedirect,
        now: u64,
    ) -> Result<(u64, ShardId), FollowRefusal> {
        let shard = ShardId(redirect.shard);
        let owned = self.authority.epoch_of(shard);
        let mut door = self
            .admission
            .lock()
            .map_err(|_| FollowRefusal::Malformed("admission door poisoned".into()))?;
        door.admit(&redirect.token, redirect.player, shard, owned, now)
            .map_err(FollowRefusal::Admission)?;
        Ok((redirect.player, shard))
    }

    /// Parse a `ClientNet::Follow` payload and admit it in one step.
    pub fn admit_follow_json(
        &self,
        value: &serde_json::Value,
        now: u64,
    ) -> Result<(u64, ShardId), FollowRefusal> {
        let redirect: SignedRedirect = serde_json::from_value(value.clone())
            .map_err(|e| FollowRefusal::Malformed(e.to_string()))?;
        self.admit_follow(&redirect, now)
    }

    /// Mutable membership, so a revoked node stops being able to send us
    /// players on the very next admission.
    pub fn revoke_member(&self, key: &PubKey) {
        if let Ok(mut d) = self.admission.lock() {
            d.membership_mut().revoke(key);
        }
    }

    /// How many follow tokens this node has redeemed.
    pub fn redeemed_count(&self) -> usize {
        self.admission.lock().map(|d| d.redeemed_count()).unwrap_or(0)
    }
}

impl std::fmt::Debug for FleetSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FleetSession")
            .field("node_key", &self.node_key().to_hex())
            .field("is_redirect_source", &self.transport.is_some())
            .finish()
    }
}

fn hex_of(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Current unix seconds.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::{Redirector, SignedRedirect};
    use crate::fleet::PeerRoute;

    fn ident(seed: u8) -> Arc<RawKeypairAuth> {
        Arc::new(RawKeypairAuth::from_seed([seed; 32]))
    }

    fn session_for(node: &Arc<RawKeypairAuth>, members: Vec<PubKey>) -> FleetSession {
        FleetSession::new(
            Arc::clone(node),
            ShardAuthority::new(),
            ClusterMembership::from_keys(members),
        )
    }

    #[test]
    fn hello_proof_verifies_against_the_node_key_and_nothing_else() {
        let node = ident(1);
        let s = session_for(&node, vec![node.node_pubkey()]);
        let sig_hex = s.sign_hello("client-nonce");
        let raw = (0..sig_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&sig_hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&raw);
        let sig = magnetite_seams::identity::Sig(sig);

        assert!(<RawKeypairAuth as Identity>::verify(
            &s.node_key(),
            &node_hello_bytes("client-nonce", &s.node_key()),
            &sig
        ));
        // A different nonce is a different statement — no replay onto a fresh
        // challenge.
        assert!(!<RawKeypairAuth as Identity>::verify(
            &s.node_key(),
            &node_hello_bytes("other-nonce", &s.node_key()),
            &sig
        ));
        // And it is not a valid statement by any other key.
        assert!(!<RawKeypairAuth as Identity>::verify(
            &ident(2).node_pubkey(),
            &node_hello_bytes("client-nonce", &s.node_key()),
            &sig
        ));
    }

    #[test]
    fn admit_follow_is_the_cluster_door_not_a_new_one() {
        let a = ident(1);
        let b = ident(2);
        let target = session_for(&b, vec![a.node_pubkey(), b.node_pubkey()]);
        // B genuinely owns shard 4.
        target.authority().claim(ShardId(4), b"state".to_vec());
        let epoch = target.authority().epoch_of(ShardId(4)).unwrap();

        let route = PeerRoute::new("127.0.0.1:1", b.node_pubkey());
        let now = now_secs();
        let r = SignedRedirect::mint(&a, 77, ShardId(4), epoch, &route, now, 30);

        assert_eq!(target.admit_follow(&r, now).unwrap(), (77, ShardId(4)));
        // One shot.
        assert!(matches!(
            target.admit_follow(&r, now),
            Err(FollowRefusal::Admission(AdmitError::Replayed))
        ));
    }

    #[test]
    fn a_non_member_issuer_cannot_send_us_players() {
        let outsider = ident(66);
        let b = ident(2);
        let target = session_for(&b, vec![b.node_pubkey()]);
        target.authority().claim(ShardId(4), b"state".to_vec());
        let epoch = target.authority().epoch_of(ShardId(4)).unwrap();

        let route = PeerRoute::new("127.0.0.1:1", b.node_pubkey());
        let now = now_secs();
        let r = SignedRedirect::mint(&outsider, 77, ShardId(4), epoch, &route, now, 30);

        assert!(matches!(
            target.admit_follow(&r, now),
            Err(FollowRefusal::Admission(AdmitError::IssuerNotAMember(_)))
        ));
    }

    #[test]
    fn tracking_follows_connect_and_disconnect() {
        let a = ident(1);
        let authority = ShardAuthority::new();
        authority.claim(ShardId(9), b"s".to_vec());
        let transport = Arc::new(Mutex::new(NetworkHandoffTransport::new(
            Arc::clone(&a),
            authority.clone(),
        )));
        let s = FleetSession::new(
            Arc::clone(&a),
            authority,
            ClusterMembership::from_keys([a.node_pubkey()]),
        )
        .with_transport(transport);

        s.attach_player(ShardId(9), 1);
        s.attach_player(ShardId(9), 2);
        s.attach_player(ShardId(9), 1); // idempotent
        assert_eq!(s.tracked_players(ShardId(9)), vec![1, 2]);

        s.detach_player(ShardId(9), 1);
        assert_eq!(s.tracked_players(ShardId(9)), vec![2]);
    }

    #[test]
    fn a_node_with_no_transport_mints_nothing() {
        let a = ident(1);
        let s = session_for(&a, vec![a.node_pubkey()]);
        s.attach_player(ShardId(1), 5);
        assert!(s.tracked_players(ShardId(1)).is_empty());
        assert!(s.drain_redirects().is_empty());
    }

    #[test]
    fn redirects_only_appear_after_a_committed_migration() {
        let a = ident(1);
        let b = ident(2);
        let authority = ShardAuthority::new();
        authority.claim(ShardId(3), b"s".to_vec());
        // Route points at a port nobody is listening on: the migration cannot
        // commit.
        let mut t = NetworkHandoffTransport::new(Arc::clone(&a), authority.clone())
            .with_membership(ClusterMembership::from_keys([b.node_pubkey()]))
            .with_redirects(Redirector::new())
            .with_timeout(std::time::Duration::from_millis(50));
        t.add_route(ShardId(3), PeerRoute::new("127.0.0.1:1", b.node_pubkey()));
        let transport = Arc::new(Mutex::new(t));
        let s = FleetSession::new(
            Arc::clone(&a),
            authority,
            ClusterMembership::from_keys([a.node_pubkey()]),
        )
        .with_transport(Arc::clone(&transport));

        s.attach_player(ShardId(3), 42);
        assert!(transport.lock().unwrap().migrate_shard(ShardId(3)).is_err());
        assert!(
            s.drain_redirects().is_empty(),
            "a failed migration must mint no redirect"
        );
    }
}
