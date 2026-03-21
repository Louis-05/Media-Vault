use crate::db::{queries, schema};
use crate::descriptions_file;
use crate::embedding;
use crate::media::{detect_media_type, rename_to_uuid};
use crate::state::AppState;
use crate::thumbnail;
use rusqlite::Connection;
use std::path::Path;
use tauri::{AppHandle, Emitter};

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
            let _ = descriptions_file::remove(vault_path, id);
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

/// Scan the media folder for new/unregistered files and import them.
/// Returns the number of newly imported files.
pub fn process_new_files(vault_path: &Path) -> Result<u32, String> {
    let media_dir = vault_path.join("media");
    let db_path = vault_path.join("vault.db");

    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;
    schema::initialize_db(&conn).map_err(|e| format!("Failed to init DB: {e}"))?;

    let entries = std::fs::read_dir(&media_dir)
        .map_err(|e| format!("Failed to read media dir: {e}"))?;

    let mut count = 0u32;

    for entry in entries.flatten() {
        let filename = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        if filename.starts_with('.') {
            continue;
        }

        let ext = Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if detect_media_type(&ext).is_none() {
            continue;
        }

        match rename_to_uuid(vault_path, &filename) {
            Ok(Some(info)) => {
                // Check for duplicate by checksum
                if let Ok(Some(_existing_id)) = queries::find_media_by_checksum(&conn, &info.checksum) {
                    let dup_file = media_dir.join(format!("{}.{}", info.id, info.extension));
                    let _ = std::fs::remove_file(dup_file);
                    log::info!("Skipped duplicate: {filename}");
                    continue;
                }

                if let Err(e) = queries::insert_media(
                    &conn,
                    &info.id,
                    &info.extension,
                    &info.media_type,
                    info.codec.as_deref(),
                    info.file_size,
                    Some(&info.checksum),
                ) {
                    log::error!("Failed to insert {}: {e}", info.id);
                    continue;
                }

                let media_file = media_dir.join(format!("{}.{}", info.id, info.extension));
                if info.media_type != "audio" {
                    let _ = thumbnail::generate_thumbnail(vault_path, &media_file, &info.id);
                    if info.media_type == "video" || info.media_type == "gif" {
                        let _ = thumbnail::generate_animated_preview(vault_path, &media_file, &info.id);
                    }
                }

                log::info!("Imported via refresh (renamed): {}.{} (type={}, original={})", info.id, info.extension, info.media_type, filename);
                count += 1;
            }
            Ok(None) => {
                let stem = Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                if let Ok(None) = queries::get_media_by_id(&conn, stem) {
                    let media_file = media_dir.join(&filename);
                    let checksum = crate::media::compute_checksum(&media_file).ok();

                    // Check for duplicate by checksum
                    if let Some(ref cs) = checksum {
                        if let Ok(Some(_existing_id)) = queries::find_media_by_checksum(&conn, cs) {
                            let _ = std::fs::remove_file(&media_file);
                            log::info!("Skipped duplicate: {filename}");
                            continue;
                        }
                    }

                    let codec = crate::media::detect_codec(&media_file);
                    if let Err(e) = queries::insert_media(
                        &conn,
                        stem,
                        &ext,
                        detect_media_type(&ext).unwrap_or("image"),
                        codec.as_deref(),
                        entry.metadata().ok().map(|m| m.len()),
                        checksum.as_deref(),
                    ) {
                        log::error!("Failed to insert {filename}: {e}");
                        continue;
                    }
                    let media_type = detect_media_type(&ext).unwrap_or("image");
                    if media_type != "audio" {
                        let _ = thumbnail::generate_thumbnail(vault_path, &media_file, stem);
                        if media_type == "video" || media_type == "gif" {
                            let _ = thumbnail::generate_animated_preview(vault_path, &media_file, stem);
                        }
                    }

                    log::info!("Imported via refresh (existing UUID): {filename} (type={media_type})");
                    count += 1;
                }
            }
            Err(e) => {
                log::error!("Failed to process {filename}: {e}");
            }
        }
    }

    Ok(count)
}

/// Generate missing previews for all media already in the DB.
pub fn generate_missing_previews(vault_path: &Path, app_handle: &AppHandle) -> Result<u32, String> {
    let db_path = vault_path.join("vault.db");
    let media_dir = vault_path.join("media");

    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT id, extension, media_type FROM media")
        .map_err(|e| format!("Query failed: {e}"))?;

    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| format!("Query failed: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // Collect items that need preview generation
    let mut to_generate: Vec<(&String, &String, &String)> = Vec::new();
    for (id, ext, media_type) in &rows {
        if media_type == "audio" {
            continue;
        }
        let media_file = media_dir.join(format!("{id}.{ext}"));
        if !media_file.exists() {
            continue;
        }
        let needs_thumb = thumbnail::get_thumbnail_path(vault_path, id).is_none();
        let needs_preview = (media_type == "video" || media_type == "gif") && thumbnail::get_preview_path(vault_path, id).is_none();
        if needs_thumb || needs_preview {
            to_generate.push((id, ext, media_type));
        }
    }

    let total = to_generate.len();
    if total == 0 {
        return Ok(0);
    }

    let mut count = 0u32;

    for (i, (id, ext, media_type)) in to_generate.iter().enumerate() {
        crate::set_loading_status(app_handle, &format!("Generating preview {}/{total}", i + 1));

        let media_file = media_dir.join(format!("{id}.{ext}"));

        if thumbnail::get_thumbnail_path(vault_path, id).is_none() {
            if let Err(e) = thumbnail::generate_thumbnail(vault_path, &media_file, id) {
                log::error!("Failed to generate thumbnail for {id}: {e}");
            } else {
                count += 1;
            }
        }

        if (*media_type == "video" || *media_type == "gif") && thumbnail::get_preview_path(vault_path, id).is_none() {
            if let Err(e) = thumbnail::generate_animated_preview(vault_path, &media_file, id) {
                log::error!("Failed to generate animated preview for {id}: {e}");
            } else {
                count += 1;
            }
        }
    }

    crate::set_loading_status(app_handle, "");

    Ok(count)
}

/// Sync descriptions from JSON file into the DB.
/// If a description in JSON differs from DB (or DB is missing it), update DB and recompute embedding.
pub fn sync_descriptions(vault_path: &Path, app_handle: &AppHandle) -> Result<u32, String> {
    use tauri::Manager;

    let json_descriptions = descriptions_file::load(vault_path);
    if json_descriptions.is_empty() {
        return Ok(0);
    }

    let db_path = vault_path.join("vault.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open DB: {e}"))?;

    // Find descriptions that need syncing
    let mut to_sync: Vec<(String, String)> = Vec::new();
    for (media_id, json_desc) in &json_descriptions {
        let db_desc = queries::get_description(&conn, media_id)
            .unwrap_or(None);

        if db_desc.as_deref() != Some(json_desc.as_str()) {
            to_sync.push((media_id.clone(), json_desc.clone()));
        }
    }

    let total = to_sync.len();
    if total == 0 {
        return Ok(0);
    }

    let state = app_handle.state::<AppState>();

    for (i, (media_id, description)) in to_sync.iter().enumerate() {
        crate::set_loading_status(
            app_handle,
            &format!("Syncing description {}/{total}", i + 1),
        );

        // Update description in DB
        if let Err(e) = queries::set_description(&conn, media_id, description) {
            log::error!("Failed to sync description for {media_id}: {e}");
            continue;
        }

        // Recompute embedding
        let vector = {
            let mut embedder_guard = state.embedder.lock().unwrap();
            if let Some(embedder) = embedder_guard.as_mut() {
                match embedding::embed_document(embedder, description) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        log::error!("Failed to embed description for {media_id}: {e}");
                        None
                    }
                }
            } else {
                log::error!("Embedder not loaded, skipping embedding for {media_id}");
                None
            }
        };

        if let Some(vec) = vector {
            let bytes = embedding::vector_to_bytes(&vec);
            if let Err(e) = queries::insert_embedding(&conn, media_id, &bytes) {
                log::error!("Failed to store embedding for {media_id}: {e}");
            }
        }
    }

    crate::set_loading_status(app_handle, "");

    Ok(total as u32)
}
