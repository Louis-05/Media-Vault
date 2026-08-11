mod commands;
mod db;
mod tags_file;
mod embedding;
mod log_buffer;
mod logging;
mod media;
mod state;
mod thumbnail;
pub mod watcher;
pub mod worker;

use state::AppState;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// Update the loading status in state and emit an event to the frontend.
pub fn set_loading_status(app_handle: &AppHandle, status: &str) {
    let state = app_handle.state::<AppState>();
    *state.loading_status.lock().unwrap() = status.to_string();
    let _ = app_handle.emit("loading-status", status);
}

#[tauri::command]
fn get_loading_status(state: tauri::State<AppState>) -> String {
    state.loading_status.lock().unwrap().clone()
}

#[tauri::command]
fn get_build_info() -> (String, String) {
    (
        env!("CARGO_PKG_VERSION").to_string(),
        env!("BUILD_DATETIME").to_string(),
    )
}

pub fn run() {
    logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .register_uri_scheme_protocol("media", |_ctx, request| {
            use std::io::{Read, Seek, SeekFrom};
            use tauri::http::Response;

            let uri = request.uri().to_string();
            let path = uri
                .strip_prefix("media://localhost")
                .or_else(|| uri.strip_prefix("http://media.localhost"))
                .unwrap_or("");
            let path = percent_encoding::percent_decode_str(path)
                .decode_utf8_lossy()
                .to_string();

            // On Windows, strip leading "/" from paths like "/C:/Users/..."
            #[cfg(windows)]
            let path = {
                let p = path.as_str();
                if p.len() >= 3 && p.starts_with('/') && p.as_bytes()[2] == b':' {
                    p[1..].to_string()
                } else {
                    path
                }
            };

            let file_path = std::path::Path::new(&path);
            if !file_path.exists() {
                log::warn!("[media://] 404 — file not found: {path}");
                return Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap();
            }

            let mut file = match std::fs::File::open(file_path) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("[media://] 500 — failed to open: {e}");
                    return Response::builder()
                        .status(500)
                        .body(Vec::new())
                        .unwrap();
                }
            };

            let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
            let mime = mime_guess::from_path(file_path)
                .first_or_octet_stream()
                .to_string();

            // Handle Range requests for video seeking
            let range_header = request.headers().get("range")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            if let Some(range) = range_header {
                if file_size == 0 {
                    return Response::builder()
                        .status(416)
                        .header("Content-Range", format!("bytes */{file_size}"))
                        .body(Vec::new())
                        .unwrap();
                }

                // Parse "bytes=START-END" or "bytes=START-"
                let range = range.strip_prefix("bytes=").unwrap_or(&range);
                let parts: Vec<&str> = range.split('-').collect();
                let start: u64 = parts.first()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let end: u64 = parts.get(1)
                    .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
                    .unwrap_or(file_size - 1)
                    .min(file_size - 1);

                let length = end - start + 1;
                let _ = file.seek(SeekFrom::Start(start));
                let mut buf = vec![0u8; length as usize];
                let _ = file.read_exact(&mut buf);

                Response::builder()
                    .status(206)
                    .header("Content-Type", &mime)
                    .header("Content-Length", length)
                    .header("Content-Range", format!("bytes {start}-{end}/{file_size}"))
                    .header("Accept-Ranges", "bytes")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(buf)
                    .unwrap()
            } else {
                let mut data = Vec::with_capacity(file_size as usize);
                let _ = file.read_to_end(&mut data);

                Response::builder()
                    .status(200)
                    .header("Content-Type", &mime)
                    .header("Content-Length", file_size)
                    .header("Accept-Ranges", "bytes")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(data)
                    .unwrap()
            }
        })
        .manage(AppState {
            db: Mutex::new(None),
            vault_path: Mutex::new(None),
            embedder: Mutex::new(None),
            loading_status: Mutex::new("Loading embedding model...".to_string()),
            worker_tx: Mutex::new(None),
            search_request_tx: Mutex::new(None),
            search_done_tx: Mutex::new(None),
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // Load embedding model in a background thread
            std::thread::spawn(move || {
                set_loading_status(&handle, "Loading embedding model...");

                match embedding::create_embedder() {
                    Ok(embedder) => {
                        let state = handle.state::<AppState>();
                        *state.embedder.lock().unwrap() = Some(embedder);
                        set_loading_status(&handle, "");
                        log::info!("Embedding model loaded successfully");
                    }
                    Err(e) => {
                        log::error!("Failed to load embedding model: {e}");
                        set_loading_status(&handle, "");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_loading_status,
            get_build_info,
            commands::vault::create_vault,
            commands::vault::open_vault,
            commands::vault::close_vault,
            commands::vault::get_last_vault,
            commands::vault::deep_refresh,
            commands::vault::regenerate_all_previews,
            commands::vault::is_ffmpeg_available,
            commands::vault::pause_processing,
            commands::vault::resume_processing,
            commands::vault::get_processed_count,
            commands::vault::get_zoom_level,
            commands::vault::set_zoom_level,
            commands::media::import_media,
            commands::media::get_media_list,
            commands::media::get_media_thumbnail,
            commands::media::get_thumbnail_path,
            commands::media::get_preview_data,
            commands::media::get_preview_file_path,
            commands::media::get_media_path,
            commands::media::delete_media,
            commands::media::reveal_media,
            commands::media::copy_media_file,
            commands::media::copy_media_path,
            commands::descriptions::get_media_for_description,
            commands::descriptions::get_media_index,
            commands::descriptions::get_filtered_media_ids,
            commands::descriptions::get_media_by_id,
            commands::descriptions::get_missing_counts,
            commands::search::search_media,
            commands::tags::get_media_tags,
            commands::tags::set_media_tags,
            commands::tags::get_all_tag_keys,
            commands::tags::get_tag_values,
            commands::tags::create_tag_key,
            commands::tags::rename_tag_key,
            commands::tags::rename_tag_value,
            commands::tags::delete_tag_key,
            commands::logs::get_logs_since,
            commands::logs::clear_logs,
            commands::logs::copy_logs,
            commands::logs::open_logs_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
