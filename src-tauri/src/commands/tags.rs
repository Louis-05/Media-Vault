use crate::db::models::{TagInfo, TagKeyInfo};
use crate::db::queries;
use crate::embedding;
use crate::state::AppState;
use crate::tags_file;
use rusqlite::Connection;
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

/// Helper: open DB from vault path in state.
fn open_db(state: &AppState) -> Result<(std::path::PathBuf, Connection), String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;
    let conn = Connection::open(vault_path.join("vault.db"))
        .map_err(|e| format!("DB error: {e}"))?;
    Ok((vault_path, conn))
}

/// Convert Vec<TagInfo> to the HashMap format used by tags_file.
fn tags_to_map(tags: &[TagInfo]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for tag in tags {
        map.entry(tag.key.clone()).or_default().push(tag.value.clone());
    }
    map
}

#[tauri::command]
pub async fn get_media_tags(app_handle: AppHandle, media_id: String) -> Result<Vec<TagInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (_vault_path, conn) = open_db(&state)?;
        queries::get_tags(&conn, &media_id).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn set_media_tags(
    app_handle: AppHandle,
    media_id: String,
    tags: Vec<TagInfo>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (vault_path, conn) = open_db(&state)?;

        // Save tags to DB
        queries::set_tags(&conn, &media_id, &tags)
            .map_err(|e| format!("Failed to save tags: {e}"))?;

        // Persist to tags.json
        let tag_map = tags_to_map(&tags);
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
pub async fn get_all_tag_keys(app_handle: AppHandle) -> Result<Vec<TagKeyInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (_vault_path, conn) = open_db(&state)?;
        queries::get_all_tag_keys(&conn).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn get_tag_values(app_handle: AppHandle, key: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (_vault_path, conn) = open_db(&state)?;
        queries::get_tag_values(&conn, &key).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn create_tag_key(app_handle: AppHandle, key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (_vault_path, conn) = open_db(&state)?;
        queries::create_tag_key(&conn, &key).map_err(|e| format!("Failed to create tag key: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn rename_tag_key(
    app_handle: AppHandle,
    old_key: String,
    new_key: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (vault_path, conn) = open_db(&state)?;
        queries::rename_tag_key(&conn, &old_key, &new_key)
            .map_err(|e| format!("Failed to rename tag: {e}"))?;
        tags_file::rename_tag_key(&vault_path, &old_key, &new_key)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn rename_tag_value(
    app_handle: AppHandle,
    key: String,
    old_value: String,
    new_value: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (vault_path, conn) = open_db(&state)?;

        let affected = queries::rename_tag_value(&conn, &key, &old_value, &new_value)
            .map_err(|e| format!("Failed to rename tag value: {e}"))?;
        tags_file::rename_tag_value(&vault_path, &key, &old_value, &new_value)?;

        // Recompute text embeddings for affected media
        for media_id in &affected {
            if let Ok(Some(tag_text)) = queries::assemble_tag_text(&conn, media_id) {
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
                        &conn, media_id, "text", &bytes, embedding::MODEL_NAME,
                    );
                }
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn delete_tag_key(app_handle: AppHandle, key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let (vault_path, conn) = open_db(&state)?;
        queries::delete_tag_key(&conn, &key)
            .map_err(|e| format!("Failed to delete tag: {e}"))?;
        tags_file::remove_tag_key(&vault_path, &key)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}
