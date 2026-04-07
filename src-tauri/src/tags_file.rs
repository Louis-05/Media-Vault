use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Tags JSON file maps media_id -> { tag_key -> [values] }.
/// Stored at `{vault_root}/tags.json`.
pub type MediaTags = HashMap<String, Vec<String>>;
pub type AllTags = HashMap<String, MediaTags>;

fn tags_path(vault_path: &Path) -> std::path::PathBuf {
    vault_path.join("tags.json")
}

fn legacy_path(vault_path: &Path) -> std::path::PathBuf {
    vault_path.join("descriptions.json")
}

/// Load tags from tags.json. Falls back to descriptions.json (old format) if tags.json
/// doesn't exist, converting string values to {"description": [value]}.
pub fn load(vault_path: &Path) -> AllTags {
    let path = tags_path(vault_path);
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => return serde_json::from_str(&content).unwrap_or_default(),
            Err(e) => {
                log::error!("Failed to read tags.json: {e}");
                return HashMap::new();
            }
        }
    }

    // Backwards compat: try old descriptions.json
    let legacy = legacy_path(vault_path);
    if legacy.exists() {
        if let Ok(content) = fs::read_to_string(&legacy) {
            if let Ok(old_map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                let mut tags: AllTags = HashMap::new();
                for (media_id, desc) in old_map {
                    let mut media_tags = MediaTags::new();
                    media_tags.insert("description".to_string(), vec![desc]);
                    tags.insert(media_id, media_tags);
                }
                // Save as new format
                let _ = save(vault_path, &tags);
                return tags;
            }
        }
    }

    HashMap::new()
}

pub fn save(vault_path: &Path, tags: &AllTags) -> Result<(), String> {
    let path = tags_path(vault_path);
    let json = serde_json::to_string_pretty(tags)
        .map_err(|e| format!("Failed to serialize tags: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write tags.json: {e}"))
}

/// Set all tags for a single media and persist.
pub fn set_media_tags(vault_path: &Path, media_id: &str, media_tags: &MediaTags) -> Result<(), String> {
    let mut all = load(vault_path);
    all.insert(media_id.to_string(), media_tags.clone());
    save(vault_path, &all)
}

/// Remove all tags for a media from the JSON file.
pub fn remove(vault_path: &Path, media_id: &str) -> Result<(), String> {
    let mut all = load(vault_path);
    if all.remove(media_id).is_some() {
        save(vault_path, &all)?;
    }
    Ok(())
}

/// Remove a specific tag key from all media in the JSON file.
pub fn remove_tag_key(vault_path: &Path, key: &str) -> Result<(), String> {
    let mut all = load(vault_path);
    let mut changed = false;
    for media_tags in all.values_mut() {
        if media_tags.remove(key).is_some() {
            changed = true;
        }
    }
    if changed {
        save(vault_path, &all)?;
    }
    Ok(())
}

/// Rename a value within a tag key across all media in the JSON file.
/// If the new value already exists for the same key on a media, the duplicate
/// is collapsed.
pub fn rename_tag_value(
    vault_path: &Path,
    key: &str,
    old_value: &str,
    new_value: &str,
) -> Result<(), String> {
    let mut all = load(vault_path);
    let mut changed = false;
    for media_tags in all.values_mut() {
        if let Some(values) = media_tags.get_mut(key) {
            let mut updated = Vec::with_capacity(values.len());
            let mut had_old = false;
            for v in values.drain(..) {
                if v == old_value {
                    had_old = true;
                    if !updated.iter().any(|u: &String| u == new_value) {
                        updated.push(new_value.to_string());
                    }
                } else if v == new_value {
                    if !updated.iter().any(|u: &String| u == new_value) {
                        updated.push(v);
                    }
                } else {
                    updated.push(v);
                }
            }
            *values = updated;
            if had_old {
                changed = true;
            }
        }
    }
    if changed {
        save(vault_path, &all)?;
    }
    Ok(())
}

/// Rename a tag key across all media in the JSON file.
pub fn rename_tag_key(vault_path: &Path, old_key: &str, new_key: &str) -> Result<(), String> {
    let mut all = load(vault_path);
    let mut changed = false;
    for media_tags in all.values_mut() {
        if let Some(values) = media_tags.remove(old_key) {
            media_tags.insert(new_key.to_string(), values);
            changed = true;
        }
    }
    if changed {
        save(vault_path, &all)?;
    }
    Ok(())
}
