use std::collections::HashMap;
use crate::db::models::SearchResult;
use crate::db::queries;
use crate::embedding;
use crate::state::AppState;
use rusqlite::Connection;
use tauri::{Manager, AppHandle};

#[tauri::command]
pub async fn search_media(
    app_handle: AppHandle,
    query: String,
    offset: u32,
    limit: u32,
) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Run on a background thread to avoid blocking the UI
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();

        // Signal worker to yield the embedder
        if let Some(ref tx) = *state.search_request_tx.lock().unwrap() {
            let _ = tx.send(());
        }

        // Lock embedder, embed query, release
        let query_vec = {
            let mut embedder_guard = state.embedder.lock().unwrap();
            let embedder = embedder_guard.as_mut().ok_or("Embedding model not loaded yet")?;
            embedding::embed_query(embedder, &query)?
        };

        // Signal worker that search is done with embedder
        if let Some(ref tx) = *state.search_done_tx.lock().unwrap() {
            let _ = tx.send(());
        }

        // Use a dedicated DB connection to avoid blocking other threads
        let vault_path = state.vault_path.lock().unwrap()
            .clone()
            .ok_or("No vault open")?;
        let db_path = vault_path.join("vault.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open DB: {e}"))?;

        let all_embeddings =
            queries::get_all_embeddings(&conn).map_err(|e| format!("Query failed: {e}"))?;

        // Group scores by media_id, taking the max across embedding types (image, text)
        let mut best_scores: HashMap<String, f32> = HashMap::new();

        for (media_id, _embedding_type, blob) in &all_embeddings {
            let stored_vec = embedding::bytes_to_vector(blob);
            let score = embedding::cosine_similarity(&query_vec, &stored_vec);
            let entry = best_scores.entry(media_id.clone()).or_insert(0.0);
            if score > *entry {
                *entry = score;
            }
        }

        let mut scored: Vec<(String, f32)> = best_scores.into_iter().collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply offset + limit pagination
        let page: Vec<_> = scored
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();

        let mut results = Vec::new();
        for (media_id, score) in page {
            if let Ok(Some(media)) = queries::get_media_by_id(&conn, &media_id) {
                results.push(SearchResult { media, score });
            }
        }

        Ok(results)
    })
    .await
    .map_err(|e| format!("Search task failed: {e}"))?
}
