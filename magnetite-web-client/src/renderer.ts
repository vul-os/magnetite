/**
 * magnetite-web-client/src/renderer.js
 *
 * Canvas renderer for the arena-shooter ArenaView.
 *
 * The renderer is pluggable: createClient accepts a custom `render` function.
 * This default renderer handles the arena-shooter View shape:
 *
 *   ArenaView {
 *     self_state: ShooterPlayer | null,
 *     other_players: ShooterPlayer[],
 *     projectiles: Projectile[],
 *     tick: u64
 *   }
 *
 *   ShooterPlayer { id, x, y, angle, hp, alive, last_shot_tick, score }
 *   Projectile    { id, owner, x, y, vx, vy, ticks_left }
 *
 * World coordinate system: origin at centre of canvas.
 * Arena: 200×200 world units (constants from types.rs).
 */

const ARENA_WIDTH = 200;
const ARENA_HEIGHT = 200;
const PLAYER_RADIUS = 3.0;
const PROJECTILE_RADIUS = 1.5;
const MAX_HP = 100.0;

// Design colours — Industrial Magnetite palette
const COLORS = {
  bg: '#07070b',
  bgGrid: '#0f0f16',
  arena: '#14141d',
  arenaBorder: '#23232e',
  self: '#38e1c8',     // electric teal — local player
  enemy: '#ff5468',    // red — enemies
  dead: '#3b3b4a',     // muted — dead players
  projectileSelf: '#38e1c8',
  projectileEnemy: '#f5a524',
  hpBar: '#3ddc84',
  hpBarLow: '#ff5468',
  text: '#f4f4f6',
  textMuted: '#6b6b78',
  healthBarBg: '#1b1b27',
};

// ---------------------------------------------------------------------------
// Arena renderer
// ---------------------------------------------------------------------------

/**
 * Default arena-shooter render function.
 *
 * Suitable as the `render` argument to createClient for the arena-shooter game.
 *
 * @param {CanvasRenderingContext2D} ctx  - 2D canvas context
 * @param {import('./types.js').ArenaView} state - current (possibly predicted) view
 * @param {string | null} _localPlayerId  - the local player's id string (for colouring)
 */
export function renderArenaView(ctx, state, _localPlayerId) {
  const cw = ctx.canvas.width;
  const ch = ctx.canvas.height;

  // Scale: world units → pixels
  const scaleX = cw / ARENA_WIDTH;
  const scaleY = ch / ARENA_HEIGHT;
  const scale = Math.min(scaleX, scaleY);

  // Clear
  ctx.fillStyle = COLORS.bg;
  ctx.fillRect(0, 0, cw, ch);

  ctx.save();

  // Center origin
  ctx.translate(cw / 2, ch / 2);
  ctx.scale(scale, scale);

  // Draw arena floor
  ctx.fillStyle = COLORS.arena;
  ctx.fillRect(-ARENA_WIDTH / 2, -ARENA_HEIGHT / 2, ARENA_WIDTH, ARENA_HEIGHT);

  // Draw subtle grid
  _drawGrid(ctx);

  // Draw arena border
  ctx.strokeStyle = COLORS.arenaBorder;
  ctx.lineWidth = 0.5;
  ctx.strokeRect(-ARENA_WIDTH / 2, -ARENA_HEIGHT / 2, ARENA_WIDTH, ARENA_HEIGHT);

  // Draw projectiles
  const allProjectiles = state.projectiles || [];
  for (const proj of allProjectiles) {
    const isMine = state.self_state && String(proj.owner) === String(state.self_state.id);
    ctx.fillStyle = isMine ? COLORS.projectileSelf : COLORS.projectileEnemy;
    ctx.beginPath();
    ctx.arc(proj.x, proj.y, PROJECTILE_RADIUS, 0, Math.PI * 2);
    ctx.fill();
  }

  // Draw other players
  const others = state.other_players || [];
  for (const player of others) {
    _drawPlayer(ctx, player, COLORS.enemy, false);
  }

  // Draw self (on top)
  if (state.self_state) {
    _drawPlayer(ctx, state.self_state, COLORS.self, true);
  }

  ctx.restore();

  // HUD overlay (in screen space)
  _drawHUD(ctx, state, cw, ch);
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

function _drawGrid(ctx) {
  ctx.strokeStyle = 'rgba(35,35,46,0.5)';
  ctx.lineWidth = 0.25;
  const step = 20;
  for (let x = -ARENA_WIDTH / 2; x <= ARENA_WIDTH / 2; x += step) {
    ctx.beginPath();
    ctx.moveTo(x, -ARENA_HEIGHT / 2);
    ctx.lineTo(x, ARENA_HEIGHT / 2);
    ctx.stroke();
  }
  for (let y = -ARENA_HEIGHT / 2; y <= ARENA_HEIGHT / 2; y += step) {
    ctx.beginPath();
    ctx.moveTo(-ARENA_WIDTH / 2, y);
    ctx.lineTo(ARENA_WIDTH / 2, y);
    ctx.stroke();
  }
}

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {{ x: number, y: number, angle: number, hp: number, alive: boolean, score: number, id: unknown }} player
 * @param {string} color
 * @param {boolean} isSelf
 */
function _drawPlayer(ctx, player, color, isSelf) {
  const alpha = player.alive ? 1.0 : 0.3;
  ctx.globalAlpha = alpha;

  const fillColor = player.alive ? color : COLORS.dead;

  // Body circle
  ctx.beginPath();
  ctx.arc(player.x, player.y, PLAYER_RADIUS, 0, Math.PI * 2);
  ctx.fillStyle = fillColor;
  ctx.fill();

  // Outline
  ctx.strokeStyle = isSelf ? '#ffffff' : 'rgba(255,255,255,0.3)';
  ctx.lineWidth = 0.5;
  ctx.stroke();

  // Direction indicator (gun barrel)
  if (player.alive) {
    ctx.beginPath();
    ctx.moveTo(player.x, player.y);
    const barrelLen = PLAYER_RADIUS * 2.0;
    ctx.lineTo(
      player.x + Math.cos(player.angle) * barrelLen,
      player.y + Math.sin(player.angle) * barrelLen
    );
    ctx.strokeStyle = fillColor;
    ctx.lineWidth = 0.8;
    ctx.stroke();
  }

  // HP bar (above player, in world space)
  if (player.alive) {
    const barW = PLAYER_RADIUS * 3;
    const barH = 1.2;
    const barX = player.x - barW / 2;
    const barY = player.y - PLAYER_RADIUS - 3;
    const hpFrac = Math.max(0, Math.min(1, player.hp / MAX_HP));

    ctx.fillStyle = COLORS.healthBarBg;
    ctx.fillRect(barX, barY, barW, barH);

    ctx.fillStyle = hpFrac > 0.3 ? COLORS.hpBar : COLORS.hpBarLow;
    ctx.fillRect(barX, barY, barW * hpFrac, barH);
  }

  ctx.globalAlpha = 1.0;
}

/**
 * Draw HUD overlay in screen-space (scores, tick, ping).
 *
 * @param {CanvasRenderingContext2D} ctx
 * @param {import('./types.js').ArenaView} state
 * @param {number} cw
 * @param {number} ch
 */
function _drawHUD(ctx, state, cw, ch) {
  const pad = 12;

  ctx.font = '12px "JetBrains Mono", "Courier New", monospace';
  ctx.fillStyle = COLORS.text;

  // Tick counter (top-left)
  ctx.fillText(`TICK ${state.tick ?? 0}`, pad, pad + 12);

  // Self stats (top-right)
  if (state.self_state) {
    const self = state.self_state;
    const hpText = `HP ${Math.round(self.hp ?? 0)}`;
    const scoreText = `SCORE ${self.score ?? 0}`;
    const aliveText = self.alive ? '' : ' [DEAD]';

    ctx.textAlign = 'right';
    ctx.fillText(hpText + aliveText, cw - pad, pad + 12);
    ctx.fillText(scoreText, cw - pad, pad + 28);
    ctx.textAlign = 'left';
  }

  // Player count (bottom-left)
  const playerCount =
    (state.other_players ? state.other_players.length : 0) +
    (state.self_state ? 1 : 0);
  ctx.fillStyle = COLORS.textMuted;
  ctx.font = '11px "JetBrains Mono", "Courier New", monospace';
  ctx.fillText(`${playerCount} PLAYER${playerCount !== 1 ? 'S' : ''}`, pad, ch - pad);
}
