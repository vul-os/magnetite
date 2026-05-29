-- Add fee_amount, final_score, payout_status to play_sessions
ALTER TABLE play_sessions
ADD COLUMN IF NOT EXISTS fee_amount DECIMAL(18, 6) NOT NULL DEFAULT 0,
ADD COLUMN IF NOT EXISTS final_score BIGINT,
ADD COLUMN IF NOT EXISTS payout_status VARCHAR(50) DEFAULT 'pending';

-- Create game_revenue table if not exists
CREATE TABLE IF NOT EXISTS game_revenue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES games(id),
    developer_id UUID NOT NULL REFERENCES users(id),
    session_id UUID REFERENCES play_sessions(id),
    amount DECIMAL(18, 6) NOT NULL,
    developer_share DECIMAL(18, 6) NOT NULL,
    platform_share DECIMAL(18, 6) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for game_revenue
CREATE INDEX IF NOT EXISTS idx_game_revenue_game_id ON game_revenue(game_id);
CREATE INDEX IF NOT EXISTS idx_game_revenue_developer_id ON game_revenue(developer_id);
CREATE INDEX IF NOT EXISTS idx_game_revenue_session_id ON game_revenue(session_id);
CREATE INDEX IF NOT EXISTS idx_game_revenue_status ON game_revenue(status);

-- Update existing play_sessions indexes
CREATE INDEX IF NOT EXISTS idx_play_sessions_status ON play_sessions(status);
CREATE INDEX IF NOT EXISTS idx_play_sessions_payout_status ON play_sessions(payout_status);
