// Game WebSocket handler and server-authoritative game loop — platform surface for real-time
// multiplayer; not yet wired to main router (see GameWsHandler::router for the integration point).
#![allow(dead_code)]

use axum::extract::{Path, State, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::time::interval;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
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
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tick_interval: Duration::from_millis(16),
        }
    }

    pub async fn get_or_create_session(&self, game_id: &str) -> Arc<Mutex<GameSession>> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(game_id) {
            return Arc::clone(session);
        }
        let session = GameSession::new(game_id.to_string());
        let arc = Arc::new(Mutex::new(session));
        sessions.insert(game_id.to_string(), Arc::clone(&arc));
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
}

impl GameWsHandler {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(GameManager::new()),
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

impl Default for GameWsHandler {
    fn default() -> Self {
        Self::new()
    }
}

async fn handle_game_connection(
    ws: WebSocketUpgrade,
    State(handler): State<Arc<GameWsHandler>>,
    Path(game_id): Path<String>,
) -> axum::response::Response {
    let manager = handler.get_manager();

    ws.on_upgrade(move |socket| async move {
        let (mut write, mut read) = socket.split();
        let (tx, mut rx) = broadcast::channel::<GameMessage>(100);

        let _game_id_clone = game_id.clone();
        let manager_clone = Arc::clone(&manager);
        let tx_ping = tx.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(30));
            loop {
                ticker.tick().await;
                if tx_ping.send(GameMessage::Ping).is_err() {
                    break;
                }
            }
        });

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

        let session = manager_clone.get_or_create_session(&game_id).await;

        let player_id = format!(
            "player_{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        );

        {
            let mut session_lock = session.lock().await;
            session_lock.register_player(player_id.clone(), tx.clone());

            let join_msg = GameMessage::PlayerJoin {
                player_id: player_id.clone(),
            };
            let msg = GameMessage::StateUpdate {
                state: session_lock.state.clone(),
            };
            let _ = tx.send(msg);
            session_lock.broadcast_state(join_msg);
        }

        let join_msg = GameMessage::PlayerJoin {
            player_id: player_id.clone(),
        };
        let _ = tx.send(join_msg);

        let _manager_for_read = Arc::clone(&manager_clone);
        let player_id_clone = player_id.clone();

        tokio::spawn(async move {
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

            while let Some(result) = read.next().await {
                if let Ok(axum::extract::ws::Message::Text(text)) = result {
                    if let Ok(msg) = serde_json::from_str::<GameMessage>(&text) {
                        let mut session_lock = session.lock().await;
                        match &msg {
                            GameMessage::Input { player_id, data } => {
                                session_lock.handle_input(player_id.clone(), data.clone());
                            }
                            GameMessage::Chat { player_id, message } => {
                                session_lock.broadcast_state(GameMessage::Chat {
                                    player_id: player_id.clone(),
                                    message: message.clone(),
                                });
                            }
                            GameMessage::Pong => {
                                if let Some(player) = session_lock.players.get_mut(&player_id_clone)
                                {
                                    player.last_pong = Instant::now();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            let mut session_lock = session.lock().await;
            session_lock.remove_player(&player_id_clone);
            let leave_msg = GameMessage::PlayerLeave {
                player_id: player_id_clone,
            };
            session_lock.broadcast_state(leave_msg);
        });
    })
}

pub fn router() -> Router {
    let handler = Arc::new(GameWsHandler::new());
    handler.router()
}

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
