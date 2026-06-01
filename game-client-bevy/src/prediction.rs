//! Client-side prediction and server reconciliation.
//!
//! This module is **Bevy-free** and can be unit-tested without any graphics
//! stack. It contains the entire prediction/reconciliation loop that the Bevy
//! app calls from its tick system.
//!
//! # Prediction loop (one frame)
//!
//! ```text
//! 1. Build an Input from local key/mouse state (keys: W/A/S/D, mouse aim, attack).
//! 2. Call ClientPredictor::predict(input) → records in PredictionBuffer, returns
//!    the ClientNet::InputFrame to send over the WebSocket.
//! 3. Apply the input to the *local* predicted view for immediate feedback.
//! ```
//!
//! # Reconciliation (on Ack / Snapshot)
//!
//! ```text
//! Ack { seq, tick }:
//!   a. buf.acknowledge(seq)          — drop confirmed frames
//!   b. authoritative_view = server_view  — adopt server truth
//!   c. For each remaining frame in buf.pending():
//!        predicted_view = apply_input(authoritative_view, frame)
//!   → predicted_view is the new display state
//!
//! Snapshot { tick, full }:
//!   a. Deserialise ArenaSnapshot
//!   b. authoritative_view = snapshot-derived view
//!   c. Re-run ALL pending unacked inputs (same as Ack path but from snapshot)
//! ```

use magnetite_sdk::input::Input;
use magnetite_sdk::networking::PredictionBuffer;
use magnetite_sdk::protocol::ClientNet;
use magnetite_sdk::state::PlayerId;
use magnetite_sdk::Tick;

use game_template_authoritative::types::{
    ArenaSnapshot, ArenaView, Projectile, ShooterPlayer, ARENA_HEIGHT, ARENA_WIDTH, MAX_SPEED,
    PROJECTILE_LIFETIME_TICKS, PROJECTILE_SPEED, SHOOT_COOLDOWN_TICKS,
};

// ─────────────────────────────────────────────────────────────────────────────
// Local predicted state
// ─────────────────────────────────────────────────────────────────────────────

/// The client's local prediction of the game world.
///
/// This is derived from the last authoritative [`ArenaView`] received from the
/// server, then advanced forward by re-simulating all unacked input frames.
#[derive(Debug, Clone)]
pub struct PredictedState {
    /// The observing player's predicted position and stats.
    pub self_player: Option<ShooterPlayer>,
    /// Other players as of the last server view (not locally predicted —
    /// we don't have their inputs, so we interpolate or hold last known).
    pub other_players: Vec<ShooterPlayer>,
    /// In-flight projectiles (locally predicted for self; server-authoritative
    /// for others).
    pub projectiles: Vec<Projectile>,
    /// The tick of the last authoritative state we reconciled from.
    pub authoritative_tick: Tick,
}

impl Default for PredictedState {
    fn default() -> Self {
        Self {
            self_player: None,
            other_players: Vec::new(),
            projectiles: Vec::new(),
            authoritative_tick: 0,
        }
    }
}

impl PredictedState {
    /// Build a predicted state from an authoritative [`ArenaView`].
    pub fn from_view(view: &ArenaView, player_id: PlayerId) -> Self {
        let self_player = view.self_state.clone();
        let other_players = view.other_players.clone();
        let projectiles = view.projectiles.clone();
        // Sanity: verify self_player id matches if present.
        debug_assert!(
            self_player
                .as_ref()
                .map(|p| p.id == player_id)
                .unwrap_or(true),
            "view.self_state player id must match local player_id"
        );
        Self {
            self_player,
            other_players,
            projectiles,
            authoritative_tick: view.tick,
        }
    }

    /// Build a predicted state from a full [`ArenaSnapshot`] for `player_id`.
    pub fn from_snapshot(snap: &ArenaSnapshot, player_id: PlayerId) -> Self {
        let self_player = snap.players.iter().find(|p| p.id == player_id).cloned();
        let other_players = snap
            .players
            .iter()
            .filter(|p| p.id != player_id)
            .cloned()
            .collect();
        Self {
            self_player,
            other_players,
            projectiles: snap.projectiles.clone(),
            authoritative_tick: snap.tick,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Local simulation helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Apply one input frame to the local predicted player state.
///
/// This mirrors the server's `ArenaShooter::step` logic for the local player
/// only. It is intentionally simple — the server is always authoritative; we
/// only predict to mask latency.
///
/// # Movement
/// W/A/S/D keys produce a delta clamped to [`MAX_SPEED`].
/// The player is clamped to the arena bounds.
///
/// # Aiming
/// Mouse `x`/`y` (viewport pixels) → angle via `atan2`.
/// We use a fixed viewport size of 800×600 for the reference client.
///
/// # Shooting
/// `attack` key fires if the shoot cooldown has elapsed.
/// Projectile physics advance every frame.
/// Apply one input frame to the local predicted player state and advance projectiles.
///
/// `next_proj_id` is a monotonically increasing local projectile id counter
/// managed by the caller.
pub fn apply_input_to_player(
    player: &mut ShooterPlayer,
    projectiles: &mut Vec<Projectile>,
    input: &Input,
    current_tick: Tick,
    next_proj_id: &mut u64,
) {
    if !player.alive {
        advance_projectiles(projectiles);
        return;
    }

    // ── Movement ──────────────────────────────────────────────────────────
    let mut dx = 0.0f32;
    let mut dy = 0.0f32;
    if input.keys.forward {
        dy += 1.0;
    }
    if input.keys.backward {
        dy -= 1.0;
    }
    if input.keys.left {
        dx -= 1.0;
    }
    if input.keys.right {
        dx += 1.0;
    }

    // Normalise diagonal movement and clamp to MAX_SPEED.
    let mag = (dx * dx + dy * dy).sqrt();
    if mag > 0.0 {
        let scale = MAX_SPEED / mag;
        dx *= scale.min(1.0);
        dy *= scale.min(1.0);
    }

    player.x = (player.x + dx).clamp(-ARENA_WIDTH / 2.0, ARENA_WIDTH / 2.0);
    player.y = (player.y + dy).clamp(-ARENA_HEIGHT / 2.0, ARENA_HEIGHT / 2.0);

    // ── Aim (mouse angle) ─────────────────────────────────────────────────
    // Map viewport mouse position to world angle.
    // Viewport: 800×600 → world: [-100, 100] × [-75, 75] (approximate).
    let view_w = 800.0f32;
    let view_h = 600.0f32;
    let world_x = (input.mouse.x as f32 / view_w - 0.5) * ARENA_WIDTH;
    let world_y = -(input.mouse.y as f32 / view_h - 0.5) * ARENA_HEIGHT;
    let aim_dx = world_x - player.x;
    let aim_dy = world_y - player.y;
    if aim_dx.abs() > 0.1 || aim_dy.abs() > 0.1 {
        player.angle = aim_dy.atan2(aim_dx);
    }

    // ── Shoot ─────────────────────────────────────────────────────────────
    // Mirror the server rule: `last_shot_tick == 0` means "never shot" — first
    // shot is always allowed regardless of cooldown.
    let on_cooldown = player.last_shot_tick > 0
        && current_tick.saturating_sub(player.last_shot_tick) < SHOOT_COOLDOWN_TICKS;
    if input.keys.attack && !on_cooldown {
        player.last_shot_tick = current_tick;
        let vx = player.angle.cos() * PROJECTILE_SPEED;
        let vy = player.angle.sin() * PROJECTILE_SPEED;
        // Use a local counter for projectile IDs (prefixed high bit to
        // distinguish from server-assigned ids if needed).
        *next_proj_id = next_proj_id.wrapping_add(1);
        projectiles.push(Projectile {
            id: *next_proj_id | (1u64 << 63), // high bit = local prediction
            owner: player.id,
            x: player.x,
            y: player.y,
            vx,
            vy,
            ticks_left: PROJECTILE_LIFETIME_TICKS,
        });
    }

    advance_projectiles(projectiles);
}

/// Advance all projectiles by one tick: move + decrement lifetime.
pub fn advance_projectiles(projectiles: &mut Vec<Projectile>) {
    for proj in projectiles.iter_mut() {
        proj.x += proj.vx;
        proj.y += proj.vy;
        proj.ticks_left = proj.ticks_left.saturating_sub(1);
    }
    projectiles.retain(|p| p.ticks_left > 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// ClientPredictor
// ─────────────────────────────────────────────────────────────────────────────

/// The client-side prediction engine.
///
/// Holds the [`PredictionBuffer`], the last authoritative server state, and
/// the current predicted state. Call `predict` once per client frame to record
/// input and advance local state, then call `reconcile_ack` or
/// `reconcile_snapshot` when server messages arrive.
pub struct ClientPredictor {
    /// The player we control on this client.
    pub player_id: PlayerId,
    /// Buffer of unacknowledged input frames.
    pub buffer: PredictionBuffer,
    /// The current locally-predicted game state (what we render).
    pub predicted: PredictedState,
    /// The last authoritative view from the server.
    pub authoritative: PredictedState,
    /// Monotonic client-local sequence number for `ClientNet::InputFrame`.
    next_seq: u32,
    /// Local counter for predicted projectile ids.
    next_proj_id: u64,
    /// The last tick reported in the authoritative state (used for local
    /// simulation when no server tick is available yet).
    pub local_tick: Tick,
}

impl ClientPredictor {
    /// Create a new predictor for the given player.
    ///
    /// `buffer_capacity` is the maximum number of unacked frames to retain.
    /// 128 frames at 60 Hz ≈ ~2 seconds of buffer, enough for high-latency
    /// connections.
    pub fn new(player_id: PlayerId, buffer_capacity: usize) -> Self {
        Self {
            player_id,
            buffer: PredictionBuffer::new(buffer_capacity),
            predicted: PredictedState::default(),
            authoritative: PredictedState::default(),
            next_seq: 0,
            next_proj_id: 0,
            local_tick: 0,
        }
    }

    /// Record the current input, advance the local predicted state, and return
    /// the `ClientNet::InputFrame` to send to the server.
    ///
    /// Call this once per client frame *before* rendering.
    pub fn predict(&mut self, input: Input) -> ClientNet {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);

        // The tick we're targeting: one ahead of the last known authoritative tick.
        self.local_tick = self.local_tick.wrapping_add(1);
        let target_tick = self.local_tick;

        // Record in the prediction buffer (uses input.sequence for ack matching).
        let buffered = Input {
            sequence: u64::from(seq),
            ..input
        };
        self.buffer.push(buffered);

        // Apply the input to the local predicted player.
        if let Some(ref mut player) = self.predicted.self_player {
            apply_input_to_player(
                player,
                &mut self.predicted.projectiles,
                &input,
                target_tick,
                &mut self.next_proj_id,
            );
        }

        ClientNet::InputFrame {
            seq,
            tick: target_tick,
            input,
        }
    }

    /// Handle a [`ServerNet::Ack`] from the server.
    ///
    /// 1. Acknowledges frames ≤ `seq` in the buffer.
    /// 2. Adopts `server_view` as the new authoritative state.
    /// 3. Re-simulates all remaining unacked frames to produce the new
    ///    predicted state.
    pub fn reconcile_ack(&mut self, seq: u32, server_view: ArenaView) {
        // Update the authoritative tick from the view.
        self.local_tick = server_view.tick;

        // Discard acknowledged frames.
        self.buffer.acknowledge(u64::from(seq));

        // Replace authoritative state.
        self.authoritative = PredictedState::from_view(&server_view, self.player_id);

        // Re-simulate unacked inputs on top of authoritative state.
        self.resimulate();
    }

    /// Handle a [`ServerNet::Snapshot`] from the server.
    ///
    /// Replaces the authoritative state from the full snapshot bytes and
    /// re-simulates all pending unacked inputs.
    ///
    /// Returns an error if the snapshot bytes cannot be deserialised.
    pub fn reconcile_snapshot(&mut self, snap_bytes: &[u8]) -> Result<(), serde_json::Error> {
        let snap: ArenaSnapshot = serde_json::from_slice(snap_bytes)?;
        self.local_tick = snap.tick;

        // Replace authoritative state from snapshot.
        self.authoritative = PredictedState::from_snapshot(&snap, self.player_id);

        // Re-simulate all pending unacked inputs.
        self.resimulate();
        Ok(())
    }

    /// Handle a [`ServerNet::Delta`] by applying a partial state update.
    ///
    /// For the reference client we just accept the latest view of other
    /// players and projectiles from the delta, without re-simulating (the
    /// delta does not contain our own authoritative position — use `Ack` for
    /// that). In a production client you would merge the delta more carefully.
    ///
    /// Returns an error if the delta bytes cannot be deserialised.
    pub fn apply_delta(
        &mut self,
        _since_tick: Tick,
        delta_bytes: &[u8],
    ) -> Result<(), serde_json::Error> {
        use game_template_authoritative::types::ArenaDelta;
        let delta: ArenaDelta = serde_json::from_slice(delta_bytes)?;

        // Update changed players (other than self).
        for changed in &delta.changed_players {
            if changed.id == self.player_id {
                // Our own authoritative position — update authoritative state.
                // Predicted state will be corrected on next Ack.
                self.authoritative.self_player = Some(changed.clone());
                continue;
            }
            // Update in predicted state directly (we can't predict other players).
            let entry = self
                .predicted
                .other_players
                .iter_mut()
                .find(|p| p.id == changed.id);
            match entry {
                Some(p) => *p = changed.clone(),
                None => self.predicted.other_players.push(changed.clone()),
            }
        }

        // Remove expired/hit projectiles.
        let removed: std::collections::HashSet<u64> =
            delta.removed_projectile_ids.iter().copied().collect();
        self.predicted.projectiles.retain(|p| {
            // Keep locally-predicted projectiles (high bit set) even if server
            // says one with a different id was removed.
            if p.id & (1u64 << 63) != 0 {
                return true;
            }
            !removed.contains(&p.id)
        });

        // Add new server-authoritative projectiles.
        for new_proj in delta.new_projectiles {
            if !self
                .predicted
                .projectiles
                .iter()
                .any(|p| p.id == new_proj.id)
            {
                self.predicted.projectiles.push(new_proj);
            }
        }

        Ok(())
    }

    /// Re-simulate all currently unacked input frames on top of `authoritative`,
    /// producing a new `predicted` state.
    ///
    /// Called after every `Ack` and `Snapshot` reconciliation.
    fn resimulate(&mut self) {
        // Start from authoritative state.
        let mut predicted = self.authoritative.clone();

        let pending = self.buffer.pending().to_vec();
        let base_tick = self.authoritative.authoritative_tick;

        let mut local_proj_id = self.next_proj_id;

        for (i, frame) in pending.iter().enumerate() {
            let tick = base_tick + i as Tick + 1;
            if let Some(ref mut player) = predicted.self_player {
                apply_input_to_player(
                    player,
                    &mut predicted.projectiles,
                    frame,
                    tick,
                    &mut local_proj_id,
                );
            }
        }

        self.predicted = predicted;
    }

    /// Initialise local state from a `Welcome` message.
    ///
    /// Call this once when the server sends `ServerNet::Welcome`.
    pub fn on_welcome(&mut self, _config: &magnetite_sdk::MatchConfig) {
        // Reset tick counter; the server will send a snapshot shortly.
        self.local_tick = 0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests  (no Bevy dependency)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::input::{Input, KeyState, MouseState};
    use magnetite_sdk::state::PlayerId;

    use game_template_authoritative::types::{ArenaDelta, ArenaView, ShooterPlayer, MAX_HP};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_player(id: u64, x: f32, y: f32) -> ShooterPlayer {
        ShooterPlayer {
            id: PlayerId::new(id),
            x,
            y,
            angle: 0.0,
            hp: MAX_HP,
            alive: true,
            last_shot_tick: 0,
            score: 0,
        }
    }

    fn make_view(player_id: PlayerId, x: f32, y: f32, tick: u64) -> ArenaView {
        ArenaView {
            self_state: Some(make_player(player_id.as_u64(), x, y)),
            other_players: Vec::new(),
            projectiles: Vec::new(),
            tick,
        }
    }

    fn forward_input(seq: u64) -> Input {
        Input {
            keys: KeyState {
                forward: true,
                ..Default::default()
            },
            mouse: MouseState::default(),
            sequence: seq,
            timestamp_ms: 0,
        }
    }

    fn idle_input(seq: u64) -> Input {
        Input {
            sequence: seq,
            ..Default::default()
        }
    }

    // ── basic prediction ──────────────────────────────────────────────────────

    #[test]
    fn predict_advances_local_state() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);

        // Give the predictor an initial state.
        let view = make_view(pid, 0.0, 0.0, 0);
        predictor.authoritative = PredictedState::from_view(&view, pid);
        predictor.predicted = PredictedState::from_view(&view, pid);

        // Predict forward movement.
        let _frame = predictor.predict(forward_input(0));

        let player = predictor.predicted.self_player.as_ref().unwrap();
        // Player should have moved upward (forward = +y).
        assert!(
            player.y > 0.0,
            "forward input must increase y; got {}",
            player.y
        );
    }

    #[test]
    fn predict_records_frame_in_buffer() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);
        let view = make_view(pid, 0.0, 0.0, 0);
        predictor.authoritative = PredictedState::from_view(&view, pid);
        predictor.predicted = PredictedState::from_view(&view, pid);

        predictor.predict(idle_input(0));
        predictor.predict(idle_input(1));
        predictor.predict(idle_input(2));

        assert_eq!(predictor.buffer.len(), 3, "three frames must be buffered");
    }

    // ── ack / reconcile ───────────────────────────────────────────────────────

    #[test]
    fn reconcile_ack_discards_acknowledged_frames() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);
        let view = make_view(pid, 0.0, 0.0, 0);
        predictor.authoritative = PredictedState::from_view(&view, pid);
        predictor.predicted = PredictedState::from_view(&view, pid);

        // Send 5 frames (seq 0..4).
        for i in 0..5 {
            predictor.predict(idle_input(i));
        }
        assert_eq!(predictor.buffer.len(), 5);

        // Server acks seq=2: frames 0,1,2 should be discarded.
        let server_view = make_view(pid, 0.0, 0.0, 3);
        predictor.reconcile_ack(2, server_view);

        // Frames 3 and 4 (seq values assigned internally as 3,4) remain.
        assert_eq!(
            predictor.buffer.len(),
            2,
            "frames 0-2 acknowledged → 2 remaining"
        );
    }

    #[test]
    fn reconcile_ack_adopts_authoritative_position() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);
        let initial = make_view(pid, 0.0, 0.0, 0);
        predictor.authoritative = PredictedState::from_view(&initial, pid);
        predictor.predicted = PredictedState::from_view(&initial, pid);

        // Predict 3 forward frames.
        for i in 0..3u64 {
            predictor.predict(forward_input(i));
        }

        // Server corrects our position to (10, 20) at tick 3.
        let corrected_view = make_view(pid, 10.0, 20.0, 3);
        predictor.reconcile_ack(2, corrected_view);

        // Authoritative position must match the server correction.
        let auth_player = predictor.authoritative.self_player.as_ref().unwrap();
        assert_eq!(auth_player.x, 10.0, "authoritative x must be 10.0");
        assert_eq!(auth_player.y, 20.0, "authoritative y must be 20.0");
    }

    #[test]
    fn reconcile_ack_resimulates_pending_frames() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);
        let initial = make_view(pid, 0.0, 0.0, 0);
        predictor.authoritative = PredictedState::from_view(&initial, pid);
        predictor.predicted = PredictedState::from_view(&initial, pid);

        // Predict 4 frames, all forward.
        for i in 0..4u64 {
            predictor.predict(forward_input(i));
        }
        // Server acks 0 and 1 (seq 0, 1), leaving 2 and 3 unacked.
        let auth_view = make_view(pid, 0.0, 8.0, 2); // server says y=8 at tick 2
        predictor.reconcile_ack(1, auth_view);

        // After reconcile the predicted state should be ahead of (0, 8)
        // because frames 2 and 3 were re-simulated on top.
        let predicted_player = predictor.predicted.self_player.as_ref().unwrap();
        assert!(
            predicted_player.y > 8.0,
            "re-simulated prediction must be ahead of authoritative: y={}",
            predicted_player.y
        );
    }

    // ── snapshot reconcile ────────────────────────────────────────────────────

    #[test]
    fn reconcile_snapshot_replaces_authoritative() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);
        let initial = make_view(pid, 0.0, 0.0, 0);
        predictor.authoritative = PredictedState::from_view(&initial, pid);
        predictor.predicted = PredictedState::from_view(&initial, pid);

        // Predict a few frames.
        for i in 0..3u64 {
            predictor.predict(idle_input(i));
        }

        // Server sends a full snapshot placing us at (50, -30) at tick 10.
        let snap = game_template_authoritative::types::ArenaSnapshot {
            players: vec![make_player(1, 50.0, -30.0)],
            projectiles: Vec::new(),
            tick: 10,
        };
        let snap_bytes = serde_json::to_vec(&snap).unwrap();
        predictor.reconcile_snapshot(&snap_bytes).unwrap();

        let auth_player = predictor.authoritative.self_player.as_ref().unwrap();
        assert_eq!(auth_player.x, 50.0);
        assert_eq!(auth_player.y, -30.0);
    }

    // ── local physics ─────────────────────────────────────────────────────────

    #[test]
    fn apply_input_forward_moves_player() {
        let mut player = make_player(1, 0.0, 0.0);
        let mut projs: Vec<Projectile> = Vec::new();
        let input = forward_input(0);
        let mut next_id = 0u64;
        apply_input_to_player(&mut player, &mut projs, &input, 1, &mut next_id);
        assert!(player.y > 0.0, "forward must increase y");
    }

    #[test]
    fn apply_input_left_moves_player() {
        let mut player = make_player(1, 0.0, 0.0);
        let mut projs: Vec<Projectile> = Vec::new();
        let input = Input {
            keys: KeyState {
                left: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut next_id = 0u64;
        apply_input_to_player(&mut player, &mut projs, &input, 1, &mut next_id);
        assert!(player.x < 0.0, "left must decrease x");
    }

    #[test]
    fn apply_input_clamps_to_arena_bounds() {
        let mut player = make_player(1, ARENA_WIDTH / 2.0, ARENA_HEIGHT / 2.0);
        let mut projs: Vec<Projectile> = Vec::new();
        let input = Input {
            keys: KeyState {
                forward: true,
                right: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut next_id = 0u64;
        apply_input_to_player(&mut player, &mut projs, &input, 1, &mut next_id);
        assert!(
            player.x <= ARENA_WIDTH / 2.0,
            "x must not exceed arena bound"
        );
        assert!(
            player.y <= ARENA_HEIGHT / 2.0,
            "y must not exceed arena bound"
        );
    }

    #[test]
    fn apply_input_diagonal_normalised_to_max_speed() {
        let mut player = make_player(1, 0.0, 0.0);
        let mut projs: Vec<Projectile> = Vec::new();
        let input = Input {
            keys: KeyState {
                forward: true,
                right: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut next_id = 0u64;
        apply_input_to_player(&mut player, &mut projs, &input, 1, &mut next_id);
        let speed = (player.x * player.x + player.y * player.y).sqrt();
        // Speed must be at most MAX_SPEED (floating point tolerance).
        assert!(
            speed <= MAX_SPEED + 1e-4,
            "diagonal speed {speed} must be ≤ MAX_SPEED {MAX_SPEED}"
        );
    }

    #[test]
    fn apply_input_shoot_spawns_projectile() {
        let mut player = make_player(1, 0.0, 0.0);
        let mut projs: Vec<Projectile> = Vec::new();
        let input = Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut next_id = 0u64;
        apply_input_to_player(&mut player, &mut projs, &input, 1, &mut next_id);
        assert_eq!(projs.len(), 1, "one projectile must be spawned");
    }

    #[test]
    fn apply_input_shoot_respects_cooldown() {
        let mut player = make_player(1, 0.0, 0.0);
        player.last_shot_tick = 10; // fired at tick 10
        let mut projs: Vec<Projectile> = Vec::new();
        let input = Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut next_id = 0u64;
        // Tick 11: cooldown not elapsed (SHOOT_COOLDOWN_TICKS = 12).
        apply_input_to_player(&mut player, &mut projs, &input, 11, &mut next_id);
        assert_eq!(projs.len(), 0, "shot must be blocked by cooldown");

        // Tick 10 + SHOOT_COOLDOWN_TICKS: cooldown elapsed.
        apply_input_to_player(
            &mut player,
            &mut projs,
            &input,
            10 + SHOOT_COOLDOWN_TICKS,
            &mut next_id,
        );
        assert_eq!(projs.len(), 1, "shot must succeed after cooldown");
    }

    #[test]
    fn projectiles_advance_and_expire() {
        let mut projs = vec![Projectile {
            id: 1,
            owner: PlayerId::new(1),
            x: 0.0,
            y: 0.0,
            vx: 12.0,
            vy: 0.0,
            ticks_left: 2,
        }];
        advance_projectiles(&mut projs);
        assert_eq!(projs.len(), 1, "projectile still alive after tick 1");
        assert!((projs[0].x - 12.0).abs() < 1e-4, "x must advance by vx");
        advance_projectiles(&mut projs);
        assert_eq!(projs.len(), 0, "projectile must expire after 2 ticks");
    }

    #[test]
    fn predicted_state_from_snapshot() {
        let pid = PlayerId::new(3);
        let snap = game_template_authoritative::types::ArenaSnapshot {
            players: vec![make_player(3, 1.0, 2.0), make_player(4, 3.0, 4.0)],
            projectiles: Vec::new(),
            tick: 99,
        };
        let state = PredictedState::from_snapshot(&snap, pid);
        assert_eq!(state.authoritative_tick, 99);
        let sp = state.self_player.unwrap();
        assert_eq!(sp.id, pid);
        assert_eq!(sp.x, 1.0);
        assert_eq!(state.other_players.len(), 1);
        assert_eq!(state.other_players[0].id, PlayerId::new(4));
    }

    #[test]
    fn apply_delta_updates_other_player() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);
        let view = make_view(pid, 0.0, 0.0, 0);
        predictor.predicted = PredictedState::from_view(&view, pid);
        predictor
            .predicted
            .other_players
            .push(make_player(2, 5.0, 5.0));

        let delta = ArenaDelta {
            changed_players: vec![make_player(2, 99.0, 77.0)],
            removed_projectile_ids: Vec::new(),
            new_projectiles: Vec::new(),
        };
        let delta_bytes = serde_json::to_vec(&delta).unwrap();
        predictor.apply_delta(1, &delta_bytes).unwrap();

        let other = predictor
            .predicted
            .other_players
            .iter()
            .find(|p| p.id == PlayerId::new(2))
            .unwrap();
        assert_eq!(other.x, 99.0);
        assert_eq!(other.y, 77.0);
    }

    #[test]
    fn apply_delta_removes_projectiles() {
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 64);
        let view = make_view(pid, 0.0, 0.0, 0);
        predictor.predicted = PredictedState::from_view(&view, pid);
        predictor.predicted.projectiles.push(Projectile {
            id: 42, // server-assigned id (no high bit)
            owner: PlayerId::new(2),
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            ticks_left: 10,
        });

        let delta = ArenaDelta {
            changed_players: Vec::new(),
            removed_projectile_ids: vec![42],
            new_projectiles: Vec::new(),
        };
        let delta_bytes = serde_json::to_vec(&delta).unwrap();
        predictor.apply_delta(1, &delta_bytes).unwrap();

        assert!(
            predictor.predicted.projectiles.is_empty(),
            "projectile id=42 must be removed"
        );
    }

    // ── full round-trip ───────────────────────────────────────────────────────

    #[test]
    fn full_predict_ack_cycle_converges() {
        // Simulate 10 ticks; server acks every 2.
        let pid = PlayerId::new(1);
        let mut predictor = ClientPredictor::new(pid, 128);
        let initial = make_view(pid, 0.0, 0.0, 0);
        predictor.authoritative = PredictedState::from_view(&initial, pid);
        predictor.predicted = PredictedState::from_view(&initial, pid);

        let mut auth_y = 0.0f32;
        for tick in 0u64..10 {
            predictor.predict(forward_input(tick));
            auth_y += MAX_SPEED; // server simulates the same move

            // Server acks every 2 ticks.
            if tick % 2 == 1 {
                let acked_seq = tick as u32; // seq = tick index
                let server_view = make_view(pid, 0.0, auth_y, tick + 1);
                predictor.reconcile_ack(acked_seq, server_view);
            }
        }

        // After 10 ticks the buffer should only contain at most 2 unacked frames.
        assert!(
            predictor.buffer.len() <= 2,
            "buffer should nearly drain with regular acks; len={}",
            predictor.buffer.len()
        );

        // Predicted y must be at or ahead of authoritative y.
        let pred_y = predictor.predicted.self_player.as_ref().unwrap().y;
        assert!(
            pred_y >= auth_y - 1e-3,
            "predicted y {pred_y} should be >= authoritative y {auth_y}"
        );
    }
}
