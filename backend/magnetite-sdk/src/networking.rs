use crate::{GameState, Input, PlayerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    PlayerJoin(PlayerId),
    PlayerLeave(PlayerId),
    Input(Input),
    StateSync(GameState),
    StateSyncRequest,
    PlayerJoined { player_id: PlayerId, state: GameState },
    PlayerLeft { player_id: PlayerId },
    Ping,
    Pong,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncProtocol {
    pub tick: u64,
    pub state: GameState,
    pub checksum: u32,
}

impl StateSyncProtocol {
    pub fn new(tick: u64, state: GameState) -> Self {
        let checksum = Self::calculate_checksum(&state);
        Self { tick, state, checksum }
    }

    fn calculate_checksum(state: &GameState) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        serde_json::to_string(state).unwrap_or_default().hash(&mut hasher);
        hasher.finish() as u32
    }

    pub fn is_valid(&self) -> bool {
        self.checksum == Self::calculate_checksum(&self.state)
    }
}

pub struct NetworkManager {
    server_addr: String,
}

impl NetworkManager {
    pub fn new(server_addr: &str) -> Self {
        Self {
            server_addr: server_addr.to_string(),
        }
    }

    pub fn connect(&self) -> std::io::Result<Connection> {
        let stream = TcpStream::connect(&self.server_addr)?;
        Ok(Connection {
            stream,
            player_id: None,
        })
    }

    pub fn server() -> ServerNetworkManager {
        ServerNetworkManager::new()
    }
}

pub struct ServerNetworkManager {
    players: HashMap<PlayerId, std::net::TcpStream>,
}

impl ServerNetworkManager {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
        }
    }

    pub fn add_player(&mut self, player_id: PlayerId, stream: std::net::TcpStream) {
        self.players.insert(player_id, stream);
    }

    pub fn remove_player(&mut self, player_id: &PlayerId) -> Option<std::net::TcpStream> {
        self.players.remove(player_id)
    }

    pub fn broadcast(&mut self, msg: &Message) -> std::io::Result<()> {
        let data = serde_json::to_vec(msg).unwrap();
        let len = data.len() as u32;
        for stream in self.players.values_mut() {
            stream.write_all(&len.to_be_bytes())?;
            stream.write_all(&data)?;
        }
        Ok(())
    }

    pub fn send_to(&mut self, player_id: &PlayerId, msg: &Message) -> std::io::Result<()> {
        if let Some(stream) = self.players.get_mut(player_id) {
            let data = serde_json::to_vec(msg).unwrap();
            let len = data.len() as u32;
            stream.write_all(&len.to_be_bytes())?;
            stream.write_all(&data)?;
        }
        Ok(())
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }
}

impl Default for ServerNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Connection {
    stream: TcpStream,
    player_id: Option<PlayerId>,
}

impl Connection {
    pub fn send(&mut self, msg: &Message) -> std::io::Result<()> {
        let data = serde_json::to_vec(msg).unwrap();
        let len = data.len() as u32;
        self.stream.write_all(&len.to_be_bytes())?;
        self.stream.write_all(&data)?;
        Ok(())
    }

    pub fn receive(&mut self) -> std::io::Result<Message> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data)?;
        let msg: Message = serde_json::from_slice(&data).unwrap();
        Ok(msg)
    }

    pub fn set_player_id(&mut self, player_id: PlayerId) {
        self.player_id = Some(player_id);
    }

    pub fn player_id(&self) -> Option<PlayerId> {
        self.player_id
    }

    pub fn send_input(&mut self, input: Input) -> std::io::Result<()> {
        self.send(&Message::Input(input))
    }

    pub fn request_state_sync(&mut self) -> std::io::Result<()> {
        self.send(&Message::StateSyncRequest)
    }

    pub fn join_game(&mut self) -> std::io::Result<()> {
        self.send(&Message::PlayerJoin(PlayerId(0)))
    }
}
