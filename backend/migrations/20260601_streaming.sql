-- Streaming lifecycle — adds fields to the `streams` table created in
-- 20260530_communities.sql.  If you are running from scratch those fields are
-- already present, so every statement uses ALTER … ADD COLUMN IF NOT EXISTS.
--
-- New fields:
--   ingest_key     — secret key used by the broadcaster to authenticate the
--                    ingest endpoint (MediaMTX / nginx-rtmp / rtmp-server);
--                    generated server-side on go-live, never returned to the
--                    frontend in list responses.
--   rtmp_target    — optional external RTMP destination URL (e.g. Twitch ingest)
--   stream_key     — external stream key for rtmp_target (Twitch / YouTube)
--   hls_url        — HLS manifest URL served by the media server; populated
--                    once the broadcaster's RTMP ingest is detected as live.
--   viewer_count   — already present; kept here for documentation clarity.
--   game_id        — already present (nullable FK to games).
--
-- Media-server dependency note:
--   Magnetite does NOT bundle a media server.  The recommended self-hosted
--   stack is MediaMTX (https://github.com/bluenviron/mediamtx) which accepts
--   RTMP/RTSP/HLS/WebRTC ingest and outputs HLS for viewers.  In production
--   a CDN (Cloudflare Stream, Mux, or AWS MediaLive) replaces this.
--   See docs/streaming.md for the full architecture diagram.

ALTER TABLE streams ADD COLUMN IF NOT EXISTS ingest_key TEXT;
ALTER TABLE streams ADD COLUMN IF NOT EXISTS rtmp_target TEXT;
ALTER TABLE streams ADD COLUMN IF NOT EXISTS stream_key  TEXT;

-- Composite index: list live streams by community
CREATE INDEX IF NOT EXISTS idx_streams_community_live
    ON streams(community_id, status)
    WHERE status = 'live';

-- Composite index: list live streams by game
CREATE INDEX IF NOT EXISTS idx_streams_game_live
    ON streams(game_id, status)
    WHERE status = 'live' AND game_id IS NOT NULL;
