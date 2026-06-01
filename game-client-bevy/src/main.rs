//! `game-client-bevy` — Reference Bevy client for Magnetite authoritative server.
//!
//! Requires the `render` feature (default). To run:
//!
//! ```sh
//! # Start the server first:
//! cargo run -p magnetite-cli -- dev
//!
//! # Then launch the client:
//! cargo run -p game-client-bevy
//! ```
//!
//! Controls:
//!   W/A/S/D or Arrow keys — move
//!   Mouse cursor          — aim
//!   Space / Z             — shoot

use magnetite_sdk::state::PlayerId;

use game_client_bevy::app::{build_app, NetConfig};
use game_client_bevy::net::NetConfig as _NetConfig;

fn main() {
    // In a real game the player id comes from the auth/matchmaking system.
    // For the reference client we use a fixed id of 1.
    let player_id = PlayerId::new(1);

    // Connect to a local dev server by default.
    // Override with the `MAGNETITE_SERVER` env var.
    let url =
        std::env::var("MAGNETITE_SERVER").unwrap_or_else(|_| "ws://127.0.0.1:9000/ws".to_string());

    let net_config = NetConfig {
        url,
        token: "dev-token".to_string(),
        player_id,
    };

    build_app(player_id, net_config).run();
}
