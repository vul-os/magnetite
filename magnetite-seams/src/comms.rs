//! Seam §3.5 — `CommsProvider` (chat / voice / video / streaming).
//!
//! **We build none of it.** Every social feature is an adapter behind this one
//! trait. Intended providers (NOT implemented in this crate — they live behind
//! their own feature-gated adapter modules and external SDKs):
//!
//! - `MatrixProvider`  — text / DMs / presence / spaces (Element homeservers).
//! - `JitsiProvider`   — voice + video SFU.
//! - `LiveKitProvider` — voice + video at scale.
//! - `OwncastProvider` / `PeerTubeProvider` — live + VOD streaming.
//!
//! The one default here, [`BuiltinProvider`], is the demoted old in-house stack:
//! it returns local room addresses and mints join credentials via the node's
//! [`AuthProvider`], so "one keypair login → SSO into comms" works offline.

use std::collections::HashSet;
use std::sync::Mutex;

use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::blobstore::Hash;
use crate::error::Result;
use crate::identity::{AuthProvider, Audience, PubKey, Scope, Token};
use crate::now_unix;

/// What a room is for. Drives naming and (in real providers) provisioning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomScope {
    /// Ephemeral room bound to a live match of a specific game.
    Match(Hash),
    /// Pre-game lobby.
    Lobby,
    /// Persistent community space.
    Community(String),
    /// Voice channel.
    Voice,
    /// Video channel.
    Video,
    /// One-to-many live stream.
    Stream,
}

impl RoomScope {
    fn tag(&self) -> &'static str {
        match self {
            RoomScope::Match(_) => "match",
            RoomScope::Lobby => "lobby",
            RoomScope::Community(_) => "community",
            RoomScope::Voice => "voice",
            RoomScope::Video => "video",
            RoomScope::Stream => "stream",
        }
    }
}

/// Provider-agnostic address of a room (opaque URI; format is the provider's).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomAddr(pub String);

/// A time-boxed credential admitting a user into a room.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinCred {
    /// Which room.
    pub room: RoomAddr,
    /// Who it admits.
    pub user: PubKey,
    /// Scoped token minted by the node acting as IdP.
    pub token: Token,
}

/// Pluggable chat/voice/video/streaming provider (§3.5).
#[async_trait::async_trait]
pub trait CommsProvider {
    /// Create (or reference) a room for a scope; returns its address.
    async fn create_room(&self, scope: RoomScope) -> RoomAddr;
    /// Mint a join credential for a user into a room (node acts as IdP).
    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred;
    /// Tear a room down.
    async fn teardown(&self, room: &RoomAddr) -> Result<()>;
}

/// Default comms: the demoted in-house stack, as a local shim.
///
/// Rooms are addressed `builtin://<tag>/<hex>`. Join credentials are minted from
/// the player's keypair via the wired [`AuthProvider`], audience `"builtin"`.
pub struct BuiltinProvider<A: AuthProvider + Send + Sync> {
    auth: A,
    rooms: Mutex<HashSet<RoomAddr>>,
}

impl<A: AuthProvider + Send + Sync> BuiltinProvider<A> {
    /// Wire the provider to an auth provider (its IdP for join tokens).
    pub fn new(auth: A) -> Self {
        Self {
            auth,
            rooms: Mutex::new(HashSet::new()),
        }
    }
    /// Whether a room is currently live.
    pub fn exists(&self, room: &RoomAddr) -> bool {
        self.rooms.lock().unwrap().contains(room)
    }
}

#[async_trait::async_trait]
impl<A: AuthProvider + Send + Sync> CommsProvider for BuiltinProvider<A> {
    async fn create_room(&self, scope: RoomScope) -> RoomAddr {
        let mut id = [0u8; 16];
        OsRng.fill_bytes(&mut id);
        let addr = RoomAddr(format!("builtin://{}/{}", scope.tag(), hex::encode(id)));
        self.rooms.lock().unwrap().insert(addr.clone());
        addr
    }

    async fn issue_join_credential(&self, user: &PubKey, room: &RoomAddr) -> JoinCred {
        let token = self
            .auth
            .mint_scoped_token(
                user,
                Audience("builtin".into()),
                Scope(vec![format!("room:join:{}", room.0), format!("ts:{}", now_unix())]),
            )
            .await;
        JoinCred {
            room: room.clone(),
            user: *user,
            token,
        }
    }

    async fn teardown(&self, room: &RoomAddr) -> Result<()> {
        self.rooms.lock().unwrap().remove(room);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::RawKeypairAuth;

    #[tokio::test]
    async fn create_join_and_teardown() {
        let node = RawKeypairAuth::from_seed([21u8; 32]);
        let node_pk = node.node_pubkey();
        let comms = BuiltinProvider::new(node);
        let player = PubKey([42u8; 32]);

        let room = comms.create_room(RoomScope::Lobby).await;
        assert!(room.0.starts_with("builtin://lobby/"));
        assert!(comms.exists(&room));

        let cred = comms.issue_join_credential(&player, &room).await;
        assert_eq!(cred.user, player);
        assert_eq!(cred.room, room);
        // Credential is a valid, node-issued, builtin-audience token.
        assert!(cred.token.is_valid_at(now_unix()));
        assert_eq!(cred.token.claims.issuer, node_pk);
        assert_eq!(cred.token.claims.audience, Audience("builtin".into()));

        comms.teardown(&room).await.unwrap();
        assert!(!comms.exists(&room));
    }

    #[tokio::test]
    async fn match_rooms_are_scope_tagged_and_unique() {
        let comms = BuiltinProvider::new(RawKeypairAuth::from_seed([22u8; 32]));
        let g = Hash::of(b"game");
        let a = comms.create_room(RoomScope::Match(g)).await;
        let b = comms.create_room(RoomScope::Match(g)).await;
        assert!(a.0.starts_with("builtin://match/"));
        assert_ne!(a, b, "each room gets a fresh id");
    }
}
