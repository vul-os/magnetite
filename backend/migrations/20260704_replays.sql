-- Replay storage for Magnetite — one row per finished authoritative match.
-- The full ReplayLog JSON (inputs + state-hashes) lives in replay_json (JSONB).

CREATE TABLE IF NOT EXISTS replays (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id            UUID NOT NULL REFERENCES games(id),
    -- match_id is a logical reference; tournament_matches rows may not exist for
    -- every match, so it is stored as a plain UUID without a FK constraint.
    match_id           UUID,
    recorded_by        UUID NOT NULL REFERENCES users(id),
    -- Full ReplayLog serde JSON: { config, frames, state_hashes }
    replay_json        JSONB NOT NULL,
    -- state_hash of the final tick, mirrored here for fast integrity queries.
    state_hash_final   BIGINT,
    -- Total number of ticks in the recording.
    duration_ticks     BIGINT NOT NULL DEFAULT 0,
    created_at         TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_replays_game_id    ON replays(game_id);
CREATE INDEX IF NOT EXISTS idx_replays_match_id   ON replays(match_id);
CREATE INDEX IF NOT EXISTS idx_replays_recorded_by ON replays(recorded_by);
CREATE INDEX IF NOT EXISTS idx_replays_created_at  ON replays(created_at DESC);

-- Extend tournament_matches with an optional replay FK so the bracket can link
-- directly to the stored replay of each match.
ALTER TABLE tournament_matches
    ADD COLUMN IF NOT EXISTS replay_id UUID REFERENCES replays(id);

-- Leaderboard / standings view for a tournament: wins, losses, points.
-- This is a plain view so no migration data is needed; the tournaments API
-- queries it directly.
CREATE OR REPLACE VIEW tournament_standings AS
SELECT
    tp.tournament_id,
    tp.user_id,
    u.username,
    tp.seed,
    tp.status                                                           AS participant_status,
    COUNT(CASE WHEN tm.winner_id = tp.user_id THEN 1 END)              AS wins,
    COUNT(CASE WHEN tm.status = 'completed'
               AND (tm.player1_id = tp.user_id OR tm.player2_id = tp.user_id)
               AND tm.winner_id != tp.user_id THEN 1 END)              AS losses,
    COUNT(CASE WHEN tm.winner_id = tp.user_id THEN 1 END) * 3         AS points
FROM tournament_participants tp
JOIN users u ON u.id = tp.user_id
LEFT JOIN tournament_matches tm
    ON tm.tournament_id = tp.tournament_id
    AND (tm.player1_id = tp.user_id OR tm.player2_id = tp.user_id)
GROUP BY tp.tournament_id, tp.user_id, u.username, tp.seed, tp.status;
