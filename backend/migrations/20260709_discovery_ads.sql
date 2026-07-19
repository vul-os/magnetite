-- Discovery tracker soft state (DECENTRALIZATION.md §3.4).
--
-- This table is a PHONEBOOK CACHE, not a source of truth. Every row is a lease
-- a hosting node must renew by heartbeat; rows lapse at `expires_at` and are
-- swept on the next announce. Losing this table costs nothing but a few minutes
-- of re-announcement — which is the point: a tracker holds no authority over
-- who may host what.
--
-- Authorship is enforced by signature at the API layer (`SignedAd`), and the
-- `(game_hash, node_addr)` unique key plus a node_key equality predicate on
-- UPDATE means one node can never overwrite another's listing.

CREATE TABLE IF NOT EXISTS discovery_ads (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Content address of the game (BLAKE3 hex). Two nodes advertising the same
    -- hash are provably running the same build.
    game_hash       TEXT   NOT NULL,
    -- Opaque reach address for the hosting node (e.g. host:port).
    node_addr       TEXT   NOT NULL,
    -- Ed25519 public key (hex) that signed the ad. Owns this slot.
    node_key        TEXT   NOT NULL,

    -- Self-measured capacity (§4 — player cap is emergent, never a constant).
    cpu_cores       INTEGER NOT NULL,
    ram_mb          BIGINT  NOT NULL,
    bandwidth_mbps  INTEGER NOT NULL,
    free_slots      INTEGER NOT NULL,
    max_shards      INTEGER NOT NULL,

    ping_hint       INTEGER NOT NULL DEFAULT 0,

    -- Optional price hint. Settlement is the PaymentRail seam's business; the
    -- tracker only repeats what the node advertises.
    price_amount    BIGINT,
    price_currency  TEXT,
    price_unit      TEXT,

    -- Optional pre-provisioned comms rooms (CommsProvider seam addresses).
    chat_room       TEXT,
    voice_room      TEXT,

    -- Unsigned display counters for the server browser.
    players         INTEGER,
    max_players     INTEGER,

    -- Lease window, unix seconds, as signed by the node.
    issued_at       BIGINT NOT NULL,
    expires_at      BIGINT NOT NULL,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT discovery_ads_slot_unique UNIQUE (game_hash, node_addr)
);

-- The server-browser query: live ads for a game, cheapest ping first.
CREATE INDEX IF NOT EXISTS idx_discovery_ads_game_live
    ON discovery_ads (game_hash, expires_at DESC);
-- The expiry sweep.
CREATE INDEX IF NOT EXISTS idx_discovery_ads_expires
    ON discovery_ads (expires_at);
