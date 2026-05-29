CREATE TABLE IF NOT EXISTS categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    slug VARCHAR(100) UNIQUE NOT NULL,
    icon VARCHAR(50),
    description TEXT,
    sort_order INTEGER DEFAULT 0
);

INSERT INTO categories (name, slug, icon, sort_order) VALUES
('Action', 'action', '🎮', 1),
('Adventure', 'adventure', '🗺️', 2),
('Puzzle', 'puzzle', '🧩', 3),
('Strategy', 'strategy', '♟️', 4),
('RPG', 'rpg', '⚔️', 5),
('Sports', 'sports', '⚽', 6),
('Racing', 'racing', '🏎️', 7),
('Simulation', 'simulation', '🎯', 8),
('Multiplayer', 'multiplayer', '👥', 9);

ALTER TABLE games ADD COLUMN category_id UUID REFERENCES categories(id);

CREATE INDEX IF NOT EXISTS idx_games_category_id ON games(category_id);