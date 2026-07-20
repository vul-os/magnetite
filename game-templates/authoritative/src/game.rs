//! [`ArenaShooter`] — the [`AuthoritativeGame`] implementation.

use magnetite_sdk::authority::{AuthoritativeGame, MatchConfig, RejectReason, StepCtx, Tick};
use magnetite_sdk::input::Input;
use magnetite_sdk::state::PlayerId;

use crate::types::{
    ArenaCommand, ArenaDelta, ArenaSnapshot, ArenaView, Projectile, ShooterPlayer, ARENA_HEIGHT,
    ARENA_WIDTH, MAX_SPEED, PLAYER_RADIUS, PROJECTILE_LIFETIME_TICKS, PROJECTILE_RADIUS,
    PROJECTILE_SPEED, SHOOT_COOLDOWN_TICKS,
};

// ---------------------------------------------------------------------------
// Spawn helpers
// ---------------------------------------------------------------------------

/// Return a deterministic spawn position for player `index` out of `total`.
///
/// Arranges players around the arena perimeter at equal angular intervals,
/// offset from the centre by 80% of the minimum arena half-dimension.
fn spawn_position(index: usize, total: usize) -> (f32, f32) {
    let total = total.max(1) as f32;
    let idx = index as f32;
    // Full circle, evenly spaced.
    let angle = (2.0 * core::f32::consts::PI * idx) / total;
    let radius = (ARENA_WIDTH.min(ARENA_HEIGHT) * 0.4).min(80.0);
    (radius * angle.cos(), radius * angle.sin())
}

// ---------------------------------------------------------------------------
// ArenaShooter
// ---------------------------------------------------------------------------

/// Reference authoritative top-down arena shooter.
///
/// All mutation happens inside [`step`](AuthoritativeGame::step); `validate`
/// only translates raw input into clean commands, never mutates state.
pub struct ArenaShooter {
    players: Vec<ShooterPlayer>,
    projectiles: Vec<Projectile>,
    tick: Tick,
    /// Max players, from [`MatchConfig::max_players`].
    max_players: u32,
}

impl AuthoritativeGame for ArenaShooter {
    type Snapshot = ArenaSnapshot;
    type Delta = ArenaDelta;
    type View = ArenaView;
    type Command = ArenaCommand;

    // ------------------------------------------------------------------ //
    // Lifecycle                                                            //
    // ------------------------------------------------------------------ //

    fn init(cfg: &MatchConfig) -> Self {
        Self {
            players: Vec::new(),
            projectiles: Vec::new(),
            tick: 0,
            max_players: cfg.max_players,
        }
    }

    fn on_join(&mut self, p: PlayerId) {
        // Spawn at a deterministic position based on join order.
        let index = self.players.len();
        let total = self.max_players as usize;
        let (x, y) = spawn_position(index, total);
        self.players.push(ShooterPlayer::spawn(p, x, y));
    }

    fn on_leave(&mut self, p: PlayerId) {
        self.players.retain(|ps| ps.id != p);
        self.projectiles.retain(|proj| proj.owner != p);
    }

    // ------------------------------------------------------------------ //
    // Validation                                                           //
    // ------------------------------------------------------------------ //

    /// Translate raw client input into 0–3 authoritative commands.
    ///
    /// Rules:
    /// * Dead players → `Err(Unauthorized)`.
    /// * Movement: normalised to [`MAX_SPEED`] if the client over-reports.
    /// * Shoot: rejected if the per-player cooldown has not elapsed.
    fn validate(
        &self,
        player: PlayerId,
        input: &Input,
        tick: Tick,
    ) -> Result<Vec<ArenaCommand>, RejectReason> {
        // Look up the player.
        let ps = self
            .players
            .iter()
            .find(|p| p.id == player)
            .ok_or(RejectReason::Unauthorized)?;

        if !ps.alive {
            return Err(RejectReason::Unauthorized);
        }

        let mut commands = Vec::new();

        // Movement: W/A/S/D → dx/dy.
        let raw_dx = if input.keys.right { 1.0_f32 } else { 0.0 }
            - if input.keys.left { 1.0_f32 } else { 0.0 };
        let raw_dy = if input.keys.forward { 1.0_f32 } else { 0.0 }
            - if input.keys.backward { 1.0_f32 } else { 0.0 };

        if raw_dx != 0.0 || raw_dy != 0.0 {
            // Normalise diagonal movement to MAX_SPEED.
            let mag = (raw_dx * raw_dx + raw_dy * raw_dy).sqrt();
            let dx = (raw_dx / mag) * MAX_SPEED;
            let dy = (raw_dy / mag) * MAX_SPEED;
            commands.push(ArenaCommand::Move { dx, dy });
        }

        // Aim: derive from mouse position relative to some virtual screen centre.
        // In this reference we use mouse_x/y as world-space aim direction.
        // (A real client would send the angle; here we compute from the mouse delta.)
        if input.mouse.delta_x.abs() > 0.001 || input.mouse.delta_y.abs() > 0.001 {
            let angle = (input.mouse.delta_y as f32).atan2(input.mouse.delta_x as f32);
            commands.push(ArenaCommand::Aim { angle });
        }

        // Shoot: left mouse button or `attack` key; enforce cooldown.
        // `last_shot_tick == 0` means "never shot" — first shot always allowed.
        if input.keys.attack || input.mouse.left_button {
            let on_cooldown = ps.last_shot_tick > 0
                && tick.saturating_sub(ps.last_shot_tick) < SHOOT_COOLDOWN_TICKS;
            if on_cooldown {
                // Cooldown still active — omit the Shoot command; movement
                // commands already queued are still valid.
            } else {
                commands.push(ArenaCommand::Shoot);
            }
        }

        Ok(commands)
    }

    // ------------------------------------------------------------------ //
    // Step (deterministic state transition)                               //
    // ------------------------------------------------------------------ //

    fn step(&mut self, ctx: &mut StepCtx, commands: &[(PlayerId, ArenaCommand)]) {
        self.tick = ctx.tick;

        // 1. Apply player commands.
        for (player_id, cmd) in commands {
            if let Some(ps) = self.players.iter_mut().find(|p| p.id == *player_id) {
                if !ps.alive {
                    continue;
                }
                match cmd {
                    ArenaCommand::Move { dx, dy } => {
                        ps.x = (ps.x + dx).clamp(-ARENA_WIDTH / 2.0, ARENA_WIDTH / 2.0);
                        ps.y = (ps.y + dy).clamp(-ARENA_HEIGHT / 2.0, ARENA_HEIGHT / 2.0);
                    }
                    ArenaCommand::Aim { angle } => {
                        ps.angle = *angle;
                    }
                    ArenaCommand::Shoot => {
                        // Use deterministic RNG only for the projectile ID.
                        let proj_id = ctx.rng.next_u64();
                        let vx = ps.angle.cos() * PROJECTILE_SPEED;
                        let vy = ps.angle.sin() * PROJECTILE_SPEED;
                        self.projectiles.push(Projectile {
                            id: proj_id,
                            owner: *player_id,
                            x: ps.x,
                            y: ps.y,
                            vx,
                            vy,
                            ticks_left: PROJECTILE_LIFETIME_TICKS,
                        });
                        ps.last_shot_tick = ctx.tick;
                    }
                }
            }
        }

        // 2. Advance projectiles and detect collisions.
        let mut hits: Vec<(PlayerId, PlayerId)> = Vec::new(); // (victim, shooter)
        let mut expired_ids: Vec<u64> = Vec::new();

        for proj in &mut self.projectiles {
            proj.x += proj.vx;
            proj.y += proj.vy;
            proj.ticks_left = proj.ticks_left.saturating_sub(1);

            if proj.ticks_left == 0 {
                expired_ids.push(proj.id);
                continue;
            }

            // Out of arena → expire.
            if proj.x.abs() > ARENA_WIDTH / 2.0 + PROJECTILE_RADIUS
                || proj.y.abs() > ARENA_HEIGHT / 2.0 + PROJECTILE_RADIUS
            {
                expired_ids.push(proj.id);
                continue;
            }

            // Check collision against all alive players (excluding owner).
            for ps in &self.players {
                if !ps.alive || ps.id == proj.owner {
                    continue;
                }
                let dx = proj.x - ps.x;
                let dy = proj.y - ps.y;
                let dist_sq = dx * dx + dy * dy;
                let hit_dist = PROJECTILE_RADIUS + PLAYER_RADIUS;
                if dist_sq <= hit_dist * hit_dist {
                    hits.push((ps.id, proj.owner));
                    expired_ids.push(proj.id);
                    break; // one hit per projectile
                }
            }
        }

        // 3. Remove expired/hit projectiles (deduplicate by id).
        expired_ids.sort_unstable();
        expired_ids.dedup();
        self.projectiles.retain(|p| !expired_ids.contains(&p.id));

        // 4. Apply damage and update scores.
        for (victim_id, shooter_id) in hits {
            let mut killed = false;
            if let Some(victim) = self.players.iter_mut().find(|p| p.id == victim_id) {
                victim.hp -= crate::types::HIT_DAMAGE;
                if victim.hp <= 0.0 {
                    victim.hp = 0.0;
                    victim.alive = false;
                    killed = true;
                }
            }
            if killed {
                if let Some(shooter) = self.players.iter_mut().find(|p| p.id == shooter_id) {
                    shooter.score += 1;
                }
            }
        }

        // 5. Keep collections sorted for deterministic snapshot hashing.
        self.players.sort_by_key(|p| p.id.as_u64());
        self.projectiles.sort_by_key(|p| p.id);
    }

    // ------------------------------------------------------------------ //
    // Snapshot / delta / view                                             //
    // ------------------------------------------------------------------ //

    fn snapshot(&self) -> ArenaSnapshot {
        ArenaSnapshot {
            players: self.players.clone(),
            projectiles: self.projectiles.clone(),
            tick: self.tick,
        }
    }

    fn restore(snap: &ArenaSnapshot, cfg: &MatchConfig) -> Self {
        Self {
            players: snap.players.clone(),
            projectiles: snap.projectiles.clone(),
            tick: snap.tick,
            max_players: cfg.max_players,
        }
    }

    fn delta(&self, since: &ArenaSnapshot) -> ArenaDelta {
        // Players whose state differs from the prior snapshot.
        let changed_players: Vec<ShooterPlayer> = self
            .players
            .iter()
            .filter(|p| !since.players.iter().any(|sp| sp.id == p.id && sp == *p))
            .cloned()
            .collect();

        // Projectile IDs present in `since` but not in current state → removed.
        let removed_projectile_ids: Vec<u64> = since
            .projectiles
            .iter()
            .filter(|sp| !self.projectiles.iter().any(|p| p.id == sp.id))
            .map(|sp| sp.id)
            .collect();

        // Projectiles present now but not in `since` → new.
        let new_projectiles: Vec<Projectile> = self
            .projectiles
            .iter()
            .filter(|p| !since.projectiles.iter().any(|sp| sp.id == p.id))
            .cloned()
            .collect();

        ArenaDelta {
            changed_players,
            removed_projectile_ids,
            new_projectiles,
        }
    }

    fn view_for(&self, player: PlayerId) -> ArenaView {
        let self_state = self.players.iter().find(|p| p.id == player).cloned();
        let other_players = self
            .players
            .iter()
            .filter(|p| p.id != player)
            .cloned()
            .collect();
        ArenaView {
            self_state,
            other_players,
            projectiles: self.projectiles.clone(),
            tick: self.tick,
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
        DeterministicRng, GameExecutor, MatchConfig, NativeExecutor, ReplayLog, ReplayVerdict,
        StepCtx,
    };
    use magnetite_sdk::input::{Input, KeyState, MouseState};
    use magnetite_sdk::state::PlayerId;

    fn make_cfg() -> MatchConfig {
        MatchConfig::auto(4)
    }

    fn make_input_move_right() -> Input {
        Input {
            keys: KeyState {
                right: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn make_input_shoot() -> Input {
        Input {
            keys: KeyState {
                attack: true,
                ..Default::default()
            },
            mouse: MouseState {
                delta_x: 1.0,
                delta_y: 0.0,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    // ------------------------------------------------------------------ //
    // Basic lifecycle                                                      //
    // ------------------------------------------------------------------ //

    #[test]
    fn init_creates_empty_state() {
        let cfg = make_cfg();
        let game = ArenaShooter::init(&cfg);
        let snap = game.snapshot();
        assert!(snap.players.is_empty());
        assert!(snap.projectiles.is_empty());
    }

    #[test]
    fn on_join_spawns_player() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        let snap = game.snapshot();
        assert_eq!(snap.players.len(), 1);
        assert_eq!(snap.players[0].id, p1);
        assert_eq!(snap.players[0].hp, crate::types::MAX_HP);
        assert!(snap.players[0].alive);
    }

    #[test]
    fn on_leave_removes_player_and_projectiles() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        // Manually add a projectile owned by p1.
        game.projectiles.push(Projectile {
            id: 42,
            owner: p1,
            x: 0.0,
            y: 0.0,
            vx: 1.0,
            vy: 0.0,
            ticks_left: 10,
        });
        game.on_leave(p1);
        let snap = game.snapshot();
        assert!(snap.players.is_empty());
        assert!(snap.projectiles.is_empty());
    }

    // ------------------------------------------------------------------ //
    // Movement + arena clamping                                            //
    // ------------------------------------------------------------------ //

    #[test]
    fn player_moves_right_on_right_key() {
        let p1 = PlayerId::new(1);

        let cfg2 = make_cfg();
        let mut game = ArenaShooter::init(&cfg2);
        game.on_join(p1);
        let x0 = game.players[0].x;
        let mut rng = DeterministicRng::new(0);
        let mut ctx = StepCtx {
            tick: 1,
            dt_ms: 16,
            rng: &mut rng,
        };
        let cmds = game.validate(p1, &make_input_move_right(), 1).unwrap();
        let pairs: Vec<(PlayerId, ArenaCommand)> = cmds.into_iter().map(|c| (p1, c)).collect();
        game.step(&mut ctx, &pairs);
        assert!(
            game.players[0].x > x0,
            "player x should increase after moving right"
        );
    }

    #[test]
    fn player_clamped_at_arena_boundary() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        // Manually place player near right edge.
        game.players[0].x = ARENA_WIDTH / 2.0 - 1.0;

        let mut rng = DeterministicRng::new(0);
        // Move right many times — should clamp.
        for tick in 1u64..=20 {
            let mut ctx = StepCtx {
                tick,
                dt_ms: 16,
                rng: &mut rng,
            };
            let cmds = game.validate(p1, &make_input_move_right(), tick).unwrap();
            let pairs: Vec<(PlayerId, ArenaCommand)> = cmds.into_iter().map(|c| (p1, c)).collect();
            game.step(&mut ctx, &pairs);
        }
        assert!(
            game.players[0].x <= ARENA_WIDTH / 2.0,
            "player must not exceed arena boundary"
        );
    }

    // ------------------------------------------------------------------ //
    // Shooting and cooldown                                                //
    // ------------------------------------------------------------------ //

    #[test]
    fn shoot_creates_projectile() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        game.players[0].angle = 0.0; // aim right

        let mut rng = DeterministicRng::new(99);
        let mut ctx = StepCtx {
            tick: 1,
            dt_ms: 16,
            rng: &mut rng,
        };
        let cmds = game.validate(p1, &make_input_shoot(), 1).unwrap();
        assert!(
            cmds.iter().any(|c| matches!(c, ArenaCommand::Shoot)),
            "shoot input must produce Shoot command"
        );
        let pairs: Vec<(PlayerId, ArenaCommand)> = cmds.into_iter().map(|c| (p1, c)).collect();
        game.step(&mut ctx, &pairs);
        assert_eq!(game.projectiles.len(), 1);
    }

    #[test]
    fn shoot_cooldown_enforced_in_validate() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);

        // First shot: allowed.
        let cmds = game.validate(p1, &make_input_shoot(), 1).unwrap();
        assert!(cmds.iter().any(|c| matches!(c, ArenaCommand::Shoot)));

        // Record last_shot_tick manually.
        game.players[0].last_shot_tick = 1;

        // Next tick (tick 2): cooldown not elapsed (SHOOT_COOLDOWN_TICKS = 12).
        let cmds = game.validate(p1, &make_input_shoot(), 2).unwrap();
        assert!(
            !cmds.iter().any(|c| matches!(c, ArenaCommand::Shoot)),
            "shoot must be absent during cooldown"
        );

        // Tick 1 + SHOOT_COOLDOWN_TICKS = 13: allowed again.
        let cmds = game
            .validate(p1, &make_input_shoot(), 1 + SHOOT_COOLDOWN_TICKS)
            .unwrap();
        assert!(
            cmds.iter().any(|c| matches!(c, ArenaCommand::Shoot)),
            "shoot must be allowed after cooldown"
        );
    }

    // ------------------------------------------------------------------ //
    // Projectile travel + expiry                                          //
    // ------------------------------------------------------------------ //

    #[test]
    fn projectile_expires_after_lifetime() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        game.players[0].angle = 0.0;

        let mut rng = DeterministicRng::new(7);
        // Shoot on tick 1.
        {
            let mut ctx = StepCtx {
                tick: 1,
                dt_ms: 16,
                rng: &mut rng,
            };
            let cmds = game.validate(p1, &make_input_shoot(), 1).unwrap();
            let pairs: Vec<(PlayerId, ArenaCommand)> = cmds.into_iter().map(|c| (p1, c)).collect();
            game.step(&mut ctx, &pairs);
        }
        assert_eq!(game.projectiles.len(), 1);

        // Advance until lifetime expires.
        for tick in 2u64..=(1 + PROJECTILE_LIFETIME_TICKS as u64) {
            let mut ctx = StepCtx {
                tick,
                dt_ms: 16,
                rng: &mut rng,
            };
            game.step(&mut ctx, &[]);
        }
        assert_eq!(
            game.projectiles.len(),
            0,
            "projectile must expire after lifetime"
        );
    }

    // ------------------------------------------------------------------ //
    // Hit detection + damage                                              //
    // ------------------------------------------------------------------ //

    #[test]
    fn projectile_hits_and_damages_enemy() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);
        game.on_join(p1);
        game.on_join(p2);

        // Place p1 at (0, 0) aiming right, p2 just to the right.
        game.players.sort_by_key(|p| p.id.as_u64());
        game.players[0].x = 0.0;
        game.players[0].y = 0.0;
        game.players[0].angle = 0.0;
        // Place p2 a few units right — within one tick of PROJECTILE_SPEED.
        game.players[1].x = PROJECTILE_SPEED + PLAYER_RADIUS - 0.5;
        game.players[1].y = 0.0;

        let hp_before = game.players[1].hp;
        let mut rng = DeterministicRng::new(55);
        // Tick 1: shoot.
        {
            let mut ctx = StepCtx {
                tick: 1,
                dt_ms: 16,
                rng: &mut rng,
            };
            let cmds = game.validate(p1, &make_input_shoot(), 1).unwrap();
            let pairs: Vec<(PlayerId, ArenaCommand)> = cmds.into_iter().map(|c| (p1, c)).collect();
            game.step(&mut ctx, &pairs);
        }
        // Tick 2: projectile advances and hits p2.
        {
            let mut ctx = StepCtx {
                tick: 2,
                dt_ms: 16,
                rng: &mut rng,
            };
            game.step(&mut ctx, &[]);
        }
        let p2_state = game.players.iter().find(|p| p.id == p2).unwrap();
        assert!(
            p2_state.hp < hp_before,
            "p2 should have taken damage from projectile"
        );
    }

    #[test]
    fn kill_increments_shooter_score() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);
        game.on_join(p1);
        game.on_join(p2);

        game.players.sort_by_key(|p| p.id.as_u64());
        // Set p2 HP to just above HIT_DAMAGE so one hit kills.
        game.players[1].hp = crate::types::HIT_DAMAGE - 0.1;
        game.players[0].x = 0.0;
        game.players[0].y = 0.0;
        game.players[0].angle = 0.0;
        game.players[1].x = PROJECTILE_SPEED + PLAYER_RADIUS - 0.5;
        game.players[1].y = 0.0;

        let mut rng = DeterministicRng::new(88);
        {
            let mut ctx = StepCtx {
                tick: 1,
                dt_ms: 16,
                rng: &mut rng,
            };
            let cmds = game.validate(p1, &make_input_shoot(), 1).unwrap();
            let pairs: Vec<(PlayerId, ArenaCommand)> = cmds.into_iter().map(|c| (p1, c)).collect();
            game.step(&mut ctx, &pairs);
        }
        {
            let mut ctx = StepCtx {
                tick: 2,
                dt_ms: 16,
                rng: &mut rng,
            };
            game.step(&mut ctx, &[]);
        }
        let p1_state = game.players.iter().find(|p| p.id == p1).unwrap();
        assert_eq!(p1_state.score, 1, "kill should increment shooter score");
        let p2_state = game.players.iter().find(|p| p.id == p2).unwrap();
        assert!(!p2_state.alive, "p2 should be dead");
    }

    // ------------------------------------------------------------------ //
    // Dead player validation                                              //
    // ------------------------------------------------------------------ //

    #[test]
    fn dead_player_input_rejected() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        game.players[0].alive = false;

        let result = game.validate(p1, &make_input_move_right(), 1);
        assert_eq!(result, Err(RejectReason::Unauthorized));
    }

    // ------------------------------------------------------------------ //
    // Snapshot / restore                                                  //
    // ------------------------------------------------------------------ //

    #[test]
    fn snapshot_restore_roundtrip() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);

        let snap = game.snapshot();
        let restored = ArenaShooter::restore(&snap, &cfg);
        assert_eq!(
            restored.snapshot(),
            snap,
            "restore must produce identical snapshot"
        );
    }

    #[test]
    fn native_executor_snapshot_restore_deterministic() {
        let cfg = make_cfg();
        let mut exec = NativeExecutor::<ArenaShooter>::new(cfg.clone());
        // We can't call on_join via GameExecutor — use raw game + restore.
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        let initial_snap_bytes = serde_json::to_vec(&game.snapshot()).unwrap();
        exec.restore(&initial_snap_bytes);

        let input = make_input_move_right();
        let out1 = exec.step(1, &[(p1, input.clone())]);
        let snap_bytes = exec.snapshot();

        // Restore and replay.
        exec.restore(&initial_snap_bytes);
        let out1b = exec.step(1, &[(p1, input)]);

        assert_eq!(
            out1.state_hash, out1b.state_hash,
            "same inputs after restore must yield same hash"
        );
        let _ = snap_bytes; // used above
    }

    // ------------------------------------------------------------------ //
    // Delta                                                               //
    // ------------------------------------------------------------------ //

    #[test]
    fn delta_no_change_when_identical() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        let snap = game.snapshot();
        let delta = game.delta(&snap);
        assert!(
            delta.changed_players.is_empty(),
            "no changes → empty changed_players"
        );
        assert!(delta.new_projectiles.is_empty());
        assert!(delta.removed_projectile_ids.is_empty());
    }

    #[test]
    fn delta_detects_player_move() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        game.on_join(p1);
        let snap_before = game.snapshot();

        let mut rng = DeterministicRng::new(0);
        let mut ctx = StepCtx {
            tick: 1,
            dt_ms: 16,
            rng: &mut rng,
        };
        let cmds = game.validate(p1, &make_input_move_right(), 1).unwrap();
        let pairs: Vec<(PlayerId, ArenaCommand)> = cmds.into_iter().map(|c| (p1, c)).collect();
        game.step(&mut ctx, &pairs);

        let delta = game.delta(&snap_before);
        assert!(
            !delta.changed_players.is_empty(),
            "moved player must appear in delta"
        );
    }

    // ------------------------------------------------------------------ //
    // View (interest filtering)                                           //
    // ------------------------------------------------------------------ //

    #[test]
    fn view_for_own_player() {
        let cfg = make_cfg();
        let mut game = ArenaShooter::init(&cfg);
        let p1 = PlayerId::new(1);
        let p2 = PlayerId::new(2);
        game.on_join(p1);
        game.on_join(p2);

        let view = game.view_for(p1);
        assert!(
            view.self_state.is_some(),
            "view_for must include own player state"
        );
        assert_eq!(view.self_state.unwrap().id, p1);
        assert_eq!(
            view.other_players.len(),
            1,
            "view_for must include other players"
        );
        assert_eq!(view.other_players[0].id, p2);
    }

    #[test]
    fn view_for_unknown_player_self_is_none() {
        let cfg = make_cfg();
        let game = ArenaShooter::init(&cfg);
        let view = game.view_for(PlayerId::new(99));
        assert!(view.self_state.is_none());
    }

    // ------------------------------------------------------------------ //
    // Determinism — replay verification                                   //
    // ------------------------------------------------------------------ //

    #[test]
    fn determinism_same_seed_same_hash_sequence() {
        // Two independent executors with identical seed must produce identical
        // state hashes on every tick when given identical inputs.
        let cfg_a = MatchConfig {
            seed: 12345,
            ..make_cfg()
        };
        let cfg_b = MatchConfig {
            seed: 12345,
            ..make_cfg()
        };
        let p1 = PlayerId::new(1);

        let mut exec_a = NativeExecutor::<ArenaShooter>::new(cfg_a.clone());
        let mut exec_b = NativeExecutor::<ArenaShooter>::new(cfg_b.clone());

        // Inject initial state with player joined (same for both).
        let mut init_game = ArenaShooter::init(&cfg_a);
        init_game.on_join(p1);
        let init_bytes = serde_json::to_vec(&init_game.snapshot()).unwrap();
        exec_a.restore(&init_bytes);
        exec_b.restore(&init_bytes);

        for tick in 1u64..=10 {
            let input = make_input_move_right();
            let out_a = exec_a.step(tick, &[(p1, input.clone())]);
            let out_b = exec_b.step(tick, &[(p1, input)]);
            assert_eq!(
                out_a.state_hash, out_b.state_hash,
                "hash mismatch at tick {tick}"
            );
        }
    }

    #[test]
    fn replay_verify_clean() {
        // `verify_replay` re-creates an ArenaShooter from `log.config` via
        // `NativeExecutor::new` — which produces an empty game (no players).
        // We record the log against the same empty-game executor so the hashes
        // stay consistent between the recording run and the re-simulation.
        let cfg = MatchConfig {
            seed: 77777,
            ..make_cfg()
        };

        // Empty game — no players joined; all inputs produce empty command lists
        // (validate returns Ok([])) but the game still steps and produces a hash.
        let mut exec = NativeExecutor::<ArenaShooter>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg.clone());

        for tick in 1u64..=8 {
            // Empty input list — no players, nothing to validate.
            let inputs: Vec<(PlayerId, Input)> = vec![];
            let out = exec.step(tick, &inputs);
            log.record(tick, inputs, out.state_hash);
        }

        let verdict = magnetite_sdk::authority::verify_replay::<ArenaShooter>(&log);
        assert_eq!(
            verdict,
            ReplayVerdict::Clean,
            "clean log must verify as Clean"
        );
    }

    #[test]
    fn replay_verify_tampered_diverges() {
        // Same approach: record against an empty game so verify_replay agrees
        // on the starting state, then tamper one hash to force divergence.
        let cfg = MatchConfig {
            seed: 42,
            ..make_cfg()
        };

        let mut exec = NativeExecutor::<ArenaShooter>::new(cfg.clone());
        let mut log = ReplayLog::new(cfg.clone());

        for tick in 1u64..=5 {
            let inputs: Vec<(PlayerId, Input)> = vec![];
            let out = exec.step(tick, &inputs);
            log.record(tick, inputs, out.state_hash);
        }

        // Tamper the hash at tick 3.
        for (tick, hash) in &mut log.state_hashes {
            if *tick == 3 {
                *hash = hash.wrapping_add(1);
            }
        }

        let verdict = magnetite_sdk::authority::verify_replay::<ArenaShooter>(&log);
        assert!(
            matches!(verdict, ReplayVerdict::Divergence { tick: 3, .. }),
            "tampered log must diverge at tick 3, got {verdict:?}"
        );
    }
}
