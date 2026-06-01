-- Subscription lifecycle: cancel-at-period-end flag, upgrade/downgrade proration support.
-- AX2 Agent A — 2026-06-05

-- Add missing columns to user_subscriptions if they do not already exist.
-- These were defined in payment.rs::init_tables() but never in a migration file.

ALTER TABLE user_subscriptions
    ADD COLUMN IF NOT EXISTS payment_provider TEXT NOT NULL DEFAULT 'free';

ALTER TABLE user_subscriptions
    ADD COLUMN IF NOT EXISTS provider_subscription_id TEXT;

ALTER TABLE user_subscriptions
    ADD COLUMN IF NOT EXISTS cancel_at_period_end BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE user_subscriptions
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

-- Ensure subscription_transactions has a transaction_type column to distinguish
-- initial, upgrade, downgrade, proration, and renewal events.
ALTER TABLE subscription_transactions
    ADD COLUMN IF NOT EXISTS transaction_type TEXT NOT NULL DEFAULT 'subscribe';

-- Index to make cancel-at-period-end renewal queries fast.
CREATE INDEX IF NOT EXISTS idx_user_subscriptions_status_period_end
    ON user_subscriptions (status, current_period_end)
    WHERE cancel_at_period_end = false;

-- Index for looking up the active subscription of a user quickly.
CREATE INDEX IF NOT EXISTS idx_user_subscriptions_user_status
    ON user_subscriptions (user_id, status);
