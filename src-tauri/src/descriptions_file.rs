use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// The descriptions JSON file maps media_id -> description text.
/// Stored at `{vault_root}/descriptions.json`.

pub fn descriptions_path(vault_path: &Path) -> std::path::PathBuf {
    vault_path.join("descriptions.json")
}

pub fn load(vault_path: &Path) -> HashMap<String, String> {
    let path = descriptions_path(vault_path);
    if !path.exists() {
        return HashMap::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            log::error!("Failed to read descriptions.json: {e}");
            HashMap::new()
        }
    }
}

pub fn save(vault_path: &Path, descriptions: &HashMap<String, String>) -> Result<(), String> {
    let path = descriptions_path(vault_path);
    let json = serde_json::to_string_pretty(descriptions)
        .map_err(|e| format!("Failed to serialize descriptions: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write descriptions.json: {e}"))
}

/// Set a single description and persist to JSON.
pub fn set(vault_path: &Path, media_id: &str, description: &str) -> Result<(), String> {
    let mut descriptions = load(vault_path);
    descriptions.insert(media_id.to_string(), description.to_string());
    save(vault_path, &descriptions)
}

/// Remove a description from the JSON file.
pub fn remove(vault_path: &Path, media_id: &str) -> Result<(), String> {
    let mut descriptions = load(vault_path);
    if descriptions.remove(media_id).is_some() {
        save(vault_path, &descriptions)?;
    }
    Ok(())
}
