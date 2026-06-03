-- Auto-flag support: chat_flags table + source column on review_reports.
-- Created: 2026-07-03
--
-- Three additions:
--
-- 1. review_reports.source — distinguishes user-filed reports from system
--    auto-flag events.  Values: 'user' | 'auto_flag'.  Default 'user' so all
--    existing rows retain their meaning.
--
-- 2. review_reports.reporter_id — made nullable so auto-flag rows can omit it
--    (auto-flagged reports have no human reporter).
--
-- 3. chat_flags — auto-flagged chat messages.  When the lightweight content
--    heuristic in ws/comms.rs triggers, a row is inserted here with
--    status='pending'.  Admins can list and act on these via
--    GET /admin/chat-flags and POST /admin/chat-flags/:id/action.

-- ── 1. review_reports: make reporter_id nullable for auto-flag rows ───────────
--
-- The existing NOT NULL constraint must be dropped first.  We recreate the
-- unique constraint so duplicates are still prevented (using NULLS NOT DISTINCT
-- on PG15+ or a partial index on older versions for the NULL case).

ALTER TABLE review_reports
    ALTER COLUMN reporter_id DROP NOT NULL;

-- ── 2. review_reports.source ─────────────────────────────────────────────────

ALTER TABLE review_reports
    ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'user';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'chk_review_reports_source'
          AND table_name = 'review_reports'
    ) THEN
        ALTER TABLE review_reports
            ADD CONSTRAINT chk_review_reports_source
            CHECK (source IN ('user', 'auto_flag'));
    END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_review_reports_source
    ON review_reports(source);

-- Composite index for the admin queue query (status + source).
CREATE INDEX IF NOT EXISTS idx_review_reports_status_source
    ON review_reports(status, source);

-- ── 2. chat_flags ─────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS chat_flags (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The channel where the message was posted.
    channel_id   UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    -- The author of the flagged message.
    author_id    UUID NOT NULL REFERENCES users(id)    ON DELETE CASCADE,
    -- Snapshot of the flagged message content (the message itself is not
    -- deleted; this is a moderation record only).
    content      TEXT NOT NULL,
    -- Comma-separated list of triggered heuristics (e.g. "profanity,url_flood").
    flag_reasons TEXT NOT NULL DEFAULT '',
    -- Moderation state.
    status       TEXT NOT NULL DEFAULT 'pending',
    -- Admin who acted on this flag (NULL while pending).
    resolved_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    resolved_at  TIMESTAMP WITH TIME ZONE,
    -- Optional note from the admin.
    resolution_note TEXT,
    created_at   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),

    CONSTRAINT chk_chat_flags_status
        CHECK (status IN ('pending', 'dismissed', 'resolved'))
);

CREATE INDEX IF NOT EXISTS idx_chat_flags_channel_id  ON chat_flags(channel_id);
CREATE INDEX IF NOT EXISTS idx_chat_flags_author_id   ON chat_flags(author_id);
CREATE INDEX IF NOT EXISTS idx_chat_flags_status      ON chat_flags(status);
CREATE INDEX IF NOT EXISTS idx_chat_flags_created_at  ON chat_flags(created_at DESC);
