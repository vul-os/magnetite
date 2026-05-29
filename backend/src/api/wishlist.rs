use axum::{extract::{Path, State}, Extension, Json};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Result, AppError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WishlistEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct WishlistItem {
    pub id: Uuid,
    pub game_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct WishlistCheckResponse {
    pub wishlisted: bool,
}

pub async fn list_wishlist(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<Vec<WishlistItem>>> {
    let items = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT w.id, w.game_id, g.title, g.description, w.created_at
         FROM wishlists w
         JOIN games g ON w.game_id = g.id
         WHERE w.user_id = $1 AND g.active = true
         ORDER BY w.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let wishlist = items
        .into_iter()
        .map(|(id, game_id, title, description, created_at)| WishlistItem {
            id,
            game_id,
            title,
            description,
            created_at,
        })
        .collect();

    Ok(Json(wishlist))
}

pub async fn add_to_wishlist(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<WishlistEntry>> {
    let existing = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM wishlists WHERE user_id = $1 AND game_id = $2",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_optional(&pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Game already in wishlist".to_string()));
    }

    let game_exists = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM games WHERE id = $1 AND active = true",
    )
    .bind(game_id)
    .fetch_optional(&pool)
    .await?;

    if game_exists.is_none() {
        return Err(AppError::NotFound("Game not found".to_string()));
    }

    let entry = sqlx::query_as::<_, WishlistEntry>(
        "INSERT INTO wishlists (user_id, game_id)
         VALUES ($1, $2)
         RETURNING id, user_id, game_id, created_at",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(entry))
}

pub async fn remove_from_wishlist(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<()>> {
    let result = sqlx::query(
        "DELETE FROM wishlists WHERE user_id = $1 AND game_id = $2",
    )
    .bind(user_id)
    .bind(game_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Wishlist entry not found".to_string()));
    }

    Ok(Json(()))
}

pub async fn check_wishlist(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<WishlistCheckResponse>> {
    let existing = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM wishlists WHERE user_id = $1 AND game_id = $2",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_optional(&pool)
    .await?;

    Ok(Json(WishlistCheckResponse {
        wishlisted: existing.is_some(),
    }))
}