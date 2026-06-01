-- Migration: wise_recipients — stores a developer's registered Wise payout recipient.
-- Each developer can have one current recipient per currency (soft-deleted by created_at ordering;
-- the newest row for a developer is treated as current).

CREATE TABLE IF NOT EXISTS wise_recipients (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    developer_id        UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Wise-assigned recipient/account id (numeric string or sandbox_ prefix in dev).
    wise_recipient_id   TEXT        NOT NULL,
    currency            TEXT        NOT NULL DEFAULT 'USD',
    account_holder_name TEXT        NOT NULL,
    -- JSON blob of the full details (account type, routing number, etc.) — never surfaced raw to clients.
    detail              JSONB       NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS wise_recipients_developer_id_idx ON wise_recipients (developer_id, created_at DESC);
