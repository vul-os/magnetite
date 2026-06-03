-- Notification Preferences — per-channel, per-category toggles
-- Channels: email, in_app, push
-- Categories: payouts, social, achievements, marketing

CREATE TABLE IF NOT EXISTS notification_preferences (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- payouts (PAYOUT_COMPLETE, SUBSCRIPTION_RENEWAL)
    payouts_email   BOOLEAN NOT NULL DEFAULT true,
    payouts_in_app  BOOLEAN NOT NULL DEFAULT true,
    payouts_push    BOOLEAN NOT NULL DEFAULT true,

    -- social (FRIEND_REQUEST, GAME_INVITE)
    social_email    BOOLEAN NOT NULL DEFAULT true,
    social_in_app   BOOLEAN NOT NULL DEFAULT true,
    social_push     BOOLEAN NOT NULL DEFAULT true,

    -- achievements (ACHIEVEMENT_UNLOCKED)
    achievements_email   BOOLEAN NOT NULL DEFAULT true,
    achievements_in_app  BOOLEAN NOT NULL DEFAULT true,
    achievements_push    BOOLEAN NOT NULL DEFAULT false,

    -- marketing (SYSTEM notifications with marketing flag)
    marketing_email   BOOLEAN NOT NULL DEFAULT false,
    marketing_in_app  BOOLEAN NOT NULL DEFAULT false,
    marketing_push    BOOLEAN NOT NULL DEFAULT false,

    created_at  TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at  TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE (user_id)
);

CREATE INDEX IF NOT EXISTS idx_notification_preferences_user_id
    ON notification_preferences (user_id);
