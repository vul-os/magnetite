-- Wave 8: Points Economy + Developer Marketplace
-- Created: 2026-05-31

-- ─────────────────────────────────────────────
-- 1. SEASONS
-- ─────────────────────────────────────────────
-- A season defines a named period for point resets / competitions.
CREATE TABLE IF NOT EXISTS seasons (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,                 -- e.g. "Season 1 — Launch"
    starts_at   TIMESTAMP WITH TIME ZONE NOT NULL,
    ends_at     TIMESTAMP WITH TIME ZONE,      -- NULL = ongoing
    is_active   BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_seasons_active_one
    ON seasons(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_seasons_starts_at ON seasons(starts_at);

-- ─────────────────────────────────────────────
-- 2. POINTS LEDGER
-- ─────────────────────────────────────────────
-- Append-only ledger; every point change produces a row.
-- balance_snapshot is the user's total balance *after* this entry.
CREATE TABLE IF NOT EXISTS points_ledger (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    delta            BIGINT NOT NULL,             -- positive = award, negative = spend
    reason           TEXT NOT NULL,               -- e.g. 'game_complete', 'purchase', 'season_reset'
    game_id          UUID REFERENCES games(id) ON DELETE SET NULL,
    season_id        UUID REFERENCES seasons(id) ON DELETE SET NULL,
    balance_snapshot BIGINT NOT NULL,             -- running balance after this entry
    metadata         JSONB,                       -- arbitrary context (match_id, item_id, …)
    created_at       TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_points_ledger_user_id      ON points_ledger(user_id);
CREATE INDEX IF NOT EXISTS idx_points_ledger_user_created ON points_ledger(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_points_ledger_game_id      ON points_ledger(game_id) WHERE game_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_points_ledger_season_id    ON points_ledger(season_id) WHERE season_id IS NOT NULL;

-- ─────────────────────────────────────────────
-- 3. POINT BALANCES (materialised running total)
-- ─────────────────────────────────────────────
-- Maintained atomically alongside points_ledger inserts so reads are O(1).
CREATE TABLE IF NOT EXISTS point_balances (
    user_id      UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance      BIGINT NOT NULL DEFAULT 0,
    season_id    UUID REFERENCES seasons(id) ON DELETE SET NULL,  -- season of last reset
    updated_at   TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ─────────────────────────────────────────────
-- 4. POINT REWARDS CATALOG
-- ─────────────────────────────────────────────
-- Defines earnable or redeemable reward definitions.
CREATE TABLE IF NOT EXISTS point_rewards (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT NOT NULL,
    description  TEXT,
    kind         TEXT NOT NULL CHECK (kind IN ('earn', 'redeem')),
    points       BIGINT NOT NULL,                 -- pts awarded (earn) or cost (redeem)
    game_id      UUID REFERENCES games(id) ON DELETE CASCADE,
    active       BOOLEAN NOT NULL DEFAULT true,
    metadata     JSONB,
    created_at   TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at   TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_point_rewards_game_id ON point_rewards(game_id) WHERE game_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_point_rewards_kind    ON point_rewards(kind, active);

-- ─────────────────────────────────────────────
-- 5. DEV STORES
-- ─────────────────────────────────────────────
-- One store per game, owned by the developer.
CREATE TABLE IF NOT EXISTS dev_stores (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id      UUID NOT NULL UNIQUE REFERENCES games(id) ON DELETE CASCADE,
    developer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    description  TEXT,
    active       BOOLEAN NOT NULL DEFAULT true,
    metadata     JSONB,
    created_at   TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at   TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_dev_stores_developer_id ON dev_stores(developer_id);
CREATE INDEX IF NOT EXISTS idx_dev_stores_game_id      ON dev_stores(game_id);

-- ─────────────────────────────────────────────
-- 6. STORE ITEMS
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS store_items (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id     UUID NOT NULL REFERENCES dev_stores(id) ON DELETE CASCADE,
    game_id      UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    sku          TEXT NOT NULL,
    name         TEXT NOT NULL,
    description  TEXT,
    price        NUMERIC(18, 6) NOT NULL,
    currency     TEXT NOT NULL DEFAULT 'USDC',    -- 'USDC' or 'points'
    kind         TEXT NOT NULL CHECK (kind IN ('cosmetic', 'item', 'dlc', 'pass')),
    active       BOOLEAN NOT NULL DEFAULT true,
    metadata     JSONB,                           -- e.g. {icon_url, rarity, boost_multiplier}
    created_at   TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at   TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (store_id, sku)
);

CREATE INDEX IF NOT EXISTS idx_store_items_store_id ON store_items(store_id);
CREATE INDEX IF NOT EXISTS idx_store_items_game_id  ON store_items(game_id);
CREATE INDEX IF NOT EXISTS idx_store_items_kind     ON store_items(kind, active);
CREATE INDEX IF NOT EXISTS idx_store_items_currency ON store_items(currency, active);

-- ─────────────────────────────────────────────
-- 7. STORE PURCHASES
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS store_purchases (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id          UUID NOT NULL REFERENCES store_items(id) ON DELETE RESTRICT,
    store_id         UUID NOT NULL REFERENCES dev_stores(id) ON DELETE RESTRICT,
    game_id          UUID NOT NULL REFERENCES games(id) ON DELETE RESTRICT,
    price_paid       NUMERIC(18, 6) NOT NULL,
    currency         TEXT NOT NULL,               -- 'USDC' or 'points'
    -- developer_share: 70 % of price_paid for USDC purchases; not tracked for points
    developer_share  NUMERIC(18, 6),
    platform_fee     NUMERIC(18, 6),
    status           TEXT NOT NULL DEFAULT 'completed'
                         CHECK (status IN ('completed', 'refunded', 'failed')),
    idempotency_key  TEXT UNIQUE,
    metadata         JSONB,
    created_at       TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_store_purchases_user_id   ON store_purchases(user_id);
CREATE INDEX IF NOT EXISTS idx_store_purchases_item_id   ON store_purchases(item_id);
CREATE INDEX IF NOT EXISTS idx_store_purchases_store_id  ON store_purchases(store_id);
CREATE INDEX IF NOT EXISTS idx_store_purchases_game_id   ON store_purchases(game_id);
CREATE INDEX IF NOT EXISTS idx_store_purchases_created   ON store_purchases(created_at DESC);

-- ─────────────────────────────────────────────
-- 8. ENTITLEMENTS
-- ─────────────────────────────────────────────
-- Which users own which items (survives item deactivation).
CREATE TABLE IF NOT EXISTS entitlements (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id     UUID NOT NULL REFERENCES store_items(id) ON DELETE CASCADE,
    purchase_id UUID REFERENCES store_purchases(id) ON DELETE SET NULL,
    granted_at  TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at  TIMESTAMP WITH TIME ZONE,         -- NULL = permanent
    revoked     BOOLEAN NOT NULL DEFAULT false,
    UNIQUE (user_id, item_id)
);

CREATE INDEX IF NOT EXISTS idx_entitlements_user_id   ON entitlements(user_id);
CREATE INDEX IF NOT EXISTS idx_entitlements_item_id   ON entitlements(item_id);
CREATE INDEX IF NOT EXISTS idx_entitlements_user_item ON entitlements(user_id, item_id, revoked);

-- ─────────────────────────────────────────────
-- 9. SEED: first active season
-- ─────────────────────────────────────────────
INSERT INTO seasons (name, starts_at, is_active)
VALUES ('Season 1 — Launch', NOW(), true)
ON CONFLICT DO NOTHING;
