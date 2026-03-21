use crate::db::models::MediaInfo;
use crate::db::queries;
use crate::descriptions_file;
use crate::media as media_util;
use crate::state::AppState;
use crate::thumbnail;
use base64::Engine;
use std::process::Command;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn import_media(state: State<AppState>, app_handle: AppHandle, file_paths: Vec<String>) -> Result<(), String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;

    let handle = app_handle.clone();

    // Everything runs in a background thread — GUI stays responsive
    std::thread::spawn(move || {
        let total = file_paths.len();
        let mut imported_ids: Vec<(String, String, String)> = Vec::new(); // (id, ext, media_type)
        let mut duplicates = 0u32;

        // Phase 1: Copy files, check duplicates, insert into DB
        // Open a dedicated DB connection for this thread
        let db_path = vault_path.join("vault.db");
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to open DB for import: {e}");
                crate::set_loading_status(&handle, "");
                return;
            }
        };

        for (i, file_path) in file_paths.iter().enumerate() {
            crate::set_loading_status(&handle, &format!("Importing {}/{total}", i + 1));

            match media_util::import_file(&vault_path, file_path) {
                Ok(info) => {
                    if let Ok(Some(_)) = queries::find_media_by_checksum(&conn, &info.checksum) {
                        let dup_file = vault_path
                            .join("media")
                            .join(format!("{}.{}", info.id, info.extension));
                        let _ = std::fs::remove_file(dup_file);
                        log::info!("Skipped duplicate: {file_path}");
                        duplicates += 1;
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
                        log::error!("DB insert failed for {file_path}: {e}");
                        continue;
                    }

                    log::info!("Imported via drag-and-drop: {}.{} (type={}, source={})", info.id, info.extension, info.media_type, file_path);
                    imported_ids.push((info.id, info.extension, info.media_type));
                }
                Err(e) => {
                    log::error!("Failed to import {file_path}: {e}");
                }
            }
        }

        // Notify frontend so grid updates with new items
        if !imported_ids.is_empty() {
            let _ = handle.emit("media-changed", imported_ids.len());
        }
        if duplicates > 0 {
            let _ = handle.emit("duplicates-skipped", duplicates);
        }

        // Phase 2: Generate thumbnails/previews
        let preview_items: Vec<_> = imported_ids.iter()
            .filter(|(_, _, mt)| mt != "audio")
            .collect();
        let preview_total = preview_items.len();

        for (i, (id, ext, media_type)) in preview_items.iter().enumerate() {
            let media_file = vault_path.join("media").join(format!("{id}.{ext}"));

            crate::set_loading_status(&handle, &format!("Generating thumbnail {}/{preview_total}", i + 1));
            if let Err(e) = thumbnail::generate_thumbnail(&vault_path, &media_file, id) {
                log::error!("Thumbnail failed for {id}: {e}");
            }

            if *media_type == "video" || *media_type == "gif" {
                crate::set_loading_status(&handle, &format!("Generating preview {}/{preview_total}", i + 1));
                if let Err(e) = thumbnail::generate_animated_preview(&vault_path, &media_file, id) {
                    log::error!("Animated preview failed for {id}: {e}");
                }
            }

            let _ = handle.emit("media-changed", 0);
        }

        crate::set_loading_status(&handle, "");
    });

    Ok(())
}

#[tauri::command]
pub fn get_media_list(
    state: State<AppState>,
    offset: u32,
    limit: u32,
) -> Result<Vec<MediaInfo>, String> {
    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;
    queries::get_media_list(conn, offset, limit).map_err(|e| format!("Query failed: {e}"))
}

#[tauri::command]
pub fn get_media_thumbnail(state: State<AppState>, media_id: String) -> Result<String, String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;

    let path = thumbnail::get_thumbnail_path(&vault_path, &media_id)
        .ok_or("No thumbnail found")?;

    let data = std::fs::read(&path).map_err(|e| format!("Failed to read thumbnail: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

#[tauri::command]
pub fn get_thumbnail_path(state: State<AppState>, media_id: String) -> Result<String, String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;

    let path = thumbnail::get_thumbnail_path(&vault_path, &media_id)
        .ok_or("No thumbnail found")?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_preview_data(state: State<AppState>, media_id: String) -> Result<Option<String>, String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;

    if let Some(path) = thumbnail::get_preview_path(&vault_path, &media_id) {
        let data = std::fs::read(&path).map_err(|e| format!("Failed to read preview: {e}"))?;
        return Ok(Some(base64::engine::general_purpose::STANDARD.encode(&data)));
    }

    Ok(None)
}

#[tauri::command]
pub fn get_preview_file_path(state: State<AppState>, media_id: String) -> Result<Option<String>, String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;

    Ok(thumbnail::get_preview_path(&vault_path, &media_id)
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn get_media_path(state: State<AppState>, media_id: String) -> Result<String, String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;
    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;

    let media = queries::get_media_by_id(conn, &media_id)
        .map_err(|e| format!("Query failed: {e}"))?
        .ok_or("Media not found")?;

    let path = vault_path
        .join("media")
        .join(format!("{}.{}", media.id, media.extension));

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_media(state: State<AppState>, media_id: String) -> Result<(), String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;
    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;

    if let Ok(Some(media)) = queries::get_media_by_id(conn, &media_id) {
        let file_path = vault_path
            .join("media")
            .join(format!("{}.{}", media.id, media.extension));
        let _ = std::fs::remove_file(file_path);
    }

    thumbnail::remove_previews(&vault_path, &media_id);
    let _ = descriptions_file::remove(&vault_path, &media_id);

    queries::delete_media(conn, &media_id).map_err(|e| format!("Delete failed: {e}"))
}

#[tauri::command]
pub fn reveal_media(state: State<AppState>, media_id: String) -> Result<(), String> {
    let path = get_media_path(state, media_id)?;

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new(&path)))
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {e}"))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
pub fn copy_media_file(state: State<AppState>, media_id: String) -> Result<(), String> {
    let path = get_media_path(state, media_id)?;

    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
    clipboard
        .set_text(&path)
        .map_err(|e| format!("Failed to copy to clipboard: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn copy_media_path(state: State<AppState>, media_id: String) -> Result<(), String> {
    let path = get_media_path(state, media_id)?;

    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
    clipboard
        .set_text(&path)
        .map_err(|e| format!("Failed to copy to clipboard: {e}"))?;

    Ok(())
}
