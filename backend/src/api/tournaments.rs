// Tournament API — bracket management, match results, registration; mounted at /api/v1/tournaments.
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

// ---------------------------------------------------------------------------
// Status enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "PascalCase")]
#[allow(dead_code)]
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

// ---------------------------------------------------------------------------
// Domain structs (sqlx::FromRow)
// ---------------------------------------------------------------------------

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

/// One row in the standings leaderboard view.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StandingEntry {
    pub user_id: Uuid,
    pub username: String,
    pub seed: Option<i32>,
    pub participant_status: String,
    pub wins: i64,
    pub losses: i64,
    pub points: i64,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

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
    /// Optional replay UUID to link to this match.
    pub replay_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct TournamentQuery {
    pub status: Option<String>,
    pub game_id: Option<Uuid>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

// ---------------------------------------------------------------------------
// Composite response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TournamentDetails {
    pub tournament: Tournament,
    pub participants: Vec<TournamentParticipant>,
    pub matches: Vec<TournamentMatch>,
}

// ---------------------------------------------------------------------------
// Handler: list_tournaments
// ---------------------------------------------------------------------------

pub async fn list_tournaments(
    State(pool): State<PgPool>,
    Query(query): Query<TournamentQuery>,
) -> Result<Json<PaginatedResponse<Tournament>>> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    // Build optional WHERE clauses from query parameters.  We avoid string
    // interpolation of user-supplied values; game_id and status are bound as
    // typed parameters or validated against an allowlist.
    let (tournaments, total) = match (query.game_id, &query.status) {
        (Some(gid), Some(s)) => {
            validate_status(s)?;
            let rows = sqlx::query_as::<_, Tournament>(
                "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
                 FROM tournaments
                 WHERE game_id = $1 AND status = $2
                 ORDER BY start_time DESC LIMIT $3 OFFSET $4",
            )
            .bind(gid)
            .bind(s)
            .bind(per_page as i64)
            .bind(offset as i64)
            .fetch_all(&pool)
            .await?;

            let total: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM tournaments WHERE game_id = $1 AND status = $2",
            )
            .bind(gid)
            .bind(s)
            .fetch_one(&pool)
            .await?;

            (rows, total as u64)
        }
        (Some(gid), None) => {
            let rows = sqlx::query_as::<_, Tournament>(
                "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
                 FROM tournaments WHERE game_id = $1 ORDER BY start_time DESC LIMIT $2 OFFSET $3",
            )
            .bind(gid)
            .bind(per_page as i64)
            .bind(offset as i64)
            .fetch_all(&pool)
            .await?;

            let total: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM tournaments WHERE game_id = $1")
                    .bind(gid)
                    .fetch_one(&pool)
                    .await?;

            (rows, total as u64)
        }
        (None, Some(s)) => {
            validate_status(s)?;
            let rows = sqlx::query_as::<_, Tournament>(
                "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
                 FROM tournaments WHERE status = $1 ORDER BY start_time DESC LIMIT $2 OFFSET $3",
            )
            .bind(s)
            .bind(per_page as i64)
            .bind(offset as i64)
            .fetch_all(&pool)
            .await?;

            let total: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM tournaments WHERE status = $1")
                    .bind(s)
                    .fetch_one(&pool)
                    .await?;

            (rows, total as u64)
        }
        (None, None) => {
            let rows = sqlx::query_as::<_, Tournament>(
                "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
                 FROM tournaments ORDER BY start_time DESC LIMIT $1 OFFSET $2",
            )
            .bind(per_page as i64)
            .bind(offset as i64)
            .fetch_all(&pool)
            .await?;

            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tournaments")
                .fetch_one(&pool)
                .await?;

            (rows, total as u64)
        }
    };

    Ok(response::paginated(tournaments, page, per_page, total))
}

// ---------------------------------------------------------------------------
// Handler: get_tournament
// ---------------------------------------------------------------------------

pub async fn get_tournament(
    State(pool): State<PgPool>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<TournamentDetails>>> {
    let tournament = fetch_tournament(&pool, tournament_id).await?;

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

// ---------------------------------------------------------------------------
// Handler: create_tournament
// ---------------------------------------------------------------------------

pub async fn create_tournament(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Json(payload): Json<CreateTournamentRequest>,
) -> Result<Json<crate::api::response::ApiResponse<Tournament>>> {
    sqlx::query_scalar::<_, bool>("SELECT active FROM games WHERE id = $1")
        .bind(payload.game_id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Game not found".to_string()))?;

    let max_players = payload.max_players.unwrap_or(8);
    if max_players < 2 || max_players > 512 {
        return Err(AppError::Validation(
            "max_players must be between 2 and 512".to_string(),
        ));
    }

    let tournament_id = Uuid::new_v4();
    let tournament = sqlx::query_as::<_, Tournament>(
        "INSERT INTO tournaments (id, name, game_id, status, max_players, entry_fee, prize_pool, start_time)
         VALUES ($1, $2, $3, 'Draft', $4, $5, $6, $7)
         RETURNING id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at",
    )
    .bind(tournament_id)
    .bind(&payload.name)
    .bind(payload.game_id)
    .bind(max_players)
    .bind(payload.entry_fee)
    .bind(payload.prize_pool.unwrap_or(Decimal::ZERO))
    .bind(payload.start_time)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(tournament))
}

// ---------------------------------------------------------------------------
// Handler: update_tournament
// ---------------------------------------------------------------------------

pub async fn update_tournament(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Path(tournament_id): Path<Uuid>,
    Json(payload): Json<UpdateTournamentRequest>,
) -> Result<Json<crate::api::response::ApiResponse<Tournament>>> {
    let existing = fetch_tournament(&pool, tournament_id).await?;

    if existing.status != "Draft" && existing.status != "Registration" {
        return Err(AppError::BadRequest(
            "Tournament cannot be updated in current state".to_string(),
        ));
    }

    if let Some(ref status) = payload.status {
        validate_status(status)?;
    }

    let tournament = sqlx::query_as::<_, Tournament>(
        "UPDATE tournaments SET
         name        = COALESCE($1, name),
         status      = COALESCE($2, status),
         max_players = COALESCE($3, max_players),
         entry_fee   = COALESCE($4, entry_fee),
         prize_pool  = COALESCE($5, prize_pool),
         start_time  = COALESCE($6, start_time),
         updated_at  = NOW()
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

// ---------------------------------------------------------------------------
// Handler: register_for_tournament
// ---------------------------------------------------------------------------

pub async fn register_for_tournament(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<TournamentParticipant>>> {
    let tournament = fetch_tournament(&pool, tournament_id).await?;

    if tournament.status != "Registration" {
        return Err(AppError::BadRequest(
            "Tournament is not open for registration".to_string(),
        ));
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

// ---------------------------------------------------------------------------
// Handler: start_tournament  (generates single-elimination bracket)
// ---------------------------------------------------------------------------

pub async fn start_tournament(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<TournamentDetails>>> {
    let tournament = fetch_tournament(&pool, tournament_id).await?;

    if tournament.status != "Registration" {
        return Err(AppError::BadRequest(
            "Tournament must be in Registration status to start".to_string(),
        ));
    }

    // Collect registered participants ordered by seed then registration time.
    let participants: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM tournament_participants
         WHERE tournament_id = $1 AND status = 'registered'
         ORDER BY seed NULLS LAST, registered_at",
    )
    .bind(tournament_id)
    .fetch_all(&pool)
    .await?;

    if participants.len() < 2 {
        return Err(AppError::BadRequest(
            "Tournament needs at least 2 participants to start".to_string(),
        ));
    }

    // Build a power-of-two single-elimination bracket.
    // Participants may be fewer than the bracket size; bye slots are left as
    // NULL player IDs in later rounds.
    let n = participants.len();
    // Round up to the next power of two for the bracket size.
    let bracket_size: usize = n.next_power_of_two();
    let num_rounds = bracket_size.trailing_zeros() as i32; // log2(bracket_size)

    // Round 1: seed-ordered matchups (1 vs bracket_size, 2 vs bracket_size-1, …).
    // Players that get a bye (no opponent) auto-advance — handled at submit_match_result.
    let mut tx = pool.begin().await?;

    for round in 1..=num_rounds {
        let matches_in_round = bracket_size >> round; // bracket_size / 2^round
        for match_num in 1..=(matches_in_round as i32) {
            let match_id = Uuid::new_v4();

            // Only seed Round 1 with real player IDs; later rounds are filled
            // in by advance_bracket as results arrive.
            let (p1, p2) = if round == 1 {
                let idx1 = (match_num - 1) as usize; // 0-indexed upper seed
                let idx2 = bracket_size - 1 - idx1; // mirrored lower seed
                let p1 = participants.get(idx1).map(|r| r.0);
                // idx2 may exceed len — that slot is a bye (None).
                let p2 = participants.get(idx2).map(|r| r.0);
                (p1, p2)
            } else {
                (None, None)
            };

            sqlx::query(
                "INSERT INTO tournament_matches
                 (id, tournament_id, round, match_number, player1_id, player2_id, status)
                 VALUES ($1, $2, $3, $4, $5, $6,
                         CASE WHEN $5 IS NOT NULL AND $6 IS NULL THEN 'bye'
                              ELSE 'pending' END)",
            )
            .bind(match_id)
            .bind(tournament_id)
            .bind(round)
            .bind(match_num)
            .bind(p1)
            .bind(p2)
            .execute(&mut *tx)
            .await?;

            // If it's a bye, auto-set winner_id so advance_bracket works cleanly.
            if round == 1 && p1.is_some() && p2.is_none() {
                sqlx::query(
                    "UPDATE tournament_matches SET winner_id = $1, completed_at = NOW()
                     WHERE tournament_id = $2 AND round = $3 AND match_number = $4",
                )
                .bind(p1)
                .bind(tournament_id)
                .bind(round)
                .bind(match_num)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    // Mark tournament as InProgress.
    sqlx::query("UPDATE tournaments SET status = 'InProgress', updated_at = NOW() WHERE id = $1")
        .bind(tournament_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // Return the full detail view so the caller has the generated bracket.
    get_tournament(State(pool), Path(tournament_id)).await
}

// ---------------------------------------------------------------------------
// Handler: submit_match_result  +  bracket advancement
// ---------------------------------------------------------------------------

pub async fn submit_match_result(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path((tournament_id, match_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SubmitMatchResultRequest>,
) -> Result<Json<crate::api::response::ApiResponse<TournamentMatch>>> {
    let tournament = fetch_tournament(&pool, tournament_id).await?;

    if tournament.status != "InProgress" {
        return Err(AppError::BadRequest(
            "Tournament is not in progress".to_string(),
        ));
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

    // The winner must be one of the two players in the match.
    let valid_player = (tournament_match.player1_id == Some(payload.winner_id))
        || (tournament_match.player2_id == Some(payload.winner_id));
    if !valid_player {
        return Err(AppError::BadRequest(
            "winner_id is not a participant in this match".to_string(),
        ));
    }

    // Only match participants or (for now) any authenticated user can report.
    // Guard: at minimum the caller must be one of the two players.
    let is_participant = (tournament_match.player1_id == Some(user_id))
        || (tournament_match.player2_id == Some(user_id));
    if !is_participant {
        return Err(AppError::Forbidden(
            "You are not a participant in this match".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;

    // Update the match as completed.
    let updated_match = sqlx::query_as::<_, TournamentMatch>(
        "UPDATE tournament_matches SET
         winner_id      = $1,
         player1_score  = $2,
         player2_score  = $3,
         status         = 'completed',
         completed_at   = NOW()
         WHERE id = $4
         RETURNING id, tournament_id, round, match_number, player1_id, player2_id, winner_id,
                   player1_score, player2_score, status, scheduled_at, completed_at",
    )
    .bind(payload.winner_id)
    .bind(payload.player1_score)
    .bind(payload.player2_score)
    .bind(match_id)
    .fetch_one(&mut *tx)
    .await?;

    // Optionally link a replay to this match.
    if let Some(rid) = payload.replay_id {
        sqlx::query("UPDATE tournament_matches SET replay_id = $1 WHERE id = $2")
            .bind(rid)
            .bind(match_id)
            .execute(&mut *tx)
            .await?;
    }

    // Advance bracket: seed the winner into the next round's match slot.
    advance_bracket(&mut tx, tournament_id, &updated_match).await?;

    // Check if the entire bracket is done → mark tournament Completed.
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tournament_matches
         WHERE tournament_id = $1 AND status NOT IN ('completed','bye')",
    )
    .bind(tournament_id)
    .fetch_one(&mut *tx)
    .await?;

    if remaining == 0 {
        sqlx::query(
            "UPDATE tournaments SET status = 'Completed', updated_at = NOW() WHERE id = $1",
        )
        .bind(tournament_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(response::success_response(updated_match))
}

/// Seed the winner of `finished_match` into the correct slot in the next round.
///
/// In a standard single-elimination bracket:
/// - matches are numbered 1..N in each round
/// - the winner of match M feeds into round+1, match ceil(M/2)
/// - they take player1 slot if M is odd, player2 slot if M is even
async fn advance_bracket(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tournament_id: Uuid,
    finished: &TournamentMatch,
) -> Result<()> {
    let next_round = finished.round + 1;
    let next_match_number = (finished.match_number + 1) / 2;

    // Check whether a next-round match exists for this slot.
    let next_match: Option<(Uuid, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, player1_id, player2_id FROM tournament_matches
         WHERE tournament_id = $1 AND round = $2 AND match_number = $3",
    )
    .bind(tournament_id)
    .bind(next_round)
    .bind(next_match_number)
    .fetch_optional(&mut **tx)
    .await?;

    let (next_id, p1, _p2) = match next_match {
        Some(row) => row,
        None => return Ok(()), // Final round — no next match to seed.
    };

    let winner_id = match finished.winner_id {
        Some(id) => id,
        None => return Ok(()), // No winner yet; shouldn't happen but guard anyway.
    };

    // Odd match_number → player1 slot, even → player2 slot.
    if finished.match_number % 2 == 1 {
        sqlx::query("UPDATE tournament_matches SET player1_id = $1 WHERE id = $2")
            .bind(winner_id)
            .bind(next_id)
            .execute(&mut **tx)
            .await?;
    } else {
        sqlx::query("UPDATE tournament_matches SET player2_id = $1 WHERE id = $2")
            .bind(winner_id)
            .bind(next_id)
            .execute(&mut **tx)
            .await?;
    }

    // If now both slots are filled, activate the match.
    let refreshed: (Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("SELECT player1_id, player2_id FROM tournament_matches WHERE id = $1")
            .bind(next_id)
            .fetch_one(&mut **tx)
            .await?;

    if refreshed.0.is_some() && refreshed.1.is_some() {
        sqlx::query(
            "UPDATE tournament_matches SET status = 'scheduled' WHERE id = $1 AND status = 'pending'",
        )
        .bind(next_id)
        .execute(&mut **tx)
        .await?;
    } else if refreshed.0.is_none() && p1.is_none() {
        // Other slot is also empty — leave as pending.
        // If exactly one side has a player and it's a bye, auto-advance.
        let winner = refreshed.0.or(refreshed.1);
        if let Some(w) = winner {
            sqlx::query(
                "UPDATE tournament_matches
                 SET status = 'bye', winner_id = $1, completed_at = NOW()
                 WHERE id = $2 AND status = 'pending'",
            )
            .bind(w)
            .bind(next_id)
            .execute(&mut **tx)
            .await?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Handler: standings  (leaderboard for a tournament)
// ---------------------------------------------------------------------------

pub async fn get_standings(
    State(pool): State<PgPool>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<Vec<StandingEntry>>>> {
    fetch_tournament(&pool, tournament_id).await?; // 404 guard

    let standings = sqlx::query_as::<_, StandingEntry>(
        "SELECT user_id, username, seed, participant_status,
                COALESCE(wins, 0)   AS wins,
                COALESCE(losses, 0) AS losses,
                COALESCE(points, 0) AS points
         FROM tournament_standings
         WHERE tournament_id = $1
         ORDER BY points DESC, wins DESC, losses ASC, seed ASC NULLS LAST",
    )
    .bind(tournament_id)
    .fetch_all(&pool)
    .await?;

    Ok(response::success_response(standings))
}

// ---------------------------------------------------------------------------
// Handler: get_bracket  (matches only, public)
// ---------------------------------------------------------------------------

pub async fn get_bracket(
    State(pool): State<PgPool>,
    Path(tournament_id): Path<Uuid>,
) -> Result<Json<crate::api::response::ApiResponse<Vec<TournamentMatch>>>> {
    fetch_tournament(&pool, tournament_id).await?;

    let matches = sqlx::query_as::<_, TournamentMatch>(
        "SELECT id, tournament_id, round, match_number, player1_id, player2_id, winner_id,
                player1_score, player2_score, status, scheduled_at, completed_at
         FROM tournament_matches WHERE tournament_id = $1 ORDER BY round, match_number",
    )
    .bind(tournament_id)
    .fetch_all(&pool)
    .await?;

    Ok(response::success_response(matches))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

async fn fetch_tournament(pool: &PgPool, id: Uuid) -> Result<Tournament> {
    sqlx::query_as::<_, Tournament>(
        "SELECT id, name, game_id, status, max_players, entry_fee, prize_pool, start_time, created_at
         FROM tournaments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Tournament not found".to_string()))
}

fn validate_status(s: &str) -> Result<()> {
    const VALID: &[&str] = &[
        "Draft",
        "Registration",
        "InProgress",
        "Completed",
        "Cancelled",
    ];
    if VALID.contains(&s) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("Invalid status: {}", s)))
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_tournaments))
        .route(
            "/",
            post(create_tournament).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route("/:id", get(get_tournament))
        .route(
            "/:id",
            put(update_tournament).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:id/register",
            post(register_for_tournament).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:id/start",
            post(start_tournament).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/:id/match/:match_id/result",
            post(submit_match_result).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // Bracket view (public)
        .route("/:id/bracket", get(get_bracket))
        // Standings/leaderboard (public)
        .route("/:id/standings", get(get_standings))
        .with_state(pool)
}
