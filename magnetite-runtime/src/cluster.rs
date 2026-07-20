//! Cluster membership, discovery-driven routing, and player session-follow.
//!
//! [`crate::fleet`] gave a fleet an authenticated wire and an epoch-fenced
//! two-phase handoff. Two things were still missing, and this module supplies
//! exactly those two — without loosening anything `fleet` established.
//!
//! ## 1. A cluster that configures itself, but does not admit strangers
//!
//! Routes used to be hand-registered ([`NetworkHandoffTransport::add_route`]).
//! [`RouteDirectory`] derives them instead from the signed [`SignedAd`]s already
//! flowing through the [`magnetite_seams::discovery`] phonebook.
//!
//! **Discovery is an open phonebook. Anyone may announce.** So a derived route is
//! *not* permission to receive a shard. The rule this module enforces is:
//!
//! > Discovery may supply an **address**. Only the operator may confer
//! > **membership**, and membership is keyed on the node's **public key**.
//!
//! Concretely:
//!
//! * [`ClusterMembership`] is the operator-authorized set of node keys. It is a
//!   deny-by-default allowlist: an empty membership authorizes **nobody**, so a
//!   misconfigured node hands shards to no one rather than to anyone.
//! * [`RouteDirectory::observe`] refuses an ad whose signature does not verify,
//!   whose lease has lapsed, or whose `node_key` is not a member — in that order,
//!   and it never learns an address from a rejected ad.
//! * The pinned `pubkey` of a derived [`PeerRoute`] comes from the **signed ad**,
//!   never from the address. The `fleet` handshake still aborts if the far side
//!   presents a different key, so a hijacked address gets nothing.
//! * [`NetworkHandoffTransport::with_membership`] re-checks membership at
//!   migration time, so even a *hand-registered* route to a non-member is
//!   refused. Announcing that you host a game therefore never makes you eligible
//!   to receive shards of a world you were not admitted to.
//!
//! ## 2. The player's session follows the shard
//!
//! When shard `S` migrates from node A to node B, the players connected to A are
//! talking to a node that no longer owns their shard. A **redirects** them.
//!
//! It is a redirect, not a proxy: proxying would make the source a permanent
//! middleman and defeat the point of moving the shard at all.
//!
//! ```text
//! A (source, already authenticated to the client)
//!   ── SignedRedirect{ shard, epoch, addr, target_key, FollowToken, exp, sig_A } ──► client
//! client   verifies sig_A against the node key it ALREADY authenticated
//!          (a redirect from anyone else is discarded)
//!   ── connect(addr), pin target_key ────────────────────────────────────────────► B
//!          B presenting any other key aborts the connection
//!   ── FollowToken ──────────────────────────────────────────────────────────────► B
//! B   admits only if: token.target == B's own key
//!                     token.issuer is a CLUSTER MEMBER
//!                     sig verifies, token unexpired, nonce unused
//!                     token.player == the connecting player
//!                     token.shard  == the shard being joined
//!                     token.epoch  == the epoch B actually owns right now
//! ```
//!
//! Every one of those is a refusal, not a downgrade. In particular a forged
//! redirect cannot hijack a player to an attacker's node (the client checks the
//! issuer), and a token minted for player X, shard S, or a superseded epoch will
//! not admit player Y, shard T, or a stale migration.
//!
//! Redirects are minted **only** in the success path of a migration — after the
//! `CommitAck` that lets the source release authority. A migration that fails or
//! is rolled back (see `ShardedRuntime::step_with_transport`) emits none.
//!
//! ## Not solved here
//!
//! NAT traversal. There is still no hole punching and no relay: nodes must be
//! mutually reachable, and clients must be able to reach the target's address.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use magnetite_seams::discovery::SignedAd;
use magnetite_seams::identity::{Identity, PubKey, RawKeypairAuth, Sig};

use crate::fleet::PeerRoute;
use crate::shard::ShardId;

/// Domain-separation tag for follow-token signatures (`v1`).
pub const FOLLOW_TOKEN_DOMAIN: &[u8] = b"magnetite/fleet/follow-token/v1";
/// Domain-separation tag for session-redirect signatures (`v1`).
pub const REDIRECT_DOMAIN: &[u8] = b"magnetite/fleet/redirect/v1";

/// Default lifetime of a redirect + its follow token. Short on purpose: it only
/// has to survive one reconnect.
pub const DEFAULT_REDIRECT_TTL_SECS: u64 = 30;

/// Clock skew tolerated when verifying a signed ad's lease.
pub const AD_SKEW_SECS: u64 = 60;

fn push_bytes(buf: &mut Vec<u8>, b: &[u8]) {
    buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
    buf.extend_from_slice(b);
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// A fresh 32-byte random value, hex-encoded, from the OS CSPRNG.
///
/// Derived from a throwaway keypair so this module needs no extra dependency —
/// the same trick `fleet` uses for handshake nonces.
fn random_hex() -> String {
    hex32(&RawKeypairAuth::generate().node_pubkey().0)
}

// ---------------------------------------------------------------------------
// Cluster membership
// ---------------------------------------------------------------------------

/// The operator-authorized set of node keys that make up one cluster.
///
/// This is the boundary between "a box that announced itself" and "a box that
/// may hold our world's authority". It is **deny by default**: a membership with
/// no keys in it authorizes nobody, so the failure mode of a missing or
/// half-applied configuration is *no handoffs*, never *handoffs to strangers*.
///
/// Membership is keyed on the node **public key**, never on an address, so it
/// survives a node changing IP and cannot be claimed by squatting an address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClusterMembership {
    members: HashSet<PubKey>,
}

impl ClusterMembership {
    /// An empty membership — authorizes nobody.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from the operator's authorized keys.
    pub fn from_keys(keys: impl IntoIterator<Item = PubKey>) -> Self {
        Self {
            members: keys.into_iter().collect(),
        }
    }

    /// Authorize a node key to hold this cluster's shards.
    pub fn authorize(&mut self, key: PubKey) -> &mut Self {
        self.members.insert(key);
        self
    }

    /// Revoke a node key. Existing derived routes to it stop being usable the
    /// next time membership is consulted — which is on every migration.
    pub fn revoke(&mut self, key: &PubKey) -> bool {
        self.members.remove(key)
    }

    /// Whether `key` is an operator-authorized member of this cluster.
    pub fn contains(&self, key: &PubKey) -> bool {
        self.members.contains(key)
    }

    /// Number of authorized keys.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether nobody is authorized (in which case nothing may be handed off).
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// The membership as a peer allowlist for [`crate::fleet::FleetNode::bind`],
    /// so the same operator decision gates the inbound door as well.
    pub fn allowlist(&self) -> HashSet<PubKey> {
        self.members.clone()
    }

    /// The authorized keys, sorted by hex for stable output.
    pub fn keys(&self) -> Vec<PubKey> {
        let mut v: Vec<PubKey> = self.members.iter().copied().collect();
        v.sort_by_key(|k| k.to_hex());
        v
    }
}

// ---------------------------------------------------------------------------
// Discovery-derived routes
// ---------------------------------------------------------------------------

/// Why an announced ad did not become a usable route. Every variant is a
/// refusal; none of them yields a partially-trusted route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteRejection {
    /// The ad's signature, lease window, or TTL failed verification.
    Unverified(String),
    /// The ad verified, but the announcing node is **not a cluster member**.
    /// This is the open-phonebook defence: announcing is not joining.
    NotAMember(String),
    /// The lease has already lapsed at the time of evaluation.
    Expired,
    /// The ad carried no usable address.
    BadAddress(String),
    /// No live route is known for that node key.
    Unknown(String),
}

impl std::fmt::Display for RouteRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteRejection::Unverified(m) => write!(f, "ad failed verification: {m}"),
            RouteRejection::NotAMember(k) => write!(
                f,
                "node {k} announced itself but is not an authorized member of this cluster"
            ),
            RouteRejection::Expired => write!(f, "ad lease has lapsed"),
            RouteRejection::BadAddress(m) => write!(f, "ad carried no usable address: {m}"),
            RouteRejection::Unknown(k) => write!(f, "no live route known for node {k}"),
        }
    }
}

impl std::error::Error for RouteRejection {}

#[derive(Debug, Clone)]
struct DerivedRoute {
    route: PeerRoute,
    expires_at: u64,
}

/// Routes learned from signed discovery ads, filtered by cluster membership.
///
/// The directory is the *only* thing discovery is allowed to influence: it turns
/// "this key says it is at this address" into a [`PeerRoute`] with the key
/// pinned. It never decides *who* may receive a shard — [`ClusterMembership`]
/// does, and the directory refuses to record anything for a non-member.
#[derive(Debug, Clone)]
pub struct RouteDirectory {
    membership: ClusterMembership,
    routes: HashMap<PubKey, DerivedRoute>,
}

impl RouteDirectory {
    /// A directory gated by `membership`.
    pub fn new(membership: ClusterMembership) -> Self {
        Self {
            membership,
            routes: HashMap::new(),
        }
    }

    /// The membership gating this directory.
    pub fn membership(&self) -> &ClusterMembership {
        &self.membership
    }

    /// Mutable access, so an operator can authorize/revoke at runtime. Revoking
    /// takes effect immediately: [`RouteDirectory::route_for`] consults
    /// membership on every lookup, and so does the transport at migration time.
    pub fn membership_mut(&mut self) -> &mut ClusterMembership {
        &mut self.membership
    }

    /// Ingest one signed ad.
    ///
    /// Order of checks matters and is deliberate: cryptographic verification
    /// first (so a forged ad never even reaches the membership test), then
    /// membership, then the address. Nothing is recorded unless all three pass.
    ///
    /// `now` is unix seconds.
    pub fn observe(&mut self, ad: &SignedAd, now: u64) -> Result<PeerRoute, RouteRejection> {
        // 1. Authorship + lease. Rejects forged, tampered, expired, future-dated
        //    and absurdly-long-lived ads (see `SignedAd::verify`).
        ad.verify::<RawKeypairAuth>(now, AD_SKEW_SECS)
            .map_err(|e| RouteRejection::Unverified(e.to_string()))?;

        // 2. Membership. An open phonebook must never confer cluster membership:
        //    a node that merely announces is NOT eligible to receive shards.
        if !self.membership.contains(&ad.node_key) {
            return Err(RouteRejection::NotAMember(ad.node_key.to_hex()));
        }

        // 3. A usable address.
        let addr = ad.ad.node.0.trim().to_string();
        if addr.is_empty() {
            return Err(RouteRejection::BadAddress("empty".into()));
        }

        // The pinned key comes from the SIGNED ad, not from the address. Reaching
        // this address later still proves nothing — the `fleet` handshake aborts
        // unless the far side proves control of exactly this key.
        let route = PeerRoute::new(addr, ad.node_key);
        self.routes.insert(
            ad.node_key,
            DerivedRoute {
                route: route.clone(),
                expires_at: ad.expires_at,
            },
        );
        Ok(route)
    }

    /// Record a route the **operator configured by hand**, rather than one
    /// learned from the phonebook.
    ///
    /// This is not a hole in the discovery rules, because the rules were never
    /// about addresses — they were about who may hold authority, and that is
    /// still [`ClusterMembership`]: a non-member address is refused here exactly
    /// as it is in [`Self::observe`]. What changes is only the *source* of the
    /// address, and an operator's own config file is a strictly better source
    /// than an open phonebook, not a worse one.
    ///
    /// The pinned key is the one the operator wrote down, and the `fleet`
    /// handshake still aborts unless the far side proves control of exactly that
    /// key — so a wrong or hijacked address yields a failed connection, never a
    /// misdirected shard.
    ///
    /// Operator routes carry **no lease**: they are as current as the file they
    /// came from, and they stop being usable the moment the key is revoked from
    /// membership (which is re-checked on every lookup). Liveness is therefore
    /// established by actually reaching the node, not by an ad's expiry — see
    /// [`crate::rebalance`], which treats an unanswered peer as a place not to
    /// put work.
    pub fn admit_operator_route(
        &mut self,
        route: PeerRoute,
    ) -> Result<PeerRoute, RouteRejection> {
        if !self.membership.contains(&route.pubkey) {
            return Err(RouteRejection::NotAMember(route.pubkey.to_hex()));
        }
        if route.addr.trim().is_empty() {
            return Err(RouteRejection::BadAddress("empty".into()));
        }
        self.routes.insert(
            route.pubkey,
            DerivedRoute {
                route: route.clone(),
                expires_at: u64::MAX,
            },
        );
        Ok(route)
    }

    /// Ingest many ads, returning the routes that were accepted. Rejections are
    /// silently skipped — an open phonebook is expected to contain entries that
    /// are not ours.
    pub fn observe_all<'a>(
        &mut self,
        ads: impl IntoIterator<Item = &'a SignedAd>,
        now: u64,
    ) -> Vec<PeerRoute> {
        ads.into_iter()
            .filter_map(|ad| self.observe(ad, now).ok())
            .collect()
    }

    /// The live route to a member node, or a refusal.
    ///
    /// Re-checks membership **and** the lease on every call, so a revoked node or
    /// a node whose lease lapsed stops being routable without any sweep.
    pub fn route_for(&self, key: &PubKey, now: u64) -> Result<PeerRoute, RouteRejection> {
        if !self.membership.contains(key) {
            return Err(RouteRejection::NotAMember(key.to_hex()));
        }
        match self.routes.get(key) {
            Some(d) if d.expires_at > now => Ok(d.route.clone()),
            Some(_) => Err(RouteRejection::Expired),
            None => Err(RouteRejection::Unknown(key.to_hex())),
        }
    }

    /// Every currently-usable route (member + unexpired), sorted by key hex.
    pub fn live_routes(&self, now: u64) -> Vec<PeerRoute> {
        let mut v: Vec<PeerRoute> = self
            .routes
            .values()
            .filter(|d| d.expires_at > now && self.membership.contains(&d.route.pubkey))
            .map(|d| d.route.clone())
            .collect();
        v.sort_by_key(|r| r.pubkey.to_hex());
        v
    }

    /// Drop lapsed and no-longer-member entries. Purely housekeeping — lookups
    /// already filter, so skipping this is safe, just leakier.
    pub fn prune(&mut self, now: u64) {
        let membership = &self.membership;
        self.routes
            .retain(|k, d| d.expires_at > now && membership.contains(k));
    }

    /// How many entries are held (including lapsed ones not yet pruned).
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Whether the directory holds nothing.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Follow token — what admits a player to the TARGET node
// ---------------------------------------------------------------------------

/// A short-lived bearer credential minted by the **source** node that admits one
/// player to one shard at one epoch on one target node.
///
/// It is bound to all four of those, so it is useless for anything else: a token
/// for player X will not admit player Y, a token for shard S will not admit to
/// shard T, a token for epoch `e` will not admit once the shard has moved on to
/// `e+1`, and a token for node B will not admit at node C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowToken {
    /// The player this token admits, and nobody else.
    pub player: u64,
    /// The shard this token admits to, and no other.
    pub shard: u32,
    /// The epoch established by the migration this token accompanies.
    pub epoch: u64,
    /// The node key that may redeem it — the migration target.
    pub target: PubKey,
    /// The node key that minted it — the migration source.
    pub issuer: PubKey,
    /// Unix seconds when minted.
    pub issued_at: u64,
    /// Unix seconds after which it is refused.
    pub expires_at: u64,
    /// Single-use nonce; the target records redeemed nonces.
    pub nonce: String,
    /// `issuer`'s signature over [`FollowToken::signing_bytes`].
    pub sig: Sig,
}

/// Equality compares the full signed body **and** the signature bytes
/// (`Sig` itself is not `PartialEq`), so two tokens are equal only if they are
/// byte-identical.
impl PartialEq for FollowToken {
    fn eq(&self, other: &Self) -> bool {
        self.signing_bytes() == other.signing_bytes() && self.sig.0 == other.sig.0
    }
}
impl Eq for FollowToken {}

impl FollowToken {
    /// Canonical bytes covered by [`FollowToken::sig`].
    pub fn signing_bytes(&self) -> Vec<u8> {
        Self::payload(
            self.player,
            self.shard,
            self.epoch,
            &self.target,
            &self.issuer,
            self.issued_at,
            self.expires_at,
            &self.nonce,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn payload(
        player: u64,
        shard: u32,
        epoch: u64,
        target: &PubKey,
        issuer: &PubKey,
        issued_at: u64,
        expires_at: u64,
        nonce: &str,
    ) -> Vec<u8> {
        let mut b = Vec::with_capacity(160);
        b.extend_from_slice(FOLLOW_TOKEN_DOMAIN);
        b.extend_from_slice(&player.to_le_bytes());
        b.extend_from_slice(&shard.to_le_bytes());
        b.extend_from_slice(&epoch.to_le_bytes());
        b.extend_from_slice(&target.0);
        b.extend_from_slice(&issuer.0);
        b.extend_from_slice(&issued_at.to_le_bytes());
        b.extend_from_slice(&expires_at.to_le_bytes());
        push_bytes(&mut b, nonce.as_bytes());
        b
    }

    /// Mint a token with the source node's identity.
    #[allow(clippy::too_many_arguments)]
    pub fn mint(
        id: &RawKeypairAuth,
        player: u64,
        shard: ShardId,
        epoch: u64,
        target: PubKey,
        now: u64,
        ttl_secs: u64,
    ) -> Self {
        let issuer = id.node_pubkey();
        let expires_at = now.saturating_add(ttl_secs);
        let nonce = random_hex();
        let sig = id.sign(&Self::payload(
            player, shard.0, epoch, &target, &issuer, now, expires_at, &nonce,
        ));
        Self {
            player,
            shard: shard.0,
            epoch,
            target,
            issuer,
            issued_at: now,
            expires_at,
            nonce,
            sig,
        }
    }
}

/// Why a target node refused to admit a following player. **Every variant means
/// the player is not admitted** — there is no partial admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitError {
    /// The token names a different node as its target.
    WrongTarget,
    /// The token was minted by a key this cluster did not authorize. This is
    /// what stops an outsider from writing their own admission tickets.
    IssuerNotAMember(String),
    /// The signature does not verify under the claimed issuer.
    BadSignature,
    /// The token has expired (or was minted in the future).
    Expired,
    /// The token admits a different player.
    WrongPlayer {
        /// Player the token was minted for.
        expected: u64,
        /// Player that presented it.
        got: u64,
    },
    /// The token admits to a different shard.
    WrongShard {
        /// Shard the token was minted for.
        expected: u32,
        /// Shard it was presented against.
        got: u32,
    },
    /// The token's epoch is not the epoch this node currently owns — a stale or
    /// replayed redirect from a superseded migration.
    StaleEpoch {
        /// Epoch named by the token.
        token: u64,
        /// Epoch this node actually holds (`None` ⇒ it owns the shard at no
        /// epoch at all).
        owned: Option<u64>,
    },
    /// This token was already redeemed.
    Replayed,
}

impl std::fmt::Display for AdmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdmitError::WrongTarget => write!(f, "follow token was not minted for this node"),
            AdmitError::IssuerNotAMember(k) => {
                write!(f, "follow token issued by non-member node {k}")
            }
            AdmitError::BadSignature => write!(f, "follow token signature does not verify"),
            AdmitError::Expired => write!(f, "follow token is expired or future-dated"),
            AdmitError::WrongPlayer { expected, got } => {
                write!(f, "follow token admits player {expected}, not {got}")
            }
            AdmitError::WrongShard { expected, got } => {
                write!(f, "follow token admits to shard {expected}, not {got}")
            }
            AdmitError::StaleEpoch { token, owned } => write!(
                f,
                "follow token is for epoch {token}, this node owns {owned:?}"
            ),
            AdmitError::Replayed => write!(f, "follow token has already been redeemed"),
        }
    }
}

impl std::error::Error for AdmitError {}

/// The **target** node's door for players following a migrated shard.
///
/// Holds the node's own key, the cluster membership (so only member-issued
/// tokens are honoured) and the set of redeemed nonces (so a token is one-shot).
#[derive(Debug)]
pub struct FollowAdmission {
    node_key: PubKey,
    membership: ClusterMembership,
    redeemed: HashSet<String>,
}

impl FollowAdmission {
    /// Build a door for the node identified by `node_key`, admitting tokens
    /// issued by any member of `membership`.
    pub fn new(node_key: PubKey, membership: ClusterMembership) -> Self {
        Self {
            node_key,
            membership,
            redeemed: HashSet::new(),
        }
    }

    /// Mutable membership, so revocation takes effect on the next admission.
    pub fn membership_mut(&mut self) -> &mut ClusterMembership {
        &mut self.membership
    }

    /// Admit `player` to `shard` on the strength of `token`.
    ///
    /// `owned_epoch` is the epoch this node currently holds for `shard` (from
    /// [`crate::fleet::ShardAuthority::epoch_of`]); `None` means it does not own
    /// the shard, which is itself a refusal. Fails closed on every negative and
    /// consumes the token only on success.
    pub fn admit(
        &mut self,
        token: &FollowToken,
        player: u64,
        shard: ShardId,
        owned_epoch: Option<u64>,
        now: u64,
    ) -> Result<(), AdmitError> {
        // 1. Is this token even for us? (An attacker replaying a token minted for
        //    another node gets nothing.)
        if token.target != self.node_key {
            return Err(AdmitError::WrongTarget);
        }
        // 2. Only an operator-authorized node may write admission tickets.
        if !self.membership.contains(&token.issuer) {
            return Err(AdmitError::IssuerNotAMember(token.issuer.to_hex()));
        }
        // 3. Authorship.
        if !<RawKeypairAuth as Identity>::verify(&token.issuer, &token.signing_bytes(), &token.sig) {
            return Err(AdmitError::BadSignature);
        }
        // 4. Freshness.
        if token.expires_at <= now || token.issued_at > now.saturating_add(AD_SKEW_SECS) {
            return Err(AdmitError::Expired);
        }
        // 5. Bound to this exact player…
        if token.player != player {
            return Err(AdmitError::WrongPlayer {
                expected: token.player,
                got: player,
            });
        }
        // 6. …this exact shard…
        if token.shard != shard.0 {
            return Err(AdmitError::WrongShard {
                expected: token.shard,
                got: shard.0,
            });
        }
        // 7. …and the epoch we ACTUALLY own. This is the same fence the handoff
        //    protocol uses: a redirect from a superseded migration is refused.
        if owned_epoch != Some(token.epoch) {
            return Err(AdmitError::StaleEpoch {
                token: token.epoch,
                owned: owned_epoch,
            });
        }
        // 8. One shot.
        if !self.redeemed.insert(token.nonce.clone()) {
            return Err(AdmitError::Replayed);
        }
        Ok(())
    }

    /// How many tokens have been redeemed here.
    pub fn redeemed_count(&self) -> usize {
        self.redeemed.len()
    }

    /// Forget redeemed nonces that can no longer be replayed anyway (all tokens
    /// expire within [`DEFAULT_REDIRECT_TTL_SECS`]). Housekeeping only.
    pub fn clear_redeemed(&mut self) {
        self.redeemed.clear();
    }
}

// ---------------------------------------------------------------------------
// Session redirect — what the SOURCE hands the client
// ---------------------------------------------------------------------------

/// "Your shard moved; go here." Signed by the **source** node, which the client
/// has already authenticated, and carrying the target's pinned key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRedirect {
    /// The player being redirected.
    pub player: u64,
    /// The shard that moved.
    pub shard: u32,
    /// The epoch the target now owns it at.
    pub epoch: u64,
    /// Where to reconnect (`host:port`). An address is a hint, never an identity.
    pub addr: String,
    /// The target's node key — **pinned**. The client must abort if the far side
    /// presents anything else.
    pub target_key: PubKey,
    /// The source node key that signed this redirect.
    pub issuer: PubKey,
    /// Unix seconds when minted.
    pub issued_at: u64,
    /// Unix seconds after which the client must discard it.
    pub expires_at: u64,
    /// The credential to present at the target.
    pub token: FollowToken,
    /// `issuer`'s signature over [`SignedRedirect::signing_bytes`].
    pub sig: Sig,
}

/// Why a client refused a redirect. Every variant means "stay put / drop it";
/// a client must never follow a redirect it could not fully verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectError {
    /// Signed by someone other than the node this client is connected to — the
    /// forged-redirect case.
    WrongIssuer {
        /// The node key the client had authenticated.
        expected: String,
        /// The key that signed the redirect.
        got: String,
    },
    /// The signature does not verify.
    BadSignature,
    /// Expired or future-dated.
    Expired,
    /// The redirect is for a different player.
    WrongPlayer,
    /// The embedded token disagrees with the redirect envelope.
    TokenMismatch(String),
    /// The redirect carries no usable address.
    BadAddress,
}

impl std::fmt::Display for RedirectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedirectError::WrongIssuer { expected, got } => write!(
                f,
                "redirect signed by {got}, but this session is authenticated to {expected}"
            ),
            RedirectError::BadSignature => write!(f, "redirect signature does not verify"),
            RedirectError::Expired => write!(f, "redirect is expired or future-dated"),
            RedirectError::WrongPlayer => write!(f, "redirect is addressed to another player"),
            RedirectError::TokenMismatch(m) => write!(f, "redirect/token disagree: {m}"),
            RedirectError::BadAddress => write!(f, "redirect carries no usable address"),
        }
    }
}

impl std::error::Error for RedirectError {}

/// Byte-identical equality, as for [`FollowToken`].
impl PartialEq for SignedRedirect {
    fn eq(&self, other: &Self) -> bool {
        self.signing_bytes() == other.signing_bytes()
            && self.sig.0 == other.sig.0
            && self.token == other.token
    }
}
impl Eq for SignedRedirect {}

impl SignedRedirect {
    /// Canonical bytes covered by [`SignedRedirect::sig`]. The token's own
    /// signature is folded in, so the envelope and the credential cannot be
    /// mixed and matched.
    pub fn signing_bytes(&self) -> Vec<u8> {
        Self::payload(
            self.player,
            self.shard,
            self.epoch,
            &self.addr,
            &self.target_key,
            &self.issuer,
            self.issued_at,
            self.expires_at,
            &self.token,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn payload(
        player: u64,
        shard: u32,
        epoch: u64,
        addr: &str,
        target_key: &PubKey,
        issuer: &PubKey,
        issued_at: u64,
        expires_at: u64,
        token: &FollowToken,
    ) -> Vec<u8> {
        let mut b = Vec::with_capacity(256);
        b.extend_from_slice(REDIRECT_DOMAIN);
        b.extend_from_slice(&player.to_le_bytes());
        b.extend_from_slice(&shard.to_le_bytes());
        b.extend_from_slice(&epoch.to_le_bytes());
        push_bytes(&mut b, addr.as_bytes());
        b.extend_from_slice(&target_key.0);
        b.extend_from_slice(&issuer.0);
        b.extend_from_slice(&issued_at.to_le_bytes());
        b.extend_from_slice(&expires_at.to_le_bytes());
        b.extend_from_slice(&token.sig.0);
        b
    }

    /// Mint a redirect for one player, for a migration that has **already
    /// committed**.
    pub fn mint(
        id: &RawKeypairAuth,
        player: u64,
        shard: ShardId,
        epoch: u64,
        route: &PeerRoute,
        now: u64,
        ttl_secs: u64,
    ) -> Self {
        let token = FollowToken::mint(id, player, shard, epoch, route.pubkey, now, ttl_secs);
        let issuer = id.node_pubkey();
        let expires_at = now.saturating_add(ttl_secs);
        let sig = id.sign(&Self::payload(
            player,
            shard.0,
            epoch,
            &route.addr,
            &route.pubkey,
            &issuer,
            now,
            expires_at,
            &token,
        ));
        Self {
            player,
            shard: shard.0,
            epoch,
            addr: route.addr.clone(),
            target_key: route.pubkey,
            issuer,
            issued_at: now,
            expires_at,
            token,
            sig,
        }
    }

    /// **Client side.** Verify this redirect came from the node we are already
    /// talking to, is fresh, is addressed to us, and is internally consistent —
    /// then return the route to reconnect to, with the target key pinned.
    ///
    /// `expected_issuer` is the node key the client authenticated when it
    /// connected. Checking it first is what makes a forged redirect inert: an
    /// attacker who can inject a message into the session still cannot sign as
    /// the node, and a redirect signed by anyone else is discarded outright.
    pub fn verify_for(
        &self,
        expected_issuer: &PubKey,
        player: u64,
        now: u64,
    ) -> Result<PeerRoute, RedirectError> {
        if self.issuer != *expected_issuer {
            return Err(RedirectError::WrongIssuer {
                expected: expected_issuer.to_hex(),
                got: self.issuer.to_hex(),
            });
        }
        if !<RawKeypairAuth as Identity>::verify(&self.issuer, &self.signing_bytes(), &self.sig) {
            return Err(RedirectError::BadSignature);
        }
        if self.expires_at <= now || self.issued_at > now.saturating_add(AD_SKEW_SECS) {
            return Err(RedirectError::Expired);
        }
        if self.player != player {
            return Err(RedirectError::WrongPlayer);
        }
        // The envelope must not promise something the credential does not back.
        if self.token.player != self.player {
            return Err(RedirectError::TokenMismatch("player".into()));
        }
        if self.token.shard != self.shard {
            return Err(RedirectError::TokenMismatch("shard".into()));
        }
        if self.token.epoch != self.epoch {
            return Err(RedirectError::TokenMismatch("epoch".into()));
        }
        if self.token.target != self.target_key {
            return Err(RedirectError::TokenMismatch("target node key".into()));
        }
        if self.token.issuer != self.issuer {
            return Err(RedirectError::TokenMismatch("issuer".into()));
        }
        if self.addr.trim().is_empty() {
            return Err(RedirectError::BadAddress);
        }
        Ok(PeerRoute::new(self.addr.clone(), self.target_key))
    }
}

/// Mints redirects for the players affected by a committed migration.
///
/// Held by the source node's transport and driven **only** from the success path
/// of a migration — see [`crate::fleet::NetworkHandoffTransport`].
#[derive(Debug, Clone)]
pub struct Redirector {
    ttl_secs: u64,
}

impl Default for Redirector {
    fn default() -> Self {
        Self {
            ttl_secs: DEFAULT_REDIRECT_TTL_SECS,
        }
    }
}

impl Redirector {
    /// A redirector with the default short TTL.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the redirect/token TTL.
    pub fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = ttl_secs;
        self
    }

    /// The TTL in force.
    pub fn ttl_secs(&self) -> u64 {
        self.ttl_secs
    }

    /// Mint one redirect per affected player for a **committed** migration.
    pub fn redirects_for(
        &self,
        id: &RawKeypairAuth,
        players: &[u64],
        shard: ShardId,
        epoch: u64,
        route: &PeerRoute,
        now: u64,
    ) -> Vec<SignedRedirect> {
        players
            .iter()
            .map(|p| SignedRedirect::mint(id, *p, shard, epoch, route, now, self.ttl_secs))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::blobstore::Hash;
    use magnetite_seams::discovery::{Capacity, NodeAddr, SessionAd};

    fn ident(seed: u8) -> RawKeypairAuth {
        RawKeypairAuth::from_seed([seed; 32])
    }

    fn ad_for(node: &str) -> SessionAd {
        SessionAd {
            game: Hash::of(b"snake"),
            node: NodeAddr(node.into()),
            operator: None,
            region: None,
            capacity: Capacity {
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
        }
    }

    fn signed(id: &RawKeypairAuth, node: &str, now: u64, ttl: u64) -> SignedAd {
        SignedAd::sign(id, ad_for(node), now, ttl)
    }

    // ── Task A: discovery may supply addresses, never membership ───────────

    #[test]
    fn a_member_ad_becomes_a_key_pinned_route() {
        let member = ident(1);
        let membership = ClusterMembership::from_keys([member.node_pubkey()]);
        let mut dir = RouteDirectory::new(membership);

        let route = dir
            .observe(&signed(&member, "10.0.0.5:7100", 1_000, 60), 1_000)
            .expect("a member's signed ad yields a route");
        assert_eq!(route.addr, "10.0.0.5:7100");
        assert_eq!(
            route.pubkey,
            member.node_pubkey(),
            "the pinned key comes from the SIGNED ad, not from the address"
        );
        assert_eq!(dir.route_for(&member.node_pubkey(), 1_000).unwrap(), route);
        assert_eq!(dir.live_routes(1_000).len(), 1);
    }

    #[test]
    fn an_announcing_non_member_is_never_a_handoff_target() {
        // THE trap: discovery is an open phonebook. This node announces loudly,
        // with a perfectly valid signature — and must still get nothing.
        let member = ident(1);
        let volunteer = ident(66);
        let mut dir = RouteDirectory::new(ClusterMembership::from_keys([member.node_pubkey()]));

        let err = dir
            .observe(&signed(&volunteer, "attacker.example:7100", 1_000, 60), 1_000)
            .unwrap_err();
        assert!(
            matches!(err, RouteRejection::NotAMember(_)),
            "announcing is not joining, got {err:?}"
        );
        assert!(dir.is_empty(), "a rejected ad teaches the directory nothing");
        assert!(matches!(
            dir.route_for(&volunteer.node_pubkey(), 1_000),
            Err(RouteRejection::NotAMember(_))
        ));
        assert!(dir.live_routes(1_000).is_empty());
    }

    #[test]
    fn hosting_a_game_does_not_admit_you_to_a_world_you_never_joined() {
        // Same box, two clusters. It is a member of A's cluster only.
        let node = ident(7);
        let cluster_a = ClusterMembership::from_keys([node.node_pubkey(), ident(1).node_pubkey()]);
        let cluster_b = ClusterMembership::from_keys([ident(2).node_pubkey()]);

        let ad = signed(&node, "10.0.0.7:7100", 1_000, 60);
        assert!(RouteDirectory::new(cluster_a).observe(&ad, 1_000).is_ok());
        assert!(
            matches!(
                RouteDirectory::new(cluster_b).observe(&ad, 1_000),
                Err(RouteRejection::NotAMember(_))
            ),
            "the very same ad confers nothing in a cluster that did not admit it"
        );
    }

    #[test]
    fn forged_and_tampered_ads_are_refused() {
        let member = ident(1);
        let attacker = ident(2);
        let mut dir = RouteDirectory::new(ClusterMembership::from_keys([member.node_pubkey()]));

        // Take a member's real ad and re-point it at the attacker's address.
        let mut hijacked = signed(&member, "10.0.0.5:7100", 1_000, 60);
        hijacked.ad.node = NodeAddr("attacker.example:1".into());
        assert!(matches!(
            dir.observe(&hijacked, 1_000),
            Err(RouteRejection::Unverified(_))
        ));

        // Sign an ad with the attacker's key while claiming the member's key.
        let mut impersonated = signed(&attacker, "attacker.example:1", 1_000, 60);
        impersonated.node_key = member.node_pubkey();
        assert!(matches!(
            dir.observe(&impersonated, 1_000),
            Err(RouteRejection::Unverified(_))
        ));
        assert!(dir.is_empty());
    }

    #[test]
    fn expired_and_lapsed_ads_are_not_routed_to() {
        let member = ident(1);
        let mut dir = RouteDirectory::new(ClusterMembership::from_keys([member.node_pubkey()]));

        // Already expired when observed.
        assert!(matches!(
            dir.observe(&signed(&member, "10.0.0.5:7100", 1_000, 60), 2_000),
            Err(RouteRejection::Unverified(_))
        ));

        // Accepted while live, refused once the lease lapses.
        dir.observe(&signed(&member, "10.0.0.5:7100", 1_000, 60), 1_000)
            .unwrap();
        assert!(dir.route_for(&member.node_pubkey(), 1_030).is_ok());
        assert!(matches!(
            dir.route_for(&member.node_pubkey(), 1_061),
            Err(RouteRejection::Expired)
        ));
        assert!(dir.live_routes(1_061).is_empty());
        dir.prune(1_061);
        assert!(dir.is_empty());
    }

    #[test]
    fn revocation_takes_effect_immediately() {
        let member = ident(1);
        let mut dir = RouteDirectory::new(ClusterMembership::from_keys([member.node_pubkey()]));
        dir.observe(&signed(&member, "10.0.0.5:7100", 1_000, 60), 1_000)
            .unwrap();
        assert!(dir.route_for(&member.node_pubkey(), 1_010).is_ok());

        dir.membership_mut().revoke(&member.node_pubkey());
        assert!(
            matches!(
                dir.route_for(&member.node_pubkey(), 1_010),
                Err(RouteRejection::NotAMember(_))
            ),
            "a revoked node stops being routable without waiting for its lease"
        );
        assert!(dir.live_routes(1_010).is_empty());
    }

    #[test]
    fn an_empty_membership_authorizes_nobody() {
        let anyone = ident(3);
        let mut dir = RouteDirectory::new(ClusterMembership::new());
        assert!(dir.membership().is_empty());
        assert!(
            dir.observe(&signed(&anyone, "10.0.0.9:7100", 1_000, 60), 1_000)
                .is_err(),
            "deny by default: a missing config must hand shards to NOBODY"
        );
    }

    #[test]
    fn observe_all_keeps_members_and_drops_the_rest() {
        let m1 = ident(1);
        let m2 = ident(2);
        let stranger = ident(90);
        let mut dir = RouteDirectory::new(ClusterMembership::from_keys([
            m1.node_pubkey(),
            m2.node_pubkey(),
        ]));
        let ads = vec![
            signed(&m1, "10.0.0.1:7100", 1_000, 60),
            signed(&stranger, "evil:7100", 1_000, 60),
            signed(&m2, "10.0.0.2:7100", 1_000, 60),
        ];
        let got = dir.observe_all(&ads, 1_000);
        assert_eq!(got.len(), 2);
        assert_eq!(dir.live_routes(1_000).len(), 2);
    }

    // ── Task B: the session follows the shard ─────────────────────────────

    fn route_of(id: &RawKeypairAuth, addr: &str) -> PeerRoute {
        PeerRoute::new(addr, id.node_pubkey())
    }

    #[test]
    fn a_player_follows_a_migrated_shard_end_to_end() {
        let a = ident(10); // source, already authenticated to the client
        let b = ident(11); // target
        let membership = ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey()]);
        let route_b = route_of(&b, "10.0.0.11:7100");

        // A commits the migration at epoch 5, then redirects the player.
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_b, 1_000, 30);

        // Client: verifies it came from the node it is talking to, gets a route
        // with B's key PINNED.
        let follow = r.verify_for(&a.node_pubkey(), 42, 1_001).unwrap();
        assert_eq!(follow.pubkey, b.node_pubkey(), "target key is pinned");
        assert_eq!(follow.addr, "10.0.0.11:7100");

        // B admits, because it really does own shard 3 at epoch 5.
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);
        door.admit(&r.token, 42, ShardId(3), Some(5), 1_001).unwrap();
        assert_eq!(door.redeemed_count(), 1);
    }

    #[test]
    fn a_forged_redirect_cannot_hijack_a_player() {
        let a = ident(10); // the node the client is actually talking to
        let attacker = ident(99);
        let attacker_node = route_of(&attacker, "attacker.example:7100");

        // The attacker signs a perfectly well-formed redirect pointing at itself.
        let forged = SignedRedirect::mint(&attacker, 42, ShardId(3), 5, &attacker_node, 1_000, 30);
        let err = forged.verify_for(&a.node_pubkey(), 42, 1_001).unwrap_err();
        assert!(
            matches!(err, RedirectError::WrongIssuer { .. }),
            "a redirect not signed by THIS session's node must be discarded, got {err:?}"
        );

        // …and tampering with a genuine redirect to re-point it also fails.
        let mut tampered = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&ident(11), "10.0.0.11:7100"), 1_000, 30);
        tampered.addr = "attacker.example:7100".into();
        tampered.target_key = attacker.node_pubkey();
        assert!(matches!(
            tampered.verify_for(&a.node_pubkey(), 42, 1_001),
            Err(RedirectError::BadSignature)
        ));
    }

    #[test]
    fn an_expired_redirect_is_refused_by_client_and_target() {
        let a = ident(10);
        let b = ident(11);
        let membership = ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey()]);
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);

        assert!(matches!(
            r.verify_for(&a.node_pubkey(), 42, 1_031),
            Err(RedirectError::Expired)
        ));
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);
        assert_eq!(
            door.admit(&r.token, 42, ShardId(3), Some(5), 1_031),
            Err(AdmitError::Expired),
            "even if the client follows a stale redirect, the target refuses it"
        );
    }

    #[test]
    fn a_stale_epoch_redirect_is_refused() {
        let a = ident(10);
        let b = ident(11);
        let membership = ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey()]);
        // A redirect from an OLD migration (epoch 5) replayed after the shard has
        // moved on to epoch 7.
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);
        assert_eq!(
            door.admit(&r.token, 42, ShardId(3), Some(7), 1_001),
            Err(AdmitError::StaleEpoch {
                token: 5,
                owned: Some(7)
            })
        );
        // And a node that does not own the shard at all admits nobody.
        assert_eq!(
            door.admit(&r.token, 42, ShardId(3), None, 1_001),
            Err(AdmitError::StaleEpoch {
                token: 5,
                owned: None
            })
        );
    }

    #[test]
    fn a_token_for_one_player_does_not_admit_another() {
        let a = ident(10);
        let b = ident(11);
        let membership = ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey()]);
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);

        assert_eq!(
            door.admit(&r.token, 43, ShardId(3), Some(5), 1_001),
            Err(AdmitError::WrongPlayer {
                expected: 42,
                got: 43
            }),
            "player X's ticket must not admit player Y"
        );
        // And a client cannot even get that far: the redirect is not addressed
        // to them.
        assert!(matches!(
            r.verify_for(&a.node_pubkey(), 43, 1_001),
            Err(RedirectError::WrongPlayer)
        ));
    }

    #[test]
    fn a_token_for_one_shard_does_not_admit_to_another() {
        let a = ident(10);
        let b = ident(11);
        let membership = ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey()]);
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);
        assert_eq!(
            door.admit(&r.token, 42, ShardId(4), Some(5), 1_001),
            Err(AdmitError::WrongShard {
                expected: 3,
                got: 4
            })
        );
    }

    #[test]
    fn a_token_is_single_use() {
        let a = ident(10);
        let b = ident(11);
        let membership = ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey()]);
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);
        door.admit(&r.token, 42, ShardId(3), Some(5), 1_001).unwrap();
        assert_eq!(
            door.admit(&r.token, 42, ShardId(3), Some(5), 1_002),
            Err(AdmitError::Replayed)
        );
    }

    #[test]
    fn a_token_minted_for_one_node_does_not_admit_at_another() {
        let a = ident(10);
        let b = ident(11);
        let c = ident(12);
        let membership =
            ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey(), c.node_pubkey()]);
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);
        // C is a perfectly legitimate cluster member — and still must not redeem
        // a ticket written for B.
        let mut door_c = FollowAdmission::new(c.node_pubkey(), membership);
        assert_eq!(
            door_c.admit(&r.token, 42, ShardId(3), Some(5), 1_001),
            Err(AdmitError::WrongTarget)
        );
    }

    #[test]
    fn a_token_from_a_non_member_issuer_is_refused() {
        // An outsider tries to write its own admission ticket into a real cluster.
        let outsider = ident(98);
        let b = ident(11);
        let membership = ClusterMembership::from_keys([ident(10).node_pubkey(), b.node_pubkey()]);
        let token = FollowToken::mint(&outsider, 42, ShardId(3), 5, b.node_pubkey(), 1_000, 30);
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);
        assert!(matches!(
            door.admit(&token, 42, ShardId(3), Some(5), 1_001),
            Err(AdmitError::IssuerNotAMember(_))
        ));
    }

    #[test]
    fn a_tampered_token_does_not_verify() {
        let a = ident(10);
        let b = ident(11);
        let membership = ClusterMembership::from_keys([a.node_pubkey(), b.node_pubkey()]);
        let mut token = FollowToken::mint(&a, 42, ShardId(3), 5, b.node_pubkey(), 1_000, 30);
        // Rewrite the player field, keeping the signature.
        token.player = 43;
        let mut door = FollowAdmission::new(b.node_pubkey(), membership);
        assert_eq!(
            door.admit(&token, 43, ShardId(3), Some(5), 1_001),
            Err(AdmitError::BadSignature)
        );
    }

    #[test]
    fn the_redirect_envelope_cannot_be_paired_with_a_foreign_token() {
        let a = ident(10);
        let b = ident(11);
        let attacker_target = ident(99);
        let mut r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);
        // Swap in a token that A also legitimately minted, but for a different
        // target. The envelope signature covers the token signature, so this dies.
        r.token = FollowToken::mint(
            &a,
            42,
            ShardId(3),
            5,
            attacker_target.node_pubkey(),
            1_000,
            30,
        );
        assert!(matches!(
            r.verify_for(&a.node_pubkey(), 42, 1_001),
            Err(RedirectError::BadSignature)
        ));
    }

    #[test]
    fn redirects_serialize_over_the_wire() {
        let a = ident(10);
        let b = ident(11);
        let r = SignedRedirect::mint(&a, 42, ShardId(3), 5, &route_of(&b, "10.0.0.11:7100"), 1_000, 30);
        let json = serde_json::to_string(&r).unwrap();
        let back: SignedRedirect = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
        back.verify_for(&a.node_pubkey(), 42, 1_001).unwrap();
    }

    #[test]
    fn redirector_mints_one_per_affected_player() {
        let a = ident(10);
        let b = ident(11);
        let route = route_of(&b, "10.0.0.11:7100");
        let rs = Redirector::new()
            .with_ttl(15)
            .redirects_for(&a, &[1, 2, 3], ShardId(3), 5, &route, 1_000);
        assert_eq!(rs.len(), 3);
        for (i, r) in rs.iter().enumerate() {
            assert_eq!(r.player, i as u64 + 1);
            assert_eq!(r.expires_at, 1_015);
            r.verify_for(&a.node_pubkey(), r.player, 1_001).unwrap();
        }
        // Nonces are distinct, so one player's token cannot shadow another's.
        let nonces: HashSet<&str> = rs.iter().map(|r| r.token.nonce.as_str()).collect();
        assert_eq!(nonces.len(), 3);
    }
}
