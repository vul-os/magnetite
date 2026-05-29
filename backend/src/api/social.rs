use axum::{
    extract::{Path, Query, State, Extension},
    middleware::from_fn_with_state,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::notifications::NotificationService;
use crate::api::response;
use crate::error::{Result, AppError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FriendRequest {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub to_user_id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Friendship {
    pub id: Uuid,
    pub user_id: Uuid,
    pub friend_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GameInvite {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub to_user_id: Uuid,
    pub game_id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct Friend {
    pub user_id: Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
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
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SendFriendRequestRequest {
    pub to_user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct GameInviteRequest {
    pub to_user_id: Uuid,
    pub game_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<i32>,
}

pub async fn list_friends(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::PaginatedResponse<Friend>>> {
    let friends = sqlx::query_as::<_, (Uuid, String, Option<String>, chrono::DateTime<chrono::Utc>)>(
        r#"
        SELECT u.id, u.username, u.avatar_url, f.created_at
        FROM friendships f
        JOIN users u ON (f.friend_id = u.id AND f.user_id = $1) OR (f.user_id = u.id AND f.friend_id = $1)
        WHERE f.user_id = $1 OR f.friend_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let friend_list: Vec<Friend> = friends
        .into_iter()
        .map(|(id, username, avatar_url, created_at)| Friend {
            user_id: id,
            username,
            avatar_url,
            created_at,
        })
        .collect();

    let total = friend_list.len() as u64;
    Ok(response::paginated(friend_list, 1, 100, total))
}

pub async fn send_friend_request(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<SendFriendRequestRequest>,
) -> Result<Json<response::ApiResponse<FriendRequest>>> {
    if user_id == payload.to_user_id {
        return Err(AppError::BadRequest("Cannot send friend request to yourself".to_string()));
    }

    let existing = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
    )
    .bind(user_id)
    .bind(payload.to_user_id)
    .fetch_optional(&pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Already friends".to_string()));
    }

    let existing_request = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM friend_requests WHERE ((from_user_id = $1 AND to_user_id = $2) OR (from_user_id = $2 AND to_user_id = $1)) AND status = 'pending'",
    )
    .bind(user_id)
    .bind(payload.to_user_id)
    .fetch_optional(&pool)
    .await?;

    if existing_request.is_some() {
        return Err(AppError::BadRequest("Friend request already exists".to_string()));
    }

    let blocked = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM blocked_users WHERE (user_id = $1 AND blocked_id = $2) OR (user_id = $2 AND blocked_id = $1)",
    )
    .bind(user_id)
    .bind(payload.to_user_id)
    .fetch_optional(&pool)
    .await?;

    if blocked.is_some() {
        return Err(AppError::Forbidden("Cannot send friend request to blocked user".to_string()));
    }

    let request_id = Uuid::new_v4();
    let request = sqlx::query_as::<_, FriendRequest>(
        "INSERT INTO friend_requests (id, from_user_id, to_user_id, status, created_at)
         VALUES ($1, $2, $3, 'pending', NOW())
         RETURNING id, from_user_id, to_user_id, status, created_at",
    )
    .bind(request_id)
    .bind(user_id)
    .bind(payload.to_user_id)
    .fetch_one(&pool)
    .await?;

    let from_user = sqlx::query_as::<_, (String,)>(
        "SELECT username FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let notif_service = NotificationService::new(pool.clone());
    let _ = notif_service.create_friend_request_notification(payload.to_user_id, &from_user.0).await;

    Ok(response::success_response(request))
}

pub async fn accept_friend_request(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(request_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    let request = sqlx::query_as::<_, FriendRequest>(
        "SELECT id, from_user_id, to_user_id, status, created_at FROM friend_requests WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
    )
    .bind(request_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Friend request not found".to_string()))?;

    sqlx::query(
        "UPDATE friend_requests SET status = 'accepted' WHERE id = $1",
    )
    .bind(request_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO friendships (id, user_id, friend_id, created_at) VALUES ($1, $2, $3, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(request.from_user_id)
    .bind(request.to_user_id)
    .execute(&pool)
    .await?;

    let notif_service = NotificationService::new(pool.clone());

    let to_user_username = sqlx::query_as::<_, (String,)>(
        "SELECT username FROM users WHERE id = $1",
    )
    .bind(request.to_user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let _ = notif_service.create_system_notification(
        request.from_user_id,
        "Friend Request Accepted",
        &format!("{} accepted your friend request", to_user_username.0),
    ).await;

    Ok(response::success_response(()))
}

pub async fn reject_friend_request(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(request_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    let result = sqlx::query(
        "UPDATE friend_requests SET status = 'rejected' WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
    )
    .bind(request_id)
    .bind(user_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Friend request not found".to_string()));
    }

    Ok(response::success_response(()))
}

pub async fn remove_friend(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(friend_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    let result = sqlx::query(
        "DELETE FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
    )
    .bind(user_id)
    .bind(friend_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Friendship not found".to_string()));
    }

    Ok(response::success_response(()))
}

pub async fn block_user(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(blocked_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    if user_id == blocked_id {
        return Err(AppError::BadRequest("Cannot block yourself".to_string()));
    }

    let existing = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM blocked_users WHERE user_id = $1 AND blocked_id = $2",
    )
    .bind(user_id)
    .bind(blocked_id)
    .fetch_optional(&pool)
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
    .execute(&pool)
    .await?;

    sqlx::query(
        "DELETE FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
    )
    .bind(user_id)
    .bind(blocked_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        "DELETE FROM friend_requests WHERE (from_user_id = $1 AND to_user_id = $2) OR (from_user_id = $2 AND to_user_id = $1)",
    )
    .bind(user_id)
    .bind(blocked_id)
    .execute(&pool)
    .await?;

    Ok(response::success_response(()))
}

pub async fn invite_to_game(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<GameInviteRequest>,
) -> Result<Json<response::ApiResponse<GameInvite>>> {
    if user_id == payload.to_user_id {
        return Err(AppError::BadRequest("Cannot invite yourself to game".to_string()));
    }

    let friendship = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
    )
    .bind(user_id)
    .bind(payload.to_user_id)
    .fetch_optional(&pool)
    .await?;

    if friendship.is_none() {
        return Err(AppError::Forbidden("Can only invite friends to games".to_string()));
    }

    let game = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND active = true",
    )
    .bind(payload.game_id)
    .fetch_optional(&pool)
    .await?;

    if game.is_none() {
        return Err(AppError::NotFound("Game not found".to_string()));
    }

    let blocked = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM blocked_users WHERE (user_id = $1 AND blocked_id = $2) OR (user_id = $2 AND blocked_id = $1)",
    )
    .bind(user_id)
    .bind(payload.to_user_id)
    .fetch_optional(&pool)
    .await?;

    if blocked.is_some() {
        return Err(AppError::Forbidden("Cannot invite blocked user".to_string()));
    }

    let invite_id = Uuid::new_v4();
    let invite = sqlx::query_as::<_, GameInvite>(
        "INSERT INTO game_invites (id, from_user_id, to_user_id, game_id, status, created_at)
         VALUES ($1, $2, $3, $4, 'pending', NOW())
         RETURNING id, from_user_id, to_user_id, game_id, status, created_at",
    )
    .bind(invite_id)
    .bind(user_id)
    .bind(payload.to_user_id)
    .bind(payload.game_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(invite))
}

pub async fn list_invites(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::PaginatedResponse<GameInviteWithDetails>>> {
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
        WHERE (gi.from_user_id = $1 OR gi.to_user_id = $1) AND gi.status = 'pending'
        ORDER BY gi.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let invite_list: Vec<GameInviteWithDetails> = invites
        .into_iter()
        .map(|(id, from_user_id, from_username, from_avatar_url, to_user_id, to_username, to_avatar_url, game_id, game_title, status, created_at)| {
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
        })
        .collect();

    let total = invite_list.len() as u64;
    Ok(response::paginated(invite_list, 1, 100, total))
}

pub async fn accept_invite(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(invite_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    let result = sqlx::query(
        "UPDATE game_invites SET status = 'accepted' WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
    )
    .bind(invite_id)
    .bind(user_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Invite not found".to_string()));
    }

    Ok(response::success_response(()))
}

pub async fn decline_invite(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(invite_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    let result = sqlx::query(
        "UPDATE game_invites SET status = 'declined' WHERE id = $1 AND to_user_id = $2 AND status = 'pending'",
    )
    .bind(invite_id)
    .bind(user_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Invite not found".to_string()));
    }

    Ok(response::success_response(()))
}

pub async fn search_users(
    State(pool): State<PgPool>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<response::PaginatedResponse<User>>> {
    let limit = query.limit.unwrap_or(20).min(100) as i32;
    let search_pattern = format!("%{}%", query.q);

    let users = sqlx::query_as::<_, User>(
        "SELECT id, username, avatar_url, created_at FROM users WHERE username ILIKE $1 LIMIT $2",
    )
    .bind(search_pattern)
    .bind(limit)
    .fetch_all(&pool)
    .await?;

    let total = users.len() as u64;
    Ok(response::paginated(users, 1, limit as u32, total))
}

pub async fn get_user_profile(
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<User>>> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, avatar_url, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(response::success_response(user))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_friends).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/request", post(send_friend_request).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/accept/:id", post(accept_friend_request).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/reject/:id", post(reject_friend_request).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/:id", delete(remove_friend).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/block/:id", post(block_user).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .with_state(pool)
}

pub fn invites_router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_invites).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/:id/accept", post(accept_invite).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/:id/decline", post(decline_invite).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .with_state(pool)
}

pub fn users_router(pool: PgPool) -> Router {
    Router::new()
        .route("/search", get(search_users))
        .route("/:id", get(get_user_profile))
        .with_state(pool)
}
