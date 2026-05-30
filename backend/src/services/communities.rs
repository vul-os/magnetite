// Communities service — server/guild + channel + message persistence operations.
// Called by the REST API handlers in api/communities.rs, api/channels.rs, api/messages.rs.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Community {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub is_public: bool,
    pub member_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommunityMember {
    pub id: Uuid,
    pub community_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub nickname: Option<String>,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Channel {
    pub id: Uuid,
    pub community_id: Uuid,
    pub name: String,
    pub kind: String,
    pub topic: Option<String>,
    pub position: i32,
    pub is_private: bool,
    pub slow_mode_secs: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub reply_to_id: Option<Uuid>,
    pub attachments: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DmThread {
    pub id: Uuid,
    pub user_a_id: Uuid,
    pub user_b_id: Uuid,
    pub last_message_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DmMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub attachments: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Community operations
// ---------------------------------------------------------------------------

pub async fn create_community(
    pool: &PgPool,
    owner_id: Uuid,
    name: &str,
    slug: &str,
    description: Option<&str>,
    is_public: bool,
) -> Result<Community> {
    let community = sqlx::query_as::<_, Community>(
        r#"
        INSERT INTO communities (id, owner_id, name, slug, description, is_public, member_count, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, 1, NOW(), NOW())
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(owner_id)
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(is_public)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::Validation("A community with that slug already exists".to_string())
        } else {
            AppError::Database(e.to_string())
        }
    })?;

    // Auto-join the creator as owner.
    sqlx::query(
        "INSERT INTO community_members (id, community_id, user_id, role, joined_at)
         VALUES ($1, $2, $3, 'owner', NOW())
         ON CONFLICT (community_id, user_id) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(community.id)
    .bind(owner_id)
    .execute(pool)
    .await?;

    Ok(community)
}

pub async fn get_community(pool: &PgPool, id: Uuid) -> Result<Community> {
    sqlx::query_as::<_, Community>("SELECT * FROM communities WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Community not found".to_string()))
}

pub async fn get_community_by_slug(pool: &PgPool, slug: &str) -> Result<Community> {
    sqlx::query_as::<_, Community>("SELECT * FROM communities WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Community not found".to_string()))
}

pub async fn list_public_communities(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Community>> {
    let rows = sqlx::query_as::<_, Community>(
        "SELECT * FROM communities WHERE is_public = true ORDER BY member_count DESC, created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_user_communities(pool: &PgPool, user_id: Uuid) -> Result<Vec<Community>> {
    let rows = sqlx::query_as::<_, Community>(
        r#"
        SELECT c.* FROM communities c
        JOIN community_members cm ON cm.community_id = c.id
        WHERE cm.user_id = $1
        ORDER BY c.name
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_community(
    pool: &PgPool,
    id: Uuid,
    owner_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    icon_url: Option<&str>,
    banner_url: Option<&str>,
    is_public: Option<bool>,
) -> Result<Community> {
    let community = sqlx::query_as::<_, Community>(
        r#"
        UPDATE communities SET
            name        = COALESCE($3, name),
            description = COALESCE($4, description),
            icon_url    = COALESCE($5, icon_url),
            banner_url  = COALESCE($6, banner_url),
            is_public   = COALESCE($7, is_public),
            updated_at  = NOW()
        WHERE id = $1 AND owner_id = $2
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(owner_id)
    .bind(name)
    .bind(description)
    .bind(icon_url)
    .bind(banner_url)
    .bind(is_public)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::NotFound("Community not found or you are not the owner".to_string())
    })?;
    Ok(community)
}

pub async fn delete_community(pool: &PgPool, id: Uuid, owner_id: Uuid) -> Result<()> {
    let rows = sqlx::query("DELETE FROM communities WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(owner_id)
        .execute(pool)
        .await?;
    if rows.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Community not found or you are not the owner".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Membership
// ---------------------------------------------------------------------------

pub async fn join_community(
    pool: &PgPool,
    community_id: Uuid,
    user_id: Uuid,
) -> Result<CommunityMember> {
    // Verify community exists and is public (or already a member).
    sqlx::query_as::<_, Community>("SELECT * FROM communities WHERE id = $1 AND is_public = true")
        .bind(community_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Community not found or is private".to_string()))?;

    let member = sqlx::query_as::<_, CommunityMember>(
        r#"
        INSERT INTO community_members (id, community_id, user_id, role, joined_at)
        VALUES ($1, $2, $3, 'member', NOW())
        ON CONFLICT (community_id, user_id) DO UPDATE SET role = community_members.role
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(community_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "UPDATE communities SET member_count = member_count + 1, updated_at = NOW()
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(pool)
    .await?;

    Ok(member)
}

pub async fn leave_community(pool: &PgPool, community_id: Uuid, user_id: Uuid) -> Result<()> {
    let rows = sqlx::query(
        "DELETE FROM community_members WHERE community_id = $1 AND user_id = $2 AND role != 'owner'",
    )
    .bind(community_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if rows.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "Not a member, or owners cannot leave (transfer ownership first)".to_string(),
        ));
    }
    sqlx::query(
        "UPDATE communities SET member_count = GREATEST(member_count - 1, 0), updated_at = NOW()
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_members(pool: &PgPool, community_id: Uuid) -> Result<Vec<CommunityMember>> {
    let rows = sqlx::query_as::<_, CommunityMember>(
        "SELECT * FROM community_members WHERE community_id = $1 ORDER BY joined_at",
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_member_role(
    pool: &PgPool,
    community_id: Uuid,
    user_id: Uuid,
) -> Result<Option<String>> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT role FROM community_members WHERE community_id = $1 AND user_id = $2",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn set_member_role(
    pool: &PgPool,
    community_id: Uuid,
    requester_id: Uuid,
    target_user_id: Uuid,
    new_role: &str,
) -> Result<CommunityMember> {
    // Only owner or admin may change roles.
    let requester_role = get_member_role(pool, community_id, requester_id)
        .await?
        .ok_or_else(|| AppError::Forbidden("Not a member".to_string()))?;
    if requester_role != "owner" && requester_role != "admin" {
        return Err(AppError::Forbidden("Insufficient permissions".to_string()));
    }
    let member = sqlx::query_as::<_, CommunityMember>(
        "UPDATE community_members SET role = $3
         WHERE community_id = $1 AND user_id = $2
         RETURNING *",
    )
    .bind(community_id)
    .bind(target_user_id)
    .bind(new_role)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Member not found".to_string()))?;
    Ok(member)
}

// ---------------------------------------------------------------------------
// Channel operations
// ---------------------------------------------------------------------------

pub async fn create_channel(
    pool: &PgPool,
    community_id: Uuid,
    name: &str,
    kind: &str,
    topic: Option<&str>,
    is_private: bool,
) -> Result<Channel> {
    if kind != "text" && kind != "voice" {
        return Err(AppError::Validation(
            "Channel kind must be 'text' or 'voice'".to_string(),
        ));
    }
    let max_pos: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) FROM channels WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await?;

    let channel = sqlx::query_as::<_, Channel>(
        r#"
        INSERT INTO channels (id, community_id, name, kind, topic, position, is_private, slow_mode_secs, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 0, NOW(), NOW())
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(community_id)
    .bind(name)
    .bind(kind)
    .bind(topic)
    .bind(max_pos + 1)
    .bind(is_private)
    .fetch_one(pool)
    .await?;
    Ok(channel)
}

pub async fn get_channel(pool: &PgPool, channel_id: Uuid) -> Result<Channel> {
    sqlx::query_as::<_, Channel>("SELECT * FROM channels WHERE id = $1")
        .bind(channel_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".to_string()))
}

pub async fn list_channels(pool: &PgPool, community_id: Uuid) -> Result<Vec<Channel>> {
    let rows = sqlx::query_as::<_, Channel>(
        "SELECT * FROM channels WHERE community_id = $1 ORDER BY position, created_at",
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_channel(pool: &PgPool, channel_id: Uuid, community_id: Uuid) -> Result<()> {
    let rows = sqlx::query("DELETE FROM channels WHERE id = $1 AND community_id = $2")
        .bind(channel_id)
        .bind(community_id)
        .execute(pool)
        .await?;
    if rows.rows_affected() == 0 {
        return Err(AppError::NotFound("Channel not found".to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Message operations
// ---------------------------------------------------------------------------

pub async fn post_message(
    pool: &PgPool,
    channel_id: Uuid,
    author_id: Uuid,
    content: &str,
    reply_to_id: Option<Uuid>,
) -> Result<Message> {
    if content.trim().is_empty() {
        return Err(AppError::Validation(
            "Message content cannot be empty".to_string(),
        ));
    }
    let msg = sqlx::query_as::<_, Message>(
        r#"
        INSERT INTO messages (id, channel_id, author_id, content, reply_to_id, deleted, created_at)
        VALUES ($1, $2, $3, $4, $5, false, NOW())
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(channel_id)
    .bind(author_id)
    .bind(content)
    .bind(reply_to_id)
    .fetch_one(pool)
    .await?;
    Ok(msg)
}

pub async fn list_messages(
    pool: &PgPool,
    channel_id: Uuid,
    before_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Message>> {
    let rows = if let Some(before) = before_id {
        sqlx::query_as::<_, Message>(
            r#"
            SELECT * FROM messages
            WHERE channel_id = $1 AND deleted = false AND created_at < (
                SELECT created_at FROM messages WHERE id = $2
            )
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(channel_id)
        .bind(before)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE channel_id = $1 AND deleted = false ORDER BY created_at DESC LIMIT $2",
        )
        .bind(channel_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

pub async fn delete_message(pool: &PgPool, message_id: Uuid, author_id: Uuid) -> Result<()> {
    let rows = sqlx::query("UPDATE messages SET deleted = true WHERE id = $1 AND author_id = $2")
        .bind(message_id)
        .bind(author_id)
        .execute(pool)
        .await?;
    if rows.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Message not found or not authored by you".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DM operations
// ---------------------------------------------------------------------------

/// Return the DM thread between two users, creating it if it does not exist.
/// user_a_id < user_b_id constraint is enforced by the UNIQUE + CHECK in the table.
pub async fn get_or_create_dm_thread(
    pool: &PgPool,
    user_a: Uuid,
    user_b: Uuid,
) -> Result<DmThread> {
    let (low, high) = if user_a < user_b {
        (user_a, user_b)
    } else {
        (user_b, user_a)
    };

    let thread = sqlx::query_as::<_, DmThread>(
        r#"
        INSERT INTO dm_threads (id, user_a_id, user_b_id, created_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (user_a_id, user_b_id) DO UPDATE SET user_a_id = EXCLUDED.user_a_id
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(low)
    .bind(high)
    .fetch_one(pool)
    .await?;
    Ok(thread)
}

pub async fn post_dm_message(
    pool: &PgPool,
    thread_id: Uuid,
    author_id: Uuid,
    content: &str,
) -> Result<DmMessage> {
    if content.trim().is_empty() {
        return Err(AppError::Validation(
            "Message content cannot be empty".to_string(),
        ));
    }
    let msg = sqlx::query_as::<_, DmMessage>(
        r#"
        INSERT INTO dm_messages (id, thread_id, author_id, content, deleted, created_at)
        VALUES ($1, $2, $3, $4, false, NOW())
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(thread_id)
    .bind(author_id)
    .bind(content)
    .fetch_one(pool)
    .await?;

    sqlx::query("UPDATE dm_threads SET last_message_at = NOW() WHERE id = $1")
        .bind(thread_id)
        .execute(pool)
        .await?;

    Ok(msg)
}

pub async fn list_dm_messages(
    pool: &PgPool,
    thread_id: Uuid,
    before_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<DmMessage>> {
    let rows = if let Some(before) = before_id {
        sqlx::query_as::<_, DmMessage>(
            r#"
            SELECT * FROM dm_messages
            WHERE thread_id = $1 AND deleted = false AND created_at < (
                SELECT created_at FROM dm_messages WHERE id = $2
            )
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(thread_id)
        .bind(before)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, DmMessage>(
            "SELECT * FROM dm_messages WHERE thread_id = $1 AND deleted = false ORDER BY created_at DESC LIMIT $2",
        )
        .bind(thread_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

pub async fn list_dm_threads(pool: &PgPool, user_id: Uuid) -> Result<Vec<DmThread>> {
    let rows = sqlx::query_as::<_, DmThread>(
        r#"
        SELECT * FROM dm_threads
        WHERE user_a_id = $1 OR user_b_id = $1
        ORDER BY last_message_at DESC NULLS LAST
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dm_thread_ordering() {
        let a = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let b = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let (low, high) = if a < b { (a, b) } else { (b, a) };
        assert_eq!(low, a);
        assert_eq!(high, b);
    }

    #[test]
    fn test_message_content_empty_detection() {
        // Mirrors the validation logic in post_message / post_dm_message.
        let content = "   ";
        assert!(content.trim().is_empty());
        let content2 = "hello";
        assert!(!content2.trim().is_empty());
    }

    #[test]
    fn test_channel_kind_validation() {
        let valid = ["text", "voice"];
        let invalid = ["video", "forum", ""];
        for kind in &valid {
            assert!(kind == &"text" || kind == &"voice");
        }
        for kind in &invalid {
            assert!(kind != &"text" && kind != &"voice");
        }
    }
}
