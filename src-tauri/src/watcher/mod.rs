use crate::db::queries;
use crate::tags_file;
use crate::thumbnail;
use rusqlite::Connection;
use std::path::Path;

/// Remove DB entries (and description file entries) for media whose files no longer exist on disk.
/// Returns the number of removed entries.
pub fn cleanup_missing_files(vault_path: &Path) -> Result<u32, String> {
    let media_dir = vault_path.join("media");
    let db_path = vault_path.join("vault.db");

    let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT id, extension FROM media")
        .map_err(|e| format!("Query failed: {e}"))?;

    let rows: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("Query failed: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut count = 0u32;

    for (id, ext) in &rows {
        let media_file = media_dir.join(format!("{id}.{ext}"));
        if !media_file.exists() {
            thumbnail::remove_previews(vault_path, id);
            let _ = tags_file::remove(vault_path, id);
            if let Err(e) = queries::delete_media(&conn, id) {
                log::error!("Failed to remove ghost entry {id}: {e}");
            } else {
                log::info!("Removed ghost entry: {id}.{ext}");
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Sync tags from tags.json into the DB.
/// Marks changed items as unprocessed so the worker re-processes them.
/// Returns the number of synced media items.
pub fn sync_tags(vault_path: &Path) -> Result<u32, String> {
    let all_tags = tags_file::load(vault_path);
    if all_tags.is_empty() {
        return Ok(0);
    }

    let db_path = vault_path.join("vault.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let mut count = 0u32;

    for (media_id, json_tags) in &all_tags {
        // Skip media that don't exist in DB yet (worker will handle them on import)
        if let Ok(None) = queries::get_media_by_id(&conn, media_id) {
            continue;
        }

        // Build TagInfo list from JSON
        let mut new_tags: Vec<crate::db::models::TagInfo> = Vec::new();
        for (key, values) in json_tags {
            for value in values {
                new_tags.push(crate::db::models::TagInfo {
                    key: key.clone(),
                    value: value.clone(),
                });
            }
        }

        // Compare with DB tags
        let db_tags = queries::get_tags(&conn, media_id).unwrap_or_default();

        // Simple comparison: sort and compare
        let mut new_sorted: Vec<(String, String)> = new_tags.iter().map(|t| (t.key.clone(), t.value.clone())).collect();
        let mut db_sorted: Vec<(String, String)> = db_tags.iter().map(|t| (t.key.clone(), t.value.clone())).collect();
        new_sorted.sort();
        db_sorted.sort();

        if new_sorted != db_sorted {
            if let Err(e) = queries::set_tags(&conn, media_id, &new_tags) {
                log::error!("Failed to sync tags for {media_id}: {e}");
                continue;
            }
            let _ = queries::mark_media_unprocessed(&conn, media_id);
            count += 1;
        }
    }

    Ok(count)
}

/// Detect processed media that need reprocessing (stale embeddings, missing previews).
/// Marks them as unprocessed so the worker handles them.
/// Returns the number of items marked for reprocessing.
pub fn detect_stale_media(vault_path: &Path) -> Result<u32, String> {
    let db_path = vault_path.join("vault.db");
    let media_dir = vault_path.join("media");
    let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let mut count = 0u32;

    // Find media with stale text embeddings (wrong model)
    let stale_text =
        queries::get_media_ids_needing_text_reembedding(&conn, crate::embedding::MODEL_NAME)
            .map_err(|e| format!("Query failed: {e}"))?;

    for (id, _desc) in &stale_text {
        let _ = queries::mark_media_unprocessed(&conn, id);
        count += 1;
    }

    // Find processed media with missing previews
    let mut stmt = conn
        .prepare("SELECT id, extension, media_type FROM media WHERE processed = 1")
        .map_err(|e| format!("Query failed: {e}"))?;

    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| format!("Query failed: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    for (id, ext, media_type) in &rows {
        if media_type == "audio" {
            continue;
        }
        let media_file = media_dir.join(format!("{id}.{ext}"));
        if !media_file.exists() {
            continue;
        }
        let needs_thumb = thumbnail::get_thumbnail_path(vault_path, id).is_none();
        let needs_preview = (media_type == "video" || media_type == "gif")
            && thumbnail::get_preview_path(vault_path, id).is_none();
        if needs_thumb || needs_preview {
            let _ = queries::mark_media_unprocessed(&conn, id);
            count += 1;
        }
    }

    Ok(count)
}
