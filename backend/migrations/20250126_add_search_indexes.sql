-- Add pg_trgm extension for fuzzy search
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Create indexes for search
CREATE INDEX IF NOT EXISTS idx_games_title_search ON games USING gin(title gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_games_description_search ON games USING gin(description gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_users_username_search ON users USING gin(username gin_trgm_ops);