-- Communities (Discord-class comms): servers/guilds, channels, messages, DMs,
-- presence, voice rooms, voice participants, and streams.
-- Created: 2026-05-30

-- ---------------------------------------------------------------------------
-- communities — "servers" / "guilds"; a community groups channels + members
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS communities (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL UNIQUE,
    description     TEXT,
    icon_url        TEXT,
    banner_url      TEXT,
    is_public       BOOLEAN NOT NULL DEFAULT true,
    member_count    INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- community_members — membership + role within a community
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS community_members (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id    UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            TEXT NOT NULL DEFAULT 'member',  -- 'owner' | 'admin' | 'moderator' | 'member'
    nickname        TEXT,
    joined_at       TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (community_id, user_id)
);

-- ---------------------------------------------------------------------------
-- channels — text or voice channel inside a community
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS channels (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id    UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL DEFAULT 'text',   -- 'text' | 'voice'
    topic           TEXT,
    position        INTEGER NOT NULL DEFAULT 0,
    is_private      BOOLEAN NOT NULL DEFAULT false,
    slow_mode_secs  INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- channel_members — explicit access list for private channels
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS channel_members (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id      UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    can_write       BOOLEAN NOT NULL DEFAULT true,
    added_at        TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (channel_id, user_id)
);

-- ---------------------------------------------------------------------------
-- messages — text messages in a channel
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id      UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    author_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content         TEXT NOT NULL,
    edited_at       TIMESTAMP WITH TIME ZONE,
    deleted         BOOLEAN NOT NULL DEFAULT false,
    reply_to_id     UUID REFERENCES messages(id) ON DELETE SET NULL,
    attachments     JSONB,              -- [{url, mime, size_bytes, name}]
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- dm_threads — direct-message conversation between two users
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS dm_threads (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_a_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_b_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_message_at TIMESTAMP WITH TIME ZONE,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (user_a_id, user_b_id),
    CONSTRAINT dm_threads_ordered CHECK (user_a_id < user_b_id)
);

-- ---------------------------------------------------------------------------
-- dm_messages — messages within a DM thread
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS dm_messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id       UUID NOT NULL REFERENCES dm_threads(id) ON DELETE CASCADE,
    author_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content         TEXT NOT NULL,
    edited_at       TIMESTAMP WITH TIME ZONE,
    deleted         BOOLEAN NOT NULL DEFAULT false,
    attachments     JSONB,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- presence — last-seen status per user; upserted by the WS handler on connect/
-- disconnect.  Games can also write 'in_game' status via the SDK.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS presence (
    user_id         UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'online',  -- 'online' | 'idle' | 'dnd' | 'in_game' | 'offline'
    activity        TEXT,                             -- free-form: "Playing Rustcraft", "Streaming"
    game_id         UUID REFERENCES games(id) ON DELETE SET NULL,
    last_seen       TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- voice_rooms — active voice session attached to a channel or to a match/lobby
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS voice_rooms (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id      UUID REFERENCES channels(id) ON DELETE CASCADE,
    game_session_id UUID,                             -- optional: wired to a match/lobby
    room_token      TEXT NOT NULL UNIQUE,             -- opaque token used by clients to join
    max_participants INTEGER NOT NULL DEFAULT 16,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ended_at        TIMESTAMP WITH TIME ZONE
);

-- ---------------------------------------------------------------------------
-- voice_participants — who is currently in a voice room
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS voice_participants (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id         UUID NOT NULL REFERENCES voice_rooms(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_muted        BOOLEAN NOT NULL DEFAULT false,
    is_deafened     BOOLEAN NOT NULL DEFAULT false,
    is_video        BOOLEAN NOT NULL DEFAULT false,
    joined_at       TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    left_at         TIMESTAMP WITH TIME ZONE,
    UNIQUE (room_id, user_id)
);

-- ---------------------------------------------------------------------------
-- streams — go-live / screen-share / game-capture sessions
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS streams (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    streamer_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    community_id    UUID REFERENCES communities(id) ON DELETE SET NULL,
    channel_id      UUID REFERENCES channels(id) ON DELETE SET NULL,
    title           TEXT NOT NULL,
    game_id         UUID REFERENCES games(id) ON DELETE SET NULL,
    status          TEXT NOT NULL DEFAULT 'offline', -- 'offline' | 'live' | 'ended'
    viewer_count    INTEGER NOT NULL DEFAULT 0,
    -- HLS endpoint for in-platform viewers; populated once the stream is live
    hls_url         TEXT,
    -- RTMP egress config for external services (Twitch/YouTube); the backend
    -- acts as an RTMP relay forwarder — heavy media infra (LiveKit / mediasoup)
    -- is the documented scale path for full SFU/MCU support.
    rtmp_key        TEXT,
    external_rtmp_url TEXT,
    started_at      TIMESTAMP WITH TIME ZONE,
    ended_at        TIMESTAMP WITH TIME ZONE,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_communities_owner     ON communities(owner_id);
CREATE INDEX IF NOT EXISTS idx_communities_slug      ON communities(slug);

CREATE INDEX IF NOT EXISTS idx_community_members_community ON community_members(community_id);
CREATE INDEX IF NOT EXISTS idx_community_members_user      ON community_members(user_id);

CREATE INDEX IF NOT EXISTS idx_channels_community    ON channels(community_id);
CREATE INDEX IF NOT EXISTS idx_channels_kind         ON channels(community_id, kind);

CREATE INDEX IF NOT EXISTS idx_channel_members_channel ON channel_members(channel_id);
CREATE INDEX IF NOT EXISTS idx_channel_members_user    ON channel_members(user_id);

CREATE INDEX IF NOT EXISTS idx_messages_channel      ON messages(channel_id);
CREATE INDEX IF NOT EXISTS idx_messages_author       ON messages(author_id);
CREATE INDEX IF NOT EXISTS idx_messages_created      ON messages(channel_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_reply        ON messages(reply_to_id) WHERE reply_to_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_dm_threads_user_a     ON dm_threads(user_a_id);
CREATE INDEX IF NOT EXISTS idx_dm_threads_user_b     ON dm_threads(user_b_id);
CREATE INDEX IF NOT EXISTS idx_dm_threads_last_msg   ON dm_threads(last_message_at DESC);

CREATE INDEX IF NOT EXISTS idx_dm_messages_thread    ON dm_messages(thread_id);
CREATE INDEX IF NOT EXISTS idx_dm_messages_created   ON dm_messages(thread_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_presence_status       ON presence(status);
CREATE INDEX IF NOT EXISTS idx_presence_game         ON presence(game_id) WHERE game_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_voice_rooms_channel   ON voice_rooms(channel_id);
CREATE INDEX IF NOT EXISTS idx_voice_rooms_active    ON voice_rooms(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_voice_participants_room  ON voice_participants(room_id);
CREATE INDEX IF NOT EXISTS idx_voice_participants_user  ON voice_participants(user_id);
CREATE INDEX IF NOT EXISTS idx_voice_participants_active
    ON voice_participants(room_id) WHERE left_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_streams_streamer      ON streams(streamer_id);
CREATE INDEX IF NOT EXISTS idx_streams_community     ON streams(community_id) WHERE community_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_streams_status        ON streams(status);
CREATE INDEX IF NOT EXISTS idx_streams_live          ON streams(status) WHERE status = 'live';
