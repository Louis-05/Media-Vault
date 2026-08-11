use crate::db::models::MediaInfo;
use crate::db::queries;
use crate::tags_file;
use crate::media as media_util;
use crate::state::{AppState, WorkerMsg};
use crate::thumbnail;
use base64::Engine;
use std::process::Command;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

/// Helper: get vault_path from state (cloned).
fn get_vault_path(state: &AppState) -> Result<std::path::PathBuf, String> {
    state.vault_path.lock().unwrap().clone().ok_or_else(|| "No vault open".to_string())
}

/// Helper: open a DB connection to the vault.
fn open_db(state: &AppState) -> Result<Connection, String> {
    let vault_path = get_vault_path(state)?;
    Connection::open(vault_path.join("vault.db")).map_err(|e| format!("DB error: {e}"))
}

/// Helper: resolve full media file path.
fn resolve_media_path(state: &AppState, media_id: &str) -> Result<String, String> {
    let vault_path = get_vault_path(state)?;
    let conn = open_db(state)?;
    let media = queries::get_media_by_id(&conn, media_id)
        .map_err(|e| format!("Query failed: {e}"))?
        .ok_or("Media not found")?;
    let path = vault_path.join("media").join(format!("{}.{}", media.id, media.extension));
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_media(app_handle: AppHandle, file_paths: Vec<String>) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let vault_path = get_vault_path(&state)?;
    let worker_tx = state.worker_tx.lock().unwrap().clone();
    let handle = app_handle.clone();

    // Copy files and insert into DB in background — worker handles all processing
    std::thread::spawn(move || {
        let total = file_paths.len();
        let mut imported = 0u32;
        let mut duplicates = 0u32;

        let conn = match Connection::open(vault_path.join("vault.db")) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to open DB for import: {e}");
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
                        info.duration,
                    ) {
                        log::error!("DB insert failed for {file_path}: {e}");
                        continue;
                    }

                    log::info!("Imported: {}.{} (type={}, source={})", info.id, info.extension, info.media_type, file_path);
                    imported += 1;
                }
                Err(e) => {
                    log::error!("Failed to import {file_path}: {e}");
                }
            }
        }

        if duplicates > 0 {
            let _ = handle.emit("duplicates-skipped", duplicates);
        }

        crate::set_loading_status(&handle, "");

        if imported > 0 {
            if let Some(tx) = worker_tx {
                let _ = tx.send(WorkerMsg::Wake);
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn get_media_list(
    app_handle: AppHandle,
    offset: u32,
    limit: u32,
) -> Result<Vec<MediaInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let conn = open_db(&state)?;
        queries::get_media_list(&conn, offset, limit).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_media_thumbnail(app_handle: AppHandle, media_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = get_vault_path(&state)?;
        let path = thumbnail::get_display_thumbnail_path(&vault_path, &media_id)
            .ok_or("No thumbnail found")?;
        let data = std::fs::read(&path).map_err(|e| format!("Failed to read thumbnail: {e}"))?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&data))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_thumbnail_path(app_handle: AppHandle, media_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = get_vault_path(&state)?;
        let path = thumbnail::get_display_thumbnail_path(&vault_path, &media_id)
            .ok_or("No thumbnail found")?;
        Ok(path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_preview_data(app_handle: AppHandle, media_id: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = get_vault_path(&state)?;
        if let Some(path) = thumbnail::get_display_preview_path(&vault_path, &media_id) {
            let data = std::fs::read(&path).map_err(|e| format!("Failed to read preview: {e}"))?;
            return Ok(Some(base64::engine::general_purpose::STANDARD.encode(&data)));
        }
        Ok(None)
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_preview_file_path(app_handle: AppHandle, media_id: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = get_vault_path(&state)?;
        Ok(thumbnail::get_display_preview_path(&vault_path, &media_id)
            .map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_media_path(app_handle: AppHandle, media_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        resolve_media_path(&state, &media_id)
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn delete_media(app_handle: AppHandle, media_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = get_vault_path(&state)?;
        let conn = open_db(&state)?;

        if let Ok(Some(media)) = queries::get_media_by_id(&conn, &media_id) {
            let file_path = vault_path
                .join("media")
                .join(format!("{}.{}", media.id, media.extension));
            let _ = std::fs::remove_file(file_path);
        }

        thumbnail::remove_previews(&vault_path, &media_id);
        let _ = tags_file::remove(&vault_path, &media_id);

        queries::delete_media(&conn, &media_id).map_err(|e| format!("Delete failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn reveal_media(app_handle: AppHandle, media_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let path = resolve_media_path(&state, &media_id)?;

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
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn copy_media_file(app_handle: AppHandle, media_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let path = resolve_media_path(&state, &media_id)?;
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
        clipboard
            .set_text(&path)
            .map_err(|e| format!("Failed to copy to clipboard: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn copy_media_path(app_handle: AppHandle, media_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let path = resolve_media_path(&state, &media_id)?;
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
        clipboard
            .set_text(&path)
            .map_err(|e| format!("Failed to copy to clipboard: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}
