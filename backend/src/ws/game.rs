// Game WebSocket handler and server-authoritative game loop.
// Clients connect to /ws/game/<game_id>?token=<jwt>.
// On connect: the JWT is validated; unauthenticated connections are rejected.
//   Ban check: check_ban() is called on every connect; banned users are closed immediately.
// Input frames from the client are processed by the server-authoritative loop
// and suspicious inputs are flagged via the anti-cheat service.
// On session end: detect_anomalies() runs; high/critical findings trigger ban_user() +
//   store_replay() so the session data persists for review.
// Several game-logic helpers (physics, interpolation) are platform APIs intended for
// custom game-logic crates; dead_code is suppressed so the surface compiles cleanly.
#![allow(dead_code)]

use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::time::interval;
use uuid::Uuid;

use crate::api::middleware::validate_token;
use crate::services::anticheat::{
    self as anticheat_svc, DeviceFingerprint, Input as AntiCheatInput,
    Position as AntiCheatPosition, SessionData, Severity,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameMessage {
    PlayerJoin { player_id: String },
    PlayerLeave { player_id: String },
    Input { player_id: String, data: InputData },
    StateUpdate { state: GameState },
    Chat { player_id: String, message: String },
    Ping,
    Pong,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InputData {
    pub keys: Vec<String>,
    pub mouse: Option<MouseData>,
    pub timestamp: u64,
    pub seq: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MouseData {
    pub x: f32,
    pub y: f32,
    pub buttons: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameState {
    pub players: HashMap<String, PlayerState>,
    pub entities: HashMap<String, EntityState>,
    pub tick: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerState {
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub last_input_seq: u64,
    pub interpolated: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntityState {
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub snapshot_time: f64,
}

#[derive(Clone, Debug)]
pub struct QueuedInput {
    pub player_id: String,
    pub data: InputData,
    pub received_at: Instant,
}

#[derive(Clone)]
pub struct ClientSnapshot {
    pub tick: u64,
    pub state: GameState,
    pub timestamp: f64,
}

pub struct GameSession {
    id: String,
    players: HashMap<String, PlayerInfo>,
    pending_inputs: Vec<QueuedInput>,
    state: GameState,
    state_history: VecDeque<ClientSnapshot>,
    last_state_broadcast: Instant,
    tick_rate: u64,
    interpolation_delay_ticks: u64,
    cleanup_timer: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
pub struct PlayerInfo {
    pub player_id: String,
    pub sender: broadcast::Sender<GameMessage>,
    pub last_pong: Instant,
    pub confirmed_input_seq: u64,
}

impl GameSession {
    pub fn new(id: String) -> Self {
        Self {
            id,
            players: HashMap::new(),
            pending_inputs: Vec::new(),
            state: GameState {
                players: HashMap::new(),
                entities: HashMap::new(),
                tick: 0,
            },
            state_history: VecDeque::new(),
            last_state_broadcast: Instant::now(),
            tick_rate: 60,
            interpolation_delay_ticks: 3,
            cleanup_timer: None,
        }
    }

    pub fn register_player(&mut self, player_id: String, sender: broadcast::Sender<GameMessage>) {
        let player_state = PlayerState {
            x: rand_position(),
            y: rand_position(),
            rotation: 0.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            last_input_seq: 0,
            interpolated: false,
        };
        self.state.players.insert(player_id.clone(), player_state);
        self.players.insert(
            player_id.clone(),
            PlayerInfo {
                player_id,
                sender,
                last_pong: Instant::now(),
                confirmed_input_seq: 0,
            },
        );
    }

    pub fn handle_input(&mut self, player_id: String, input: InputData) {
        self.pending_inputs.push(QueuedInput {
            player_id,
            data: input.clone(),
            received_at: Instant::now(),
        });
    }

    pub fn process_inputs(&mut self) {
        for queued in &self.pending_inputs {
            if let Some(player) = self.state.players.get_mut(&queued.player_id) {
                process_player_input(player, &queued.data);
                player.last_input_seq = queued.data.seq;
            }
        }
        self.pending_inputs.clear();
    }

    pub fn tick(&mut self) {
        self.state.tick += 1;

        for player in self.state.players.values_mut() {
            update_player_physics(player);
        }

        self.state_history.push_back(ClientSnapshot {
            tick: self.state.tick,
            state: self.state.clone(),
            timestamp: now_as_f64(),
        });

        while self.state_history.len() > 120 {
            self.state_history.pop_front();
        }
    }

    pub fn get_interpolated_state(
        &self,
        _player_id: &str,
        interpolation_time: f64,
    ) -> Option<GameState> {
        let delayed_tick = self
            .state
            .tick
            .saturating_sub(self.interpolation_delay_ticks);

        let (before, after) = self.find_snapshot_bounds(delayed_tick)?;

        let t = (interpolation_time - before.timestamp) / (after.timestamp - before.timestamp);
        let t = t.clamp(0.0, 1.0);

        let mut interpolated = GameState {
            players: HashMap::new(),
            entities: HashMap::new(),
            tick: delayed_tick,
        };

        for (id, before_player) in &before.state.players {
            if let Some(after_player) = after.state.players.get(id) {
                let interp_x = lerp(before_player.x, after_player.x, t as f32);
                let interp_y = lerp(before_player.y, after_player.y, t as f32);
                let interp_rot =
                    lerp_angle(before_player.rotation, after_player.rotation, t as f32);

                interpolated.players.insert(
                    id.clone(),
                    PlayerState {
                        x: interp_x,
                        y: interp_y,
                        rotation: interp_rot,
                        velocity_x: after_player.velocity_x,
                        velocity_y: after_player.velocity_y,
                        last_input_seq: after_player.last_input_seq,
                        interpolated: true,
                    },
                );
            }
        }

        for (id, before_entity) in &before.state.entities {
            if let Some(after_entity) = after.state.entities.get(id) {
                let interp_x = lerp(before_entity.x, after_entity.x, t as f32);
                let interp_y = lerp(before_entity.y, after_entity.y, t as f32);
                let interp_rot =
                    lerp_angle(before_entity.rotation, after_entity.rotation, t as f32);

                interpolated.entities.insert(
                    id.clone(),
                    EntityState {
                        x: interp_x,
                        y: interp_y,
                        rotation: interp_rot,
                        snapshot_time: t,
                    },
                );
            }
        }

        Some(interpolated)
    }

    fn find_snapshot_bounds(&self, target_tick: u64) -> Option<(ClientSnapshot, ClientSnapshot)> {
        if self.state_history.len() < 2 {
            return None;
        }

        let mut before_idx = 0;
        for (i, snapshot) in self.state_history.iter().enumerate() {
            if snapshot.tick <= target_tick {
                before_idx = i;
            } else {
                let after_idx = i;
                return Some((
                    self.state_history[before_idx].clone(),
                    self.state_history[after_idx].clone(),
                ));
            }
        }

        let last = self.state_history.len() - 1;
        Some((
            self.state_history[before_idx].clone(),
            self.state_history[last].clone(),
        ))
    }

    pub fn remove_player(&mut self, player_id: &str) {
        self.state.players.remove(player_id);
        self.players.remove(player_id);
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    pub fn broadcast_state(&self, msg: GameMessage) {
        for player in self.players.values() {
            let _ = player.sender.send(msg.clone());
        }
    }
}

fn process_player_input(player: &mut PlayerState, input: &InputData) {
    const SPEED: f32 = 5.0;

    for key in &input.keys {
        match key.as_str() {
            "w" | "ArrowUp" => player.velocity_y = -SPEED,
            "s" | "ArrowDown" => player.velocity_y = SPEED,
            "a" | "ArrowLeft" => player.velocity_x = -SPEED,
            "d" | "ArrowRight" => player.velocity_x = SPEED,
            _ => {}
        }
    }

    if let Some(mouse) = &input.mouse {
        player.rotation = mouse.y.atan2(mouse.x);
    }
}

fn update_player_physics(player: &mut PlayerState) {
    const FRICTION: f32 = 0.9;
    const GRAVITY: f32 = 0.5;

    player.x += player.velocity_x;
    player.y += player.velocity_y;

    player.velocity_x *= FRICTION;
    player.velocity_y *= FRICTION;
    player.velocity_y += GRAVITY;

    player.x = player.x.max(0.0).min(1000.0);
    player.y = player.y.max(0.0).min(1000.0);
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let diff = b - a;
    let diff =
        ((diff + std::f32::consts::PI) % (2.0 * std::f32::consts::PI)) - std::f32::consts::PI;
    a + diff * t
}

fn rand_position() -> f32 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos as f32 % 800.0) + 100.0
}

fn now_as_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

pub struct GameManager {
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<GameSession>>>>>,
    tick_interval: Duration,
    /// Live observability gauge; updated whenever the sessions map changes.
    gauges: Option<Arc<crate::ws::gauges::WsGauges>>,
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tick_interval: Duration::from_millis(16),
            gauges: None,
        }
    }

    /// Attach the live-session gauge (called from `GameWsHandler::new`).
    pub fn with_gauges(mut self, gauges: Arc<crate::ws::gauges::WsGauges>) -> Self {
        self.gauges = Some(gauges);
        self
    }

    fn sync_session_gauge(&self, count: usize) {
        if let Some(g) = &self.gauges {
            g.set_game_sessions(count as u64);
        }
    }

    /// A clone of the gauges handle, if attached (used to count game sockets).
    pub fn gauges_handle(&self) -> Option<Arc<crate::ws::gauges::WsGauges>> {
        self.gauges.clone()
    }

    pub async fn get_or_create_session(&self, game_id: &str) -> Arc<Mutex<GameSession>> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(game_id) {
            return Arc::clone(session);
        }
        let session = GameSession::new(game_id.to_string());
        let arc = Arc::new(Mutex::new(session));
        sessions.insert(game_id.to_string(), Arc::clone(&arc));
        self.sync_session_gauge(sessions.len());
        arc
    }

    pub async fn start_game_loop(&self, game_id: String) {
        let sessions = Arc::clone(&self.sessions);
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(16));
            loop {
                ticker.tick().await;

                let session = {
                    let sessions = sessions.lock().await;
                    sessions.get(&game_id).map(Arc::clone)
                };

                if let Some(session) = session {
                    let mut session = session.lock().await;
                    session.process_inputs();
                    session.tick();

                    let state = session.state.clone();
                    let msg = GameMessage::StateUpdate { state };
                    session.broadcast_state(msg);
                } else {
                    break;
                }
            }
        });
    }

    pub async fn cleanup_empty_sessions(&self) {
        let mut sessions = self.sessions.lock().await;
        let to_remove: Vec<String> = sessions
            .iter()
            .filter_map(|(id, session)| {
                if session.try_lock().map(|s| s.is_empty()).unwrap_or(false) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();

        for id in to_remove {
            tracing::info!("Cleaning up empty game session: {}", id);
            sessions.remove(&id);
        }
        self.sync_session_gauge(sessions.len());
    }

    pub async fn get_session(&self, game_id: &str) -> Option<Arc<Mutex<GameSession>>> {
        let sessions = self.sessions.lock().await;
        sessions.get(game_id).map(Arc::clone)
    }
}

impl Default for GameManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct GameWsHandler {
    manager: Arc<GameManager>,
    /// DB pool injected so the WS handler can call DB-backed anti-cheat functions
    /// (check_ban on connect, ban_user + store_replay at session end).
    pool: sqlx::PgPool,
}

impl GameWsHandler {
    pub fn new(pool: sqlx::PgPool, gauges: Arc<crate::ws::gauges::WsGauges>) -> Self {
        Self {
            manager: Arc::new(GameManager::new().with_gauges(gauges)),
            pool,
        }
    }

    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/ws/game/:game_id", get(handle_game_connection))
            .with_state(self)
    }

    pub fn get_manager(&self) -> Arc<GameManager> {
        Arc::clone(&self.manager)
    }
}

/// Query params for the game WebSocket upgrade (mirrors `ws/comms.rs`).
#[derive(Debug, Deserialize)]
pub struct GameWsQuery {
    pub token: Option<String>,
}

async fn handle_game_connection(
    ws: WebSocketUpgrade,
    State(handler): State<Arc<GameWsHandler>>,
    Path(game_id): Path<String>,
    Query(query): Query<GameWsQuery>,
) -> axum::response::Response {
    // ── Auth on connect ──────────────────────────────────────────────────────
    // Validate the JWT supplied in the ?token= query parameter.  Connections
    // without a valid token are rejected before the WebSocket handshake
    // completes (returning 400 is the only option before upgrade).
    let user_uuid = match query
        .token
        .as_deref()
        .and_then(|t| validate_token(t).ok())
        .and_then(|c| Uuid::parse_str(&c.sub).ok())
    {
        Some(id) => id,
        None => {
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::UNAUTHORIZED)
                .body(axum::body::Body::from("missing or invalid token"))
                .unwrap();
        }
    };

    // ── Anti-cheat ban check on connect ──────────────────────────────────────
    // Build a minimal fingerprint (no real HTTP headers available at WS upgrade
    // time via Axum's WebSocketUpgrade extractor, so we supply a zeroed sentinel
    // that still allows the user-ID–based ban path to fire correctly).
    let connect_fingerprint = DeviceFingerprint {
        user_agent: "ws-connect".to_string(),
        screen_resolution: "unknown".to_string(),
        timezone: "unknown".to_string(),
        language: "unknown".to_string(),
        ip_address: "unknown".to_string(),
        hash: format!("ws-{}", user_uuid),
    };
    match anticheat_svc::check_ban(&handler.pool, user_uuid, &connect_fingerprint).await {
        Ok(true) => {
            tracing::warn!(user_id = %user_uuid, "Game WS: banned user rejected on connect");
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::FORBIDDEN)
                .body(axum::body::Body::from("banned"))
                .unwrap();
        }
        Err(e) => {
            // Log but don't block on DB errors — fail-open so a DB blip doesn't
            // shut out legitimate players.
            tracing::error!(user_id = %user_uuid, error = %e, "Anti-cheat ban check DB error");
        }
        Ok(false) => {}
    }

    let manager = handler.get_manager();
    let pool = handler.pool.clone();

    let conn_gauges = manager.gauges_handle();
    ws.on_upgrade(move |socket| async move {
        // Count this game socket in the live WS gauge for its whole lifetime.
        let _conn = conn_gauges.map(crate::ws::gauges::ConnGuard::new);
        let (mut write, mut read) = socket.split();
        let (tx, mut rx) = broadcast::channel::<GameMessage>(100);

        let manager_clone = Arc::clone(&manager);
        let tx_ping = tx.clone();

        // ── Keepalive ping task ───────────────────────────────────────────────
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(30));
            loop {
                ticker.tick().await;
                if tx_ping.send(GameMessage::Ping).is_err() {
                    break;
                }
            }
        });

        // ── Write task — forward broadcast frames to the WebSocket ────────────
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if write
                        .send(axum::extract::ws::Message::Text(json))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });

        // ── Session / player setup ────────────────────────────────────────────
        let session = manager_clone.get_or_create_session(&game_id).await;

        // Use the authenticated user's UUID as the canonical player_id so the
        // game loop, anti-cheat, and replay can be cross-referenced by user.
        let player_id = user_uuid.to_string();

        {
            let mut session_lock = session.lock().await;
            session_lock.register_player(player_id.clone(), tx.clone());

            // Send the current state snapshot immediately on join.
            let state_msg = GameMessage::StateUpdate {
                state: session_lock.state.clone(),
            };
            let _ = tx.send(state_msg);

            // Announce the new player to everyone already in the room.
            session_lock.broadcast_state(GameMessage::PlayerJoin {
                player_id: player_id.clone(),
            });
        }

        // Also deliver the join notification to the new player's own channel.
        let _ = tx.send(GameMessage::PlayerJoin {
            player_id: player_id.clone(),
        });

        // ── Anti-cheat state tracking for this session ────────────────────────
        // We accumulate position snapshots and inputs so we can run
        // detect_anomalies() at disconnect or on a periodic basis.
        let ac_session_id = Uuid::new_v4();
        let ac_start = Utc::now();
        let mut ac_inputs: Vec<AntiCheatInput> = Vec::new();
        let mut ac_positions: Vec<AntiCheatPosition> = Vec::new();
        // Scores are submitted by the game layer; the base WS loop doesn't track them.
        let ac_scores: Vec<i64> = Vec::new();

        // Read from config or env for velocity threshold.
        let max_velocity: f64 = std::env::var("ANTICHEAT_MAX_VELOCITY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50.0);

        let player_id_clone = player_id.clone();

        // ── Cleanup timer (runs in a nested spawn) ────────────────────────────
        {
            let mut session_lock = session.lock().await;
            session_lock.cleanup_timer = Some(tokio::spawn(async move {
                let mut cleanup_interval = interval(Duration::from_secs(60));
                loop {
                    cleanup_interval.tick().await;
                    manager_clone.cleanup_empty_sessions().await;
                }
            }));
        }

        // ── Main read loop ────────────────────────────────────────────────────
        while let Some(result) = read.next().await {
            if let Ok(axum::extract::ws::Message::Text(text)) = result {
                if let Ok(msg) = serde_json::from_str::<GameMessage>(&text) {
                    let mut session_lock = session.lock().await;
                    match &msg {
                        GameMessage::Input { player_id: pid, data } => {
                            // ── Anti-cheat: record input for anomaly detection ──
                            ac_inputs.push(AntiCheatInput {
                                timestamp: data.timestamp as f64,
                                input_type: "key".to_string(),
                                data: serde_json::json!({ "keys": data.keys }),
                            });

                            // Snapshot position from current authoritative state.
                            if let Some(ps) = session_lock.state.players.get(pid.as_str()) {
                                let new_pos = AntiCheatPosition {
                                    x: ps.x as f64,
                                    y: ps.y as f64,
                                    z: 0.0,
                                };
                                // ── Velocity check (real-time, per-input) ──────
                                if !ac_positions.is_empty() {
                                    let prev = ac_positions.last().unwrap();
                                    let dx = new_pos.x - prev.x;
                                    let dy = new_pos.y - prev.y;
                                    let dist = (dx * dx + dy * dy).sqrt();
                                    // time_delta: 1 game tick ≈ 16 ms
                                    let speed = dist / 0.016;
                                    if speed > max_velocity {
                                        tracing::warn!(
                                            player_id = %pid,
                                            session_id = %ac_session_id,
                                            speed = speed,
                                            max_velocity = max_velocity,
                                            "Anti-cheat: velocity violation detected"
                                        );
                                        // Broadcast a server-side flag so observers know.
                                        session_lock.broadcast_state(GameMessage::Chat {
                                            player_id: "server".to_string(),
                                            message: format!(
                                                "[anticheat] velocity violation: player {} speed={:.1}",
                                                pid, speed
                                            ),
                                        });
                                    }
                                }
                                ac_positions.push(new_pos);
                            }

                            session_lock.handle_input(pid.clone(), data.clone());
                        }
                        GameMessage::Chat { player_id: pid, message } => {
                            session_lock.broadcast_state(GameMessage::Chat {
                                player_id: pid.clone(),
                                message: message.clone(),
                            });
                        }
                        GameMessage::Pong => {
                            if let Some(player) = session_lock.players.get_mut(&player_id_clone) {
                                player.last_pong = Instant::now();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // ── Disconnect: run full anomaly detection on accumulated session data ─
        let ac_inputs_for_replay = ac_inputs.clone();
        let ac_session_data = SessionData {
            session_id: ac_session_id,
            user_id: user_uuid,
            inputs: ac_inputs,
            positions: ac_positions,
            scores: ac_scores,
            start_time: ac_start,
            end_time: Some(Utc::now()),
        };

        let anomalies = anticheat_svc::detect_anomalies(&ac_session_data);
        if !anomalies.is_empty() {
            tracing::warn!(
                player_id = %player_id_clone,
                session_id = %ac_session_id,
                anomaly_count = anomalies.len(),
                "Anti-cheat: {} anomal(y/ies) detected at session end",
                anomalies.len()
            );
            for anomaly in &anomalies {
                tracing::warn!(
                    anomaly_type = ?anomaly.anomaly_type,
                    severity = ?anomaly.severity,
                    description = %anomaly.description,
                    "Anti-cheat anomaly"
                );
            }

            // ── DB writes: ban + replay for high/critical violations ───────────
            // Determine the worst severity in this session's anomaly list.
            let worst_severity = anomalies.iter().fold(Severity::Low, |worst, a| {
                if a.severity == Severity::Critical
                    || (a.severity == Severity::High && worst != Severity::Critical)
                {
                    a.severity.clone()
                } else {
                    worst
                }
            });

            let is_bannable = worst_severity == Severity::Critical || worst_severity == Severity::High;

            if is_bannable {
                // Build a summary reason from all anomaly descriptions.
                let reason = anomalies
                    .iter()
                    .map(|a| format!("[{:?}] {}", a.anomaly_type, a.description))
                    .collect::<Vec<_>>()
                    .join("; ");

                let ban_fingerprint = DeviceFingerprint {
                    user_agent: "ws-session".to_string(),
                    screen_resolution: "unknown".to_string(),
                    timezone: "unknown".to_string(),
                    language: "unknown".to_string(),
                    ip_address: "unknown".to_string(),
                    hash: format!("ws-{}", user_uuid),
                };

                // Duration: 30-day ban for critical, 7-day for high.
                let ban_days = if worst_severity == Severity::Critical {
                    Some(30)
                } else {
                    Some(7)
                };

                match anticheat_svc::ban_user(
                    &pool,
                    user_uuid,
                    &reason,
                    ban_days,
                    Some(&ban_fingerprint),
                )
                .await
                {
                    Ok(ban) => tracing::warn!(
                        user_id = %user_uuid,
                        ban_id = %ban.id,
                        ban_days = ?ban_days,
                        "Anti-cheat: user banned after session anomaly"
                    ),
                    Err(e) => tracing::error!(
                        user_id = %user_uuid,
                        error = %e,
                        "Anti-cheat: failed to write ban record"
                    ),
                }
            }

            // Always store the replay when anomalies are detected so reviewers
            // have the raw input sequence regardless of whether a ban was issued.
            match anticheat_svc::store_replay(&pool, ac_session_id, ac_inputs_for_replay).await {
                Ok(replay) => tracing::info!(
                    session_id = %ac_session_id,
                    replay_id = %replay.id,
                    "Anti-cheat: session replay stored"
                ),
                Err(e) => tracing::error!(
                    session_id = %ac_session_id,
                    error = %e,
                    "Anti-cheat: failed to store session replay"
                ),
            }
        }

        // ── Disconnect: remove player and notify the room ─────────────────────
        let mut session_lock = session.lock().await;
        session_lock.remove_player(&player_id_clone);
        session_lock.broadcast_state(GameMessage::PlayerLeave {
            player_id: player_id_clone,
        });
    })
}

// Note: use `Arc::new(GameWsHandler::new(pool))` directly; this module no longer
// exports a standalone router() because the handler requires a DB pool at construction time.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_physics() {
        let mut player = PlayerState {
            x: 100.0,
            y: 100.0,
            rotation: 0.0,
            velocity_x: 5.0,
            velocity_y: 0.0,
            last_input_seq: 0,
            interpolated: false,
        };

        update_player_physics(&mut player);

        assert!(player.x > 100.0);
        assert!(player.velocity_x < 5.0);
    }

    #[test]
    fn test_input_processing() {
        let mut player = PlayerState {
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            last_input_seq: 0,
            interpolated: false,
        };

        let input = InputData {
            keys: vec!["w".to_string()],
            mouse: None,
            timestamp: 0,
            seq: 1,
        };

        process_player_input(&mut player, &input);

        assert_eq!(player.velocity_y, -5.0);
    }

    #[test]
    fn test_game_session_registration() {
        let mut session = GameSession::new("test".to_string());
        let (tx, _) = broadcast::channel(100);

        session.register_player("player1".to_string(), tx);

        assert!(session.state.players.contains_key("player1"));
        assert!(session.is_empty() == false);
    }

    #[test]
    fn test_game_session_removal() {
        let mut session = GameSession::new("test".to_string());
        let (tx, _) = broadcast::channel(100);

        session.register_player("player1".to_string(), tx);
        session.remove_player("player1");

        assert!(!session.state.players.contains_key("player1"));
    }

    #[test]
    fn test_lerp() {
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
    }

    #[tokio::test]
    async fn test_game_manager_sessions() {
        let manager = GameManager::new();

        let session1 = manager.get_or_create_session("game1").await;
        let session2 = manager.get_or_create_session("game1").await;

        assert!(Arc::ptr_eq(&session1, &session2));

        let session3 = manager.get_or_create_session("game2").await;
        assert!(!Arc::ptr_eq(&session1, &session3));
    }
}
