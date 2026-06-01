//! Authoritative tick scheduler.
//!
//! [`TickScheduler`] drives the server loop at exactly `tick_hz` ticks per
//! second.  Each tick it:
//!
//! 1. Drains all buffered client inputs from the [`ConnectionManager`].
//! 2. **Runs each input through the [`Anticheat`] pipeline** — flagged inputs are
//!    dropped and `ServerNet::Reject` is sent immediately; escalated players
//!    (Kick/Ban) are also rejected.
//! 3. Calls [`GameExecutor::step`] with the ordered, sanitised input list.
//! 4. For each player:
//!    - Sends [`ServerNet::Ack`] / [`ServerNet::Reject`] based on step output.
//!    - Sends an interest-filtered [`ServerNet::Delta`] (every tick).
//!    - Sends a full [`ServerNet::Snapshot`] every `snapshot_every` ticks.
//!
//! The scheduler also records a [`ReplayLog`] that can be handed to the
//! anti-cheat verifier.
//!
//! ## Anti-cheat flow
//!
//! ```text
//!  drain_inputs()
//!       │  Vec<(PlayerId, seq, Input)>
//!       ▼
//!  anticheat.inspect(player, input, tick)
//!       ├─ Allow  → keep, send to executor
//!       ├─ Reject → drop, send ServerNet::Reject
//!       ├─ Kick   → drop, send ServerNet::Reject (caller may disconnect)
//!       └─ Ban    → drop, send ServerNet::Reject (caller may disconnect)
//!       ▼
//!  executor.step(tick, sanitised_inputs)
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{self, Instant};
use tracing::{debug, info, warn};

use magnetite_anticheat::{Anticheat, AnticheatConfig, Decision};
use magnetite_sdk::authority::{
    GameExecutor, MatchConfig, RejectReason, ReplayLog, Tick, ValidatorChain,
};
use magnetite_sdk::input::Input;
use magnetite_sdk::protocol::ServerNet;
use magnetite_sdk::state::PlayerId;

use crate::connection::ConnectionManager;

// ---------------------------------------------------------------------------
// AnticheatConfig re-export
// ---------------------------------------------------------------------------

/// Anticheat configuration exposed on the server.  Alias of
/// [`magnetite_anticheat::AnticheatConfig`] so callers don't need a direct dep
/// on `magnetite-anticheat`.
pub use magnetite_anticheat::AnticheatConfig as ServerAnticheatConfig;

// ---------------------------------------------------------------------------
// TickScheduler
// ---------------------------------------------------------------------------

/// Drives the authoritative tick loop with integrated anticheat.
///
/// Create via [`TickScheduler::new`] or [`TickScheduler::with_anticheat`] and
/// call [`TickScheduler::run`] inside a [`tokio::spawn`]'d task.
pub struct TickScheduler {
    /// The game executor, boxed for dynamic dispatch (native or wasm).
    executor: Arc<Mutex<Box<dyn GameExecutor>>>,
    connection_mgr: ConnectionManager,
    config: MatchConfig,
    replay_log: Arc<Mutex<ReplayLog>>,
    anticheat: Arc<Mutex<Anticheat>>,
}

impl TickScheduler {
    /// Create a new scheduler with a **default** anticheat chain (sdk
    /// built-ins: `RateLimit(120)` + `InputSchema`).
    ///
    /// - `executor` — any [`GameExecutor`] impl (native or wasm).
    /// - `connection_mgr` — shared connection registry.
    /// - `config` — match configuration (tick rate, snapshot cadence, …).
    pub fn new(
        executor: impl GameExecutor + 'static,
        connection_mgr: ConnectionManager,
        config: MatchConfig,
    ) -> Self {
        let chain = ValidatorChain::new()
            .add(magnetite_sdk::authority::RateLimit::new(120))
            .add(magnetite_sdk::authority::InputSchema::default());
        let anticheat = Anticheat::new(chain, AnticheatConfig::default());
        Self::with_anticheat(Box::new(executor), connection_mgr, config, anticheat)
    }

    /// Create a new scheduler with a **custom** [`Anticheat`] pipeline.
    ///
    /// Use this when you need to compose additional [`Validator`]s (e.g.
    /// `magnetite-anticheat`'s `AimbotSnap`, `PositionTeleport`, …) or tune
    /// thresholds.
    ///
    /// The executor is accepted as a `Box<dyn GameExecutor>` so callers can
    /// pass either a `NativeExecutor`, a `WasmExecutor`, or any custom impl.
    pub fn with_anticheat(
        executor: Box<dyn GameExecutor>,
        connection_mgr: ConnectionManager,
        config: MatchConfig,
        anticheat: Anticheat,
    ) -> Self {
        let replay_log = ReplayLog::new(config.clone());
        Self {
            executor: Arc::new(Mutex::new(executor)),
            connection_mgr,
            replay_log: Arc::new(Mutex::new(replay_log)),
            anticheat: Arc::new(Mutex::new(anticheat)),
            config,
        }
    }

    /// Return a shared reference to the replay log.
    ///
    /// The anti-cheat module can read this after the match ends.
    pub fn replay_log(&self) -> Arc<Mutex<ReplayLog>> {
        Arc::clone(&self.replay_log)
    }

    /// Run the tick loop until `shutdown` is triggered.
    ///
    /// This is a blocking `async` call — run it inside `tokio::spawn`.
    pub async fn run(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let tick_duration =
            Duration::from_micros(1_000_000 / u64::from(self.config.tick_hz.max(1)));

        let mut tick: Tick = 0;
        let mut interval = time::interval(tick_duration);
        // Don't try to catch up if we fall behind — skip missed ticks.
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        info!(
            tick_hz = self.config.tick_hz,
            snapshot_every = self.config.snapshot_every,
            "tick scheduler starting"
        );

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    tick += 1;
                    self.run_tick(tick).await;
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!(tick, "tick scheduler shutdown");
                        break;
                    }
                }
            }
        }
    }

    /// Execute one authoritative tick.
    async fn run_tick(&self, tick: Tick) {
        let start = Instant::now();

        // 1. Drain buffered inputs.
        let raw_inputs = self.connection_mgr.drain_inputs().await;

        // 2. Run anticheat on each input.
        //    Collect (player, seq) pairs that were anticheat-rejected so we
        //    can immediately send Reject frames and skip those inputs.
        let mut ac_rejected: HashMap<PlayerId, (u32, RejectReason)> = HashMap::new();
        let mut sanitised_inputs: Vec<(PlayerId, Input)> = Vec::with_capacity(raw_inputs.len());

        {
            let mut ac = self.anticheat.lock().await;
            for (player, seq, input) in &raw_inputs {
                match ac.inspect(*player, input, tick) {
                    Decision::Allow => {
                        sanitised_inputs.push((*player, *input));
                    }
                    Decision::Reject(reason) => {
                        warn!(%player, %tick, ?reason, "anticheat: input rejected");
                        ac_rejected.insert(*player, (*seq, reason));
                    }
                    Decision::Kick(pid) => {
                        warn!(%pid, %tick, "anticheat: player kicked");
                        ac_rejected.insert(
                            pid,
                            (*seq, RejectReason::IllegalAction("kicked".to_string())),
                        );
                    }
                    Decision::Ban(pid) => {
                        warn!(%pid, %tick, "anticheat: player banned");
                        ac_rejected.insert(
                            pid,
                            (*seq, RejectReason::IllegalAction("banned".to_string())),
                        );
                    }
                }
            }
        }

        // Send immediate Reject for anticheat-flagged inputs.
        for (player, (seq, reason)) in &ac_rejected {
            self.connection_mgr
                .send_to(
                    *player,
                    ServerNet::Reject {
                        seq: *seq,
                        reason: reason.clone(),
                    },
                )
                .await;
        }

        // 3. Step the executor with sanitised inputs.
        let step_out = {
            let mut exec = self.executor.lock().await;
            exec.step(tick, &sanitised_inputs)
        };

        // 4. Record in replay log (record sanitised inputs only, matching
        //    what the executor actually processed).
        {
            let mut log = self.replay_log.lock().await;
            log.record(tick, sanitised_inputs.clone(), step_out.state_hash);
        }

        // Build a set of executor-rejected players for O(1) lookup.
        let exec_reject_set: HashMap<PlayerId, RejectReason> = step_out
            .rejects
            .iter()
            .map(|(pid, reason)| (*pid, reason.clone()))
            .collect();

        // 5. Fan-out per player.
        let player_ids = self.connection_mgr.player_ids().await;

        // Capture current snapshot bytes once (used for deltas and periodic
        // full snapshots).
        let current_snapshot_bytes: Vec<u8> = {
            let exec = self.executor.lock().await;
            exec.snapshot()
        };

        for player_id in &player_ids {
            let player_id = *player_id;

            // Skip players that were already sent an anticheat Reject above.
            if ac_rejected.contains_key(&player_id) {
                // Still send a delta/snapshot so the client stays in sync.
                // (fall through to 5c/5d below — but no extra Ack/Reject needed)
            } else {
                // 5a. Find the seq for this player (if they sent an input that
                //     passed anticheat).
                let seq_opt = sanitised_inputs
                    .iter()
                    .find(|(id, _)| *id == player_id)
                    .and_then(|(id, _)| {
                        raw_inputs
                            .iter()
                            .find(|(rid, _, _)| *rid == *id)
                            .map(|(_, seq, _)| *seq)
                    });

                // 5b. Ack or Reject based on executor output.
                if let Some(seq) = seq_opt {
                    if let Some(reason) = exec_reject_set.get(&player_id) {
                        self.connection_mgr
                            .send_to(
                                player_id,
                                ServerNet::Reject {
                                    seq,
                                    reason: reason.clone(),
                                },
                            )
                            .await;
                    } else {
                        self.connection_mgr
                            .send_to(player_id, ServerNet::Ack { seq, tick })
                            .await;
                    }
                }
            }

            // 5c. Full snapshot on cadence.
            let send_snapshot = tick % u64::from(self.config.snapshot_every) == 0;
            if send_snapshot {
                self.connection_mgr
                    .send_to(
                        player_id,
                        ServerNet::Snapshot {
                            tick,
                            full: current_snapshot_bytes.clone(),
                        },
                    )
                    .await;
                self.connection_mgr
                    .set_last_snapshot_tick(player_id, tick)
                    .await;
            } else {
                // 5d. Interest-filtered delta every tick.
                let last_snap_tick = self.connection_mgr.last_snapshot_tick(player_id).await;

                if last_snap_tick == 0 {
                    // Bootstrap: send full snapshot.
                    self.connection_mgr
                        .send_to(
                            player_id,
                            ServerNet::Snapshot {
                                tick,
                                full: current_snapshot_bytes.clone(),
                            },
                        )
                        .await;
                    self.connection_mgr
                        .set_last_snapshot_tick(player_id, tick)
                        .await;
                } else {
                    // Interest-filtered delta.
                    let diff: Vec<u8> = {
                        let exec = self.executor.lock().await;
                        exec.delta_since(&current_snapshot_bytes)
                    };

                    self.connection_mgr
                        .send_to(
                            player_id,
                            ServerNet::Delta {
                                tick,
                                since_tick: last_snap_tick,
                                diff,
                            },
                        )
                        .await;
                }
            }
        }

        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(u64::from(1000 / self.config.tick_hz.max(1))) {
            warn!(tick, ?elapsed, "tick took longer than tick duration");
        } else {
            debug!(tick, ?elapsed, "tick complete");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_anticheat::{
        validators::{AimbotSnap, FireRateCooldown},
        Anticheat, AnticheatConfig,
    };
    use magnetite_sdk::authority::{
        AuthoritativeGame, MatchConfig, NativeExecutor, RejectReason, StepCtx, ValidatorChain,
    };
    use magnetite_sdk::input::{Input, MouseState};
    use magnetite_sdk::protocol::ServerNet;
    use magnetite_sdk::state::PlayerId;

    // Minimal test game — counts ticks.
    struct TickGame {
        ticks: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct TickSnap {
        ticks: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct TickDelta {
        added: u64,
    }

    #[derive(serde::Serialize)]
    struct TickView {
        ticks: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct TickCmd;

    impl AuthoritativeGame for TickGame {
        type Snapshot = TickSnap;
        type Delta = TickDelta;
        type View = TickView;
        type Command = TickCmd;

        fn init(_cfg: &MatchConfig) -> Self {
            TickGame { ticks: 0 }
        }
        fn validate(
            &self,
            _p: PlayerId,
            _i: &Input,
            _t: Tick,
        ) -> Result<Vec<TickCmd>, RejectReason> {
            Ok(vec![])
        }
        fn step(&mut self, _ctx: &mut StepCtx, _cmds: &[(PlayerId, TickCmd)]) {
            self.ticks += 1;
        }
        fn snapshot(&self) -> TickSnap {
            TickSnap { ticks: self.ticks }
        }
        fn restore(s: &TickSnap, _cfg: &MatchConfig) -> Self {
            TickGame { ticks: s.ticks }
        }
        fn delta(&self, since: &TickSnap) -> TickDelta {
            TickDelta {
                added: self.ticks.saturating_sub(since.ticks),
            }
        }
        fn view_for(&self, _p: PlayerId) -> TickView {
            TickView { ticks: self.ticks }
        }
    }

    #[tokio::test]
    async fn tick_scheduler_runs_ticks_and_sends_snapshot() {
        let cfg = MatchConfig {
            tick_hz: 60,
            snapshot_every: 1, // snapshot every tick for test speed
            ..MatchConfig::auto(2)
        };
        let executor = NativeExecutor::<TickGame>::new(cfg.clone());
        let conn_mgr = ConnectionManager::new();
        let conn_mgr2 = conn_mgr.clone();

        // Register a player.
        let (_player_id, mut rx) = conn_mgr.register().await;

        let scheduler = TickScheduler::new(executor, conn_mgr, cfg.clone());

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move {
            scheduler.run(shutdown_rx).await;
        });

        // Wait for at least one snapshot to arrive.
        let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for snapshot")
            .expect("channel closed");

        assert!(
            matches!(msg, ServerNet::Snapshot { .. }),
            "first message should be a bootstrap Snapshot, got {msg:?}"
        );

        // Shutdown.
        let _ = shutdown_tx.send(true);
        let _ = handle.await;
        drop(conn_mgr2);
    }

    #[tokio::test]
    async fn tick_scheduler_sends_ack_for_input() {
        let cfg = MatchConfig {
            tick_hz: 60,
            snapshot_every: 300,
            ..MatchConfig::auto(2)
        };
        let executor = NativeExecutor::<TickGame>::new(cfg.clone());
        let conn_mgr = ConnectionManager::new();
        let conn_mgr_input = conn_mgr.clone();

        let (player_id, mut rx) = conn_mgr.register().await;

        // Push an input before the scheduler starts.
        conn_mgr_input
            .push_input(player_id, 42, Input::default())
            .await;

        let scheduler = TickScheduler::new(executor, conn_mgr, cfg);
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move {
            scheduler.run(shutdown_rx).await;
        });

        // Drain messages until we find an Ack for seq=42.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut found_ack = false;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(ServerNet::Ack { seq: 42, .. })) => {
                    found_ack = true;
                    break;
                }
                Ok(Some(_)) => continue,
                _ => break,
            }
        }

        let _ = shutdown_tx.send(true);
        let _ = handle.await;

        assert!(found_ack, "should have received Ack for seq=42");
    }

    /// Anticheat should drop an aimbot-snap input and send Reject instead of Ack.
    #[tokio::test]
    async fn anticheat_rejects_aimbot_snap() {
        let cfg = MatchConfig {
            tick_hz: 60,
            snapshot_every: 300,
            ..MatchConfig::auto(2)
        };
        let executor = NativeExecutor::<TickGame>::new(cfg.clone());
        let conn_mgr = ConnectionManager::new();
        let conn_mgr_input = conn_mgr.clone();

        let (player_id, mut rx) = conn_mgr.register().await;

        // Build a strict anticheat that catches any mouse delta > 0.0001.
        let chain = ValidatorChain::new().add(AimbotSnap::new(0.0001));
        let ac = Anticheat::new(chain, AnticheatConfig::default());
        let scheduler = TickScheduler::with_anticheat(Box::new(executor), conn_mgr, cfg, ac);

        // Push an input with a huge mouse snap.
        let bad_input = Input {
            mouse: MouseState {
                delta_x: 500.0,
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        conn_mgr_input.push_input(player_id, 7, bad_input).await;

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move {
            scheduler.run(shutdown_rx).await;
        });

        // We should receive a Reject, not an Ack.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut found_reject = false;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(ServerNet::Reject { seq: 7, .. })) => {
                    found_reject = true;
                    break;
                }
                // Snapshot bootstrap frames are expected before the Reject.
                Ok(Some(ServerNet::Snapshot { .. })) => continue,
                Ok(Some(ServerNet::Delta { .. })) => continue,
                Ok(Some(_)) => {}
                _ => break,
            }
        }

        let _ = shutdown_tx.send(true);
        let _ = handle.await;

        assert!(
            found_reject,
            "anticheat should have sent Reject for aimbot-snap input"
        );
    }

    /// An input that passes anticheat but is also sent without a fire-rate
    /// cooldown violation should receive Ack normally.
    #[tokio::test]
    async fn anticheat_allows_clean_input() {
        let cfg = MatchConfig {
            tick_hz: 60,
            snapshot_every: 300,
            ..MatchConfig::auto(2)
        };
        let executor = NativeExecutor::<TickGame>::new(cfg.clone());
        let conn_mgr = ConnectionManager::new();
        let conn_mgr_input = conn_mgr.clone();

        let (player_id, mut rx) = conn_mgr.register().await;

        // Standard anticheat — clean input should pass.
        let chain = ValidatorChain::new().add(FireRateCooldown::new(5));
        let ac = Anticheat::new(chain, AnticheatConfig::default());
        let scheduler = TickScheduler::with_anticheat(Box::new(executor), conn_mgr, cfg, ac);

        // Push a clean (no fire) input.
        conn_mgr_input
            .push_input(player_id, 99, Input::default())
            .await;

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move {
            scheduler.run(shutdown_rx).await;
        });

        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut found_ack = false;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(ServerNet::Ack { seq: 99, .. })) => {
                    found_ack = true;
                    break;
                }
                Ok(Some(ServerNet::Snapshot { .. })) => continue,
                Ok(Some(ServerNet::Delta { .. })) => continue,
                Ok(Some(_)) => {}
                _ => break,
            }
        }

        let _ = shutdown_tx.send(true);
        let _ = handle.await;

        assert!(found_ack, "clean input should receive Ack");
    }

    /// `with_anticheat` constructor should compile and behave the same as `new`.
    #[tokio::test]
    async fn with_anticheat_constructor_works() {
        let cfg = MatchConfig::auto(2);
        let executor = NativeExecutor::<TickGame>::new(cfg.clone());
        let conn_mgr = ConnectionManager::new();
        let ac = Anticheat::new(ValidatorChain::new(), AnticheatConfig::default());
        // Should compile and construct without panic.
        let _scheduler = TickScheduler::with_anticheat(Box::new(executor), conn_mgr, cfg, ac);
    }

    #[test]
    fn server_anticheat_config_alias() {
        // Ensure the re-export compiles and has the same defaults.
        let cfg = ServerAnticheatConfig::default();
        assert!(cfg.kick_threshold > cfg.warn_threshold);
        assert!(cfg.ban_threshold > cfg.kick_threshold);
    }
}
