-- Add is_admin column to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN NOT NULL DEFAULT false;

-- Add banned_at column for ban tracking
ALTER TABLE users ADD COLUMN IF NOT EXISTS banned_at TIMESTAMP WITH TIME ZONE;

-- Add featured_at column to games table for feature tracking
ALTER TABLE games ADD COLUMN IF NOT EXISTS featured_at TIMESTAMP WITH TIME ZONE;

-- Add reviewed_at and reviewed_by columns to games table
ALTER TABLE games ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE games ADD COLUMN IF NOT EXISTS reviewed_by UUID REFERENCES users(id);

-- Add payout_status and payout_amount columns to wallet_transactions
ALTER TABLE wallet_transactions ADD COLUMN IF NOT EXISTS payout_status VARCHAR(50) DEFAULT NULL;
ALTER TABLE wallet_transactions ADD COLUMN IF NOT EXISTS payout_amount DECIMAL(18, 6) DEFAULT NULL;

-- Create payouts table
CREATE TABLE IF NOT EXISTS payouts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    amount DECIMAL(18, 6) NOT NULL,
    destination TEXT NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE,
    cancelled_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX IF NOT EXISTS idx_payouts_user_id ON payouts(user_id);
CREATE INDEX IF NOT EXISTS idx_payouts_status ON payouts(status);