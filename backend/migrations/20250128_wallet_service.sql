-- Wallet service enhancements for Magnetite

-- Platform settings table for configurable limits
CREATE TABLE IF NOT EXISTS platform_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key VARCHAR(100) NOT NULL UNIQUE,
    value TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert default platform settings
INSERT INTO platform_settings (key, value, description) VALUES
    ('deposit_min_amount', '1.00', 'Minimum deposit amount in currency units'),
    ('deposit_max_amount', '10000.00', 'Maximum deposit amount in currency units'),
    ('withdrawal_min_amount', '1.00', 'Minimum withdrawal amount in currency units'),
    ('withdrawal_max_amount', '10000.00', 'Maximum withdrawal amount in currency units'),
    ('aml_daily_limit', '10000.00', 'AML daily deposit/transfer limit'),
    ('aml_monthly_limit', '100000.00', 'AML monthly deposit/transfer limit'),
    ('aml_transaction_count_limit', '100', 'Maximum transactions per day for AML')
ON CONFLICT (key) DO NOTHING;

-- Add new columns to wallet_transactions for idempotency and relations
ALTER TABLE wallet_transactions
    ADD COLUMN IF NOT EXISTS currency VARCHAR(10) NOT NULL DEFAULT 'USDC',
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(255),
    ADD COLUMN IF NOT EXISTS related_transaction_id UUID REFERENCES wallet_transactions(id),
    ADD COLUMN IF NOT EXISTS created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

-- Create index for idempotency lookups
CREATE INDEX IF NOT EXISTS idx_wallet_transactions_idempotency
    ON wallet_transactions(idempotency_key) WHERE idempotency_key IS NOT NULL;

-- Create index for AML velocity checks
CREATE INDEX IF NOT EXISTS idx_wallet_transactions_aml
    ON wallet_transactions(user_id, currency, tx_type, status, created_at);

-- Ensure wallet_balances has updated_at column
ALTER TABLE wallet_balances
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();
