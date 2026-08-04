/**
 * Static template metadata for the Magnetite game template gallery.
 *
 * This file is the authoritative source for the gallery UI. The backend
 * may serve the same data from GET /api/v1/templates; this module is the
 * fallback used when that endpoint is unavailable (offline, cold start, etc.).
 *
 * Each template entry describes a scaffold tier that `magnetite new` and the
 * Web Studio both support. The `cliCommand` field shows the exact command a
 * developer would run; the `previewImage` field points into public/illustrations/.
 *
 * Tier ordering: Minimal → ArenaShooter → FpsStarter → MotorsportStarter
 * (ascending complexity / lines-of-code).
 */

export interface Template {
  /** Stable identifier used by the CLI and the Studio. */
  id: string;
  /** Display name shown in the gallery. */
  name: string;
  /** Complexity tier: "minimal" | "action" | "advanced". */
  tier: string;
  /** One-to-two sentence description for gallery cards. */
  blurb: string;
  /** Filterable tags. */
  tags: string[];
  /** Default max_players for MatchConfig.auto(). */
  maxPlayers: number;
  /** Default tick rate in Hz. */
  tickHz: number;
  /** Absolute path to a preview image under /public/. */
  previewImage: string;
  /** Exact `magnetite new` command to scaffold this template. */
  cliCommand: string;
  /** In-app link to the relevant developer guide. */
  docsUrl: string;
  /** Whether to show on the homepage template gallery. */
  featured: boolean;
}

export const templates: Template[] = [
  {
    id: "minimal",
    name: "Minimal",
    tier: "minimal",
    blurb:
      "A bare AuthoritativeGame stub — just the trait, the Wasm ABI, and a hello-world " +
      "tick counter. Start here when you want full creative control from line one.",
    tags: ["starter", "any-genre"],
    maxPlayers: 16,
    tickHz: 60,
    previewImage: "/illustrations/template-minimal.svg",
    cliCommand: "magnetite new my-game --template minimal",
    docsUrl: "/docs/moat/quickstart",
    featured: false,
  },
  {
    id: "arena-shooter",
    name: "Arena Shooter",
    tier: "action",
    blurb:
      "Top-down arena shooter — the canonical Magnetite reference game. Includes " +
      "player spawn/respawn, WASD movement clamped to arena bounds, mouse-aim, " +
      "projectile physics, hit detection, shoot cooldown, kill scoring, and full " +
      "Snapshot/Delta/View shapes. Anti-cheat validators and replay verification " +
      "are wired up out of the box.",
    tags: ["action", "shooter", "reference", "featured"],
    maxPlayers: 16,
    tickHz: 60,
    previewImage: "/illustrations/template-arena-shooter.svg",
    cliCommand: "magnetite new my-game --template arena-shooter",
    docsUrl: "/docs/moat/quickstart",
    featured: true,
  },
  {
    id: "fps-starter",
    name: "FPS Starter",
    tier: "advanced",
    blurb:
      "First-person movement and physics skeleton — character controller, " +
      "camera pitch/yaw, gravity, jumping, and a stub hitscan weapon. " +
      "Uses fixed-point position accumulation for cross-platform determinism. " +
      "Extend with rapier physics or your own collision layer.",
    tags: ["fps", "action", "3d", "advanced"],
    maxPlayers: 32,
    tickHz: 60,
    previewImage: "/illustrations/template-fps-starter.svg",
    cliCommand: "magnetite new my-game --template fps-starter",
    docsUrl: "/docs/for-developers/fps-starter",
    featured: true,
  },
  {
    id: "motorsport-starter",
    name: "Motorsport Starter",
    tier: "advanced",
    blurb:
      "Track racing template with a vehicle controller, lap counter, checkpoint " +
      "system, and a collision hull stub. Deterministic fixed-timestep physics " +
      "using integer velocities and a seeded track generator. Drop in your own " +
      "track geometry and tune the handling constants.",
    tags: ["racing", "3d", "physics", "advanced"],
    maxPlayers: 16,
    tickHz: 20,
    previewImage: "/illustrations/template-motorsport-starter.svg",
    cliCommand: "magnetite new my-game --template motorsport-starter",
    docsUrl: "/docs/for-developers/motorsport-starter",
    featured: true,
  },
];

/**
 * Return a single template by id, or undefined if not found.
 */
export function getTemplate(id: string): Template | undefined {
  return templates.find((t) => t.id === id);
}

/**
 * Return templates that are flagged as featured (shown on the homepage gallery).
 */
export function getFeaturedTemplates(): Template[] {
  return templates.filter((t) => t.featured);
}

/**
 * Return templates filtered by tag.
 */
export function getTemplatesByTag(tag: string): Template[] {
  return templates.filter((t) => t.tags.includes(tag));
}
