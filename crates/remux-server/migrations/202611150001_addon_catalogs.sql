-- Addon catalogs state table
-- This table becomes the source of truth for catalog enabled/max_items/tags/last_refreshed_at
-- replacing the JSON storage in addons.preset.config["catalogs"]

CREATE TABLE IF NOT EXISTS addon_catalogs (
    addon_id          TEXT NOT NULL,
    catalog_id        TEXT NOT NULL,          -- local catalog id (e.g. "popular_movies")
    collection_id     BLOB NOT NULL,          -- uuid_v5(addon_id, catalog_id), as X'..'
    enabled           INTEGER NOT NULL DEFAULT 1,
    max_items         INTEGER,                -- NULL = inherit Settings.catalog_max_items
    tags              TEXT NOT NULL DEFAULT '[]',
    last_refreshed_at TIMESTAMP,
    updated_at        TIMESTAMP NOT NULL,
    PRIMARY KEY (addon_id, catalog_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_addon_catalogs_collection_id ON addon_catalogs(collection_id);

-- Backfill existing catalog states from addons.preset.config["catalogs"]
-- This runs once at startup via a Rust function (no network calls)
-- INSERT OR IGNORE preserves any admin values already in the table