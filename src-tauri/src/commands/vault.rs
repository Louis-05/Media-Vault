use crate::db::models::VaultInfo;
use crate::db::{queries, schema};
use crate::state::AppState;
use crate::watcher;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

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

/// Scan for new files, clean ghosts, sync descriptions, generate previews.
fn do_refresh(vault_path: &Path, app_handle: &AppHandle) {
    // Clean up ghost entries
    match watcher::cleanup_missing_files(vault_path) {
        Ok(count) => {
            if count > 0 {
                log::info!("Cleaned up {count} ghost entries");
            }
        }
        Err(e) => log::error!("Failed to clean up missing files: {e}"),
    }

    // Scan for new files
    match watcher::process_new_files(vault_path) {
        Ok(count) => {
            if count > 0 {
                log::info!("Imported {count} new files");
                let _ = app_handle.emit("media-changed", count);
            }
        }
        Err(e) => log::error!("Failed to scan for new files: {e}"),
    }

    // Sync descriptions from JSON
    match watcher::sync_descriptions(vault_path, app_handle) {
        Ok(count) => {
            if count > 0 {
                log::info!("Synced {count} descriptions from JSON");
                let _ = app_handle.emit("media-changed", count);
            }
        }
        Err(e) => log::error!("Failed to sync descriptions: {e}"),
    }

    // Generate missing previews
    match watcher::generate_missing_previews(vault_path, app_handle) {
        Ok(count) => {
            if count > 0 {
                log::info!("Generated {count} missing previews");
                let _ = app_handle.emit("media-changed", count);
            }
        }
        Err(e) => {
            log::error!("Failed to generate missing previews: {e}");
            crate::set_loading_status(app_handle, "");
        }
    }
}

#[tauri::command]
pub fn create_vault(
    state: State<AppState>,
    app_handle: AppHandle,
    path: String,
) -> Result<VaultInfo, String> {
    let vault_path = Path::new(&path);

    let media_dir = vault_path.join("media");
    fs::create_dir_all(&media_dir).map_err(|e| format!("Failed to create media dir: {e}"))?;

    let db_path = vault_path.join("vault.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to create database: {e}"))?;
    schema::initialize_db(&conn).map_err(|e| format!("Failed to initialize database: {e}"))?;

    let info =
        queries::get_vault_info(&conn, &path).map_err(|e| format!("Failed to get info: {e}"))?;

    *state.vault_path.lock().unwrap() = Some(vault_path.to_path_buf());
    *state.db.lock().unwrap() = Some(conn);

    save_last_vault(&path);

    Ok(info)
}

#[tauri::command]
pub fn open_vault(
    state: State<AppState>,
    app_handle: AppHandle,
    path: String,
) -> Result<VaultInfo, String> {
    let vault_path = Path::new(&path);
    let media_dir = vault_path.join("media");

    if !media_dir.exists() {
        return Err("No media folder found — not a vault".to_string());
    }

    // Create DB if missing (descriptions are in JSON, everything else is regenerated)
    let db_path = vault_path.join("vault.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open database: {e}"))?;
    schema::initialize_db(&conn).map_err(|e| format!("Failed to initialize database: {e}"))?;

    *state.vault_path.lock().unwrap() = Some(vault_path.to_path_buf());
    *state.db.lock().unwrap() = Some(conn);

    save_last_vault(&path);

    // Run refresh tasks in background
    let vault_path_owned = vault_path.to_path_buf();
    let handle = app_handle.clone();
    std::thread::spawn(move || {
        do_refresh(&vault_path_owned, &handle);
    });

    let info = {
        let db = state.db.lock().unwrap();
        let conn = db.as_ref().ok_or("No database connection")?;
        queries::get_vault_info(conn, &path).map_err(|e| format!("Failed to get info: {e}"))?
    };

    Ok(info)
}

#[tauri::command]
pub fn close_vault(state: State<AppState>) -> Result<(), String> {
    *state.db.lock().unwrap() = None;
    *state.vault_path.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub fn refresh_vault(state: State<AppState>, app_handle: AppHandle) -> Result<(), String> {
    let vault_path = state
        .vault_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No vault open")?;

    let handle = app_handle.clone();
    std::thread::spawn(move || {
        do_refresh(&vault_path, &handle);
    });

    Ok(())
}
