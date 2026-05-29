use axum::{extract::Query, Json};
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

async fn search_games(
    pool: &PgPool,
    query: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<GameSearchResult>> {
    let search_pattern = format!("%{}%", query);

    let games = sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, String)>(
        r#"
        SELECT g.id, g.title, g.description, u.username as developer_username
        FROM games g
        JOIN users u ON g.developer_id = u.id
        WHERE g.active = true
          AND (g.title ILIKE $1 OR g.description ILIKE $1 OR u.username ILIKE $1)
        ORDER BY
            CASE WHEN g.title ILIKE $1 THEN 0 ELSE 1 END,
            g.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(&search_pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let results = games
        .into_iter()
        .map(|(id, title, description, developer_username)| GameSearchResult {
            id,
            title,
            description,
            developer_username,
            result_type: "game".to_string(),
        })
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

async fn count_games(pool: &PgPool, query: &str) -> Result<i64> {
    let search_pattern = format!("%{}%", query);

    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM games g
        JOIN users u ON g.developer_id = u.id
        WHERE g.active = true
          AND (g.title ILIKE $1 OR g.description ILIKE $1 OR u.username ILIKE $1)
        "#,
    )
    .bind(&search_pattern)
    .fetch_one(pool)
    .await?;

    Ok(count.0)
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

    let (results, total) = match query.search_type.as_str() {
        "games" => {
            let games = search_games(&pool, &query.q, limit, offset).await?;
            let total = count_games(&pool, &query.q).await?;
            let search_results: Vec<SearchResult> = games.into_iter().map(SearchResult::Game).collect();
            (search_results, total)
        }
        "users" => {
            let users = search_users(&pool, &query.q, limit, offset).await?;
            let total = count_users(&pool, &query.q).await?;
            let search_results: Vec<SearchResult> = users.into_iter().map(SearchResult::User).collect();
            (search_results, total)
        }
        _ => {
            let games = search_games(&pool, &query.q, limit / 2 + 1, 0).await?;
            let users = search_users(&pool, &query.q, limit / 2 + 1, 0).await?;

            let mut all_results: Vec<SearchResult> = games
                .into_iter()
                .map(SearchResult::Game)
                .collect();
            all_results.extend(users.into_iter().map(SearchResult::User));

            let total_games = count_games(&pool, &query.q).await?;
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