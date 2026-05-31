//! WebSocket connection manager.
//!
//! Each connected player gets a [`PlayerConn`] that holds:
//! - An mpsc sender for outbound [`ServerNet`] frames.
//! - The last `seq` received from this player (for `Ack` / `Reject` matching).
//! - The last snapshot tick sent to this player (for delta computation).
//!
//! The [`ConnectionManager`] is the authoritative registry of all live
//! connections.  It is used by the tick loop to collect inputs and fan-out
//! state to players.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::Mutex;

use magnetite_sdk::input::Input;
use magnetite_sdk::protocol::ServerNet;
use magnetite_sdk::state::PlayerId;

/// Capacity of each player's outbound message queue.
const OUTBOUND_QUEUE_CAPACITY: usize = 64;

/// Per-player connection state held by the manager.
pub(crate) struct PlayerConn {
    /// Send half for frames going to this player's WS task.
    pub(crate) tx: mpsc::Sender<ServerNet>,
    /// Most recently buffered input (filled each tick by the WS task).
    pub(crate) pending_input: Option<(u32, Input)>, // (seq, input)
    /// The authoritative tick of the last snapshot sent to this player.
    pub(crate) last_snapshot_tick: u64,
}

/// Thread-safe connection registry.
///
/// Wrapped in an `Arc<Mutex<_>>` so both the WS accept loop and the tick
/// scheduler can access it.
pub struct ConnectionManager {
    inner: Arc<Mutex<ConnectionManagerInner>>,
    next_player_id: Arc<AtomicU64>,
}

struct ConnectionManagerInner {
    connections: HashMap<PlayerId, PlayerConn>,
}

impl ConnectionManager {
    /// Create a new, empty connection manager.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConnectionManagerInner {
                connections: HashMap::new(),
            })),
            next_player_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Assign a fresh [`PlayerId`] and register a new connection.
    ///
    /// Returns the assigned [`PlayerId`] and the receive half of the outbound
    /// channel.  The WS task forwards frames from the receive half to the
    /// WebSocket.
    pub async fn register(&self) -> (PlayerId, mpsc::Receiver<ServerNet>) {
        let id = PlayerId::new(self.next_player_id.fetch_add(1, Ordering::Relaxed));
        let (tx, rx) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
        let conn = PlayerConn {
            tx,
            pending_input: None,
            last_snapshot_tick: 0,
        };
        self.inner.lock().await.connections.insert(id, conn);
        (id, rx)
    }

    /// Remove a player's connection (called on disconnect).
    pub async fn remove(&self, id: PlayerId) {
        self.inner.lock().await.connections.remove(&id);
    }

    /// Buffer an incoming input frame for the next tick.
    ///
    /// If the player is unknown (already disconnected), the input is silently
    /// dropped.
    pub async fn push_input(&self, id: PlayerId, seq: u32, input: Input) {
        let mut inner = self.inner.lock().await;
        if let Some(conn) = inner.connections.get_mut(&id) {
            // Latest input wins — we always want the most current frame.
            conn.pending_input = Some((seq, input));
        }
    }

    /// Drain all pending inputs and return them as an ordered list.
    ///
    /// Clears the pending slot for each player so the next tick starts fresh.
    /// Returns `(player_id, seq, input)` triples.
    pub async fn drain_inputs(&self) -> Vec<(PlayerId, u32, Input)> {
        let mut inner = self.inner.lock().await;
        let mut out = Vec::with_capacity(inner.connections.len());
        for (id, conn) in inner.connections.iter_mut() {
            if let Some((seq, input)) = conn.pending_input.take() {
                out.push((*id, seq, input));
            }
        }
        // Deterministic ordering by player id (mirrors NativeExecutor's sort).
        out.sort_by_key(|(id, _, _)| id.as_u64());
        out
    }

    /// Send a [`ServerNet`] frame to a specific player.
    ///
    /// Failures (full queue / disconnected) are silently dropped — the next
    /// snapshot will reconcile.
    pub async fn send_to(&self, id: PlayerId, msg: ServerNet) {
        let inner = self.inner.lock().await;
        if let Some(conn) = inner.connections.get(&id) {
            let _ = conn.tx.try_send(msg);
        }
    }

    /// Broadcast a frame to every connected player.
    pub async fn broadcast(&self, msg: ServerNet)
    where
        ServerNet: Clone,
    {
        let inner = self.inner.lock().await;
        for conn in inner.connections.values() {
            let _ = conn.tx.try_send(msg.clone());
        }
    }

    /// Return the list of currently connected player ids.
    pub async fn player_ids(&self) -> Vec<PlayerId> {
        let inner = self.inner.lock().await;
        inner.connections.keys().copied().collect()
    }

    /// Read the last snapshot tick recorded for a player.
    pub async fn last_snapshot_tick(&self, id: PlayerId) -> u64 {
        let inner = self.inner.lock().await;
        inner
            .connections
            .get(&id)
            .map(|c| c.last_snapshot_tick)
            .unwrap_or(0)
    }

    /// Update the last snapshot tick recorded for a player.
    pub async fn set_last_snapshot_tick(&self, id: PlayerId, tick: u64) {
        let mut inner = self.inner.lock().await;
        if let Some(conn) = inner.connections.get_mut(&id) {
            conn.last_snapshot_tick = tick;
        }
    }

    /// Number of currently registered connections.
    pub async fn len(&self) -> usize {
        self.inner.lock().await.connections.len()
    }

    /// Returns `true` when no players are connected.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ConnectionManager {
    /// Clone the manager — both instances share the same registry.
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            next_player_id: Arc::clone(&self.next_player_id),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::input::Input;

    #[tokio::test]
    async fn register_assigns_unique_ids() {
        let mgr = ConnectionManager::new();
        let (id1, _rx1) = mgr.register().await;
        let (id2, _rx2) = mgr.register().await;
        assert_ne!(id1.as_u64(), id2.as_u64());
    }

    #[tokio::test]
    async fn player_ids_tracks_connected() {
        let mgr = ConnectionManager::new();
        let (id1, _rx1) = mgr.register().await;
        let (id2, _rx2) = mgr.register().await;
        let ids = mgr.player_ids().await;
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        assert_eq!(ids.len(), 2);
    }

    #[tokio::test]
    async fn remove_cleans_up() {
        let mgr = ConnectionManager::new();
        let (id, _rx) = mgr.register().await;
        mgr.remove(id).await;
        assert!(mgr.is_empty().await);
    }

    #[tokio::test]
    async fn drain_inputs_returns_pending_and_clears() {
        let mgr = ConnectionManager::new();
        let (id, _rx) = mgr.register().await;

        mgr.push_input(id, 1, Input::default()).await;

        let drained = mgr.drain_inputs().await;
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].0, id);
        assert_eq!(drained[0].1, 1u32);

        // Second drain should be empty.
        let drained2 = mgr.drain_inputs().await;
        assert!(drained2.is_empty());
    }

    #[tokio::test]
    async fn drain_inputs_latest_wins() {
        let mgr = ConnectionManager::new();
        let (id, _rx) = mgr.register().await;

        mgr.push_input(
            id,
            1,
            Input {
                sequence: 1,
                ..Default::default()
            },
        )
        .await;
        mgr.push_input(
            id,
            2,
            Input {
                sequence: 2,
                ..Default::default()
            },
        )
        .await;

        let drained = mgr.drain_inputs().await;
        assert_eq!(drained.len(), 1);
        // seq=2 wins
        assert_eq!(drained[0].1, 2u32);
    }

    #[tokio::test]
    async fn send_to_delivers_message() {
        use magnetite_sdk::authority::RejectReason;
        use magnetite_sdk::protocol::ServerNet;

        let mgr = ConnectionManager::new();
        let (id, mut rx) = mgr.register().await;

        let msg = ServerNet::Reject {
            seq: 5,
            reason: RejectReason::RateLimited,
        };
        mgr.send_to(id, msg).await;

        let received = rx.recv().await.expect("should receive message");
        assert!(matches!(received, ServerNet::Reject { seq: 5, .. }));
    }

    #[tokio::test]
    async fn snapshot_tick_tracking() {
        let mgr = ConnectionManager::new();
        let (id, _rx) = mgr.register().await;

        assert_eq!(mgr.last_snapshot_tick(id).await, 0);
        mgr.set_last_snapshot_tick(id, 300).await;
        assert_eq!(mgr.last_snapshot_tick(id).await, 300);
    }

    #[tokio::test]
    async fn clone_shares_registry() {
        let mgr = ConnectionManager::new();
        let mgr2 = mgr.clone();

        let (id, _rx) = mgr.register().await;

        // The clone should see the same connection.
        assert!(mgr2.player_ids().await.contains(&id));
    }
}
