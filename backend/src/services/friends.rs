// Friend service — social graph operations (send/accept/reject/remove/block/unblock).
// Platform surface API; callers are api/social.rs and future SDK bindings.
#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

use crate::api::social::{FriendRequest, Friendship, User};
use crate::error::{AppError, Result};

pub struct FriendService;

impl FriendService {
    pub fn new() -> Self {
        Self
    }

    pub async fn send_request(
        &self,
        pool: &PgPool,
        from_user_id: Uuid,
        to_user_id: Uuid,
    ) -> Result<FriendRequest> {
        if from_user_id == to_user_id {
            return Err(AppError::BadRequest(
                "Cannot send friend request to yourself".to_string(),
            ));
        }

        let existing = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
        )
        .bind(from_user_id)
        .bind(to_user_id)
        .fetch_optional(pool)
        .await?;

        if existing.is_some() {
            return Err(AppError::BadRequest("Already friends".to_string()));
        }

        let existing_request = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM friend_requests WHERE ((from_user_id = $1 AND to_user_id = $2) OR (from_user_id = $2 AND to_user_id = $1)) AND status = 'pending'",
        )
        .bind(from_user_id)
        .bind(to_user_id)
        .fetch_optional(pool)
        .await?;

        if existing_request.is_some() {
            return Err(AppError::BadRequest(
                "Friend request already exists".to_string(),
            ));
        }

        let blocked = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM blocked_users WHERE (user_id = $1 AND blocked_id = $2) OR (user_id = $2 AND blocked_id = $1)",
        )
        .bind(from_user_id)
        .bind(to_user_id)
        .fetch_optional(pool)
        .await?;

        if blocked.is_some() {
            return Err(AppError::Forbidden(
                "Cannot send friend request to blocked user".to_string(),
            ));
        }

        let request_id = Uuid::new_v4();
        let request = sqlx::query_as::<_, FriendRequest>(
            "INSERT INTO friend_requests (id, from_user_id, to_user_id, status, created_at)
             VALUES ($1, $2, $3, 'pending', NOW())
             RETURNING id, from_user_id, to_user_id, status, created_at",
        )
        .bind(request_id)
        .bind(from_user_id)
        .bind(to_user_id)
        .fetch_one(pool)
        .await?;

        Ok(request)
    }

    pub async fn accept_request(
        &self,
        pool: &PgPool,
        request_id: Uuid,
        user_id: Uuid,
    ) -> Result<Friendship> {
        let request = sqlx::query_as::<_, FriendRequest>(
            "SELECT id, from_user_id, to_user_id, status, created_at FROM friend_requests WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
        )
        .bind(request_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Friend request not found".to_string()))?;

        sqlx::query("UPDATE friend_requests SET status = 'accepted' WHERE id = $1")
            .bind(request_id)
            .execute(pool)
            .await?;

        let friendship_id = Uuid::new_v4();
        let friendship = sqlx::query_as::<_, Friendship>(
            "INSERT INTO friendships (id, user_id, friend_id, created_at) VALUES ($1, $2, $3, NOW()) RETURNING id, user_id, friend_id, created_at",
        )
        .bind(friendship_id)
        .bind(request.from_user_id)
        .bind(request.to_user_id)
        .fetch_one(pool)
        .await?;

        Ok(friendship)
    }

    pub async fn reject_request(
        &self,
        pool: &PgPool,
        request_id: Uuid,
        user_id: Uuid,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE friend_requests SET status = 'rejected' WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
        )
        .bind(request_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Friend request not found".to_string()));
        }

        Ok(())
    }

    pub async fn remove_friend(&self, pool: &PgPool, user_id: Uuid, friend_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            "DELETE FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
        )
        .bind(user_id)
        .bind(friend_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Friendship not found".to_string()));
        }

        Ok(())
    }

    pub async fn block(&self, pool: &PgPool, user_id: Uuid, blocked_id: Uuid) -> Result<()> {
        if user_id == blocked_id {
            return Err(AppError::BadRequest("Cannot block yourself".to_string()));
        }

        let existing = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM blocked_users WHERE user_id = $1 AND blocked_id = $2",
        )
        .bind(user_id)
        .bind(blocked_id)
        .fetch_optional(pool)
        .await?;

        if existing.is_some() {
            return Err(AppError::BadRequest("User already blocked".to_string()));
        }

        sqlx::query(
            "INSERT INTO blocked_users (id, user_id, blocked_id, created_at) VALUES ($1, $2, $3, NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(blocked_id)
        .execute(pool)
        .await?;

        sqlx::query(
            "DELETE FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
        )
        .bind(user_id)
        .bind(blocked_id)
        .execute(pool)
        .await?;

        sqlx::query(
            "DELETE FROM friend_requests WHERE (from_user_id = $1 AND to_user_id = $2) OR (from_user_id = $2 AND to_user_id = $1)",
        )
        .bind(user_id)
        .bind(blocked_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn unblock(&self, pool: &PgPool, user_id: Uuid, blocked_id: Uuid) -> Result<()> {
        let result =
            sqlx::query("DELETE FROM blocked_users WHERE user_id = $1 AND blocked_id = $2")
                .bind(user_id)
                .bind(blocked_id)
                .execute(pool)
                .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Blocked user not found".to_string()));
        }

        Ok(())
    }

    pub async fn get_friends(&self, pool: &PgPool, user_id: Uuid) -> Result<Vec<User>> {
        let friends = sqlx::query_as::<_, User>(
            r#"
            SELECT u.id, u.username, u.avatar_url, u.created_at
            FROM friendships f
            JOIN users u ON (f.friend_id = u.id AND f.user_id = $1) OR (f.user_id = u.id AND f.friend_id = $1)
            WHERE f.user_id = $1 OR f.friend_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(friends)
    }

    pub async fn get_pending_requests(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<FriendRequest>> {
        let requests = sqlx::query_as::<_, FriendRequest>(
            "SELECT id, from_user_id, to_user_id, status, created_at FROM friend_requests WHERE to_user_id = $1 AND status = 'pending' ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(requests)
    }

    pub async fn is_blocked(&self, pool: &PgPool, user_id: Uuid, other_id: Uuid) -> Result<bool> {
        let blocked = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM blocked_users WHERE (user_id = $1 AND blocked_id = $2) OR (user_id = $2 AND blocked_id = $1)",
        )
        .bind(user_id)
        .bind(other_id)
        .fetch_optional(pool)
        .await?;

        Ok(blocked.is_some())
    }
}
