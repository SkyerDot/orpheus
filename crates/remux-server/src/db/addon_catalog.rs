use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::addons::Addon;

/// Represents a row in the `addon_catalogs` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AddonCatalogRow {
    pub addon_id: Uuid,
    pub catalog_id: String,
    pub collection_id: Uuid,
    pub enabled: bool,
    pub max_items: Option<i64>,
    pub tags: String, // JSON array
    pub last_refreshed_at: Option<NaiveDateTime>,
    pub updated_at: NaiveDateTime,
}

/// Represents a catalog state for API purposes.
#[derive(Debug, Clone)]
pub struct AddonCatalogState {
    pub catalog_id: String,
    pub enabled: bool,
    pub max_items: Option<i64>,
    pub tags: Vec<String>,
    pub last_refreshed_at: Option<NaiveDateTime>,
}

impl AddonCatalogRow {
    /// Convert to `AddonCatalogState` for API use.
    pub fn to_state(&self) -> Result<AddonCatalogState> {
        let tags: Vec<String> = serde_json::from_str(&self.tags).unwrap_or_default();
        Ok(AddonCatalogState {
            catalog_id: self
                .catalog_id
                .clone(),
            enabled: self.enabled,
            max_items: self.max_items,
            tags,
            last_refreshed_at: self.last_refreshed_at,
        })
    }
}

/// Get all catalog states for an addon.
pub async fn get_catalog_states(
    db: &SqlitePool,
    addon_id: Uuid,
) -> Result<Vec<AddonCatalogRow>> {
    let rows = sqlx::query_as::<_, AddonCatalogRow>(
        "SELECT addon_id, catalog_id, collection_id, enabled, max_items, tags, last_refreshed_at, updated_at \
         FROM addon_catalogs WHERE addon_id = ?1 ORDER BY catalog_id",
    )
    .bind(addon_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

/// Get a single catalog state.
pub async fn get_catalog_state(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
) -> Result<Option<AddonCatalogRow>> {
    let row = sqlx::query_as::<_, AddonCatalogRow>(
        "SELECT addon_id, catalog_id, collection_id, enabled, max_items, tags, last_refreshed_at, updated_at \
         FROM addon_catalogs WHERE addon_id = ?1 AND catalog_id = ?2",
    )
    .bind(addon_id)
    .bind(catalog_id)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

/// Upsert a catalog state (insert or update).
pub async fn upsert_catalog_state(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
    collection_id: Uuid,
    enabled: bool,
    max_items: Option<i64>,
    tags: &[String],
    last_refreshed_at: Option<NaiveDateTime>,
) -> Result<()> {
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    let now = Utc::now().naive_utc();

    sqlx::query(
        "INSERT INTO addon_catalogs (addon_id, catalog_id, collection_id, enabled, max_items, tags, last_refreshed_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT (addon_id, catalog_id) DO UPDATE SET \
             enabled = excluded.enabled, \
             max_items = excluded.max_items, \
             tags = excluded.tags, \
             last_refreshed_at = excluded.last_refreshed_at, \
             updated_at = excluded.updated_at",
    )
    .bind(addon_id)
    .bind(catalog_id)
    .bind(collection_id)
    .bind(enabled)
    .bind(max_items)
    .bind(tags_json)
    .bind(last_refreshed_at)
    .bind(now)
    .execute(db)
    .await?;
    Ok(())
}

/// Update only the `enabled` field of a catalog.
pub async fn set_catalog_enabled(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
    enabled: bool,
) -> Result<()> {
    let now = Utc::now().naive_utc();
    sqlx::query(
        "UPDATE addon_catalogs SET enabled = ?1, updated_at = ?2 WHERE addon_id = ?3 AND catalog_id = ?4",
    )
    .bind(enabled)
    .bind(now)
    .bind(addon_id)
    .bind(catalog_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Update only the `max_items` field of a catalog.
pub async fn set_catalog_max_items(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
    max_items: Option<i64>,
) -> Result<()> {
    let now = Utc::now().naive_utc();
    sqlx::query(
        "UPDATE addon_catalogs SET max_items = ?1, updated_at = ?2 WHERE addon_id = ?3 AND catalog_id = ?4",
    )
    .bind(max_items)
    .bind(now)
    .bind(addon_id)
    .bind(catalog_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Update only the `tags` field of a catalog.
pub async fn set_catalog_tags(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
    tags: &[String],
) -> Result<()> {
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    let now = Utc::now().naive_utc();
    sqlx::query(
        "UPDATE addon_catalogs SET tags = ?1, updated_at = ?2 WHERE addon_id = ?3 AND catalog_id = ?4",
    )
    .bind(tags_json)
    .bind(now)
    .bind(addon_id)
    .bind(catalog_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Update `last_refreshed_at` for a catalog (called after successful import).
pub async fn set_catalog_last_refreshed(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
) -> Result<()> {
    let now = Utc::now().naive_utc();
    sqlx::query(
        "UPDATE addon_catalogs SET last_refreshed_at = ?1, updated_at = ?1 WHERE addon_id = ?2 AND catalog_id = ?3",
    )
    .bind(now)
    .bind(addon_id)
    .bind(catalog_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Set `last_refreshed_at` to NULL (used when purging catalog data).
pub async fn clear_catalog_last_refreshed(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
) -> Result<()> {
    let now = Utc::now().naive_utc();
    sqlx::query(
        "UPDATE addon_catalogs SET last_refreshed_at = NULL, updated_at = ?1 WHERE addon_id = ?2 AND catalog_id = ?3",
    )
    .bind(now)
    .bind(addon_id)
    .bind(catalog_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Get all enabled catalog collection IDs for an addon (for refresh planning).
pub async fn get_enabled_catalog_collection_ids(
    db: &SqlitePool,
    addon_id: Uuid,
) -> Result<Vec<Uuid>> {
    let ids = sqlx::query_scalar(
        "SELECT collection_id FROM addon_catalogs WHERE addon_id = ?1 AND enabled = 1",
    )
    .bind(addon_id)
    .fetch_all(db)
    .await?;
    Ok(ids)
}

/// Get all catalog collection IDs for an addon (enabled or disabled, for domain scoping).
pub async fn get_all_catalog_collection_ids(
    db: &SqlitePool,
    addon_id: Uuid,
) -> Result<Vec<Uuid>> {
    let ids = sqlx::query_scalar(
        "SELECT collection_id FROM addon_catalogs WHERE addon_id = ?1",
    )
    .bind(addon_id)
    .fetch_all(db)
    .await?;
    Ok(ids)
}

/// Get disabled (frozen) catalog collection IDs for an addon.
pub async fn get_frozen_catalog_collection_ids(
    db: &SqlitePool,
    addon_id: Uuid,
) -> Result<Vec<Uuid>> {
    let ids = sqlx::query_scalar(
        "SELECT collection_id FROM addon_catalogs WHERE addon_id = ?1 AND enabled = 0",
    )
    .bind(addon_id)
    .fetch_all(db)
    .await?;
    Ok(ids)
}

/// Delete a catalog state row (used when purging catalog data).
pub async fn delete_catalog_state(
    db: &SqlitePool,
    addon_id: Uuid,
    catalog_id: &str,
) -> Result<()> {
    sqlx::query("DELETE FROM addon_catalogs WHERE addon_id = ?1 AND catalog_id = ?2")
        .bind(addon_id)
        .bind(catalog_id)
        .execute(db)
        .await?;
    Ok(())
}

/// One-time backfill at startup: populate `addon_catalogs` from existing `addons.preset.config["catalogs"]` JSON.
/// Uses `INSERT OR IGNORE` so it never overwrites admin values already in the table.
/// Does NOT call `catalog_list()` — no network.
pub async fn backfill_catalog_states(db: &SqlitePool) -> Result<()> {
    let addons = Addon::list(db).await?;
    for addon in addons {
        let states = addon.catalog_states();
        if states.is_empty() {
            continue;
        }
        let addon_id = addon.id;
        for (local_id, state) in states {
            let collection_id = Uuid::new_v5(&addon_id, local_id.as_bytes());
            let tags_json =
                serde_json::to_string(&state.tags).unwrap_or_else(|_| "[]".to_string());
            sqlx::query(
                "INSERT OR IGNORE INTO addon_catalogs (addon_id, catalog_id, collection_id, enabled, max_items, tags, last_refreshed_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            )
            .bind(addon_id)
            .bind(&local_id)
            .bind(collection_id)
            .bind(state.enabled)
            .bind(state.max_items)
            .bind(tags_json)
            .bind(addon.updated_at)
            .execute(db)
            .await?;
        }
    }
    Ok(())
}

/// Ensure catalog rows exist for newly-discovered catalogs (from `resolve_catalogs`).
/// Uses `INSERT OR IGNORE` with provider defaults so it never overwrites admin values.
pub async fn ensure_catalog_rows(
    db: &SqlitePool,
    addon_id: Uuid,
    catalogs: &[crate::addons::CatalogInfo],
) -> Result<()> {
    for info in catalogs {
        let tags_json = "[]".to_string(); // CatalogInfo doesn't have tags, they come from overrides
        let collection_id = Uuid::new_v5(
            &addon_id,
            info.provider_catalog_id
                .as_bytes(),
        );
        sqlx::query(
            "INSERT OR IGNORE INTO addon_catalogs (addon_id, catalog_id, collection_id, enabled, max_items, tags, last_refreshed_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, datetime('now'))",
        )
        .bind(addon_id)
        .bind(&info.provider_catalog_id)
        .bind(collection_id)
        .bind(info.default_enabled)
        .bind(info.default_max_items)
        .bind(tags_json)
        .execute(db)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration_test::new_test_server;

    #[tokio::test]
    async fn backfill_is_idempotent_and_preserves_admin_values() {
        let (_server, guard) = new_test_server()
            .await
            .unwrap();
        let db = &guard
            .0
            .db;

        // Find a TMDB addon (system addon, should have catalogs)
        let addon_id: Uuid = sqlx::query_scalar(
            "SELECT id FROM addons WHERE json_extract(preset, '$.kind') = 'tmdb'",
        )
        .fetch_one(db)
        .await
        .unwrap();

        // First backfill
        backfill_catalog_states(db)
            .await
            .unwrap();

        // Verify rows exist
        let count_before: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM addon_catalogs WHERE addon_id = ?",
        )
        .bind(addon_id)
        .fetch_one(db)
        .await
        .unwrap();
        assert!(count_before > 0);

        // Manually update one catalog to have admin-chosen values
        let catalog_id = "popular_movies";
        sqlx::query(
            "UPDATE addon_catalogs SET enabled = 0, max_items = 50, tags = ? WHERE addon_id = ? AND catalog_id = ?",
        )
        .bind(r#"["admin"]"#)
        .bind(addon_id)
        .bind(catalog_id)
        .execute(db)
        .await
        .unwrap();

        // Second backfill (should not overwrite)
        backfill_catalog_states(db)
            .await
            .unwrap();

        // Verify admin values preserved
        let row = get_catalog_state(db, addon_id, catalog_id)
            .await
            .unwrap()
            .unwrap();
        assert!(!row.enabled);
        assert_eq!(row.max_items, Some(50));
        let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap();
        assert_eq!(tags, vec!["admin"]);

        // Count should be same (idempotent)
        let count_after: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM addon_catalogs WHERE addon_id = ?",
        )
        .bind(addon_id)
        .fetch_one(db)
        .await
        .unwrap();
        assert_eq!(count_before, count_after);
    }

    #[tokio::test]
    async fn ensure_catalog_rows_creates_new_and_preserves_existing() {
        let (_server, guard) = new_test_server()
            .await
            .unwrap();
        let db = &guard
            .0
            .db;

        let addon_id = Uuid::new_v4();
        let catalog_id = "new_catalog";
        let collection_id = Uuid::new_v5(&addon_id, catalog_id.as_bytes());

        let catalogs = vec![crate::addons::CatalogInfo {
            provider_catalog_id: catalog_id.to_string(),
            name: "New Catalog".to_string(),
            default_enabled: true,
            default_max_items: Some(100),
            collection_media_kind: Some(crate::db::CollectionMediaKind::Movie),
            media_kind: Some(crate::db::MediaKind::Movie),
        }];

        // First ensure
        ensure_catalog_rows(db, addon_id, &catalogs)
            .await
            .unwrap();

        let row = get_catalog_state(db, addon_id, catalog_id)
            .await
            .unwrap()
            .unwrap();
        assert!(row.enabled);
        assert_eq!(row.max_items, Some(100));

        // Admin changes it
        sqlx::query(
            "UPDATE addon_catalogs SET enabled = 0, max_items = 25 WHERE addon_id = ? AND catalog_id = ?",
        )
        .bind(addon_id)
        .bind(catalog_id)
        .execute(db)
        .await
        .unwrap();

        // Second ensure (should not overwrite)
        ensure_catalog_rows(db, addon_id, &catalogs)
            .await
            .unwrap();

        let row = get_catalog_state(db, addon_id, catalog_id)
            .await
            .unwrap()
            .unwrap();
        assert!(!row.enabled);
        assert_eq!(row.max_items, Some(25));
    }
}
