// Invite service — game session invites; platform surface, not yet wired.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::api::notifications::{broadcast_notification, Notification, NotificationType};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GameInvite {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub to_user_id: Uuid,
    pub game_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInviteWithDetails {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub from_username: String,
    pub from_avatar_url: Option<String>,
    pub to_user_id: Uuid,
    pub to_username: String,
    pub to_avatar_url: Option<String>,
    pub game_id: Uuid,
    pub game_title: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinSession {
    pub session_id: Uuid,
    pub game_id: Uuid,
    pub game_title: String,
    pub inviter_id: Uuid,
    pub inviter_username: String,
    pub invite_id: Uuid,
    pub started_at: DateTime<Utc>,
}

pub struct InviteService;

impl InviteService {
    pub fn new() -> Self {
        Self
    }

    async fn are_friends(db: &sqlx::PgPool, user_id: Uuid, other_user_id: Uuid) -> Result<bool> {
        let friendship = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
        )
        .bind(user_id)
        .bind(other_user_id)
        .fetch_optional(db)
        .await?;

        Ok(friendship.is_some())
    }

    async fn is_blocked(db: &sqlx::PgPool, user_id: Uuid, other_user_id: Uuid) -> Result<bool> {
        let blocked = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM blocked_users WHERE (user_id = $1 AND blocked_id = $2) OR (user_id = $2 AND blocked_id = $1)",
        )
        .bind(user_id)
        .bind(other_user_id)
        .fetch_optional(db)
        .await?;

        Ok(blocked.is_some())
    }

    async fn get_user_email(db: &sqlx::PgPool, user_id: Uuid) -> Result<String> {
        sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))
    }

    async fn get_user_username(db: &sqlx::PgPool, user_id: Uuid) -> Result<String> {
        sqlx::query_scalar::<_, String>("SELECT username FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))
    }

    async fn game_exists(db: &sqlx::PgPool, game_id: Uuid) -> Result<bool> {
        let game =
            sqlx::query_as::<_, (Uuid,)>("SELECT id FROM games WHERE id = $1 AND active = true")
                .bind(game_id)
                .fetch_optional(db)
                .await?;

        Ok(game.is_some())
    }

    async fn get_game_title(db: &sqlx::PgPool, game_id: Uuid) -> Result<String> {
        sqlx::query_scalar::<_, String>("SELECT title FROM games WHERE id = $1")
            .bind(game_id)
            .fetch_optional(db)
            .await?
            .ok_or_else(|| AppError::NotFound("Game not found".to_string()))
    }

    pub async fn create_invite(
        &self,
        db: &sqlx::PgPool,
        email_service: &crate::services::email::EmailService,
        from_user_id: Uuid,
        to_user_id: Uuid,
        game_id: Uuid,
    ) -> Result<GameInvite> {
        if from_user_id == to_user_id {
            return Err(AppError::BadRequest(
                "Cannot invite yourself to game".to_string(),
            ));
        }

        if !Self::are_friends(db, from_user_id, to_user_id).await? {
            return Err(AppError::Forbidden(
                "Can only invite friends to games".to_string(),
            ));
        }

        if Self::is_blocked(db, from_user_id, to_user_id).await? {
            return Err(AppError::Forbidden(
                "Cannot invite blocked user".to_string(),
            ));
        }

        if !Self::game_exists(db, game_id).await? {
            return Err(AppError::NotFound("Game not found".to_string()));
        }

        let existing_pending = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM game_invites WHERE from_user_id = $1 AND to_user_id = $2 AND game_id = $3 AND status = 'pending'",
        )
        .bind(from_user_id)
        .bind(to_user_id)
        .bind(game_id)
        .fetch_optional(db)
        .await?;

        if existing_pending.is_some() {
            return Err(AppError::BadRequest("Invite already exists".to_string()));
        }

        let invite_id = Uuid::new_v4();
        let invite = sqlx::query_as::<_, GameInvite>(
            "INSERT INTO game_invites (id, from_user_id, to_user_id, game_id, status, created_at)
             VALUES ($1, $2, $3, $4, 'pending', NOW())
             RETURNING id, from_user_id, to_user_id, game_id, status, created_at",
        )
        .bind(invite_id)
        .bind(from_user_id)
        .bind(to_user_id)
        .bind(game_id)
        .fetch_one(db)
        .await?;

        let from_username = Self::get_user_username(db, from_user_id).await?;
        let to_email = Self::get_user_email(db, to_user_id).await?;
        let game_title = Self::get_game_title(db, game_id).await?;

        let subject = format!("Game Invite from {}", from_username);
        let text = format!(
            "You have been invited by {} to play {}.\n\nAccept the invite to join the game!",
            from_username, game_title
        );
        let html = format!(
            "<p>You have been invited by <strong>{}</strong> to play <strong>{}</strong>.</p><p>Accept the invite to join the game!</p>",
            from_username, game_title
        );

        if let Err(e) = email_service
            .send_email(&to_email, &subject, &text, &html)
            .await
        {
            tracing::warn!("Failed to send invite email: {}", e);
        }

        let notification = sqlx::query_as::<_, Notification>(
            "INSERT INTO notifications (id, user_id, type, title, body, data, read, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, false, NOW())
             RETURNING id, user_id, type, title, body, data, read, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(to_user_id)
        .bind(NotificationType::GameInvite.as_str())
        .bind("Game Invite")
        .bind(format!(
            "{} invited you to play {}",
            from_username, game_title
        ))
        .bind(serde_json::json!({ "game_id": game_id, "invite_id": invite_id }))
        .fetch_one(db)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        broadcast_notification(notification).await;

        Ok(invite)
    }

    pub async fn accept_invite(
        &self,
        db: &sqlx::PgPool,
        invite_id: Uuid,
        user_id: Uuid,
    ) -> Result<JoinSession> {
        let invite = sqlx::query_as::<_, GameInvite>(
            "SELECT id, from_user_id, to_user_id, game_id, status, created_at 
             FROM game_invites 
             WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
        )
        .bind(invite_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Invite not found".to_string()))?;

        sqlx::query("UPDATE game_invites SET status = 'accepted' WHERE id = $1")
            .bind(invite_id)
            .execute(db)
            .await?;

        let session_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO play_sessions (id, game_id, user_id, status, started_at) VALUES ($1, $2, $3, 'active', $4)",
        )
        .bind(session_id)
        .bind(invite.game_id)
        .bind(user_id)
        .bind(now)
        .execute(db)
        .await?;

        let inviter_username = Self::get_user_username(db, invite.from_user_id).await?;
        let game_title = Self::get_game_title(db, invite.game_id).await?;

        Ok(JoinSession {
            session_id,
            game_id: invite.game_id,
            game_title,
            inviter_id: invite.from_user_id,
            inviter_username,
            invite_id,
            started_at: now,
        })
    }

    pub async fn decline_invite(
        &self,
        db: &sqlx::PgPool,
        invite_id: Uuid,
        user_id: Uuid,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE game_invites SET status = 'declined' WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
        )
        .bind(invite_id)
        .bind(user_id)
        .execute(db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Invite not found".to_string()));
        }

        Ok(())
    }

    pub async fn cancel_invite(
        &self,
        db: &sqlx::PgPool,
        invite_id: Uuid,
        user_id: Uuid,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE game_invites SET status = 'cancelled' WHERE id = $1 AND from_user_id = $2 AND status = 'pending'",
        )
        .bind(invite_id)
        .bind(user_id)
        .execute(db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Invite not found".to_string()));
        }

        Ok(())
    }

    pub async fn get_pending_invites(
        &self,
        db: &sqlx::PgPool,
        user_id: Uuid,
    ) -> Result<Vec<GameInviteWithDetails>> {
        let invites = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, Uuid, String, Option<String>, Uuid, String, String, chrono::DateTime<chrono::Utc>)>(
            r#"
            SELECT
                gi.id, gi.from_user_id, fu.username as from_username, fu.avatar_url as from_avatar_url,
                gi.to_user_id, tu.username as to_username, tu.avatar_url as to_avatar_url,
                gi.game_id, g.title as game_title, gi.status, gi.created_at
            FROM game_invites gi
            JOIN users fu ON gi.from_user_id = fu.id
            JOIN users tu ON gi.to_user_id = tu.id
            JOIN games g ON gi.game_id = g.id
            WHERE gi.to_user_id = $1 AND gi.status = 'pending'
            ORDER BY gi.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(db)
        .await?;

        let invite_list: Vec<GameInviteWithDetails> = invites
            .into_iter()
            .map(
                |(
                    id,
                    from_user_id,
                    from_username,
                    from_avatar_url,
                    to_user_id,
                    to_username,
                    to_avatar_url,
                    game_id,
                    game_title,
                    status,
                    created_at,
                )| {
                    GameInviteWithDetails {
                        id,
                        from_user_id,
                        from_username,
                        from_avatar_url,
                        to_user_id,
                        to_username,
                        to_avatar_url,
                        game_id,
                        game_title,
                        status,
                        created_at,
                    }
                },
            )
            .collect();

        Ok(invite_list)
    }

    pub async fn get_sent_invites(
        &self,
        db: &sqlx::PgPool,
        user_id: Uuid,
    ) -> Result<Vec<GameInviteWithDetails>> {
        let invites = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, Uuid, String, Option<String>, Uuid, String, String, chrono::DateTime<chrono::Utc>)>(
            r#"
            SELECT
                gi.id, gi.from_user_id, fu.username as from_username, fu.avatar_url as from_avatar_url,
                gi.to_user_id, tu.username as to_username, tu.avatar_url as to_avatar_url,
                gi.game_id, g.title as game_title, gi.status, gi.created_at
            FROM game_invites gi
            JOIN users fu ON gi.from_user_id = fu.id
            JOIN users tu ON gi.to_user_id = tu.id
            JOIN games g ON gi.game_id = g.id
            WHERE gi.from_user_id = $1 AND gi.status = 'pending'
            ORDER BY gi.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(db)
        .await?;

        let invite_list: Vec<GameInviteWithDetails> = invites
            .into_iter()
            .map(
                |(
                    id,
                    from_user_id,
                    from_username,
                    from_avatar_url,
                    to_user_id,
                    to_username,
                    to_avatar_url,
                    game_id,
                    game_title,
                    status,
                    created_at,
                )| {
                    GameInviteWithDetails {
                        id,
                        from_user_id,
                        from_username,
                        from_avatar_url,
                        to_user_id,
                        to_username,
                        to_avatar_url,
                        game_id,
                        game_title,
                        status,
                        created_at,
                    }
                },
            )
            .collect();

        Ok(invite_list)
    }
}

impl Default for InviteService {
    fn default() -> Self {
        Self::new()
    }
}
