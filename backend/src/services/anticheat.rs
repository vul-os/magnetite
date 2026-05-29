use axum::http::HeaderMap;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    pub timestamp: f64,
    pub input_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub inputs: Vec<Input>,
    pub positions: Vec<Position>,
    pub scores: Vec<i64>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub severity: Severity,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    VelocityViolation,
    InputRateAnomaly,
    ScoreProgressionAnomaly,
    PatternAnomaly,
    TimingAnomaly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

pub fn check_velocity(player_positions: &[Position], time_delta: f64) -> bool {
    if player_positions.len() < 2 || time_delta <= 0.0 {
        return false;
    }

    const MAX_WALK_SPEED: f64 = 8.0;
    const MAX_SPRINT_SPEED: f64 = 14.0;
    const MAX_VEHICLE_SPEED: f64 = 50.0;
    const MAX_AIR_SPEED: f64 = 2.0;

    let max_allowable_speed = MAX_VEHICLE_SPEED;

    for i in 1..player_positions.len() {
        let prev = &player_positions[i - 1];
        let curr = &player_positions[i];

        let dx = curr.x - prev.x;
        let dy = curr.y - prev.y;
        let dz = curr.z - prev.z;

        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        let time_slice = time_delta / (player_positions.len() as f64 - 1.0).max(1.0);
        let speed = distance / time_slice;

        if speed > max_allowable_speed {
            return true;
        }
    }

    false
}

pub fn detect_anomalies(session_data: &SessionData) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();
    let now = Utc::now();

    if let Some(ref end_time) = session_data.end_time {
        let duration = (*end_time - session_data.start_time).num_seconds() as f64;
        if duration > 0.0 {
            let input_rate = session_data.inputs.len() as f64 / duration;
            if input_rate > 50.0 {
                anomalies.push(Anomaly {
                    anomaly_type: AnomalyType::InputRateAnomaly,
                    severity: Severity::High,
                    description: format!(
                        "Extremely high input rate: {:.2} inputs/second",
                        input_rate
                    ),
                    detected_at: now,
                    session_id: session_data.session_id,
                });
            } else if input_rate > 30.0 {
                anomalies.push(Anomaly {
                    anomaly_type: AnomalyType::InputRateAnomaly,
                    severity: Severity::Medium,
                    description: format!(
                        "Abnormally high input rate: {:.2} inputs/second",
                        input_rate
                    ),
                    detected_at: now,
                    session_id: session_data.session_id,
                });
            }
        }
    }

    if session_data.scores.len() >= 3 {
        for i in 2..session_data.scores.len() {
            let prev_gap = session_data.scores[i - 1] - session_data.scores[i - 2];
            let curr_gap = session_data.scores[i] - session_data.scores[i - 1];

            if prev_gap > 0 && curr_gap > prev_gap * 10 {
                anomalies.push(Anomaly {
                    anomaly_type: AnomalyType::ScoreProgressionAnomaly,
                    severity: Severity::High,
                    description: format!(
                        "Suspicious score jump: {} -> {} (gap: {})",
                        session_data.scores[i - 1],
                        session_data.scores[i],
                        curr_gap
                    ),
                    detected_at: now,
                    session_id: session_data.session_id,
                });
            }

            if curr_gap < 0 && session_data.scores[i] > session_data.scores[i - 1] * 2 {
                anomalies.push(Anomaly {
                    anomaly_type: AnomalyType::ScoreProgressionAnomaly,
                    severity: Severity::Medium,
                    description: format!(
                        "Score anomaly: {} -> {} (impossible progression)",
                        session_data.scores[i - 1],
                        session_data.scores[i]
                    ),
                    detected_at: now,
                    session_id: session_data.session_id,
                });
            }
        }
    }

    let timing_variance = detect_timing_anomalies(&session_data.inputs);
    if timing_variance > 0.0 {
        anomalies.push(Anomaly {
            anomaly_type: AnomalyType::TimingAnomaly,
            severity: if timing_variance > 0.8 {
                Severity::High
            } else {
                Severity::Low
            },
            description: format!("Unusual timing patterns detected (variance: {:.2})", timing_variance),
            detected_at: now,
            session_id: session_data.session_id,
        });
    }

    anomalies
}

fn detect_timing_anomalies(inputs: &[Input]) -> f64 {
    if inputs.len() < 3 {
        return 0.0;
    }

    let mut intervals: Vec<f64> = Vec::new();
    for i in 1..inputs.len() {
        let interval = inputs[i].timestamp - inputs[i - 1].timestamp;
        if interval > 0.0 && interval < 1000.0 {
            intervals.push(interval);
        }
    }

    if intervals.len() < 2 {
        return 0.0;
    }

    let mean: f64 = intervals.iter().sum::<f64>() / intervals.len() as f64;
    let variance: f64 = intervals
        .iter()
        .map(|&x| {
            let diff = x - mean;
            diff * diff
        })
        .sum::<f64>()
        / intervals.len() as f64;

    let std_dev = variance.sqrt();
    let coefficient_of_variation = if mean > 0.0 { std_dev / mean } else { 0.0 };

    coefficient_of_variation.min(1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    pub user_agent: String,
    pub screen_resolution: String,
    pub timezone: String,
    pub language: String,
    pub ip_address: String,
    pub hash: String,
}

pub fn generate_fingerprint(headers: &HeaderMap) -> DeviceFingerprint {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let screen_resolution = headers
        .get("x-screen-resolution")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let timezone = headers
        .get("x-timezone")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let language = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let ip_address = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .trim()
        .to_string();

    let fingerprint_string = format!(
        "{}|{}|{}|{}|{}",
        user_agent, screen_resolution, timezone, language, ip_address
    );

    let mut hasher = Sha256::new();
    hasher.update(fingerprint_string.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    DeviceFingerprint {
        user_agent,
        screen_resolution,
        timezone,
        language,
        ip_address,
        hash,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BanRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub reason: Option<String>,
    pub fingerprint: Option<String>,
    pub banned_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub async fn check_ban(db: &sqlx::PgPool, user_id: Uuid, fingerprint: &DeviceFingerprint) -> Result<bool> {
    let ban = sqlx::query_as::<_, BanRecord>(
        r#"
        SELECT * FROM anti_cheat_bans
        WHERE user_id = $1
        AND (expires_at IS NULL OR expires_at > NOW())
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    if ban.is_some() {
        return Ok(true);
    }

    let fingerprint_ban = sqlx::query_as::<_, BanRecord>(
        r#"
        SELECT * FROM anti_cheat_bans
        WHERE fingerprint = $1
        AND (expires_at IS NULL OR expires_at > NOW())
        LIMIT 1
        "#,
    )
    .bind(&fingerprint.hash)
    .fetch_optional(db)
    .await?;

    Ok(fingerprint_ban.is_some())
}

pub async fn ban_user(
    db: &sqlx::PgPool,
    user_id: Uuid,
    reason: &str,
    duration_days: Option<i32>,
    fingerprint: Option<&DeviceFingerprint>,
) -> Result<BanRecord> {
    let ban_id = Uuid::new_v4();
    let expires_at = duration_days.map(|days| Utc::now() + Duration::days(days as i64));
    let fingerprint_hash = fingerprint.as_ref().map(|f| f.hash.clone());

    sqlx::query_as::<_, BanRecord>(
        r#"
        INSERT INTO anti_cheat_bans (id, user_id, reason, fingerprint, banned_at, expires_at)
        VALUES ($1, $2, $3, $4, NOW(), $5)
        RETURNING *
        "#,
    )
    .bind(ban_id)
    .bind(user_id)
    .bind(reason)
    .bind(fingerprint_hash)
    .bind(expires_at)
    .fetch_one(db)
    .await
    .map_err(AppError::from)
}

pub async fn global_ban_list(db: &sqlx::PgPool, limit: i32, offset: i32) -> Result<Vec<BanRecord>> {
    let bans = sqlx::query_as::<_, BanRecord>(
        r#"
        SELECT * FROM anti_cheat_bans
        ORDER BY banned_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await?;

    Ok(bans)
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionReplay {
    pub id: Uuid,
    pub session_id: Uuid,
    pub inputs: serde_json::Value,
    pub recorded_at: DateTime<Utc>,
}

pub async fn store_replay(
    db: &sqlx::PgPool,
    session_id: Uuid,
    inputs: Vec<Input>,
) -> Result<SessionReplay> {
    let replay_id = Uuid::new_v4();
    let inputs_json = serde_json::to_value(&inputs)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let replay = sqlx::query_as::<_, SessionReplay>(
        r#"
        INSERT INTO session_replays (id, session_id, inputs, recorded_at)
        VALUES ($1, $2, $3, NOW())
        RETURNING *
        "#,
    )
    .bind(replay_id)
    .bind(session_id)
    .bind(inputs_json)
    .fetch_one(db)
    .await
    .map_err(AppError::from)?;

    Ok(replay)
}

pub async fn get_replay(db: &sqlx::PgPool, session_id: Uuid) -> Result<Option<Vec<Input>>> {
    let replay = sqlx::query_as::<_, SessionReplay>(
        "SELECT * FROM session_replays WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
    .map_err(AppError::from)?;

    match replay {
        Some(r) => {
            let inputs: Vec<Input> = serde_json::from_value(r.inputs)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(Some(inputs))
        }
        None => Ok(None),
    }
}

pub async fn unban_user(db: &sqlx::PgPool, user_id: Uuid) -> Result<()> {
    let result = sqlx::query(
        "DELETE FROM anti_cheat_bans WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(db)
    .await
    .map_err(AppError::from)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("No ban found for user".to_string()));
    }

    Ok(())
}
