use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::response;
use crate::error::{AppError, Result};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Review {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_id: Uuid,
    pub rating: i32,
    pub content: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReviewWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub game_id: Uuid,
    pub rating: i32,
    pub content: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReviewRequest {
    pub rating: i32,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReviewRequest {
    pub rating: Option<i32>,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewListQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedReviews {
    pub reviews: Vec<ReviewWithUser>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct GameRating {
    pub average: f64,
    pub count: i64,
}

async fn has_played_game(pool: &PgPool, user_id: Uuid, game_id: Uuid) -> Result<bool> {
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM play_sessions WHERE user_id = $1 AND game_id = $2 AND status = 'completed'",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_one(pool)
    .await?;

    Ok(result > 0)
}

pub async fn list_reviews(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Query(query): Query<ReviewListQuery>,
) -> Result<Json<response::PaginatedResponse<ReviewWithUser>>> {
    let page = query.page.unwrap_or(1).max(1) as u32;
    let limit = query.limit.unwrap_or(20).clamp(1, 100) as u32;
    let offset = (page - 1) * limit;

    let sort = query.sort.as_deref().unwrap_or("recent");
    let order_clause = match sort {
        "rating_high" => "r.rating DESC, r.created_at DESC",
        "rating_low" => "r.rating ASC, r.created_at DESC",
        _ => "r.created_at DESC",
    };

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reviews WHERE game_id = $1",
    )
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    let reviews = sqlx::query_as::<_, ReviewWithUser>(
        &format!(
            "SELECT r.id, r.user_id, u.username, r.game_id, r.rating, r.content, r.created_at, r.updated_at
             FROM reviews r
             JOIN users u ON r.user_id = u.id
             WHERE r.game_id = $1
             ORDER BY {}
             LIMIT $2 OFFSET $3",
            order_clause
        ),
    )
    .bind(game_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    Ok(response::paginated(reviews, page, limit, total as u64))
}

pub async fn create_review(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<CreateReviewRequest>,
) -> Result<Json<response::ApiResponse<Review>>> {
    if payload.rating < 1 || payload.rating > 5 {
        return Err(AppError::Validation("Rating must be between 1 and 5".to_string()));
    }

    let game_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM games WHERE id = $1 AND active = true)",
    )
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    if !game_exists {
        return Err(AppError::NotFound("Game not found".to_string()));
    }

    if !has_played_game(&pool, user_id, game_id).await? {
        return Err(AppError::Forbidden("You must play the game before reviewing it".to_string()));
    }

    let existing_review = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM reviews WHERE user_id = $1 AND game_id = $2)",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    if existing_review {
        return Err(AppError::BadRequest("You have already reviewed this game".to_string()));
    }

    let review = sqlx::query_as::<_, Review>(
        "INSERT INTO reviews (user_id, game_id, rating, content, created_at, updated_at)
         VALUES ($1, $2, $3, $4, NOW(), NOW())
         RETURNING id, user_id, game_id, rating, content, created_at, updated_at",
    )
    .bind(user_id)
    .bind(game_id)
    .bind(payload.rating)
    .bind(&payload.content)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(review))
}

pub async fn update_review(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(review_id): Path<Uuid>,
    Json(payload): Json<UpdateReviewRequest>,
) -> Result<Json<response::ApiResponse<Review>>> {
    let existing = sqlx::query_as::<_, Review>(
        "SELECT id, user_id, game_id, rating, content, created_at, updated_at FROM reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Review not found".to_string()))?;

    if existing.user_id != user_id {
        return Err(AppError::Forbidden("You can only update your own reviews".to_string()));
    }

    if let Some(rating) = payload.rating {
        if rating < 1 || rating > 5 {
            return Err(AppError::Validation("Rating must be between 1 and 5".to_string()));
        }
    }

    let updated_review = sqlx::query_as::<_, Review>(
        "UPDATE reviews SET
         rating = COALESCE($1, rating),
         content = COALESCE($2, content),
         updated_at = NOW()
         WHERE id = $3
         RETURNING id, user_id, game_id, rating, content, created_at, updated_at",
    )
    .bind(payload.rating)
    .bind(&payload.content)
    .bind(review_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(updated_review))
}

pub async fn delete_review(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(review_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    let existing = sqlx::query_as::<_, Review>(
        "SELECT id, user_id, game_id, rating, content, created_at, updated_at FROM reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Review not found".to_string()))?;

    if existing.user_id != user_id {
        return Err(AppError::Forbidden("You can only delete your own reviews".to_string()));
    }

    sqlx::query("DELETE FROM reviews WHERE id = $1")
        .bind(review_id)
        .execute(&pool)
        .await?;

    Ok(response::success_response(()))
}

pub async fn get_game_rating(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<GameRating>>> {
    let result = sqlx::query_as::<_, (f64, i64)>(
        "SELECT COALESCE(AVG(rating)::float, 0), COUNT(*) FROM reviews WHERE game_id = $1",
    )
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(GameRating {
        average: result.0,
        count: result.1,
    }))
}