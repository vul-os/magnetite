-- Runtime instances: tracks provisioned magnetite-runtime process instances.
-- Created: 2026-06-03
--
-- Each row represents one authoritative game-server instance that was requested
-- for a specific game artifact.  The backend records the intended ws_endpoint
-- (the URL clients use to connect) and the current lifecycle status.
--
-- Bucket-D runner seam
-- ─────────────────────
-- The backend itself does NOT exec magnetite-runtime.  Instead:
--   1. When a provisioning request arrives the backend inserts a row here with
--      status = 'pending'.
--   2. A self-hosted runner (scripts/provision-instance.sh or a container
--      orchestrator) polls GET /api/v1/provisioning/pending and starts the
--      actual magnetite-runtime process, then PATCHes the instance to
--      status = 'running' with the bound ws_endpoint.
--   3. Clients call GET /api/v1/provisioning/instances/:id and read
--      ws_endpoint to connect once status = 'running'.
--
-- When RUNTIME_BIN_PATH env var is set the backend CAN attempt to spawn
-- the runtime binary directly (single-host dev mode) — see ProvisioningService
-- in backend/src/services/provisioning.rs.

CREATE TABLE IF NOT EXISTS runtime_instances (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id         UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    version_id      UUID REFERENCES game_versions(id) ON DELETE SET NULL,
    artifact_id     UUID REFERENCES game_artifacts(id) ON DELETE SET NULL,

    -- Lifecycle: pending → running → stopped | failed
    status          TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending','running','stopped','failed')),

    -- WebSocket URL the game client connects to, e.g. ws://10.0.0.5:9000
    -- NULL while status = 'pending'; set by runner or by the backend spawn path.
    ws_endpoint     TEXT,

    -- Topology hint used when launching (serialised from MatchConfig.topology).
    -- Allows the runner to configure the correct tick-hz / shard params.
    topology        TEXT NOT NULL DEFAULT 'SingleRoom',   -- 'SingleRoom' | 'Dedicated' | 'Sharded'
    max_players     INT  NOT NULL DEFAULT 4,
    tick_hz         INT  NOT NULL DEFAULT 20,

    -- OS process id if the backend spawned the runtime locally (dev mode).
    local_pid       INT,

    -- Freeform notes from runner (version string, host, error message, …).
    runner_note     TEXT,

    requested_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_runtime_instances_game_id
    ON runtime_instances(game_id);

CREATE INDEX IF NOT EXISTS idx_runtime_instances_status
    ON runtime_instances(status);

CREATE INDEX IF NOT EXISTS idx_runtime_instances_artifact_id
    ON runtime_instances(artifact_id);
