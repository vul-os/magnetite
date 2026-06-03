-- Super-admin panel: in-house request analytics + an immutable audit log.
--
-- These tables back the hardened, server-rendered super-admin surface mounted at
-- /superadmin (separate from the JSON /api/v1/admin API and the React SPA). They
-- are written by the analytics-recording middleware and the super-admin action
-- handlers respectively; nothing in the public API reads or mutates them.

-- ── Request analytics ──────────────────────────────────────────────────────
-- One row per recorded HTTP request (health/metrics/superadmin traffic is
-- skipped by the recorder). Geo columns are populated by the in-house offline
-- GeoIP resolver when a GeoLite2 database is configured; otherwise they are NULL.
CREATE TABLE IF NOT EXISTS analytics_events (
    id          BIGSERIAL PRIMARY KEY,
    occurred_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    ip          TEXT,
    country     TEXT,           -- ISO-3166 alpha-2, e.g. 'US' (NULL when unknown)
    region      TEXT,
    city        TEXT,
    method      TEXT NOT NULL,
    path        TEXT NOT NULL,
    status      INTEGER NOT NULL,
    duration_ms INTEGER,
    user_id     UUID REFERENCES users(id) ON DELETE SET NULL,
    user_agent  TEXT,
    referer     TEXT
);

CREATE INDEX IF NOT EXISTS idx_analytics_events_occurred_at ON analytics_events (occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_analytics_events_country     ON analytics_events (country);
CREATE INDEX IF NOT EXISTS idx_analytics_events_path        ON analytics_events (path);
CREATE INDEX IF NOT EXISTS idx_analytics_events_user_id     ON analytics_events (user_id);

-- ── Super-admin audit log ──────────────────────────────────────────────────
-- Append-only record of every privileged super-admin action. Never updated or
-- deleted by application code; retained for forensic review.
CREATE TABLE IF NOT EXISTS superadmin_audit_log (
    id          BIGSERIAL PRIMARY KEY,
    occurred_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    actor_email TEXT NOT NULL,          -- the super-admin identity (from env credential)
    actor_ip    TEXT,
    action      TEXT NOT NULL,          -- e.g. 'login', 'user.ban', 'game.approve'
    target      TEXT,                   -- affected entity id / description
    detail      TEXT,                   -- optional human-readable context
    outcome     TEXT NOT NULL DEFAULT 'ok'  -- 'ok' | 'denied' | 'error'
);

CREATE INDEX IF NOT EXISTS idx_superadmin_audit_occurred_at ON superadmin_audit_log (occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_superadmin_audit_action      ON superadmin_audit_log (action);
