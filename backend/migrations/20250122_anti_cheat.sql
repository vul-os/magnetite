-- Anti-Cheat Tables

CREATE TABLE IF NOT EXISTS anti_cheat_bans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    reason TEXT,
    fingerprint TEXT,
    banned_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS session_replays (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID REFERENCES play_sessions(id),
    inputs JSONB,
    recorded_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for anti_cheat_bans
CREATE INDEX IF NOT EXISTS idx_anti_cheat_bans_user_id ON anti_cheat_bans(user_id);
CREATE INDEX IF NOT EXISTS idx_anti_cheat_bans_fingerprint ON anti_cheat_bans(fingerprint);
CREATE INDEX IF NOT EXISTS idx_anti_cheat_bans_expires_at ON anti_cheat_bans(expires_at);

-- Indexes for session_replays
CREATE INDEX IF NOT EXISTS idx_session_replays_session_id ON session_replays(session_id);
CREATE INDEX IF NOT EXISTS idx_session_replays_recorded_at ON session_replays(recorded_at);
