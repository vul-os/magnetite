-- API keys + 2FA TOTP schema

-- API keys: one-time plaintext returned at creation; only the hash stored.
-- prefix is the first 8 chars of the plaintext key, safe to display in listings.
CREATE TABLE IF NOT EXISTS api_keys (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        VARCHAR(255) NOT NULL,
    key_hash    VARCHAR(255) NOT NULL UNIQUE,
    prefix      VARCHAR(16)  NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_api_keys_user_id   ON api_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash   ON api_keys(key_hash);

-- 2FA TOTP columns on users.
-- totp_secret is the base32-encoded shared secret (stored as pending until verified).
-- totp_enabled is set true only after a successful verify call.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'users' AND column_name = 'totp_secret'
    ) THEN
        ALTER TABLE users ADD COLUMN totp_secret VARCHAR(64);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'users' AND column_name = 'totp_enabled'
    ) THEN
        ALTER TABLE users ADD COLUMN totp_enabled BOOLEAN NOT NULL DEFAULT FALSE;
    END IF;
END $$;
