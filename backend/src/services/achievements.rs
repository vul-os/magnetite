// Achievement service — unlock tracking and leaderboard integration; platform surface, not yet wired.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::notifications::{broadcast_notification, Notification, NotificationType};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Achievement {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub threshold: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserAchievement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub achievement_id: Uuid,
    pub progress: i32,
    pub unlocked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AchievementWithProgress {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub threshold: i32,
    pub progress: i32,
    pub unlocked_at: Option<DateTime<Utc>>,
    pub locked: bool,
}

#[derive(Debug, Serialize)]
pub struct AchievementUnlocked {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub unlocked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AchievementLeaderboardEntry {
    pub rank: i32,
    pub user_id: Uuid,
    pub username: String,
    pub achievement_count: i64,
}

#[derive(Debug)]
pub enum AchievementEvent {
    GamePlayed { game_id: Uuid },
    ScoreSubmitted { game_id: Uuid, score: i64 },
    FriendAdded,
    SessionMinutes { minutes: i64 },
}

pub struct AchievementService;

impl AchievementService {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_user_achievements(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<AchievementWithProgress>> {
        let achievements = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                i32,
                i32,
                Option<DateTime<Utc>>,
            ),
        >(
            r#"
            SELECT
                a.id, a.name, a.description, a.icon, a.category, a.threshold,
                COALESCE(ua.progress, 0), ua.unlocked_at
            FROM achievements a
            LEFT JOIN user_achievements ua ON a.id = ua.achievement_id AND ua.user_id = $1
            ORDER BY a.category, a.name
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let result = achievements
            .into_iter()
            .map(|row| {
                let (id, name, description, icon, category, threshold, progress, unlocked_at) = row;
                let locked = unlocked_at.is_none();
                AchievementWithProgress {
                    id,
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

        Ok(result)
    }

    pub async fn update_progress(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        achievement_id: Uuid,
        progress: i32,
    ) -> Result<Option<AchievementUnlocked>> {
        let achievement = sqlx::query_as::<_, Achievement>(
            "SELECT id, name, description, icon, category, threshold, created_at FROM achievements WHERE id = $1",
        )
        .bind(achievement_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Achievement not found".to_string()))?;

        let existing = sqlx::query_as::<_, (Uuid, i32, Option<DateTime<Utc>>)>(
            "SELECT id, progress, unlocked_at FROM user_achievements WHERE user_id = $1 AND achievement_id = $2",
        )
        .bind(user_id)
        .bind(achievement_id)
        .fetch_optional(pool)
        .await?;

        let (user_achievement_id, current_progress, unlocked_at) = match existing {
            Some((id, progress, unlocked_at)) => (id, progress, unlocked_at),
            None => (Uuid::new_v4(), 0, None),
        };

        let new_progress = progress.max(current_progress);
        let should_unlock = new_progress >= achievement.threshold && unlocked_at.is_none();
        let unlocked_at_value = if should_unlock {
            Some(Utc::now())
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
        .execute(pool)
        .await?;

        if should_unlock {
            Ok(Some(AchievementUnlocked {
                id: achievement.id,
                name: achievement.name,
                description: achievement.description,
                icon: achievement.icon,
                category: achievement.category,
                unlocked_at: unlocked_at_value.unwrap(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn check_achievements(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        event: &AchievementEvent,
    ) -> Result<Vec<Achievement>> {
        let mut unlocked_achievements = Vec::new();

        match event {
            AchievementEvent::GamePlayed { game_id: _ } => {
                if let Some(achievement) = self.get_achievement_by_slug(pool, "first-game").await? {
                    if let Some(unlocked) = self
                        .update_progress(pool, user_id, achievement.id, 1)
                        .await?
                    {
                        unlocked_achievements.push(Achievement {
                            id: unlocked.id,
                            name: unlocked.name,
                            description: unlocked.description,
                            icon: unlocked.icon,
                            category: unlocked.category,
                            threshold: 1,
                            created_at: unlocked.unlocked_at,
                        });
                    }
                }
                if let Some(achievement) = self.get_achievement_by_slug(pool, "century").await? {
                    let current = self
                        .get_user_progress(pool, user_id, achievement.id)
                        .await?;
                    if let Some(unlocked) = self
                        .update_progress(pool, user_id, achievement.id, current + 1)
                        .await?
                    {
                        unlocked_achievements.push(Achievement {
                            id: unlocked.id,
                            name: unlocked.name,
                            description: unlocked.description,
                            icon: unlocked.icon,
                            category: unlocked.category,
                            threshold: achievement.threshold,
                            created_at: unlocked.unlocked_at,
                        });
                    }
                }
            }
            AchievementEvent::ScoreSubmitted {
                game_id: _,
                score: _,
            } => {
                if let Some(achievement) = self.get_achievement_by_slug(pool, "high-roller").await?
                {
                    if let Some(unlocked) = self
                        .update_progress(pool, user_id, achievement.id, 1)
                        .await?
                    {
                        unlocked_achievements.push(Achievement {
                            id: unlocked.id,
                            name: unlocked.name,
                            description: unlocked.description,
                            icon: unlocked.icon,
                            category: unlocked.category,
                            threshold: achievement.threshold,
                            created_at: unlocked.unlocked_at,
                        });
                    }
                }
            }
            AchievementEvent::FriendAdded => {
                if let Some(achievement) = self
                    .get_achievement_by_slug(pool, "social-butterfly")
                    .await?
                {
                    let current = self
                        .get_user_progress(pool, user_id, achievement.id)
                        .await?;
                    if let Some(unlocked) = self
                        .update_progress(pool, user_id, achievement.id, current + 1)
                        .await?
                    {
                        unlocked_achievements.push(Achievement {
                            id: unlocked.id,
                            name: unlocked.name,
                            description: unlocked.description,
                            icon: unlocked.icon,
                            category: unlocked.category,
                            threshold: achievement.threshold,
                            created_at: unlocked.unlocked_at,
                        });
                    }
                }
            }
            AchievementEvent::SessionMinutes { minutes: _ } => {}
        }

        for achievement in &unlocked_achievements {
            let notification = sqlx::query_as::<_, Notification>(
                "INSERT INTO notifications (id, user_id, type, title, body, data, read, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, false, NOW())
                 RETURNING id, user_id, type, title, body, data, read, created_at",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(NotificationType::AchievementUnlocked.as_str())
            .bind(format!("Achievement Unlocked: {}", achievement.name))
            .bind("Congratulations on unlocking this achievement!")
            .bind(
                achievement
                    .icon
                    .as_ref()
                    .map(|icon| serde_json::json!({ "achievement_icon": icon })),
            )
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            broadcast_notification(notification).await;
        }

        Ok(unlocked_achievements)
    }

    pub async fn get_leaderboard(
        &self,
        pool: &PgPool,
        limit: usize,
    ) -> Result<Vec<AchievementLeaderboardEntry>> {
        let entries = sqlx::query_as::<_, (Uuid, String, i64)>(
            r#"
            SELECT u.id, u.username, COUNT(ua.id) as achievement_count
            FROM users u
            LEFT JOIN user_achievements ua ON u.id = ua.user_id AND ua.unlocked_at IS NOT NULL
            GROUP BY u.id, u.username
            ORDER BY achievement_count DESC, u.username ASC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        let leaderboard = entries
            .into_iter()
            .enumerate()
            .map(
                |(i, (user_id, username, achievement_count))| AchievementLeaderboardEntry {
                    rank: (i + 1) as i32,
                    user_id,
                    username,
                    achievement_count,
                },
            )
            .collect();

        Ok(leaderboard)
    }

    async fn get_achievement_by_slug(
        &self,
        pool: &PgPool,
        slug: &str,
    ) -> Result<Option<Achievement>> {
        let achievement = sqlx::query_as::<_, Achievement>(
            "SELECT id, name, description, icon, category, threshold, created_at FROM achievements WHERE LOWER(REPLACE(name, ' ', '-')) = $1",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;

        Ok(achievement)
    }

    async fn get_user_progress(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        achievement_id: Uuid,
    ) -> Result<i32> {
        let result = sqlx::query_scalar::<_, i32>(
            "SELECT progress FROM user_achievements WHERE user_id = $1 AND achievement_id = $2",
        )
        .bind(user_id)
        .bind(achievement_id)
        .fetch_optional(pool)
        .await?;

        Ok(result.unwrap_or(0))
    }
}

impl Default for AchievementService {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn seed_default_achievements(pool: &PgPool) -> Result<()> {
    let achievements = vec![
        (
            "First Game",
            "Play your first game",
            Some("trophy"),
            Some("games"),
            1,
        ),
        (
            "Century",
            "Play 100 games",
            Some("medal"),
            Some("games"),
            100,
        ),
        (
            "High Roller",
            "Get top 10 on any leaderboard",
            Some("crown"),
            Some("leaderboard"),
            1,
        ),
        (
            "Social Butterfly",
            "Add 10 friends",
            Some("users"),
            Some("social"),
            10,
        ),
    ];

    for (name, description, icon, category, threshold) in achievements {
        let existing = sqlx::query_scalar::<_, Uuid>("SELECT id FROM achievements WHERE name = $1")
            .bind(name)
            .fetch_optional(pool)
            .await?;

        if existing.is_none() {
            sqlx::query(
                r#"
                INSERT INTO achievements (id, name, description, icon, category, threshold, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(name)
            .bind(description)
            .bind(icon)
            .bind(category)
            .bind(threshold)
            .bind(Utc::now())
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}
