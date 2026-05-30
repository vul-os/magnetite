// Presence service — tracks online/idle/in_game/offline status per user.
// The WebSocket handler upserts presence on connect/disconnect; the REST API
// exposes read endpoints for friend lists and community member panels.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Presence {
    pub user_id: Uuid,
    pub status: String,
    pub activity: Option<String>,
    pub game_id: Option<Uuid>,
    pub last_seen: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceUpdate {
    pub status: String,
    pub activity: Option<String>,
    pub game_id: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Upsert a user's presence record.  Called from the WS comms handler on
/// join/leave and optionally by the SDK when entering/exiting a game.
pub async fn set_presence(
    pool: &PgPool,
    user_id: Uuid,
    status: &str,
    activity: Option<&str>,
    game_id: Option<Uuid>,
) -> Result<Presence> {
    let presence = sqlx::query_as::<_, Presence>(
        r#"
        INSERT INTO presence (user_id, status, activity, game_id, last_seen, updated_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW())
        ON CONFLICT (user_id) DO UPDATE SET
            status     = EXCLUDED.status,
            activity   = EXCLUDED.activity,
            game_id    = EXCLUDED.game_id,
            last_seen  = NOW(),
            updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(status)
    .bind(activity)
    .bind(game_id)
    .fetch_one(pool)
    .await?;
    Ok(presence)
}

/// Mark a user as offline (called on WS disconnect).
pub async fn set_offline(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO presence (user_id, status, last_seen, updated_at)
        VALUES ($1, 'offline', NOW(), NOW())
        ON CONFLICT (user_id) DO UPDATE SET
            status     = 'offline',
            activity   = NULL,
            game_id    = NULL,
            last_seen  = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get the current presence for a single user.
pub async fn get_presence(pool: &PgPool, user_id: Uuid) -> Result<Option<Presence>> {
    let p = sqlx::query_as::<_, Presence>("SELECT * FROM presence WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(p)
}

/// Bulk-fetch presence for a list of user IDs.  Used by the community member
/// panel and friend list to annotate each entry with an online indicator.
pub async fn get_bulk_presence(pool: &PgPool, user_ids: &[Uuid]) -> Result<Vec<Presence>> {
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    // sqlx does not support `= ANY($1::uuid[])` with a Vec<Uuid> directly in
    // all driver versions; build the query with explicit binding.
    let rows = sqlx::query_as::<_, Presence>("SELECT * FROM presence WHERE user_id = ANY($1)")
        .bind(user_ids)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// List all users currently online in a community (intersection of members and
/// presence rows where status != 'offline').
pub async fn list_online_community_members(
    pool: &PgPool,
    community_id: Uuid,
) -> Result<Vec<Presence>> {
    let rows = sqlx::query_as::<_, Presence>(
        r#"
        SELECT p.* FROM presence p
        JOIN community_members cm ON cm.user_id = p.user_id
        WHERE cm.community_id = $1 AND p.status != 'offline'
        ORDER BY p.last_seen DESC
        "#,
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presence_status_values() {
        let statuses = ["online", "idle", "dnd", "in_game", "offline"];
        for s in &statuses {
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn test_presence_update_serde() {
        let update = PresenceUpdate {
            status: "online".to_string(),
            activity: Some("Playing Rustcraft".to_string()),
            game_id: None,
        };
        let json = serde_json::to_string(&update).unwrap();
        let decoded: PresenceUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status, "online");
        assert_eq!(decoded.activity.as_deref(), Some("Playing Rustcraft"));
    }
}
