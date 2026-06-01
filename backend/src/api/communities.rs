// Communities API — CRUD for servers/guilds, membership, and role management.
#![allow(dead_code)]

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
use crate::services::communities as svc;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateCommunityRequest {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommunityRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ListCommunitiesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SetRoleRequest {
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct MembersResponse {
    pub members: Vec<svc::CommunityMember>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct VoiceRoomInfo {
    pub id: Uuid,
    pub channel_id: Option<Uuid>,
    pub room_token: String,
    pub max_participants: i32,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct VoiceJoinTokenResponse {
    /// The room_token the client should pass as ?room=<token> when connecting
    /// to the /ws/voice WebSocket endpoint.
    pub room_token: String,
    pub room_id: Uuid,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/communities?limit=&offset=
pub async fn list_communities(
    State(pool): State<PgPool>,
    Query(q): Query<ListCommunitiesQuery>,
) -> Result<Json<response::ApiResponse<Vec<svc::Community>>>> {
    let limit = q.limit.unwrap_or(20).min(100);
    let offset = q.offset.unwrap_or(0);
    let communities = svc::list_public_communities(&pool, limit, offset).await?;
    Ok(response::success_response(communities))
}

/// GET /api/v1/communities/me  — communities the authenticated user has joined
pub async fn list_my_communities(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::ApiResponse<Vec<svc::Community>>>> {
    let communities = svc::list_user_communities(&pool, user_id).await?;
    Ok(response::success_response(communities))
}

/// POST /api/v1/communities
pub async fn create_community(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<CreateCommunityRequest>,
) -> Result<Json<response::ApiResponse<svc::Community>>> {
    if payload.name.trim().is_empty() {
        return Err(AppError::Validation(
            "Community name is required".to_string(),
        ));
    }
    if payload.slug.trim().is_empty() {
        return Err(AppError::Validation(
            "Community slug is required".to_string(),
        ));
    }
    // Slugs: lowercase letters, numbers, hyphens only.
    if !payload
        .slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(AppError::Validation(
            "Slug may only contain lowercase letters, numbers, and hyphens".to_string(),
        ));
    }
    let is_public = payload.is_public.unwrap_or(true);
    let community = svc::create_community(
        &pool,
        user_id,
        payload.name.trim(),
        payload.slug.trim(),
        payload.description.as_deref(),
        is_public,
    )
    .await?;
    Ok(response::success_response(community))
}

/// GET /api/v1/communities/:id
pub async fn get_community(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<svc::Community>>> {
    let community = svc::get_community(&pool, id).await?;
    Ok(response::success_response(community))
}

/// PUT /api/v1/communities/:id
pub async fn update_community(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCommunityRequest>,
) -> Result<Json<response::ApiResponse<svc::Community>>> {
    let community = svc::update_community(
        &pool,
        id,
        user_id,
        payload.name.as_deref(),
        payload.description.as_deref(),
        payload.icon_url.as_deref(),
        payload.banner_url.as_deref(),
        payload.is_public,
    )
    .await?;
    Ok(response::success_response(community))
}

/// DELETE /api/v1/communities/:id
pub async fn delete_community(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    svc::delete_community(&pool, id, user_id).await?;
    Ok(response::success_response(()))
}

/// POST /api/v1/communities/:id/join
pub async fn join_community(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<svc::CommunityMember>>> {
    let member = svc::join_community(&pool, id, user_id).await?;
    Ok(response::success_response(member))
}

/// POST /api/v1/communities/:id/leave
pub async fn leave_community(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<()>>> {
    svc::leave_community(&pool, id, user_id).await?;
    Ok(response::success_response(()))
}

/// GET /api/v1/communities/:id/members
pub async fn list_members(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<MembersResponse>>> {
    let members = svc::list_members(&pool, id).await?;
    Ok(response::success_response(MembersResponse { members }))
}

/// PUT /api/v1/communities/:id/roles
pub async fn set_member_role(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SetRoleRequest>,
) -> Result<Json<response::ApiResponse<svc::CommunityMember>>> {
    let valid_roles = ["admin", "moderator", "member"];
    if !valid_roles.contains(&payload.role.as_str()) {
        return Err(AppError::Validation(
            "Role must be 'admin', 'moderator', or 'member'".to_string(),
        ));
    }
    let member = svc::set_member_role(&pool, id, user_id, payload.user_id, &payload.role).await?;
    Ok(response::success_response(member))
}

/// GET /api/v1/communities/:id/voice-rooms — list active voice rooms in a community.
/// Voice rooms are attached to voice-kind channels.
pub async fn list_voice_rooms(
    State(pool): State<PgPool>,
    Path(community_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<Vec<VoiceRoomInfo>>>> {
    let rooms = sqlx::query_as::<_, VoiceRoomInfo>(
        r#"
        SELECT vr.id, vr.channel_id, vr.room_token, vr.max_participants, vr.is_active, vr.created_at
        FROM voice_rooms vr
        JOIN channels c ON vr.channel_id = c.id
        WHERE c.community_id = $1 AND vr.is_active = true
        ORDER BY vr.created_at ASC
        "#,
    )
    .bind(community_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(response::success_response(rooms))
}

/// POST /api/v1/voice-rooms/:id/join — return a join token for the given voice room.
/// The token is already stored in the DB; this endpoint just looks it up.
pub async fn get_voice_join_token(
    State(pool): State<PgPool>,
    Extension(_user_id): Extension<Uuid>,
    Path(room_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<VoiceJoinTokenResponse>>> {
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, room_token FROM voice_rooms WHERE id = $1 AND is_active = true",
    )
    .bind(room_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Voice room not found or inactive".to_string()))?;

    Ok(response::success_response(VoiceJoinTokenResponse {
        room_id: row.0,
        room_token: row.1,
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool) -> Router {
    let auth_routes = Router::new()
        .route("/me", get(list_my_communities))
        .route("/", post(create_community))
        .route("/:id", put(update_community))
        .route("/:id", delete(delete_community))
        .route("/:id/join", post(join_community))
        .route("/:id/leave", post(leave_community))
        .route("/:id/roles", put(set_member_role))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ));

    let public_routes = Router::new()
        .route("/", get(list_communities))
        .route("/:id", get(get_community))
        .route("/:id/members", get(list_members))
        // Voice rooms for a community — public list; join requires auth (separate router)
        .route("/:id/voice-rooms", get(list_voice_rooms));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .with_state(pool)
}

/// Voice-rooms top-level router — mounted at /voice-rooms in main.rs.
/// Exposes POST /voice-rooms/:id/join (auth-required) which returns the room_token
/// the client needs to connect to /ws/voice?token=<jwt>&room=<room_token>.
pub fn voice_rooms_router(pool: PgPool) -> Router {
    Router::new()
        .route(
            "/:id/join",
            post(get_voice_join_token).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
