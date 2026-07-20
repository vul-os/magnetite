// Game Template Registry — GET /api/v1/templates
//
// Returns the list of available game templates that a developer can scaffold
// from. Templates are backed by the real on-disk crates under game-templates/
// (arcade, authoritative, fps, motorsport). Directory names match the catalog
// `id` values below.
//
// No auth required — templates are public catalog data.

use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::error::Result;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Graphics / engine complexity tier of a template.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsTier {
    /// 2-D sprites, minimal dependencies — jam / browser friendly.
    Lite2d,
    /// 3-D standard renderer, Bevy ECS + rapier3d.
    Standard3d,
    /// 3-D advanced: full Bevy + rapier3d physics, post-processing hooks.
    Advanced3d,
}

/// A single entry in the template catalog.
#[derive(Debug, Clone, Serialize)]
pub struct GameTemplate {
    /// Stable identifier used in POST /developer/games/scaffold.
    pub id: &'static str,
    /// Human-readable display name.
    pub name: &'static str,
    /// Feature tier (arcade / authoritative / fps / motorsport).
    pub tier: &'static str,
    /// Short marketing description.
    pub description: &'static str,
    /// Graphics / engine complexity.
    pub graphics_tier: GraphicsTier,
    /// On-disk crate path relative to the repo root (for CLI reference).
    pub template_path: &'static str,
    /// Canonical GitHub repo / path where the template lives.
    pub template_repo: &'static str,
    /// Optional preview image URL (relative to the CDN or null).
    pub preview_url: Option<&'static str>,
    /// Starter files included in the scaffold (informational file list).
    pub starter_files: &'static [&'static str],
}

// ---------------------------------------------------------------------------
// Static catalog — backed by real on-disk template crates
// ---------------------------------------------------------------------------

static TEMPLATES: &[GameTemplate] = &[
    GameTemplate {
        id: "arcade",
        name: "Arcade Starter",
        tier: "arcade",
        description: "A minimal 2-D arcade game with a simple game loop. Perfect for game jams \
                       and quick prototypes. No Bevy, no rapier — just pure Rust logic and the \
                       Magnetite SDK.",
        graphics_tier: GraphicsTier::Lite2d,
        template_path: "game-templates/arcade",
        template_repo: "magnetite/game-templates/arcade",
        preview_url: None,
        starter_files: &["Cargo.toml", "src/lib.rs", "src/game.rs", "README.md"],
    },
    GameTemplate {
        id: "authoritative",
        name: "Authoritative Arena Shooter",
        tier: "authoritative",
        description: "A top-down arena shooter implementing the full \
                       `AuthoritativeGame` trait: Snapshot/Delta/View/Command, \
                       deterministic tick loop, interest-filtered views (anti-wallhack), \
                       WASM ABI for the sandbox executor, and replay verification.",
        graphics_tier: GraphicsTier::Lite2d,
        template_path: "game-templates/authoritative",
        template_repo: "magnetite/game-templates/authoritative",
        preview_url: None,
        starter_files: &[
            "Cargo.toml",
            "src/lib.rs",
            "src/types.rs",
            "src/game.rs",
            "src/wasm_abi.rs",
            "README.md",
        ],
    },
    GameTemplate {
        id: "fps",
        name: "FPS Starter",
        tier: "fps",
        description: "A first-person shooter starter built on Bevy + rapier3d. Includes \
                       server-authoritative game logic, hitscan raycast helpers, gamepad \
                       input mapping, a static level, and an optional Bevy rendering client. \
                       Scales from browser WASM to dedicated server.",
        graphics_tier: GraphicsTier::Standard3d,
        template_path: "game-templates/fps",
        template_repo: "magnetite/game-templates/fps",
        preview_url: None,
        starter_files: &[
            "Cargo.toml",
            "src/lib.rs",
            "src/game.rs",
            "src/hitscan.rs",
            "src/level.rs",
            "src/input_map.rs",
            "README.md",
        ],
    },
    GameTemplate {
        id: "motorsport",
        name: "Motorsport / Racing Starter",
        tier: "motorsport",
        description: "A vehicle / racing starter (Circuit Rush) with a discrete \
                       raycast-suspension physics model, analog throttle/brake/steer from \
                       the gamepad layer, lap timing, platform score submission, and a \
                       Bevy ECS rendering client. Zero rapier dependency on the server path — \
                       `cargo check --no-default-features` stays fast.",
        graphics_tier: GraphicsTier::Advanced3d,
        template_path: "game-templates/motorsport",
        template_repo: "magnetite/game-templates/motorsport",
        preview_url: None,
        starter_files: &[
            "Cargo.toml",
            "src/lib.rs",
            "src/game.rs",
            "src/vehicle.rs",
            "src/track.rs",
            "src/lap_timer.rs",
            "src/input_map.rs",
            "README.md",
        ],
    },
];

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/templates
///
/// Returns the full template catalog. Public — no auth required.
pub async fn list_templates() -> Result<Json<Vec<GameTemplate>>> {
    Ok(Json(TEMPLATES.to_vec()))
}

/// GET /api/v1/templates/:id
///
/// Returns a single template by stable `id`. Returns 404 if not found.
pub async fn get_template(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<&'static GameTemplate>> {
    let tpl = TEMPLATES
        .iter()
        .find(|t| t.id == id.as_str())
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Template '{}' not found", id)))?;
    Ok(Json(tpl))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_templates))
        .route("/:id", get(get_template))
}

// ---------------------------------------------------------------------------
// Public helpers used by developer::scaffold_game
// ---------------------------------------------------------------------------

/// Return a template by its stable `id`, or `None`.
pub fn find_template(id: &str) -> Option<&'static GameTemplate> {
    TEMPLATES.iter().find(|t| t.id == id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_template_ids_unique() {
        let mut ids: Vec<&str> = TEMPLATES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), TEMPLATES.len(), "duplicate template id found");
    }

    #[test]
    fn find_by_valid_id() {
        assert!(find_template("arcade").is_some());
        assert!(find_template("authoritative").is_some());
        assert!(find_template("fps").is_some());
        assert!(find_template("motorsport").is_some());
    }

    #[test]
    fn find_by_invalid_id_returns_none() {
        assert!(find_template("nonexistent").is_none());
    }

    #[test]
    fn authoritative_has_wasm_abi_file() {
        let t = find_template("authoritative").unwrap();
        assert!(t.starter_files.contains(&"src/wasm_abi.rs"));
    }

    #[test]
    fn template_paths_nonempty() {
        for t in TEMPLATES {
            assert!(!t.template_path.is_empty(), "empty path for {}", t.id);
            assert!(!t.template_repo.is_empty(), "empty repo for {}", t.id);
        }
    }
}
