-- Wave PROD-1 Agent 4: Store-purchase hardening
-- Adds refund tracking columns and a receipts view.

-- ─────────────────────────────────────────────
-- 1. Refund tracking on store_purchases
-- ─────────────────────────────────────────────
ALTER TABLE store_purchases
    ADD COLUMN IF NOT EXISTS refunded_at  TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS refunded_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS refund_reason TEXT;

-- Index to make refunded-purchase queries fast.
CREATE INDEX IF NOT EXISTS idx_store_purchases_refunded
    ON store_purchases(refunded_at) WHERE refunded_at IS NOT NULL;

-- ─────────────────────────────────────────────
-- 2. purchase_receipts view
-- ─────────────────────────────────────────────
-- Joins store_purchases with store_items so the API can return
-- item name / SKU / kind alongside each purchase without an extra query.
CREATE OR REPLACE VIEW purchase_receipts AS
SELECT
    sp.id               AS purchase_id,
    sp.user_id,
    sp.item_id,
    sp.store_id,
    sp.game_id,
    si.sku              AS item_sku,
    si.name             AS item_name,
    si.kind             AS item_kind,
    sp.price_paid,
    sp.currency,
    sp.developer_share,
    sp.platform_fee,
    sp.status,
    sp.idempotency_key,
    sp.metadata,
    sp.created_at       AS purchased_at,
    sp.refunded_at,
    sp.refunded_by,
    sp.refund_reason
FROM store_purchases sp
JOIN store_items si ON si.id = sp.item_id;
