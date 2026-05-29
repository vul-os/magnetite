use axum::{
    extract::{Path, State, Extension},
    Json,
    Router,
    routing::{get, put, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Result, AppError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub avatar_url: Option<String>,
    pub is_developer: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserProfileWithStats {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub avatar_url: Option<String>,
    pub is_developer: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub games_played: i64,
    pub total_play_time_seconds: i64,
    pub achievements_count: i64,
    pub friends_count: i64,
}

#[derive(Debug, Serialize)]
pub struct PublicUserProfile {
    pub id: Uuid,
    pub username: String,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub avatar_url: Option<String>,
    pub is_developer: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserStats {
    pub games_played: i64,
    pub total_play_time_seconds: i64,
    pub achievements_count: i64,
    pub friends_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub username: Option<String>,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AvatarUploadResponse {
    pub avatar_url: String,
}

pub async fn get_me(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<UserProfileWithStats>> {
    let profile = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<String>, Option<String>, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, username, email, bio, location, avatar_url, is_developer, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let (id, username, email, bio, location, avatar_url, is_developer, created_at) = profile;

    let stats = get_user_stats_internal(&pool, user_id).await?;

    Ok(Json(UserProfileWithStats {
        id,
        username,
        email,
        bio,
        location,
        avatar_url,
        is_developer,
        created_at,
        games_played: stats.games_played,
        total_play_time_seconds: stats.total_play_time_seconds,
        achievements_count: stats.achievements_count,
        friends_count: stats.friends_count,
    }))
}

pub async fn update_me(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<UserProfile>> {
    if let Some(ref username) = payload.username {
        if username.is_empty() || username.len() > 100 {
            return Err(AppError::Validation("Invalid username".to_string()));
        }

        let existing = sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM users WHERE username = $1 AND id != $2",
        )
        .bind(username)
        .bind(user_id)
        .fetch_optional(&pool)
        .await?;

        if existing.is_some() {
            return Err(AppError::BadRequest("Username already taken".to_string()));
        }
    }

    let current = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        "SELECT username, bio, location, avatar_url FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let new_username = payload.username.unwrap_or(current.0);
    let new_bio = payload.bio.or(current.1);
    let new_location = payload.location.or(current.2);
    let new_avatar_url = payload.avatar_url.or(current.3);

    let updated = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<String>, Option<String>, bool, chrono::DateTime<chrono::Utc>)>(
        "UPDATE users SET username = $1, bio = $2, location = $3, avatar_url = $4, updated_at = NOW() WHERE id = $5 RETURNING id, username, email, bio, location, avatar_url, is_developer, created_at",
    )
    .bind(&new_username)
    .bind(&new_bio)
    .bind(&new_location)
    .bind(&new_avatar_url)
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(UserProfile {
        id: updated.0,
        username: updated.1,
        email: updated.2,
        bio: updated.3,
        location: updated.4,
        avatar_url: updated.5,
        is_developer: updated.6,
        created_at: updated.7,
    }))
}

pub async fn get_user(
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<PublicUserProfile>> {
    let profile = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>, Option<String>, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, username, bio, location, avatar_url, is_developer, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(PublicUserProfile {
        id: profile.0,
        username: profile.1,
        bio: profile.2,
        location: profile.3,
        avatar_url: profile.4,
        is_developer: profile.5,
        created_at: profile.6,
    }))
}

pub async fn get_user_stats(
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserStats>> {
    let _user_exists = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let stats = get_user_stats_internal(&pool, user_id).await?;

    Ok(Json(stats))
}

async fn get_user_stats_internal(pool: &PgPool, user_id: Uuid) -> Result<UserStats> {
    let games_played = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM play_sessions WHERE user_id = $1 AND status = 'completed'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .0;

    let total_play_time = sqlx::query_as::<_, (Option<i64>,)>(
        "SELECT SUM(EXTRACT(EPOCH FROM (ended_at - started_at))::INTEGER) FROM play_sessions WHERE user_id = $1 AND ended_at IS NOT NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .0
    .unwrap_or(0);

    let achievements_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM user_achievements WHERE user_id = $1 AND unlocked_at IS NOT NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .0;

    let friends_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM friendships WHERE user_id = $1 OR friend_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .0;

    Ok(UserStats {
        games_played,
        total_play_time_seconds: total_play_time,
        achievements_count,
        friends_count,
    })
}

pub async fn upload_avatar(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    mut payload: axum::extract::Multipart,
) -> Result<Json<AvatarUploadResponse>> {
    let field = payload.next_field().await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {}", e)))?
        .ok_or_else(|| AppError::BadRequest("No file provided".to_string()))?;

    let filename = field.file_name()
        .ok_or_else(|| AppError::BadRequest("Invalid filename".to_string()))?
        .to_string();

    let content_type = field.content_type()
        .ok_or_else(|| AppError::BadRequest("Invalid content type".to_string()))?;

    if !content_type.starts_with("image/") {
        return Err(AppError::BadRequest("Only image files are allowed".to_string()));
    }

    let extension = filename.rsplit('.').next()
        .ok_or_else(|| AppError::BadRequest("Invalid file extension".to_string()))?;

    let allowed_extensions = ["jpg", "jpeg", "png", "gif", "webp"];
    if !allowed_extensions.contains(&extension.to_lowercase().as_str()) {
        return Err(AppError::BadRequest("Invalid file extension".to_string()));
    }

    let data: Vec<u8> = field.bytes().await
        .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?
        .to_vec();

    if data.len() > 5 * 1024 * 1024 {
        return Err(AppError::BadRequest("File too large (max 5MB)".to_string()));
    }

    let avatar_id = Uuid::new_v4();
    let avatar_filename = format!("{}.{}", avatar_id, extension);
    let _avatar_url = format!("/avatars/{}", avatar_filename);

    let uploads_dir = std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "./uploads".to_string());
    let avatars_dir = format!("{}/avatars", uploads_dir);

    tokio::fs::create_dir_all(&avatars_dir).await
        .map_err(|e| AppError::Internal(format!("Failed to create directory: {}", e)))?;

    let avatar_path = format!("{}/{}", avatars_dir, avatar_filename);
    tokio::fs::write(&avatar_path, &data).await
        .map_err(|e| AppError::Internal(format!("Failed to save file: {}", e)))?;

    let full_avatar_url = format!("/api/users/me/avatar/{}", avatar_filename);

    sqlx::query(
        "UPDATE users SET avatar_url = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&full_avatar_url)
    .bind(user_id)
    .execute(&pool)
    .await?;

    Ok(Json(AvatarUploadResponse {
        avatar_url: full_avatar_url,
    }))
}

pub async fn serve_avatar(
    Path(filename): Path<String>,
) -> Result<impl axum::response::IntoResponse> {
    let uploads_dir = std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "./uploads".to_string());
    let avatar_path = format!("{}/avatars/{}", uploads_dir, filename);

    let data = tokio::fs::read(&avatar_path).await
        .map_err(|_| AppError::NotFound("Avatar not found".to_string()))?;

    let extension = filename.rsplit('.').next().unwrap_or("png").to_lowercase();
    let content_type = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };

    Ok(axum::response::Response::builder()
        .status(200)
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=31536000")
        .body(axum::body::Body::from(data))
        .unwrap())
}

