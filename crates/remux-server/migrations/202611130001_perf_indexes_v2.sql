-- Performance indexes for query optimization
-- Addresses: OR conditions in push_release_date_filter, RANDOM() sorting, NOT EXISTS correlations

-- Composite index for kind + enabled + date filtering (covers push_release_date_filter and general list queries)
CREATE INDEX IF NOT EXISTS idx_media_kind_enabled_dates
  ON media(kind, enabled, digital_released_at, released_at);

-- Covering index for user_media_state lookups by user_id + play_count (fixes played=false NOT EXISTS)
CREATE INDEX IF NOT EXISTS idx_ums_user_playcount
  ON user_media_state(user_id, play_count, media_id);

-- Index for random sampling support (if needed for future optimization)
-- CREATE INDEX IF NOT EXISTS idx_media_rowid ON media(rowid);

-- Analyze to update query planner statistics
ANALYZE;