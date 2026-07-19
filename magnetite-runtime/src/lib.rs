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

pub mod capacity;
pub mod connection;
pub mod fleet;
pub mod node;
pub mod server;
pub mod shard;
pub mod tick;
pub mod tracker;

pub use capacity::{measure_capacity, with_occupancy};
pub use node::{
    announce, build_session_ad, content_address, load_verified_game, prepare_game, run_node,
    NodeConfig, NodeError, PreparedGame,
};
pub use server::{GameServer, GameServerConfig, ServerError};
pub use fleet::{
    FleetNode, NetworkHandoffTransport, OwnedShard, PeerRoute, ShardAuthority, DEFAULT_TIMEOUT,
};
pub use shard::{
    ExecutorFactory, HandoffError, HandoffEvent, HandoffTransport, LoopbackTransport, ShardId,
    ShardManager, ShardedRuntime, ShardedStepOutput,
};
pub use tick::ServerAnticheatConfig;

// Re-export the seam types a node-hosting caller (e.g. `magnetite-cli`) needs,
// so downstream binaries don't need a direct `magnetite-seams` dependency to
// stand up a capacity-elastic, self-advertising, content-addressed node.
pub use magnetite_seams::blobstore::{BlobStore, Hash, LocalBlobStore};
pub use magnetite_seams::discovery::{
    Capacity, Discovery, FanoutDiscovery, Filter, LanDiscovery, NodeAddr, Price, SessionAd,
};

// Re-export the SDK scaling primitives so callers can drive the scheduler and
// emergent-capacity helpers through the runtime facade.
pub use magnetite_sdk::scaling::{
    player_capacity, shards_for_capacity, LocalScheduler, NodeCapacity, NodeId, Placement,
    Shardable, ShardKey, ShardScheduler, SpreadScheduler,
};

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
