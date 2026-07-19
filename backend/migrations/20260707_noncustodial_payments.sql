-- Non-custodial crypto payments (DECENTRALIZATION.md §2 + §3.6).
--
-- The platform holds no funds. A purchase is an atomic wallet→wallet checkout
-- through the `PaymentRail` seam; the signed receipt IS the entitlement.
-- Fiat/custody tables (wallet_balances, wallet_transactions, payouts,
-- payout_requests, developer_balances, wise_recipients) are DEPRECATED — they are
-- left in place (additive migration, no data loss) but nothing writes to them any
-- more and they will be dropped in a later wave once no reader remains.

-- ── Users carry a wallet ADDRESS, never a balance ────────────────────────────
ALTER TABLE users ADD COLUMN IF NOT EXISTS wallet_address TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_wallet_address
    ON users (wallet_address)
    WHERE wallet_address IS NOT NULL;

-- ── Signed receipts — the durable proof of a wallet→wallet purchase ──────────
CREATE TABLE IF NOT EXISTS payment_receipts (
    id             UUID PRIMARY KEY,
    -- 'item_purchase' | 'subscription' | 'hosting'
    kind           TEXT        NOT NULL,
    -- Local account that initiated the checkout (for listing/audit only).
    buyer_id       UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Hex Ed25519 buyer wallet the rail actually charged.
    buyer_pubkey   TEXT        NOT NULL,
    purchase_id    UUID,
    item_id        UUID,
    game_id        UUID,
    -- Amounts are in the rail's smallest unit (e.g. USDC cents).
    total          BIGINT      NOT NULL,
    protocol_fee   BIGINT      NOT NULL DEFAULT 0,
    -- [{ wallet: hex, amount: bigint }, ...] — developer, [operator], [protocol].
    payouts        JSONB       NOT NULL,
    nonce          TEXT        NOT NULL,
    rail_pubkey    TEXT        NOT NULL,
    sig            TEXT        NOT NULL,
    rail           TEXT        NOT NULL DEFAULT 'mock',
    voided         BOOLEAN     NOT NULL DEFAULT false,
    voided_at      TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_payment_receipts_buyer   ON payment_receipts (buyer_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_payment_receipts_game    ON payment_receipts (game_id) WHERE game_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_payment_receipts_purchase
    ON payment_receipts (purchase_id) WHERE purchase_id IS NOT NULL;

-- ── Entitlement → receipt link (verify_receipt gates the grant) ──────────────
ALTER TABLE entitlements ADD COLUMN IF NOT EXISTS receipt_id UUID;

DO $$
BEGIN
    ALTER TABLE entitlements
        ADD CONSTRAINT entitlements_receipt_fk
        FOREIGN KEY (receipt_id) REFERENCES payment_receipts (id) ON DELETE SET NULL;
EXCEPTION
    WHEN duplicate_object THEN NULL;
    WHEN undefined_table  THEN NULL;
END $$;

CREATE INDEX IF NOT EXISTS idx_entitlements_receipt
    ON entitlements (receipt_id) WHERE receipt_id IS NOT NULL;

-- ── Hosting-fee payment channels (per-seat / per-hour to an operator) ────────
-- Scaffold: the mock rail returns a deterministic channel id. TODO(chain): anchor
-- a real channel on-chain and record off-chain signed channel updates per join.
CREATE TABLE IF NOT EXISTS hosting_channels (
    id              UUID PRIMARY KEY,
    -- Hex-encoded deterministic channel id from the rail.
    channel_id      TEXT        NOT NULL UNIQUE,
    payer_id        UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    operator_pubkey TEXT        NOT NULL,
    server_id       UUID,
    rail_pubkey     TEXT        NOT NULL,
    open            BOOLEAN     NOT NULL DEFAULT true,
    closed_at       TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_hosting_channels_payer ON hosting_channels (payer_id);

-- ── Mark the fiat/custody tables deprecated (comment only — no DROP) ─────────
DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'wallet_balances', 'wallet_transactions', 'payouts',
        'payout_requests', 'developer_balances', 'wise_recipients'
    ]
    LOOP
        IF to_regclass('public.' || t) IS NOT NULL THEN
            EXECUTE format(
                'COMMENT ON TABLE public.%I IS %L',
                t,
                'DEPRECATED (non-custodial migration 20260707): fiat/custody table. '
                || 'No writer remains; payments settle wallet-to-wallet via payment_receipts. '
                || 'Safe to DROP once no reader remains.'
            );
        END IF;
    END LOOP;
END $$;
