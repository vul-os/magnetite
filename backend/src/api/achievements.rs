use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::achievements::{AchievementEvent, AchievementService};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Achievement {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub threshold: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserAchievement {
    pub id: Uuid,
    pub achievement_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub threshold: i32,
    pub progress: i32,
    pub unlocked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub locked: bool,
}

#[derive(Debug, Serialize)]
pub struct AchievementDetail {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub threshold: i32,
    pub progress: i32,
    pub unlocked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub locked: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProgressRequest {
    pub progress: i32,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub user_id: Uuid,
    pub username: String,
    pub achievement_count: i64,
}

pub async fn list_achievements(
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<response::PaginatedResponse<UserAchievement>>> {
    let achievements = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            i32,
            i32,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        r#"
        SELECT
            ua.id, ua.achievement_id, a.name, a.description, a.icon, a.category,
            a.threshold, COALESCE(ua.progress, 0), ua.unlocked_at
        FROM achievements a
        LEFT JOIN user_achievements ua ON a.id = ua.achievement_id AND ua.user_id = $1
        ORDER BY a.category, a.name
        "#,
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    let result: Vec<UserAchievement> = achievements
        .into_iter()
        .map(|row| {
            let (
                id,
                achievement_id,
                name,
                description,
                icon,
                category,
                threshold,
                progress,
                unlocked_at,
            ) = row;
            let locked = unlocked_at.is_none();
            UserAchievement {
                id,
                achievement_id,
                name,
                description,
                icon,
                category,
                threshold,
                progress,
                unlocked_at,
                locked,
            }
        })
        .collect();

    let total = result.len() as u64;
    Ok(response::paginated(result, 1, 100, total))
}

pub async fn get_achievement(
    State(pool): State<PgPool>,
    Path((user_id, achievement_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<response::ApiResponse<AchievementDetail>>> {
    let result = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            i32,
            i32,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        r#"
        SELECT
            a.id, a.name, a.description, a.icon, a.category, a.threshold,
            COALESCE(ua.progress, 0), ua.unlocked_at
        FROM achievements a
        LEFT JOIN user_achievements ua ON a.id = ua.achievement_id AND ua.user_id = $1
        WHERE a.id = $2
        "#,
    )
    .bind(user_id)
    .bind(achievement_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Achievement not found".to_string()))?;

    let (id, name, description, icon, category, threshold, progress, unlocked_at) = result;
    let locked = unlocked_at.is_none();

    Ok(response::success_response(AchievementDetail {
        id,
        name,
        description,
        icon,
        category,
        threshold,
        progress,
        unlocked_at,
        locked,
    }))
}

pub async fn update_progress(
    State(pool): State<PgPool>,
    Path((user_id, achievement_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateProgressRequest>,
) -> Result<Json<response::ApiResponse<UserAchievement>>> {
    let achievement = sqlx::query_as::<_, Achievement>(
        "SELECT id, name, description, icon, category, threshold, created_at FROM achievements WHERE id = $1",
    )
    .bind(achievement_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Achievement not found".to_string()))?;

    let existing = sqlx::query_as::<_, (Uuid, i32, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, progress, unlocked_at FROM user_achievements WHERE user_id = $1 AND achievement_id = $2",
    )
    .bind(user_id)
    .bind(achievement_id)
    .fetch_optional(&pool)
    .await?;

    let (user_achievement_id, current_progress, unlocked_at) = match existing {
        Some((id, progress, unlocked_at)) => (id, progress, unlocked_at),
        None => (Uuid::new_v4(), 0, None),
    };

    let new_progress = payload.progress.max(current_progress);
    let should_unlock = new_progress >= achievement.threshold && unlocked_at.is_none();
    let unlocked_at_value = if should_unlock {
        Some(chrono::Utc::now())
    } else {
        unlocked_at
    };

    sqlx::query(
        r#"
        INSERT INTO user_achievements (id, user_id, achievement_id, progress, unlocked_at)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id, achievement_id)
        DO UPDATE SET progress = $4, unlocked_at = COALESCE(user_achievements.unlocked_at, $5)
        "#,
    )
    .bind(user_achievement_id)
    .bind(user_id)
    .bind(achievement_id)
    .bind(new_progress)
    .bind(unlocked_at_value)
    .execute(&pool)
    .await?;

    // Fire cross-achievement unlock tracking via AchievementService (broadcasts
    // notifications for any newly-unlocked achievements triggered by this progress update).
    let svc = AchievementService::new();
    let _ = svc
        .check_achievements(
            &pool,
            user_id,
            &AchievementEvent::GamePlayed {
                game_id: achievement_id,
            },
        )
        .await;

    Ok(response::success_response(UserAchievement {
        id: user_achievement_id,
        achievement_id,
        name: achievement.name,
        description: achievement.description,
        icon: achievement.icon,
        category: achievement.category,
        threshold: achievement.threshold,
        progress: new_progress,
        unlocked_at: unlocked_at_value,
        locked: unlocked_at_value.is_none(),
    }))
}

pub async fn get_leaderboard(
    State(pool): State<PgPool>,
) -> Result<Json<response::PaginatedResponse<LeaderboardEntry>>> {
    let entries = sqlx::query_as::<_, (Uuid, String, i64)>(
        r#"
        SELECT u.id, u.username, COUNT(ua.id) as achievement_count
        FROM users u
        LEFT JOIN user_achievements ua ON u.id = ua.user_id AND ua.unlocked_at IS NOT NULL
        GROUP BY u.id, u.username
        ORDER BY achievement_count DESC, u.username ASC
        LIMIT 100
        "#,
    )
    .fetch_all(&pool)
    .await?;

    let leaderboard: Vec<LeaderboardEntry> = entries
        .into_iter()
        .enumerate()
        .map(
            |(i, (user_id, username, achievement_count))| LeaderboardEntry {
                rank: (i + 1) as i32,
                user_id,
                username,
                achievement_count,
            },
        )
        .collect();

    let total = leaderboard.len() as u64;
    Ok(response::paginated(leaderboard, 1, 100, total))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/:user_id", get(list_achievements))
        .route("/:user_id/:id", get(get_achievement))
        .route("/:user_id/:id/progress", post(update_progress))
        .route("/leaderboard", get(get_leaderboard))
        .with_state(pool)
}
