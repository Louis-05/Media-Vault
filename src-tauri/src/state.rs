use fastembed::TextEmbedding;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub vault_path: Mutex<Option<PathBuf>>,
    pub embedder: Mutex<Option<TextEmbedding>>,
    pub loading_status: Mutex<String>,
}
