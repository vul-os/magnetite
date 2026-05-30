// Marketplace API — store CRUD, item CRUD, purchase, entitlements, revenue.
// Developer-facing writes require auth; buyer-facing reads are public (store/items).

use axum::{
    extract::{Extension, Path, Query, State},
    middleware::from_fn_with_state,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::middleware;
use crate::api::response;
use crate::error::Result;
use crate::services::marketplace::{
    CreateItemRequest, CreateStoreRequest, MarketplaceService, StorePurchase, UpdateItemRequest,
    UpdateStoreRequest,
};

// ─── Query params ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ItemsQuery {
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PurchasesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PurchaseRequest {
    pub idempotency_key: Option<String>,
}

// ─── Store handlers ───────────────────────────────────────────────────────────

/// GET /marketplace/stores/:game_id — public store for a game.
pub async fn get_store_for_game(
    State(pool): State<PgPool>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let store = svc.get_store_by_game(game_id).await?;
    let val = serde_json::to_value(store).unwrap_or(serde_json::Value::Null);
    Ok(response::success_response(val))
}

/// POST /marketplace/stores — developer creates a store for a game.
pub async fn create_store(
    State(pool): State<PgPool>,
    Extension(developer_id): Extension<Uuid>,
    Path(game_id): Path<Uuid>,
    Json(payload): Json<CreateStoreRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let store = svc.create_store(developer_id, game_id, payload).await?;
    let val = serde_json::to_value(store).unwrap_or(serde_json::Value::Null);
    Ok(response::success_response(val))
}

/// PUT /marketplace/stores/:store_id — developer updates their store.
pub async fn update_store(
    State(pool): State<PgPool>,
    Extension(developer_id): Extension<Uuid>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<UpdateStoreRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let store = svc.update_store(developer_id, store_id, payload).await?;
    let val = serde_json::to_value(store).unwrap_or(serde_json::Value::Null);
    Ok(response::success_response(val))
}

/// GET /marketplace/my-stores — developer lists their own stores.
pub async fn list_my_stores(
    State(pool): State<PgPool>,
    Extension(developer_id): Extension<Uuid>,
) -> Result<Json<response::PaginatedResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let stores = svc.list_developer_stores(developer_id).await?;
    let total = stores.len() as u64;
    let vals: Vec<serde_json::Value> = stores
        .into_iter()
        .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
        .collect();
    Ok(response::paginated(vals, 1, 100, total))
}

// ─── Item handlers ────────────────────────────────────────────────────────────

/// GET /marketplace/stores/:store_id/items — list active items in a store.
pub async fn list_items(
    State(pool): State<PgPool>,
    Path(store_id): Path<Uuid>,
    Query(q): Query<ItemsQuery>,
) -> Result<Json<response::PaginatedResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let items = svc.list_items(store_id, q.kind.as_deref()).await?;
    let total = items.len() as u64;
    let vals: Vec<serde_json::Value> = items
        .into_iter()
        .map(|i| serde_json::to_value(i).unwrap_or(serde_json::Value::Null))
        .collect();
    Ok(response::paginated(vals, 1, 200, total))
}

/// GET /marketplace/items/:item_id — single item details.
pub async fn get_item(
    State(pool): State<PgPool>,
    Path(item_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let item = svc.get_item(item_id).await?;
    let val = serde_json::to_value(item).unwrap_or(serde_json::Value::Null);
    Ok(response::success_response(val))
}

/// POST /marketplace/stores/:store_id/items — developer adds an item.
pub async fn create_item(
    State(pool): State<PgPool>,
    Extension(developer_id): Extension<Uuid>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<CreateItemRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let item = svc.create_item(developer_id, store_id, payload).await?;
    let val = serde_json::to_value(item).unwrap_or(serde_json::Value::Null);
    Ok(response::success_response(val))
}

/// PUT /marketplace/items/:item_id — developer updates an item.
pub async fn update_item(
    State(pool): State<PgPool>,
    Extension(developer_id): Extension<Uuid>,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<UpdateItemRequest>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let item = svc.update_item(developer_id, item_id, payload).await?;
    let val = serde_json::to_value(item).unwrap_or(serde_json::Value::Null);
    Ok(response::success_response(val))
}

// ─── Purchase handlers ────────────────────────────────────────────────────────

/// POST /marketplace/items/:item_id/purchase — buy an item.
pub async fn purchase_item(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<PurchaseRequest>,
) -> Result<Json<response::ApiResponse<StorePurchase>>> {
    let svc = MarketplaceService::new(pool);
    let purchase = svc
        .purchase(user_id, item_id, payload.idempotency_key.as_deref())
        .await?;
    Ok(response::success_response(purchase))
}

/// GET /marketplace/purchases — caller's purchase history.
pub async fn list_my_purchases(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Query(q): Query<PurchasesQuery>,
) -> Result<Json<response::PaginatedResponse<StorePurchase>>> {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0).max(0);

    let svc = MarketplaceService::new(pool);
    let purchases = svc.user_purchases(user_id, limit, offset).await?;
    let total = purchases.len() as u64 + offset as u64;

    let page = if limit > 0 {
        (offset / limit + 1) as u32
    } else {
        1
    };
    Ok(response::paginated(purchases, page, limit as u32, total))
}

// ─── Entitlement handlers ─────────────────────────────────────────────────────

/// GET /marketplace/entitlements — caller's owned items.
pub async fn list_my_entitlements(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<response::PaginatedResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let ents = svc.list_entitlements(user_id).await?;
    let total = ents.len() as u64;
    let vals: Vec<serde_json::Value> = ents
        .into_iter()
        .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
        .collect();
    Ok(response::paginated(vals, 1, 200, total))
}

/// GET /marketplace/entitlements/:item_id/check — does caller own this item?
pub async fn check_entitlement(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
    Path(item_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let owned = svc.has_entitlement(user_id, item_id).await?;
    Ok(response::success_response(serde_json::json!({
        "item_id": item_id,
        "owned": owned
    })))
}

// ─── Revenue handler (developer) ──────────────────────────────────────────────

/// GET /marketplace/stores/:store_id/revenue — revenue summary for dev.
pub async fn store_revenue(
    State(pool): State<PgPool>,
    Extension(developer_id): Extension<Uuid>,
    Path(store_id): Path<Uuid>,
) -> Result<Json<response::ApiResponse<serde_json::Value>>> {
    let svc = MarketplaceService::new(pool);
    let rev = svc.store_revenue(developer_id, store_id).await?;
    Ok(response::success_response(rev))
}

// ─── Router ───────────────────────────────────────────────────────────────────

pub fn router(pool: PgPool) -> Router {
    Router::new()
        // ── Public ──────────────────────────────────────────────────────────
        .route("/stores/:game_id", get(get_store_for_game))
        .route("/stores/:store_id/items", get(list_items))
        .route("/items/:item_id", get(get_item))
        // ── Authenticated: developer store management ─────────────────────
        .route(
            "/games/:game_id/store",
            post(create_store).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/stores/:store_id",
            put(update_store).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/my-stores",
            get(list_my_stores).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/stores/:store_id/items",
            post(create_item).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/items/:item_id",
            put(update_item).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/stores/:store_id/revenue",
            get(store_revenue).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        // ── Authenticated: buyer ─────────────────────────────────────────
        .route(
            "/items/:item_id/purchase",
            post(purchase_item).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/purchases",
            get(list_my_purchases).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/entitlements",
            get(list_my_entitlements).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/entitlements/:item_id/check",
            get(check_entitlement).layer(from_fn_with_state(
                pool.clone(),
                middleware::auth_middleware,
            )),
        )
        .with_state(pool)
}
