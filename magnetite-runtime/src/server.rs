//! Authoritative game-server host.
//!
//! [`GameServer::serve`] is the single entry point.  It:
//!
//! 1. Binds a TCP listener on `config.bind_addr`.
//! 2. Spawns a [`TickScheduler`] task for the authoritative tick loop.
//! 3. Accepts WebSocket connections in a loop, spawning a handler task per
//!    connection.
//!
//! ## Topology dispatch
//!
//! | Topology | Behavior |
//! |---|---|
//! | [`Topology::SingleRoom`] | All players in one room; identical to Dedicated but capped at 16. |
//! | [`Topology::Dedicated`] | Standard authoritative + interest-filtered delta path. |
//! | [`Topology::Sharded`] | Uses the [`ShardManager`] seam; single-shard in N1. |
//!
//! ## WebSocket protocol
//!
//! ```text
//! Client connects → server sends ServerNet::Welcome
//!                 ← client sends ClientNet::InputFrame (every tick)
//!                 → server sends ServerNet::Ack / Reject / Delta / Snapshot
//! Client disconnects → server removes connection + calls on_leave
//! ```

use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use magnetite_sdk::authority::{GameExecutor, MatchConfig, Topology};
use magnetite_sdk::protocol::{ClientNet, ServerNet};
use magnetite_sdk::state::PlayerId;

use crate::connection::ConnectionManager;
use crate::shard::ShardManager;
use crate::tick::TickScheduler;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Fatal server error (e.g. TCP bind failure).
#[derive(Debug)]
pub struct ServerError(String);

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ServerError {}

/// Simple, Send-safe error wrapper for WS send failures.
#[derive(Debug)]
struct SendError(String);

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SendError {}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for [`GameServer::serve`].
#[derive(Debug, Clone)]
pub struct GameServerConfig {
    /// Address to bind the WebSocket listener.
    ///
    /// Example: `"0.0.0.0:9000"` or `"127.0.0.1:9000"`.
    pub bind_addr: String,

    /// Match configuration (topology, tick rate, seed, …).
    pub match_config: MatchConfig,
}

impl Default for GameServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9000".to_string(),
            match_config: MatchConfig::auto(4),
        }
    }
}

/// Authoritative game-server host.
///
/// Stateless entry point — all state lives in the executor, connection manager,
/// and shard manager.
pub struct GameServer;

impl GameServer {
    /// Start the authoritative server.
    ///
    /// Blocks until a fatal error occurs or the process is killed.  For graceful
    /// shutdown in tests, use [`GameServer::serve_with_shutdown`].
    ///
    /// # Arguments
    ///
    /// * `executor` — any [`GameExecutor`] (typically [`NativeExecutor`] or
    ///   `WasmExecutor` from `magnetite-sandbox`).
    /// * `config` — server + match configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP listener cannot bind to `config.bind_addr`.
    pub async fn serve(
        executor: impl GameExecutor + 'static,
        config: GameServerConfig,
    ) -> Result<(), ServerError> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self::serve_with_shutdown(executor, config, shutdown_rx, shutdown_tx).await
    }

    /// Start the server with an explicit shutdown signal.
    ///
    /// Send `true` on `shutdown_tx` to trigger graceful shutdown.  This is
    /// used internally by the acceptance loop and is the recommended pattern
    /// for tests.
    pub async fn serve_with_shutdown(
        executor: impl GameExecutor + 'static,
        config: GameServerConfig,
        mut shutdown_rx: watch::Receiver<bool>,
        _shutdown_tx: watch::Sender<bool>,
    ) -> Result<(), ServerError> {
        let listener = TcpListener::bind(&config.bind_addr)
            .await
            .map_err(|e| ServerError(e.to_string()))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| ServerError(e.to_string()))?;
        info!(addr = %local_addr, topology = ?config.match_config.topology, "game server listening");

        // Build shared connection manager and shard manager.
        let conn_mgr = ConnectionManager::new();
        let shard_mgr = ShardManager::new(config.match_config.topology.clone());
        // Shard manager is single-threaded in N1 (all mutations happen on the
        // accept task which is single-threaded per connection).
        let shard_mgr = std::sync::Arc::new(tokio::sync::Mutex::new(shard_mgr));

        // Spawn the tick scheduler.
        let tick_conn_mgr = conn_mgr.clone();
        let tick_config = config.match_config.clone();
        let scheduler = TickScheduler::new(executor, tick_conn_mgr, tick_config);
        let tick_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            scheduler.run(tick_shutdown).await;
        });

        // Accept loop.
        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            let conn_mgr_clone = conn_mgr.clone();
                            let shard_mgr_clone = std::sync::Arc::clone(&shard_mgr);
                            let match_config = config.match_config.clone();
                            tokio::spawn(async move {
                                handle_connection(
                                    stream,
                                    peer_addr,
                                    conn_mgr_clone,
                                    shard_mgr_clone,
                                    match_config,
                                )
                                .await;
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "accept error");
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("server shutdown signal received");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Return the topology-specific player cap.
    pub fn player_cap(topology: &Topology) -> u32 {
        match topology {
            Topology::SingleRoom => 16,
            Topology::Dedicated { .. } => 256,
            Topology::Sharded { max_per_shard, .. } => *max_per_shard,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-connection handler
// ---------------------------------------------------------------------------

/// Drive a single WebSocket connection.
///
/// Performs the WS handshake, sends `ServerNet::Welcome`, then enters a loop
/// that forwards inbound `ClientNet` frames to the connection manager and
/// forwards outbound `ServerNet` frames from the tick loop to the client.
async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    conn_mgr: ConnectionManager,
    shard_mgr: std::sync::Arc<tokio::sync::Mutex<ShardManager>>,
    match_config: MatchConfig,
) {
    // WebSocket handshake.
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!(%peer_addr, error = %e, "WebSocket handshake failed");
            return;
        }
    };

    // Register player.
    let (player_id, mut outbound_rx) = conn_mgr.register().await;
    let _shard = shard_mgr.lock().await.assign(player_id);
    info!(%peer_addr, %player_id, "player connected");

    // Send Welcome.
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let welcome = ServerNet::Welcome {
        player_id,
        config: match_config.clone(),
    };
    if let Err(e) = send_server_net(&mut ws_tx, &welcome).await {
        warn!(%player_id, error = %e, "failed to send Welcome");
        cleanup(player_id, &conn_mgr, &shard_mgr).await;
        return;
    }

    // Drive the connection: forward inputs inbound, frames outbound.
    loop {
        tokio::select! {
            // Inbound: client → server.
            msg = ws_rx.next() => {
                match msg {
                    None => break, // client disconnected
                    Some(Err(e)) => {
                        warn!(%player_id, error = %e, "WebSocket receive error");
                        break;
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(
                            player_id,
                            text.as_bytes(),
                            &conn_mgr,
                        )
                        .await;
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        handle_client_message(player_id, &bytes, &conn_mgr).await;
                    }
                    Some(Ok(_)) => {} // Ping/Pong handled by tungstenite
                }
            }

            // Outbound: tick loop → client.
            frame = outbound_rx.recv() => {
                match frame {
                    None => break, // channel closed (server shutdown)
                    Some(net_msg) => {
                        if let Err(e) = send_server_net(&mut ws_tx, &net_msg).await {
                            warn!(%player_id, error = %e, "WebSocket send error");
                            break;
                        }
                    }
                }
            }
        }
    }

    cleanup(player_id, &conn_mgr, &shard_mgr).await;
    info!(%player_id, "player disconnected");
}

/// Deserialise and dispatch a raw client message.
async fn handle_client_message(player_id: PlayerId, bytes: &[u8], conn_mgr: &ConnectionManager) {
    let net_msg: ClientNet = match serde_json::from_slice(bytes) {
        Ok(m) => m,
        Err(e) => {
            warn!(%player_id, error = %e, "failed to parse ClientNet frame");
            return;
        }
    };

    match net_msg {
        ClientNet::InputFrame {
            seq,
            tick: _,
            input,
        } => {
            conn_mgr.push_input(player_id, seq, input).await;
        }
    }
}

/// Serialise and send a [`ServerNet`] frame as a JSON text WebSocket message.
async fn send_server_net<S>(sink: &mut S, msg: &ServerNet) -> Result<(), SendError>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let text = serde_json::to_string(msg).map_err(|e| SendError(e.to_string()))?;
    sink.send(Message::Text(text.into()))
        .await
        .map_err(|e| SendError(e.to_string()))?;
    Ok(())
}

/// Remove the player from all shared state after disconnect.
async fn cleanup(
    player_id: PlayerId,
    conn_mgr: &ConnectionManager,
    shard_mgr: &std::sync::Arc<tokio::sync::Mutex<ShardManager>>,
) {
    conn_mgr.remove(player_id).await;
    shard_mgr.lock().await.remove(player_id);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::authority::{
        AuthoritativeGame, MatchConfig, NativeExecutor, RejectReason, StepCtx,
    };
    use magnetite_sdk::input::Input;
    use magnetite_sdk::state::PlayerId;
    use std::time::Duration;

    struct NopGame;

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct NopSnap;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct NopDelta;

    #[derive(serde::Serialize)]
    struct NopView;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct NopCmd;

    impl AuthoritativeGame for NopGame {
        type Snapshot = NopSnap;
        type Delta = NopDelta;
        type View = NopView;
        type Command = NopCmd;

        fn init(_cfg: &MatchConfig) -> Self {
            NopGame
        }
        fn validate(
            &self,
            _p: PlayerId,
            _i: &Input,
            _t: crate::Tick,
        ) -> Result<Vec<NopCmd>, RejectReason> {
            Ok(vec![])
        }
        fn step(&mut self, _ctx: &mut StepCtx, _cmds: &[(PlayerId, NopCmd)]) {}
        fn snapshot(&self) -> NopSnap {
            NopSnap
        }
        fn restore(_s: &NopSnap, _cfg: &MatchConfig) -> Self {
            NopGame
        }
        fn delta(&self, _s: &NopSnap) -> NopDelta {
            NopDelta
        }
        fn view_for(&self, _p: PlayerId) -> NopView {
            NopView
        }
    }

    #[test]
    fn player_cap_values() {
        assert_eq!(GameServer::player_cap(&Topology::SingleRoom), 16);
        assert_eq!(
            GameServer::player_cap(&Topology::Dedicated { tick_hz: 60 }),
            256
        );
        assert_eq!(
            GameServer::player_cap(&Topology::Sharded {
                tick_hz: 20,
                cell_size: 500.0,
                max_per_shard: 64,
            }),
            64
        );
    }

    #[tokio::test]
    async fn server_starts_and_shuts_down() {
        let cfg = MatchConfig::auto(4);
        let executor = NativeExecutor::<NopGame>::new(cfg.clone());
        let server_cfg = GameServerConfig {
            bind_addr: "127.0.0.1:0".to_string(), // OS-assigned port
            match_config: cfg,
        };

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let handle = tokio::spawn(async move {
            GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx, shutdown_tx)
                .await
                .unwrap();
        });

        // Give the server a moment to bind, then shut it down.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The server should still be running.
        assert!(!handle.is_finished());

        // Abort (process-level shutdown).
        handle.abort();
    }

    #[tokio::test]
    async fn server_accepts_websocket_connection() {
        use tokio_tungstenite::connect_async;

        let cfg = MatchConfig::auto(4);
        let executor = NativeExecutor::<NopGame>::new(cfg.clone());

        // Bind on port 0 to let the OS pick an available port.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener); // release so the server can bind

        let server_cfg = GameServerConfig {
            bind_addr: addr.to_string(),
            match_config: cfg.clone(),
        };

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let server_handle = tokio::spawn(async move {
            GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx, shutdown_tx)
                .await
                .unwrap();
        });

        // Give server time to bind.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Connect a WebSocket client.
        let url = format!("ws://{addr}");
        let (mut ws, _) = connect_async(&url).await.expect("WebSocket connect failed");

        // First message should be ServerNet::Welcome.
        if let Some(Ok(Message::Text(text))) = ws.next().await {
            let msg: ServerNet =
                serde_json::from_str(&text).expect("should parse ServerNet::Welcome");
            assert!(
                matches!(msg, ServerNet::Welcome { .. }),
                "expected Welcome, got {msg:?}"
            );
        } else {
            panic!("expected text message from server");
        }

        // Close the connection.
        let _ = ws.close(None).await;
        server_handle.abort();
    }
}
