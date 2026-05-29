-- Platform Settings
-- Created: 2025-01-26

CREATE TABLE IF NOT EXISTS platform_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key VARCHAR(100) UNIQUE NOT NULL,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed defaults
INSERT INTO platform_settings (key, value) VALUES
('platform_fee_percentage', '30'),
('min_payout_amount', '25'),
('max_deposit_amount', '10000'),
('max_withdraw_amount', '10000'),
('maintenance_mode', 'false'),
('registration_enabled', 'true')
ON CONFLICT (key) DO NOTHING;