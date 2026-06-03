use axum::{
    extract::{Extension, Path, Query, State},
    middleware::from_fn_with_state,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};

// ── Content moderation heuristic ─────────────────────────────────────────────
//
// Lightweight, dependency-free content check.  Returns a Vec of triggered
// flag reasons; empty means clean.  Callers may insert a review_report row
// when the result is non-empty.

/// Profanity/abuse wordlist — lower-case, checked via substring.
const PROFANITY_WORDS: &[&str] = &[
    "fuck", "shit", "asshole", "bitch", "cunt", "nigger", "faggot", "retard", "whore", "bastard",
    "dick", "cock", "pussy",
];

/// Spam-signal words/phrases — marketing, scam, or off-topic solicitation.
const SPAM_WORDS: &[&str] = &[
    "buy now",
    "click here",
    "free money",
    "earn $",
    "make money fast",
    "limited offer",
    "casino",
    "bitcoin investment",
    "crypto giveaway",
    "discount code",
    "promo code",
    "follow me",
    "check my profile",
];

/// URL pattern — matches http/https or bare domain patterns.
fn looks_like_url(s: &str) -> bool {
    s.contains("http://")
        || s.contains("https://")
        || s.contains("www.")
        // bare domain with common TLDs
        || {
            let s = s.to_lowercase();
            [".com", ".net", ".org", ".io", ".gg", ".xyz"]
                .iter()
                .any(|tld| s.contains(tld))
        }
}

/// Count distinct "words" (whitespace-separated tokens) in a string.
fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Repetition heuristic: flag if any single word appears more than 5 times.
fn has_word_repetition(s: &str, threshold: usize) -> bool {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for word in s.split_whitespace() {
        let c = counts.entry(word).or_insert(0);
        *c += 1;
        if *c >= threshold {
            return true;
        }
    }
    false
}

/// Count URLs in content.
fn url_count(s: &str) -> usize {
    s.split_whitespace().filter(|w| looks_like_url(w)).count()
}

/// Run all heuristics and return triggered flag reasons.
pub fn content_flag_reasons(content: &str) -> Vec<String> {
    let lower = content.to_lowercase();
    let mut reasons = Vec::new();

    // 1. Profanity
    if PROFANITY_WORDS.iter().any(|w| lower.contains(w)) {
        reasons.push("profanity".to_string());
    }

    // 2. Spam keywords
    if SPAM_WORDS.iter().any(|w| lower.contains(w)) {
        reasons.push("spam".to_string());
    }

    // 3. URL flood — more than 2 URLs in the content
    if url_count(&lower) > 2 {
        reasons.push("url_flood".to_string());
    }

    // 4. Word repetition — any word used 6+ times
    if word_count(content) > 5 && has_word_repetition(content, 6) {
        reasons.push("repetition".to_string());
    }

    reasons
}

#[cfg(test)]
mod heuristic_tests {
    use super::*;

    #[test]
    fn clean_content_passes() {
        assert!(content_flag_reasons("Great game, really enjoyed it.").is_empty());
    }

    #[test]
    fn profanity_flagged() {
        let r = content_flag_reasons("This game is such bullshit and fucking terrible");
        assert!(
            r.contains(&"profanity".to_string()),
            "expected profanity: {:?}",
            r
        );
    }

    #[test]
    fn spam_flagged() {
        let r = content_flag_reasons("Buy now and earn $ fast with this amazing deal");
        assert!(r.contains(&"spam".to_string()), "expected spam: {:?}", r);
    }

    #[test]
    fn url_flood_flagged() {
        let r = content_flag_reasons(
            "Visit http://example.com http://spam.net https://click.io/here for deals",
        );
        assert!(
            r.contains(&"url_flood".to_string()),
            "expected url_flood: {:?}",
            r
        );
    }

    #[test]
    fn repetition_flagged() {
        let r = content_flag_reasons("bad bad bad bad bad bad game");
        assert!(
            r.contains(&"repetition".to_string()),
            "expected repetition: {:?}",
            r
        );
    }
}

// ── Review types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Review {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_id: Uuid,
    pub rating: i32,
    pub content: Option<String>,
    pub helpful_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReviewWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub game_id: Uuid,
    pub rating: i32,
    pub content: Option<String>,
    pub helpful_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReviewRequest {
    pub rating: i32,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReviewRequest {
    pub rating: Option<i32>,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewListQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GameRating {
    pub average: f64,
    pub count: i64,
}

// ── Helpful / report types ────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HelpfulToggleResult {
    pub voted: bool,
    pub helpful_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct ReportReviewRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ReviewReportResult {
    pub id: Uuid,
    pub review_id: Uuid,
    pub reporter_id: Uuid,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn has_played_game(pool: &PgPool, user_id: Uuid, game_id: Uuid) -> Result<bool> {
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM play_sessions WHERE user_id = $1 AND game_id = $2 AND status = 'completed'",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_one(pool)
    .await?;

    Ok(result > 0)
}

// ── Existing review handlers ──────────────────────────────────────────────────

pub async fn list_reviews(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
    Query(query): Query<ReviewListQuery>,
) -> Result<Json<response::PaginatedResponse<ReviewWithUser>>> {
    let page = query.page.unwrap_or(1).max(1) as u32;
    let limit = query.limit.unwrap_or(20).clamp(1, 100) as u32;
    let offset = (page - 1) * limit;

    let sort = query.sort.as_deref().unwrap_or("recent");
    let order_clause = match sort {
        "rating_high" => "r.rating DESC, r.created_at DESC",
        "rating_low" => "r.rating ASC, r.created_at DESC",
        "most_helpful" => "r.helpful_count DESC, r.created_at DESC",
        _ => "r.created_at DESC",
    };

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reviews WHERE game_id = $1")
        .bind(game_id)
        .fetch_one(&pool)
        .await?;

    let reviews = sqlx::query_as::<_, ReviewWithUser>(&format!(
        "SELECT r.id, r.user_id, u.username, r.game_id, r.rating, r.content,
                    r.helpful_count, r.created_at, r.updated_at
             FROM reviews r
             JOIN users u ON r.user_id = u.id
             WHERE r.game_id = $1
             ORDER BY {}
             LIMIT $2 OFFSET $3",
        order_clause
    ))
    .bind(game_id)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(&pool)
    .await?;

    Ok(response::paginated(reviews, page, limit, total as u64))
}

pub async fn create_review(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<CreateReviewRequest>,
) -> Result<Json<response::ApiResponse<Review>>> {
    if payload.rating < 1 || payload.rating > 5 {
        return Err(AppError::Validation(
            "Rating must be between 1 and 5".to_string(),
        ));
    }

    let game_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM games WHERE id = $1 AND active = true)",
    )
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    if !game_exists {
        return Err(AppError::NotFound("Game not found".to_string()));
    }

    if !has_played_game(&pool, user_id, game_id).await? {
        return Err(AppError::Forbidden(
            "You must play the game before reviewing it".to_string(),
        ));
    }

    let existing_review = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM reviews WHERE user_id = $1 AND game_id = $2)",
    )
    .bind(user_id)
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    if existing_review {
        return Err(AppError::BadRequest(
            "You have already reviewed this game".to_string(),
        ));
    }

    let review = sqlx::query_as::<_, Review>(
        "INSERT INTO reviews (user_id, game_id, rating, content, created_at, updated_at)
         VALUES ($1, $2, $3, $4, NOW(), NOW())
         RETURNING id, user_id, game_id, rating, content, helpful_count, created_at, updated_at",
    )
    .bind(user_id)
    .bind(game_id)
    .bind(payload.rating)
    .bind(&payload.content)
    .fetch_one(&pool)
    .await?;

    // ── Auto-flag heuristic ───────────────────────────────────────────────────
    // Run content checks on the review text.  If any heuristic fires, insert a
    // review_report row with source='auto_flag' so moderators see it in the queue.
    // Failure to insert the flag is non-fatal (we log and continue).
    if let Some(ref text) = review.content {
        let reasons = content_flag_reasons(text);
        if !reasons.is_empty() {
            let reason_str = reasons.join(", ");
            tracing::info!(
                review_id = %review.id,
                user_id = %user_id,
                reasons = %reason_str,
                "Auto-flagging review content"
            );
            if let Err(e) = sqlx::query(
                "INSERT INTO review_reports (review_id, reporter_id, reason, status, source)
                 VALUES ($1, NULL, $2, 'pending', 'auto_flag')
                 ON CONFLICT DO NOTHING",
            )
            .bind(review.id)
            .bind(&reason_str)
            .execute(&pool)
            .await
            {
                tracing::warn!(
                    review_id = %review.id,
                    error = %e,
                    "Failed to insert auto-flag review_report (non-fatal)"
                );
            }
        }
    }

    Ok(response::success_response(review))
}

pub async fn update_review(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(review_id): Path<Uuid>,
    Json(payload): Json<UpdateReviewRequest>,
) -> Result<Json<response::ApiResponse<Review>>> {
    let existing = sqlx::query_as::<_, Review>(
        "SELECT id, user_id, game_id, rating, content, helpful_count, created_at, updated_at
         FROM reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Review not found".to_string()))?;

    if existing.user_id != user_id {
        return Err(AppError::Forbidden(
            "You can only update your own reviews".to_string(),
        ));
    }

    if let Some(rating) = payload.rating {
        if rating < 1 || rating > 5 {
            return Err(AppError::Validation(
                "Rating must be between 1 and 5".to_string(),
            ));
        }
    }

    let updated_review = sqlx::query_as::<_, Review>(
        "UPDATE reviews SET
         rating = COALESCE($1, rating),
         content = COALESCE($2, content),
         updated_at = NOW()
         WHERE id = $3
         RETURNING id, user_id, game_id, rating, content, helpful_count, created_at, updated_at",
    )
    .bind(payload.rating)
    .bind(&payload.content)
    .bind(review_id)
    .fetch_one(&pool)
    .await?;

    // ── Auto-flag on update ───────────────────────────────────────────────────
    // Only re-check if the caller actually changed the content.
    if payload.content.is_some() {
        if let Some(ref text) = updated_review.content {
            let reasons = content_flag_reasons(text);
            if !reasons.is_empty() {
                let reason_str = reasons.join(", ");
                tracing::info!(
                    review_id = %updated_review.id,
                    reasons = %reason_str,
                    "Auto-flagging updated review content"
                );
                let _ = sqlx::query(
                    "INSERT INTO review_reports (review_id, reporter_id, reason, status, source)
                     VALUES ($1, NULL, $2, 'pending', 'auto_flag')
                     ON CONFLICT DO NOTHING",
                )
                .bind(updated_review.id)
                .bind(&reason_str)
                .execute(&pool)
                .await;
            }
        }
    }

    Ok(response::success_response(updated_review))
}

pub async fn delete_review(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(review_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    let existing = sqlx::query_as::<_, Review>(
        "SELECT id, user_id, game_id, rating, content, helpful_count, created_at, updated_at
         FROM reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Review not found".to_string()))?;

    if existing.user_id != user_id {
        return Err(AppError::Forbidden(
            "You can only delete your own reviews".to_string(),
        ));
    }

    sqlx::query("DELETE FROM reviews WHERE id = $1")
        .bind(review_id)
        .execute(&pool)
        .await?;

    Ok(response::success_response(()))
}

pub async fn get_game_rating(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<GameRating>>> {
    let result = sqlx::query_as::<_, (f64, i64)>(
        "SELECT COALESCE(AVG(rating)::float, 0), COUNT(*) FROM reviews WHERE game_id = $1",
    )
    .bind(game_id)
    .fetch_one(&pool)
    .await?;

    Ok(response::success_response(GameRating {
        average: result.0,
        count: result.1,
    }))
}

// ── Helpful vote handler ──────────────────────────────────────────────────────
//
// POST /api/games/:id/reviews/:reviewId/helpful
//
// Toggles the calling user's helpful vote on a review.
// - If the user has not yet voted → insert into review_helpful (trigger increments helpful_count).
// - If the user already voted     → delete from review_helpful (trigger decrements helpful_count).
// Returns { voted: bool, helpful_count: i64 }.

pub async fn toggle_helpful(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path((_game_id, review_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<response::ApiResponse<HelpfulToggleResult>>> {
    // Verify the review exists (and optionally belongs to the game, done by the migration FK).
    let review_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM reviews WHERE id = $1)")
            .bind(review_id)
            .fetch_one(&pool)
            .await?;

    if !review_exists {
        return Err(AppError::NotFound("Review not found".to_string()));
    }

    // Check whether the user already voted.
    let already_voted = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM review_helpful WHERE review_id = $1 AND user_id = $2)",
    )
    .bind(review_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    if already_voted {
        // Remove vote (trigger decrements helpful_count).
        sqlx::query("DELETE FROM review_helpful WHERE review_id = $1 AND user_id = $2")
            .bind(review_id)
            .bind(user_id)
            .execute(&pool)
            .await?;
    } else {
        // Cast vote (trigger increments helpful_count).
        sqlx::query(
            "INSERT INTO review_helpful (review_id, user_id) VALUES ($1, $2)
             ON CONFLICT (review_id, user_id) DO NOTHING",
        )
        .bind(review_id)
        .bind(user_id)
        .execute(&pool)
        .await?;
    }

    // Return the fresh helpful_count from the reviews row.
    let helpful_count: i32 = sqlx::query_scalar("SELECT helpful_count FROM reviews WHERE id = $1")
        .bind(review_id)
        .fetch_one(&pool)
        .await?;

    Ok(response::success_response(HelpfulToggleResult {
        voted: !already_voted,
        helpful_count,
    }))
}

// ── Report handler ────────────────────────────────────────────────────────────
//
// POST /api/games/:id/reviews/:reviewId/report
//
// Files a report against a review.  One report per (review, reporter, reason)
// triple — duplicate reports are silently ignored (ON CONFLICT DO NOTHING) so
// the caller gets a stable 200 rather than a confusing error.

pub async fn report_review(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path((_game_id, review_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<ReportReviewRequest>,
) -> Result<Json<response::ApiResponse<ReviewReportResult>>> {
    if payload.reason.trim().is_empty() {
        return Err(AppError::Validation(
            "Report reason must not be empty".to_string(),
        ));
    }

    // Verify the review exists.
    let review_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM reviews WHERE id = $1)")
            .bind(review_id)
            .fetch_one(&pool)
            .await?;

    if !review_exists {
        return Err(AppError::NotFound("Review not found".to_string()));
    }

    // Insert or retrieve existing report (ON CONFLICT DO NOTHING keeps the call idempotent).
    let report = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, chrono::DateTime<chrono::Utc>)>(
        "INSERT INTO review_reports (review_id, reporter_id, reason)
         VALUES ($1, $2, $3)
         ON CONFLICT (review_id, reporter_id, reason) DO NOTHING
         RETURNING id, review_id, reporter_id, reason, created_at",
    )
    .bind(review_id)
    .bind(user_id)
    .bind(&payload.reason)
    .fetch_optional(&pool)
    .await?;

    // If the row already existed (DO NOTHING fired), fetch it.
    let (id, r_review_id, reporter_id, reason, created_at) = if let Some(row) = report {
        row
    } else {
        sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, review_id, reporter_id, reason, created_at
             FROM review_reports WHERE review_id = $1 AND reporter_id = $2 AND reason = $3",
        )
        .bind(review_id)
        .bind(user_id)
        .bind(&payload.reason)
        .fetch_one(&pool)
        .await?
    };

    Ok(response::success_response(ReviewReportResult {
        id,
        review_id: r_review_id,
        reporter_id,
        reason,
        created_at,
    }))
}

// ── Contact handler ───────────────────────────────────────────────────────────
//
// POST /api/contact
//
// Persists a Contact-page form submission to contact_messages.
// Optionally sends a notification email via EmailService; failure is logged
// but never fatal — the submission is always persisted.
//
// This handler lives here (rather than a separate contact.rs module) so that
// the router() below can mount it without requiring a new `pub mod contact;`
// declaration in mod.rs.  The contact.rs file documents this arrangement.

#[derive(Debug, Deserialize)]
pub struct ContactRequest {
    pub name: String,
    pub email: String,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ContactResult {
    pub id: Uuid,
}

pub async fn submit_contact(
    State(pool): State<PgPool>,
    Json(payload): Json<ContactRequest>,
) -> Result<Json<response::ApiResponse<ContactResult>>> {
    // Basic validation.
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("Name must not be empty".to_string()));
    }
    if payload.email.trim().is_empty() || !payload.email.contains('@') {
        return Err(AppError::Validation(
            "A valid email is required".to_string(),
        ));
    }
    if payload.subject.trim().is_empty() {
        return Err(AppError::Validation(
            "Subject must not be empty".to_string(),
        ));
    }
    if payload.message.trim().is_empty() {
        return Err(AppError::Validation(
            "Message must not be empty".to_string(),
        ));
    }

    // Persist to DB.
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO contact_messages (name, email, subject, message)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(&payload.name)
    .bind(&payload.email)
    .bind(&payload.subject)
    .bind(&payload.message)
    .fetch_one(&pool)
    .await?;

    // Optional notification email — constructed from env; failure is logged, not fatal.
    let notify_addr = std::env::var("CONTACT_NOTIFY_EMAIL").unwrap_or_else(|_| String::new());
    if !notify_addr.is_empty() {
        match crate::services::email::EmailService::from_env() {
            Ok(svc) => {
                let subject = format!("[Contact] {} — {}", payload.name, payload.subject);
                let text = format!(
                    "New contact message from {} <{}>\n\nSubject: {}\n\nMessage:\n{}",
                    payload.name, payload.email, payload.subject, payload.message
                );
                let html = format!(
                    "<p><strong>From:</strong> {} &lt;{}&gt;</p>\
                     <p><strong>Subject:</strong> {}</p>\
                     <hr/><p>{}</p>",
                    payload.name,
                    payload.email,
                    payload.subject,
                    payload.message.replace('\n', "<br/>")
                );
                if let Err(e) = svc.send_email(&notify_addr, &subject, &text, &html).await {
                    tracing::warn!(
                        contact_id = %id,
                        error = %e,
                        "Contact notification email failed (submission still saved)"
                    );
                }
            }
            Err(e) => {
                tracing::info!(
                    contact_id = %id,
                    error = %e,
                    "Email provider not configured — contact notification skipped"
                );
            }
        }
    }

    Ok(response::success_response(ContactResult { id }))
}

// ── Router ────────────────────────────────────────────────────────────────────
//
// Mount in main.rs:
//   .nest("/games", reviews::router(pool.clone()))   // adds /:id/reviews/* sub-routes
//   .route("/api/v1/contact", post(reviews::submit_contact))  // or via this router's /contact
//
// All review-write endpoints (create, update, delete, helpful, report) require
// a valid Bearer JWT; list and rating are public.
// The /contact endpoint is public (no auth required).
//
// NOTE: This router is designed to be nested under /games in main.rs so that
// the path becomes /games/:id/reviews/... — the same nesting already used by
// games::router for /:id/leaderboard etc.  The contact route is exposed at
// /contact relative to where this router is nested (typically /api/v1/contact
// when merged at the top level as shown below in the router fn).

pub fn router(pool: PgPool) -> Router {
    // ── Auth-required review routes ───────────────────────────────────────────
    let auth_review_routes = Router::new()
        .route("/:id/reviews", post(create_review))
        .route("/:id/reviews/:review_id", put(update_review))
        .route("/:id/reviews/:review_id", delete(delete_review))
        .route("/:id/reviews/:review_id/helpful", post(toggle_helpful))
        .route("/:id/reviews/:review_id/report", post(report_review))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ));

    // ── Public review + contact routes ────────────────────────────────────────
    let public_routes = Router::new()
        .route("/:id/reviews", get(list_reviews))
        .route("/:id/rating", get(get_game_rating));

    // ── Contact route (public, no auth) ──────────────────────────────────────
    // Exposed at /contact relative to the router's mount point.
    // When this router is nested at /games in the api_v1 router the full
    // path is /api/v1/games/contact — which is non-ideal.  The orchestrator
    // should additionally mount:
    //   .route("/contact", post(reviews::submit_contact).with_state(pool))
    // at the api_v1 level so the canonical path is /api/v1/contact.
    let contact_route = Router::new().route("/contact", post(submit_contact));

    Router::new()
        .merge(auth_review_routes)
        .merge(public_routes)
        .merge(contact_route)
        .with_state(pool)
}
