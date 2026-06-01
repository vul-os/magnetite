//! # `magnetite-runtime`
//!
//! **Authoritative game-server host** for the Magnetite platform.
//!
//! This crate integrates:
//! - [`magnetite-sdk`] — `GameExecutor`, `AuthoritativeGame`, `MatchConfig`,
//!   `Topology`, netcode frames, SDK validators.
//! - [`magnetite-sandbox`] — `WasmExecutor` (sandboxed Wasmtime executor).
//! - [`magnetite-anticheat`] — `Anticheat`, `AnticheatConfig`, composable
//!   validators, replay verifier.
//!
//! ## Architecture
//!
//! ```text
//!  WS Clients
//!      │  ClientNet::InputFrame
//!      ▼
//! ┌────────────────────────────────────┐
//! │  ConnectionManager                 │  ← assign PlayerId, track seq
//! │  (tokio::sync channels per player) │
//! └───────────────┬────────────────────┘
//!                 │ Vec<(PlayerId, Input)>
//!                 ▼
//! ┌────────────────────────────────────────────────┐
//! │  TickScheduler (tick_hz timer)                 │
//! │  per tick:                                     │
//! │    Anticheat::inspect → drop/reject flagged    │
//! │    GameExecutor::step (native or sandboxed)    │
//! │    → ServerNet::Delta per player               │
//! │    → ServerNet::Snapshot every N               │
//! │    → ServerNet::Ack / Reject                   │
//! └────────────────────────────────────────────────┘
//!                 │
//!                 ▼
//! ┌────────────────────────────────────┐
//! │  ShardManager                      │  ← spatial routing (Sharded topology)
//! │  Sharded: cell_size grid → shard   │
//! │  HANDOFF on cell crossing          │
//! └────────────────────────────────────┘
//! ```
//!
//! ## Topologies
//!
//! | Topology | Players | Strategy |
//! |---|---|---|
//! | [`Topology::SingleRoom`] | ≲16 | All players in one room; broadcast all |
//! | [`Topology::Dedicated`] | ≲256 | Authoritative + interest-filtered snapshots |
//! | [`Topology::Sharded`] | AAA | Spatial cells; per-shard executors; HANDOFF |
//!
//! ## Quick start — native executor
//!
//! ```rust,no_run
//! use magnetite_runtime::{GameServer, GameServerConfig};
//! use magnetite_sdk::authority::{
//!     AuthoritativeGame, DeterministicRng, MatchConfig, NativeExecutor,
//!     RejectReason, StepCtx, Tick,
//! };
//! use magnetite_sdk::input::Input;
//! use magnetite_sdk::state::PlayerId;
//!
//! #[derive(serde::Serialize, serde::Deserialize, Clone)]
//! struct MySnap { tick: u64 }
//! #[derive(serde::Serialize, serde::Deserialize)]
//! struct MyDelta {}
//! #[derive(serde::Serialize)]
//! struct MyView {}
//! #[derive(serde::Serialize, serde::Deserialize)]
//! struct MyCmd;
//!
//! struct MyGame { tick: u64 }
//!
//! impl AuthoritativeGame for MyGame {
//!     type Snapshot = MySnap;
//!     type Delta    = MyDelta;
//!     type View     = MyView;
//!     type Command  = MyCmd;
//!     fn init(_cfg: &MatchConfig) -> Self { MyGame { tick: 0 } }
//!     fn validate(&self, _p: PlayerId, _i: &Input, _t: Tick)
//!         -> Result<Vec<MyCmd>, RejectReason> { Ok(vec![]) }
//!     fn step(&mut self, ctx: &mut StepCtx, _cmds: &[(PlayerId, MyCmd)]) {
//!         self.tick = ctx.tick;
//!     }
//!     fn snapshot(&self) -> MySnap { MySnap { tick: self.tick } }
//!     fn restore(s: &MySnap, _cfg: &MatchConfig) -> Self { MyGame { tick: s.tick } }
//!     fn delta(&self, _s: &MySnap) -> MyDelta { MyDelta {} }
//!     fn view_for(&self, _p: PlayerId) -> MyView { MyView {} }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let cfg = MatchConfig::auto(4);
//!     let executor = NativeExecutor::<MyGame>::new(cfg.clone());
//!     let server_cfg = GameServerConfig {
//!         bind_addr: "127.0.0.1:9000".to_string(),
//!         match_config: cfg,
//!         anticheat: None,
//!     };
//!     GameServer::serve(executor, server_cfg).await.unwrap();
//! }
//! ```
//!
//! ## Quick start — Wasm (sandboxed) executor
//!
//! ```rust,no_run
//! use magnetite_runtime::{GameServer, GameServerConfig};
//! use magnetite_sandbox::LimitsConfig;
//! use magnetite_sdk::authority::MatchConfig;
//!
//! #[tokio::main]
//! async fn main() {
//!     let cfg = MatchConfig::auto(4);
//!     let server_cfg = GameServerConfig {
//!         bind_addr: "127.0.0.1:9000".to_string(),
//!         match_config: cfg,
//!         anticheat: None,
//!     };
//!     GameServer::serve_wasm("game.wasm", LimitsConfig::default(), server_cfg)
//!         .await
//!         .unwrap();
//! }
//! ```

pub mod connection;
pub mod server;
pub mod shard;
pub mod tick;

pub use server::{GameServer, GameServerConfig, ServerError};
pub use shard::{HandoffEvent, ShardId, ShardManager};
pub use tick::ServerAnticheatConfig;

// Re-export key sandbox types so callers get `serve_wasm` without a direct
// `magnetite-sandbox` dep in their Cargo.toml.
pub use magnetite_sandbox::LimitsConfig;

// Re-export anticheat types so callers can configure the pipeline without
// needing a direct `magnetite-anticheat` dep.
pub use magnetite_anticheat::{Anticheat, AnticheatConfig, Decision};

// Re-export commonly used SDK types so downstream crates don't need to
// depend on magnetite-sdk directly for the runtime API surface.
pub use magnetite_sdk::authority::{GameExecutor, MatchConfig, Tick, Topology};
pub use magnetite_sdk::state::PlayerId;
