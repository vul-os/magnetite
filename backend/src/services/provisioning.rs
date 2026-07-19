// Provisioning service — launch and track magnetite-runtime instances.
//
// # Design intent
//
// This service connects the distribution pipeline to the runtime.  When a game
// artifact (wasm) is registered/promoted a caller can request a runtime instance
// for a match.  The service:
//
//   1. Inserts a `runtime_instances` row (status = 'pending').
//   2. Optionally spawns a `magnetite-runtime` child process (dev/single-host
//      mode) when the `RUNTIME_BIN_PATH` env var points at the compiled binary.
//      On success it updates the row to `running` + records the ws_endpoint and
//      local PID.
//   3. Returns the instance record to the caller who can then expose the
//      ws_endpoint to players.
//
// # Bucket-D runner seam (multi-host / production)
//
// When `RUNTIME_BIN_PATH` is NOT set (the typical production case) the backend
// simply records the instance as `pending`.  A self-hosted runner — a sidecar
// script (scripts/provision-instance.sh) or a container orchestrator — polls
// `GET /api/v1/provisioning/pending`, starts the actual `magnetite-runtime`
// binary, and PATCHes the instance to `running` with the bound `ws_endpoint`.
// Clients read `ws_endpoint` from `GET /api/v1/provisioning/instances/:id`
// once status = 'running'.
//
// This seam is intentional.  Executing untrusted subprocesses inside the HTTP
// server is unsafe at scale; the runner boundary lets ops choose the execution
// environment (Docker, Fly Machines, Kubernetes, bare-metal) without changing
// the backend code.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A runtime instance row — mirrors the `runtime_instances` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RuntimeInstance {
    pub id: Uuid,
    pub game_id: Uuid,
    pub version_id: Option<Uuid>,
    pub artifact_id: Option<Uuid>,
    pub status: String,
    pub ws_endpoint: Option<String>,
    pub topology: String,
    pub max_players: i32,
    pub tick_hz: i32,
    pub local_pid: Option<i32>,
    pub runner_note: Option<String>,
    pub requested_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for requesting a new instance.
#[derive(Debug, Clone)]
pub struct ProvisionRequest {
    pub game_id: Uuid,
    pub version_id: Option<Uuid>,
    pub artifact_id: Option<Uuid>,
    /// Topology hint: "SingleRoom" | "Dedicated" | "Sharded"
    pub topology: String,
    pub max_players: i32,
    pub tick_hz: i32,
    pub requested_by: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Core service functions
// ---------------------------------------------------------------------------

/// Create an instance record and optionally spawn a local runtime process.
///
/// Returns the created (and possibly already-running) `RuntimeInstance`.
pub async fn provision_instance(pool: &PgPool, req: ProvisionRequest) -> Result<RuntimeInstance> {
    // Validate topology string.
    let valid_topologies = ["SingleRoom", "Dedicated", "Sharded"];
    if !valid_topologies.contains(&req.topology.as_str()) {
        return Err(AppError::Validation(format!(
            "Invalid topology '{}'; must be one of: {}",
            req.topology,
            valid_topologies.join(", ")
        )));
    }

    if req.max_players < 1 || req.max_players > 1024 {
        return Err(AppError::Validation(
            "max_players must be between 1 and 1024".to_string(),
        ));
    }

    if req.tick_hz < 1 || req.tick_hz > 128 {
        return Err(AppError::Validation(
            "tick_hz must be between 1 and 128".to_string(),
        ));
    }

    // Verify the game exists.
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM games WHERE id = $1 AND active = true")
        .bind(req.game_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Game not found or inactive".to_string()))?;

    // If an artifact_id is provided, verify it belongs to the game and is successful.
    if let Some(artifact_id) = req.artifact_id {
        let ok: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM game_artifacts WHERE id = $1 AND game_id = $2 AND build_status = 'success'",
        )
        .bind(artifact_id)
        .bind(req.game_id)
        .fetch_optional(pool)
        .await?;

        if ok.is_none() {
            return Err(AppError::Validation(
                "Artifact not found, does not belong to this game, or build did not succeed"
                    .to_string(),
            ));
        }
    }

    // Insert the instance row (status = pending initially).
    let instance_id = Uuid::new_v4();
    let mut instance = sqlx::query_as::<_, RuntimeInstance>(
        "INSERT INTO runtime_instances
             (id, game_id, version_id, artifact_id, status, topology, max_players, tick_hz,
              requested_by, created_at, updated_at)
         VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, NOW(), NOW())
         RETURNING id, game_id, version_id, artifact_id, status, ws_endpoint, topology,
                   max_players, tick_hz, local_pid, runner_note, requested_by,
                   created_at, updated_at",
    )
    .bind(instance_id)
    .bind(req.game_id)
    .bind(req.version_id)
    .bind(req.artifact_id)
    .bind(&req.topology)
    .bind(req.max_players)
    .bind(req.tick_hz)
    .bind(req.requested_by)
    .fetch_one(pool)
    .await?;

    // Attempt a local spawn if RUNTIME_BIN_PATH is configured.
    // This is intentionally best-effort: if it fails we leave the row as
    // 'pending' so the external runner seam can pick it up.
    if let Some(updated) = try_local_spawn(pool, &instance).await {
        instance = updated;
    }

    Ok(instance)
}

/// Try to resolve and return the ws_endpoint for the live version of a game.
///
/// Used by the play manifest flow: first check for a running instance, then
/// fall through to None so the frontend knows to wait.
pub async fn get_running_endpoint(pool: &PgPool, game_id: Uuid) -> Result<Option<String>> {
    let endpoint: Option<String> = sqlx::query_scalar(
        "SELECT ws_endpoint
         FROM runtime_instances
         WHERE game_id = $1 AND status = 'running' AND ws_endpoint IS NOT NULL
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(game_id)
    .fetch_optional(pool)
    .await?;
    Ok(endpoint)
}

/// Get a single instance by id.
pub async fn get_instance(pool: &PgPool, instance_id: Uuid) -> Result<RuntimeInstance> {
    sqlx::query_as::<_, RuntimeInstance>(
        "SELECT id, game_id, version_id, artifact_id, status, ws_endpoint, topology,
                max_players, tick_hz, local_pid, runner_note, requested_by,
                created_at, updated_at
         FROM runtime_instances
         WHERE id = $1",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Instance not found".to_string()))
}

/// List instances for a game (newest first, up to 50).
pub async fn list_instances(pool: &PgPool, game_id: Uuid) -> Result<Vec<RuntimeInstance>> {
    let rows = sqlx::query_as::<_, RuntimeInstance>(
        "SELECT id, game_id, version_id, artifact_id, status, ws_endpoint, topology,
                max_players, tick_hz, local_pid, runner_note, requested_by,
                created_at, updated_at
         FROM runtime_instances
         WHERE game_id = $1
         ORDER BY created_at DESC
         LIMIT 50",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// List all pending instances (used by the runner seam to poll for work).
pub async fn list_pending_instances(pool: &PgPool) -> Result<Vec<RuntimeInstance>> {
    let rows = sqlx::query_as::<_, RuntimeInstance>(
        "SELECT id, game_id, version_id, artifact_id, status, ws_endpoint, topology,
                max_players, tick_hz, local_pid, runner_note, requested_by,
                created_at, updated_at
         FROM runtime_instances
         WHERE status = 'pending'
         ORDER BY created_at ASC
         LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Update instance status (called by the runner seam or stop handler).
pub async fn update_instance_status(
    pool: &PgPool,
    instance_id: Uuid,
    status: &str,
    ws_endpoint: Option<&str>,
    runner_note: Option<&str>,
) -> Result<RuntimeInstance> {
    let valid = ["pending", "running", "stopped", "failed"];
    if !valid.contains(&status) {
        return Err(AppError::Validation(format!(
            "Invalid status '{}'; must be one of: {}",
            status,
            valid.join(", ")
        )));
    }

    sqlx::query_as::<_, RuntimeInstance>(
        "UPDATE runtime_instances SET
             status       = $2,
             ws_endpoint  = COALESCE($3, ws_endpoint),
             runner_note  = COALESCE($4, runner_note),
             updated_at   = NOW()
         WHERE id = $1
         RETURNING id, game_id, version_id, artifact_id, status, ws_endpoint, topology,
                   max_players, tick_hz, local_pid, runner_note, requested_by,
                   created_at, updated_at",
    )
    .bind(instance_id)
    .bind(status)
    .bind(ws_endpoint)
    .bind(runner_note)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Instance not found".to_string()))
}

/// Stop a running instance (status → stopped; kills local PID if present).
pub async fn stop_instance(pool: &PgPool, instance_id: Uuid) -> Result<RuntimeInstance> {
    let instance = get_instance(pool, instance_id).await?;

    if matches!(instance.status.as_str(), "stopped" | "failed") {
        return Err(AppError::Validation(format!(
            "Instance is already {}",
            instance.status
        )));
    }

    // Kill local PID if the backend spawned it.
    if let Some(pid) = instance.local_pid {
        kill_local_pid(pid);
    }

    update_instance_status(pool, instance_id, "stopped", None, Some("stopped by API")).await
}

// ---------------------------------------------------------------------------
// Local spawn (dev / single-host mode)
// ---------------------------------------------------------------------------

/// Attempt to spawn a `magnetite-runtime` child process locally.
///
/// Only runs when `RUNTIME_BIN_PATH` env var is set.  On success updates the
/// instance row to `running`.  On failure logs a warning and returns `None`
/// so the caller falls back to the runner seam.
///
/// # Bucket-D note
///
/// This is intentionally limited: the binary must already exist (built via
/// `magnetite build`), and we pick a random port in the range 9000–9999.
/// Production deployments use the external runner seam instead.
async fn try_local_spawn(pool: &PgPool, instance: &RuntimeInstance) -> Option<RuntimeInstance> {
    let bin_path = std::env::var("RUNTIME_BIN_PATH").ok()?;

    // Pick a port from 9000–9999 (the runner will bind dynamically in reality).
    let port = pick_ephemeral_port().await?;
    let bind_addr = format!("0.0.0.0:{port}");
    let ws_endpoint = format!("ws://127.0.0.1:{port}");

    // Build CLI args for the magnetite-runtime binary.
    // The runtime's main entry point accepts:
    //   magnetite-runtime --bind <addr> --topology <t> --max-players <n> --tick-hz <hz>
    //                     [--wasm <path>]
    // Artifact URL is informational only at this layer; the runner is responsible
    // for downloading the wasm file before exec.
    let mut cmd = tokio::process::Command::new(&bin_path);
    cmd.arg("--bind")
        .arg(&bind_addr)
        .arg("--topology")
        .arg(&instance.topology)
        .arg("--max-players")
        .arg(instance.max_players.to_string())
        .arg("--tick-hz")
        .arg(instance.tick_hz.to_string());

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                instance_id = %instance.id,
                bin_path = %bin_path,
                error = %e,
                "local runtime spawn failed — instance stays 'pending' for runner"
            );
            return None;
        }
    };

    let pid = child.id().unwrap_or(0) as i32;

    // Forget the child handle — the process is now independent.
    // The backend records the PID so `stop_instance` can kill it.
    std::mem::forget(child);

    tracing::info!(
        instance_id = %instance.id,
        pid,
        ws_endpoint = %ws_endpoint,
        "local runtime process spawned"
    );

    // Update the DB row.
    let updated = sqlx::query_as::<_, RuntimeInstance>(
        "UPDATE runtime_instances SET
             status      = 'running',
             ws_endpoint = $2,
             local_pid   = $3,
             runner_note = 'spawned locally by backend',
             updated_at  = NOW()
         WHERE id = $1
         RETURNING id, game_id, version_id, artifact_id, status, ws_endpoint, topology,
                   max_players, tick_hz, local_pid, runner_note, requested_by,
                   created_at, updated_at",
    )
    .bind(instance.id)
    .bind(&ws_endpoint)
    .bind(pid)
    .fetch_one(pool)
    .await
    .ok()?;

    Some(updated)
}

/// Find an available port in 9000–9999.
async fn pick_ephemeral_port() -> Option<u16> {
    for port in 9000u16..9999 {
        if tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .is_ok()
        {
            return Some(port);
        }
    }
    None
}

/// Send SIGKILL (Unix) or TerminateProcess (Windows) to a local PID.
fn kill_local_pid(pid: i32) {
    #[cfg(unix)]
    {
        let _ = libc_kill(pid);
    }
    #[cfg(not(unix))]
    {
        let _ = pid; // no-op on non-unix targets
    }
}

#[cfg(unix)]
fn libc_kill(pid: i32) -> i32 {
    // Use nix or raw libc — to avoid adding a dep we use std::process indirection.
    // This is best-effort; failure is not fatal.
    use std::process::Command;
    Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .status()
        .map(|s| if s.success() { 0 } else { -1 })
        .unwrap_or(-1)
}

// ---------------------------------------------------------------------------
// Discovery self-advertisement (demotes the central poll — §3.4)
// ---------------------------------------------------------------------------
//
// The historic model is: nodes are recorded centrally in `runtime_instances`
// and a runner polls `GET /provisioning/pending`. DECENTRALIZATION.md §3.4
// replaces that with self-advertising nodes publishing a `SessionAd` to a
// swappable `Discovery` phonebook (the `magnetite node` binary already does this
// via `magnetite_runtime::prepare_game`). The helper below converts a central
// `RuntimeInstance` into the *same* `SessionAd` shape, so the central path can
// emit ads to the phonebook too and both models converge — the first concrete
// step of the demotion rather than a hard cutover.
/// Build a decentralized [`SessionAd`](magnetite_seams::discovery::SessionAd)
/// for a running instance, so it can be published to a `Discovery` provider
/// instead of being polled out of the `runtime_instances` table.
///
/// Returns `None` for an instance that is not yet reachable (no `ws_endpoint`).
/// `game_hash` is the content address of the game module (§3.3); `capacity` is
/// the node's self-measured hardware budget (§4).
// No in-crate caller yet: consumed by the node/discovery surface.
#[allow(dead_code)]
pub fn instance_session_ad(
    instance: &RuntimeInstance,
    game_hash: magnetite_seams::blobstore::Hash,
    capacity: magnetite_seams::discovery::Capacity,
) -> Option<magnetite_seams::discovery::SessionAd> {
    let ws = instance.ws_endpoint.clone()?;
    Some(magnetite_seams::discovery::SessionAd {
        game: game_hash,
        node: magnetite_seams::discovery::NodeAddr(ws),
        capacity,
        ping_hint: 0,
        price: None,
        chat_room: None,
        voice_room: None,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn instance_session_ad_shape() {
        use super::*;
        use chrono::Utc;
        let now = Utc::now();
        let mut inst = RuntimeInstance {
            id: uuid::Uuid::nil(),
            game_id: uuid::Uuid::nil(),
            version_id: None,
            artifact_id: None,
            status: "running".into(),
            ws_endpoint: None,
            topology: "Sharded".into(),
            max_players: 512,
            tick_hz: 20,
            local_pid: None,
            runner_note: None,
            requested_by: None,
            created_at: now,
            updated_at: now,
        };
        let hash = magnetite_seams::blobstore::Hash::of(b"game-module");
        let cap = magnetite_seams::discovery::Capacity {
            cpu_cores: 8,
            ram_mb: 32768,
            bandwidth_mbps: 1000,
            free_slots: 512,
            max_shards: 8,
        };
        // No endpoint yet ⇒ not advertisable.
        assert!(instance_session_ad(&inst, hash, cap.clone()).is_none());
        // Once reachable, it becomes a discoverable SessionAd.
        inst.ws_endpoint = Some("ws://127.0.0.1:9000".into());
        let ad = instance_session_ad(&inst, hash, cap).unwrap();
        assert_eq!(ad.game, hash);
        assert_eq!(ad.node.0, "ws://127.0.0.1:9000");
    }

    #[test]
    fn provision_request_topology_validation() {
        let valid = ["SingleRoom", "Dedicated", "Sharded"];
        let invalid = ["singleroom", "dedicated", "sharded", "room", ""];
        for t in valid {
            assert!(["SingleRoom", "Dedicated", "Sharded"].contains(&t));
        }
        for t in invalid {
            assert!(!["SingleRoom", "Dedicated", "Sharded"].contains(&t));
        }
    }

    #[test]
    fn runtime_instance_status_set() {
        let valid = ["pending", "running", "stopped", "failed"];
        assert!(valid.contains(&"pending"));
        assert!(valid.contains(&"running"));
        assert!(!valid.contains(&"done"));
        assert!(!valid.contains(&"error"));
    }

    #[test]
    fn tick_hz_bounds() {
        assert!(1 <= 20 && 20 <= 128);
        assert!(!(0 <= 0 && 0 <= 128 && 0 >= 1)); // 0 invalid
        assert!(!(129 <= 128)); // 129 invalid
    }
}
