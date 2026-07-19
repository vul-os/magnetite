-- Pluggable comms (DECENTRALIZATION.md §2 + §3.5).
--
-- Chat / voice / video / streaming are no longer something Magnetite builds.
-- Every social surface is a room behind the `CommsProvider` seam; the old
-- in-house stack (communities/channels/messages/ws-comms/ws-voice/streaming +
-- MediaMTX) is DEMOTED to the `builtin` provider — still fully working, still
-- the offline default, but now one adapter among many (Matrix, Jitsi, LiveKit,
-- Owncast/PeerTube).
--
-- Two structural changes land here:
--   1. `comms_rooms` — the provider-agnostic room registry (address + scope +
--      which provider owns it + its OWN media host + optional join price).
--   2. per-node media — `streams.media_host` / `voice_rooms.media_host` kill the
--      assumption of ONE global MEDIA_SERVER_BASE_URL. Every operator runs their
--      own media server; a room/stream record carries its own host.

CREATE TABLE IF NOT EXISTS comms_rooms (
    id            UUID PRIMARY KEY,
    -- Provider-agnostic opaque address, e.g. `builtin://voice/ab12…`,
    -- `matrix://#magnetite-lobby:example.org`, `livekit://room-name`.
    addr          TEXT        NOT NULL UNIQUE,
    -- Which adapter owns this room: builtin | matrix | jitsi | livekit | owncast.
    provider      TEXT        NOT NULL DEFAULT 'builtin',
    -- match | lobby | community | voice | video | stream
    scope         TEXT        NOT NULL,
    -- Scope payload: game hash for `match`, community id for `community`.
    scope_ref     TEXT,
    -- Per-node media: the host serving THIS room (Jitsi/LiveKit/Owncast/MediaMTX
    -- base URL). NULL means "ask the provider's configured default".
    media_host    TEXT,
    -- Local rows this room fronts, when it wraps the demoted builtin stack.
    community_id  UUID,
    channel_id    UUID,
    -- Receipt-gated join (§3.5 + §3.6): >0 means a verified, non-voided
    -- payment receipt for this room is required before a JoinCred is minted.
    price_units   BIGINT      NOT NULL DEFAULT 0,
    created_by    UUID REFERENCES users (id) ON DELETE SET NULL,
    closed_at     TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_comms_rooms_scope     ON comms_rooms (scope, scope_ref);
CREATE INDEX IF NOT EXISTS idx_comms_rooms_provider  ON comms_rooms (provider);
CREATE INDEX IF NOT EXISTS idx_comms_rooms_community ON comms_rooms (community_id) WHERE community_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_comms_rooms_open      ON comms_rooms (created_at DESC) WHERE closed_at IS NULL;

-- ── Per-node media hosts (no single global MEDIA_SERVER_BASE_URL) ────────────
ALTER TABLE streams     ADD COLUMN IF NOT EXISTS media_host TEXT;
ALTER TABLE voice_rooms ADD COLUMN IF NOT EXISTS media_host TEXT;

-- Which provider is fronting a given stream / voice room (default: demoted builtin).
ALTER TABLE streams     ADD COLUMN IF NOT EXISTS comms_provider TEXT NOT NULL DEFAULT 'builtin';
ALTER TABLE voice_rooms ADD COLUMN IF NOT EXISTS comms_provider TEXT NOT NULL DEFAULT 'builtin';

-- Link a stream / voice room to its seam-level room address, when one exists.
ALTER TABLE streams     ADD COLUMN IF NOT EXISTS room_addr TEXT;
ALTER TABLE voice_rooms ADD COLUMN IF NOT EXISTS room_addr TEXT;

COMMENT ON TABLE comms_rooms IS
    'Provider-agnostic room registry for the CommsProvider seam (§3.5). '
    'The builtin (in-house chat/voice/streaming) stack is one provider among '
    'Matrix / Jitsi / LiveKit / Owncast, not the only path.';
COMMENT ON COLUMN streams.media_host IS
    'Per-node media host for this stream. Replaces the global MEDIA_SERVER_BASE_URL '
    'assumption — every operator runs their own media server.';
