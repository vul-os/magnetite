-- Magnetite — baseline schema (folded).
--
-- The 47-file migration history (2025-01 → 2026-07) is collapsed into this one
-- forward-only baseline. Magnetite has no production database, and its old
-- migration chain could no longer initialise a fresh database at all: the real
-- runner is sqlx::migrate! (src/db/pool.rs), which runs each file in a
-- transaction and aborts on error, and 20260605_..._search_leaderboard.sql
-- adds a generated games.search_vector column referencing a genre column that
-- was never added to games. (The vestigial tools/migrate.sh masked this by
-- piping to psql with 2>/dev/null and no ON_ERROR_STOP.)
--
-- This baseline is the intended schema those migrations describe: every
-- constraint inlined into its CREATE TABLE, the only trailing ALTERs closing
-- table reference cycles. It is verified to apply cleanly under a strict
-- runner and to reproduce, byte-for-byte, the schema the old chain produced.
--
-- Deliberately NOT invented here (flagged for a product decision, see ROADMAP):
--   * games.genre + games.search_vector (a generated tsvector over genre) —
--     the genre column was never added, so full-text search has never been
--     indexed. Restoring it is a schema + product change, not a mechanical fold.
--   * a wallet platform_settings seed insert referenced a non-existent
--     description column; that is seed data, not schema, so out of scope here.
--
-- Applied by sqlx::migrate! on a fresh database.

-- ── Extensions ──────────────────────────────────────────────────────────────
CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;

-- ── Functions ───────────────────────────────────────────────────────────────
CREATE FUNCTION fn_sync_helpful_count() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE reviews SET helpful_count = helpful_count + 1 WHERE id = NEW.review_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE reviews SET helpful_count = GREATEST(helpful_count - 1, 0) WHERE id = OLD.review_id;
    END IF;
    RETURN NULL;
END;
$$;

-- ── Sequences ───────────────────────────────────────────────────────────────
CREATE SEQUENCE _migrations_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;
CREATE SEQUENCE analytics_events_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;
CREATE SEQUENCE superadmin_audit_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

-- ── Tables ──────────────────────────────────────────────────────────────────
CREATE TABLE _migrations (
    id integer NOT NULL DEFAULT nextval('_migrations_id_seq'::regclass),
    name text NOT NULL,
    executed_at timestamp with time zone DEFAULT now(),
    CONSTRAINT _migrations_pkey PRIMARY KEY (id),
    CONSTRAINT _migrations_name_key UNIQUE (name)
);
ALTER SEQUENCE _migrations_id_seq OWNED BY _migrations.id;

CREATE TABLE achievements (
    id uuid NOT NULL,
    name text NOT NULL,
    description text,
    icon text,
    category text,
    threshold integer NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT achievements_pkey PRIMARY KEY (id)
);

CREATE TABLE users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    username character varying(100) NOT NULL,
    email character varying(255) NOT NULL,
    password_hash character varying(255) NOT NULL,
    is_developer boolean DEFAULT false NOT NULL,
    is_admin boolean DEFAULT false NOT NULL,
    is_banned boolean DEFAULT false NOT NULL,
    banned_at timestamp with time zone,
    ban_reason text,
    google_id text,
    discord_id text,
    github_id text,
    gitlab_id text,
    avatar_url text,
    email_verified boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    bio text,
    location text,
    totp_secret character varying(256),
    totp_enabled boolean DEFAULT false NOT NULL,
    wallet_address text,
    CONSTRAINT users_pkey PRIMARY KEY (id),
    CONSTRAINT users_discord_id_key UNIQUE (discord_id),
    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_github_id_key UNIQUE (github_id),
    CONSTRAINT users_gitlab_id_key UNIQUE (gitlab_id),
    CONSTRAINT users_google_id_key UNIQUE (google_id),
    CONSTRAINT users_username_key UNIQUE (username)
);

CREATE TABLE admin_actions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    admin_id uuid NOT NULL,
    action_type text NOT NULL,
    target_type text,
    target_id uuid,
    details jsonb,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT admin_actions_pkey PRIMARY KEY (id),
    CONSTRAINT admin_actions_admin_id_fkey FOREIGN KEY (admin_id) REFERENCES users(id)
);

CREATE TABLE analytics_events (
    id bigint NOT NULL DEFAULT nextval('analytics_events_id_seq'::regclass),
    occurred_at timestamp with time zone DEFAULT now() NOT NULL,
    ip text,
    country text,
    region text,
    city text,
    method text NOT NULL,
    path text NOT NULL,
    status integer NOT NULL,
    duration_ms integer,
    user_id uuid,
    user_agent text,
    referer text,
    CONSTRAINT analytics_events_pkey PRIMARY KEY (id),
    CONSTRAINT analytics_events_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);
ALTER SEQUENCE analytics_events_id_seq OWNED BY analytics_events.id;

CREATE TABLE anti_cheat_bans (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    reason text,
    fingerprint text,
    banned_at timestamp with time zone DEFAULT now(),
    expires_at timestamp with time zone,
    CONSTRAINT anti_cheat_bans_pkey PRIMARY KEY (id),
    CONSTRAINT anti_cheat_bans_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE api_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    key_hash character varying(255) NOT NULL,
    prefix character varying(16) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone,
    revoked_at timestamp with time zone,
    CONSTRAINT api_keys_pkey PRIMARY KEY (id),
    CONSTRAINT api_keys_key_hash_key UNIQUE (key_hash),
    CONSTRAINT api_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE blocked_users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    blocked_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT unique_block CHECK ((user_id <> blocked_id)),
    CONSTRAINT blocked_users_pkey PRIMARY KEY (id),
    CONSTRAINT blocked_users_blocked_id_fkey FOREIGN KEY (blocked_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT blocked_users_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE categories (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    slug character varying(100) NOT NULL,
    icon character varying(50),
    description text,
    sort_order integer DEFAULT 0,
    CONSTRAINT categories_pkey PRIMARY KEY (id),
    CONSTRAINT categories_slug_key UNIQUE (slug)
);

CREATE TABLE games (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    developer_id uuid NOT NULL,
    github_repo text NOT NULL,
    title character varying(255) NOT NULL,
    description text,
    status character varying(50) DEFAULT 'draft'::character varying NOT NULL,
    active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    featured_at timestamp with time zone,
    reviewed_at timestamp with time zone,
    reviewed_by uuid,
    category_id uuid,
    subscription_tier_required text DEFAULT 'free'::text,
    template_id text,
    content_rating text DEFAULT 'everyone'::text NOT NULL,
    CONSTRAINT chk_games_content_rating CHECK ((content_rating = ANY (ARRAY['everyone'::text, 'teen'::text, 'mature'::text]))),
    CONSTRAINT games_pkey PRIMARY KEY (id),
    CONSTRAINT games_category_id_fkey FOREIGN KEY (category_id) REFERENCES categories(id),
    CONSTRAINT games_developer_id_fkey FOREIGN KEY (developer_id) REFERENCES users(id),
    CONSTRAINT games_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES users(id)
);

CREATE TABLE build_status (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    repository text NOT NULL,
    commit_sha text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    conclusion text,
    build_logs text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    game_id uuid,
    runner_token uuid,
    CONSTRAINT build_status_pkey PRIMARY KEY (id),
    CONSTRAINT build_status_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE SET NULL
);

CREATE TABLE build_logs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    build_id uuid,
    step text NOT NULL,
    output text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT build_logs_pkey PRIMARY KEY (id),
    CONSTRAINT build_logs_build_id_fkey FOREIGN KEY (build_id) REFERENCES build_status(id)
);

CREATE TABLE communities (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    owner_id uuid NOT NULL,
    name text NOT NULL,
    slug text NOT NULL,
    description text,
    icon_url text,
    banner_url text,
    is_public boolean DEFAULT true NOT NULL,
    member_count integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT communities_pkey PRIMARY KEY (id),
    CONSTRAINT communities_slug_key UNIQUE (slug),
    CONSTRAINT communities_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE channels (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    community_id uuid NOT NULL,
    name text NOT NULL,
    kind text DEFAULT 'text'::text NOT NULL,
    topic text,
    "position" integer DEFAULT 0 NOT NULL,
    is_private boolean DEFAULT false NOT NULL,
    slow_mode_secs integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT channels_pkey PRIMARY KEY (id),
    CONSTRAINT channels_community_id_fkey FOREIGN KEY (community_id) REFERENCES communities(id) ON DELETE CASCADE
);

CREATE TABLE channel_members (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    channel_id uuid NOT NULL,
    user_id uuid NOT NULL,
    can_write boolean DEFAULT true NOT NULL,
    added_at timestamp with time zone DEFAULT now(),
    CONSTRAINT channel_members_pkey PRIMARY KEY (id),
    CONSTRAINT channel_members_channel_id_user_id_key UNIQUE (channel_id, user_id),
    CONSTRAINT channel_members_channel_id_fkey FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE,
    CONSTRAINT channel_members_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE chat_flags (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    channel_id uuid NOT NULL,
    author_id uuid NOT NULL,
    content text NOT NULL,
    flag_reasons text DEFAULT ''::text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    resolved_by uuid,
    resolved_at timestamp with time zone,
    resolution_note text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT chk_chat_flags_status CHECK ((status = ANY (ARRAY['pending'::text, 'dismissed'::text, 'resolved'::text]))),
    CONSTRAINT chat_flags_pkey PRIMARY KEY (id),
    CONSTRAINT chat_flags_author_id_fkey FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT chat_flags_channel_id_fkey FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE,
    CONSTRAINT chat_flags_resolved_by_fkey FOREIGN KEY (resolved_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE cicd_pipelines (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    repository text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    commit_sha text,
    pr_number integer,
    triggered_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cicd_pipelines_pkey PRIMARY KEY (id)
);

CREATE TABLE comms_rooms (
    id uuid NOT NULL,
    addr text NOT NULL,
    provider text DEFAULT 'builtin'::text NOT NULL,
    scope text NOT NULL,
    scope_ref text,
    media_host text,
    community_id uuid,
    channel_id uuid,
    price_units bigint DEFAULT 0 NOT NULL,
    created_by uuid,
    closed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT comms_rooms_pkey PRIMARY KEY (id),
    CONSTRAINT comms_rooms_addr_key UNIQUE (addr),
    CONSTRAINT comms_rooms_created_by_fkey FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE community_members (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    community_id uuid NOT NULL,
    user_id uuid NOT NULL,
    role text DEFAULT 'member'::text NOT NULL,
    nickname text,
    joined_at timestamp with time zone DEFAULT now(),
    CONSTRAINT community_members_pkey PRIMARY KEY (id),
    CONSTRAINT community_members_community_id_user_id_key UNIQUE (community_id, user_id),
    CONSTRAINT community_members_community_id_fkey FOREIGN KEY (community_id) REFERENCES communities(id) ON DELETE CASCADE,
    CONSTRAINT community_members_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE contact_messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    email text NOT NULL,
    subject text NOT NULL,
    message text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT contact_messages_pkey PRIMARY KEY (id)
);

CREATE TABLE deployments (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    repository text NOT NULL,
    commit_sha text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    deployed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT deployments_pkey PRIMARY KEY (id)
);

CREATE TABLE dev_stores (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    developer_id uuid NOT NULL,
    name text NOT NULL,
    description text,
    active boolean DEFAULT true NOT NULL,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT dev_stores_pkey PRIMARY KEY (id),
    CONSTRAINT dev_stores_game_id_key UNIQUE (game_id),
    CONSTRAINT dev_stores_developer_id_fkey FOREIGN KEY (developer_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT dev_stores_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
);

CREATE TABLE discovery_ads (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_hash text NOT NULL,
    node_addr text NOT NULL,
    node_key text NOT NULL,
    cpu_cores integer NOT NULL,
    ram_mb bigint NOT NULL,
    bandwidth_mbps integer NOT NULL,
    free_slots integer NOT NULL,
    max_shards integer NOT NULL,
    ping_hint integer DEFAULT 0 NOT NULL,
    price_amount bigint,
    price_currency text,
    price_unit text,
    chat_room text,
    voice_room text,
    players integer,
    max_players integer,
    issued_at bigint NOT NULL,
    expires_at bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    operator text,
    region text,
    CONSTRAINT discovery_ads_pkey PRIMARY KEY (id),
    CONSTRAINT discovery_ads_slot_unique UNIQUE (game_hash, node_addr)
);

CREATE TABLE dm_threads (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_a_id uuid NOT NULL,
    user_b_id uuid NOT NULL,
    last_message_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT dm_threads_ordered CHECK ((user_a_id < user_b_id)),
    CONSTRAINT dm_threads_pkey PRIMARY KEY (id),
    CONSTRAINT dm_threads_user_a_id_user_b_id_key UNIQUE (user_a_id, user_b_id),
    CONSTRAINT dm_threads_user_a_id_fkey FOREIGN KEY (user_a_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT dm_threads_user_b_id_fkey FOREIGN KEY (user_b_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE dm_messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    thread_id uuid NOT NULL,
    author_id uuid NOT NULL,
    content text NOT NULL,
    edited_at timestamp with time zone,
    deleted boolean DEFAULT false NOT NULL,
    attachments jsonb,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT dm_messages_pkey PRIMARY KEY (id),
    CONSTRAINT dm_messages_author_id_fkey FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT dm_messages_thread_id_fkey FOREIGN KEY (thread_id) REFERENCES dm_threads(id) ON DELETE CASCADE
);

CREATE TABLE payment_receipts (
    id uuid NOT NULL,
    kind text NOT NULL,
    buyer_id uuid NOT NULL,
    buyer_pubkey text NOT NULL,
    purchase_id uuid,
    item_id uuid,
    game_id uuid,
    total bigint NOT NULL,
    protocol_fee bigint DEFAULT 0 NOT NULL,
    payouts jsonb NOT NULL,
    nonce text NOT NULL,
    rail_pubkey text NOT NULL,
    sig text NOT NULL,
    rail text DEFAULT 'mock'::text NOT NULL,
    voided boolean DEFAULT false NOT NULL,
    voided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    binding jsonb,
    CONSTRAINT payment_receipts_pkey PRIMARY KEY (id),
    CONSTRAINT payment_receipts_buyer_id_fkey FOREIGN KEY (buyer_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE store_items (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    store_id uuid NOT NULL,
    game_id uuid NOT NULL,
    sku text NOT NULL,
    name text NOT NULL,
    description text,
    price numeric(18,6) NOT NULL,
    currency text DEFAULT 'USDC'::text NOT NULL,
    kind text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT store_items_kind_check CHECK ((kind = ANY (ARRAY['cosmetic'::text, 'item'::text, 'dlc'::text, 'pass'::text]))),
    CONSTRAINT store_items_pkey PRIMARY KEY (id),
    CONSTRAINT store_items_store_id_sku_key UNIQUE (store_id, sku),
    CONSTRAINT store_items_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
    CONSTRAINT store_items_store_id_fkey FOREIGN KEY (store_id) REFERENCES dev_stores(id) ON DELETE CASCADE
);

CREATE TABLE store_purchases (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    item_id uuid NOT NULL,
    store_id uuid NOT NULL,
    game_id uuid NOT NULL,
    price_paid numeric(18,6) NOT NULL,
    currency text NOT NULL,
    developer_share numeric(18,6),
    platform_fee numeric(18,6),
    status text DEFAULT 'completed'::text NOT NULL,
    idempotency_key text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now(),
    refunded_at timestamp with time zone,
    refunded_by uuid,
    refund_reason text,
    CONSTRAINT store_purchases_status_check CHECK ((status = ANY (ARRAY['completed'::text, 'refunded'::text, 'failed'::text]))),
    CONSTRAINT store_purchases_pkey PRIMARY KEY (id),
    CONSTRAINT store_purchases_idempotency_key_key UNIQUE (idempotency_key),
    CONSTRAINT store_purchases_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE RESTRICT,
    CONSTRAINT store_purchases_item_id_fkey FOREIGN KEY (item_id) REFERENCES store_items(id) ON DELETE RESTRICT,
    CONSTRAINT store_purchases_refunded_by_fkey FOREIGN KEY (refunded_by) REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT store_purchases_store_id_fkey FOREIGN KEY (store_id) REFERENCES dev_stores(id) ON DELETE RESTRICT,
    CONSTRAINT store_purchases_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE entitlements (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    item_id uuid NOT NULL,
    purchase_id uuid,
    granted_at timestamp with time zone DEFAULT now(),
    expires_at timestamp with time zone,
    revoked boolean DEFAULT false NOT NULL,
    receipt_id uuid,
    CONSTRAINT entitlements_pkey PRIMARY KEY (id),
    CONSTRAINT entitlements_user_id_item_id_key UNIQUE (user_id, item_id),
    CONSTRAINT entitlements_item_id_fkey FOREIGN KEY (item_id) REFERENCES store_items(id) ON DELETE CASCADE,
    CONSTRAINT entitlements_purchase_id_fkey FOREIGN KEY (purchase_id) REFERENCES store_purchases(id) ON DELETE SET NULL,
    CONSTRAINT entitlements_receipt_fk FOREIGN KEY (receipt_id) REFERENCES payment_receipts(id) ON DELETE SET NULL,
    CONSTRAINT entitlements_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE friend_requests (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    from_user_id uuid,
    to_user_id uuid,
    status text DEFAULT 'pending'::text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT unique_friend_request CHECK ((from_user_id <> to_user_id)),
    CONSTRAINT friend_requests_pkey PRIMARY KEY (id),
    CONSTRAINT friend_requests_from_user_id_fkey FOREIGN KEY (from_user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT friend_requests_to_user_id_fkey FOREIGN KEY (to_user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE friendships (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    friend_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT unique_friendship CHECK ((user_id <> friend_id)),
    CONSTRAINT friendships_pkey PRIMARY KEY (id),
    CONSTRAINT friendships_friend_id_fkey FOREIGN KEY (friend_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT friendships_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE game_versions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    version text NOT NULL,
    commit_sha text NOT NULL,
    release_notes text,
    is_live boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT game_versions_pkey PRIMARY KEY (id),
    CONSTRAINT game_versions_game_id_version_key UNIQUE (game_id, version),
    CONSTRAINT game_versions_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
);

CREATE TABLE game_artifacts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    version_id uuid,
    build_id uuid,
    artifact_type text DEFAULT 'wasm'::text NOT NULL,
    artifact_url text,
    file_size_bytes bigint,
    sha256_hash text,
    build_status text DEFAULT 'pending'::text NOT NULL,
    error_message text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT game_artifacts_pkey PRIMARY KEY (id),
    CONSTRAINT game_artifacts_build_id_fkey FOREIGN KEY (build_id) REFERENCES build_status(id) ON DELETE SET NULL,
    CONSTRAINT game_artifacts_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
    CONSTRAINT game_artifacts_version_id_fkey FOREIGN KEY (version_id) REFERENCES game_versions(id) ON DELETE SET NULL
);

CREATE TABLE seasons (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    starts_at timestamp with time zone NOT NULL,
    ends_at timestamp with time zone,
    is_active boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT seasons_pkey PRIMARY KEY (id)
);

CREATE TABLE game_high_scores (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    user_id uuid NOT NULL,
    score bigint NOT NULL,
    recorded_at timestamp with time zone DEFAULT now(),
    season_id uuid,
    CONSTRAINT game_high_scores_pkey PRIMARY KEY (id),
    CONSTRAINT game_high_scores_game_id_user_id_key UNIQUE (game_id, user_id),
    CONSTRAINT game_high_scores_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT game_high_scores_season_id_fkey FOREIGN KEY (season_id) REFERENCES seasons(id) ON DELETE SET NULL,
    CONSTRAINT game_high_scores_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE game_invites (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    from_user_id uuid,
    to_user_id uuid,
    game_id uuid,
    status text DEFAULT 'pending'::text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT unique_game_invite CHECK ((from_user_id <> to_user_id)),
    CONSTRAINT game_invites_pkey PRIMARY KEY (id),
    CONSTRAINT game_invites_from_user_id_fkey FOREIGN KEY (from_user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT game_invites_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
    CONSTRAINT game_invites_to_user_id_fkey FOREIGN KEY (to_user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE play_sessions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    user_id uuid NOT NULL,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    started_at timestamp with time zone DEFAULT now(),
    ended_at timestamp with time zone,
    fee_amount numeric(18,6) DEFAULT 0 NOT NULL,
    final_score bigint,
    payout_status character varying(50) DEFAULT 'pending'::character varying,
    CONSTRAINT play_sessions_pkey PRIMARY KEY (id),
    CONSTRAINT play_sessions_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT play_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE game_revenue (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    developer_id uuid NOT NULL,
    session_id uuid,
    amount numeric(18,6) NOT NULL,
    developer_share numeric(18,6) NOT NULL,
    platform_share numeric(18,6) NOT NULL,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT game_revenue_pkey PRIMARY KEY (id),
    CONSTRAINT game_revenue_developer_id_fkey FOREIGN KEY (developer_id) REFERENCES users(id),
    CONSTRAINT game_revenue_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT game_revenue_session_id_fkey FOREIGN KEY (session_id) REFERENCES play_sessions(id)
);

CREATE TABLE game_scaffolds (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    developer_id uuid NOT NULL,
    template_id text NOT NULL,
    cli_command text NOT NULL,
    template_repo text NOT NULL,
    manifest jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT game_scaffolds_pkey PRIMARY KEY (id),
    CONSTRAINT game_scaffolds_developer_id_fkey FOREIGN KEY (developer_id) REFERENCES users(id),
    CONSTRAINT game_scaffolds_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
);

CREATE TABLE github_installations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    installation_id bigint NOT NULL,
    account_id bigint NOT NULL,
    account_login text NOT NULL,
    account_type text,
    repository_selection text DEFAULT 'all'::text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT github_installations_pkey PRIMARY KEY (id)
);

CREATE TABLE hosting_channels (
    id uuid NOT NULL,
    channel_id text NOT NULL,
    payer_id uuid NOT NULL,
    operator_pubkey text NOT NULL,
    server_id uuid,
    rail_pubkey text NOT NULL,
    open boolean DEFAULT true NOT NULL,
    closed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT hosting_channels_pkey PRIMARY KEY (id),
    CONSTRAINT hosting_channels_channel_id_key UNIQUE (channel_id),
    CONSTRAINT hosting_channels_payer_id_fkey FOREIGN KEY (payer_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE matchmaking_queue (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    game_id uuid NOT NULL,
    status character varying(50) DEFAULT 'waiting'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    matched_at timestamp with time zone,
    CONSTRAINT matchmaking_queue_pkey PRIMARY KEY (id),
    CONSTRAINT matchmaking_queue_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT matchmaking_queue_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    channel_id uuid NOT NULL,
    author_id uuid NOT NULL,
    content text NOT NULL,
    edited_at timestamp with time zone,
    deleted boolean DEFAULT false NOT NULL,
    reply_to_id uuid,
    attachments jsonb,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT messages_pkey PRIMARY KEY (id),
    CONSTRAINT messages_author_id_fkey FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT messages_channel_id_fkey FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE,
    CONSTRAINT messages_reply_to_id_fkey FOREIGN KEY (reply_to_id) REFERENCES messages(id) ON DELETE SET NULL
);

CREATE TABLE notification_preferences (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    payouts_email boolean DEFAULT true NOT NULL,
    payouts_in_app boolean DEFAULT true NOT NULL,
    payouts_push boolean DEFAULT true NOT NULL,
    social_email boolean DEFAULT true NOT NULL,
    social_in_app boolean DEFAULT true NOT NULL,
    social_push boolean DEFAULT true NOT NULL,
    achievements_email boolean DEFAULT true NOT NULL,
    achievements_in_app boolean DEFAULT true NOT NULL,
    achievements_push boolean DEFAULT false NOT NULL,
    marketing_email boolean DEFAULT false NOT NULL,
    marketing_in_app boolean DEFAULT false NOT NULL,
    marketing_push boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT notification_preferences_pkey PRIMARY KEY (id),
    CONSTRAINT notification_preferences_user_id_key UNIQUE (user_id),
    CONSTRAINT notification_preferences_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE notifications (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    type character varying(50) NOT NULL,
    title text NOT NULL,
    body text,
    data jsonb,
    read boolean DEFAULT false,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT notifications_pkey PRIMARY KEY (id),
    CONSTRAINT notifications_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE oauth_identities (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    provider character varying(32) NOT NULL,
    provider_id character varying(255) NOT NULL,
    email character varying(255),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT oauth_identities_pkey PRIMARY KEY (id),
    CONSTRAINT oauth_identities_provider_provider_id_key UNIQUE (provider, provider_id),
    CONSTRAINT oauth_identities_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE payouts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    amount numeric(18,6) NOT NULL,
    currency text DEFAULT 'USDC'::text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    destination text NOT NULL,
    processed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT payouts_pkey PRIMARY KEY (id),
    CONSTRAINT payouts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE platform_settings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    key character varying(100) NOT NULL,
    value text NOT NULL,
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT platform_settings_pkey PRIMARY KEY (id),
    CONSTRAINT platform_settings_key_key UNIQUE (key)
);

CREATE TABLE point_balances (
    user_id uuid NOT NULL,
    balance bigint DEFAULT 0 NOT NULL,
    season_id uuid,
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT point_balances_pkey PRIMARY KEY (user_id),
    CONSTRAINT point_balances_season_id_fkey FOREIGN KEY (season_id) REFERENCES seasons(id) ON DELETE SET NULL,
    CONSTRAINT point_balances_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE point_rewards (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    description text,
    kind text NOT NULL,
    points bigint NOT NULL,
    game_id uuid,
    active boolean DEFAULT true NOT NULL,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT point_rewards_kind_check CHECK ((kind = ANY (ARRAY['earn'::text, 'redeem'::text]))),
    CONSTRAINT point_rewards_pkey PRIMARY KEY (id),
    CONSTRAINT point_rewards_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
);

CREATE TABLE points_ledger (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    delta bigint NOT NULL,
    reason text NOT NULL,
    game_id uuid,
    season_id uuid,
    balance_snapshot bigint NOT NULL,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT points_ledger_pkey PRIMARY KEY (id),
    CONSTRAINT points_ledger_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE SET NULL,
    CONSTRAINT points_ledger_season_id_fkey FOREIGN KEY (season_id) REFERENCES seasons(id) ON DELETE SET NULL,
    CONSTRAINT points_ledger_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE presence (
    user_id uuid NOT NULL,
    status text DEFAULT 'online'::text NOT NULL,
    activity text,
    game_id uuid,
    last_seen timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT presence_pkey PRIMARY KEY (user_id),
    CONSTRAINT presence_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE SET NULL,
    CONSTRAINT presence_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE pull_request_tests (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    repository text NOT NULL,
    pr_number integer NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    conclusion text,
    created_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone,
    CONSTRAINT pull_request_tests_pkey PRIMARY KEY (id)
);

CREATE TABLE wallet_transactions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    tx_type character varying(50) NOT NULL,
    amount numeric(18,6) NOT NULL,
    reference_id character varying(255),
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    payout_status character varying(50) DEFAULT NULL::character varying,
    payout_amount numeric(18,6) DEFAULT NULL::numeric,
    currency character varying(10) DEFAULT 'USD'::character varying NOT NULL,
    idempotency_key character varying(255),
    related_transaction_id uuid,
    CONSTRAINT wallet_transactions_pkey PRIMARY KEY (id),
    CONSTRAINT wallet_transactions_related_transaction_id_fkey FOREIGN KEY (related_transaction_id) REFERENCES wallet_transactions(id),
    CONSTRAINT wallet_transactions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE refund_records (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    transaction_id uuid NOT NULL,
    user_id uuid NOT NULL,
    admin_id uuid NOT NULL,
    amount numeric(18,6) NOT NULL,
    provider text DEFAULT 'none'::text NOT NULL,
    provider_ref text,
    status text DEFAULT 'pending'::text NOT NULL,
    reason text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT refund_records_pkey PRIMARY KEY (id),
    CONSTRAINT refund_records_admin_id_fkey FOREIGN KEY (admin_id) REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT refund_records_transaction_id_fkey FOREIGN KEY (transaction_id) REFERENCES wallet_transactions(id) ON DELETE RESTRICT,
    CONSTRAINT refund_records_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE registered_games (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid,
    github_installation_id uuid,
    repo_full_name text NOT NULL,
    default_branch text DEFAULT 'main'::text,
    last_synced_at timestamp with time zone,
    build_status text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT registered_games_pkey PRIMARY KEY (id),
    CONSTRAINT registered_games_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT registered_games_github_installation_id_fkey FOREIGN KEY (github_installation_id) REFERENCES github_installations(id)
);

CREATE TABLE replays (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    match_id uuid,
    recorded_by uuid NOT NULL,
    replay_json jsonb NOT NULL,
    state_hash_final bigint,
    duration_ticks bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT replays_pkey PRIMARY KEY (id),
    CONSTRAINT replays_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT replays_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES users(id)
);

CREATE TABLE reviews (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    game_id uuid NOT NULL,
    rating integer NOT NULL,
    content text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    helpful_count integer DEFAULT 0 NOT NULL,
    CONSTRAINT reviews_rating_check CHECK (((rating >= 1) AND (rating <= 5))),
    CONSTRAINT reviews_pkey PRIMARY KEY (id),
    CONSTRAINT reviews_user_id_game_id_key UNIQUE (user_id, game_id),
    CONSTRAINT reviews_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
    CONSTRAINT reviews_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE review_helpful (
    review_id uuid NOT NULL,
    user_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT review_helpful_pkey PRIMARY KEY (review_id, user_id),
    CONSTRAINT review_helpful_review_id_fkey FOREIGN KEY (review_id) REFERENCES reviews(id) ON DELETE CASCADE,
    CONSTRAINT review_helpful_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE review_reports (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    review_id uuid NOT NULL,
    reporter_id uuid,
    reason text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    resolved_by uuid,
    resolved_at timestamp with time zone,
    resolution_note text,
    source text DEFAULT 'user'::text NOT NULL,
    CONSTRAINT chk_review_reports_source CHECK ((source = ANY (ARRAY['user'::text, 'auto_flag'::text]))),
    CONSTRAINT chk_review_reports_status CHECK ((status = ANY (ARRAY['pending'::text, 'dismissed'::text, 'resolved'::text]))),
    CONSTRAINT review_reports_pkey PRIMARY KEY (id),
    CONSTRAINT review_reports_review_id_reporter_id_reason_key UNIQUE (review_id, reporter_id, reason),
    CONSTRAINT review_reports_reporter_id_fkey FOREIGN KEY (reporter_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT review_reports_resolved_by_fkey FOREIGN KEY (resolved_by) REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT review_reports_review_id_fkey FOREIGN KEY (review_id) REFERENCES reviews(id) ON DELETE CASCADE
);

CREATE TABLE runtime_instances (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    version_id uuid,
    artifact_id uuid,
    status text DEFAULT 'pending'::text NOT NULL,
    ws_endpoint text,
    topology text DEFAULT 'SingleRoom'::text NOT NULL,
    max_players integer DEFAULT 4 NOT NULL,
    tick_hz integer DEFAULT 20 NOT NULL,
    local_pid integer,
    runner_note text,
    requested_by uuid,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT runtime_instances_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'running'::text, 'stopped'::text, 'failed'::text]))),
    CONSTRAINT runtime_instances_pkey PRIMARY KEY (id),
    CONSTRAINT runtime_instances_artifact_id_fkey FOREIGN KEY (artifact_id) REFERENCES game_artifacts(id) ON DELETE SET NULL,
    CONSTRAINT runtime_instances_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
    CONSTRAINT runtime_instances_requested_by_fkey FOREIGN KEY (requested_by) REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT runtime_instances_version_id_fkey FOREIGN KEY (version_id) REFERENCES game_versions(id) ON DELETE SET NULL
);

CREATE TABLE scores (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    game_id uuid NOT NULL,
    user_id uuid NOT NULL,
    score bigint DEFAULT 0 NOT NULL,
    recorded_at timestamp with time zone DEFAULT now(),
    CONSTRAINT scores_pkey PRIMARY KEY (id),
    CONSTRAINT scores_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT scores_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE session_replays (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    session_id uuid,
    inputs jsonb,
    recorded_at timestamp with time zone DEFAULT now(),
    CONSTRAINT session_replays_pkey PRIMARY KEY (id),
    CONSTRAINT session_replays_session_id_fkey FOREIGN KEY (session_id) REFERENCES play_sessions(id)
);

CREATE TABLE sessions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    refresh_token_hash text NOT NULL,
    user_agent text,
    ip_address text,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    token_prefix character varying(32),
    CONSTRAINT sessions_pkey PRIMARY KEY (id),
    CONSTRAINT sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE streams (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    streamer_id uuid NOT NULL,
    community_id uuid,
    channel_id uuid,
    title text NOT NULL,
    game_id uuid,
    status text DEFAULT 'offline'::text NOT NULL,
    viewer_count integer DEFAULT 0 NOT NULL,
    hls_url text,
    rtmp_key text,
    external_rtmp_url text,
    started_at timestamp with time zone,
    ended_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    ingest_key text,
    rtmp_target text,
    stream_key text,
    media_host text,
    comms_provider text DEFAULT 'builtin'::text NOT NULL,
    room_addr text,
    CONSTRAINT streams_pkey PRIMARY KEY (id),
    CONSTRAINT streams_channel_id_fkey FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE SET NULL,
    CONSTRAINT streams_community_id_fkey FOREIGN KEY (community_id) REFERENCES communities(id) ON DELETE SET NULL,
    CONSTRAINT streams_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE SET NULL,
    CONSTRAINT streams_streamer_id_fkey FOREIGN KEY (streamer_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE subscription_tiers (
    id uuid NOT NULL,
    name character varying(100) NOT NULL,
    slug character varying(50) NOT NULL,
    price_usdc numeric(10,2) DEFAULT 0 NOT NULL,
    price_zar numeric(10,2) DEFAULT 0 NOT NULL,
    features jsonb DEFAULT '{}'::jsonb NOT NULL,
    max_games integer,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT subscription_tiers_pkey PRIMARY KEY (id),
    CONSTRAINT subscription_tiers_slug_key UNIQUE (slug)
);

CREATE TABLE user_subscriptions (
    id uuid NOT NULL,
    user_id uuid,
    tier_id uuid,
    status text NOT NULL,
    current_period_start timestamp with time zone,
    current_period_end timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    payment_provider text DEFAULT 'free'::text NOT NULL,
    provider_subscription_id text,
    cancel_at_period_end boolean DEFAULT false NOT NULL,
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT user_subscriptions_pkey PRIMARY KEY (id),
    CONSTRAINT user_subscriptions_tier_id_fkey FOREIGN KEY (tier_id) REFERENCES subscription_tiers(id),
    CONSTRAINT user_subscriptions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE subscription_transactions (
    id uuid NOT NULL,
    user_subscription_id uuid,
    amount numeric(10,2) NOT NULL,
    currency text NOT NULL,
    status text NOT NULL,
    payment_provider text,
    payment_id text,
    created_at timestamp with time zone DEFAULT now(),
    transaction_type text DEFAULT 'subscribe'::text NOT NULL,
    CONSTRAINT subscription_transactions_pkey PRIMARY KEY (id),
    CONSTRAINT subscription_transactions_user_subscription_id_fkey FOREIGN KEY (user_subscription_id) REFERENCES user_subscriptions(id)
);

CREATE TABLE superadmin_audit_log (
    id bigint NOT NULL DEFAULT nextval('superadmin_audit_log_id_seq'::regclass),
    occurred_at timestamp with time zone DEFAULT now() NOT NULL,
    actor_email text NOT NULL,
    actor_ip text,
    action text NOT NULL,
    target text,
    detail text,
    outcome text DEFAULT 'ok'::text NOT NULL,
    CONSTRAINT superadmin_audit_log_pkey PRIMARY KEY (id)
);
ALTER SEQUENCE superadmin_audit_log_id_seq OWNED BY superadmin_audit_log.id;

CREATE TABLE tournaments (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    game_id uuid NOT NULL,
    status character varying(50) DEFAULT 'draft'::character varying NOT NULL,
    max_players integer DEFAULT 8 NOT NULL,
    entry_fee numeric(18,6),
    prize_pool numeric(18,6) DEFAULT 0 NOT NULL,
    start_time timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT tournaments_pkey PRIMARY KEY (id),
    CONSTRAINT tournaments_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id)
);

CREATE TABLE tournament_matches (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tournament_id uuid NOT NULL,
    round integer NOT NULL,
    match_number integer NOT NULL,
    player1_id uuid,
    player2_id uuid,
    winner_id uuid,
    player1_score integer,
    player2_score integer,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    scheduled_at timestamp with time zone,
    completed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    replay_id uuid,
    CONSTRAINT tournament_matches_pkey PRIMARY KEY (id),
    CONSTRAINT tournament_matches_player1_id_fkey FOREIGN KEY (player1_id) REFERENCES users(id),
    CONSTRAINT tournament_matches_player2_id_fkey FOREIGN KEY (player2_id) REFERENCES users(id),
    CONSTRAINT tournament_matches_replay_id_fkey FOREIGN KEY (replay_id) REFERENCES replays(id),
    CONSTRAINT tournament_matches_tournament_id_fkey FOREIGN KEY (tournament_id) REFERENCES tournaments(id) ON DELETE CASCADE,
    CONSTRAINT tournament_matches_winner_id_fkey FOREIGN KEY (winner_id) REFERENCES users(id)
);

CREATE TABLE tournament_participants (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tournament_id uuid NOT NULL,
    user_id uuid NOT NULL,
    registered_at timestamp with time zone DEFAULT now(),
    status character varying(50) DEFAULT 'registered'::character varying NOT NULL,
    seed integer,
    CONSTRAINT tournament_participants_pkey PRIMARY KEY (id),
    CONSTRAINT tournament_participants_tournament_id_user_id_key UNIQUE (tournament_id, user_id),
    CONSTRAINT tournament_participants_tournament_id_fkey FOREIGN KEY (tournament_id) REFERENCES tournaments(id) ON DELETE CASCADE,
    CONSTRAINT tournament_participants_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE transactions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    game_id uuid,
    type character varying(50) NOT NULL,
    amount numeric(10,6) NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT transactions_pkey PRIMARY KEY (id),
    CONSTRAINT transactions_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id),
    CONSTRAINT transactions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE user_achievements (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    achievement_id uuid NOT NULL,
    progress integer DEFAULT 0,
    unlocked_at timestamp with time zone,
    CONSTRAINT user_achievements_pkey PRIMARY KEY (id),
    CONSTRAINT user_achievements_user_id_achievement_id_key UNIQUE (user_id, achievement_id),
    CONSTRAINT user_achievements_achievement_id_fkey FOREIGN KEY (achievement_id) REFERENCES achievements(id) ON DELETE CASCADE,
    CONSTRAINT user_achievements_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE verification_tokens (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    token character varying(255) NOT NULL,
    token_type character varying(50) NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT verification_tokens_pkey PRIMARY KEY (id),
    CONSTRAINT verification_tokens_token_key UNIQUE (token),
    CONSTRAINT verification_tokens_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE voice_rooms (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    channel_id uuid,
    game_session_id uuid,
    room_token text NOT NULL,
    max_participants integer DEFAULT 16 NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    ended_at timestamp with time zone,
    media_host text,
    comms_provider text DEFAULT 'builtin'::text NOT NULL,
    room_addr text,
    CONSTRAINT voice_rooms_pkey PRIMARY KEY (id),
    CONSTRAINT voice_rooms_room_token_key UNIQUE (room_token),
    CONSTRAINT voice_rooms_channel_id_fkey FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
);

CREATE TABLE voice_participants (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    room_id uuid NOT NULL,
    user_id uuid NOT NULL,
    is_muted boolean DEFAULT false NOT NULL,
    is_deafened boolean DEFAULT false NOT NULL,
    is_video boolean DEFAULT false NOT NULL,
    joined_at timestamp with time zone DEFAULT now(),
    left_at timestamp with time zone,
    CONSTRAINT voice_participants_pkey PRIMARY KEY (id),
    CONSTRAINT voice_participants_room_id_user_id_key UNIQUE (room_id, user_id),
    CONSTRAINT voice_participants_room_id_fkey FOREIGN KEY (room_id) REFERENCES voice_rooms(id) ON DELETE CASCADE,
    CONSTRAINT voice_participants_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE wallet_balances (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    currency character varying(10) DEFAULT 'USD'::character varying NOT NULL,
    balance numeric(18,6) DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT wallet_balances_pkey PRIMARY KEY (id),
    CONSTRAINT wallet_balances_user_id_currency_key UNIQUE (user_id, currency),
    CONSTRAINT wallet_balances_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE wise_recipients (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    developer_id uuid NOT NULL,
    wise_recipient_id text NOT NULL,
    currency text DEFAULT 'USD'::text NOT NULL,
    account_holder_name text NOT NULL,
    detail jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT wise_recipients_pkey PRIMARY KEY (id),
    CONSTRAINT wise_recipients_developer_id_fkey FOREIGN KEY (developer_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE wishlists (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    game_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT wishlists_pkey PRIMARY KEY (id),
    CONSTRAINT wishlists_user_id_game_id_key UNIQUE (user_id, game_id),
    CONSTRAINT wishlists_game_id_fkey FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
    CONSTRAINT wishlists_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- ── Indexes ─────────────────────────────────────────────────────────────────
CREATE INDEX game_high_scores_season_idx ON game_high_scores USING btree (game_id, season_id, score DESC);
CREATE INDEX idx_admin_actions_admin_id ON admin_actions USING btree (admin_id);
CREATE INDEX idx_admin_actions_created_at ON admin_actions USING btree (created_at);
CREATE INDEX idx_admin_actions_target_id ON admin_actions USING btree (target_id);
CREATE INDEX idx_analytics_events_country ON analytics_events USING btree (country);
CREATE INDEX idx_analytics_events_occurred_at ON analytics_events USING btree (occurred_at DESC);
CREATE INDEX idx_analytics_events_path ON analytics_events USING btree (path);
CREATE INDEX idx_analytics_events_user_id ON analytics_events USING btree (user_id);
CREATE INDEX idx_anti_cheat_bans_expires_at ON anti_cheat_bans USING btree (expires_at);
CREATE INDEX idx_anti_cheat_bans_fingerprint ON anti_cheat_bans USING btree (fingerprint);
CREATE INDEX idx_anti_cheat_bans_user_id ON anti_cheat_bans USING btree (user_id);
CREATE INDEX idx_api_keys_key_hash ON api_keys USING btree (key_hash);
CREATE INDEX idx_api_keys_user_id ON api_keys USING btree (user_id);
CREATE INDEX idx_blocked_users_blocked_id ON blocked_users USING btree (blocked_id);
CREATE INDEX idx_blocked_users_user_id ON blocked_users USING btree (user_id);
CREATE INDEX idx_build_logs_build_id ON build_logs USING btree (build_id);
CREATE INDEX idx_build_status_commit_sha ON build_status USING btree (commit_sha);
CREATE INDEX idx_build_status_game_id ON build_status USING btree (game_id);
CREATE INDEX idx_build_status_repository ON build_status USING btree (repository);
CREATE INDEX idx_build_status_runner_token ON build_status USING btree (runner_token) WHERE (runner_token IS NOT NULL);
CREATE INDEX idx_build_status_status ON build_status USING btree (status);
CREATE INDEX idx_channel_members_channel ON channel_members USING btree (channel_id);
CREATE INDEX idx_channel_members_user ON channel_members USING btree (user_id);
CREATE INDEX idx_channels_community ON channels USING btree (community_id);
CREATE INDEX idx_channels_kind ON channels USING btree (community_id, kind);
CREATE INDEX idx_chat_flags_author_id ON chat_flags USING btree (author_id);
CREATE INDEX idx_chat_flags_channel_id ON chat_flags USING btree (channel_id);
CREATE INDEX idx_chat_flags_created_at ON chat_flags USING btree (created_at DESC);
CREATE INDEX idx_chat_flags_status ON chat_flags USING btree (status);
CREATE INDEX idx_cicd_pipelines_repository ON cicd_pipelines USING btree (repository);
CREATE INDEX idx_cicd_pipelines_status ON cicd_pipelines USING btree (status);
CREATE INDEX idx_comms_rooms_community ON comms_rooms USING btree (community_id) WHERE (community_id IS NOT NULL);
CREATE INDEX idx_comms_rooms_open ON comms_rooms USING btree (created_at DESC) WHERE (closed_at IS NULL);
CREATE INDEX idx_comms_rooms_provider ON comms_rooms USING btree (provider);
CREATE INDEX idx_comms_rooms_scope ON comms_rooms USING btree (scope, scope_ref);
CREATE INDEX idx_communities_owner ON communities USING btree (owner_id);
CREATE INDEX idx_communities_slug ON communities USING btree (slug);
CREATE INDEX idx_community_members_community ON community_members USING btree (community_id);
CREATE INDEX idx_community_members_user ON community_members USING btree (user_id);
CREATE INDEX idx_contact_messages_created_at ON contact_messages USING btree (created_at);
CREATE INDEX idx_contact_messages_email ON contact_messages USING btree (email);
CREATE INDEX idx_deployments_repository ON deployments USING btree (repository);
CREATE INDEX idx_deployments_status ON deployments USING btree (status);
CREATE INDEX idx_dev_stores_developer_id ON dev_stores USING btree (developer_id);
CREATE INDEX idx_dev_stores_game_id ON dev_stores USING btree (game_id);
CREATE INDEX idx_discovery_ads_expires ON discovery_ads USING btree (expires_at);
CREATE INDEX idx_discovery_ads_game_live ON discovery_ads USING btree (game_hash, expires_at DESC);
CREATE INDEX idx_dm_messages_created ON dm_messages USING btree (thread_id, created_at DESC);
CREATE INDEX idx_dm_messages_thread ON dm_messages USING btree (thread_id);
CREATE INDEX idx_dm_threads_last_msg ON dm_threads USING btree (last_message_at DESC);
CREATE INDEX idx_dm_threads_user_a ON dm_threads USING btree (user_a_id);
CREATE INDEX idx_dm_threads_user_b ON dm_threads USING btree (user_b_id);
CREATE INDEX idx_entitlements_item_id ON entitlements USING btree (item_id);
CREATE INDEX idx_entitlements_receipt ON entitlements USING btree (receipt_id) WHERE (receipt_id IS NOT NULL);
CREATE INDEX idx_entitlements_user_id ON entitlements USING btree (user_id);
CREATE INDEX idx_entitlements_user_item ON entitlements USING btree (user_id, item_id, revoked);
CREATE INDEX idx_friend_requests_from_user_id ON friend_requests USING btree (from_user_id);
CREATE INDEX idx_friend_requests_status ON friend_requests USING btree (status);
CREATE INDEX idx_friend_requests_to_user_id ON friend_requests USING btree (to_user_id);
CREATE INDEX idx_friendships_friend_id ON friendships USING btree (friend_id);
CREATE INDEX idx_friendships_user_id ON friendships USING btree (user_id);
CREATE INDEX idx_game_artifacts_build_id ON game_artifacts USING btree (build_id);
CREATE INDEX idx_game_artifacts_build_status ON game_artifacts USING btree (build_status);
CREATE INDEX idx_game_artifacts_game_id ON game_artifacts USING btree (game_id);
CREATE INDEX idx_game_artifacts_version_id ON game_artifacts USING btree (version_id);
CREATE INDEX idx_game_high_scores_game_id ON game_high_scores USING btree (game_id);
CREATE INDEX idx_game_invites_from_user_id ON game_invites USING btree (from_user_id);
CREATE INDEX idx_game_invites_game_id ON game_invites USING btree (game_id);
CREATE INDEX idx_game_invites_status ON game_invites USING btree (status);
CREATE INDEX idx_game_invites_to_user_id ON game_invites USING btree (to_user_id);
CREATE INDEX idx_game_revenue_developer_id ON game_revenue USING btree (developer_id);
CREATE INDEX idx_game_revenue_game_id ON game_revenue USING btree (game_id);
CREATE INDEX idx_game_revenue_session_id ON game_revenue USING btree (session_id);
CREATE INDEX idx_game_revenue_status ON game_revenue USING btree (status);
CREATE INDEX idx_game_scaffolds_developer_id ON game_scaffolds USING btree (developer_id);
CREATE INDEX idx_game_scaffolds_game_id ON game_scaffolds USING btree (game_id);
CREATE INDEX idx_game_versions_game_id ON game_versions USING btree (game_id);
CREATE INDEX idx_game_versions_is_live ON game_versions USING btree (game_id, is_live) WHERE (is_live = true);
CREATE INDEX idx_games_active ON games USING btree (active);
CREATE INDEX idx_games_active_status ON games USING btree (status) WHERE ((active = true) AND ((status)::text = 'active'::text));
CREATE INDEX idx_games_category_id ON games USING btree (category_id);
CREATE INDEX idx_games_content_rating ON games USING btree (content_rating);
CREATE INDEX idx_games_created_at ON games USING btree (created_at DESC);
CREATE INDEX idx_games_description_search ON games USING gin (description gin_trgm_ops);
CREATE INDEX idx_games_developer_id ON games USING btree (developer_id);
CREATE INDEX idx_games_status ON games USING btree (status);
CREATE INDEX idx_games_title_search ON games USING gin (title gin_trgm_ops);
CREATE INDEX idx_github_installations_account_id ON github_installations USING btree (account_id);
CREATE INDEX idx_github_installations_installation_id ON github_installations USING btree (installation_id);
CREATE INDEX idx_hosting_channels_payer ON hosting_channels USING btree (payer_id);
CREATE INDEX idx_matchmaking_queue_game_id ON matchmaking_queue USING btree (game_id);
CREATE INDEX idx_matchmaking_queue_status ON matchmaking_queue USING btree (status);
CREATE INDEX idx_matchmaking_queue_user_id ON matchmaking_queue USING btree (user_id);
CREATE INDEX idx_matchmaking_waiting ON matchmaking_queue USING btree (status, created_at) WHERE ((status)::text = 'waiting'::text);
CREATE INDEX idx_messages_author ON messages USING btree (author_id);
CREATE INDEX idx_messages_channel ON messages USING btree (channel_id);
CREATE INDEX idx_messages_created ON messages USING btree (channel_id, created_at DESC);
CREATE INDEX idx_messages_reply ON messages USING btree (reply_to_id) WHERE (reply_to_id IS NOT NULL);
CREATE INDEX idx_notification_preferences_user_id ON notification_preferences USING btree (user_id);
CREATE INDEX idx_notifications_created_at ON notifications USING btree (created_at DESC);
CREATE INDEX idx_notifications_user_id ON notifications USING btree (user_id);
CREATE INDEX idx_notifications_user_unread ON notifications USING btree (user_id, read) WHERE (read = false);
CREATE INDEX idx_oauth_identities_user_id ON oauth_identities USING btree (user_id);
CREATE INDEX idx_payment_receipts_buyer ON payment_receipts USING btree (buyer_id, created_at DESC);
CREATE INDEX idx_payment_receipts_game ON payment_receipts USING btree (game_id) WHERE (game_id IS NOT NULL);
CREATE UNIQUE INDEX idx_payment_receipts_purchase ON payment_receipts USING btree (purchase_id) WHERE (purchase_id IS NOT NULL);
CREATE INDEX idx_payouts_status ON payouts USING btree (status);
CREATE INDEX idx_payouts_user_id ON payouts USING btree (user_id);
CREATE INDEX idx_play_sessions_game_id ON play_sessions USING btree (game_id);
CREATE INDEX idx_play_sessions_payout_status ON play_sessions USING btree (payout_status);
CREATE INDEX idx_play_sessions_status ON play_sessions USING btree (status);
CREATE INDEX idx_play_sessions_user_id ON play_sessions USING btree (user_id);
CREATE INDEX idx_point_rewards_game_id ON point_rewards USING btree (game_id) WHERE (game_id IS NOT NULL);
CREATE INDEX idx_point_rewards_kind ON point_rewards USING btree (kind, active);
CREATE INDEX idx_points_ledger_game_id ON points_ledger USING btree (game_id) WHERE (game_id IS NOT NULL);
CREATE INDEX idx_points_ledger_season_id ON points_ledger USING btree (season_id) WHERE (season_id IS NOT NULL);
CREATE INDEX idx_points_ledger_user_created ON points_ledger USING btree (user_id, created_at DESC);
CREATE INDEX idx_points_ledger_user_id ON points_ledger USING btree (user_id);
CREATE INDEX idx_presence_game ON presence USING btree (game_id) WHERE (game_id IS NOT NULL);
CREATE INDEX idx_presence_status ON presence USING btree (status);
CREATE INDEX idx_pull_request_tests_pr_number ON pull_request_tests USING btree (pr_number);
CREATE INDEX idx_pull_request_tests_repository ON pull_request_tests USING btree (repository);
CREATE INDEX idx_pull_request_tests_status ON pull_request_tests USING btree (status);
CREATE INDEX idx_refund_records_admin_id ON refund_records USING btree (admin_id);
CREATE INDEX idx_refund_records_status ON refund_records USING btree (status);
CREATE INDEX idx_refund_records_transaction_id ON refund_records USING btree (transaction_id);
CREATE INDEX idx_refund_records_user_id ON refund_records USING btree (user_id);
CREATE INDEX idx_registered_games_game_id ON registered_games USING btree (game_id);
CREATE INDEX idx_registered_games_github_installation_id ON registered_games USING btree (github_installation_id);
CREATE INDEX idx_registered_games_repo_full_name ON registered_games USING btree (repo_full_name);
CREATE INDEX idx_replays_created_at ON replays USING btree (created_at DESC);
CREATE INDEX idx_replays_game_id ON replays USING btree (game_id);
CREATE INDEX idx_replays_match_id ON replays USING btree (match_id);
CREATE INDEX idx_replays_recorded_by ON replays USING btree (recorded_by);
CREATE INDEX idx_review_helpful_review_id ON review_helpful USING btree (review_id);
CREATE INDEX idx_review_helpful_user_id ON review_helpful USING btree (user_id);
CREATE INDEX idx_review_reports_reporter_id ON review_reports USING btree (reporter_id);
CREATE INDEX idx_review_reports_resolved_by ON review_reports USING btree (resolved_by) WHERE (resolved_by IS NOT NULL);
CREATE INDEX idx_review_reports_review_id ON review_reports USING btree (review_id);
CREATE INDEX idx_review_reports_source ON review_reports USING btree (source);
CREATE INDEX idx_review_reports_status ON review_reports USING btree (status);
CREATE INDEX idx_review_reports_status_source ON review_reports USING btree (status, source);
CREATE INDEX idx_reviews_game_id ON reviews USING btree (game_id);
CREATE INDEX idx_reviews_rating ON reviews USING btree (rating);
CREATE INDEX idx_reviews_user_id ON reviews USING btree (user_id);
CREATE INDEX idx_runtime_instances_artifact_id ON runtime_instances USING btree (artifact_id);
CREATE INDEX idx_runtime_instances_game_id ON runtime_instances USING btree (game_id);
CREATE INDEX idx_runtime_instances_status ON runtime_instances USING btree (status);
CREATE INDEX idx_scores_game_id ON scores USING btree (game_id);
CREATE INDEX idx_scores_game_user ON scores USING btree (game_id, user_id);
CREATE INDEX idx_scores_recorded_at ON scores USING btree (recorded_at DESC);
CREATE INDEX idx_scores_user_id ON scores USING btree (user_id);
CREATE UNIQUE INDEX idx_seasons_active_one ON seasons USING btree (is_active) WHERE (is_active = true);
CREATE INDEX idx_seasons_starts_at ON seasons USING btree (starts_at);
CREATE INDEX idx_session_replays_recorded_at ON session_replays USING btree (recorded_at);
CREATE INDEX idx_session_replays_session_id ON session_replays USING btree (session_id);
CREATE INDEX idx_sessions_expires_at ON sessions USING btree (expires_at);
CREATE INDEX idx_sessions_token_prefix ON sessions USING btree (token_prefix) WHERE (token_prefix IS NOT NULL);
CREATE INDEX idx_sessions_user_id ON sessions USING btree (user_id);
CREATE INDEX idx_sessions_user_id_expires_at ON sessions USING btree (user_id, expires_at);
CREATE INDEX idx_store_items_currency ON store_items USING btree (currency, active);
CREATE INDEX idx_store_items_game_id ON store_items USING btree (game_id);
CREATE INDEX idx_store_items_kind ON store_items USING btree (kind, active);
CREATE INDEX idx_store_items_store_id ON store_items USING btree (store_id);
CREATE INDEX idx_store_purchases_created ON store_purchases USING btree (created_at DESC);
CREATE INDEX idx_store_purchases_game_id ON store_purchases USING btree (game_id);
CREATE INDEX idx_store_purchases_item_id ON store_purchases USING btree (item_id);
CREATE INDEX idx_store_purchases_refunded ON store_purchases USING btree (refunded_at) WHERE (refunded_at IS NOT NULL);
CREATE INDEX idx_store_purchases_store_id ON store_purchases USING btree (store_id);
CREATE INDEX idx_store_purchases_user_id ON store_purchases USING btree (user_id);
CREATE INDEX idx_streams_community ON streams USING btree (community_id) WHERE (community_id IS NOT NULL);
CREATE INDEX idx_streams_community_live ON streams USING btree (community_id, status) WHERE (status = 'live'::text);
CREATE INDEX idx_streams_game_live ON streams USING btree (game_id, status) WHERE ((status = 'live'::text) AND (game_id IS NOT NULL));
CREATE INDEX idx_streams_live ON streams USING btree (status) WHERE (status = 'live'::text);
CREATE INDEX idx_streams_status ON streams USING btree (status);
CREATE INDEX idx_streams_streamer ON streams USING btree (streamer_id);
CREATE INDEX idx_superadmin_audit_action ON superadmin_audit_log USING btree (action);
CREATE INDEX idx_superadmin_audit_occurred_at ON superadmin_audit_log USING btree (occurred_at DESC);
CREATE INDEX idx_tournament_matches_round ON tournament_matches USING btree (tournament_id, round);
CREATE INDEX idx_tournament_matches_tournament_id ON tournament_matches USING btree (tournament_id);
CREATE INDEX idx_tournament_participants_tournament_id ON tournament_participants USING btree (tournament_id);
CREATE INDEX idx_tournament_participants_user_id ON tournament_participants USING btree (user_id);
CREATE INDEX idx_tournaments_game_id ON tournaments USING btree (game_id);
CREATE INDEX idx_tournaments_start_time ON tournaments USING btree (start_time);
CREATE INDEX idx_tournaments_status ON tournaments USING btree (status);
CREATE INDEX idx_transactions_game_id ON transactions USING btree (game_id);
CREATE INDEX idx_transactions_user_id ON transactions USING btree (user_id);
CREATE INDEX idx_user_achievements_achievement_id ON user_achievements USING btree (achievement_id);
CREATE INDEX idx_user_achievements_user_id ON user_achievements USING btree (user_id);
CREATE INDEX idx_user_subscriptions_status_period_end ON user_subscriptions USING btree (status, current_period_end) WHERE (cancel_at_period_end = false);
CREATE INDEX idx_user_subscriptions_user_status ON user_subscriptions USING btree (user_id, status);
CREATE INDEX idx_users_created_at ON users USING btree (created_at DESC);
CREATE INDEX idx_users_discord_id ON users USING btree (discord_id) WHERE (discord_id IS NOT NULL);
CREATE INDEX idx_users_email ON users USING btree (email);
CREATE INDEX idx_users_github_id ON users USING btree (github_id) WHERE (github_id IS NOT NULL);
CREATE INDEX idx_users_google_id ON users USING btree (google_id) WHERE (google_id IS NOT NULL);
CREATE INDEX idx_users_username_search ON users USING gin (username gin_trgm_ops);
CREATE UNIQUE INDEX idx_users_wallet_address ON users USING btree (wallet_address) WHERE (wallet_address IS NOT NULL);
CREATE INDEX idx_verification_tokens_expires_at ON verification_tokens USING btree (expires_at);
CREATE INDEX idx_verification_tokens_token ON verification_tokens USING btree (token);
CREATE INDEX idx_verification_tokens_token_type ON verification_tokens USING btree (token_type);
CREATE INDEX idx_verification_tokens_user_id ON verification_tokens USING btree (user_id);
CREATE INDEX idx_voice_participants_active ON voice_participants USING btree (room_id) WHERE (left_at IS NULL);
CREATE INDEX idx_voice_participants_room ON voice_participants USING btree (room_id);
CREATE INDEX idx_voice_participants_user ON voice_participants USING btree (user_id);
CREATE INDEX idx_voice_rooms_active ON voice_rooms USING btree (is_active) WHERE (is_active = true);
CREATE INDEX idx_voice_rooms_channel ON voice_rooms USING btree (channel_id);
CREATE INDEX idx_wallet_balances_user_id ON wallet_balances USING btree (user_id);
CREATE INDEX idx_wallet_transactions_aml ON wallet_transactions USING btree (user_id, currency, tx_type, status, created_at);
CREATE INDEX idx_wallet_transactions_idempotency ON wallet_transactions USING btree (idempotency_key) WHERE (idempotency_key IS NOT NULL);
CREATE INDEX idx_wallet_transactions_status ON wallet_transactions USING btree (status) WHERE ((status)::text = 'pending'::text);
CREATE INDEX idx_wallet_transactions_user_created ON wallet_transactions USING btree (user_id, created_at DESC);
CREATE INDEX idx_wallet_transactions_user_id ON wallet_transactions USING btree (user_id);
CREATE INDEX idx_wishlists_game_id ON wishlists USING btree (game_id);
CREATE INDEX idx_wishlists_user_id ON wishlists USING btree (user_id);
CREATE INDEX wise_recipients_developer_id_idx ON wise_recipients USING btree (developer_id, created_at DESC);

-- ── Triggers ────────────────────────────────────────────────────────────────
CREATE TRIGGER trg_review_helpful_count AFTER INSERT OR DELETE ON review_helpful FOR EACH ROW EXECUTE FUNCTION fn_sync_helpful_count();

-- ── Grants ──────────────────────────────────────────────────────────────────
REVOKE USAGE ON SCHEMA public FROM PUBLIC;

-- ── Comments ────────────────────────────────────────────────────────────────
COMMENT ON SCHEMA public IS '';
COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';
COMMENT ON TABLE comms_rooms IS 'Provider-agnostic room registry for the CommsProvider seam (§3.5). The builtin (in-house chat/voice/streaming) stack is one provider among Matrix / Jitsi / LiveKit / Owncast, not the only path.';
COMMENT ON TABLE payouts IS 'DEPRECATED (non-custodial migration 20260707): fiat/custody table. No writer remains; payments settle wallet-to-wallet via payment_receipts. Safe to DROP once no reader remains.';
COMMENT ON COLUMN streams.media_host IS 'Per-node media host for this stream. Replaces the global MEDIA_SERVER_BASE_URL assumption — every operator runs their own media server.';
COMMENT ON COLUMN subscription_tiers.price_usdc IS 'Fiat USD price (was USDC; Circle removed 2026-06-01)';
COMMENT ON TABLE wallet_balances IS 'DEPRECATED (non-custodial migration 20260707): fiat/custody table. No writer remains; payments settle wallet-to-wallet via payment_receipts. Safe to DROP once no reader remains.';
COMMENT ON TABLE wallet_transactions IS 'DEPRECATED (non-custodial migration 20260707): fiat/custody table. No writer remains; payments settle wallet-to-wallet via payment_receipts. Safe to DROP once no reader remains.';
COMMENT ON TABLE wise_recipients IS 'DEPRECATED (non-custodial migration 20260707): fiat/custody table. No writer remains; payments settle wallet-to-wallet via payment_receipts. Safe to DROP once no reader remains.';

-- ── Carried through verbatim ───────────────────────────────────────────────
\restrict Zbdlmm5ywEaeRFtfGRtSUZGS3jWJWcKnrMll2tkabbatblR1ieUYPcOyJUxzfXi




SET statement_timeout = 0;
CREATE VIEW public.purchase_receipts AS
 SELECT sp.id AS purchase_id,
    sp.user_id,
    sp.item_id,
    sp.store_id,
    sp.game_id,
    si.sku AS item_sku,
    si.name AS item_name,
    si.kind AS item_kind,
    sp.price_paid,
    sp.currency,
    sp.developer_share,
    sp.platform_fee,
    sp.status,
    sp.idempotency_key,
    sp.metadata,
    sp.created_at AS purchased_at,
    sp.refunded_at,
    sp.refunded_by,
    sp.refund_reason
   FROM (public.store_purchases sp
     JOIN public.store_items si ON ((si.id = sp.item_id)));
CREATE VIEW public.tournament_standings AS
 SELECT tp.tournament_id,
    tp.user_id,
    u.username,
    tp.seed,
    tp.status AS participant_status,
    count(
        CASE
            WHEN (tm.winner_id = tp.user_id) THEN 1
            ELSE NULL::integer
        END) AS wins,
    count(
        CASE
            WHEN (((tm.status)::text = 'completed'::text) AND ((tm.player1_id = tp.user_id) OR (tm.player2_id = tp.user_id)) AND (tm.winner_id <> tp.user_id)) THEN 1
            ELSE NULL::integer
        END) AS losses,
    (count(
        CASE
            WHEN (tm.winner_id = tp.user_id) THEN 1
            ELSE NULL::integer
        END) * 3) AS points
   FROM ((public.tournament_participants tp
     JOIN public.users u ON ((u.id = tp.user_id)))
     LEFT JOIN public.tournament_matches tm ON (((tm.tournament_id = tp.tournament_id) AND ((tm.player1_id = tp.user_id) OR (tm.player2_id = tp.user_id)))))
  GROUP BY tp.tournament_id, tp.user_id, u.username, tp.seed, tp.status;
\unrestrict Zbdlmm5ywEaeRFtfGRtSUZGS3jWJWcKnrMll2tkabbatblR1ieUYPcOyJUxzfXi;

