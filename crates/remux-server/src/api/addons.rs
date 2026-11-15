use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use tracing::warn;

use remux_macros::{delete, get, post};
use uuid::Uuid;

use crate::{
    AppState, OptionExt, ResultExt,
    addons::{
        Addon, AddonCapabilities, AddonCatalogDto, AddonDto, AddonMetadata,
        AddonPreset, AddonService, CreateAddonRequest, UpdateAddonCatalogRequest,
        UpdateAddonRequest, registered_presets, set_user_addon_override,
        user_addon_override,
    },
    db::{Media, MediaKind as DbMediaKind, MediaRelation, addon_catalog, auth},
};
use axum_anyhow::{ApiResult as Result, IntoApiError as AxumIntoApiError};

type CapabilitySnapshot = (Vec<remux_sdks::stremio::ResourceType>, Vec<DbMediaKind>);

fn preset_capability_snapshot(preset: &dyn AddonPreset) -> CapabilitySnapshot {
    let metadata = preset.metadata();
    (
        metadata
            .supported_resources
            .into_iter()
            .map(|resource| resource.name)
            .collect(),
        metadata
            .supported_types
            .into_iter()
            .map(DbMediaKind::from)
            .collect(),
    )
}

async fn capability_snapshot(
    preset: &dyn AddonPreset,
    caps: &AddonCapabilities,
) -> anyhow::Result<CapabilitySnapshot> {
    let Some(kind) = caps
        .kind
        .as_deref()
    else {
        return Ok(preset_capability_snapshot(preset));
    };
    let Some((resources, types)) = kind
        .available_info()
        .await?
    else {
        return Ok(preset_capability_snapshot(preset));
    };
    let resources = resources
        .into_iter()
        .map(|resource| resource.name)
        .collect();
    let types: Vec<_> = types
        .into_iter()
        .filter_map(crate::addons::recognized_manifest_media_kind)
        .map(DbMediaKind::from)
        .collect();
    let types = if types.is_empty() {
        preset_capability_snapshot(preset).1
    } else {
        types
    };
    Ok((resources, types))
}

fn addon_to_dto(addon: Addon, addons: &AddonService) -> AddonDto {
    let preset = registered_presets()
        .into_iter()
        .find(|p| {
            p.id()
                == addon
                    .preset
                    .kind
        });

    let mut manifest_unreachable = false;
    let (
        supported_resources,
        supported_types,
        supported_resources_user,
        supported_types_user,
    ) = if let Some(ref p) = preset {
        // Runtime metadata was resolved when the addon was loaded. Listing
        // addons must not make another remote manifest request: one stalled
        // provider would otherwise hold the entire dashboard response open.
        let loaded = addons.list();
        let runtime = loaded
            .iter()
            .find(|r| {
                r.row
                    .id
                    == addon.id
            });
        manifest_unreachable = runtime.is_some_and(|r| {
            r.caps
                .manifest_unreachable
        });
        let meta = runtime
            .map(|r| {
                r.caps
                    .metadata
                    .clone()
            })
            .unwrap_or_else(|| p.metadata());
        let resources_user = meta
            .supported_resources_user
            .clone();
        let types_user = meta
            .supported_types_user
            .clone();
        let mut resources: Vec<_> = meta
            .supported_resources
            .into_iter()
            .map(|r| r.name)
            .collect();
        let mut types = meta.supported_types;
        // Disabled addons have no loaded runtime, so keep whatever was already
        // enabled for them selectable instead of showing only the preset defaults.
        if runtime.is_none() {
            for r in &addon.resources {
                if !resources.contains(r) {
                    resources.push(r.clone());
                }
            }
            for t in addon
                .types
                .iter()
                .cloned()
                .map(Into::into)
            {
                if !types.contains(&t) {
                    types.push(t);
                }
            }
        }
        (resources, types, resources_user, types_user)
    } else {
        (vec![], vec![], vec![], vec![])
    };

    AddonDto {
        id: addon.id,
        kind: addon
            .preset
            .kind,
        name: addon.name,
        config: addon
            .preset
            .config
            .into_inner(),
        resources: addon.resources,
        types: addon
            .types
            .iter()
            .cloned()
            .map(Into::into)
            .collect(),
        enabled: addon.enabled,
        supported_resources,
        supported_types,
        supported_resources_user,
        supported_types_user,
        priority: addon.priority,
        system: addon.system,
        is_default: addon.is_default,
        http_redirect_stream: addon.http_redirect_stream,
        subtitle_extraction: addon.subtitle_extraction,
        probe_on_scan: addon.probe_on_scan,
        service_filter: addon.service_filter,
        description: preset.map(|p| {
            p.metadata()
                .description
        }),
        manifest_unreachable,
        created_at: addon.created_at,
        updated_at: addon.updated_at,
    }
}

/// List metadata for every registered addon kind. Drives the dashboard's
/// "add addon" picker and form renderer.
/// Excludes system presets that are already installed (e.g. TMDB) to prevent duplicates.
#[get("/addon-kinds")]
pub async fn list_addon_kinds(
    State(state): State<AppState>,
    _session: auth::AdminSession,
) -> Result<Json<Vec<AddonMetadata>>> {
    let addons = Addon::list(
        &state
            .ctx
            .db,
    )
    .await?;
    let system_preset_kinds: std::collections::HashSet<String> = addons
        .iter()
        .filter(|a| a.system)
        .map(|a| {
            a.preset
                .kind
                .clone()
        })
        .collect();

    Ok(Json(
        registered_presets()
            .iter()
            .filter(|p| !system_preset_kinds.contains(p.id()))
            .map(|p| p.metadata())
            .collect(),
    ))
}

/// List all configured addon instances.
#[get("/addons")]
pub async fn list_addons(
    State(state): State<AppState>,
    _session: auth::AdminSession,
) -> Result<Json<Vec<AddonDto>>> {
    let addons = Addon::list(
        &state
            .ctx
            .db,
    )
    .await?;
    let dtos = addons
        .into_iter()
        .map(|addon| {
            addon_to_dto(
                addon,
                &state
                    .ctx
                    .addons,
            )
        })
        .collect();
    Ok(Json(dtos))
}

/// Get a single addon instance by ID.
#[get("/addons/{id}")]
pub async fn get_addon(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path(id): Path<Uuid>,
) -> Result<Json<AddonDto>> {
    let addon = Addon::get(
        &state
            .ctx
            .db,
        id,
    )
    .await?
    .context_not_found("Addon not found")?;
    Ok(Json(addon_to_dto(
        addon,
        &state
            .ctx
            .addons,
    )))
}

/// Create a new addon instance.
#[post("/addons")]
pub async fn create_addon(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Json(mut payload): Json<CreateAddonRequest>,
) -> Result<(StatusCode, Json<AddonDto>)> {
    let presets = registered_presets();
    let preset = presets
        .iter()
        .find(|p| {
            p.id()
                == payload
                    .preset
                    .kind
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unknown addon kind: {}",
                payload
                    .preset
                    .kind
            )
        })
        .context_bad_request("Unknown addon kind")?;

    // Reject creating a second instance of a system preset (e.g. TMDB).
    // System addons are singletons — only one can exist.
    let existing_system = Addon::list(
        &state
            .ctx
            .db,
    )
    .await?
    .into_iter()
    .find(|a| {
        a.system
            && a.preset
                .kind
                == payload
                    .preset
                    .kind
    });
    if existing_system.is_some() {
        return Err(
            anyhow::anyhow!("A system addon of this kind already exists")
                .context_conflict("Conflict", "System addon already exists"),
        );
    }

    let addon_id = Uuid::new_v4();
    let normalized_config = preset
        .normalize_cfg(
            payload
                .preset
                .config
                .into_inner(),
            &state
                .ctx
                .config,
        )
        .context_bad_request("Invalid addon configuration")?;
    payload
        .preset
        .config = normalized_config.into();
    let caps = preset
        .from_cfg(
            addon_id,
            payload
                .preset
                .config
                .expose(),
            &state
                .ctx
                .config,
        )
        .context_bad_request("Invalid addon configuration")?;
    let (derived_resources, derived_types) =
        capability_snapshot(preset.as_ref(), &caps)
            .await
            .context_not_reachable()?;

    let resources: Vec<remux_sdks::stremio::ResourceType> = if payload
        .resources
        .is_empty()
    {
        derived_resources
    } else {
        payload.resources
    };
    let types: Vec<DbMediaKind> = if payload
        .types
        .is_empty()
    {
        derived_types
    } else {
        payload
            .types
            .into_iter()
            .map(DbMediaKind::from)
            .collect()
    };

    let now = Utc::now().naive_utc();
    let addon = Addon {
        id: addon_id,
        preset: payload.preset,
        name: payload.name,
        resources,
        types,
        enabled: true,
        priority: payload.priority,
        created_at: now,
        updated_at: now,
        system: false,
        is_default: payload.is_default,
        http_redirect_stream: false,
        subtitle_extraction: false,
        probe_on_scan: false,
        service_filter: vec![],
    };

    addon
        .insert(
            &state
                .ctx
                .db,
        )
        .await?;
    state
        .ctx
        .addons
        .reload(
            &state
                .ctx
                .db,
            &state
                .ctx
                .config,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(addon_to_dto(
            addon,
            &state
                .ctx
                .addons,
        )),
    ))
}

/// Update an existing addon instance. Any field omitted is left unchanged.
#[post("/addons/{id}")]
pub async fn update_addon(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAddonRequest>,
) -> Result<Json<AddonDto>> {
    let UpdateAddonRequest {
        name,
        config,
        resources,
        types,
        enabled,
        priority,
        is_default,
        http_redirect_stream,
        subtitle_extraction,
        probe_on_scan,
        service_filter,
    } = payload;
    let mut addon = Addon::get(
        &state
            .ctx
            .db,
        id,
    )
    .await?
    .context_not_found("Addon not found")?;

    // Keep these Options available below: explicit fields take precedence over
    // capability values derived when the config changes.
    if let Some(resources) = &resources {
        if addon.system {
            if resources != &addon.resources {
                warn!("ignoring resources change on system addon {}", addon.id);
            }
        } else {
            addon.resources = resources.clone();
        }
    }
    if let Some(types) = &types {
        let types: Vec<_> = types
            .iter()
            .cloned()
            .map(DbMediaKind::from)
            .collect();
        if addon.system {
            if types != addon.types {
                warn!("ignoring types change on system addon {}", addon.id);
            }
        } else {
            addon.types = types;
        }
    }
    if let Some(name) = name {
        addon.name = name;
    }
    if let Some(enabled) = enabled {
        if addon.system && !enabled {
            return Err(anyhow::anyhow!("Cannot disable a system addon")
                .context_forbidden("Forbidden", "System addons cannot be disabled"));
        }
        addon.enabled = enabled;
    }
    if let Some(priority) = priority {
        addon.priority = priority;
    }
    if let Some(is_default) = is_default {
        addon.is_default = is_default;
    }
    if let Some(http_redirect_stream) = http_redirect_stream {
        addon.http_redirect_stream = http_redirect_stream;
    }
    if let Some(probe_on_scan) = probe_on_scan {
        addon.probe_on_scan = probe_on_scan;
    }
    if let Some(subtitle_extraction) = subtitle_extraction {
        addon.subtitle_extraction = subtitle_extraction;
    }
    if let Some(service_filter) = service_filter {
        addon.service_filter = service_filter;
    }
    addon.updated_at = Utc::now().naive_utc();

    let presets = registered_presets();
    let preset = presets
        .iter()
        .find(|p| {
            p.id()
                == addon
                    .preset
                    .kind
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unknown addon kind: {}",
                addon
                    .preset
                    .kind
            )
        })
        .context_bad_request("Unknown addon kind")?;
    let config_changed = config.is_some();
    if let Some(config) = config {
        addon
            .preset
            .config = preset
            .normalize_cfg(
                config,
                &state
                    .ctx
                    .config,
            )
            .context_bad_request("Invalid addon configuration")?
            .into();
    }
    let caps = preset
        .from_cfg(
            addon.id,
            addon
                .preset
                .config
                .expose(),
            &state
                .ctx
                .config,
        )
        .context_bad_request("Invalid addon configuration")?;

    if config_changed && !addon.system {
        let (derived_resources, derived_types) =
            match capability_snapshot(preset.as_ref(), &caps).await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    warn!(
                        addon_id = %addon.id,
                        %error,
                        "failed to refresh addon capabilities; using preset metadata"
                    );
                    preset_capability_snapshot(preset.as_ref())
                }
            };
        if resources.is_none() {
            addon.resources = derived_resources;
        }
        if types.is_none() {
            addon.types = derived_types;
        }
    }

    addon
        .update(
            &state
                .ctx
                .db,
        )
        .await?;
    state
        .ctx
        .addons
        .reload(
            &state
                .ctx
                .db,
            &state
                .ctx
                .config,
        )
        .await?;
    Ok(Json(addon_to_dto(
        addon,
        &state
            .ctx
            .addons,
    )))
}

/// Delete an addon instance.
#[delete("/addons/{id}")]
pub async fn delete_addon(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let addon_row = Addon::get(
        &state
            .ctx
            .db,
        id,
    )
    .await?
    .context_not_found("Addon not found")?;

    if addon_row.system {
        return Err(anyhow::anyhow!("Cannot delete a system addon")
            .context_forbidden("Forbidden", "Cannot delete a system addon"));
    }

    // Purge the addon's index (removes e.g. IPTV channels) before deleting.
    if let Some(runtime) = state
        .ctx
        .addons
        .get(id)
    {
        if let Some(index) = &runtime.index {
            if let Err(e) = index
                .purge_index(&state.ctx, &addon_row)
                .await
            {
                warn!(addon = %id, error = %e, "purge_index failed on addon delete");
            }
        }
    }

    Addon::delete(
        &state
            .ctx
            .db,
        id,
    )
    .await?;
    state
        .ctx
        .addons
        .reload(
            &state
                .ctx
                .db,
            &state
                .ctx
                .config,
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// List catalogs for an addon merged with their config state.
/// Catalogs are disabled by default until explicitly enabled.
#[get("/addons/{id}/catalogs")]
pub async fn get_addon_catalogs(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AddonCatalogDto>>> {
    let runtime = state
        .ctx
        .addons
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("addon not instantiated"))
        .context_bad_request("Addon could not be instantiated")?;

    let resolved = runtime
        .resolve_catalogs(&state.ctx)
        .await
        .context_not_reachable()?;

    // Fetch persisted catalog states from addon_catalogs table for last_refreshed_at
    let catalog_rows = addon_catalog::get_catalog_states(
        &state
            .ctx
            .db,
        id,
    )
    .await?;
    let catalog_state_map: std::collections::HashMap<
        String,
        addon_catalog::AddonCatalogState,
    > = catalog_rows
        .into_iter()
        .filter_map(|row| {
            row.to_state()
                .ok()
                .map(|s| {
                    (
                        s.catalog_id
                            .clone(),
                        s,
                    )
                })
        })
        .collect();

    let result = resolved
        .into_iter()
        .map(|c| {
            let extra = catalog_state_map.get(&c.provider_catalog_id);
            AddonCatalogDto {
                catalog_id: c.catalog_id,
                name: c.name,
                enabled: c.enabled,
                max_items: c.max_items,
                tags: c.tags,
                collection_id: Some(c.collection_id),
                // Add last_refreshed_at under remux namespace per AGENTS.md
                remux: extra.map(|e| remux_sdks::remux::AddonCatalogRemux {
                    last_refreshed_at: e
                        .last_refreshed_at
                        .map(|dt| dt.to_string()),
                }),
            }
        })
        .collect();

    Ok(Json(result))
}

/// Batch-update enabled/max_items/tags for an addon's catalogs.
/// Partial update: only fields present in each request entry are changed.
/// `enabled: false` alone must not clear `max_items` or `tags`.
#[post("/addons/{id}/catalogs")]
pub async fn update_addon_catalogs(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path(id): Path<Uuid>,
    Json(payload): Json<Vec<UpdateAddonCatalogRequest>>,
) -> Result<StatusCode> {
    // Verify addon exists
    let _addon = Addon::get(
        &state
            .ctx
            .db,
        id,
    )
    .await?
    .context_not_found("Addon not found")?;

    for req in &payload {
        let local_id = req
            .catalog_id
            .strip_prefix(&format!("addon:{id}:"))
            .unwrap_or(&req.catalog_id)
            .to_string();

        // Fetch current state from addon_catalogs table for partial update
        let current = addon_catalog::get_catalog_state(
            &state
                .ctx
                .db,
            id,
            &local_id,
        )
        .await?;
        let (current_enabled, current_max_items, current_tags) = match current {
            Some(row) => {
                let tags: Vec<String> =
                    serde_json::from_str(&row.tags).unwrap_or_default();
                (row.enabled, row.max_items, tags)
            }
            None => (true, None, Vec::new()), // Default: enabled, no max_items, no tags
        };

        // Partial update: only override fields that are explicitly provided
        let enabled = req.enabled; // enabled is required in the request
        let max_items = req
            .max_items
            .or(current_max_items);
        let tags = req
            .tags
            .clone()
            .unwrap_or_else(|| current_tags.clone());

        let collection_id = Uuid::new_v5(&id, local_id.as_bytes());

        // Upsert to addon_catalogs table
        addon_catalog::upsert_catalog_state(
            &state
                .ctx
                .db,
            id,
            &local_id,
            collection_id,
            enabled,
            max_items,
            &tags,
            None, // last_refreshed_at unchanged
        )
        .await?;

        // Apply tags immediately to all media already in this catalog.
        // We need to sync tags: remove old provider tags not in new set, add new ones.
        let old_provider_tags: std::collections::HashSet<String> = current_tags
            .iter()
            .filter(|t| t.starts_with("provider:"))
            .cloned()
            .collect();
        let new_provider_tags: std::collections::HashSet<String> = tags
            .iter()
            .filter(|t| t.starts_with("provider:"))
            .cloned()
            .collect();

        // Remove tags that are no longer in the catalog config
        for tag in &old_provider_tags {
            if !new_provider_tags.contains(tag) {
                if let Err(e) = sqlx::query(
                    "DELETE FROM media_tags WHERE tag = ? AND media_id IN (
                         SELECT mr.right_media_id FROM media_relations mr
                         WHERE mr.left_media_id = ? AND mr.role = 'catalog'
                     )",
                )
                .bind(tag)
                .bind(collection_id)
                .execute(
                    &state
                        .ctx
                        .db,
                )
                .await
                {
                    warn!(addon = %id, catalog = %local_id, tag = %tag, error = %e, "failed to remove catalog tag");
                }
            }
        }

        // Add new provider tags
        for tag in &new_provider_tags {
            if !old_provider_tags.contains(tag) {
                if let Err(e) = sqlx::query(
                    "INSERT OR IGNORE INTO media_tags (media_id, tag) \
                     SELECT mr.right_media_id, ? FROM media_relations mr \
                     WHERE mr.left_media_id = ? AND mr.role = 'catalog'",
                )
                .bind(tag)
                .bind(collection_id)
                .execute(
                    &state
                        .ctx
                        .db,
                )
                .await
                {
                    warn!(addon = %id, catalog = %local_id, tag = %tag, error = %e, "failed to apply catalog tag");
                }
            }
        }
    }

    // Reload addon service to pick up changes
    state
        .ctx
        .addons
        .reload(
            &state
                .ctx
                .db,
            &state
                .ctx
                .config,
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Purge all catalog data for a specific catalog (destructive, explicit admin action).
/// Deletes media_relations for this catalog, orphaned playlists, and resets last_refreshed_at.
/// Refuses if the catalog is still enabled.
/// Does NOT delete media rows or library files.
#[post("/addons/{id}/catalogs/{catalog_id}/purge")]
pub async fn purge_addon_catalog(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path((id, catalog_id)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>> {
    let addon = Addon::get(
        &state
            .ctx
            .db,
        id,
    )
    .await?
    .context_not_found("Addon not found")?;

    let local_id = catalog_id
        .strip_prefix(&format!("addon:{id}:"))
        .unwrap_or(&catalog_id)
        .to_string();

    // Check current state
    let current = addon_catalog::get_catalog_state(
        &state
            .ctx
            .db,
        id,
        &local_id,
    )
    .await?;
    let Some(row) = current else {
        return Err(anyhow::anyhow!("Catalog not found")
            .context_not_found("Not Found", "Catalog not found"));
    };

    if row.enabled {
        return Err(anyhow::anyhow!(
            "Cannot purge an enabled catalog. Disable it first."
        )
        .context_conflict("Conflict", "Catalog must be disabled before purging"));
    }

    let collection_id = row.collection_id;

    // Delete catalog memberships
    let relation_count: i64 = sqlx::query(
        "DELETE FROM media_relations WHERE left_media_id = ? AND role = 'catalog'",
    )
    .bind(collection_id)
    .execute(
        &state
            .ctx
            .db,
    )
    .await?
    .rows_affected() as i64;

    // Delete orphaned playlists (reuse existing logic scoped to this collection)
    let mut playlist_count = 0i64;
    let playlist_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT m.id FROM media m \
         JOIN media_relations r ON r.right_media_id = m.id \
         WHERE r.role = 'catalog' AND m.kind = 'playlist' AND r.left_media_id = ?",
    )
    .bind(collection_id)
    .fetch_all(
        &state
            .ctx
            .db,
    )
    .await?;
    for pid in &playlist_ids {
        if let Err(e) = MediaRelation::delete_by_left_id(
            &state
                .ctx
                .db,
            pid,
        )
        .await
        {
            warn!(playlist = %pid, error = %e, "failed to delete playlist relations");
        }
        if let Err(e) = Media::delete(
            &state
                .ctx
                .db,
            pid,
        )
        .await
        {
            warn!(playlist = %pid, error = %e, "failed to delete playlist media");
        } else {
            playlist_count += 1;
        }
    }

    // Reset last_refreshed_at to NULL
    addon_catalog::clear_catalog_last_refreshed(
        &state
            .ctx
            .db,
        id,
        &local_id,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "deleted_relations": relation_count,
        "deleted_playlists": playlist_count,
        "message": "Catalog data purged. Library items and files were not affected."
    })))
}

/// Get a user's addon override list (ordered by priority).
/// Returns an empty array when the user has no override (uses the default list).
#[get("/users/{user_id}/addons")]
pub async fn get_user_addons(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let ids = user_addon_override(
        &state
            .ctx
            .db,
        user_id,
    )
    .await?
    .unwrap_or_default();
    Ok(Json(ids))
}

/// Set a user's addon override list. Send an empty array to clear the override
/// and fall back to the default addon list.
#[post("/users/{user_id}/addons")]
pub async fn set_user_addons(
    State(state): State<AppState>,
    _session: auth::AdminSession,
    Path(user_id): Path<Uuid>,
    Json(addon_ids): Json<Vec<Uuid>>,
) -> Result<StatusCode> {
    set_user_addon_override(
        &state
            .ctx
            .db,
        user_id,
        &addon_ids,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::integration_test::{auth_header_with_token, authenticated_server};
    use async_trait::async_trait;
    use http::header::HeaderValue;
    use remux_sdks::remux::MediaKind;
    use serde_json::json;
    use std::sync::Arc;

    struct ManifestKind(Vec<remux_sdks::stremio::MediaType>);

    #[async_trait]
    impl crate::addons::AddonKind for ManifestKind {
        fn id(&self) -> &'static str {
            "test"
        }

        async fn available_info(
            &self,
        ) -> anyhow::Result<
            Option<(
                Vec<remux_sdks::stremio::ResourceRef>,
                Vec<remux_sdks::stremio::MediaType>,
            )>,
        > {
            Ok(Some((
                vec![],
                self.0
                    .clone(),
            )))
        }
    }

    fn auth(token: &str) -> (http::header::HeaderName, HeaderValue) {
        (
            http::header::AUTHORIZATION,
            HeaderValue::from_str(&auth_header_with_token(token)).unwrap(),
        )
    }

    #[tokio::test]
    async fn capability_snapshot_uses_shared_mapping_and_falls_back_for_unknown_types()
    {
        let preset = registered_presets()
            .into_iter()
            .find(|preset| preset.id() == "stremio")
            .unwrap();

        let genre = capability_snapshot(
            preset.as_ref(),
            &AddonCapabilities {
                kind: Some(Arc::new(ManifestKind(vec![
                    remux_sdks::stremio::MediaType::Other("genre".to_string()),
                ]))),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(genre.1, vec![DbMediaKind::Genre]);

        let unknown = capability_snapshot(
            preset.as_ref(),
            &AddonCapabilities {
                kind: Some(Arc::new(ManifestKind(vec![
                    remux_sdks::stremio::MediaType::Other("anime".to_string()),
                ]))),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(
            !unknown
                .1
                .is_empty()
        );
    }

    #[tokio::test]
    async fn list_addon_kinds_includes_stremio() {
        let (server, _ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);

        let resp = server
            .get("/addon-kinds")
            .add_header(h, v)
            .await;
        resp.assert_status_ok();

        let kinds: Vec<AddonMetadata> = resp.json();
        assert!(
            kinds
                .iter()
                .any(|k| k.id == "stremio"),
            "stremio kind should be registered"
        );
        let stremio = kinds
            .iter()
            .find(|k| k.id == "stremio")
            .unwrap();
        assert_eq!(
            stremio
                .options
                .len(),
            1
        );
        assert_eq!(stremio.options[0].id, "manifest_url");
    }

    #[tokio::test]
    async fn create_list_delete_addon_roundtrip() {
        let (server, ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);

        // Record baseline count (migrations may seed default addons).
        let initial: Vec<AddonDto> = server
            .get("/addons")
            .add_header(h.clone(), v.clone())
            .await
            .json();
        let initial_count = initial.len();

        let create_resp = server
            .post("/addons")
            .add_header(h.clone(), v.clone())
            .json(&json!({
                "preset": {
                    "kind": "stremio",
                    "config": { "manifest_url": "https://v3-cinemeta.strem.io/manifest.json" }
                },
                "name": "Test Cinemeta",
                "resources": ["catalog"],
            }))
            .await;
        create_resp.assert_status(http::StatusCode::CREATED);

        let created: AddonDto = create_resp.json();
        assert_eq!(created.kind, "stremio");
        assert_eq!(created.name, "Test Cinemeta");

        // Registry should reflect the new addon immediately.
        assert!(
            ctx.0
                .addons
                .get(created.id)
                .is_some(),
            "registry did not pick up the new addon"
        );

        // List shows exactly one more than baseline.
        let list_resp = server
            .get("/addons")
            .add_header(h.clone(), v.clone())
            .await;
        list_resp.assert_status_ok();
        let list: Vec<AddonDto> = list_resp.json();
        assert_eq!(list.len(), initial_count + 1);
        assert!(
            list.iter()
                .any(|a| a.id == created.id)
        );

        // Delete.
        let del_resp = server
            .delete(&format!("/addons/{}", created.id))
            .add_header(h.clone(), v.clone())
            .await;
        del_resp.assert_status(http::StatusCode::NO_CONTENT);

        let list_after: Vec<AddonDto> = server
            .get("/addons")
            .add_header(h, v)
            .await
            .json();
        assert_eq!(
            list_after.len(),
            initial_count,
            "addon should be gone after delete"
        );
        assert!(
            ctx.0
                .addons
                .get(created.id)
                .is_none(),
            "registry should have dropped the deleted addon"
        );
    }

    #[tokio::test]
    async fn create_addon_rejects_unknown_kind() {
        let (server, _ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);

        let resp = server
            .post("/addons")
            .add_header(h, v)
            .expect_failure()
            .json(&json!({
                "preset": { "kind": "no-such-kind", "config": {} },
                "name": "Bad",
            }))
            .await;
        resp.assert_status(http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_addon_rejects_missing_required_config() {
        let (server, _ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);

        // Stremio requires manifest_url; omitting it should fail validation
        // because from_cfg() returns Err.
        let resp = server
            .post("/addons")
            .add_header(h, v)
            .expect_failure()
            .json(&json!({
                "preset": { "kind": "stremio", "config": {} },
                "name": "Missing URL",
            }))
            .await;
        resp.assert_status(http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_config_refreshes_derived_capabilities() {
        let (server, _ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);
        let dir = std::env::temp_dir()
            .to_string_lossy()
            .to_string();

        let created: AddonDto = server
            .post("/addons")
            .add_header(h.clone(), v.clone())
            .json(&json!({
                "preset": {
                    "kind": "opendal-local",
                    "config": { "paths": [dir.clone()], "media_kind": "movie" }
                },
                "name": "Local files"
            }))
            .await
            .json();
        assert!(
            created
                .types
                .contains(&MediaKind::Movie)
        );

        let updated: AddonDto = server
            .post(&format!("/addons/{}", created.id))
            .add_header(h, v)
            .json(&json!({
                "config": { "paths": [dir], "media_kind": "episode" }
            }))
            .await
            .json();

        assert!(
            updated
                .types
                .contains(&MediaKind::Series)
        );
        assert!(
            !updated
                .types
                .contains(&MediaKind::Movie)
        );

        let (h, v) = auth(&token);
        let explicit: AddonDto = server
            .post(&format!("/addons/{}", created.id))
            .add_header(h, v)
            .json(&json!({
                "config": { "paths": [std::env::temp_dir()], "media_kind": "movie" },
                "resources": ["stream"],
                "types": ["series"]
            }))
            .await
            .json();
        assert_eq!(
            explicit.resources,
            vec![remux_sdks::stremio::ResourceType::Stream]
        );
        assert_eq!(explicit.types, vec![MediaKind::Series]);

        let (h, v) = auth(&token);
        let explicit_empty: AddonDto = server
            .post(&format!("/addons/{}", created.id))
            .add_header(h, v)
            .json(&json!({
                "config": { "paths": [std::env::temp_dir()], "media_kind": "episode" },
                "resources": [],
                "types": []
            }))
            .await
            .json();
        assert!(
            explicit_empty
                .resources
                .is_empty()
        );
        assert!(
            explicit_empty
                .types
                .is_empty()
        );
    }

    #[tokio::test]
    async fn update_config_survives_unreachable_capability_manifest() {
        let (server, ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);
        let now = Utc::now().naive_utc();
        let mut addon = Addon {
            id: Uuid::new_v4(),
            name: "Unavailable manifest".to_string(),
            preset: crate::addons::AddonPresetRef {
                kind: "stremio".to_string(),
                config: json!({ "manifest_url": "http://127.0.0.1:1/manifest.json" })
                    .into(),
            },
            resources: vec![],
            types: vec![],
            enabled: false,
            priority: 0,
            created_at: now,
            updated_at: now,
            system: false,
            is_default: false,
            http_redirect_stream: false,
            subtitle_extraction: false,
            probe_on_scan: false,
            service_filter: vec![],
        };
        addon
            .insert(
                &ctx.0
                    .db,
            )
            .await
            .unwrap();

        let response = server
            .post(&format!("/addons/{}", addon.id))
            .add_header(h, v)
            .json(&json!({
                "name": "Updated while unavailable",
                "config": { "manifest_url": "http://127.0.0.1:1/manifest.json" }
            }))
            .await;
        response.assert_status_ok();

        addon = Addon::get(
            &ctx.0
                .db,
            addon.id,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(addon.name, "Updated while unavailable");
        assert!(
            !addon
                .resources
                .is_empty()
        );
        assert!(
            !addon
                .types
                .is_empty()
        );
    }

    #[tokio::test]
    async fn list_addons_does_not_refetch_unreachable_manifest() {
        let (server, ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);
        let manifest = httpmock::MockServer::start();
        // The SDK response cache is process-wide and keyed by URL, and httpmock
        // reuses pooled servers, so this test needs a path no other test uses;
        // otherwise a manifest another test cached would answer the probe.
        let hits = manifest.mock(|when, then| {
            when.path("/unreachable-manifest/manifest.json");
            then.status(404);
        });
        let now = Utc::now().naive_utc();
        let addon = Addon {
            id: Uuid::new_v4(),
            name: "Unreachable manifest".to_string(),
            preset: crate::addons::AddonPresetRef {
                kind: "stremio".to_string(),
                config: json!({ "manifest_url": manifest.url("/unreachable-manifest/manifest.json") })
                    .into(),
            },
            resources: vec![],
            types: vec![],
            enabled: true,
            priority: 0,
            created_at: now,
            updated_at: now,
            system: false,
            is_default: false,
            http_redirect_stream: false,
            subtitle_extraction: false,
            probe_on_scan: false,
            service_filter: vec![],
        };
        addon
            .insert(
                &ctx.0
                    .db,
            )
            .await
            .unwrap();
        ctx.0
            .addons
            .reload(
                &ctx.0
                    .db,
                &ctx.0
                    .config,
            )
            .await
            .unwrap();
        let hits_after_load = hits.hits();
        assert!(hits_after_load >= 1, "load should have probed the manifest");

        for _ in 0..2 {
            let list: Vec<AddonDto> = server
                .get("/addons")
                .add_header(h.clone(), v.clone())
                .await
                .json();
            let dto = list
                .iter()
                .find(|a| a.id == addon.id)
                .expect("addon listed despite unreachable manifest");
            assert!(dto.manifest_unreachable);
            assert!(
                !dto.supported_resources
                    .is_empty()
            );
        }
        assert_eq!(
            hits.hits(),
            hits_after_load,
            "listing addons must not issue manifest requests"
        );
    }

    async fn tmdb_addon_id(db: &sqlx::SqlitePool) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM addons WHERE json_extract(preset, '$.kind') = ?1",
        )
        .bind(crate::addons::tmdb::PRESET_ID)
        .fetch_one(db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn system_addon_cannot_be_disabled() {
        let (server, ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);
        let id = tmdb_addon_id(
            &ctx.0
                .db,
        )
        .await;

        assert!(
            crate::common::tmdb_client(&ctx.0)
                .await
                .is_some()
        );

        // Attempting to disable a system addon should return 403 Forbidden
        let resp = server
            .post(&format!("/addons/{id}"))
            .add_header(h.clone(), v.clone())
            .expect_failure()
            .json(&json!({ "enabled": false }))
            .await;
        resp.assert_status(http::StatusCode::FORBIDDEN);

        // System addon should still be enabled
        assert!(
            ctx.0
                .addons
                .has_enabled_preset(crate::addons::tmdb::PRESET_ID)
        );
        assert!(
            crate::common::tmdb_client(&ctx.0)
                .await
                .is_some()
        );
    }

    #[tokio::test]
    async fn disabling_another_source_leaves_tmdb_on() {
        let (server, ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);
        let deezer: Uuid = sqlx::query_scalar(
            "SELECT id FROM addons WHERE json_extract(preset, '$.kind') = 'deezer'",
        )
        .fetch_one(
            &ctx.0
                .db,
        )
        .await
        .unwrap();
        server
            .post(&format!("/addons/{deezer}"))
            .add_header(h, v)
            .json(&json!({ "enabled": false }))
            .await
            .assert_status_ok();
        assert!(
            crate::common::tmdb_client(&ctx.0)
                .await
                .is_some()
        );
    }

    #[tokio::test]
    async fn a_second_tmdb_instance_rejected() {
        let (server, ctx, token) = authenticated_server().await;
        let (h, v) = auth(&token);
        let seeded = tmdb_addon_id(
            &ctx.0
                .db,
        )
        .await;

        // Attempting to create a second TMDB instance should be rejected
        let second = crate::common::get_uuid();
        sqlx::query(
            "INSERT INTO addons (id, name, preset, resources, types, enabled, priority, created_at, updated_at) \
             VALUES (?1, 'TMDB 2', '{\"kind\":\"tmdb\",\"config\":{}}', '[\"meta\"]', '[]', 1, 100, datetime('now'), datetime('now'))",
        )
        .bind(second)
        .execute(&ctx.0.db)
        .await
        .unwrap();
        ctx.0
            .addons
            .reload(
                &ctx.0
                    .db,
                &ctx.0
                    .config,
            )
            .await
            .unwrap();

        // The system addon (seeded) should still be on
        assert!(
            crate::common::tmdb_client(&ctx.0)
                .await
                .is_some(),
            "the system TMDB instance keeps the source on"
        );

        // Attempting to disable the system addon should be rejected
        let resp = server
            .post(&format!("/addons/{seeded}"))
            .add_header(h.clone(), v.clone())
            .expect_failure()
            .json(&json!({ "enabled": false }))
            .await;
        resp.assert_status(http::StatusCode::FORBIDDEN);

        // The second instance should also not be disableable via API (it was inserted directly, not through the API)
        // But we can verify the create endpoint would reject it
        let (server2, _ctx2, token2) = authenticated_server().await;
        let (h2, v2) = auth(&token2);
        let resp = server2
            .post("/addons")
            .add_header(h2, v2)
            .expect_failure()
            .json(&json!({
                "preset": { "kind": "tmdb", "config": {} },
                "name": "TMDB Duplicate",
            }))
            .await;
        resp.assert_status(http::StatusCode::CONFLICT);
    }
}
