//! Authoritative tick scheduler.
//!
//! [`TickScheduler`] drives the server loop at exactly `tick_hz` ticks per
//! second.  Each tick it:
//!
//! 1. Drains all buffered client inputs from the [`ConnectionManager`].
//! 2. Calls [`GameExecutor::step`] with the ordered input list.
//! 3. For each player:
//!    - Sends [`ServerNet::Ack`] / [`ServerNet::Reject`] based on step output.
//!    - Sends an interest-filtered [`ServerNet::Delta`] (every tick).
//!    - Sends a full [`ServerNet::Snapshot`] every `snapshot_every` ticks.
//!
//! The scheduler also records a [`ReplayLog`] that can be handed to the
//! anti-cheat verifier.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{self, Instant};
use tracing::{debug, info, warn};

use magnetite_sdk::authority::{GameExecutor, MatchConfig, ReplayLog, Tick};
use magnetite_sdk::input::Input;
use magnetite_sdk::protocol::ServerNet;
use magnetite_sdk::state::PlayerId;

use crate::connection::ConnectionManager;

/// Drives the authoritative tick loop.
///
/// Create via [`TickScheduler::new`] and then call [`TickScheduler::run`]
/// inside a [`tokio::spawn`]'d task.
pub struct TickScheduler {
    executor: Arc<Mutex<Box<dyn GameExecutor>>>,
    connection_mgr: ConnectionManager,
    config: MatchConfig,
    replay_log: Arc<Mutex<ReplayLog>>,
}

impl TickScheduler {
    /// Create a new scheduler.
    ///
    /// - `executor` — any [`GameExecutor`] impl (native or wasm).
    /// - `connection_mgr` — shared connection registry.
    /// - `config` — match configuration (tick rate, snapshot cadence, …).
    pub fn new(
        executor: impl GameExecutor + 'static,
        connection_mgr: ConnectionManager,
        config: MatchConfig,
    ) -> Self {
        let replay_log = ReplayLog::new(config.clone());
        Self {
            executor: Arc::new(Mutex::new(Box::new(executor))),
            connection_mgr,
            replay_log: Arc::new(Mutex::new(replay_log)),
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

        // Build the input slice expected by GameExecutor.
        let executor_inputs: Vec<(PlayerId, Input)> = raw_inputs
            .iter()
            .map(|(id, _seq, input)| (*id, *input))
            .collect();

        // 2. Step the executor (synchronous call, lock held briefly).
        let step_out = {
            let mut exec = self.executor.lock().await;
            exec.step(tick, &executor_inputs)
        };

        // 3. Record in replay log.
        {
            let mut log = self.replay_log.lock().await;
            log.record(tick, executor_inputs.clone(), step_out.state_hash);
        }

        // Build a set of rejected players for O(1) lookup.
        let reject_set: HashMap<PlayerId, magnetite_sdk::authority::RejectReason> = step_out
            .rejects
            .iter()
            .map(
                |(pid, reason): &(PlayerId, magnetite_sdk::authority::RejectReason)| {
                    (*pid, reason.clone())
                },
            )
            .collect();

        // 4. Fan-out per player.
        let player_ids = self.connection_mgr.player_ids().await;

        // Capture current snapshot bytes once (used for deltas and periodic
        // full snapshots).
        let current_snapshot_bytes: Vec<u8> = {
            let exec = self.executor.lock().await;
            exec.snapshot()
        };

        for player_id in &player_ids {
            let player_id = *player_id;

            // 4a. Find the seq for this player (if they sent input).
            let seq_opt = raw_inputs
                .iter()
                .find(|(id, _, _)| *id == player_id)
                .map(|(_, seq, _)| *seq);

            // 4b. Ack or Reject.
            if let Some(seq) = seq_opt {
                if let Some(reason) = reject_set.get(&player_id) {
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

            // 4c. Full snapshot on cadence.
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
                // 4d. Interest-filtered delta every tick.
                let last_snap_tick = self.connection_mgr.last_snapshot_tick(player_id).await;

                // Compute delta relative to the last snapshot the client has.
                // If we have no prior snapshot (last_snap_tick == 0), fall
                // back to sending the full snapshot so the client can
                // bootstrap.
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
    use magnetite_sdk::authority::{
        AuthoritativeGame, MatchConfig, NativeExecutor, RejectReason, StepCtx,
    };
    use magnetite_sdk::input::Input;
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
}
