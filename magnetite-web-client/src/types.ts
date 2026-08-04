/**
 * magnetite-web-client/src/types.ts
 *
 * Shared wire/domain types for the web client. Referenced throughout the
 * original JSDoc as `import('./types.js')` even though that file never
 * existed — this file is the real thing those comments always pointed at.
 *
 * Mirrors magnetite_sdk::input and game-templates/authoritative/src/types.rs.
 */

/** Mirrors magnetite_sdk::input::KeyState — all boolean key states. */
export interface KeyState {
  forward: boolean;
  backward: boolean;
  left: boolean;
  right: boolean;
  jump: boolean;
  crouch: boolean;
  attack: boolean;
  secondary_attack: boolean;
  interact: boolean;
  sprint: boolean;
}

/** Mirrors magnetite_sdk::input::MouseState. */
export interface MouseState {
  x: number;
  y: number;
  delta_x: number;
  delta_y: number;
  left_button: boolean;
  right_button: boolean;
  middle_button: boolean;
  scroll: number;
}

/** Mirrors magnetite_sdk::input::Input. */
export interface Input {
  keys: KeyState;
  mouse: MouseState;
  sequence: number;
  timestamp_ms: number;
}

/** Mirrors game-templates/authoritative/src/types.rs::ShooterPlayer. */
export interface ShooterPlayer {
  id: string | number;
  x: number;
  y: number;
  angle: number;
  hp: number;
  alive: boolean;
  last_shot_tick: number;
  score: number;
  [key: string]: unknown;
}

/** Mirrors game-templates/authoritative/src/types.rs::Projectile. */
export interface Projectile {
  id: string | number;
  owner: string | number;
  x: number;
  y: number;
  vx: number;
  vy: number;
  ticks_left: number;
  [key: string]: unknown;
}

/** Full authoritative arena state. */
export interface ArenaSnapshot {
  players: ShooterPlayer[];
  projectiles: Projectile[];
  tick: number;
}

/** Per-player view of the arena, as sent after a Delta or derived from a Snapshot. */
export interface ArenaView {
  self_state: ShooterPlayer | null;
  other_players: ShooterPlayer[];
  projectiles: Projectile[];
  tick: number;
}

/** Mirrors game-templates/authoritative/src/types.rs::ArenaDelta. */
export interface ArenaDelta {
  changed_players: ShooterPlayer[];
  removed_projectile_ids: (string | number)[];
  new_projectiles: Projectile[];
}

/** MatchConfig, as sent in ServerNet::Welcome. Shape varies per game template. */
export interface MatchConfig {
  tick_hz?: number;
  max_players?: number;
  seed?: number;
  topology?: unknown;
  snapshot_every?: number;
  [key: string]: unknown;
}

/** A generic parsed ServerNet frame — `type` is the discriminant. */
export interface ServerMessage {
  type: string;
  [key: string]: unknown;
}
