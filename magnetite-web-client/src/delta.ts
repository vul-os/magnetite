/**
 * magnetite-web-client/src/delta.js
 *
 * Apply an ArenaDelta onto an ArenaView/ArenaSnapshot state.
 *
 * ArenaDelta shape (mirrors game-templates/authoritative/src/types.rs):
 *   {
 *     changed_players: ShooterPlayer[],
 *     removed_projectile_ids: u64[],
 *     new_projectiles: Projectile[],
 *   }
 *
 * ArenaSnapshot shape:
 *   { players: ShooterPlayer[], projectiles: Projectile[], tick: u64 }
 *
 * ArenaView shape:
 *   { self_state: ShooterPlayer | null, other_players: ShooterPlayer[],
 *     projectiles: Projectile[], tick: u64 }
 *
 * Both carry the same projectile/player list structure; this module works
 * on a normalised form that matches both.
 */

// ---------------------------------------------------------------------------
// Apply a delta onto an ArenaSnapshot
// ---------------------------------------------------------------------------

/**
 * Apply a decoded ArenaDelta to an ArenaSnapshot.
 * Returns a new snapshot (does not mutate the input).
 *
 * @param {import('./types.js').ArenaSnapshot} snapshot
 * @param {import('./types.js').ArenaDelta}    delta
 * @param {number}                             tick  - new authoritative tick
 * @returns {import('./types.js').ArenaSnapshot}
 */
export function applyDeltaToSnapshot(snapshot, delta, tick) {
  if (!snapshot) return snapshot;

  // Build a mutable players map (keyed by player id string)
  const playersById = new Map(
    (snapshot.players || []).map(p => [String(p.id), { ...p }])
  );

  // Upsert changed players
  for (const changed of delta.changed_players || []) {
    playersById.set(String(changed.id), { ...changed });
  }

  // Build a mutable projectiles map (keyed by projectile id string)
  const projectilesById = new Map(
    (snapshot.projectiles || []).map(p => [String(p.id), { ...p }])
  );

  // Remove expired/hit projectiles
  for (const removedId of delta.removed_projectile_ids || []) {
    projectilesById.delete(String(removedId));
  }

  // Add new projectiles
  for (const proj of delta.new_projectiles || []) {
    projectilesById.set(String(proj.id), { ...proj });
  }

  return {
    players: Array.from(playersById.values()),
    projectiles: Array.from(projectilesById.values()),
    tick,
  };
}

// ---------------------------------------------------------------------------
// Convert a snapshot into an ArenaView for a specific player
// ---------------------------------------------------------------------------

/**
 * Build an ArenaView from a full ArenaSnapshot, filtered for a given player.
 *
 * This mirrors AuthoritativeGame::view_for on the client side and is used
 * to render the game after a Snapshot message arrives (the Delta path sends
 * the view directly).
 *
 * @param {import('./types.js').ArenaSnapshot} snapshot
 * @param {string | number | null} playerId  - local player's id
 * @returns {import('./types.js').ArenaView}
 */
export function snapshotToView(snapshot, playerId) {
  if (!snapshot) {
    return { self_state: null, other_players: [], projectiles: [], tick: 0 };
  }

  const pidStr = playerId !== null && playerId !== undefined ? String(playerId) : null;
  const players = snapshot.players || [];

  const self_state = pidStr !== null
    ? (players.find(p => String(p.id) === pidStr) || null)
    : null;

  const other_players = pidStr !== null
    ? players.filter(p => String(p.id) !== pidStr)
    : players;

  return {
    self_state: self_state ? { ...self_state } : null,
    other_players: other_players.map(p => ({ ...p })),
    projectiles: (snapshot.projectiles || []).map(p => ({ ...p })),
    tick: snapshot.tick || 0,
  };
}
