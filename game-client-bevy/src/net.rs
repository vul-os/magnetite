//! Async WebSocket transport for the Magnetite client.
//!
//! Spawns a `tokio` task that:
//! 1. Connects to the server at `ws://host:port/ws`.
//! 2. Sends the initial `ClientMessage::Connect` handshake.
//! 3. Forwards outgoing [`ClientNet`] frames from the Bevy `→ server` channel.
//! 4. Receives [`ServerNet`] frames and pushes them into the `← client` channel.
//!
//! The Bevy main thread reads from the receive channel each frame in
//! `NetPlugin::process_server_messages`.
//!
//! # WASM
//!
//! On `wasm32` the `render` and `wasm` features should be enabled. The WS
//! backend switches from `tokio-tungstenite` to `ewebsock`. The API of
//! [`NetChannels`] stays identical; only the internal task differs.

use serde_json;
use tokio::sync::mpsc;

use magnetite_sdk::protocol::{ClientNet, ServerNet};

// ─────────────────────────────────────────────────────────────────────────────
// Channel types
// ─────────────────────────────────────────────────────────────────────────────

/// Capacity of each direction's channel.
const CHANNEL_CAPACITY: usize = 256;

/// All channels needed to communicate between the Bevy main thread and the
/// background network task.
#[derive(Debug)]
pub struct NetChannels {
    /// Bevy → WS task: outgoing `ClientNet` frames.
    pub tx_to_server: mpsc::Sender<ClientNet>,
    /// WS task → Bevy: incoming `ServerNet` frames.
    pub rx_from_server: mpsc::Receiver<ServerNet>,
}

/// The other half of [`NetChannels`], held by the background task.
pub struct NetTaskChannels {
    pub rx_from_bevy: mpsc::Receiver<ClientNet>,
    pub tx_to_bevy: mpsc::Sender<ServerNet>,
}

/// Create a matched pair of channels.
pub fn make_channels() -> (NetChannels, NetTaskChannels) {
    let (tx_to_server, rx_from_bevy) = mpsc::channel(CHANNEL_CAPACITY);
    let (tx_to_bevy, rx_from_server) = mpsc::channel(CHANNEL_CAPACITY);
    (
        NetChannels {
            tx_to_server,
            rx_from_server,
        },
        NetTaskChannels {
            rx_from_bevy,
            tx_to_bevy,
        },
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Connection config
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the WebSocket connection.
#[derive(Debug, Clone)]
pub struct NetConfig {
    /// Full WebSocket URL, e.g. `ws://127.0.0.1:9000/ws`.
    pub url: String,
    /// Auth token sent in the Connect handshake.
    pub token: String,
    /// Player id to announce in the Connect handshake.
    pub player_id: magnetite_sdk::state::PlayerId,
}

impl NetConfig {
    /// Convenience constructor for a local dev server.
    pub fn local(player_id: magnetite_sdk::state::PlayerId) -> Self {
        Self {
            url: "ws://127.0.0.1:9000/ws".to_string(),
            token: "dev-token".to_string(),
            player_id,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Native (tokio-tungstenite) network task
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "wasm"))]
pub use native::spawn_net_task;

#[cfg(not(feature = "wasm"))]
mod native {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use magnetite_sdk::protocol::{ClientMessage, Envelope};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    /// Spawn the background network task.
    ///
    /// Returns the [`NetChannels`] the Bevy app uses to communicate.
    /// The task runs until the WebSocket closes or an error occurs.
    pub fn spawn_net_task(config: NetConfig) -> NetChannels {
        let (channels, task_ch) = make_channels();
        tokio::spawn(run_net_task(config, task_ch));
        channels
    }

    async fn run_net_task(config: NetConfig, mut task_ch: NetTaskChannels) {
        let url = config.url.clone();
        let ws_stream = match connect_async(&url).await {
            Ok((stream, _)) => stream,
            Err(e) => {
                tracing_log(&format!("WS connect error: {e}"));
                return;
            }
        };

        let (mut ws_write, mut ws_read) = ws_stream.split();

        // Send the Connect handshake.
        let connect = Envelope::new(ClientMessage::Connect {
            player_id: config.player_id,
            token: config.token.clone(),
        });
        if let Ok(bytes) = connect.encode() {
            let _ = ws_write.send(Message::Binary(bytes.into())).await;
        }

        loop {
            tokio::select! {
                // Outgoing: Bevy → server.
                Some(msg) = task_ch.rx_from_bevy.recv() => {
                    match serde_json::to_vec(&msg) {
                        Ok(bytes) => {
                            let _ = ws_write.send(Message::Binary(bytes.into())).await;
                        }
                        Err(e) => tracing_log(&format!("serialize error: {e}")),
                    }
                }

                // Incoming: server → Bevy.
                Some(ws_msg) = ws_read.next() => {
                    match ws_msg {
                        Ok(Message::Binary(bytes)) => {
                            match serde_json::from_slice::<ServerNet>(&bytes) {
                                Ok(server_msg) => {
                                    let _ = task_ch.tx_to_bevy.send(server_msg).await;
                                }
                                Err(e) => tracing_log(&format!("deserialize error: {e}")),
                            }
                        }
                        Ok(Message::Text(text)) => {
                            match serde_json::from_str::<ServerNet>(&text) {
                                Ok(server_msg) => {
                                    let _ = task_ch.tx_to_bevy.send(server_msg).await;
                                }
                                Err(e) => tracing_log(&format!("deserialize error (text): {e}")),
                            }
                        }
                        Ok(Message::Close(_)) => {
                            tracing_log("WS connection closed by server");
                            break;
                        }
                        Err(e) => {
                            tracing_log(&format!("WS error: {e}"));
                            break;
                        }
                        _ => {}
                    }
                }

                else => break,
            }
        }
    }

    fn tracing_log(msg: &str) {
        // Use eprintln in tests / when tracing is not configured.
        eprintln!("[net] {msg}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WASM (ewebsock) network task
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "wasm")]
pub use wasm_net::spawn_net_task;

#[cfg(feature = "wasm")]
mod wasm_net {
    use super::*;
    use ewebsock::{WsEvent, WsMessage, WsSender};
    use magnetite_sdk::protocol::{ClientMessage, Envelope};

    pub fn spawn_net_task(config: NetConfig) -> NetChannels {
        let (channels, mut task_ch) = make_channels();

        let (mut ws_sender, ws_receiver) =
            ewebsock::connect(config.url.clone()).expect("ewebsock connect");

        // Send the Connect handshake.
        let connect = Envelope::new(ClientMessage::Connect {
            player_id: config.player_id,
            token: config.token.clone(),
        });
        if let Ok(bytes) = connect.encode() {
            ws_sender.send(WsMessage::Binary(bytes));
        }

        // WASM: no real background thread — poll in a wasm_bindgen::closure / requestAnimationFrame.
        // For this reference client we spawn a tokio task on the wasm executor.
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                // Drain outgoing messages.
                while let Ok(msg) = task_ch.rx_from_bevy.try_recv() {
                    if let Ok(bytes) = serde_json::to_vec(&msg) {
                        ws_sender.send(WsMessage::Binary(bytes));
                    }
                }

                // Drain incoming messages.
                while let Some(event) = ws_receiver.try_recv() {
                    match event {
                        WsEvent::Message(WsMessage::Binary(bytes)) => {
                            if let Ok(msg) = serde_json::from_slice::<ServerNet>(&bytes) {
                                let _ = task_ch.tx_to_bevy.send(msg).await;
                            }
                        }
                        WsEvent::Message(WsMessage::Text(text)) => {
                            if let Ok(msg) = serde_json::from_str::<ServerNet>(&text) {
                                let _ = task_ch.tx_to_bevy.send(msg).await;
                            }
                        }
                        WsEvent::Closed => break,
                        WsEvent::Error(e) => {
                            web_sys::console::error_1(&format!("[net] WS error: {e}").into());
                            break;
                        }
                        _ => {}
                    }
                }

                // Yield to the WASM executor.
                gloo_timers::future::TimeoutFuture::new(0).await;
            }
        });

        channels
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_channels_creates_working_pair() {
        let (channels, mut task_ch) = make_channels();

        // Send a ClientNet message from Bevy side.
        channels
            .tx_to_server
            .try_send(ClientNet::InputFrame {
                seq: 0,
                tick: 1,
                input: magnetite_sdk::input::Input::default(),
            })
            .expect("send must succeed on unbounded channel");

        // Receive on task side.
        let msg = task_ch
            .rx_from_bevy
            .try_recv()
            .expect("message must be available");
        assert!(matches!(msg, ClientNet::InputFrame { seq: 0, .. }));
    }

    #[test]
    fn net_config_local_defaults() {
        let pid = magnetite_sdk::state::PlayerId::new(42);
        let cfg = NetConfig::local(pid);
        assert!(cfg.url.starts_with("ws://"));
        assert_eq!(cfg.player_id, pid);
    }

    #[test]
    fn client_net_serialises_correctly() {
        let frame = ClientNet::InputFrame {
            seq: 7,
            tick: 100,
            input: magnetite_sdk::input::Input::default(),
        };
        let bytes = serde_json::to_vec(&frame).expect("serialize");
        let round: ClientNet = serde_json::from_slice(&bytes).expect("deserialize");
        assert!(matches!(
            round,
            ClientNet::InputFrame {
                seq: 7,
                tick: 100,
                ..
            }
        ));
    }

    #[test]
    fn server_net_ack_serialises_correctly() {
        let ack = ServerNet::Ack { seq: 3, tick: 10 };
        let bytes = serde_json::to_vec(&ack).expect("serialize");
        let round: ServerNet = serde_json::from_slice(&bytes).expect("deserialize");
        assert!(matches!(round, ServerNet::Ack { seq: 3, tick: 10 }));
    }
}
