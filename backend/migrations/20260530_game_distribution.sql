-- Game Distribution: artifacts, versions, and build-to-artifact linkage
-- Created: 2026-05-30

-- game_versions: one row per semantic version tag of a registered game.
-- Tracks the source commit, declared semver string, and whether this is the
-- currently-live version shown to players.
CREATE TABLE IF NOT EXISTS game_versions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id         UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    version         TEXT NOT NULL,          -- e.g. "0.3.1", "1.0.0-rc.1"
    commit_sha      TEXT NOT NULL,
    release_notes   TEXT,
    is_live         BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (game_id, version)
);

-- game_artifacts: the built output (WASM blob or native server binary) produced
-- by a successful CI build.  One version may have multiple artifacts
-- (e.g. wasm-client + linux-server).  artifact_url is where players/servers
-- fetch the binary; for now this is an S3-compatible URL or a CDN URL.
CREATE TABLE IF NOT EXISTS game_artifacts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id         UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    version_id      UUID REFERENCES game_versions(id) ON DELETE SET NULL,
    build_id        UUID REFERENCES build_status(id) ON DELETE SET NULL,
    artifact_type   TEXT NOT NULL DEFAULT 'wasm',   -- 'wasm' | 'server-linux' | 'server-windows'
    artifact_url    TEXT,                            -- CDN/S3 URL; NULL while build is pending
    file_size_bytes BIGINT,
    sha256_hash     TEXT,                            -- integrity check for the downloaded blob
    build_status    TEXT NOT NULL DEFAULT 'pending', -- pending | building | success | failed
    error_message   TEXT,
    metadata        JSONB,                           -- arbitrary extra fields (engine version, etc.)
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Add a link from build_status back to the game that triggered it, so the
-- webhook handler can update artifact records without a repo-name join.
ALTER TABLE build_status
    ADD COLUMN IF NOT EXISTS game_id UUID REFERENCES games(id) ON DELETE SET NULL;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_game_versions_game_id
    ON game_versions(game_id);
CREATE INDEX IF NOT EXISTS idx_game_versions_is_live
    ON game_versions(game_id, is_live) WHERE is_live = true;

CREATE INDEX IF NOT EXISTS idx_game_artifacts_game_id
    ON game_artifacts(game_id);
CREATE INDEX IF NOT EXISTS idx_game_artifacts_version_id
    ON game_artifacts(version_id);
CREATE INDEX IF NOT EXISTS idx_game_artifacts_build_id
    ON game_artifacts(build_id);
CREATE INDEX IF NOT EXISTS idx_game_artifacts_build_status
    ON game_artifacts(build_status);

CREATE INDEX IF NOT EXISTS idx_build_status_game_id
    ON build_status(game_id);
