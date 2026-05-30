// Marketplace service — dev store CRUD, item CRUD, purchase flow, entitlements.
// Revenue-share reuses the 70/30 split defined in payout service constants.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::services::points::PointsService;

// ─── Revenue-share constants (mirrors payout.rs 70/30 split) ─────────────────

fn developer_share_pct() -> Decimal {
    Decimal::new(70, 2) // 0.70
}

fn platform_fee_pct() -> Decimal {
    Decimal::new(30, 2) // 0.30
}

// ─── Domain types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DevStore {
    pub id: Uuid,
    pub game_id: Uuid,
    pub developer_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StoreItem {
    pub id: Uuid,
    pub store_id: Uuid,
    pub game_id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
    pub kind: String,
    pub active: bool,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StorePurchase {
    pub id: Uuid,
    pub user_id: Uuid,
    pub item_id: Uuid,
    pub store_id: Uuid,
    pub game_id: Uuid,
    pub price_paid: Decimal,
    pub currency: String,
    pub developer_share: Option<Decimal>,
    pub platform_fee: Option<Decimal>,
    pub status: String,
    pub idempotency_key: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Entitlement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub item_id: Uuid,
    pub purchase_id: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

// ─── Request structs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct CreateStoreRequest {
    pub name: String,
    pub description: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateStoreRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateItemRequest {
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
    pub kind: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateItemRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<Decimal>,
    pub active: Option<bool>,
    pub metadata: Option<Value>,
}

// ─── Service ──────────────────────────────────────────────────────────────────

pub struct MarketplaceService {
    pool: PgPool,
}

impl MarketplaceService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Store CRUD ───────────────────────────────────────────────────────────

    /// Create a store for a game. Only one store per game (enforced by DB unique constraint).
    pub async fn create_store(
        &self,
        developer_id: Uuid,
        game_id: Uuid,
        req: CreateStoreRequest,
    ) -> Result<DevStore> {
        // Verify the developer owns the game.
        self.assert_game_owner(developer_id, game_id).await?;

        let store = sqlx::query_as::<_, DevStore>(
            r#"
            INSERT INTO dev_stores (id, game_id, developer_id, name, description, active, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, true, $6, NOW(), NOW())
            RETURNING id, game_id, developer_id, name, description, active, metadata, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(game_id)
        .bind(developer_id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                AppError::Validation("A store already exists for this game".to_string())
            } else {
                AppError::Database(e.to_string())
            }
        })?;

        Ok(store)
    }

    pub async fn get_store_by_game(&self, game_id: Uuid) -> Result<Option<DevStore>> {
        let store = sqlx::query_as::<_, DevStore>(
            "SELECT id, game_id, developer_id, name, description, active, metadata, created_at, updated_at
             FROM dev_stores WHERE game_id = $1",
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(store)
    }

    pub async fn get_store(&self, store_id: Uuid) -> Result<DevStore> {
        sqlx::query_as::<_, DevStore>(
            "SELECT id, game_id, developer_id, name, description, active, metadata, created_at, updated_at
             FROM dev_stores WHERE id = $1",
        )
        .bind(store_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Store not found".to_string()))
    }

    pub async fn update_store(
        &self,
        developer_id: Uuid,
        store_id: Uuid,
        req: UpdateStoreRequest,
    ) -> Result<DevStore> {
        let store = self.get_store(store_id).await?;
        if store.developer_id != developer_id {
            return Err(AppError::Forbidden(
                "Not the owner of this store".to_string(),
            ));
        }

        let name = req.name.unwrap_or(store.name);
        let description = req.description.or(store.description);
        let active = req.active.unwrap_or(store.active);
        let metadata = req.metadata.or(store.metadata);

        let updated = sqlx::query_as::<_, DevStore>(
            r#"
            UPDATE dev_stores
            SET name = $1, description = $2, active = $3, metadata = $4, updated_at = NOW()
            WHERE id = $5
            RETURNING id, game_id, developer_id, name, description, active, metadata, created_at, updated_at
            "#,
        )
        .bind(&name)
        .bind(&description)
        .bind(active)
        .bind(&metadata)
        .bind(store_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    pub async fn list_developer_stores(&self, developer_id: Uuid) -> Result<Vec<DevStore>> {
        let stores = sqlx::query_as::<_, DevStore>(
            "SELECT id, game_id, developer_id, name, description, active, metadata, created_at, updated_at
             FROM dev_stores WHERE developer_id = $1 ORDER BY created_at DESC",
        )
        .bind(developer_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(stores)
    }

    // ── Item CRUD ────────────────────────────────────────────────────────────

    fn validate_item_kind(kind: &str) -> Result<()> {
        match kind {
            "cosmetic" | "item" | "dlc" | "pass" => Ok(()),
            _ => Err(AppError::Validation(format!(
                "Invalid item kind '{kind}'. Must be one of: cosmetic, item, dlc, pass"
            ))),
        }
    }

    fn validate_item_currency(currency: &str) -> Result<()> {
        match currency {
            "USDC" | "points" => Ok(()),
            _ => Err(AppError::Validation(format!(
                "Invalid currency '{currency}'. Must be 'USDC' or 'points'"
            ))),
        }
    }

    pub async fn create_item(
        &self,
        developer_id: Uuid,
        store_id: Uuid,
        req: CreateItemRequest,
    ) -> Result<StoreItem> {
        Self::validate_item_kind(&req.kind)?;
        Self::validate_item_currency(&req.currency)?;

        if req.price < Decimal::ZERO {
            return Err(AppError::Validation(
                "Price must be non-negative".to_string(),
            ));
        }

        let store = self.get_store(store_id).await?;
        if store.developer_id != developer_id {
            return Err(AppError::Forbidden(
                "Not the owner of this store".to_string(),
            ));
        }

        let item = sqlx::query_as::<_, StoreItem>(
            r#"
            INSERT INTO store_items
                (id, store_id, game_id, sku, name, description, price, currency, kind, active, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10, NOW(), NOW())
            RETURNING id, store_id, game_id, sku, name, description, price, currency, kind, active, metadata, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(store_id)
        .bind(store.game_id)
        .bind(&req.sku)
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.price)
        .bind(&req.currency)
        .bind(&req.kind)
        .bind(&req.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                AppError::Validation(format!("SKU '{}' already exists in this store", req.sku))
            } else {
                AppError::Database(e.to_string())
            }
        })?;

        Ok(item)
    }

    pub async fn list_items(
        &self,
        store_id: Uuid,
        kind_filter: Option<&str>,
    ) -> Result<Vec<StoreItem>> {
        let items =
            match kind_filter {
                Some(k) => sqlx::query_as::<_, StoreItem>(
                    "SELECT id, store_id, game_id, sku, name, description, price, currency, kind,
                        active, metadata, created_at, updated_at
                 FROM store_items WHERE store_id = $1 AND kind = $2 AND active = true
                 ORDER BY created_at DESC",
                )
                .bind(store_id)
                .bind(k)
                .fetch_all(&self.pool)
                .await?,

                None => sqlx::query_as::<_, StoreItem>(
                    "SELECT id, store_id, game_id, sku, name, description, price, currency, kind,
                        active, metadata, created_at, updated_at
                 FROM store_items WHERE store_id = $1 AND active = true
                 ORDER BY created_at DESC",
                )
                .bind(store_id)
                .fetch_all(&self.pool)
                .await?,
            };

        Ok(items)
    }

    pub async fn get_item(&self, item_id: Uuid) -> Result<StoreItem> {
        sqlx::query_as::<_, StoreItem>(
            "SELECT id, store_id, game_id, sku, name, description, price, currency, kind,
                    active, metadata, created_at, updated_at
             FROM store_items WHERE id = $1",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Item not found".to_string()))
    }

    pub async fn update_item(
        &self,
        developer_id: Uuid,
        item_id: Uuid,
        req: UpdateItemRequest,
    ) -> Result<StoreItem> {
        let item = self.get_item(item_id).await?;
        let store = self.get_store(item.store_id).await?;

        if store.developer_id != developer_id {
            return Err(AppError::Forbidden(
                "Not the owner of this item's store".to_string(),
            ));
        }

        let name = req.name.unwrap_or(item.name);
        let description = req.description.or(item.description);
        let price = req.price.unwrap_or(item.price);
        let active = req.active.unwrap_or(item.active);
        let metadata = req.metadata.or(item.metadata);

        let updated = sqlx::query_as::<_, StoreItem>(
            r#"
            UPDATE store_items
            SET name = $1, description = $2, price = $3, active = $4, metadata = $5, updated_at = NOW()
            WHERE id = $6
            RETURNING id, store_id, game_id, sku, name, description, price, currency, kind,
                      active, metadata, created_at, updated_at
            "#,
        )
        .bind(&name)
        .bind(&description)
        .bind(price)
        .bind(active)
        .bind(&metadata)
        .bind(item_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    // ── Purchase flow ─────────────────────────────────────────────────────────

    /// Purchase an item.
    ///
    /// - USDC items: debit wallet_balances, record revenue-share amounts.
    /// - Points items: call PointsService::spend.
    /// - Creates an entitlement on success.
    pub async fn purchase(
        &self,
        user_id: Uuid,
        item_id: Uuid,
        idempotency_key: Option<&str>,
    ) -> Result<StorePurchase> {
        // Idempotency check
        if let Some(key) = idempotency_key {
            if let Some(existing) = self.find_purchase_by_idempotency(key).await? {
                return Ok(existing);
            }
        }

        // Already owns the item?
        if self.has_entitlement(user_id, item_id).await? {
            return Err(AppError::Validation(
                "You already own this item".to_string(),
            ));
        }

        let item = self.get_item(item_id).await?;
        if !item.active {
            return Err(AppError::Validation("Item is not available".to_string()));
        }

        let store = self.get_store(item.store_id).await?;
        if !store.active {
            return Err(AppError::Validation("Store is not active".to_string()));
        }

        let purchase = match item.currency.as_str() {
            "USDC" => {
                self.purchase_usdc(user_id, &item, &store, idempotency_key)
                    .await?
            }
            "points" => {
                self.purchase_points(user_id, &item, &store, idempotency_key)
                    .await?
            }
            other => {
                return Err(AppError::Validation(format!("Unknown currency '{other}'")));
            }
        };

        // Grant entitlement
        sqlx::query(
            r#"
            INSERT INTO entitlements (id, user_id, item_id, purchase_id, granted_at, revoked)
            VALUES ($1, $2, $3, $4, NOW(), false)
            ON CONFLICT (user_id, item_id) DO NOTHING
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(item_id)
        .bind(purchase.id)
        .execute(&self.pool)
        .await?;

        Ok(purchase)
    }

    async fn purchase_usdc(
        &self,
        user_id: Uuid,
        item: &StoreItem,
        store: &DevStore,
        idempotency_key: Option<&str>,
    ) -> Result<StorePurchase> {
        let price = item.price;
        // developer_share_pct() returns 0.70 (the fractional form), so multiply directly.
        let developer_share = price * developer_share_pct();
        let platform_fee = price * platform_fee_pct();

        let mut tx = self.pool.begin().await?;

        // Lock buyer wallet
        let balance: Decimal = sqlx::query_as::<_, (Decimal,)>(
            "SELECT balance FROM wallet_balances WHERE user_id = $1 AND currency = 'USDC' FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.0)
        .unwrap_or(Decimal::ZERO);

        if balance < price {
            return Err(AppError::InsufficientFunds(format!(
                "Insufficient USDC balance. Have {balance}, need {price}"
            )));
        }

        // Debit buyer
        sqlx::query(
            "UPDATE wallet_balances SET balance = balance - $1, updated_at = NOW()
             WHERE user_id = $2 AND currency = 'USDC'",
        )
        .bind(price)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        // Credit developer balance (mirrors payout service)
        sqlx::query(
            r#"
            INSERT INTO developer_balances (user_id, balance, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (user_id) DO UPDATE
                SET balance = developer_balances.balance + $2, updated_at = NOW()
            "#,
        )
        .bind(store.developer_id)
        .bind(developer_share)
        .execute(&mut *tx)
        .await?;

        let purchase_id = Uuid::new_v4();
        let purchase = sqlx::query_as::<_, StorePurchase>(
            r#"
            INSERT INTO store_purchases
                (id, user_id, item_id, store_id, game_id, price_paid, currency,
                 developer_share, platform_fee, status, idempotency_key, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'USDC', $7, $8, 'completed', $9, NOW())
            RETURNING id, user_id, item_id, store_id, game_id, price_paid, currency,
                      developer_share, platform_fee, status, idempotency_key, metadata, created_at
            "#,
        )
        .bind(purchase_id)
        .bind(user_id)
        .bind(item.id)
        .bind(item.store_id)
        .bind(item.game_id)
        .bind(price)
        .bind(developer_share)
        .bind(platform_fee)
        .bind(idempotency_key)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(purchase)
    }

    async fn purchase_points(
        &self,
        user_id: Uuid,
        item: &StoreItem,
        _store: &DevStore,
        idempotency_key: Option<&str>,
    ) -> Result<StorePurchase> {
        let cost_pts = item.price.try_into().unwrap_or(i64::MAX);

        let ps = PointsService::new(self.pool.clone());
        ps.spend(
            user_id,
            cost_pts,
            "store_purchase",
            Some(item.game_id),
            Some(serde_json::json!({ "item_id": item.id, "sku": item.sku })),
        )
        .await?;

        let purchase = sqlx::query_as::<_, StorePurchase>(
            r#"
            INSERT INTO store_purchases
                (id, user_id, item_id, store_id, game_id, price_paid, currency,
                 developer_share, platform_fee, status, idempotency_key, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'points', NULL, NULL, 'completed', $7, NOW())
            RETURNING id, user_id, item_id, store_id, game_id, price_paid, currency,
                      developer_share, platform_fee, status, idempotency_key, metadata, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(item.id)
        .bind(item.store_id)
        .bind(item.game_id)
        .bind(item.price)
        .bind(idempotency_key)
        .fetch_one(&self.pool)
        .await?;

        Ok(purchase)
    }

    // ── Entitlements ──────────────────────────────────────────────────────────

    pub async fn has_entitlement(&self, user_id: Uuid, item_id: Uuid) -> Result<bool> {
        let row = sqlx::query_as::<_, (bool,)>(
            "SELECT EXISTS(
                SELECT 1 FROM entitlements
                WHERE user_id = $1 AND item_id = $2 AND revoked = false
                  AND (expires_at IS NULL OR expires_at > NOW())
             )",
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn list_entitlements(&self, user_id: Uuid) -> Result<Vec<Entitlement>> {
        let ents = sqlx::query_as::<_, Entitlement>(
            "SELECT id, user_id, item_id, purchase_id, granted_at, expires_at, revoked
             FROM entitlements
             WHERE user_id = $1 AND revoked = false
               AND (expires_at IS NULL OR expires_at > NOW())
             ORDER BY granted_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(ents)
    }

    // ── Purchase history ──────────────────────────────────────────────────────

    pub async fn user_purchases(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<StorePurchase>> {
        let purchases = sqlx::query_as::<_, StorePurchase>(
            r#"
            SELECT id, user_id, item_id, store_id, game_id, price_paid, currency,
                   developer_share, platform_fee, status, idempotency_key, metadata, created_at
            FROM store_purchases
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
        Ok(purchases)
    }

    /// Revenue summary for a store — total sales in USDC and points count.
    pub async fn store_revenue(
        &self,
        developer_id: Uuid,
        store_id: Uuid,
    ) -> Result<serde_json::Value> {
        let store = self.get_store(store_id).await?;
        if store.developer_id != developer_id {
            return Err(AppError::Forbidden(
                "Not the owner of this store".to_string(),
            ));
        }

        let usdc_revenue: Decimal = sqlx::query_as::<_, (Decimal,)>(
            "SELECT COALESCE(SUM(developer_share), 0)
             FROM store_purchases
             WHERE store_id = $1 AND currency = 'USDC' AND status = 'completed'",
        )
        .bind(store_id)
        .fetch_one(&self.pool)
        .await?
        .0;

        let points_sales: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*)
             FROM store_purchases
             WHERE store_id = $1 AND currency = 'points' AND status = 'completed'",
        )
        .bind(store_id)
        .fetch_one(&self.pool)
        .await?
        .0;

        let total_sales: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM store_purchases WHERE store_id = $1 AND status = 'completed'",
        )
        .bind(store_id)
        .fetch_one(&self.pool)
        .await?
        .0;

        Ok(serde_json::json!({
            "store_id": store_id,
            "developer_share_usdc": usdc_revenue,
            "points_sales_count": points_sales,
            "total_completed_purchases": total_sales,
        }))
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    async fn assert_game_owner(&self, developer_id: Uuid, game_id: Uuid) -> Result<()> {
        let owned: bool = sqlx::query_as::<_, (bool,)>(
            "SELECT EXISTS(SELECT 1 FROM games WHERE id = $1 AND developer_id = $2)",
        )
        .bind(game_id)
        .bind(developer_id)
        .fetch_one(&self.pool)
        .await?
        .0;

        if !owned {
            return Err(AppError::Forbidden("You do not own this game".to_string()));
        }
        Ok(())
    }

    async fn find_purchase_by_idempotency(&self, key: &str) -> Result<Option<StorePurchase>> {
        let p = sqlx::query_as::<_, StorePurchase>(
            r#"
            SELECT id, user_id, item_id, store_id, game_id, price_paid, currency,
                   developer_share, platform_fee, status, idempotency_key, metadata, created_at
            FROM store_purchases WHERE idempotency_key = $1
            "#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(p)
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revenue_share_sums_to_price() {
        let price = Decimal::new(1000, 2); // 10.00
                                           // developer_share_pct() = 0.70, platform_fee_pct() = 0.30; no extra /100.
        let dev = price * developer_share_pct();
        let fee = price * platform_fee_pct();
        assert_eq!(dev + fee, price);
    }

    #[test]
    fn invalid_item_kind_rejected() {
        assert!(MarketplaceService::validate_item_kind("hack").is_err());
        assert!(MarketplaceService::validate_item_kind("cosmetic").is_ok());
        assert!(MarketplaceService::validate_item_kind("pass").is_ok());
    }

    #[test]
    fn invalid_currency_rejected() {
        assert!(MarketplaceService::validate_item_currency("BTC").is_err());
        assert!(MarketplaceService::validate_item_currency("USDC").is_ok());
        assert!(MarketplaceService::validate_item_currency("points").is_ok());
    }

    #[test]
    fn developer_share_is_70_pct() {
        let price = Decimal::new(100_00, 2); // 100.00
                                             // developer_share_pct() = 0.70; multiply directly — no extra /100 needed.
        let dev = price * developer_share_pct();
        assert_eq!(dev, Decimal::new(70_00, 2));
    }
}
