use crate::db::models::SearchResult;
use crate::db::queries;
use crate::embedding;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn search_media(
    state: State<AppState>,
    query: String,
    limit: u32,
) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Lock order: embedder first, then db
    let query_vec = {
        let mut embedder_guard = state.embedder.lock().unwrap();
        let embedder = embedder_guard.as_mut().ok_or("Embedding model not loaded yet")?;
        embedding::embed_query(embedder, &query)?
    };

    let db = state.db.lock().unwrap();
    let conn = db.as_ref().ok_or("No database connection")?;
    let all_embeddings =
        queries::get_all_embeddings(conn).map_err(|e| format!("Query failed: {e}"))?;

    let mut scored: Vec<(String, f32)> = all_embeddings
        .iter()
        .map(|(media_id, blob)| {
            let stored_vec = embedding::bytes_to_vector(blob);
            let score = embedding::cosine_similarity(&query_vec, &stored_vec);
            (media_id.clone(), score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit as usize);

    let mut results = Vec::new();
    for (media_id, score) in scored {
        if let Ok(Some(media)) = queries::get_media_by_id(conn, &media_id) {
            results.push(SearchResult { media, score });
        }
    }

    Ok(results)
}
