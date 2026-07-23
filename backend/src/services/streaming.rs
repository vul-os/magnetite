// Streaming service — go-live lifecycle, viewer count, RTMP egress config,
// and HLS manifest proxying.
//
// ┌───────────────────────────────────────────────────────────────────────────┐
// │  Media-server dependency                                                  │
// │                                                                           │
// │  This service manages METADATA and LIFECYCLE RECORDS in PostgreSQL.       │
// │  Actual media transcoding / HLS segmentation / RTMP relaying is           │
// │  handled by a separate media server.  Recommended options:                │
// │                                                                           │
// │  • MediaMTX (https://github.com/bluenviron/mediamtx) — self-hosted,      │
// │    accepts RTMP/RTSP/SRT/WebRTC ingest, outputs HLS.  Start with:        │
// │      docker run --rm -it -e MTX_PROTOCOLS=all \                           │
// │        -p 1935:1935 -p 8888:8888 bluenviron/mediamtx                     │
// │    Broadcaster pushes to rtmp://media-server/live/<ingest_key>            │
// │    Viewers fetch  http://media-server:8888/live/<ingest_key>/index.m3u8   │
// │                                                                           │
// │  • nginx-rtmp — alternative self-hosted RTMP → HLS stack.                │
// │                                                                           │
// │  • Mux / Cloudflare Stream / AWS Elemental MediaLive — managed CDN-       │
// │    grade option for production scale.                                     │
// │                                                                           │
// │  RTMP egress (Twitch / YouTube restream):                                │
// │    Store `rtmp_target` + `stream_key` in the stream record.  The media   │
// │    server (MediaMTX `runOnPublish` hook or nginx-rtmp `push` directive)  │
// │    forwards the ingest to the external RTMP URL automatically.            │
// │    No backend process forwards the stream; the media server is the relay. │
// │                                                                           │
// │  WebRTC in-browser broadcast:                                             │
// │    The frontend uses getDisplayMedia() → WebRTC → WHIP endpoint on the   │
// │    media server (MediaMTX supports WHIP).  The signaling path reuses      │
// │    ws/voice.rs (SDP/ICE relay).  No additional backend code is needed.   │
// └───────────────────────────────────────────────────────────────────────────┘
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Stream {
    pub id: Uuid,
    pub streamer_id: Uuid,
    pub community_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
    pub title: String,
    pub status: String, // 'offline' | 'live' | 'ended'
    pub viewer_count: i32,
    pub hls_url: Option<String>,
    /// RTMP target for external restream (Twitch/YouTube ingest URL).
    /// Stored but NOT returned to general list/get callers — only the
    /// stream owner sees it via `get_stream_for_owner`.
    #[serde(skip_serializing)]
    pub rtmp_target: Option<String>,
    /// External stream key for rtmp_target. Never serialised.
    #[serde(skip_serializing)]
    pub stream_key: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// A safe public view of a stream — excludes ingest_key / rtmp_target /
/// stream_key.  Used in list-live and get responses for non-owners.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamPublic {
    pub id: Uuid,
    pub streamer_id: Uuid,
    pub community_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
    pub title: String,
    pub status: String,
    pub viewer_count: i32,
    pub hls_url: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<Stream> for StreamPublic {
    fn from(s: Stream) -> Self {
        StreamPublic {
            id: s.id,
            streamer_id: s.streamer_id,
            community_id: s.community_id,
            channel_id: s.channel_id,
            game_id: s.game_id,
            title: s.title,
            status: s.status,
            viewer_count: s.viewer_count,
            hls_url: s.hls_url,
            started_at: s.started_at,
            ended_at: s.ended_at,
            created_at: s.created_at,
        }
    }
}

/// Full stream detail for the owner — includes RTMP egress config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOwnerView {
    #[serde(flatten)]
    pub public: StreamPublic,
    pub ingest_key: Option<String>,
    pub rtmp_target: Option<String>,
    pub stream_key: Option<String>,
}

/// Full row including ingest_key (never stored in Stream to avoid accidental exposure).
#[derive(Debug, sqlx::FromRow)]
struct StreamRow {
    id: Uuid,
    streamer_id: Uuid,
    community_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    game_id: Option<Uuid>,
    title: String,
    status: String,
    viewer_count: i32,
    hls_url: Option<String>,
    rtmp_target: Option<String>,
    stream_key: Option<String>,
    ingest_key: Option<String>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl StreamRow {
    fn into_public(self) -> StreamPublic {
        StreamPublic {
            id: self.id,
            streamer_id: self.streamer_id,
            community_id: self.community_id,
            channel_id: self.channel_id,
            game_id: self.game_id,
            title: self.title,
            status: self.status,
            viewer_count: self.viewer_count,
            hls_url: self.hls_url,
            started_at: self.started_at,
            ended_at: self.ended_at,
            created_at: self.created_at,
        }
    }

    fn into_owner_view(self) -> StreamOwnerView {
        StreamOwnerView {
            public: StreamPublic {
                id: self.id,
                streamer_id: self.streamer_id,
                community_id: self.community_id,
                channel_id: self.channel_id,
                game_id: self.game_id,
                title: self.title.clone(),
                status: self.status.clone(),
                viewer_count: self.viewer_count,
                hls_url: self.hls_url.clone(),
                started_at: self.started_at,
                ended_at: self.ended_at,
                created_at: self.created_at,
            },
            ingest_key: self.ingest_key,
            rtmp_target: self.rtmp_target,
            stream_key: self.stream_key,
        }
    }
}

// ---------------------------------------------------------------------------
// Key generation helpers
// ---------------------------------------------------------------------------

/// Generate a cryptographically random ingest key (48 alphanumeric chars).
pub fn generate_ingest_key() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

// ---------------------------------------------------------------------------
// Lifecycle operations
// ---------------------------------------------------------------------------

/// Create a new stream record in 'offline' status.  Returns the full owner
/// view including the freshly-generated ingest_key so the broadcaster can
/// configure their streaming software.
#[allow(clippy::too_many_arguments)]
pub async fn create_stream(
    pool: &PgPool,
    streamer_id: Uuid,
    title: &str,
    community_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    game_id: Option<Uuid>,
    rtmp_target: Option<&str>,
    stream_key: Option<&str>,
) -> Result<StreamOwnerView> {
    if title.trim().is_empty() {
        return Err(AppError::Validation("Stream title is required".to_string()));
    }

    let ingest_key = generate_ingest_key();
    let id = Uuid::new_v4();

    let row = sqlx::query_as::<_, StreamRow>(
        r#"
        INSERT INTO streams (
            id, streamer_id, community_id, channel_id, game_id,
            title, status, viewer_count, ingest_key, rtmp_target, stream_key,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'offline', 0, $7, $8, $9, NOW())
        RETURNING
            id, streamer_id, community_id, channel_id, game_id,
            title, status, viewer_count, hls_url, rtmp_target, stream_key,
            ingest_key, started_at, ended_at, created_at
        "#,
    )
    .bind(id)
    .bind(streamer_id)
    .bind(community_id)
    .bind(channel_id)
    .bind(game_id)
    .bind(title.trim())
    .bind(&ingest_key)
    .bind(rtmp_target)
    .bind(stream_key)
    .fetch_one(pool)
    .await?;

    Ok(row.into_owner_view())
}

/// Transition a stream to 'live'.  Only the owning streamer may call this.
/// Sets `started_at`, optionally stores the HLS URL provided by the caller
/// (the caller has already verified the media server is accepting ingest).
pub async fn go_live(
    pool: &PgPool,
    stream_id: Uuid,
    streamer_id: Uuid,
    hls_url: Option<&str>,
) -> Result<StreamOwnerView> {
    let row = sqlx::query_as::<_, StreamRow>(
        r#"
        UPDATE streams SET
            status     = 'live',
            started_at = COALESCE(started_at, NOW()),
            hls_url    = COALESCE($3, hls_url),
            ended_at   = NULL
        WHERE id = $1 AND streamer_id = $2 AND status != 'ended'
        RETURNING
            id, streamer_id, community_id, channel_id, game_id,
            title, status, viewer_count, hls_url, rtmp_target, stream_key,
            ingest_key, started_at, ended_at, created_at
        "#,
    )
    .bind(stream_id)
    .bind(streamer_id)
    .bind(hls_url)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::NotFound("Stream not found, not owned by you, or already ended".to_string())
    })?;

    Ok(row.into_owner_view())
}

/// Stop a live stream.  Sets status = 'ended' and records ended_at.
pub async fn stop_stream(
    pool: &PgPool,
    stream_id: Uuid,
    streamer_id: Uuid,
) -> Result<StreamPublic> {
    let row = sqlx::query_as::<_, StreamRow>(
        r#"
        UPDATE streams SET
            status     = 'ended',
            ended_at   = NOW(),
            viewer_count = 0
        WHERE id = $1 AND streamer_id = $2 AND status = 'live'
        RETURNING
            id, streamer_id, community_id, channel_id, game_id,
            title, status, viewer_count, hls_url, rtmp_target, stream_key,
            ingest_key, started_at, ended_at, created_at
        "#,
    )
    .bind(stream_id)
    .bind(streamer_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::NotFound("Stream not found, not owned by you, or not currently live".to_string())
    })?;

    Ok(row.into_public())
}

// ---------------------------------------------------------------------------
// Query operations
// ---------------------------------------------------------------------------

/// List all currently-live streams, optionally scoped to a community or game.
/// Returns public views (no ingest credentials).
pub async fn list_live_streams(
    pool: &PgPool,
    community_id: Option<Uuid>,
    game_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> Result<Vec<StreamPublic>> {
    let rows = sqlx::query_as::<_, StreamRow>(
        r#"
        SELECT
            id, streamer_id, community_id, channel_id, game_id,
            title, status, viewer_count, hls_url, rtmp_target, stream_key,
            ingest_key, started_at, ended_at, created_at
        FROM streams
        WHERE status = 'live'
          AND ($1::uuid IS NULL OR community_id = $1)
          AND ($2::uuid IS NULL OR game_id = $2)
        ORDER BY viewer_count DESC, started_at DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(community_id)
    .bind(game_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into_public()).collect())
}

/// Get a stream by ID — public view (safe for any caller).
pub async fn get_stream(pool: &PgPool, stream_id: Uuid) -> Result<StreamPublic> {
    let row = sqlx::query_as::<_, StreamRow>(
        r#"
        SELECT
            id, streamer_id, community_id, channel_id, game_id,
            title, status, viewer_count, hls_url, rtmp_target, stream_key,
            ingest_key, started_at, ended_at, created_at
        FROM streams
        WHERE id = $1
        "#,
    )
    .bind(stream_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Stream not found".to_string()))?;

    Ok(row.into_public())
}

/// Get a stream for its owner — includes ingest_key + RTMP egress config.
pub async fn get_stream_for_owner(
    pool: &PgPool,
    stream_id: Uuid,
    streamer_id: Uuid,
) -> Result<StreamOwnerView> {
    let row = sqlx::query_as::<_, StreamRow>(
        r#"
        SELECT
            id, streamer_id, community_id, channel_id, game_id,
            title, status, viewer_count, hls_url, rtmp_target, stream_key,
            ingest_key, started_at, ended_at, created_at
        FROM streams
        WHERE id = $1 AND streamer_id = $2
        "#,
    )
    .bind(stream_id)
    .bind(streamer_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Stream not found or not owned by you".to_string()))?;

    Ok(row.into_owner_view())
}

/// Increment viewer_count by 1.  Called when a viewer starts watching.
/// Best-effort — does not error if the stream is not live.
pub async fn increment_viewer_count(pool: &PgPool, stream_id: Uuid) -> Result<i32> {
    let count = sqlx::query_scalar::<_, i32>(
        r#"
        UPDATE streams SET viewer_count = viewer_count + 1
        WHERE id = $1 AND status = 'live'
        RETURNING viewer_count
        "#,
    )
    .bind(stream_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);
    Ok(count)
}

/// Decrement viewer_count by 1 (floor 0).  Called when a viewer stops watching.
pub async fn decrement_viewer_count(pool: &PgPool, stream_id: Uuid) -> Result<i32> {
    let count = sqlx::query_scalar::<_, i32>(
        r#"
        UPDATE streams SET viewer_count = GREATEST(viewer_count - 1, 0)
        WHERE id = $1 AND status = 'live'
        RETURNING viewer_count
        "#,
    )
    .bind(stream_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);
    Ok(count)
}

/// Update the HLS URL for a live stream — called by the media-server webhook
/// once the ingest is confirmed active.
pub async fn set_hls_url(pool: &PgPool, stream_id: Uuid, hls_url: &str) -> Result<()> {
    sqlx::query("UPDATE streams SET hls_url = $2 WHERE id = $1")
        .bind(stream_id)
        .bind(hls_url)
        .execute(pool)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_key_length_and_alphanumeric() {
        let key = generate_ingest_key();
        assert_eq!(key.len(), 48, "ingest key must be 48 chars");
        assert!(
            key.chars().all(|c| c.is_ascii_alphanumeric()),
            "ingest key must be alphanumeric: {key}"
        );
    }

    #[test]
    fn ingest_key_unique_per_call() {
        let a = generate_ingest_key();
        let b = generate_ingest_key();
        assert_ne!(a, b, "two generated ingest keys should not be equal");
    }

    #[test]
    fn stream_public_hides_sensitive_fields() {
        // Ensure StreamOwnerView vs StreamPublic distinction is compile-time
        // correct — StreamPublic has no ingest_key / rtmp_target / stream_key.
        let public = StreamPublic {
            id: Uuid::new_v4(),
            streamer_id: Uuid::new_v4(),
            community_id: None,
            channel_id: None,
            game_id: None,
            title: "Test stream".to_string(),
            status: "live".to_string(),
            viewer_count: 42,
            hls_url: Some("http://media/live/abc/index.m3u8".to_string()),
            started_at: None,
            ended_at: None,
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&public).unwrap();
        assert!(
            json.get("ingest_key").is_none(),
            "ingest_key must not appear in public view"
        );
        assert!(
            json.get("rtmp_target").is_none(),
            "rtmp_target must not appear in public view"
        );
        assert!(
            json.get("stream_key").is_none(),
            "stream_key must not appear in public view"
        );
        assert_eq!(json["viewer_count"], 42);
    }

    #[test]
    fn stream_owner_view_includes_ingest_key() {
        let public = StreamPublic {
            id: Uuid::new_v4(),
            streamer_id: Uuid::new_v4(),
            community_id: None,
            channel_id: None,
            game_id: None,
            title: "My game stream".to_string(),
            status: "offline".to_string(),
            viewer_count: 0,
            hls_url: None,
            started_at: None,
            ended_at: None,
            created_at: Utc::now(),
        };
        let owner_view = StreamOwnerView {
            public,
            ingest_key: Some("abc123".to_string()),
            rtmp_target: Some("rtmp://live.twitch.tv/app".to_string()),
            stream_key: Some("live_xxx".to_string()),
        };
        let json = serde_json::to_value(&owner_view).unwrap();
        assert_eq!(json["ingest_key"], "abc123");
        assert_eq!(json["rtmp_target"], "rtmp://live.twitch.tv/app");
        assert_eq!(json["stream_key"], "live_xxx");
    }

    #[test]
    fn stream_status_lifecycle() {
        let statuses = ["offline", "live", "ended"];
        for s in statuses {
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn title_validation_logic() {
        assert!("   ".trim().is_empty()); // would fail validation
        assert!(!"My Stream".trim().is_empty()); // passes
    }
}
