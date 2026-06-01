-- AX2-C migration: friend request list/cancel + full-text search + leaderboard seasons
-- Agent C owns this file exclusively.

-- ── 1. Full-text search: add tsvector generated column + GIN index on games ─────────────────────

ALTER TABLE games
    ADD COLUMN IF NOT EXISTS search_vector tsvector
        GENERATED ALWAYS AS (
            to_tsvector('english',
                coalesce(title, '') || ' ' || coalesce(description, '') || ' ' || coalesce(genre, '')
            )
        ) STORED;

CREATE INDEX IF NOT EXISTS games_search_vector_gin ON games USING GIN (search_vector);

-- ── 2. Leaderboard seasons: season_id column on game_high_scores ──────────────────────────────
-- Allows per-season leaderboard slices. NULL = "all-time" (existing rows).

ALTER TABLE game_high_scores
    ADD COLUMN IF NOT EXISTS season_id UUID REFERENCES seasons(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS game_high_scores_season_idx
    ON game_high_scores (game_id, season_id, score DESC);
