use crate::db::models::DescriptionPageData;
use crate::db::queries;
use crate::state::AppState;
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn get_media_for_description(
    app_handle: AppHandle,
    index: u32,
    filter_missing_desc: bool,
    filter_missing_tags: bool,
) -> Result<Option<DescriptionPageData>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;
        queries::get_media_for_description(&conn, index, filter_missing_desc, filter_missing_tags)
            .map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_media_index(
    app_handle: AppHandle,
    media_id: String,
    filter_missing_desc: bool,
    filter_missing_tags: bool,
) -> Result<Option<u32>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;
        queries::get_media_index(&conn, &media_id, filter_missing_desc, filter_missing_tags)
            .map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_filtered_media_ids(
    app_handle: AppHandle,
    filter_missing_desc: bool,
    filter_missing_tags: bool,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;
        queries::get_filtered_media_ids(&conn, filter_missing_desc, filter_missing_tags)
            .map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_media_by_id(
    app_handle: AppHandle,
    media_id: String,
) -> Result<Option<crate::db::models::MediaInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;
        queries::get_media_by_id(&conn, &media_id).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_missing_counts(app_handle: AppHandle) -> Result<(u32, u32, u32), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;
        queries::get_missing_counts(&conn).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}
