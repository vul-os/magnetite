//! `single_room` — minimal example: a SingleRoom authoritative server with
//! anticheat enabled.
//!
//! Run with:
//! ```shell
//! cargo run --example single_room
//! ```
//!
//! The server binds on `127.0.0.1:9000` and accepts WebSocket connections.
//! Connect with any WS client (e.g. `websocat ws://127.0.0.1:9000`).
//! On connect you will receive a `ServerNet::Welcome` JSON frame.
//! Send `{"type":"input_frame","seq":1,"tick":0,"input":{...}}` to submit input.
//!
//! This example uses a trivial "counter" game that increments a tick counter
//! on every authoritative tick.  The anticheat pipeline uses the SDK built-ins
//! plus `AimbotSnap` and `PositionTeleport` from `magnetite-anticheat`.

use magnetite_anticheat::{
    validators::{AimbotSnap, PositionTeleport},
    Anticheat, AnticheatConfig,
};
use magnetite_runtime::{GameServer, GameServerConfig};
use magnetite_sdk::authority::{
    AuthoritativeGame, InputSchema, MatchConfig, NativeExecutor, RateLimit, RejectReason, StepCtx,
    Tick, Topology, ValidatorChain,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;
use tracing::info;
use tracing_subscriber::EnvFilter;

// ---------------------------------------------------------------------------
// Counter game — demo game implementing AuthoritativeGame
// ---------------------------------------------------------------------------

struct CounterGame {
    /// Monotonically increasing tick counter.
    tick: u64,
    /// Number of commands processed (all-time).
    commands: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CounterSnap {
    tick: u64,
    commands: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CounterDelta {
    tick_delta: u64,
    cmd_delta: u64,
}

#[derive(serde::Serialize)]
struct CounterView {
    tick: u64,
    commands: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum CounterCmd {
    Nop,
}

impl AuthoritativeGame for CounterGame {
    type Snapshot = CounterSnap;
    type Delta = CounterDelta;
    type View = CounterView;
    type Command = CounterCmd;

    fn init(_cfg: &MatchConfig) -> Self {
        CounterGame {
            tick: 0,
            commands: 0,
        }
    }

    fn validate(
        &self,
        _player: PlayerId,
        _input: &Input,
        _tick: Tick,
    ) -> Result<Vec<CounterCmd>, RejectReason> {
        Ok(vec![CounterCmd::Nop])
    }

    fn step(&mut self, ctx: &mut StepCtx, cmds: &[(PlayerId, CounterCmd)]) {
        self.tick = ctx.tick;
        self.commands += cmds.len() as u64;
    }

    fn snapshot(&self) -> CounterSnap {
        CounterSnap {
            tick: self.tick,
            commands: self.commands,
        }
    }

    fn restore(s: &CounterSnap, _cfg: &MatchConfig) -> Self {
        CounterGame {
            tick: s.tick,
            commands: s.commands,
        }
    }

    fn delta(&self, since: &CounterSnap) -> CounterDelta {
        CounterDelta {
            tick_delta: self.tick.saturating_sub(since.tick),
            cmd_delta: self.commands.saturating_sub(since.commands),
        }
    }

    fn view_for(&self, _player: PlayerId) -> CounterView {
        CounterView {
            tick: self.tick,
            commands: self.commands,
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), magnetite_runtime::ServerError> {
    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Build a SingleRoom config — up to 16 players, 60 Hz, snapshot every 300 ticks.
    let match_config = MatchConfig {
        topology: Topology::SingleRoom,
        max_players: 16,
        tick_hz: 60,
        seed: 0xDEAD_BEEF_CAFE_1234,
        snapshot_every: 300,
    };

    info!(
        tick_hz = match_config.tick_hz,
        max_players = match_config.max_players,
        snapshot_every = match_config.snapshot_every,
        "starting CounterGame in SingleRoom topology"
    );

    // Build a composable anticheat pipeline for this game.
    //
    // SDK built-ins:
    //   RateLimit(120)   — drop inputs faster than 120/s
    //   InputSchema      — reject inputs with out-of-range fields
    //
    // magnetite-anticheat validators:
    //   AimbotSnap(45°)           — detect instant aimbot snaps
    //   PositionTeleport(20 u/t)  — detect speed-hack / teleport
    let chain = ValidatorChain::new()
        .add(RateLimit::new(120))
        .add(InputSchema::default())
        .add(AimbotSnap::new(45.0))
        .add(PositionTeleport::new(20.0));

    let anticheat = Anticheat::new(
        chain,
        AnticheatConfig {
            warn_threshold: 3,
            kick_threshold: 8,
            ban_threshold: 15,
            decay_interval_ticks: 600, // ~10s at 60 Hz
            decay_amount: 1,
        },
    );

    let executor = NativeExecutor::<CounterGame>::new(match_config.clone());
    let server_cfg = GameServerConfig {
        bind_addr: "127.0.0.1:9000".to_string(),
        match_config,
        anticheat: Some(anticheat),
    };

    GameServer::serve(executor, server_cfg).await
}
