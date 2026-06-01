-- Review helpful votes, review reports, and contact messages.
-- Created: 2026-06-04
--
-- review_helpful: per-user helpful vote on a review; deduplicated by unique constraint.
--   Toggling calls INSERT … ON CONFLICT DO NOTHING (vote) or DELETE (un-vote).
--   helpful_count on reviews is kept in sync by triggers defined below.
--
-- review_reports: a report filed against a review (e.g. spam, abuse).
--   Multiple reports per review are allowed; one report per (review, reporter) pair
--   per reason is prevented by the unique constraint.
--
-- contact_messages: persists Contact-page form submissions so support staff
--   can review them in the admin UI or directly in the DB.
--
-- helpful_count column on reviews: added here so the column exists even when
--   running against a DB that already has the reviews table (ALTER … ADD COLUMN IF NOT EXISTS).

-- ── 1. Add helpful_count to reviews ──────────────────────────────────────────

ALTER TABLE reviews ADD COLUMN IF NOT EXISTS helpful_count INTEGER NOT NULL DEFAULT 0;

-- ── 2. review_helpful ────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS review_helpful (
    review_id   UUID NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id)   ON DELETE CASCADE,
    created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (review_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_review_helpful_review_id ON review_helpful(review_id);
CREATE INDEX IF NOT EXISTS idx_review_helpful_user_id   ON review_helpful(user_id);

-- Trigger: keep reviews.helpful_count in sync automatically.
CREATE OR REPLACE FUNCTION fn_sync_helpful_count()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE reviews SET helpful_count = helpful_count + 1 WHERE id = NEW.review_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE reviews SET helpful_count = GREATEST(helpful_count - 1, 0) WHERE id = OLD.review_id;
    END IF;
    RETURN NULL;
END;
$$;

DROP TRIGGER IF EXISTS trg_review_helpful_count ON review_helpful;
CREATE TRIGGER trg_review_helpful_count
    AFTER INSERT OR DELETE ON review_helpful
    FOR EACH ROW EXECUTE FUNCTION fn_sync_helpful_count();

-- ── 3. review_reports ────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS review_reports (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    review_id   UUID NOT NULL REFERENCES reviews(id)  ON DELETE CASCADE,
    reporter_id UUID NOT NULL REFERENCES users(id)    ON DELETE CASCADE,
    reason      TEXT NOT NULL,
    created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    -- Prevent duplicate reports for the same (review, reporter, reason) triple.
    UNIQUE (review_id, reporter_id, reason)
);

CREATE INDEX IF NOT EXISTS idx_review_reports_review_id   ON review_reports(review_id);
CREATE INDEX IF NOT EXISTS idx_review_reports_reporter_id ON review_reports(reporter_id);

-- ── 4. contact_messages ──────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS contact_messages (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    email      TEXT NOT NULL,
    subject    TEXT NOT NULL,
    message    TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_contact_messages_created_at ON contact_messages(created_at);
CREATE INDEX IF NOT EXISTS idx_contact_messages_email      ON contact_messages(email);
