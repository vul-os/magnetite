-- Wave PAY (2026-06-01): switch from USDC to USD fiat denomination.
-- Non-destructive: UPDATE existing rows, ALTER defaults; keep wallet_address columns in place.

-- wallet_balances: rename existing USDC rows to USD and update the column default.
UPDATE wallet_balances SET currency = 'USD' WHERE currency = 'USDC';
ALTER TABLE wallet_balances ALTER COLUMN currency SET DEFAULT 'USD';

-- wallet_transactions: same treatment.
UPDATE wallet_transactions SET currency = 'USD' WHERE currency = 'USDC';
ALTER TABLE wallet_transactions ALTER COLUMN currency SET DEFAULT 'USD';

-- Drop wallet_address from users if it exists (on-chain address concept removed).
ALTER TABLE users DROP COLUMN IF EXISTS wallet_address;

-- subscription_tiers: the price_usdc column is kept for schema compatibility but now
-- represents the USD price (fiat). No structural change required — just a semantic rename.
-- Add a comment for clarity:
COMMENT ON COLUMN subscription_tiers.price_usdc IS 'Fiat USD price (was USDC; Circle removed 2026-06-01)';
