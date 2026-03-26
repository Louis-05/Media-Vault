use fastembed::TextEmbedding;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;

pub enum WorkerMsg {
    Wake,
    Stop,
    Pause,
    Resume,
}

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub vault_path: Mutex<Option<PathBuf>>,
    pub embedder: Mutex<Option<TextEmbedding>>,
    pub loading_status: Mutex<String>,
    /// Channel to send commands to the worker thread
    pub worker_tx: Mutex<Option<mpsc::Sender<WorkerMsg>>>,
    /// Search priority: signal worker to yield before embedding
    pub search_request_tx: Mutex<Option<mpsc::Sender<()>>>,
    /// Search priority: signal worker that search is done
    pub search_done_tx: Mutex<Option<mpsc::Sender<()>>>,
}
