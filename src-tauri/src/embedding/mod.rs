use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::path::PathBuf;

pub const MODEL_NAME: &str = "EmbeddingGemma300M";

/// Return a `model` directory next to the running executable.
fn model_dir() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("model")))
        .unwrap_or_else(|| PathBuf::from("model"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn create_embedder() -> Result<TextEmbedding, String> {
    let options = TextInitOptions::new(EmbeddingModel::EmbeddingGemma300M)
        .with_cache_dir(model_dir());
    TextEmbedding::try_new(options)
        .map_err(|e| format!("Failed to initialize Gemma embedding model: {e}"))
}

/// Embed a document/description for storage.
/// Uses the EmbeddingGemma document prompt format.
pub fn embed_document(embedder: &mut TextEmbedding, text: &str) -> Result<Vec<f32>, String> {
    let prompted = format!("title: none | text: {text}");
    let results = embedder
        .embed(&[&prompted], None)
        .map_err(|e| format!("Failed to embed document: {e}"))?;

    results
        .into_iter()
        .next()
        .ok_or_else(|| "No embedding result".to_string())
}

/// Embed a search query.
/// Uses the EmbeddingGemma query prompt format.
pub fn embed_query(embedder: &mut TextEmbedding, text: &str) -> Result<Vec<f32>, String> {
    let prompted = format!("task: search result | query: {text}");
    let results = embedder
        .embed(&[&prompted], None)
        .map_err(|e| format!("Failed to embed query: {e}"))?;

    results
        .into_iter()
        .next()
        .ok_or_else(|| "No embedding result".to_string())
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

/// Serialize f32 vector to bytes (little-endian) for storage in SQLite BLOB
pub fn vector_to_bytes(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize bytes (little-endian) back to f32 vector
pub fn bytes_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
