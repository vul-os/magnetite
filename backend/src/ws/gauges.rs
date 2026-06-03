// Shared atomic gauges for live observability of WebSocket and game-session counts.
#![allow(dead_code)]
//
// These are the canonical source-of-truth counters incremented/decremented by
// ws/comms.rs, ws/voice.rs, and ws/game.rs when connections open and close.
// The /metrics handler reads them on every scrape.

use std::sync::atomic::{AtomicU64, Ordering};

/// Application-wide WebSocket + game-session live gauges.
pub struct WsGauges {
    /// Total number of currently-connected WebSocket clients (all types: comms, voice, game).
    pub ws_connections: AtomicU64,
    /// Number of active `GameSession` instances in `GameManager`.
    pub game_sessions: AtomicU64,
}

impl WsGauges {
    pub const fn new() -> Self {
        Self {
            ws_connections: AtomicU64::new(0),
            game_sessions: AtomicU64::new(0),
        }
    }

    /// Increment the active WS connection count.
    #[inline]
    pub fn ws_connect(&self) {
        self.ws_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the active WS connection count (floors at 0).
    #[inline]
    pub fn ws_disconnect(&self) {
        self.ws_connections
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some(v.saturating_sub(1))
            })
            .ok();
    }

    /// Set the game session count (called whenever sessions map changes).
    #[inline]
    pub fn set_game_sessions(&self, count: u64) {
        self.game_sessions.store(count, Ordering::Relaxed);
    }
}

impl Default for WsGauges {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard that increments the live WS-connection gauge on creation and
/// decrements it on drop, so the count stays correct across every exit path
/// (clean close, error, panic) of a socket handler.
pub struct ConnGuard(std::sync::Arc<WsGauges>);

impl ConnGuard {
    pub fn new(gauges: std::sync::Arc<WsGauges>) -> Self {
        gauges.ws_connect();
        Self(gauges)
    }
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.ws_disconnect();
    }
}
