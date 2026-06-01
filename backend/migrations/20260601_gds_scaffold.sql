-- GDS (Game Dev in Magnetite) scaffold: template_id column on games + game_scaffolds table.
-- Created: 2026-06-01

-- Add template_id to games so each game remembers which template it was scaffolded from.
ALTER TABLE games
    ADD COLUMN IF NOT EXISTS template_id TEXT;

-- game_scaffolds: one row per scaffold action. Records the template chosen, the
-- CLI command the developer should run, and any additional context (file manifest etc).
CREATE TABLE IF NOT EXISTS game_scaffolds (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id         UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    developer_id    UUID NOT NULL REFERENCES users(id),
    template_id     TEXT NOT NULL,
    cli_command     TEXT NOT NULL,
    template_repo   TEXT NOT NULL,
    manifest        JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_game_scaffolds_game_id
    ON game_scaffolds(game_id);
CREATE INDEX IF NOT EXISTS idx_game_scaffolds_developer_id
    ON game_scaffolds(developer_id);
