-- Auth security hardening (AX1 wave):
--   1. Add token_prefix to sessions for O(1) refresh-token lookup (avoids O(N) Argon2 scan).
--   2. Widen totp_secret column to hold hex-encoded ciphertext (encrypted at rest).

-- Add token_prefix column to sessions for indexed lookup.
-- Stores the first 16 chars of the raw refresh token (high-entropy prefix, safe to index).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'sessions' AND column_name = 'token_prefix'
    ) THEN
        ALTER TABLE sessions ADD COLUMN token_prefix VARCHAR(32);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_sessions_token_prefix
    ON sessions(token_prefix)
    WHERE token_prefix IS NOT NULL;

-- Widen totp_secret to hold hex-encoded encrypted ciphertext (was VARCHAR(64) for base32 only).
-- Encrypted form: hex(nonce[8] || ciphertext[len(plaintext)]) — up to ~128 chars for a 20-byte secret.
DO $$
BEGIN
    -- Only alter if the column is smaller than 256.
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'users'
          AND column_name = 'totp_secret'
          AND character_maximum_length < 256
    ) THEN
        ALTER TABLE users ALTER COLUMN totp_secret TYPE VARCHAR(256);
    END IF;
END $$;

-- Add oauth_identities table for linked-accounts endpoint.
CREATE TABLE IF NOT EXISTS oauth_identities (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider    VARCHAR(32) NOT NULL,   -- 'google' | 'github' | 'discord' | 'gitlab'
    provider_id VARCHAR(255) NOT NULL,  -- provider's user ID
    email       VARCHAR(255),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider, provider_id)
);

CREATE INDEX IF NOT EXISTS idx_oauth_identities_user_id ON oauth_identities(user_id);
