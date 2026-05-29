use axum::{
    extract::{Extension, Path, Query, State},
    middleware::from_fn_with_state,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response::{self, PaginatedResponse};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "PascalCase")]
pub enum TournamentStatus {
    Draft,
    Registration,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for TournamentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TournamentStatus::Draft => write!(f, "Draft"),
            TournamentStatus::Registration => write!(f, "Registration"),
            TournamentStatus::InProgress => write!(f, "InProgress"),
            TournamentStatus::Completed => write!(f, "Completed"),
            TournamentStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl std::str::FromStr for TournamentStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Draft" => Ok(TournamentStatus::Draft),
            "Registration" => Ok(TournamentStatus::Registration),
            "InProgress" => Ok(TournamentStatus::InProgress),
            "Completed" => Ok(TournamentStatus::Completed),
            "Cancelled" => Ok(TournamentStatus::Cancelled),
            _ => Err(format!("Invalid tournament status: {}", s)),
        }
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Tournament {
    pub id: Uuid,
    pub name: String,
    pub game_id: Uuid,
    pub status: String,
    pub max_players: i32,
    pub entry_fee: Option<Decimal>,
    pub prize_pool: Decimal,
    pub start_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTournamentRequest {
    pub name: String,
    pub game_id: Uuid,
    pub max_players: Option<i32>,
    pub entry_fee: Option<Decimal>,
    pub prize_pool: Option<Decimal>,
    pub start_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTournamentRequest {
    pub name: Option<String>,
    pub status: Option<String>,
    pub max_players: Option<i32>,
    pub entry_fee: Option<Decimal>,
    pub prize_pool: Option<Decimal>,
    pub start_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitMatchResultRequest {
    pub winner_id: Uuid,
    pub player1_score: Option<i32>,
    pub player2_score: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TournamentParticipant {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub registered_at: DateTime<Utc>,
    pub status: String,
    pub seed: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TournamentMatch {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub round: i32,
    pub match_number: i32,
    pub player1_id: Option<Uuid>,
    pub player2_id: Option<Uuid>,
    pub winner_id: Option<Uuid>,
    pub player1_score: Option<i32>,
    pub player2_score: Option<i32>,
    pub status: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct TournamentQuery {
    pub status: Option<String>,
    pub game_id: Option<Uuid>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TournamentDetails {
    pub tournament: Tournament,
    pub participants: Vec<TournamentParticipant>,
    pub matches: Vec<TournamentMatch>,
}

pub async fn list_tournaments(
    State(pool): State<PgPool>,
    Query(query): Query<TournamentQuery>,
) -> Result<Json<PaginatedResponse<Tournament>>> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let (tournaments, total) = if let Some(game_id) = query.game_id {
        let status_filter = query
            .status
            .as_ref()
            .map(|s| format!("AND status = '{}'", s))
            .unwrap_or_default();

        let tournaments = sqlx::query_as::<_, Tournament>(
            &format!(
                "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
                 FROM tournaments WHERE game_id = $1 {} ORDER BY start_time DESC LIMIT $2 OFFSET $3",
                status_filter
            ),
        )
        .bind(game_id)
        .bind(per_page as i32)
        .bind(offset as i32)
        .fetch_all(&pool)
        .await?;

        let total: i64 = sqlx::query_scalar(
            &format!(
                "SELECT COUNT(*) FROM tournaments WHERE game_id = $1 {}",
                status_filter
            ),
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await?;

        (tournaments, total as u64)
    } else {
        let status_filter = query
            .status
            .as_ref()
            .map(|s| format!("AND status = '{}'", s))
            .unwrap_or_default();

        let tournaments = sqlx::query_as::<_, Tournament>(
            &format!(
                "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
                 FROM tournaments WHERE 1=1 {} ORDER BY start_time DESC LIMIT $1 OFFSET $2",
                status_filter
            ),
        )
        .bind(per_page as i32)
        .bind(offset as i32)
        .fetch_all(&pool)
        .await?;

        let total: i64 = sqlx::query_scalar(
            &format!("SELECT COUNT(*) FROM tournaments WHERE 1=1 {}", status_filter),
        )
        .fetch_one(&pool)
        .await?;

        (tournaments, total as u64)
    };

    Ok(response::paginated(tournaments, page, per_page, total))
}

pub async fn get_tournament(
    State(pool): State<PgPool>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<TournamentDetails>>> {
    let tournament = sqlx::query_as::<_, Tournament>(
        "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
         FROM tournaments WHERE id = $1",
    )
    .bind(tournament_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Tournament not found".to_string()))?;

    let participants = sqlx::query_as::<_, TournamentParticipant>(
        "SELECT id, tournament_id, user_id, registered_at, status, seed
         FROM tournament_participants WHERE tournament_id = $1 ORDER BY seed NULLS LAST",
    )
    .bind(tournament_id)
    .fetch_all(&pool)
    .await?;

    let matches = sqlx::query_as::<_, TournamentMatch>(
        "SELECT id, tournament_id, round, match_number, player1_id, player2_id, winner_id,
                player1_score, player2_score, status, scheduled_at, completed_at
         FROM tournament_matches WHERE tournament_id = $1 ORDER BY round, match_number",
    )
    .bind(tournament_id)
    .fetch_all(&pool)
    .await?;

    Ok(response::success_response(TournamentDetails {
        tournament,
        participants,
        matches,
    }))
}

pub async fn create_tournament(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Json(payload): Json<CreateTournamentRequest>,
) -> Result<Json<crate::api::response::ApiResponse<Tournament>>> {
    let _game = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM games WHERE id = $1 AND active = true")
        .bind(payload.game_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let tournament_id = Uuid::new_v4();
    let tournament = sqlx::query_as::<_, Tournament>(
        "INSERT INTO tournaments (id, name, game_id, status, max_players, entry_fee, prize_pool, start_time)
         VALUES ($1, $2, $3, 'draft', $4, $5, $6, $7)
         RETURNING id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at",
    )
    .bind(tournament_id)
    .bind(&payload.name)
    .bind(payload.game_id)
    .bind(payload.max_players.unwrap_or(8))
    .bind(payload.entry_fee)
    .bind(payload.prize_pool.unwrap_or(Decimal::ZERO))
    .bind(payload.start_time)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(tournament))
}

pub async fn update_tournament(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Path(tournament_id): Path<Uuid>,
    Json(payload): Json<UpdateTournamentRequest>,
) -> Result<Json<crate::api::response::ApiResponse<Tournament>>> {
    let existing = sqlx::query_as::<_, Tournament>(
        "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
         FROM tournaments WHERE id = $1",
    )
    .bind(tournament_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Tournament not found".to_string()))?;

    if existing.status != "Draft" && existing.status != "Registration" {
        return Err(AppError::BadRequest("Tournament cannot be updated in current state".to_string()));
    }

    if let Some(ref status) = payload.status {
        let valid_statuses = ["Draft", "Registration", "InProgress", "Completed", "Cancelled"];
        if !valid_statuses.contains(&status.as_str()) {
            return Err(AppError::Validation("Invalid status value".to_string()));
        }
    }

    let tournament = sqlx::query_as::<_, Tournament>(
        "UPDATE tournaments SET
         name = COALESCE($1, name),
         status = COALESCE($2, status),
         max_players = COALESCE($3, max_players),
         entry_fee = COALESCE($4, entry_fee),
         prize_pool = COALESCE($5, prize_pool),
         start_time = COALESCE($6, start_time),
         updated_at = NOW()
         WHERE id = $7
         RETURNING id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at",
    )
    .bind(&payload.name)
    .bind(&payload.status)
    .bind(payload.max_players)
    .bind(payload.entry_fee)
    .bind(payload.prize_pool)
    .bind(payload.start_time)
    .bind(tournament_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(tournament))
}

pub async fn register_for_tournament(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<TournamentParticipant>>> {
    let tournament = sqlx::query_as::<_, Tournament>(
        "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
         FROM tournaments WHERE id = $1",
    )
    .bind(tournament_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Tournament not found".to_string()))?;

    if tournament.status != "Registration" {
        return Err(AppError::BadRequest("Tournament is not open for registration".to_string()));
    }

    let current_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tournament_participants WHERE tournament_id = $1 AND status = 'registered'",
    )
    .bind(tournament_id)
    .fetch_one(&pool)
    .await?;

    if current_count >= tournament.max_players as i64 {
        return Err(AppError::BadRequest("Tournament is full".to_string()));
    }

    let participant_id = Uuid::new_v4();
    let participant = sqlx::query_as::<_, TournamentParticipant>(
        "INSERT INTO tournament_participants (id, tournament_id, user_id, status)
         VALUES ($1, $2, $3, 'registered')
         ON CONFLICT (tournament_id, user_id) DO UPDATE SET status = 'registered'
         RETURNING id, tournament_id, user_id, registered_at, status, seed",
    )
    .bind(participant_id)
    .bind(tournament_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(participant))
}

pub async fn start_tournament(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<Tournament>>> {
    let tournament = sqlx::query_as::<_, Tournament>(
        "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
         FROM tournaments WHERE id = $1",
    )
    .bind(tournament_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Tournament not found".to_string()))?;

    if tournament.status != "Registration" {
        return Err(AppError::BadRequest("Tournament must be in Registration status to start".to_string()));
    }

    let participants: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM tournament_participants WHERE tournament_id = $1 AND status = 'registered' ORDER BY seed NULLS LAST, registered_at",
    )
    .bind(tournament_id)
    .fetch_all(&pool)
    .await?;

    if participants.len() < 2 {
        return Err(AppError::BadRequest("Tournament needs at least 2 participants to start".to_string()));
    }

    let num_players = participants.len() as i32;
    let num_rounds = (num_players as f64).log2().ceil() as i32;

    for round in 1..=num_rounds {
        let matches_in_round = 2_i32.pow((num_rounds - round) as u32);
        for match_num in 1..=matches_in_round {
            let match_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO tournament_matches (id, tournament_id, round, match_number, status)
                 VALUES ($1, $2, $3, $4, 'pending')",
            )
            .bind(match_id)
            .bind(tournament_id)
            .bind(round)
            .bind(match_num)
            .execute(&pool)
            .await?;
        }
    }

    let tournament = sqlx::query_as::<_, Tournament>(
        "UPDATE tournaments SET status = 'InProgress', updated_at = NOW()
         WHERE id = $1
         RETURNING id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at",
    )
    .bind(tournament_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(tournament))
}

pub async fn submit_match_result(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path((tournament_id, match_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SubmitMatchResultRequest>,
) -> Result<Json<crate::api::response::ApiResponse<TournamentMatch>>> {
    let tournament = sqlx::query_as::<_, Tournament>(
        "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
         FROM tournaments WHERE id = $1",
    )
    .bind(tournament_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Tournament not found".to_string()))?;

    if tournament.status != "InProgress" {
        return Err(AppError::BadRequest("Tournament is not in progress".to_string()));
    }

    let tournament_match = sqlx::query_as::<_, TournamentMatch>(
        "SELECT id, tournament_id, round, match_number, player1_id, player2_id, winner_id,
                player1_score, player2_score, status, scheduled_at, completed_at
         FROM tournament_matches WHERE id = $1 AND tournament_id = $2",
    )
    .bind(match_id)
    .bind(tournament_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Match not found".to_string()))?;

    if tournament_match.status == "completed" {
        return Err(AppError::BadRequest("Match already completed".to_string()));
    }

    let valid_player = (tournament_match.player1_id == Some(user_id))
        || (tournament_match.player2_id == Some(user_id));
    if !valid_player {
        return Err(AppError::Forbidden("You are not a participant in this match".to_string()));
    }

    let updated_match = sqlx::query_as::<_, TournamentMatch>(
        "UPDATE tournament_matches SET
         winner_id = $1,
         player1_score = $2,
         player2_score = $3,
         status = 'completed',
         completed_at = NOW()
         WHERE id = $4
         RETURNING id, tournament_id, round, match_number, player1_id, player2_id, winner_id,
                   player1_score, player2_score, status, scheduled_at, completed_at",
    )
    .bind(payload.winner_id)
    .bind(payload.player1_score)
    .bind(payload.player2_score)
    .bind(match_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(updated_match))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_tournaments))
        .route("/", post(create_tournament).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/:id", get(get_tournament))
        .route("/:id", put(update_tournament).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/:id/register", post(register_for_tournament).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/:id/start", post(start_tournament).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .route("/:id/match/:match_id/result", post(submit_match_result).layer(from_fn_with_state(pool.clone(), middleware::auth_middleware)))
        .with_state(pool)
}