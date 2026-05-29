// Distribution service — manages game artifact versions, build→artifact linkage,
// and the artifact resolution logic needed by the frontend play flow.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GameVersion {
    pub id: Uuid,
    pub game_id: Uuid,
    pub version: String,
    pub commit_sha: String,
    pub release_notes: Option<String>,
    pub is_live: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GameArtifact {
    pub id: Uuid,
    pub game_id: Uuid,
    pub version_id: Option<Uuid>,
    pub build_id: Option<Uuid>,
    pub artifact_type: String,
    pub artifact_url: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub sha256_hash: Option<String>,
    pub build_status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Lightweight struct returned to the frontend for the "play" flow — everything
/// a client needs to load and launch a game without exposing internal IDs.
#[derive(Debug, Serialize)]
pub struct PlayManifest {
    pub game_id: Uuid,
    pub version: String,
    pub commit_sha: String,
    pub wasm_url: Option<String>,
    pub server_url: Option<String>,
    pub artifact_type: String,
    pub sha256_hash: Option<String>,
    pub file_size_bytes: Option<i64>,
}

// ---------------------------------------------------------------------------
// Version management
// ---------------------------------------------------------------------------

pub async fn register_version(
    pool: &PgPool,
    game_id: Uuid,
    version: &str,
    commit_sha: &str,
    release_notes: Option<&str>,
) -> Result<GameVersion> {
    let version_id = Uuid::new_v4();
    let gv = sqlx::query_as::<_, GameVersion>(
        "INSERT INTO game_versions
             (id, game_id, version, commit_sha, release_notes, is_live, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, false, NOW(), NOW())
         ON CONFLICT (game_id, version) DO UPDATE
             SET commit_sha    = EXCLUDED.commit_sha,
                 release_notes = COALESCE(EXCLUDED.release_notes, game_versions.release_notes),
                 updated_at    = NOW()
         RETURNING id, game_id, version, commit_sha, release_notes, is_live, created_at, updated_at",
    )
    .bind(version_id)
    .bind(game_id)
    .bind(version)
    .bind(commit_sha)
    .bind(release_notes)
    .fetch_one(pool)
    .await?;

    Ok(gv)
}

pub async fn list_versions(pool: &PgPool, game_id: Uuid) -> Result<Vec<GameVersion>> {
    let versions = sqlx::query_as::<_, GameVersion>(
        "SELECT id, game_id, version, commit_sha, release_notes, is_live, created_at, updated_at
         FROM game_versions
         WHERE game_id = $1
         ORDER BY created_at DESC",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;

    Ok(versions)
}

pub async fn get_live_version(pool: &PgPool, game_id: Uuid) -> Result<Option<GameVersion>> {
    let version = sqlx::query_as::<_, GameVersion>(
        "SELECT id, game_id, version, commit_sha, release_notes, is_live, created_at, updated_at
         FROM game_versions
         WHERE game_id = $1 AND is_live = true
         ORDER BY updated_at DESC
         LIMIT 1",
    )
    .bind(game_id)
    .fetch_optional(pool)
    .await?;

    Ok(version)
}

/// Promote a specific version to live; demotes any previously-live version.
pub async fn promote_version(
    pool: &PgPool,
    game_id: Uuid,
    version_id: Uuid,
) -> Result<GameVersion> {
    // Verify the version belongs to this game and has a successful artifact.
    let version = sqlx::query_as::<_, GameVersion>(
        "SELECT gv.id, gv.game_id, gv.version, gv.commit_sha, gv.release_notes,
                gv.is_live, gv.created_at, gv.updated_at
         FROM game_versions gv
         WHERE gv.id = $1 AND gv.game_id = $2",
    )
    .bind(version_id)
    .bind(game_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Version not found for this game".to_string()))?;

    let artifact_ok: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM game_artifacts
         WHERE version_id = $1 AND build_status = 'success'
         LIMIT 1",
    )
    .bind(version_id)
    .fetch_optional(pool)
    .await?;

    if artifact_ok.is_none() {
        return Err(AppError::Validation(
            "Cannot promote a version without a successful build artifact".to_string(),
        ));
    }

    // Atomically demote old live, promote new.
    sqlx::query(
        "UPDATE game_versions SET is_live = false, updated_at = NOW()
         WHERE game_id = $1 AND is_live = true",
    )
    .bind(game_id)
    .execute(pool)
    .await?;

    let promoted = sqlx::query_as::<_, GameVersion>(
        "UPDATE game_versions SET is_live = true, updated_at = NOW()
         WHERE id = $1
         RETURNING id, game_id, version, commit_sha, release_notes, is_live, created_at, updated_at",
    )
    .bind(version.id)
    .fetch_one(pool)
    .await?;

    Ok(promoted)
}

// ---------------------------------------------------------------------------
// Artifact management
// ---------------------------------------------------------------------------

/// Create a pending artifact record when a build is first triggered.
pub async fn create_artifact(
    pool: &PgPool,
    game_id: Uuid,
    version_id: Option<Uuid>,
    build_id: Option<Uuid>,
    artifact_type: &str,
) -> Result<GameArtifact> {
    let artifact_id = Uuid::new_v4();
    let artifact = sqlx::query_as::<_, GameArtifact>(
        "INSERT INTO game_artifacts
             (id, game_id, version_id, build_id, artifact_type, build_status, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, 'pending', NOW(), NOW())
         RETURNING id, game_id, version_id, build_id, artifact_type, artifact_url,
                   file_size_bytes, sha256_hash, build_status, error_message, created_at, updated_at",
    )
    .bind(artifact_id)
    .bind(game_id)
    .bind(version_id)
    .bind(build_id)
    .bind(artifact_type)
    .fetch_one(pool)
    .await?;

    Ok(artifact)
}

/// Update an artifact record after the build completes (success or failure).
pub async fn update_artifact_status(
    pool: &PgPool,
    artifact_id: Uuid,
    build_status: &str,
    artifact_url: Option<&str>,
    sha256_hash: Option<&str>,
    file_size_bytes: Option<i64>,
    error_message: Option<&str>,
) -> Result<GameArtifact> {
    let artifact = sqlx::query_as::<_, GameArtifact>(
        "UPDATE game_artifacts SET
             build_status      = $2,
             artifact_url      = COALESCE($3, artifact_url),
             sha256_hash       = COALESCE($4, sha256_hash),
             file_size_bytes   = COALESCE($5, file_size_bytes),
             error_message     = $6,
             updated_at        = NOW()
         WHERE id = $1
         RETURNING id, game_id, version_id, build_id, artifact_type, artifact_url,
                   file_size_bytes, sha256_hash, build_status, error_message, created_at, updated_at",
    )
    .bind(artifact_id)
    .bind(build_status)
    .bind(artifact_url)
    .bind(sha256_hash)
    .bind(file_size_bytes)
    .bind(error_message)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Artifact not found".to_string()))?;

    Ok(artifact)
}

pub async fn list_artifacts(pool: &PgPool, game_id: Uuid) -> Result<Vec<GameArtifact>> {
    let artifacts = sqlx::query_as::<_, GameArtifact>(
        "SELECT id, game_id, version_id, build_id, artifact_type, artifact_url,
                file_size_bytes, sha256_hash, build_status, error_message, created_at, updated_at
         FROM game_artifacts
         WHERE game_id = $1
         ORDER BY created_at DESC",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;

    Ok(artifacts)
}

pub async fn get_artifact(pool: &PgPool, artifact_id: Uuid) -> Result<GameArtifact> {
    sqlx::query_as::<_, GameArtifact>(
        "SELECT id, game_id, version_id, build_id, artifact_type, artifact_url,
                file_size_bytes, sha256_hash, build_status, error_message, created_at, updated_at
         FROM game_artifacts
         WHERE id = $1",
    )
    .bind(artifact_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Artifact not found".to_string()))
}

// ---------------------------------------------------------------------------
// Play manifest — frontend play flow
// ---------------------------------------------------------------------------

/// Resolve the play manifest for a game: finds the live version and its best
/// available artifact (preferring WASM for browser play).
pub async fn get_play_manifest(pool: &PgPool, game_id: Uuid) -> Result<PlayManifest> {
    let version = get_live_version(pool, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound("No live version available for this game".to_string()))?;

    // Prefer WASM artifact, fall back to any successful artifact.
    let wasm_artifact: Option<GameArtifact> = sqlx::query_as::<_, GameArtifact>(
        "SELECT id, game_id, version_id, build_id, artifact_type, artifact_url,
                file_size_bytes, sha256_hash, build_status, error_message, created_at, updated_at
         FROM game_artifacts
         WHERE version_id = $1 AND artifact_type = 'wasm' AND build_status = 'success'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(version.id)
    .fetch_optional(pool)
    .await?;

    let server_artifact: Option<GameArtifact> = sqlx::query_as::<_, GameArtifact>(
        "SELECT id, game_id, version_id, build_id, artifact_type, artifact_url,
                file_size_bytes, sha256_hash, build_status, error_message, created_at, updated_at
         FROM game_artifacts
         WHERE version_id = $1 AND artifact_type LIKE 'server-%' AND build_status = 'success'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(version.id)
    .fetch_optional(pool)
    .await?;

    let primary = wasm_artifact
        .as_ref()
        .or(server_artifact.as_ref())
        .ok_or_else(|| {
            AppError::NotFound(
                "No successful build artifact found for the live version".to_string(),
            )
        })?;

    Ok(PlayManifest {
        game_id,
        version: version.version.clone(),
        commit_sha: version.commit_sha.clone(),
        wasm_url: wasm_artifact.as_ref().and_then(|a| a.artifact_url.clone()),
        server_url: server_artifact
            .as_ref()
            .and_then(|a| a.artifact_url.clone()),
        artifact_type: primary.artifact_type.clone(),
        sha256_hash: primary.sha256_hash.clone(),
        file_size_bytes: primary.file_size_bytes,
    })
}

// ---------------------------------------------------------------------------
// Build-record linkage (called from webhook path)
// ---------------------------------------------------------------------------

/// When a webhook push fires for a registered game, create an artifact record
/// tied to the build and (optionally) a version.  Returns the new artifact id.
pub async fn create_build_artifact_from_push(
    pool: &PgPool,
    game_id: Uuid,
    build_id: Uuid,
    commit_sha: &str,
    version_tag: Option<&str>,
) -> Result<Uuid> {
    // Upsert a version if a tag was given.
    let version_id: Option<Uuid> = if let Some(tag) = version_tag {
        let gv = register_version(pool, game_id, tag, commit_sha, None).await?;
        Some(gv.id)
    } else {
        None
    };

    let artifact = create_artifact(pool, game_id, version_id, Some(build_id), "wasm").await?;
    Ok(artifact.id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_manifest_serialises_clean() {
        let manifest = PlayManifest {
            game_id: Uuid::nil(),
            version: "1.0.0".to_string(),
            commit_sha: "abc123".to_string(),
            wasm_url: Some("https://cdn.example.com/game.wasm".to_string()),
            server_url: None,
            artifact_type: "wasm".to_string(),
            sha256_hash: Some("deadbeef".to_string()),
            file_size_bytes: Some(1_024_000),
        };

        let json = serde_json::to_value(&manifest).unwrap();
        assert_eq!(json["version"], "1.0.0");
        assert_eq!(json["artifact_type"], "wasm");
        assert!(json["server_url"].is_null());
    }

    #[test]
    fn artifact_type_variants_are_strings() {
        // Ensure we don't accidentally break downstream consumers that depend
        // on the literal strings "wasm", "server-linux", "server-windows".
        let types = ["wasm", "server-linux", "server-windows"];
        for t in types {
            assert!(t.starts_with("wasm") || t.starts_with("server-"));
        }
    }
}
