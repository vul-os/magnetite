use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SkillRange {
    pub min: f64,
    pub max: f64,
}

impl SkillRange {
    pub fn contains(&self, skill: f64) -> bool {
        skill >= self.min && skill <= self.max
    }

    pub fn overlaps(&self, other: &SkillRange) -> bool {
        self.min <= other.max && self.max >= other.min
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QueuedPlayer {
    pub user_id: Uuid,
    pub skill_rating: f64,
    pub joined_at: DateTime<Utc>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub user_id: Uuid,
    pub game_id: Uuid,
    pub position: i32,
    pub joined_at: DateTime<Utc>,
    pub estimated_wait_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub position: i32,
    pub total_in_queue: i32,
    pub estimated_wait_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub match_id: Uuid,
    pub player_ids: Vec<Uuid>,
    pub skill_range: SkillRange,
    pub region: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Match {
    pub id: Uuid,
    pub game_id: Uuid,
    pub status: String,
    pub skill_range: SkillRange,
    pub region: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub match_id: Uuid,
    pub player_ids: Vec<Uuid>,
    pub region: Option<String>,
    pub started_at: DateTime<Utc>,
    pub server_endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Party {
    pub id: Uuid,
    pub leader_id: Uuid,
    pub member_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyMatch {
    pub party_ids: Vec<Uuid>,
    pub combined_players: Vec<Uuid>,
    pub skill_range: SkillRange,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserStats {
    pub user_id: Uuid,
    pub elo_rating: f64,
    pub region: String,
    pub games_played: i32,
}

pub async fn elo_rating(db: &sqlx::PgPool, user_id: Uuid) -> Result<f64> {
    let result = sqlx::query_as::<_, (f64,)>(
        "SELECT COALESCE(elo_rating, 1000.0) FROM user_stats WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(result.map(|r| r.0).unwrap_or(1000.0))
}

pub fn calculate_skill_ranges(queue_time: Duration) -> SkillRange {
    let minutes = queue_time.num_minutes() as f64;
    let base_range = 100.0;
    let expansion_rate = 25.0;
    let expanded_range = base_range + (minutes * expansion_rate).min(500.0);

    SkillRange {
        min: 0.0,
        max: expanded_range,
    }
}

pub async fn find_match(
    db: &sqlx::PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<Option<MatchResult>> {
    let user_skill = elo_rating(db, user_id).await?;
    let queue_time = Duration::minutes(2);
    let skill_range = calculate_skill_ranges(queue_time);
    let user_region = get_user_region(db, user_id).await.ok();

    let candidates = sqlx::query_as::<_, QueuedPlayer>(
        r#"
        SELECT user_id, skill_rating, joined_at, ready
        FROM matchmaking_queue
        WHERE game_id = $1
          AND user_id != $2
          AND ready = true
          AND skill_rating BETWEEN $3 AND $4
          AND joined_at > NOW() - INTERVAL '1 hour'
        ORDER BY ABS(skill_rating - $5)
        LIMIT 10
        "#,
    )
    .bind(game_id)
    .bind(user_id)
    .bind(user_skill - skill_range.max)
    .bind(user_skill + skill_range.max)
    .bind(user_skill)
    .fetch_all(db)
    .await?;

    let mut matched_players = vec![QueuedPlayer {
        user_id,
        skill_rating: user_skill,
        joined_at: Utc::now(),
        ready: true,
    }];

    for candidate in candidates {
        let candidate_skill = candidate.skill_rating;
        let candidate_region = get_user_region(db, candidate.user_id).await.ok();

        if let Some(ref ur) = user_region {
            if let Some(ref cr) = candidate_region {
                if ur != cr {
                    continue;
                }
            }
        }

        let avg_skill = (user_skill + candidate_skill) / 2.0;
        let match_range = SkillRange {
            min: avg_skill - 50.0,
            max: avg_skill + 50.0,
        };

        if matched_players.len() < 4 {
            matched_players.push(candidate);
        }
    }

    if matched_players.len() >= 2 {
        let match_id = Uuid::new_v4();
        let skill_ratings: Vec<f64> = matched_players.iter().map(|p| p.skill_rating).collect();
        let min_skill = skill_ratings.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_skill = skill_ratings.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        Ok(Some(MatchResult {
            match_id,
            player_ids: matched_players.into_iter().map(|p| p.user_id).collect(),
            skill_range: SkillRange { min: min_skill, max: max_skill },
            region: user_region,
            created_at: Utc::now(),
        }))
    } else {
        Ok(None)
    }
}

pub async fn create_party(
    db: &sqlx::PgPool,
    leader_id: Uuid,
    member_ids: Vec<Uuid>,
) -> Result<Party> {
    let mut all_member_ids = vec![leader_id];
    all_member_ids.extend(member_ids);

    for member_id in &all_member_ids {
        let in_party = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM parties WHERE $1 = ANY(member_ids) AND created_at > NOW() - INTERVAL '1 hour')",
        )
        .bind(member_id)
        .fetch_one(db)
        .await?;

        if in_party {
            return Err(AppError::BadRequest(format!(
                "User {} is already in a party",
                member_id
            )));
        }
    }

    let party_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO parties (id, leader_id, member_ids, created_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(party_id)
    .bind(leader_id)
    .bind(&all_member_ids)
    .bind(now)
    .execute(db)
    .await?;

    Ok(Party {
        id: party_id,
        leader_id,
        member_ids: all_member_ids[1..].to_vec(),
        created_at: now,
    })
}

pub async fn match_parties(
    db: &sqlx::PgPool,
    queue: &MatchmakingQueue,
) -> Result<Vec<PartyMatch>> {
    let parties = sqlx::query_as::<_, Party>(
        r#"
        SELECT id, leader_id, member_ids, created_at
        FROM parties
        WHERE created_at > NOW() - INTERVAL '1 hour'
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(db)
    .await?;

    let mut matches = Vec::new();
    let mut used_parties = std::collections::HashSet::new();

    for party in &parties {
        if used_parties.contains(&party.id) {
            continue;
        }

        let mut combined_players = vec![party.leader_id];
        combined_players.extend(party.member_ids.clone());

        let mut total_skill = 0.0;
        let mut player_count = 0;
        let mut party_regions = Vec::new();

        for player_id in &combined_players {
            if let Ok(skill) = elo_rating(db, *player_id).await {
                total_skill += skill;
                player_count += 1;
            }
            if let Ok(region) = get_user_region(db, *player_id).await {
                party_regions.push(region);
            }
        }

        if player_count == 0 {
            continue;
        }

        let avg_skill = total_skill / player_count as f64;
        let mut matching_parties: Vec<Party> = Vec::new();
        for p in parties.iter() {
            if used_parties.contains(&p.id) || p.id == party.id {
                continue;
            }
            let mut p_combined = vec![p.leader_id];
            p_combined.extend(p.member_ids.clone());
            let mut p_total_skill = 0.0;
            let mut p_count = 0;
            for pid in &p_combined {
                if let Ok(skill) = elo_rating(db, *pid).await {
                    p_total_skill += skill;
                    p_count += 1;
                }
            }
            if p_count == 0 {
                continue;
            }
            let p_avg = p_total_skill / p_count as f64;
            if (avg_skill - p_avg).abs() < 100.0 {
                matching_parties.push(p.clone());
                if matching_parties.len() >= 3 {
                    break;
                }
            }
        }

        if !matching_parties.is_empty() {
            let mut all_players = combined_players.clone();
            let mut min_skill = avg_skill;
            let mut max_skill = avg_skill;

            for mp in &matching_parties {
                used_parties.insert(mp.id);
                let mut mp_combined = vec![mp.leader_id];
                mp_combined.extend(mp.member_ids.clone());

                for pid in &mp_combined {
                    if let Ok(skill) = elo_rating(db, *pid).await {
                        min_skill = min_skill.min(skill);
                        max_skill = max_skill.max(skill);
                    }
                }

                all_players.extend(mp_combined);
            }

            used_parties.insert(party.id);

            let region = party_regions.first().cloned();

            matches.push(PartyMatch {
                party_ids: std::iter::once(party.id)
                    .chain(matching_parties.iter().map(|p| p.id))
                    .collect(),
                combined_players: all_players,
                skill_range: SkillRange { min: min_skill, max: max_skill },
                region,
            });
        }
    }

    Ok(matches)
}

pub async fn get_user_region(db: &sqlx::PgPool, user_id: Uuid) -> Result<String> {
    let region = sqlx::query_scalar::<_, String>(
        "SELECT region FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    region.ok_or_else(|| AppError::NotFound("User not found".to_string()))
}

pub fn filter_by_region(
    players: Vec<QueuedPlayer>,
    region: String,
) -> Vec<QueuedPlayer> {
    players
}

pub async fn create_match(
    db: &sqlx::PgPool,
    player_ids: Vec<Uuid>,
    game_id: Uuid,
) -> Result<Match> {
    if player_ids.len() < 2 {
        return Err(AppError::Validation(
            "Match requires at least 2 players".to_string(),
        ));
    }

    let match_id = Uuid::new_v4();
    let now = Utc::now();

    let mut min_skill = f64::INFINITY;
    let mut max_skill = f64::NEG_INFINITY;
    let mut region: Option<String> = None;

    for player_id in &player_ids {
        let skill = elo_rating(db, *player_id).await?;
        min_skill = min_skill.min(skill);
        max_skill = max_skill.max(skill);

        if region.is_none() {
            region = get_user_region(db, *player_id).await.ok();
        }
    }

    let skill_range = SkillRange {
        min: min_skill,
        max: max_skill,
    };

    sqlx::query(
        r#"
        INSERT INTO matches (id, game_id, status, skill_range, region, created_at)
        VALUES ($1, $2, 'pending', $3, $4, $5)
        "#,
    )
    .bind(match_id)
    .bind(game_id)
    .bind(serde_json::to_string(&skill_range).unwrap_or_default())
    .bind(&region)
    .bind(now)
    .execute(db)
    .await?;

    for player_id in &player_ids {
        sqlx::query(
            r#"
            INSERT INTO match_players (match_id, user_id, joined_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(match_id)
        .bind(player_id)
        .bind(now)
        .execute(db)
        .await?;
    }

    Ok(Match {
        id: match_id,
        game_id,
        status: "pending".to_string(),
        skill_range,
        region,
        created_at: now,
        started_at: None,
    })
}

pub async fn start_game_session(db: &sqlx::PgPool, r#match: &Match) -> Result<SessionInfo> {
    let session_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        UPDATE matches
        SET status = 'active', started_at = $1
        WHERE id = $2
        "#,
    )
    .bind(now)
    .bind(r#match.id)
    .execute(db)
    .await?;

    let player_ids: Vec<Uuid> = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM match_players WHERE match_id = $1",
    )
    .bind(r#match.id)
    .fetch_all(db)
    .await?;

    Ok(SessionInfo {
        session_id,
        match_id: r#match.id,
        player_ids,
        region: r#match.region.clone(),
        started_at: now,
        server_endpoint: None,
    })
}

pub async fn join_queue(
    db: &sqlx::PgPool,
    user_id: Uuid,
    game_id: Uuid,
) -> Result<QueueEntry> {
    let already_queued = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM matchmaking_queue WHERE user_id = $1 AND game_id = $2 AND ready = false)",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_one(db)
    .await?;

    if already_queued {
        return Err(AppError::BadRequest("User already in queue".to_string()));
    }

    let skill_rating = elo_rating(db, user_id).await?;
    let now = Utc::now();
    let queue_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO matchmaking_queue (id, user_id, game_id, skill_rating, joined_at, ready)
        VALUES ($1, $2, $3, $4, $5, false)
        "#,
    )
    .bind(queue_id)
    .bind(user_id)
    .bind(game_id)
    .bind(skill_rating)
    .bind(now)
    .execute(db)
    .await?;

    let position = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM matchmaking_queue WHERE game_id = $1 AND joined_at <= $2",
    )
    .bind(game_id)
    .bind(now)
    .fetch_one(db)
    .await?;

    let estimated_wait = Duration::seconds((position as i64) * 30);

    Ok(QueueEntry {
        user_id,
        game_id,
        position,
        joined_at: now,
        estimated_wait_time: estimated_wait,
    })
}

pub async fn leave_queue(db: &sqlx::PgPool, user_id: Uuid) -> Result<bool> {
    let result = sqlx::query(
        "DELETE FROM matchmaking_queue WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn get_queue_position(db: &sqlx::PgPool, user_id: Uuid) -> Result<Option<Position>> {
    let entry = sqlx::query_as::<_, QueuedPlayer>(
        "SELECT user_id, skill_rating, joined_at, ready FROM matchmaking_queue WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    match entry {
        Some(e) => {
            let total = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM matchmaking_queue WHERE game_id = (SELECT game_id FROM matchmaking_queue WHERE user_id = $1)",
            )
            .bind(user_id)
            .fetch_one(db)
            .await?;

            let position = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM matchmaking_queue WHERE joined_at < $1",
            )
            .bind(e.joined_at)
            .fetch_one(db)
            .await? + 1;

            let estimated_wait = Duration::seconds((position as i64) * 30);

            Ok(Some(Position {
                position,
                total_in_queue: total,
                estimated_wait_time: estimated_wait,
            }))
        }
        None => Ok(None),
    }
}

pub struct MatchmakingQueue {
    pub game_id: Uuid,
    pub players: Vec<QueuedPlayer>,
    pub skill_ranges: SkillRange,
}

impl MatchmakingQueue {
    pub fn new(game_id: Uuid, skill_range: SkillRange) -> Self {
        Self {
            game_id,
            players: Vec::new(),
            skill_ranges: skill_range,
        }
    }

    pub fn add_player(&mut self, player: QueuedPlayer) {
        if self.skill_ranges.contains(player.skill_rating) {
            self.players.push(player);
        }
    }

    pub fn remove_player(&mut self, user_id: Uuid) -> bool {
        let len = self.players.len();
        self.players.retain(|p| p.user_id != user_id);
        self.players.len() < len
    }

    pub fn find_balanced_teams(&self, team_size: usize) -> Option<(Vec<Uuid>, Vec<Uuid>)> {
        if self.players.len() < team_size * 2 {
            return None;
        }

        let mut sorted_players = self.players.clone();
        sorted_players.sort_by(|a, b| {
            b.skill_rating
                .partial_cmp(&a.skill_rating)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut team1 = Vec::new();
        let mut team2 = Vec::new();
        let mut team1_skill = 0.0;
        let mut team2_skill = 0.0;

        for (i, player) in sorted_players.iter().enumerate() {
            if i % 2 == 0 {
                if team1_skill <= team2_skill {
                    team1.push(player.user_id);
                    team1_skill += player.skill_rating;
                } else {
                    team2.push(player.user_id);
                    team2_skill += player.skill_rating;
                }
            } else {
                if team2_skill < team1_skill {
                    team2.push(player.user_id);
                    team2_skill += player.skill_rating;
                } else {
                    team1.push(player.user_id);
                    team1_skill += player.skill_rating;
                }
            }
        }

        if team1.len() == team_size && team2.len() == team_size {
            Some((team1, team2))
        } else {
            None
        }
    }
}
