-- Review report moderation columns.
-- Created: 2026-07-01
--
-- Adds moderation tracking fields to the review_reports table so admins can
-- record whether a report was dismissed, resolved (review removed), or
-- resulted in a warn/ban action against the review author.
--
-- Columns added:
--   status          TEXT    — "pending" | "dismissed" | "resolved" (default: "pending")
--   resolved_by     UUID    — admin user who acted on the report (FK → users)
--   resolved_at     TIMESTAMPTZ — when the action was taken
--   resolution_note TEXT    — optional admin note (e.g. reason for dismissal)
--
-- All columns use IF NOT EXISTS / conditional logic so this migration is
-- idempotent and safe to re-run.

ALTER TABLE review_reports
    ADD COLUMN IF NOT EXISTS status          TEXT                     NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS resolved_by     UUID                     REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS resolved_at     TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS resolution_note TEXT;

-- Index for the common admin query: filter by status.
CREATE INDEX IF NOT EXISTS idx_review_reports_status
    ON review_reports(status);

-- Index for quickly finding all reports an admin resolved.
CREATE INDEX IF NOT EXISTS idx_review_reports_resolved_by
    ON review_reports(resolved_by)
    WHERE resolved_by IS NOT NULL;

-- Constrain status to known values (guards against typos in application code).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'chk_review_reports_status'
          AND table_name = 'review_reports'
    ) THEN
        ALTER TABLE review_reports
            ADD CONSTRAINT chk_review_reports_status
            CHECK (status IN ('pending', 'dismissed', 'resolved'));
    END IF;
END
$$;
