//! Authoritative game-server host.
//!
//! ## Entry points
//!
//! | Method | Description |
//! |---|---|
//! | [`GameServer::serve`] | Native executor (backward-compatible) |
//! | [`GameServer::serve_wasm`] | Sandboxed Wasm executor via `magnetite-sandbox` |
//! | [`GameServer::with_executor`] | Generic: bring your own `GameExecutor` |
//! | [`GameServer::serve_with_shutdown`] | Native executor + explicit shutdown handle |
//! | [`GameServer::serve_wasm_with_shutdown`] | Wasm executor + explicit shutdown handle |
//!
//! All entry points share the same inner serve loop; the executor is the only
//! difference between them.
//!
//! ## WebSocket protocol
//!
//! ```text
//! Client connects → server sends ServerNet::Welcome
//!                 ← client sends ClientNet::InputFrame (every tick)
//!                 → server sends ServerNet::Ack / Reject / Delta / Snapshot
//! Client disconnects → server removes connection + calls on_leave
//! ```
//!
//! ## Topology dispatch
//!
//! | Topology | Behavior |
//! |---|---|
//! | [`Topology::SingleRoom`] | All players in one room; identical to Dedicated but capped at 16. |
//! | [`Topology::Dedicated`] | Standard authoritative + interest-filtered delta path. |
//! | [`Topology::Sharded`] | Single-process multi-shard; [`ShardManager`] routes players. |
//!
//! ## Anticheat
//!
//! The tick loop always has an anticheat pipeline.  By default it uses the SDK
//! built-ins (`RateLimit` + `InputSchema`).  Pass a custom [`Anticheat`] via
//! [`GameServerConfig::anticheat`] to add game-specific validators.

use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use magnetite_anticheat::{Anticheat, AnticheatConfig};
use magnetite_sandbox::{LimitsConfig, WasmExecutor};
use magnetite_sdk::authority::{GameExecutor, MatchConfig, Topology, ValidatorChain};
use magnetite_sdk::protocol::{ClientNet, ServerNet};
use magnetite_sdk::state::PlayerId;

use crate::connection::ConnectionManager;
use crate::shard::ShardManager;
use crate::tick::TickScheduler;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Fatal server error (e.g. TCP bind failure, wasm load failure).
#[derive(Debug)]
pub struct ServerError(pub(crate) String);

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

/// Configuration for [`GameServer`].
///
/// Note: does not implement `Clone` because [`Anticheat`] is not `Clone`.
/// Move or reconstruct the config when multiple servers are needed.
pub struct GameServerConfig {
    /// Address to bind the WebSocket listener.
    ///
    /// Example: `"0.0.0.0:9000"` or `"127.0.0.1:9000"`.
    pub bind_addr: String,

    /// Match configuration (topology, tick rate, seed, …).
    pub match_config: MatchConfig,

    /// Optional custom anticheat pipeline.
    ///
    /// When `None`, the server uses the default chain:
    /// `RateLimit(120)` + `InputSchema`.
    ///
    /// Set this to `Some(anticheat)` to add game-specific validators (e.g.
    /// `AimbotSnap`, `PositionTeleport`) or tune kick/ban thresholds.
    pub anticheat: Option<Anticheat>,

    /// Optional fleet wiring: makes player *sessions* follow migrated shards.
    ///
    /// With `Some(session)` this listener additionally:
    /// - tracks each connected player against their shard, so the migration
    ///   path knows who to redirect;
    /// - delivers a [`crate::cluster::SignedRedirect`] on the player's live
    ///   socket after a migration commits, then closes that connection;
    /// - runs incoming `ClientNet::Follow` frames through
    ///   [`crate::cluster::FollowAdmission`] before attaching the player.
    ///
    /// With `None` the server behaves exactly as before — single-node hosting
    /// needs none of this.
    pub fleet: Option<crate::follow::FleetSession>,
}

impl Default for GameServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9000".to_string(),
            match_config: MatchConfig::auto(4),
            anticheat: None,
            fleet: None,
        }
    }
}

// ---------------------------------------------------------------------------
// GameServer
// ---------------------------------------------------------------------------

/// Authoritative game-server host.
///
/// Stateless entry point — all state lives in the executor, connection manager,
/// shard manager, and anticheat pipeline.
pub struct GameServer;

impl GameServer {
    // ---------------------------------------------------------------------- //
    // Native (backward-compatible) entry points                              //
    // ---------------------------------------------------------------------- //

    /// Start the authoritative server with a native (in-process) executor.
    ///
    /// This is the **primary entry point** and is backward-compatible with all
    /// existing callers.  It blocks until a fatal error or process kill.
    ///
    /// For graceful shutdown in tests, use [`GameServer::serve_with_shutdown`].
    pub async fn serve(
        executor: impl GameExecutor + 'static,
        config: GameServerConfig,
    ) -> Result<(), ServerError> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self::serve_with_shutdown(executor, config, shutdown_rx, shutdown_tx).await
    }

    /// Start the server with a native executor and an explicit shutdown signal.
    ///
    /// Send `true` on `shutdown_tx` to trigger graceful shutdown.
    pub async fn serve_with_shutdown(
        executor: impl GameExecutor + 'static,
        config: GameServerConfig,
        shutdown_rx: watch::Receiver<bool>,
        shutdown_tx: watch::Sender<bool>,
    ) -> Result<(), ServerError> {
        Self::serve_inner(Box::new(executor), config, shutdown_rx, shutdown_tx).await
    }

    // ---------------------------------------------------------------------- //
    // Wasm (sandboxed) entry points                                          //
    // ---------------------------------------------------------------------- //

    /// Start the authoritative server with a sandboxed Wasm executor.
    ///
    /// Loads the game module from `wasm_path`, applies `limits` for
    /// fuel/memory/epoch budgets, and serves the match exactly like
    /// [`GameServer::serve`].  The only difference is that the game logic runs
    /// inside a Wasmtime sandbox.
    ///
    /// # Errors
    ///
    /// Returns an error if the `.wasm` file cannot be read/compiled or the TCP
    /// listener cannot bind.
    pub async fn serve_wasm(
        wasm_path: impl AsRef<std::path::Path>,
        limits: LimitsConfig,
        config: GameServerConfig,
    ) -> Result<(), ServerError> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self::serve_wasm_with_shutdown(wasm_path, limits, config, shutdown_rx, shutdown_tx).await
    }

    /// Start the server with a Wasm executor and an explicit shutdown signal.
    pub async fn serve_wasm_with_shutdown(
        wasm_path: impl AsRef<std::path::Path>,
        limits: LimitsConfig,
        config: GameServerConfig,
        shutdown_rx: watch::Receiver<bool>,
        shutdown_tx: watch::Sender<bool>,
    ) -> Result<(), ServerError> {
        let executor = WasmExecutor::from_file(wasm_path, config.match_config.clone(), limits)
            .map_err(|e| ServerError(format!("wasm load error: {e}")))?;
        Self::serve_inner(Box::new(executor), config, shutdown_rx, shutdown_tx).await
    }

    // ---------------------------------------------------------------------- //
    // Generic entry point                                                     //
    // ---------------------------------------------------------------------- //

    /// Start the server with **any** [`GameExecutor`] you supply.
    ///
    /// This is the most flexible entry point.  It is equivalent to
    /// [`GameServer::serve`] for native executors and [`GameServer::serve_wasm`]
    /// for `WasmExecutor`, but lets you pass any custom implementation.
    ///
    /// ```rust,no_run
    /// use magnetite_runtime::{GameServer, GameServerConfig};
    /// use magnetite_sdk::authority::{MatchConfig, NativeExecutor};
    /// // (provide a game type that impls AuthoritativeGame)
    /// # struct MyGame;
    /// # impl magnetite_sdk::authority::AuthoritativeGame for MyGame {
    /// #   type Snapshot = (); type Delta = (); type View = (); type Command = ();
    /// #   fn init(_: &MatchConfig) -> Self { MyGame }
    /// #   fn validate(&self,_:magnetite_sdk::state::PlayerId,_:&magnetite_sdk::input::Input,_:magnetite_sdk::authority::Tick)->Result<Vec<()>,magnetite_sdk::authority::RejectReason>{Ok(vec![])}
    /// #   fn step(&mut self,_:&mut magnetite_sdk::authority::StepCtx,_:&[(magnetite_sdk::state::PlayerId,())]) {}
    /// #   fn snapshot(&self)->() {}
    /// #   fn restore(_:&(),_:&MatchConfig)->Self{MyGame}
    /// #   fn delta(&self,_:&())->() {}
    /// #   fn view_for(&self,_:magnetite_sdk::state::PlayerId)->() {}
    /// # }
    ///
    /// # async fn run() -> Result<(), magnetite_runtime::ServerError> {
    /// let cfg = MatchConfig::auto(4);
    /// let executor = NativeExecutor::<MyGame>::new(cfg.clone());
    /// let server_cfg = GameServerConfig {
    ///     bind_addr: "127.0.0.1:9000".to_string(),
    ///     match_config: cfg,
    ///     anticheat: None,
    ///     fleet: None,
    /// };
    /// GameServer::with_executor(Box::new(executor), server_cfg).await
    /// # }
    /// ```
    pub async fn with_executor(
        executor: Box<dyn GameExecutor + 'static>,
        config: GameServerConfig,
    ) -> Result<(), ServerError> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self::serve_inner(executor, config, shutdown_rx, shutdown_tx).await
    }

    // ---------------------------------------------------------------------- //
    // Utility                                                                 //
    // ---------------------------------------------------------------------- //

    /// Return the topology-specific player cap.
    pub fn player_cap(topology: &Topology) -> u32 {
        match topology {
            Topology::SingleRoom => 16,
            Topology::Dedicated { .. } => 256,
            Topology::Sharded { max_per_shard, .. } => *max_per_shard,
        }
    }

    // ---------------------------------------------------------------------- //
    // Shared inner serve loop                                                 //
    // ---------------------------------------------------------------------- //

    async fn serve_inner(
        executor: Box<dyn GameExecutor + 'static>,
        mut config: GameServerConfig,
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
        let shard_mgr = std::sync::Arc::new(tokio::sync::Mutex::new(shard_mgr));

        // Fleet wiring (optional).
        let fleet = config.fleet.take();

        // Build the anticheat pipeline.
        let anticheat = config.anticheat.take().unwrap_or_else(|| {
            Anticheat::new(
                ValidatorChain::new()
                    .add(magnetite_sdk::authority::RateLimit::new(120))
                    .add(magnetite_sdk::authority::InputSchema::default()),
                AnticheatConfig::default(),
            )
        });

        // Spawn the tick scheduler.
        let tick_conn_mgr = conn_mgr.clone();
        let tick_config = config.match_config.clone();
        let scheduler =
            TickScheduler::with_anticheat(executor, tick_conn_mgr, tick_config, anticheat);
        let tick_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            scheduler.run(tick_shutdown).await;
        });

        // Spawn the redirect pump: when a shard migrates away from this node,
        // the players who were on it get their signed redirect on the socket
        // they are already using. The queue is only ever filled by the success
        // arm of `migrate_shard`, past a verified CommitAck — a failed or
        // rolled-back migration leaves nothing here to deliver.
        if let Some(f) = fleet.clone() {
            let pump_conn_mgr = conn_mgr.clone();
            let mut pump_shutdown = shutdown_rx.clone();
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(REDIRECT_PUMP_INTERVAL);
                loop {
                    tokio::select! {
                        _ = ticker.tick() => {
                            for r in f.drain_redirects() {
                                let pid = PlayerId::new(r.player);
                                let frame = match serde_json::to_value(&r) {
                                    Ok(v) => ServerNet::Redirect { redirect: v },
                                    Err(e) => {
                                        error!(error = %e, "could not encode redirect");
                                        continue;
                                    }
                                };
                                info!(
                                    %pid,
                                    shard = r.shard,
                                    epoch = r.epoch,
                                    target = %r.target_key.to_hex(),
                                    "delivering signed redirect — session follows the shard"
                                );
                                pump_conn_mgr.send_to(pid, frame).await;
                            }
                        }
                        _ = pump_shutdown.changed() => {
                            if *pump_shutdown.borrow() { break; }
                        }
                    }
                }
            });
        }

        // Accept loop.
        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            let conn_mgr_clone = conn_mgr.clone();
                            let shard_mgr_clone = std::sync::Arc::clone(&shard_mgr);
                            let match_config = config.match_config.clone();
                            let fleet_clone = fleet.clone();
                            tokio::spawn(async move {
                                handle_connection(
                                    stream,
                                    peer_addr,
                                    conn_mgr_clone,
                                    shard_mgr_clone,
                                    match_config,
                                    fleet_clone,
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
}

// ---------------------------------------------------------------------------
// Per-connection handler
// ---------------------------------------------------------------------------

/// How often the redirect pump drains freshly-minted redirects.
const REDIRECT_PUMP_INTERVAL: std::time::Duration = std::time::Duration::from_millis(20);

/// What the inbound frame handler wants the connection loop to do next.
enum Inbound {
    /// Nothing further (the common input path).
    Nothing,
    /// Send this frame back to the client.
    Reply(Box<ServerNet>),
    /// The client presented a follow redirect that
    /// [`crate::cluster::FollowAdmission`] accepted: adopt their original
    /// player id on the named shard.
    Followed { player: PlayerId, shard: crate::shard::ShardId },
    /// The client presented a follow this node refused. Fail closed: the
    /// connection is dropped rather than degraded to an anonymous session.
    Refused(String),
}

/// Drive a single WebSocket connection.
async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    conn_mgr: ConnectionManager,
    shard_mgr: std::sync::Arc<tokio::sync::Mutex<ShardManager>>,
    match_config: MatchConfig,
    fleet: Option<crate::follow::FleetSession>,
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
    let (mut player_id, mut outbound_rx) = conn_mgr.register().await;
    let mut shard = shard_mgr.lock().await.assign(player_id);
    if let Some(f) = &fleet {
        f.attach_player(shard, player_id.as_u64());
    }
    info!(%peer_addr, %player_id, "player connected");

    // Per-connection ingress for attested sensor input (seam §3.7). Owned by
    // this connection, not shared: one peer's flood must not spend another
    // peer's budget. Its queue is `InputClass::Attested` — nothing drained from
    // it is replay-verifiable, and it is deliberately kept apart from the
    // deterministic input path that `ConnectionManager` feeds.
    let attested = crate::attested::AttestedIngress::default();

    // Send Welcome.
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let welcome = ServerNet::Welcome {
        player_id,
        config: match_config.clone(),
    };
    if let Err(e) = send_server_net(&mut ws_tx, &welcome).await {
        warn!(%player_id, error = %e, "failed to send Welcome");
        cleanup(player_id, shard, &conn_mgr, &shard_mgr, &fleet).await;
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
                        let action = handle_client_message(
                            player_id,
                            text.as_bytes(),
                            &conn_mgr,
                            &fleet,
                            &attested,
                        )
                        .await;
                        if !apply_inbound(
                            action, &mut player_id, &mut shard, &mut outbound_rx,
                            &mut ws_tx, &conn_mgr, &shard_mgr, &fleet, &match_config,
                        ).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        let action =
                            handle_client_message(player_id, &bytes, &conn_mgr, &fleet, &attested)
                                .await;
                        if !apply_inbound(
                            action, &mut player_id, &mut shard, &mut outbound_rx,
                            &mut ws_tx, &conn_mgr, &shard_mgr, &fleet, &match_config,
                        ).await {
                            break;
                        }
                    }
                    Some(Ok(_)) => {} // Ping/Pong handled by tungstenite
                }
            }

            // Outbound: tick loop → client.
            frame = outbound_rx.recv() => {
                match frame {
                    None => break, // channel closed (server shutdown)
                    Some(net_msg) => {
                        // A redirect is terminal for this connection: the shard
                        // this session was on now lives elsewhere, so there is
                        // nothing left here to be authoritative about. Deliver
                        // it, then close — the client reconnects to the target.
                        let is_redirect = matches!(net_msg, ServerNet::Redirect { .. });
                        if let Err(e) = send_server_net(&mut ws_tx, &net_msg).await {
                            warn!(%player_id, error = %e, "WebSocket send error");
                            break;
                        }
                        if is_redirect {
                            let _ = ws_tx.send(Message::Close(None)).await;
                            info!(%player_id, "session redirected — closing connection here");
                            break;
                        }
                    }
                }
            }
        }
    }

    cleanup(player_id, shard, &conn_mgr, &shard_mgr, &fleet).await;
    info!(%player_id, "player disconnected");
}

/// Apply the outcome of an inbound frame. Returns `false` when the connection
/// should be closed.
#[allow(clippy::too_many_arguments)]
async fn apply_inbound<S>(
    action: Inbound,
    player_id: &mut PlayerId,
    shard: &mut crate::shard::ShardId,
    outbound_rx: &mut tokio::sync::mpsc::Receiver<ServerNet>,
    ws_tx: &mut S,
    conn_mgr: &ConnectionManager,
    shard_mgr: &std::sync::Arc<tokio::sync::Mutex<ShardManager>>,
    fleet: &Option<crate::follow::FleetSession>,
    match_config: &MatchConfig,
) -> bool
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    match action {
        Inbound::Nothing => true,
        Inbound::Reply(frame) => send_server_net(ws_tx, &frame).await.is_ok(),
        Inbound::Refused(why) => {
            warn!(%player_id, reason = %why, "refusing session follow — closing connection");
            false
        }
        Inbound::Followed {
            player: followed_id,
            shard: followed_shard,
        } => {
            // Drop the provisional anonymous identity this connection was given
            // on accept, and adopt the id the (verified) redirect was minted
            // for. That is what makes the session continuous across the move.
            let provisional = *player_id;
            cleanup(provisional, *shard, conn_mgr, shard_mgr, fleet).await;

            let Some(rx) = conn_mgr.register_as(followed_id).await else {
                warn!(%followed_id, "follow refused: that player is already connected here");
                return false;
            };
            *outbound_rx = rx;
            *player_id = followed_id;
            *shard = shard_mgr.lock().await.place(followed_id, followed_shard);
            if let Some(f) = fleet {
                f.attach_player(followed_shard, followed_id.as_u64());
            }
            info!(
                %followed_id,
                shard = followed_shard.0,
                "session follow admitted — player attached with their original id"
            );
            let welcome = ServerNet::Welcome {
                player_id: followed_id,
                config: match_config.clone(),
            };
            send_server_net(ws_tx, &welcome).await.is_ok()
        }
    }
}

/// Deserialise and dispatch a raw client message.
async fn handle_client_message(
    player_id: PlayerId,
    bytes: &[u8],
    conn_mgr: &ConnectionManager,
    fleet: &Option<crate::follow::FleetSession>,
    attested: &crate::attested::AttestedIngress,
) -> Inbound {
    let net_msg: ClientNet = match serde_json::from_slice(bytes) {
        Ok(m) => m,
        Err(e) => {
            // An attested frame that failed to parse gets an explicit refusal
            // rather than silence — most often because it was the *unsigned*
            // shape, which carries no authorship binding and therefore has no
            // wire representation at all. Telling the client beats leaving it to
            // infer a drop from a missing ack.
            if is_attested_frame(bytes) {
                let refusal = attested.refuse_malformed(e.to_string(), crate::attested::now_ms());
                warn!(%player_id, reason = %refusal, "refusing attested frame");
                return Inbound::Reply(Box::new(ServerNet::AttestedReject {
                    seq: 0,
                    reason: refusal.to_string(),
                }));
            }
            warn!(%player_id, error = %e, "failed to parse ClientNet frame");
            return Inbound::Nothing;
        }
    };

    match net_msg {
        ClientNet::InputFrame {
            seq,
            tick: _,
            input,
        } => {
            conn_mgr.push_input(player_id, seq, input).await;
            Inbound::Nothing
        }

        // Prove which node key we hold, over a nonce the client chose. This is
        // what lets a client pin a node key — including the `target_key` from a
        // redirect it just verified — and refuse an impostor at the address.
        ClientNet::Hello { nonce } => match fleet {
            Some(f) => {
                let key = f.node_key();
                Inbound::Reply(Box::new(ServerNet::NodeIdentity {
                    node_key: key.to_hex(),
                    nonce: nonce.clone(),
                    sig: f.sign_hello(&nonce),
                }))
            }
            // A node with no fleet identity has no key to prove. Say nothing
            // rather than something reassuring.
            None => Inbound::Nothing,
        },

        // A player following a shard that migrated here. Every check lives in
        // `FollowAdmission::admit`; this arm only routes the answer.
        ClientNet::Follow { redirect } => {
            let Some(f) = fleet else {
                return Inbound::Refused("this node does not accept session follows".into());
            };
            match f.admit_follow_json(&redirect, crate::follow::now_secs()) {
                Ok((player, shard)) => Inbound::Followed {
                    player: PlayerId::new(player),
                    shard,
                },
                Err(e) => Inbound::Refused(e.to_string()),
            }
        }

        // Client-attested sensor input (seam §3.7). This arm is the *only* way
        // an attested event enters the process, and it routes to
        // `AttestedIngress` and nowhere else — in particular never to
        // `conn_mgr.push_input`, which is the deterministic, replay-verifiable
        // path. Mixing them would leave `verify_replay` passing while no longer
        // proving anything.
        //
        // Admission here means "signed by the key it names, and not physically
        // impossible". It is not verification and not anti-cheat; see
        // `crate::attested`.
        ClientNet::AttestedEvent { signed } => {
            match attested.accept(&signed, crate::attested::now_ms()).await {
                Ok(seq) => Inbound::Reply(Box::new(ServerNet::AttestedAck { seq })),
                Err(refusal) => {
                    warn!(%player_id, reason = %refusal, "refusing attested event");
                    Inbound::Reply(Box::new(ServerNet::AttestedReject {
                        seq: signed.event.seq,
                        reason: refusal.to_string(),
                    }))
                }
            }
        }
    }
}

/// Cheap peek at an unparseable frame's `"type"` tag.
///
/// Used only to decide whether a parse failure deserves an explicit attested
/// refusal. Deliberately does not attempt any recovery of the payload: a frame
/// that did not deserialize is refused, and this just makes the refusal
/// legible.
fn is_attested_frame(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|v| v.get("type")?.as_str().map(|s| s == "attested_event"))
        .unwrap_or(false)
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
    shard: crate::shard::ShardId,
    conn_mgr: &ConnectionManager,
    shard_mgr: &std::sync::Arc<tokio::sync::Mutex<ShardManager>>,
    fleet: &Option<crate::follow::FleetSession>,
) {
    conn_mgr.remove(player_id).await;
    shard_mgr.lock().await.remove(player_id);
    // Stop tracking them for redirects: a session that is gone must not be
    // minted a live follow credential by the next migration.
    if let Some(f) = fleet {
        f.detach_player(shard, player_id.as_u64());
    }
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
            anticheat: None,
            fleet: None,
        };

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let handle = tokio::spawn(async move {
            GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx, shutdown_tx)
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!handle.is_finished());
        handle.abort();
    }

    #[tokio::test]
    async fn server_accepts_websocket_connection() {
        use tokio_tungstenite::connect_async;

        let cfg = MatchConfig::auto(4);
        let executor = NativeExecutor::<NopGame>::new(cfg.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_cfg = GameServerConfig {
            bind_addr: addr.to_string(),
            match_config: cfg.clone(),
            anticheat: None,
            fleet: None,
        };

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let server_handle = tokio::spawn(async move {
            GameServer::serve_with_shutdown(executor, server_cfg, shutdown_rx, shutdown_tx)
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let url = format!("ws://{addr}");
        let (mut ws, _) = connect_async(&url).await.expect("WebSocket connect failed");

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

        let _ = ws.close(None).await;
        server_handle.abort();
    }

    /// `GameServer::with_executor` should compile and work identically to `serve`.
    #[tokio::test]
    async fn with_executor_starts_and_accepts() {
        use tokio_tungstenite::connect_async;

        let cfg = MatchConfig::auto(4);
        let executor = NativeExecutor::<NopGame>::new(cfg.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_cfg = GameServerConfig {
            bind_addr: addr.to_string(),
            match_config: cfg,
            anticheat: None,
            fleet: None,
        };

        let handle = tokio::spawn(async move {
            GameServer::with_executor(Box::new(executor), server_cfg)
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let url = format!("ws://{addr}");
        let (mut ws, _) = connect_async(&url).await.expect("WebSocket connect failed");

        if let Some(Ok(Message::Text(text))) = ws.next().await {
            let msg: ServerNet = serde_json::from_str(&text).expect("should parse ServerNet");
            assert!(matches!(msg, ServerNet::Welcome { .. }));
        } else {
            panic!("expected Welcome");
        }

        let _ = ws.close(None).await;
        handle.abort();
    }

    /// `GameServerConfig::anticheat` field accepts a custom `Anticheat`.
    #[test]
    fn server_config_with_custom_anticheat() {
        let chain = ValidatorChain::new();
        let ac = Anticheat::new(chain, AnticheatConfig::default());
        let cfg = GameServerConfig {
            bind_addr: "127.0.0.1:9000".to_string(),
            match_config: MatchConfig::auto(4),
            anticheat: Some(ac),
        fleet: None,
        };
        // `anticheat` field is `Some` — assert it holds a value.
        assert!(cfg.anticheat.is_some());
    }
}
