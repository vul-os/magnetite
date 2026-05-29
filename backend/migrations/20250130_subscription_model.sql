-- Subscription Model Migration
-- Created: 2025-01-30

-- Remove fee_per_session from games
ALTER TABLE games DROP COLUMN IF EXISTS fee_per_session;

-- Add subscription_required column if games have tier requirements
ALTER TABLE games ADD COLUMN subscription_tier_required TEXT DEFAULT 'free';

-- Create subscription tiers table
CREATE TABLE IF NOT EXISTS subscription_tiers (
    id UUID PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    slug VARCHAR(50) UNIQUE NOT NULL,
    price_usdc DECIMAL(10, 2) NOT NULL DEFAULT 0,
    price_zar DECIMAL(10, 2) NOT NULL DEFAULT 0,
    features JSONB NOT NULL DEFAULT '{}',
    max_games INTEGER,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create user_subscriptions table
CREATE TABLE IF NOT EXISTS user_subscriptions (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    tier_id UUID REFERENCES subscription_tiers(id),
    status TEXT NOT NULL,
    current_period_start TIMESTAMPTZ,
    current_period_end TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create subscription_transactions table
CREATE TABLE IF NOT EXISTS subscription_transactions (
    id UUID PRIMARY KEY,
    user_subscription_id UUID REFERENCES user_subscriptions(id),
    amount DECIMAL(10, 2) NOT NULL,
    currency TEXT NOT NULL,
    status TEXT NOT NULL,
    payment_provider TEXT,
    payment_id TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Insert subscription tiers
INSERT INTO subscription_tiers (id, name, slug, price_usdc, price_zar, features, max_games) VALUES
('00000000-0000-0000-0000-000000000001', 'Free', 'free', 0, 0, '{"access": ["free_games"], "hours": 0}', 0),
('00000000-0000-0000-0000-000000000002', 'Basic', 'basic', 4.99, 99, '{"access": ["all_games"], "hours": 600}', 999),
('00000000-0000-0000-0000-000000000003', 'Pro', 'pro', 9.99, 199, '{"access": ["all_games"], "hours": 3000}', 999),
('00000000-0000-0000-0000-000000000004', 'Unlimited', 'unlimited', 19.99, 399, '{"access": ["all_games"], "hours": -1}', 999)
ON CONFLICT (slug) DO NOTHING;

-- Update platform_settings with default subscription tier
INSERT INTO platform_settings (key, value) VALUES
('default_subscription_tier', 'free')
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value;
