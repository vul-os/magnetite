// Provisioning API — request, list, and stop magnetite-runtime instances.
//
// These endpoints connect the distribution pipeline (registered wasm artifacts)
// to the runtime layer.  A developer or the CLI `magnetite deploy` subcommand
// calls POST /:game_id/instances to spin up a server for a match; the response
// contains the ws_endpoint clients use to connect.
//
// Route summary
// ─────────────
//   POST   /api/v1/provisioning/:game_id/instances          — request a new instance
//   GET    /api/v1/provisioning/:game_id/instances          — list instances for game
//   GET    /api/v1/provisioning/:game_id/instances/:id      — single instance
//   PATCH  /api/v1/provisioning/:game_id/instances/:id      — runner reports status/endpoint
//   DELETE /api/v1/provisioning/:game_id/instances/:id      — stop an instance
//   GET    /api/v1/provisioning/pending                     — runner poll: pending instances

use axum::{
    extract::{Path, State},
    middleware::from_fn_with_state,
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response::{self, PaginatedResponse};
use crate::error::{AppError, Result};
use crate::services::provisioning as svc;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ProvisionRequest {
    /// Optional: pin to a specific artifact (must be build_status = 'success').
    pub artifact_id: Option<Uuid>,
    /// Optional: version UUID; informational, resolved from artifact if omitted.
    pub version_id: Option<Uuid>,
    /// Topology hint: "SingleRoom" | "Dedicated" | "Sharded"
    #[serde(default = "default_topology")]
    pub topology: String,
    #[serde(default = "default_max_players")]
    pub max_players: i32,
    #[serde(default = "default_tick_hz")]
    pub tick_hz: i32,
}

fn default_topology() -> String {
    "SingleRoom".to_string()
}
fn default_max_players() -> i32 {
    4
}
fn default_tick_hz() -> i32 {
    20
}

/// Subset of RuntimeInstance returned to unauthenticated callers (omits internal fields).
#[derive(Debug, Serialize)]
pub struct InstanceSummary {
    pub id: Uuid,
    pub game_id: Uuid,
    pub status: String,
    /// WebSocket URL to connect to; null while status = 'pending'.
    pub ws_endpoint: Option<String>,
    pub topology: String,
    pub max_players: i32,
    pub tick_hz: i32,
}

impl From<svc::RuntimeInstance> for InstanceSummary {
    fn from(i: svc::RuntimeInstance) -> Self {
        InstanceSummary {
            id: i.id,
            game_id: i.game_id,
            status: i.status,
            ws_endpoint: i.ws_endpoint,
            topology: i.topology,
            max_players: i.max_players,
            tick_hz: i.tick_hz,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RunnerPatchRequest {
    /// New status reported by the runner.
    pub status: String,
    /// ws_endpoint once the runtime has bound its listener.
    pub ws_endpoint: Option<String>,
    /// Free-form note (version, host info, error reason, …).
    pub runner_note: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/provisioning/:game_id/instances
/// Request a new runtime instance for a game.
pub async fn request_instance(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<ProvisionRequest>,
) -> Result<Json<crate::api::response::ApiResponse<svc::RuntimeInstance>>> {
    let req = svc::ProvisionRequest {
        game_id,
        version_id: payload.version_id,
        artifact_id: payload.artifact_id,
        topology: payload.topology,
        max_players: payload.max_players,
        tick_hz: payload.tick_hz,
        requested_by: None, // TODO: extract from auth claims (N3)
    };

    let instance = svc::provision_instance(&pool, req).await?;
    Ok(response::success_response(instance))
}

/// GET /api/v1/provisioning/:game_id/instances
/// List all instances for a game (public — clients can poll for a running endpoint).
pub async fn list_instances(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<PaginatedResponse<InstanceSummary>>> {
    let instances = svc::list_instances(&pool, game_id).await?;
    let total = instances.len() as u64;
    let summaries: Vec<InstanceSummary> = instances.into_iter().map(Into::into).collect();
    Ok(response::paginated(summaries, 1, 50, total))
}

/// GET /api/v1/provisioning/:game_id/instances/:instance_id
/// Fetch a single instance (public — used by the play flow to poll ws_endpoint).
pub async fn get_instance(
    State(pool): State<PgPool>,
    Path((game_id, instance_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<crate::api::response::ApiResponse<InstanceSummary>>> {
    let instance = svc::get_instance(&pool, instance_id).await?;
    if instance.game_id != game_id {
        return Err(AppError::NotFound("Instance not found".to_string()));
    }
    Ok(response::success_response(InstanceSummary::from(instance)))
}

/// PATCH /api/v1/provisioning/:game_id/instances/:instance_id
/// Runner seam: update instance status + ws_endpoint once the runtime is up.
/// Auth-guarded (runner uses a developer bearer token or a dedicated runner token
/// — full token scoping is Bucket-D; for now any authenticated user can call this).
pub async fn patch_instance(
    State(pool): State<PgPool>,
    Path((game_id, instance_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<RunnerPatchRequest>,
) -> Result<Json<crate::api::response::ApiResponse<svc::RuntimeInstance>>> {
    // Verify ownership.
    let existing = svc::get_instance(&pool, instance_id).await?;
    if existing.game_id != game_id {
        return Err(AppError::NotFound("Instance not found".to_string()));
    }

    let updated = svc::update_instance_status(
        &pool,
        instance_id,
        &payload.status,
        payload.ws_endpoint.as_deref(),
        payload.runner_note.as_deref(),
    )
    .await?;

    Ok(response::success_response(updated))
}

/// DELETE /api/v1/provisioning/:game_id/instances/:instance_id
/// Stop a running instance (auth-guarded).
pub async fn stop_instance(
    State(pool): State<PgPool>,
    Path((game_id, instance_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<crate::api::response::ApiResponse<svc::RuntimeInstance>>> {
    let existing = svc::get_instance(&pool, instance_id).await?;
    if existing.game_id != game_id {
        return Err(AppError::NotFound("Instance not found".to_string()));
    }

    let stopped = svc::stop_instance(&pool, instance_id).await?;
    Ok(response::success_response(stopped))
}

/// GET /api/v1/provisioning/pending
/// Runner-facing poll endpoint: returns all pending instances.
/// Auth-guarded.
pub async fn list_pending(
    State(pool): State<PgPool>,
) -> Result<Json<PaginatedResponse<svc::RuntimeInstance>>> {
    let instances = svc::list_pending_instances(&pool).await?;
    let total = instances.len() as u64;
    Ok(response::paginated(instances, 1, 100, total))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool) -> Router {
    Router::new()
        // Runner poll (auth-guarded) — no :game_id prefix.
        .route(
            "/pending",
            get(list_pending).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // Public — clients poll for ws_endpoint after requesting an instance.
        .route("/:game_id/instances", get(list_instances))
        .route("/:game_id/instances/:instance_id", get(get_instance))
        // Auth-guarded — developers / CLI / runner.
        .route(
            "/:game_id/instances",
            post(request_instance).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:game_id/instances/:instance_id",
            patch(patch_instance).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:game_id/instances/:instance_id",
            delete(stop_instance).layer(from_fn_with_state(
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
    use super::*;

    #[test]
    fn default_provision_fields() {
        assert_eq!(default_topology(), "SingleRoom");
        assert_eq!(default_max_players(), 4);
        assert_eq!(default_tick_hz(), 20);
    }

    #[test]
    fn instance_summary_from_runtime_instance() {
        let inst = svc::RuntimeInstance {
            id: Uuid::nil(),
            game_id: Uuid::nil(),
            version_id: None,
            artifact_id: None,
            status: "running".to_string(),
            ws_endpoint: Some("ws://127.0.0.1:9000".to_string()),
            topology: "SingleRoom".to_string(),
            max_players: 4,
            tick_hz: 20,
            local_pid: None,
            runner_note: None,
            requested_by: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let summary = InstanceSummary::from(inst);
        assert_eq!(summary.status, "running");
        assert_eq!(summary.ws_endpoint.as_deref(), Some("ws://127.0.0.1:9000"));
    }
}
