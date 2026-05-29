use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::games::Game;
use crate::api::response;
use crate::error::{AppError, Result};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Serialize)]
pub struct CategoryWithGameCount {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub game_count: i64,
}

pub async fn list_categories(
    State(pool): State<PgPool>,
) -> Result<Json<response::PaginatedResponse<CategoryWithGameCount>>> {
    let categories: Vec<CategoryWithGameCount> = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            Option<String>,
            Option<String>,
            i32,
            i64,
        ),
    >(
        "SELECT c.id, c.name, c.slug, c.icon, c.description, c.sort_order,
                COUNT(g.id) as game_count
         FROM categories c
         LEFT JOIN games g ON g.category_id = c.id AND g.active = true
         GROUP BY c.id, c.name, c.slug, c.icon, c.description, c.sort_order
         ORDER BY c.sort_order",
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(
        |(id, name, slug, icon, _description, _sort_order, game_count)| CategoryWithGameCount {
            id,
            name,
            slug,
            icon,
            game_count,
        },
    )
    .collect();

    let total = categories.len() as u64;
    Ok(response::paginated(categories, 1, 100, total))
}

pub async fn list_games_in_category(
    State(pool): State<PgPool>,
    Path(slug): Path<String>,
) -> Result<Json<response::PaginatedResponse<Game>>> {
    let category = sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, icon, description, sort_order FROM categories WHERE slug = $1",
    )
    .bind(&slug)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Category not found".to_string()))?;

    let games = sqlx::query_as::<_, Game>(
        "SELECT id, developer_id, github_repo, title, description, fee_per_session, status, active, created_at
         FROM games WHERE category_id = $1 AND active = true",
    )
    .bind(category.id)
    .fetch_all(&pool)
    .await?;

    let total = games.len() as u64;
    Ok(response::paginated(games, 1, 100, total))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_categories))
        .route("/:slug/games", get(list_games_in_category))
        .with_state(pool)
}
