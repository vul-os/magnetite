use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Result, AppError};

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub connection_details: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct EndSessionRequest {
    pub final_score: i64,
}

#[derive(Debug, Deserialize)]
pub struct SubmitScoreRequest {
    pub score: i64,
}

#[derive(Debug, Deserialize)]
pub struct SessionListQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PlaySession {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub fee_amount: Decimal,
    pub final_score: Option<i64>,
    pub payout_status: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub fee_amount: Decimal,
    pub final_score: Option<i64>,
    pub payout_status: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub game_title: Option<String>,
    pub connection_details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionResponse>,
    pub total_count: i64,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub session: SessionResponse,
    pub connection_details: serde_json::Value,
    pub ws_endpoint: String,
}

const PLATFORM_FEE_PERCENTAGE: &str = "0.15";

pub async fn create_game_session(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>> {
    let game = sqlx::query_as::<_, (Uuid, String, Decimal, String, bool)>(
        "SELECT id, title, fee_per_session, status, active FROM games WHERE id = $1",
    )
    .bind(game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    if !game.4 {
        return Err(AppError::Validation("Game is not active".to_string()));
    }

    if game.3 != "active" {
        return Err(AppError::Validation("Game is not available for play".to_string()));
    }

    let fee_amount = game.2;

    if fee_amount > Decimal::ZERO {
        let balance = sqlx::query_as::<_, (Option<Decimal>)>(
            "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USD'",
        )
        .bind(user_id)
        .fetch_optional(&pool)
        .await?
        .0
        .unwrap_or(Decimal::ZERO);

        if balance < fee_amount {
            return Err(AppError::InsufficientFunds("Insufficient balance to pay session fee".to_string()));
        }

        sqlx::query(
            "UPDATE wallet_balances SET balance = balance - $1 WHERE user_id = $2 AND currency = 'USD'",
        )
        .bind(fee_amount)
        .bind(user_id)
        .execute(&pool)
        .await?;

        sqlx::query(
            "INSERT INTO wallet_transactions (id, user_id, tx_type, amount, reference_id, status, created_at)
             VALUES ($1, $2, 'session_fee', $3, $4, 'completed', NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(fee_amount)
        .bind(game_id.to_string())
        .execute(&pool)
        .await?;
    }

    let session_id = Uuid::new_v4();
    let connection_details = payload.connection_details.unwrap_or(serde_json::json!({}));

    sqlx::query(
        "INSERT INTO play_sessions (id, game_id, user_id, status, fee_amount, started_at)
         VALUES ($1, $2, $3, 'active', $4, NOW())",
    )
    .bind(session_id)
    .bind(game_id)
    .bind(user_id)
    .bind(fee_amount)
    .execute(&pool)
    .await?;

    let ws_endpoint = format!("wss://api.magnetite.io/ws/game/{}", session_id);

    Ok(Json(CreateSessionResponse {
        session: SessionResponse {
            id: session_id,
            game_id,
            user_id,
            status: "active".to_string(),
            fee_amount,
            final_score: None,
            payout_status: None,
            started_at: Utc::now(),
            ended_at: None,
            game_title: Some(game.1),
            connection_details: Some(connection_details.clone()),
        },
        connection_details,
        ws_endpoint,
    }))
}

pub async fn list_sessions(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(query): Query<SessionListQuery>,
) -> Result<Json<SessionListResponse>> {
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    let status_filter = query.status.as_deref();

    let status_condition = match status_filter {
        Some("active") => "AND ps.status = 'active'",
        Some("completed") => "AND ps.status = 'completed'",
        Some("abandoned") => "AND ps.status = 'abandoned'",
        _ => "",
    };

    let sessions = sqlx::query_as::<_, PlaySession>(&format!(
        "SELECT ps.id, ps.game_id, ps.user_id, ps.status, ps.fee_amount,
                ps.final_score, ps.payout_status, ps.started_at, ps.ended_at
         FROM play_sessions ps
         WHERE ps.user_id = $1 {}
         ORDER BY ps.started_at DESC
         LIMIT $2 OFFSET $3",
        status_condition
    ))
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_count: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM play_sessions ps WHERE ps.user_id = $1 {}",
        status_condition
    ))
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let session_responses: Vec<SessionResponse> = sessions
        .into_iter()
        .map(|s| SessionResponse {
            id: s.id,
            game_id: s.game_id,
            user_id: s.user_id,
            status: s.status,
            fee_amount: s.fee_amount,
            final_score: s.final_score,
            payout_status: s.payout_status,
            started_at: s.started_at,
            ended_at: s.ended_at,
            game_title: None,
            connection_details: None,
        })
        .collect();

    let has_more = (offset as i64) + (session_responses.len() as i64) < total_count.0;

    Ok(Json(SessionListResponse {
        sessions: session_responses,
        total_count: total_count.0,
        has_more,
    }))
}

pub async fn get_session(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionResponse>> {
    let session = sqlx::query_as::<_, PlaySession>(
        "SELECT id, game_id, user_id, status, fee_amount, final_score,
                payout_status, started_at, ended_at
         FROM play_sessions
         WHERE id = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let game_title: (String,) = sqlx::query_as(
        "SELECT title FROM games WHERE id = $1",
    )
    .bind(session.game_id)
    .fetch_optional(&pool)
    .await?
    .unwrap_or(("Unknown".to_string(),));

    Ok(Json(SessionResponse {
        id: session.id,
        game_id: session.game_id,
        user_id: session.user_id,
        status: session.status,
        fee_amount: session.fee_amount,
        final_score: session.final_score,
        payout_status: session.payout_status,
        started_at: session.started_at,
        ended_at: session.ended_at,
        game_title: Some(game_title.0),
        connection_details: None,
    }))
}

pub async fn end_session(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<EndSessionRequest>,
) -> Result<Json<SessionResponse>> {
    let session = sqlx::query_as::<_, PlaySession>(
        "SELECT id, game_id, user_id, status, fee_amount, final_score,
                payout_status, started_at, ended_at
         FROM play_sessions
         WHERE id = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if session.status != "active" {
        return Err(AppError::Validation("Session is not active".to_string()));
    }

    let game = sqlx::query_as::<_, (Uuid, Decimal)>(
        "SELECT developer_id, fee_per_session FROM games WHERE id = $1",
    )
    .bind(session.game_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let platform_percentage = Decimal::from_str_exact(PLATFORM_FEE_PERCENTAGE)
        .unwrap_or(Decimal::new(15, 2));
    let developer_percentage = Decimal::ONE - platform_percentage;

    let developer_share = session.fee_amount * developer_percentage;
    let platform_share = session.fee_amount * platform_percentage;

    let updated_session = sqlx::query_as::<_, PlaySession>(
        "UPDATE play_sessions
         SET status = 'completed', final_score = $1, ended_at = NOW(), payout_status = 'pending'
         WHERE id = $2
         RETURNING id, game_id, user_id, status, fee_amount, final_score,
                   payout_status, started_at, ended_at",
    )
    .bind(payload.final_score)
    .bind(session_id)
    .fetch_one(&pool)
    .await?;

    if session.fee_amount > Decimal::ZERO {
        sqlx::query(
            "INSERT INTO game_revenue (id, game_id, developer_id, session_id, amount,
                                      developer_share, platform_share, status, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(session.game_id)
        .bind(game.0)
        .bind(session_id)
        .bind(session.fee_amount)
        .bind(developer_share)
        .bind(platform_share)
        .execute(&pool)
        .await?;
    }

    let game_title: (String,) = sqlx::query_as(
        "SELECT title FROM games WHERE id = $1",
    )
    .bind(updated_session.game_id)
    .fetch_optional(&pool)
    .await?
    .unwrap_or(("Unknown".to_string(),));

    Ok(Json(SessionResponse {
        id: updated_session.id,
        game_id: updated_session.game_id,
        user_id: updated_session.user_id,
        status: updated_session.status,
        fee_amount: updated_session.fee_amount,
        final_score: updated_session.final_score,
        payout_status: updated_session.payout_status,
        started_at: updated_session.started_at,
        ended_at: updated_session.ended_at,
        game_title: Some(game_title.0),
        connection_details: None,
    }))
}

pub async fn submit_score(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<SubmitScoreRequest>,
) -> Result<Json<SessionResponse>> {
    let session = sqlx::query_as::<_, PlaySession>(
        "SELECT id, game_id, user_id, status, fee_amount, final_score,
                payout_status, started_at, ended_at
         FROM play_sessions
         WHERE id = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if session.status != "active" {
        return Err(AppError::Validation("Can only submit score for active sessions".to_string()));
    }

    let existing = sqlx::query_as::<_, (i64,)>(
        "SELECT score FROM game_high_scores WHERE game_id = $1 AND user_id = $2",
    )
    .bind(session.game_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await?;

    let is_personal_best = existing.map(|e| payload.score > e.0).unwrap_or(true);

    sqlx::query(
        "INSERT INTO game_high_scores (game_id, user_id, score, recorded_at)
         VALUES ($1, $2, $3, NOW())
         ON CONFLICT (game_id, user_id)
         DO UPDATE SET score = $3, recorded_at = NOW()",
    )
    .bind(session.game_id)
    .bind(user_id)
    .bind(payload.score)
    .execute(&pool)
    .await?;

    let rank: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) + 1 FROM game_high_scores WHERE game_id = $1 AND score > $2",
    )
    .bind(session.game_id)
    .bind(payload.score)
    .fetch_one(&pool)
    .await?;

    let game_title: (String,) = sqlx::query_as(
        "SELECT title FROM games WHERE id = $1",
    )
    .bind(session.game_id)
    .fetch_optional(&pool)
    .await?
    .unwrap_or(("Unknown".to_string(),));

    Ok(Json(SessionResponse {
        id: session.id,
        game_id: session.game_id,
        user_id: session.user_id,
        status: session.status,
        fee_amount: session.fee_amount,
        final_score: Some(payload.score),
        payout_status: session.payout_status,
        started_at: session.started_at,
        ended_at: session.ended_at,
        game_title: Some(game_title.0),
        connection_details: None,
    }))
}
