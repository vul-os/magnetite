//! `magnetite-runtime serve` — authoritative game-server binary.
//!
//! Loads a compiled game module (`.wasm`) and runs an authoritative
//! [`GameServer`] with the Wasmtime sandbox executor over WebSocket.
//!
//! # Usage
//!
//! ```text
//! # Wasm executor (sandboxed — requires a compiled game.wasm):
//! magnetite-serve --wasm path/to/game.wasm --host 0.0.0.0 --port 9000
//!
//! # Native nop-game (for smoke-testing connectivity without a .wasm):
//! magnetite-serve --host 127.0.0.1 --port 9000
//! ```
//!
//! # Environment
//!
//! | Variable          | Default          | Description                            |
//! |---|---|---|
//! | `RUST_LOG`        | `info`           | `tracing` filter (e.g. `debug`)        |
//! | `RUNTIME_HOST`    | `127.0.0.1`      | Bind address (overridden by `--host`)  |
//! | `RUNTIME_PORT`    | `9000`           | Bind port    (overridden by `--port`)  |
//! | `RUNTIME_WORKERS` | `0` (auto)       | Tokio worker threads; 0 = Tokio default|
//!
//! # Flags
//!
//! | Flag             | Description                                         |
//! |---|---|
//! | `--wasm <path>`  | Path to a compiled `wasm32-wasip1` game module.    |
//!                     When omitted, the server uses a built-in NopGame.   |
//! | `--host <addr>`  | Bind address (default `127.0.0.1`).                 |
//! | `--port <port>`  | WebSocket port (default `9000`).                    |
//! | `--workers <n>`  | Tokio worker threads; 0 = auto (default `0`).       |
//! | `--tick-hz <n>`  | Server tick rate (default `60`).                    |
//! | `--max-players`  | Player cap (default `16`; >16 → Dedicated).         |
//! | `--seed <u64>`   | Deterministic RNG seed (default `0xDEADBEEFCAFE1234`). |
//! | `--snapshot-every <n>` | Full snapshot cadence in ticks (default `300`). |

use std::env;
use std::process;

use magnetite_runtime::{GameServer, GameServerConfig, LimitsConfig};
use magnetite_sdk::authority::{
    AuthoritativeGame, MatchConfig, NativeExecutor, RejectReason, StepCtx, Tick, Topology,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;
use tracing::info;
use tracing_subscriber::EnvFilter;

// ---------------------------------------------------------------------------
// NopGame — built-in placeholder when no --wasm is provided
// ---------------------------------------------------------------------------

/// A trivial no-op game used for smoke-testing the server without a real
/// `.wasm` file.  It accepts any input and tracks only the tick counter.
struct NopGame {
    tick: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct NopSnap {
    tick: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NopDelta {
    tick: u64,
}

#[derive(serde::Serialize)]
struct NopView {
    tick: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NopCmd;

impl AuthoritativeGame for NopGame {
    type Snapshot = NopSnap;
    type Delta = NopDelta;
    type View = NopView;
    type Command = NopCmd;

    fn init(_cfg: &MatchConfig) -> Self {
        NopGame { tick: 0 }
    }

    fn validate(
        &self,
        _player: PlayerId,
        _input: &Input,
        _tick: Tick,
    ) -> Result<Vec<NopCmd>, RejectReason> {
        Ok(vec![NopCmd])
    }

    fn step(&mut self, ctx: &mut StepCtx, _cmds: &[(PlayerId, NopCmd)]) {
        self.tick = ctx.tick;
    }

    fn snapshot(&self) -> NopSnap {
        NopSnap { tick: self.tick }
    }

    fn restore(s: &NopSnap, _cfg: &MatchConfig) -> Self {
        NopGame { tick: s.tick }
    }

    fn delta(&self, since: &NopSnap) -> NopDelta {
        NopDelta {
            tick: self.tick.saturating_sub(since.tick),
        }
    }

    fn view_for(&self, _player: PlayerId) -> NopView {
        NopView { tick: self.tick }
    }
}

// ---------------------------------------------------------------------------
// Arg parsing — lightweight; no external dep required
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Args {
    /// Path to a compiled `.wasm` game module.  `None` → use NopGame.
    wasm: Option<String>,
    host: String,
    port: u16,
    workers: usize,
    tick_hz: u16,
    max_players: u32,
    seed: u64,
    snapshot_every: u16,
}

impl Args {
    fn parse() -> Self {
        let raw: Vec<String> = env::args().collect();

        // Defaults — env-overridable, then flag-overridable.
        let mut wasm: Option<String> = None;
        let mut host = env::var("RUNTIME_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let mut port: u16 = env::var("RUNTIME_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(9000);
        let mut workers: usize = env::var("RUNTIME_WORKERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut tick_hz: u16 = 60;
        let mut max_players: u32 = 16;
        let mut seed: u64 = 0xDEAD_BEEF_CAFE_1234;
        let mut snapshot_every: u16 = 300;

        let mut i = 1;
        while i < raw.len() {
            match raw[i].as_str() {
                "--wasm" => {
                    i += 1;
                    wasm = Some(next_arg(&raw, i, "--wasm"));
                }
                "--host" => {
                    i += 1;
                    host = next_arg(&raw, i, "--host");
                }
                "--port" => {
                    i += 1;
                    port = next_arg(&raw, i, "--port")
                        .parse()
                        .unwrap_or_else(|_| die("--port must be a valid u16"));
                }
                "--workers" => {
                    i += 1;
                    workers = next_arg(&raw, i, "--workers")
                        .parse()
                        .unwrap_or_else(|_| die("--workers must be a valid usize"));
                }
                "--tick-hz" => {
                    i += 1;
                    tick_hz = next_arg(&raw, i, "--tick-hz")
                        .parse()
                        .unwrap_or_else(|_| die("--tick-hz must be a valid u16"));
                }
                "--max-players" => {
                    i += 1;
                    max_players = next_arg(&raw, i, "--max-players")
                        .parse()
                        .unwrap_or_else(|_| die("--max-players must be a valid u32"));
                }
                "--seed" => {
                    i += 1;
                    seed = next_arg(&raw, i, "--seed")
                        .parse()
                        .unwrap_or_else(|_| die("--seed must be a valid u64"));
                }
                "--snapshot-every" => {
                    i += 1;
                    snapshot_every = next_arg(&raw, i, "--snapshot-every")
                        .parse()
                        .unwrap_or_else(|_| die("--snapshot-every must be a valid u16"));
                }
                "--help" | "-h" => {
                    print_help();
                    process::exit(0);
                }
                other => {
                    eprintln!("Unknown flag: {other}");
                    print_help();
                    process::exit(1);
                }
            }
            i += 1;
        }

        Args {
            wasm,
            host,
            port,
            workers,
            tick_hz,
            max_players,
            seed,
            snapshot_every,
        }
    }
}

fn next_arg(raw: &[String], i: usize, flag: &str) -> String {
    raw.get(i)
        .cloned()
        .unwrap_or_else(|| die(&format!("{flag} requires a value")))
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    process::exit(1);
}

fn print_help() {
    println!(
        r#"magnetite-serve — authoritative game-server host

USAGE:
    magnetite-serve [FLAGS]

FLAGS:
    --wasm <path>          Path to a wasm32-wasip1 game module
                           (omit to use the built-in NopGame)
    --host <addr>          Bind address [default: 127.0.0.1]
                           (or set RUNTIME_HOST env var)
    --port <port>          WebSocket port [default: 9000]
                           (or set RUNTIME_PORT env var)
    --workers <n>          Tokio worker threads; 0 = auto [default: 0]
                           (or set RUNTIME_WORKERS env var)
    --tick-hz <n>          Server tick rate [default: 60]
    --max-players <n>      Player cap [default: 16]
    --seed <u64>           Deterministic RNG seed [default: 0xDEADBEEFCAFE1234]
    --snapshot-every <n>   Full snapshot cadence in ticks [default: 300]
    -h, --help             Print this message and exit

EXAMPLES:
    # Wasm executor (production):
    magnetite-serve --wasm ./game.wasm --host 0.0.0.0 --port 9000

    # Native NopGame (smoke-test / CI):
    magnetite-serve --host 127.0.0.1 --port 9000

    # Docker (wasm path passed via bind-mount):
    docker run -p 9000:9000 -v $(pwd)/game.wasm:/game.wasm \
        magnetite-runtime -- --wasm /game.wasm --host 0.0.0.0

ENV:
    RUST_LOG        tracing filter [default: info]
    RUNTIME_HOST    default bind address
    RUNTIME_PORT    default port
    RUNTIME_WORKERS default worker threads
"#
    );
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    // Honour RUNTIME_WORKERS for the Tokio runtime.
    let mut builder = if args.workers > 0 {
        let mut b = tokio::runtime::Builder::new_multi_thread();
        b.worker_threads(args.workers);
        b
    } else {
        tokio::runtime::Builder::new_multi_thread()
    };

    let rt = builder
        .enable_all()
        .build()
        .expect("failed to build Tokio runtime");

    rt.block_on(async_main(args));
}

async fn async_main(args: Args) {
    // Structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Build MatchConfig.
    let topology = if args.max_players <= 16 {
        Topology::SingleRoom
    } else {
        Topology::Dedicated {
            tick_hz: args.tick_hz,
        }
    };

    let match_config = MatchConfig {
        topology,
        max_players: args.max_players,
        tick_hz: args.tick_hz,
        seed: args.seed,
        snapshot_every: args.snapshot_every,
    };

    let bind_addr = format!("{}:{}", args.host, args.port);

    info!(
        bind_addr = %bind_addr,
        topology = ?match_config.topology,
        tick_hz = match_config.tick_hz,
        max_players = match_config.max_players,
        wasm = ?args.wasm,
        "magnetite-serve starting",
    );

    let server_cfg = GameServerConfig {
        bind_addr,
        match_config: match_config.clone(),
        anticheat: None, // use the runtime default: RateLimit(120) + InputSchema
        // Single-node serve: no cluster membership, so no session follow.
        fleet: None,
    };

    let result = match &args.wasm {
        Some(wasm_path) => {
            info!(path = %wasm_path, "loading Wasm game module (sandboxed executor)");
            let limits = LimitsConfig::default();
            GameServer::serve_wasm(wasm_path, limits, server_cfg).await
        }
        None => {
            info!("no --wasm supplied; using built-in NopGame (native executor)");
            let executor = NativeExecutor::<NopGame>::new(match_config);
            GameServer::serve(executor, server_cfg).await
        }
    };

    if let Err(e) = result {
        eprintln!("fatal server error: {e}");
        process::exit(1);
    }
}
