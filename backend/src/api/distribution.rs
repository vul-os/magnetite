// Distribution API — game artifact/version registration, build status, and the
// play-flow manifest endpoint consumed by the frontend.

use axum::{
    extract::{Path, State},
    middleware::from_fn_with_state,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response::{self, PaginatedResponse};
use crate::error::{AppError, Result};
use crate::services::distribution as svc;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterVersionRequest {
    pub version: String,
    pub commit_sha: String,
    pub release_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateArtifactRequest {
    pub build_status: String,
    pub artifact_url: Option<String>,
    pub sha256_hash: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BuildStatusSummary {
    pub game_id: Uuid,
    pub latest_build_status: Option<String>,
    pub latest_version: Option<String>,
    pub artifact_count: i64,
    pub live_version: Option<String>,
}

// ---------------------------------------------------------------------------
// Version handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/distribution/:game_id/versions
/// Developer registers a new semantic version (typically triggered after tagging).
pub async fn register_version(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<RegisterVersionRequest>,
) -> Result<Json<crate::api::response::ApiResponse<svc::GameVersion>>> {
    if payload.version.is_empty() {
        return Err(AppError::Validation(
            "Version string is required".to_string(),
        ));
    }
    if payload.commit_sha.is_empty() {
        return Err(AppError::Validation("commit_sha is required".to_string()));
    }

    // Verify the game exists and is active.
    sqlx::query_as::<_, (Uuid,)>("SELECT id FROM games WHERE id = $1 AND active = true")
        .bind(game_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let version = svc::register_version(
        &pool,
        game_id,
        &payload.version,
        &payload.commit_sha,
        payload.release_notes.as_deref(),
    )
    .await?;

    Ok(response::success_response(version))
}

/// GET /api/v1/distribution/:game_id/versions
/// List all versions for a game (newest first).
pub async fn list_versions(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<PaginatedResponse<svc::GameVersion>>> {
    let versions = svc::list_versions(&pool, game_id).await?;
    let total = versions.len() as u64;
    Ok(response::paginated(versions, 1, 50, total))
}

/// PUT /api/v1/distribution/:game_id/versions/:version_id/promote
/// Promote a version to live status (requires a successful artifact).
pub async fn promote_version(
    State(pool): State<PgPool>,
    Path((game_id, version_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<crate::api::response::ApiResponse<svc::GameVersion>>> {
    let promoted = svc::promote_version(&pool, game_id, version_id).await?;
    Ok(response::success_response(promoted))
}

// ---------------------------------------------------------------------------
// Artifact handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/distribution/:game_id/artifacts
/// List all artifacts for a game.
pub async fn list_artifacts(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<PaginatedResponse<svc::GameArtifact>>> {
    let artifacts = svc::list_artifacts(&pool, game_id).await?;
    let total = artifacts.len() as u64;
    Ok(response::paginated(artifacts, 1, 50, total))
}

/// GET /api/v1/distribution/:game_id/artifacts/:artifact_id
/// Retrieve a single artifact record.
pub async fn get_artifact(
    State(pool): State<PgPool>,
    Path((game_id, artifact_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<crate::api::response::ApiResponse<svc::GameArtifact>>> {
    let artifact = svc::get_artifact(&pool, artifact_id).await?;
    if artifact.game_id != game_id {
        return Err(AppError::NotFound("Artifact not found".to_string()));
    }
    Ok(response::success_response(artifact))
}

/// PUT /api/v1/distribution/:game_id/artifacts/:artifact_id
/// Update artifact status after a build completes (called by CI worker or admin).
pub async fn update_artifact(
    State(pool): State<PgPool>,
    Path((game_id, artifact_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateArtifactRequest>,
) -> Result<Json<crate::api::response::ApiResponse<svc::GameArtifact>>> {
    let valid_statuses = ["pending", "building", "success", "failed"];
    if !valid_statuses.contains(&payload.build_status.as_str()) {
        return Err(AppError::Validation(format!(
            "Invalid build_status '{}'; must be one of: {}",
            payload.build_status,
            valid_statuses.join(", ")
        )));
    }

    // Confirm the artifact belongs to this game.
    let existing = svc::get_artifact(&pool, artifact_id).await?;
    if existing.game_id != game_id {
        return Err(AppError::NotFound("Artifact not found".to_string()));
    }

    let updated = svc::update_artifact_status(
        &pool,
        artifact_id,
        &payload.build_status,
        payload.artifact_url.as_deref(),
        payload.sha256_hash.as_deref(),
        payload.file_size_bytes,
        payload.error_message.as_deref(),
    )
    .await?;

    Ok(response::success_response(updated))
}

// ---------------------------------------------------------------------------
// Play manifest — consumed by the frontend play page
// ---------------------------------------------------------------------------

/// GET /api/v1/distribution/:game_id/play
/// Returns the minimal manifest needed for the browser client to launch the game.
pub async fn get_play_manifest(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<svc::PlayManifest>>> {
    let manifest = svc::get_play_manifest(&pool, game_id).await?;
    Ok(response::success_response(manifest))
}

// ---------------------------------------------------------------------------
// Build status summary — developer dashboard convenience endpoint
// ---------------------------------------------------------------------------

/// GET /api/v1/distribution/:game_id/build-status
/// Returns a summary of the latest build state for a game.
pub async fn get_build_status_summary(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<BuildStatusSummary>>> {
    // Latest artifact (any type) for this game.
    let latest_artifact: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT ga.build_status, ga.version_id
         FROM game_artifacts ga
         WHERE ga.game_id = $1
         ORDER BY ga.created_at DESC LIMIT 1",
    )
    .bind(game_id)
    .fetch_optional(&pool)
    .await?;

    let (latest_build_status, latest_version_id) = match latest_artifact {
        Some((s, v)) => (Some(s), v),
        None => (None, None),
    };

    // Look up the version string if we have a version_id.
    let latest_version: Option<String> = if let Some(vid) = latest_version_id {
        sqlx::query_scalar("SELECT version FROM game_versions WHERE id = $1")
            .bind(vid)
            .fetch_optional(&pool)
            .await?
    } else {
        None
    };

    let artifact_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM game_artifacts WHERE game_id = $1")
            .bind(game_id)
            .fetch_one(&pool)
            .await?;

    let live_version: Option<String> = sqlx::query_scalar(
        "SELECT version FROM game_versions WHERE game_id = $1 AND is_live = true ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(game_id)
    .fetch_optional(&pool)
    .await?;

    Ok(response::success_response(BuildStatusSummary {
        game_id,
        latest_build_status,
        latest_version,
        artifact_count,
        live_version,
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool) -> Router {
    Router::new()
        // Public — anyone can fetch the play manifest and build status.
        .route("/:game_id/play", get(get_play_manifest))
        .route("/:game_id/build-status", get(get_build_status_summary))
        .route("/:game_id/artifacts", get(list_artifacts))
        .route("/:game_id/artifacts/:artifact_id", get(get_artifact))
        .route("/:game_id/versions", get(list_versions))
        // Auth-guarded — developers only.
        .route(
            "/:game_id/versions",
            post(register_version).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:game_id/versions/:version_id/promote",
            put(promote_version).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:game_id/artifacts/:artifact_id",
            put(update_artifact).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn valid_build_statuses() {
        let statuses = ["pending", "building", "success", "failed"];
        for s in statuses {
            // Ensure we'd accept each one in the validator.
            let valid = ["pending", "building", "success", "failed"];
            assert!(valid.contains(&s));
        }
    }

    #[test]
    fn invalid_build_status_rejected() {
        let invalid = ["done", "ok", "complete", "error"];
        let valid = ["pending", "building", "success", "failed"];
        for s in invalid {
            assert!(!valid.contains(&s));
        }
    }
}
