-- Add Performance Indexes
-- Created: 2025-01-25

-- Users table indexes
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_google_id ON users(google_id) WHERE google_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_discord_id ON users(discord_id) WHERE discord_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_github_id ON users(github_id) WHERE github_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at DESC);

-- Games table indexes
CREATE INDEX IF NOT EXISTS idx_games_developer_id ON games(developer_id);
CREATE INDEX IF NOT EXISTS idx_games_status ON games(status);
CREATE INDEX IF NOT EXISTS idx_games_created_at ON games(created_at DESC);

-- Wallet transactions indexes
CREATE INDEX IF NOT EXISTS idx_wallet_transactions_user_created ON wallet_transactions(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_wallet_transactions_status ON wallet_transactions(status) WHERE status = 'pending';

-- Scores indexes
CREATE INDEX IF NOT EXISTS idx_scores_game_user ON scores(game_id, user_id);
CREATE INDEX IF NOT EXISTS idx_scores_recorded_at ON scores(recorded_at DESC);

-- Sessions indexes
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at) WHERE expires_at > NOW();

-- Partial indexes for common queries
CREATE INDEX IF NOT EXISTS idx_games_active_status ON games(status) WHERE active = true AND status = 'active';
CREATE INDEX IF NOT EXISTS idx_matchmaking_waiting ON matchmaking_queue(status, created_at) WHERE status = 'waiting';
