// Streaming API — go-live, stop, list-live, get stream, viewer count, and
// HLS manifest proxy.
//
// ┌───────────────────────────────────────────────────────────────────────────┐
// │  Media-server dependency (see services/streaming.rs for full detail)     │
// │                                                                           │
// │  HLS manifest proxy (`GET /streams/:id/hls`):                            │
// │    The backend fetches the .m3u8 from the configured media server and    │
// │    returns it to the viewer.  This keeps the media-server address         │
// │    internal and lets Magnetite add auth checks before proxying.           │
// │    Set MEDIA_SERVER_BASE_URL env var (e.g. http://mediamtx:8888).        │
// │    If not set, the endpoint returns the hls_url stored on the record     │
// │    as a 302 redirect instead of proxying.                                │
// │                                                                           │
// │  WebRTC ingest:                                                           │
// │    Broadcasters using browser-based capture (getDisplayMedia) signal     │
// │    via the existing /ws/voice WebSocket (ws/voice.rs).  The stream_id   │
// │    is passed as a query parameter so the voice room can be correlated    │
// │    with the stream record.                                                │
// │                                                                           │
// │  RTMP egress (Twitch/YouTube):                                           │
// │    Store rtmp_target + stream_key in the stream record.  Configure      │
// │    MediaMTX `runOnPublish` to forward to rtmp_target.  The backend       │
// │    never touches the media bytes.                                         │
// └───────────────────────────────────────────────────────────────────────────┘
#![allow(dead_code)]

use axum::{
    extract::{Extension, Path, Query, State},
    http::{header, StatusCode},
    middleware::from_fn_with_state,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::{AppError, Result};
use crate::services::streaming as svc;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateStreamRequest {
    pub title: String,
    pub community_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
    /// External RTMP ingest URL for restreaming (e.g. Twitch/YouTube).
    pub rtmp_target: Option<String>,
    /// Stream key for the external RTMP target.
    pub stream_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GoLiveRequest {
    /// Optional: the HLS URL at which viewers can watch.  Provided by the
    /// broadcaster after the media server confirms ingest.  If omitted the
    /// backend constructs one from MEDIA_SERVER_BASE_URL + the ingest_key.
    pub hls_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListLiveQuery {
    pub community_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ViewerCountResponse {
    pub stream_id: Uuid,
    pub viewer_count: i32,
}

/// Response returned by the /watch endpoint so clients know where to find the HLS stream.
#[derive(Debug, Serialize)]
pub struct WatchInfoResponse {
    pub stream_id: Uuid,
    pub title: String,
    pub status: String,
    pub viewer_count: i32,
    pub streamer_id: Uuid,
    /// HLS playlist URL the client should open in a video player.
    pub hls_url: Option<String>,
    /// Convenience watch URL (same as the HLS endpoint on the backend).
    pub watch_url: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/streams — create a stream record; returns ingest credentials.
pub async fn create_stream(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Json(body): Json<CreateStreamRequest>,
) -> Result<Json<response::ApiResponse<svc::StreamOwnerView>>> {
    let stream = svc::create_stream(
        &pool,
        user_id,
        &body.title,
        body.community_id,
        body.channel_id,
        body.game_id,
        body.rtmp_target.as_deref(),
        body.stream_key.as_deref(),
    )
    .await?;
    Ok(response::success_response(stream))
}

/// POST /api/v1/streams/:id/go-live — transition stream to 'live'.
pub async fn go_live(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
    Json(body): Json<GoLiveRequest>,
) -> Result<Json<response::ApiResponse<svc::StreamOwnerView>>> {
    // If no hls_url supplied, try to derive one from MEDIA_SERVER_BASE_URL.
    let derived_hls = derive_hls_url(&pool, id, body.hls_url.as_deref()).await;
    let stream = svc::go_live(&pool, id, user_id, derived_hls.as_deref()).await?;
    Ok(response::success_response(stream))
}

/// POST /api/v1/streams/:id/stop — stop a live stream.
pub async fn stop_stream(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<svc::StreamPublic>>> {
    let stream = svc::stop_stream(&pool, id, user_id).await?;
    Ok(response::success_response(stream))
}

/// GET /api/v1/streams — list live streams (public; filterable by community/game).
pub async fn list_live_streams(
    State(pool): State<PgPool>,
    Query(q): Query<ListLiveQuery>,
) -> Result<Json<response::ApiResponse<Vec<svc::StreamPublic>>>> {
    let limit = q.limit.unwrap_or(20).min(100);
    let offset = q.offset.unwrap_or(0);
    let streams = svc::list_live_streams(&pool, q.community_id, q.game_id, limit, offset).await?;
    Ok(response::success_response(streams))
}

/// GET /api/v1/streams/:id — public stream detail.
pub async fn get_stream(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<svc::StreamPublic>>> {
    let stream = svc::get_stream(&pool, id).await?;
    Ok(response::success_response(stream))
}

/// GET /api/v1/streams/:id/me — owner view with ingest credentials.
pub async fn get_my_stream(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<svc::StreamOwnerView>>> {
    let stream = svc::get_stream_for_owner(&pool, id, user_id).await?;
    Ok(response::success_response(stream))
}

/// POST /api/v1/streams/:id/join — increment viewer count.
/// Call when a viewer starts watching; decrement on leave.
pub async fn join_stream(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<ViewerCountResponse>>> {
    // Verify the stream exists and is live.
    let stream = svc::get_stream(&pool, id).await?;
    if stream.status != "live" {
        return Err(AppError::BadRequest("Stream is not live".to_string()));
    }
    // Do not count the broadcaster as a viewer.
    if stream.streamer_id == user_id {
        return Ok(response::success_response(ViewerCountResponse {
            stream_id: id,
            viewer_count: stream.viewer_count,
        }));
    }
    let count = svc::increment_viewer_count(&pool, id).await?;
    Ok(response::success_response(ViewerCountResponse {
        stream_id: id,
        viewer_count: count,
    }))
}

/// POST /api/v1/streams/:id/leave — decrement viewer count.
pub async fn leave_stream(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<ViewerCountResponse>>> {
    let count = svc::decrement_viewer_count(&pool, id).await?;
    Ok(response::success_response(ViewerCountResponse {
        stream_id: id,
        viewer_count: count,
    }))
}

/// GET /api/v1/streams/:id/hls — proxy or redirect to the HLS .m3u8 manifest.
///
/// If MEDIA_SERVER_BASE_URL is set and the stream record has no hls_url yet,
/// the backend constructs the media-server URL and fetches the manifest to
/// proxy it (keeping the media server internal).  Otherwise it uses hls_url
/// directly.  If neither is available and the stream is live, returns 503.
pub async fn hls_manifest(State(pool): State<PgPool>, Path(id): Path<Uuid>) -> Response {
    let stream = match svc::get_stream(&pool, id).await {
        Ok(s) => s,
        Err(AppError::NotFound(msg)) => {
            return (StatusCode::NOT_FOUND, msg).into_response();
        }
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    if stream.status != "live" {
        return (StatusCode::NOT_FOUND, "Stream is not live").into_response();
    }

    // Determine HLS URL — prefer stored hls_url, fall back to media server base.
    let hls_url = match stream.hls_url.as_deref() {
        Some(url) => url.to_string(),
        None => {
            let base = std::env::var("MEDIA_SERVER_BASE_URL").unwrap_or_default();
            if base.is_empty() {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Media server not configured — stream not yet playable",
                )
                    .into_response();
            }
            // MediaMTX path convention: /live/<ingest_key>/index.m3u8
            // The ingest_key is NOT returned by get_stream (public view).
            // We redirect to a URL that the media server owns directly.
            format!("{base}/live/{id}/index.m3u8")
        }
    };

    // Attempt to proxy the manifest content so the media server stays internal.
    if let Ok(resp) = reqwest::get(&hls_url).await {
        if resp.status().is_success() {
            if let Ok(body) = resp.text().await {
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
                    body,
                )
                    .into_response();
            }
        }
    }

    // Fall back to a 302 redirect so the client fetches the manifest directly.
    (StatusCode::FOUND, [(header::LOCATION, hls_url.as_str())]).into_response()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// If the caller supplied an explicit hls_url, use it.  Otherwise try to
/// derive one from MEDIA_SERVER_BASE_URL + the ingest_key on the stream record.
/// Returns None if neither is available (hls_url stays NULL until the media
/// server webhook fires).
async fn derive_hls_url(pool: &PgPool, stream_id: Uuid, provided: Option<&str>) -> Option<String> {
    if let Some(url) = provided {
        return Some(url.to_string());
    }
    let base = std::env::var("MEDIA_SERVER_BASE_URL").ok()?;
    if base.is_empty() {
        return None;
    }
    // We need the ingest_key; fetch the owner row without an owner check —
    // this is called from go_live which already verified ownership.
    let key: Option<String> = sqlx::query_scalar("SELECT ingest_key FROM streams WHERE id = $1")
        .bind(stream_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    key.map(|k| format!("{base}/live/{k}/index.m3u8"))
}

/// GET /api/v1/streams/:id/watch — returns watch info including the HLS URL.
/// This is the endpoint the frontend calls before opening a stream player.
pub async fn watch_stream(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<WatchInfoResponse>>> {
    let stream = svc::get_stream(&pool, id).await?;
    let api_base = std::env::var("API_BASE_URL").unwrap_or_default();
    let watch_url = format!("{}/api/v1/streams/{}/hls", api_base, id);
    Ok(response::success_response(WatchInfoResponse {
        stream_id: stream.id,
        title: stream.title.clone(),
        status: stream.status.clone(),
        viewer_count: stream.viewer_count,
        streamer_id: stream.streamer_id,
        hls_url: stream.hls_url.clone(),
        watch_url,
    }))
}

/// GET /api/v1/communities/:community_id/streams — list live streams in a community.
/// Delegates to list_live_streams with community_id filter.
pub async fn list_community_streams(
    State(pool): State<PgPool>,
    Path(community_id): Path<Uuid>,
    Query(q): Query<ListLiveQuery>,
) -> Result<Json<response::ApiResponse<Vec<svc::StreamPublic>>>> {
    let limit = q.limit.unwrap_or(20).min(100);
    let offset = q.offset.unwrap_or(0);
    let streams =
        svc::list_live_streams(&pool, Some(community_id), q.game_id, limit, offset).await?;
    Ok(response::success_response(streams))
}

/// POST /api/v1/communities/:community_id/streams — go live scoped to a community.
pub async fn create_community_stream(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(community_id): Path<Uuid>,
    Json(mut body): Json<CreateStreamRequest>,
) -> Result<Json<response::ApiResponse<svc::StreamOwnerView>>> {
    // Force the community_id from the path (override any body-supplied value).
    body.community_id = Some(community_id);
    let stream = svc::create_stream(
        &pool,
        user_id,
        &body.title,
        body.community_id,
        body.channel_id,
        body.game_id,
        body.rtmp_target.as_deref(),
        body.stream_key.as_deref(),
    )
    .await?;
    Ok(response::success_response(stream))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(pool: PgPool) -> Router {
    // Auth-required routes (broadcaster actions + viewer join/leave).
    let auth_routes = Router::new()
        .route("/", post(create_stream))
        .route("/:id/go-live", post(go_live))
        .route("/:id/stop", post(stop_stream))
        .route("/:id/me", get(get_my_stream))
        .route("/:id/join", post(join_stream))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ));

    // Public routes — no auth needed.
    let public_routes = Router::new()
        .route("/", get(list_live_streams))
        .route("/:id", get(get_stream))
        .route("/:id/hls", get(hls_manifest))
        // /watch returns JSON watch-info (hls_url + metadata)
        .route("/:id/watch", get(watch_stream))
        .route("/:id/leave", post(leave_stream));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .with_state(pool)
}

/// Community-scoped streams sub-router — mounted at /communities/:community_id/streams
/// in main.rs so the full paths are GET/POST /api/v1/communities/:id/streams.
pub fn community_streams_router(pool: PgPool) -> Router {
    let auth_routes = Router::new()
        .route("/", post(create_community_stream))
        .layer(from_fn_with_state(
            pool.clone(),
            middleware::auth_middleware,
        ));

    let public_routes = Router::new().route("/", get(list_community_streams));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .with_state(pool)
}
