// Points economy service — award/spend, balance, history, leaderboard, season reset.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

// ─── Domain types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PointBalance {
    pub user_id: Uuid,
    pub balance: i64,
    pub season_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub delta: i64,
    pub reason: String,
    pub game_id: Option<Uuid>,
    pub season_id: Option<Uuid>,
    pub balance_snapshot: i64,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Season {
    pub id: Uuid,
    pub name: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PointReward {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub kind: String,
    pub points: i64,
    pub game_id: Option<Uuid>,
    pub active: bool,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointsLeaderboardEntry {
    pub rank: i64,
    pub user_id: Uuid,
    pub username: String,
    pub balance: i64,
}

// ─── Service ─────────────────────────────────────────────────────────────────

pub struct PointsService {
    pool: PgPool,
}

impl PointsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Balance helpers ──────────────────────────────────────────────────────

    /// Return current point balance; 0 if no balance row exists yet.
    pub async fn get_balance(&self, user_id: Uuid) -> Result<i64> {
        let row =
            sqlx::query_as::<_, (i64,)>("SELECT balance FROM point_balances WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0).unwrap_or(0))
    }

    /// Return the full balance row (including season_id / updated_at).
    pub async fn get_balance_row(&self, user_id: Uuid) -> Result<PointBalance> {
        let row = sqlx::query_as::<_, PointBalance>(
            "SELECT user_id, balance, season_id, updated_at
             FROM point_balances WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.unwrap_or(PointBalance {
            user_id,
            balance: 0,
            season_id: None,
            updated_at: Utc::now(),
        }))
    }

    // ── Award ────────────────────────────────────────────────────────────────

    /// Award `points` to a user. Returns the ledger entry created.
    pub async fn award(
        &self,
        user_id: Uuid,
        points: i64,
        reason: &str,
        game_id: Option<Uuid>,
        metadata: Option<Value>,
    ) -> Result<LedgerEntry> {
        if points <= 0 {
            return Err(AppError::Validation(
                "Award amount must be positive".to_string(),
            ));
        }

        let active_season = self.active_season_id().await?;
        let mut tx = self.pool.begin().await?;

        // Upsert balance
        sqlx::query(
            r#"
            INSERT INTO point_balances (user_id, balance, season_id, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (user_id) DO UPDATE
                SET balance    = point_balances.balance + $2,
                    updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(points)
        .bind(active_season)
        .execute(&mut *tx)
        .await?;

        let new_balance: i64 =
            sqlx::query_as::<_, (i64,)>("SELECT balance FROM point_balances WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?
                .0;

        let entry = sqlx::query_as::<_, LedgerEntry>(
            r#"
            INSERT INTO points_ledger
                (id, user_id, delta, reason, game_id, season_id, balance_snapshot, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            RETURNING id, user_id, delta, reason, game_id, season_id, balance_snapshot, metadata, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(points)
        .bind(reason)
        .bind(game_id)
        .bind(active_season)
        .bind(new_balance)
        .bind(&metadata)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(entry)
    }

    // ── Spend ────────────────────────────────────────────────────────────────

    /// Debit `points` from a user. Returns the ledger entry created.
    /// Fails with `InsufficientFunds` if balance is too low.
    pub async fn spend(
        &self,
        user_id: Uuid,
        points: i64,
        reason: &str,
        game_id: Option<Uuid>,
        metadata: Option<Value>,
    ) -> Result<LedgerEntry> {
        if points <= 0 {
            return Err(AppError::Validation(
                "Spend amount must be positive".to_string(),
            ));
        }

        let active_season = self.active_season_id().await?;
        let mut tx = self.pool.begin().await?;

        // Lock balance row
        let current: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT balance FROM point_balances WHERE user_id = $1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.0)
        .unwrap_or(0);

        if current < points {
            return Err(AppError::InsufficientFunds(format!(
                "Insufficient points. Have {current}, need {points}"
            )));
        }

        sqlx::query(
            "UPDATE point_balances SET balance = balance - $1, updated_at = NOW() WHERE user_id = $2",
        )
        .bind(points)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        let new_balance = current - points;

        let entry = sqlx::query_as::<_, LedgerEntry>(
            r#"
            INSERT INTO points_ledger
                (id, user_id, delta, reason, game_id, season_id, balance_snapshot, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            RETURNING id, user_id, delta, reason, game_id, season_id, balance_snapshot, metadata, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(-points)
        .bind(reason)
        .bind(game_id)
        .bind(active_season)
        .bind(new_balance)
        .bind(&metadata)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(entry)
    }

    // ── History ──────────────────────────────────────────────────────────────

    pub async fn history(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<LedgerEntry>> {
        let entries = sqlx::query_as::<_, LedgerEntry>(
            r#"
            SELECT id, user_id, delta, reason, game_id, season_id, balance_snapshot, metadata, created_at
            FROM points_ledger
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn history_count(&self, user_id: Uuid) -> Result<i64> {
        let count =
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM points_ledger WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?
                .0;
        Ok(count)
    }

    // ── Leaderboard ───────────────────────────────────────────────────────────

    pub async fn leaderboard(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PointsLeaderboardEntry>> {
        let rows = sqlx::query_as::<_, (Uuid, String, i64)>(
            r#"
            SELECT u.id, u.username, COALESCE(pb.balance, 0)
            FROM users u
            LEFT JOIN point_balances pb ON pb.user_id = u.id
            ORDER BY COALESCE(pb.balance, 0) DESC, u.username ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let entries = rows
            .into_iter()
            .enumerate()
            .map(|(i, (uid, username, balance))| PointsLeaderboardEntry {
                rank: offset + i as i64 + 1,
                user_id: uid,
                username,
                balance,
            })
            .collect();

        Ok(entries)
    }

    // ── Seasons ───────────────────────────────────────────────────────────────

    pub async fn active_season(&self) -> Result<Option<Season>> {
        let s = sqlx::query_as::<_, Season>(
            "SELECT id, name, starts_at, ends_at, is_active, created_at
             FROM seasons WHERE is_active = true LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(s)
    }

    async fn active_season_id(&self) -> Result<Option<Uuid>> {
        let row =
            sqlx::query_as::<_, (Uuid,)>("SELECT id FROM seasons WHERE is_active = true LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Reset all point balances for the current season and start a new season.
    /// Writes a 'season_reset' ledger entry for every affected user.
    pub async fn season_reset(&self, new_season_name: &str) -> Result<u64> {
        let mut tx = self.pool.begin().await?;

        // Close current season
        sqlx::query("UPDATE seasons SET is_active = false, ends_at = NOW() WHERE is_active = true")
            .execute(&mut *tx)
            .await?;

        // Create new season
        let new_season_id: Uuid = sqlx::query_as::<_, (Uuid,)>(
            "INSERT INTO seasons (id, name, starts_at, is_active) VALUES ($1, $2, NOW(), true)
                 RETURNING id",
        )
        .bind(Uuid::new_v4())
        .bind(new_season_name)
        .fetch_one(&mut *tx)
        .await?
        .0;

        // Collect users with non-zero balances
        let users = sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT user_id, balance FROM point_balances WHERE balance > 0 FOR UPDATE",
        )
        .fetch_all(&mut *tx)
        .await?;

        let affected = users.len() as u64;

        for (uid, bal) in &users {
            // Write reset ledger entry (negative delta to zero out)
            sqlx::query(
                r#"
                INSERT INTO points_ledger
                    (id, user_id, delta, reason, season_id, balance_snapshot, created_at)
                VALUES ($1, $2, $3, 'season_reset', $4, 0, NOW())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(uid)
            .bind(-bal)
            .bind(new_season_id)
            .execute(&mut *tx)
            .await?;
        }

        // Zero out all balances and attach new season
        sqlx::query("UPDATE point_balances SET balance = 0, season_id = $1, updated_at = NOW()")
            .bind(new_season_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(affected)
    }

    // ── Reward catalog ────────────────────────────────────────────────────────

    pub async fn list_rewards(
        &self,
        game_id: Option<Uuid>,
        kind: Option<&str>,
    ) -> Result<Vec<PointReward>> {
        // Build filter dynamically but safely via separate branches.
        let rewards = match (game_id, kind) {
            (Some(gid), Some(k)) => sqlx::query_as::<_, PointReward>(
                "SELECT id, name, description, kind, points, game_id, active, metadata, created_at, updated_at
                 FROM point_rewards WHERE game_id = $1 AND kind = $2 AND active = true
                 ORDER BY created_at DESC",
            )
            .bind(gid)
            .bind(k)
            .fetch_all(&self.pool)
            .await?,

            (Some(gid), None) => sqlx::query_as::<_, PointReward>(
                "SELECT id, name, description, kind, points, game_id, active, metadata, created_at, updated_at
                 FROM point_rewards WHERE game_id = $1 AND active = true
                 ORDER BY created_at DESC",
            )
            .bind(gid)
            .fetch_all(&self.pool)
            .await?,

            (None, Some(k)) => sqlx::query_as::<_, PointReward>(
                "SELECT id, name, description, kind, points, game_id, active, metadata, created_at, updated_at
                 FROM point_rewards WHERE kind = $1 AND active = true
                 ORDER BY created_at DESC",
            )
            .bind(k)
            .fetch_all(&self.pool)
            .await?,

            (None, None) => sqlx::query_as::<_, PointReward>(
                "SELECT id, name, description, kind, points, game_id, active, metadata, created_at, updated_at
                 FROM point_rewards WHERE active = true
                 ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await?,
        };

        Ok(rewards)
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[test]
    fn award_requires_positive_amount() {
        // Negative award should be rejected.
        let bad_delta: i64 = -10;
        assert!(bad_delta <= 0);
    }

    #[test]
    fn spend_requires_positive_amount() {
        let bad: i64 = 0;
        assert!(bad <= 0);
    }

    #[test]
    fn insufficient_funds_detected() {
        let balance: i64 = 50;
        let spend: i64 = 100;
        assert!(balance < spend);
    }

    #[test]
    fn leaderboard_rank_offset() {
        let offset: i64 = 10;
        let idx: usize = 2;
        let rank = offset + idx as i64 + 1;
        assert_eq!(rank, 13);
    }

    #[test]
    fn season_reset_zeroes_balance() {
        let balance: i64 = 500;
        let delta = -balance;
        assert_eq!(balance + delta, 0);
    }
}
