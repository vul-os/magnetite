use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::Result;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_search_type")]
    pub search_type: String,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    /// Filter games by genre (exact match)
    pub genre: Option<String>,
    /// Filter games by category (exact match)
    pub category: Option<String>,
    /// Only free-to-play games (price_usdc = 0)
    pub is_free: Option<bool>,
    /// Minimum average review rating (0.0–5.0)
    pub min_rating: Option<f64>,
}

fn default_search_type() -> String {
    "all".to_string()
}

#[derive(Debug, Serialize)]
pub struct GameSearchResult {
    pub id: uuid::Uuid,
    pub title: String,
    pub description: Option<String>,
    pub developer_username: String,
    pub result_type: String,
}

#[derive(Debug, Serialize)]
pub struct UserSearchResult {
    pub id: uuid::Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub result_type: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "result_type")]
pub enum SearchResult {
    Game(GameSearchResult),
    User(UserSearchResult),
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: i64,
    pub limit: i32,
    pub offset: i32,
}

#[allow(clippy::too_many_arguments)]
async fn search_games(
    pool: &PgPool,
    query: &str,
    limit: i32,
    offset: i32,
    genre: Option<&str>,
    category: Option<&str>,
    is_free: Option<bool>,
    min_rating: Option<f64>,
) -> Result<Vec<GameSearchResult>> {
    // Build the extra filter clauses while collecting bind parameters.
    // Parameters: $1 = tsquery, $2 = limit, $3 = offset; extras start at $4.
    let mut extra_where: Vec<String> = Vec::new();
    let mut bind_idx: i32 = 4;

    if genre.is_some() {
        extra_where.push(format!("g.genre = ${bind_idx}"));
        bind_idx += 1;
    }
    if category.is_some() {
        extra_where.push(format!("g.category = ${bind_idx}"));
        bind_idx += 1;
    }
    if let Some(free) = is_free {
        if free {
            extra_where.push("g.price_usdc = 0".to_string());
        } else {
            extra_where.push("g.price_usdc > 0".to_string());
        }
    }
    if let Some(rating) = min_rating {
        if rating > 0.0 {
            extra_where.push(format!("g.average_rating >= ${bind_idx}"));
            let _ = bind_idx; // consumed
        }
    }

    let extra_sql = if extra_where.is_empty() {
        String::new()
    } else {
        format!("AND {}", extra_where.join(" AND "))
    };

    // Use plainto_tsquery so multi-word queries work without operators.
    let sql = format!(
        r#"
        SELECT g.id, g.title, g.description, u.username AS developer_username,
               ts_rank(g.search_vector, plainto_tsquery('english', $1)) AS rank
        FROM games g
        JOIN users u ON g.developer_id = u.id
        WHERE g.active = true
          AND g.search_vector @@ plainto_tsquery('english', $1)
          {extra_sql}
        ORDER BY rank DESC, g.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    );

    let mut q = sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, String, f32)>(&sql)
        .bind(query)
        .bind(limit)
        .bind(offset);

    if let Some(g) = genre {
        q = q.bind(g.to_string());
    }
    if let Some(c) = category {
        q = q.bind(c.to_string());
    }
    if let Some(rating) = min_rating {
        if rating > 0.0 {
            q = q.bind(rating);
        }
    }

    let games = q.fetch_all(pool).await?;

    let results = games
        .into_iter()
        .map(
            |(id, title, description, developer_username, _rank)| GameSearchResult {
                id,
                title,
                description,
                developer_username,
                result_type: "game".to_string(),
            },
        )
        .collect();

    Ok(results)
}

async fn search_users(
    pool: &PgPool,
    query: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<UserSearchResult>> {
    let search_pattern = format!("%{}%", query);

    let users = sqlx::query_as::<_, (uuid::Uuid, String, Option<String>)>(
        r#"
        SELECT id, username, avatar_url
        FROM users
        WHERE is_banned = false
          AND username ILIKE $1
        ORDER BY
            CASE WHEN username ILIKE $1 THEN 0 ELSE 1 END,
            created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(&search_pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let results = users
        .into_iter()
        .map(|(id, username, avatar_url)| UserSearchResult {
            id,
            username,
            avatar_url,
            result_type: "user".to_string(),
        })
        .collect();

    Ok(results)
}

async fn count_games(
    pool: &PgPool,
    query: &str,
    genre: Option<&str>,
    category: Option<&str>,
    is_free: Option<bool>,
    min_rating: Option<f64>,
) -> Result<i64> {
    let mut extra_where: Vec<String> = Vec::new();
    let mut bind_idx: i32 = 2;

    if genre.is_some() {
        extra_where.push(format!("g.genre = ${bind_idx}"));
        bind_idx += 1;
    }
    if category.is_some() {
        extra_where.push(format!("g.category = ${bind_idx}"));
        bind_idx += 1;
    }
    if let Some(free) = is_free {
        if free {
            extra_where.push("g.price_usdc = 0".to_string());
        } else {
            extra_where.push("g.price_usdc > 0".to_string());
        }
    }
    if let Some(rating) = min_rating {
        if rating > 0.0 {
            extra_where.push(format!("g.average_rating >= ${bind_idx}"));
            let _ = bind_idx; // consumed
        }
    }

    let extra_sql = if extra_where.is_empty() {
        String::new()
    } else {
        format!("AND {}", extra_where.join(" AND "))
    };

    let sql = format!(
        r#"
        SELECT COUNT(*)
        FROM games g
        JOIN users u ON g.developer_id = u.id
        WHERE g.active = true
          AND g.search_vector @@ plainto_tsquery('english', $1)
          {extra_sql}
        "#,
    );

    let mut q = sqlx::query_as::<_, (i64,)>(&sql).bind(query);
    if let Some(g) = genre {
        q = q.bind(g.to_string());
    }
    if let Some(c) = category {
        q = q.bind(c.to_string());
    }
    if let Some(rating) = min_rating {
        if rating > 0.0 {
            q = q.bind(rating);
        }
    }

    let count = q.fetch_one(pool).await?.0;
    Ok(count)
}

async fn count_users(pool: &PgPool, query: &str) -> Result<i64> {
    let search_pattern = format!("%{}%", query);

    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM users
        WHERE is_banned = false
          AND username ILIKE $1
        "#,
    )
    .bind(&search_pattern)
    .fetch_one(pool)
    .await?;

    Ok(count.0)
}

pub fn router(pool: PgPool) -> Router {
    Router::new().route("/", get(search)).with_state(pool)
}

pub async fn search(
    State(pool): State<PgPool>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>> {
    if query.q.trim().is_empty() {
        return Ok(Json(SearchResponse {
            results: vec![],
            total: 0,
            limit: query.limit.unwrap_or(20),
            offset: query.offset.unwrap_or(0),
        }));
    }

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    let genre = query.genre.as_deref();
    let category = query.category.as_deref();
    let is_free = query.is_free;
    let min_rating = query.min_rating;

    let (results, total) = match query.search_type.as_str() {
        "games" => {
            let games = search_games(
                &pool, &query.q, limit, offset, genre, category, is_free, min_rating,
            )
            .await?;
            let total = count_games(&pool, &query.q, genre, category, is_free, min_rating).await?;
            let search_results: Vec<SearchResult> =
                games.into_iter().map(SearchResult::Game).collect();
            (search_results, total)
        }
        "users" => {
            let users = search_users(&pool, &query.q, limit, offset).await?;
            let total = count_users(&pool, &query.q).await?;
            let search_results: Vec<SearchResult> =
                users.into_iter().map(SearchResult::User).collect();
            (search_results, total)
        }
        _ => {
            let games = search_games(
                &pool,
                &query.q,
                limit / 2 + 1,
                0,
                genre,
                category,
                is_free,
                min_rating,
            )
            .await?;
            let users = search_users(&pool, &query.q, limit / 2 + 1, 0).await?;

            let mut all_results: Vec<SearchResult> =
                games.into_iter().map(SearchResult::Game).collect();
            all_results.extend(users.into_iter().map(SearchResult::User));

            let total_games =
                count_games(&pool, &query.q, genre, category, is_free, min_rating).await?;
            let total_users = count_users(&pool, &query.q).await?;

            all_results.truncate(limit as usize);
            (all_results, total_games + total_users)
        }
    };

    Ok(Json(SearchResponse {
        results,
        total,
        limit,
        offset,
    }))
}
