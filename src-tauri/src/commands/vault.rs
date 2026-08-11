use crate::db::models::VaultInfo;
use crate::db::{queries, schema};
use crate::state::{AppState, WorkerMsg};
use crate::thumbnail;
use crate::watcher;
use crate::worker;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Preferences {
    #[serde(default)]
    pub last_vault: Option<String>,
    #[serde(default)]
    pub zoom_level: Option<u32>,
}

fn preferences_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("preferences.json")))
}

fn load_preferences() -> Preferences {
    preferences_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_preferences(prefs: &Preferences) {
    if let Some(path) = preferences_path() {
        if let Ok(json) = serde_json::to_string_pretty(prefs) {
            let _ = fs::write(path, json);
        }
    }
}

#[tauri::command]
pub fn get_last_vault() -> Option<String> {
    let prefs = load_preferences();
    let path = prefs.last_vault?;
    if path.is_empty() {
        return None;
    }
    if Path::new(&path).join("media").exists() {
        Some(path)
    } else {
        None
    }
}

#[tauri::command]
pub fn get_zoom_level() -> Option<u32> {
    load_preferences().zoom_level
}

#[tauri::command]
pub fn set_zoom_level(level: u32) {
    let mut prefs = load_preferences();
    prefs.zoom_level = Some(level);
    save_preferences(&prefs);
}

fn save_last_vault(path: &str) {
    let mut prefs = load_preferences();
    prefs.last_vault = Some(path.to_string());
    save_preferences(&prefs);
}

/// Stop the current worker thread if one is running.
fn stop_worker(state: &AppState) {
    if let Some(tx) = state.worker_tx.lock().unwrap().take() {
        let _ = tx.send(WorkerMsg::Stop);
    }
    *state.search_request_tx.lock().unwrap() = None;
    *state.search_done_tx.lock().unwrap() = None;
}

/// Start a new worker thread for the given vault.
fn start_worker(state: &AppState, vault_path: &Path, app_handle: &AppHandle, bootstrap_json: bool) {
    let (worker_tx, worker_rx) = mpsc::channel::<WorkerMsg>();
    let (search_req_tx, search_req_rx) = mpsc::channel::<()>();
    let (search_done_tx, search_done_rx) = mpsc::channel::<()>();

    *state.worker_tx.lock().unwrap() = Some(worker_tx);
    *state.search_request_tx.lock().unwrap() = Some(search_req_tx);
    *state.search_done_tx.lock().unwrap() = Some(search_done_tx);

    let vault_path_owned = vault_path.to_path_buf();
    let handle = app_handle.clone();
    std::thread::spawn(move || {
        worker::run_worker(vault_path_owned, handle, worker_rx, search_req_rx, search_done_rx, bootstrap_json);
    });
}

/// Deep refresh: cleanup ghosts, detect stale media.
/// File scanning is handled by the worker thread (one at a time).
fn do_deep_refresh(vault_path: &Path, app_handle: &AppHandle) {
    // Drop placeholders written in-band by older builds so the items below are
    // seen as "missing preview" and get regenerated. Only meaningful when
    // ffmpeg is actually usable — otherwise we'd delete the only thing the UI
    // has to show and gain nothing.
    if thumbnail::ffmpeg_available() {
        thumbnail::repair_placeholder_previews(vault_path);
    }

    match watcher::cleanup_missing_files(vault_path) {
        Ok(count) => {
            if count > 0 {
                log::info!("Cleaned up {count} ghost entries");
                let _ = app_handle.emit("media-changed", count);
            }
        }
        Err(e) => log::error!("Failed to clean up missing files: {e}"),
    }

    match watcher::detect_stale_media(vault_path) {
        Ok(count) => {
            if count > 0 {
                log::info!("Found {count} stale media items to reprocess");
            }
        }
        Err(e) => log::error!("Failed to detect stale media: {e}"),
    }
}


#[tauri::command]
pub async fn create_vault(
    app_handle: AppHandle,
    path: String,
) -> Result<VaultInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = Path::new(&path);

        let media_dir = vault_path.join("media");
        fs::create_dir_all(&media_dir).map_err(|e| format!("Failed to create media dir: {e}"))?;

        let db_path = vault_path.join("vault.db");
        let conn = Connection::open(&db_path).map_err(|e| format!("Failed to create database: {e}"))?;
        schema::initialize_db(&conn).map_err(|e| format!("Failed to initialize database: {e}"))?;

        let info = queries::get_vault_info(&conn, &path).map_err(|e| format!("Failed to get info: {e}"))?;

        *state.vault_path.lock().unwrap() = Some(vault_path.to_path_buf());
        *state.db.lock().unwrap() = Some(conn);

        save_last_vault(&path);
        start_worker(&state, vault_path, &app_handle, false);

        Ok(info)
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn open_vault(
    app_handle: AppHandle,
    path: String,
) -> Result<VaultInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = Path::new(&path);
        let media_dir = vault_path.join("media");

        if !media_dir.exists() {
            return Err("No media folder found — not a vault".to_string());
        }

        let db_path = vault_path.join("vault.db");
        let db_is_fresh = !db_path.exists();
        let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open database: {e}"))?;
        schema::initialize_db(&conn).map_err(|e| format!("Failed to initialize database: {e}"))?;

        let info = queries::get_vault_info(&conn, &path).map_err(|e| format!("Failed to get info: {e}"))?;

        *state.vault_path.lock().unwrap() = Some(vault_path.to_path_buf());
        *state.db.lock().unwrap() = Some(conn);

        save_last_vault(&path);
        start_worker(&state, vault_path, &app_handle, db_is_fresh);

        // Run deep refresh in background, then wake the worker
        let vault_path_owned = vault_path.to_path_buf();
        let handle = app_handle.clone();
        let worker_tx = state.worker_tx.lock().unwrap().clone();
        std::thread::spawn(move || {
            do_deep_refresh(&vault_path_owned, &handle);
            if let Some(tx) = worker_tx {
                let _ = tx.send(WorkerMsg::Wake);
            }
        });

        Ok(info)
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn close_vault(app_handle: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        stop_worker(&state);
        *state.db.lock().unwrap() = None;
        *state.vault_path.lock().unwrap() = None;
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
pub async fn deep_refresh(app_handle: AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
    let worker_tx = state.worker_tx.lock().unwrap().clone();

    let handle = app_handle.clone();
    std::thread::spawn(move || {
        do_deep_refresh(&vault_path, &handle);
        if let Some(tx) = worker_tx {
            let _ = tx.send(WorkerMsg::Wake);
        }
    });

    Ok(())
}

/// Delete all existing thumbnails/previews and regenerate them for every processed
/// media item. Pauses the worker for the duration so the two don't fight.
#[tauri::command]
pub async fn regenerate_all_previews(app_handle: AppHandle) -> Result<(), String> {
    // Preflight: this command deletes every existing preview before rebuilding
    // it, so without a working ffmpeg it would destroy the whole set in seconds.
    if !thumbnail::ffmpeg_available() {
        return Err(format!("Cannot regenerate previews. {}", thumbnail::FFMPEG_MISSING));
    }

    let state = app_handle.state::<AppState>();
    let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
    let worker_tx = state.worker_tx.lock().unwrap().clone();

    // Pause the worker for the duration
    if let Some(ref tx) = worker_tx {
        let _ = tx.send(WorkerMsg::Pause);
    }

    let handle = app_handle.clone();
    std::thread::spawn(move || {
        let conn = match Connection::open(vault_path.join("vault.db")) {
            Ok(c) => c,
            Err(e) => {
                log::error!("regenerate_all_previews: failed to open db: {e}");
                if let Some(tx) = worker_tx {
                    let _ = tx.send(WorkerMsg::Resume);
                }
                return;
            }
        };

        let media = match queries::get_all_processed_media(&conn) {
            Ok(m) => m,
            Err(e) => {
                log::error!("regenerate_all_previews: query failed: {e}");
                if let Some(tx) = worker_tx {
                    let _ = tx.send(WorkerMsg::Resume);
                }
                return;
            }
        };

        let total = media.len();
        log::info!("Regenerating thumbnails/previews for {total} media items");
        let media_dir = vault_path.join("media");

        for (i, (id, ext, media_type)) in media.iter().enumerate() {
            crate::set_loading_status(
                &handle,
                &format!("Regenerating previews {}/{total}", i + 1),
            );

            // Delete existing previews so generators don't no-op
            thumbnail::remove_previews(&vault_path, id);

            let media_file = media_dir.join(format!("{id}.{ext}"));
            if !media_file.exists() {
                log::warn!("Media file missing during regen: {id}.{ext}");
                continue;
            }

            if media_type != "audio" {
                if let Err(e) = thumbnail::generate_thumbnail(&vault_path, &media_file, id) {
                    log::error!("Failed to regenerate thumbnail for {id}: {e}");
                }
            }
            if media_type == "video" || media_type == "gif" {
                if let Err(e) = thumbnail::generate_animated_preview(&vault_path, &media_file, id) {
                    log::error!("Failed to regenerate animated preview for {id}: {e}");
                }
            }
        }

        crate::set_loading_status(&handle, "");
        let _ = handle.emit("media-changed", total as u32);
        log::info!("Finished regenerating previews for {total} media items");

        if let Some(tx) = worker_tx {
            let _ = tx.send(WorkerMsg::Resume);
        }
    });

    Ok(())
}

/// Whether ffmpeg could be located and executed. The frontend uses this to warn
/// that thumbnails and previews cannot be generated.
#[tauri::command]
pub fn is_ffmpeg_available() -> bool {
    thumbnail::ffmpeg_available()
}

#[tauri::command]
pub fn pause_processing(state: State<AppState>) -> Result<(), String> {
    if let Some(ref tx) = *state.worker_tx.lock().unwrap() {
        let _ = tx.send(WorkerMsg::Pause);
    }
    Ok(())
}

#[tauri::command]
pub fn resume_processing(state: State<AppState>) -> Result<(), String> {
    if let Some(ref tx) = *state.worker_tx.lock().unwrap() {
        let _ = tx.send(WorkerMsg::Resume);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_processed_count(app_handle: AppHandle) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let vault_path = state.vault_path.lock().unwrap().clone().ok_or("No vault open")?;
        let conn = Connection::open(vault_path.join("vault.db"))
            .map_err(|e| format!("DB error: {e}"))?;
        queries::get_processed_count(&conn).map_err(|e| format!("Query failed: {e}"))
    })
    .await
    .map_err(|e| format!("{e}"))?
}
