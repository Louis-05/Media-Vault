use crate::db::models::DescriptionPageData;
use crate::db::queries;
use crate::descriptions_file;
use crate::embedding;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_description(state: State<AppState>, media_id: String) -> Result<Option<String>, String> {
    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;
    queries::get_description(conn, &media_id).map_err(|e| format!("Query failed: {e}"))
}

#[tauri::command]
pub fn set_description(
    state: State<AppState>,
    media_id: String,
    description: String,
) -> Result<(), String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;

    // Compute embedding first (lock embedder, then release)
    let vector = {
        let mut embedder_guard = state.embedder.lock().unwrap();
        let embedder = embedder_guard.as_mut().ok_or("Embedding model not loaded yet")?;
        embedding::embed_document(embedder, &description)?
    };
    let bytes = embedding::vector_to_bytes(&vector);

    // Update DB
    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;
    queries::set_description(conn, &media_id, &description)
        .map_err(|e| format!("Update failed: {e}"))?;
    queries::insert_embedding(conn, &media_id, &bytes)
        .map_err(|e| format!("Failed to store embedding: {e}"))?;

    // Persist to JSON file
    descriptions_file::set(&vault_path, &media_id, &description)?;

    Ok(())
}

#[tauri::command]
pub fn get_media_for_description(
    state: State<AppState>,
    index: u32,
    filter_missing: bool,
) -> Result<Option<DescriptionPageData>, String> {
    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;
    queries::get_media_for_description(conn, index, filter_missing)
        .map_err(|e| format!("Query failed: {e}"))
}

#[tauri::command]
pub fn get_media_index(
    state: State<AppState>,
    media_id: String,
    filter_missing: bool,
) -> Result<Option<u32>, String> {
    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;
    queries::get_media_index(conn, &media_id, filter_missing)
        .map_err(|e| format!("Query failed: {e}"))
}
