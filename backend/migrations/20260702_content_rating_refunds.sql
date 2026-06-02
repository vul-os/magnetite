-- Content rating for games + refund_records table.
-- Created: 2026-07-02

-- ── Content rating ────────────────────────────────────────────────────────────
-- Adds a content_rating column to games (everyone/teen/mature) for age-gating.
-- Constraint is added idempotently so re-running is safe.

ALTER TABLE games
    ADD COLUMN IF NOT EXISTS content_rating TEXT NOT NULL DEFAULT 'everyone';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'chk_games_content_rating'
          AND table_name = 'games'
    ) THEN
        ALTER TABLE games
            ADD CONSTRAINT chk_games_content_rating
            CHECK (content_rating IN ('everyone', 'teen', 'mature'));
    END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_games_content_rating ON games(content_rating);

-- ── Refund records ────────────────────────────────────────────────────────────
-- Admin-initiated refunds: records a reversal against a wallet_transaction or
-- subscription, then triggers the provider refund path (Paystack/Wise).

CREATE TABLE IF NOT EXISTS refund_records (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The wallet_transaction that is being reversed (deposit / subscription charge).
    transaction_id   UUID NOT NULL REFERENCES wallet_transactions(id) ON DELETE RESTRICT,
    -- The user whose balance is affected.
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Admin who initiated the refund.
    admin_id         UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    -- Refund amount in USD.
    amount           NUMERIC(18,6) NOT NULL,
    -- Provider that was called: "paystack" | "wise" | "none" (platform credit only).
    provider         TEXT NOT NULL DEFAULT 'none',
    -- Provider-assigned refund reference (NULL if not applicable or unconfigured).
    provider_ref     TEXT,
    -- "pending" | "completed" | "failed" | "provider_unconfigured"
    status           TEXT NOT NULL DEFAULT 'pending',
    reason           TEXT,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_refund_records_user_id ON refund_records(user_id);
CREATE INDEX IF NOT EXISTS idx_refund_records_transaction_id ON refund_records(transaction_id);
CREATE INDEX IF NOT EXISTS idx_refund_records_admin_id ON refund_records(admin_id);
CREATE INDEX IF NOT EXISTS idx_refund_records_status ON refund_records(status);
