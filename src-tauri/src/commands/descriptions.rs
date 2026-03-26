use crate::db::models::DescriptionPageData;
use crate::db::queries;
use crate::embedding;
use crate::state::AppState;
use crate::tags_file;
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn get_description(app_handle: AppHandle, media_id: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;
        queries::get_description(&conn, &media_id).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn set_description(
    app_handle: AppHandle,
    media_id: String,
    description: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;

        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;

        // Update the description tag in DB
        queries::set_description(&conn, &media_id, &description)
            .map_err(|e| format!("Update failed: {e}"))?;

        // Persist to tags.json (backup) — build map from DB tags
        let db_tags = queries::get_tags(&conn, &media_id).unwrap_or_default();
        let mut tag_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for t in &db_tags {
            tag_map.entry(t.key.clone()).or_default().push(t.value.clone());
        }
        tags_file::set_media_tags(&vault_path, &media_id, &tag_map)?;

        // Recompute text embedding from assembled tag text
        if let Ok(Some(tag_text)) = queries::assemble_tag_text(&conn, &media_id) {
            let vector = {
                let mut embedder_guard = state.embedder.lock().unwrap();
                if let Some(embedder) = embedder_guard.as_mut() {
                    embedding::embed_document(embedder, &tag_text).ok()
                } else {
                    None
                }
            };

            if let Some(vec) = vector {
                let bytes = embedding::vector_to_bytes(&vec);
                let _ = queries::insert_embedding(
                    &conn, &media_id, "text", &bytes, embedding::MODEL_NAME,
                );
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

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
