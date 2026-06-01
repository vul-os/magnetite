// Leaderboard service — Redis-backed score tracking; called from api/leaderboard.rs submit_score.
// get_top / get_rank / get_around / archive_and_reset are platform surface for future read-path wiring.
#![allow(dead_code)]
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct LeaderboardService {
    client: redis::Client,
}

impl LeaderboardService {
    pub fn new(redis_url: &str) -> Self {
        Self {
            client: redis::Client::open(redis_url).expect("Valid redis URL"),
        }
    }

    pub fn with_client(client: redis::Client) -> Self {
        Self { client }
    }

    fn leaderboard_key(game_id: Uuid) -> String {
        format!("leaderboard:{}", game_id)
    }

    fn archive_key(game_id: Uuid, period: &str) -> String {
        format!("leaderboard:{}:{}", game_id, period)
    }

    pub async fn submit_score(
        &self,
        game_id: Uuid,
        user_id: Uuid,
        score: i64,
    ) -> Result<RankResult> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let key = Self::leaderboard_key(game_id);
        let user_id_str = user_id.to_string();

        let existing: Option<f64> = conn
            .zscore(&key, &user_id_str)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let is_new = existing.is_none();
        let prev_score = existing.unwrap_or(0.0);

        if !is_new && score <= prev_score as i64 {
            let rank = self
                .get_rank_internal(&mut conn, &key, &user_id_str)
                .await?;
            return Ok(RankResult {
                rank,
                score: prev_score as i64,
                is_personal_best: false,
            });
        }

        let _: () = conn
            .zadd(&key, &user_id_str, score)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rank = self
            .get_rank_internal(&mut conn, &key, &user_id_str)
            .await?;

        Ok(RankResult {
            rank,
            score,
            is_personal_best: true,
        })
    }

    async fn get_rank_internal(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
        key: &str,
        user_id: &str,
    ) -> Result<i64> {
        let rank: Option<i64> = conn
            .zrevrank(key, user_id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(rank.map(|r| r + 1).unwrap_or(0))
    }

    pub async fn get_top(&self, game_id: Uuid, limit: usize) -> Result<Vec<ScoreEntry>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let key = Self::leaderboard_key(game_id);
        let limit = limit.min(1000) as isize;

        let entries: Vec<(String, i64)> = conn
            .zrevrange_withscores(&key, 0, limit - 1)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let result: Vec<ScoreEntry> = entries
            .into_iter()
            .enumerate()
            .map(|(i, (user_id, score))| ScoreEntry {
                rank: (i + 1) as i64,
                user_id: Uuid::parse_str(&user_id).unwrap_or_default(),
                score,
            })
            .collect();

        Ok(result)
    }

    pub async fn get_rank(&self, game_id: Uuid, user_id: Uuid) -> Result<Option<RankResult>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let key = Self::leaderboard_key(game_id);
        let user_id_str = user_id.to_string();

        let score: Option<f64> = conn
            .zscore(&key, &user_id_str)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        match score {
            Some(s) => {
                let rank = self
                    .get_rank_internal(&mut conn, &key, &user_id_str)
                    .await?;
                Ok(Some(RankResult {
                    rank,
                    score: s as i64,
                    is_personal_best: true,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_around(
        &self,
        game_id: Uuid,
        user_id: Uuid,
        range: usize,
    ) -> Result<Vec<ScoreEntry>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let key = Self::leaderboard_key(game_id);
        let user_id_str = user_id.to_string();
        let range = range.min(50) as isize;

        let rank: Option<i64> = conn
            .zrevrank(&key, &user_id_str)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rank = match rank {
            Some(r) => r as isize,
            None => return Ok(vec![]),
        };

        let start = (rank - range / 2).max(0);
        let stop = start + range * 2 - 1;

        let entries: Vec<(String, i64)> = conn
            .zrevrange_withscores(&key, start, stop)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let result: Vec<ScoreEntry> = entries
            .into_iter()
            .enumerate()
            .map(|(i, (user_id, score))| ScoreEntry {
                rank: (start + i as isize + 1) as i64,
                user_id: Uuid::parse_str(&user_id).unwrap_or_default(),
                score,
            })
            .collect();

        Ok(result)
    }

    /// Archive the current live leaderboard for `game_id` under a season-labelled key,
    /// then wipe the live sorted set so the new season starts fresh.
    ///
    /// `season_label` should be the human-readable season name (e.g. "Season 2026-Q1").
    /// It is embedded in the Redis archive key so per-season boards are permanently addressable.
    pub async fn archive_and_reset(&self, game_id: Uuid, season_label: &str) -> Result<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let key = Self::leaderboard_key(game_id);

        // Sanitise the label for use in a Redis key (replace spaces/colons/slashes).
        let safe_label = season_label
            .replace(' ', "_")
            .replace(':', "-")
            .replace('/', "-");
        let archive_key = Self::archive_key(game_id, &safe_label);

        let entries: Vec<(String, i64)> = conn
            .zrevrange_withscores(&key, 0, -1)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if !entries.is_empty() {
            for (user_id, score) in &entries {
                let _: () = conn
                    .zadd(&archive_key, user_id.as_str(), *score)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }

            // Expire the archive after 1 year (optional safeguard).
            let _: () = conn
                .expire(&archive_key, 365 * 24 * 3600)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            let _: () = conn
                .del(&key)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub rank: i64,
    pub user_id: Uuid,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankResult {
    pub rank: i64,
    pub score: i64,
    pub is_personal_best: bool,
}
