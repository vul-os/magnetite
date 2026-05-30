# Data Model

This page documents the SQL tables that back the Magnetite comms suite. They are
introduced by Wave 6 migrations under `backend/migrations/`.

All tables use **UUID primary keys** (Postgres `uuid` type), `TIMESTAMPTZ`
timestamps, and `NOT NULL` constraints unless a value is genuinely optional.
The platform's standard enum convention stores short string values rather than
Postgres `ENUM` types so that new variants can be added without a migration lock.

---

## communities

The top-level space: a server / guild that groups channels and members.

```sql
CREATE TABLE communities (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT        NOT NULL,
    description  TEXT,
    icon_url     TEXT,
    owner_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

| Column | Notes |
|--------|-------|
| `id` | UUIDv4, platform-generated |
| `name` | Display name (1–100 chars, validated at API layer) |
| `description` | Optional markdown-safe short description |
| `icon_url` | S3 URL of the community icon, nullable |
| `owner_id` | FK to `users`; owner always has the `owner` role |
| `created_at` / `updated_at` | Set and bumped by triggers / service layer |

---

## channels

A channel belongs to one community and has a **kind** that determines its behavior.

```sql
CREATE TABLE channels (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID        NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    kind         TEXT        NOT NULL DEFAULT 'text',   -- 'text' | 'voice'
    position     INTEGER     NOT NULL DEFAULT 0,
    topic        TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_channels_community ON channels(community_id, position);
```

| Column | Notes |
|--------|-------|
| `kind` | `'text'` — persistent message history; `'voice'` — live voice room |
| `position` | Ordering within the community sidebar (lower = higher) |
| `topic` | Optional channel topic shown in the header |

---

## channel_members

Membership and role of a user within a community. Every user who joins a community
gets a row here.

```sql
CREATE TABLE channel_members (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID        NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    user_id      UUID        NOT NULL REFERENCES users(id)       ON DELETE CASCADE,
    role         TEXT        NOT NULL DEFAULT 'member',   -- 'owner' | 'admin' | 'member'
    joined_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (community_id, user_id)
);

CREATE INDEX idx_channel_members_user ON channel_members(user_id);
```

| Column | Notes |
|--------|-------|
| `role` | `owner` — created the community; `admin` — manage channels/members; `member` — standard access |
| Unique constraint | A user may only appear once per community |

---

## messages

Persisted text messages inside a channel.

```sql
CREATE TABLE messages (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id   UUID        NOT NULL REFERENCES channels(id)    ON DELETE CASCADE,
    author_id    UUID        NOT NULL REFERENCES users(id)       ON DELETE SET NULL,
    content      TEXT        NOT NULL,
    edited_at    TIMESTAMPTZ,                           -- NULL = never edited
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_messages_channel_created ON messages(channel_id, created_at DESC);
```

| Column | Notes |
|--------|-------|
| `content` | Plain text (1–4000 chars); rich formatting applied client-side via safe markdown |
| `edited_at` | Set when the author edits the message; `NULL` while unedited |
| `author_id` | SET NULL on user deletion — displays as "Deleted User" in the UI |

The index on `(channel_id, created_at DESC)` enables efficient cursor-based pagination
(load before a given timestamp / message ID).

---

## voice_rooms

A voice room is the live audio session attached to a `voice` channel. It is
created on demand when the first participant joins and deleted when the last
participant leaves (or the channel is deleted).

```sql
CREATE TABLE voice_rooms (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id   UUID        NOT NULL REFERENCES channels(id)   ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (channel_id)       -- one room per voice channel at a time
);
```

| Column | Notes |
|--------|-------|
| `channel_id` | 1:1 with the owning voice channel |

---

## voice_participants

Tracks who is currently in a voice room and their audio state.

```sql
CREATE TABLE voice_participants (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    voice_room_id UUID        NOT NULL REFERENCES voice_rooms(id) ON DELETE CASCADE,
    user_id       UUID        NOT NULL REFERENCES users(id)        ON DELETE CASCADE,
    muted         BOOLEAN     NOT NULL DEFAULT FALSE,
    joined_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (voice_room_id, user_id)
);

CREATE INDEX idx_voice_participants_room ON voice_participants(voice_room_id);
```

| Column | Notes |
|--------|-------|
| `muted` | Reflects client-reported mute state; updated via `voice.mute` / `voice.unmute` WS messages |
| Unique constraint | A user can be in a room only once |

---

## streams

A live stream started by a community member from a voice room.

```sql
CREATE TABLE streams (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    voice_room_id   UUID        NOT NULL REFERENCES voice_rooms(id) ON DELETE CASCADE,
    broadcaster_id  UUID        NOT NULL REFERENCES users(id)       ON DELETE CASCADE,
    title           TEXT        NOT NULL DEFAULT '',
    watch_url       TEXT,                          -- HLS / WebRTC watch URL
    rtmp_key        TEXT,                          -- encrypted external RTMP key, nullable
    status          TEXT        NOT NULL DEFAULT 'live',  -- 'live' | 'ended'
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at        TIMESTAMPTZ,
    UNIQUE (voice_room_id)       -- one live stream per room at a time
);

CREATE INDEX idx_streams_broadcaster ON streams(broadcaster_id, started_at DESC);
```

| Column | Notes |
|--------|-------|
| `watch_url` | In-platform HLS playlist or WebRTC watch endpoint; set by the backend when the stream is created |
| `rtmp_key` | Stored encrypted; used only if the broadcaster configured RTMP egress to Twitch/YouTube |
| `status` | `'live'` while active; flipped to `'ended'` when the broadcaster stops or disconnects |
| `ended_at` | Set when status transitions to `'ended'`; duration = `ended_at - started_at` |

---

## Relationships at a glance

```
users
 ├── communities (owner_id)
 ├── channel_members (user_id)
 ├── messages (author_id)
 ├── voice_participants (user_id)
 └── streams (broadcaster_id)

communities
 └── channels (community_id)
      ├── messages (channel_id)  [kind = 'text']
      └── voice_rooms (channel_id)  [kind = 'voice']
           ├── voice_participants (voice_room_id)
           └── streams (voice_room_id)
```

---

## Future tables (reserved in §4b)

These are called out in the Wave 6 architecture decisions but are provisioned in
later waves:

| Table | Wave | Purpose |
|-------|------|---------|
| `points_ledger` | 8 | Platform-wide XP / points economy |
| `point_rewards` | 8 | Game-submitted reward events |
| `dev_stores` | 8 | Developer in-game store listings |
| `store_items` | 8 | Cosmetics / DLC / passes |
| `store_purchases` | 8 | Purchase records |

---

## See also

- [Comms Overview](./index.md)
- [Realtime Protocol](./realtime.md)
- [Architecture Overview](../architecture.md)
- [Migrations directory](../../backend/migrations/)
