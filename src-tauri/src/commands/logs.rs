use crate::log_buffer::{self, LogEntry};
use crate::logging;
use std::process::Command;

/// Every log record captured after `seq`. The frontend passes 0 on first load,
/// then the highest `seq` it has seen.
#[tauri::command]
pub fn get_logs_since(seq: u64) -> Vec<LogEntry> {
    log_buffer::snapshot_since(seq)
}

/// Empties the in-memory buffer. The log file on disk is untouched.
#[tauri::command]
pub fn clear_logs() {
    log_buffer::clear();
}

/// Copy the given text (the log lines currently shown) to the clipboard.
#[tauri::command]
pub fn copy_logs(text: String) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
    clipboard
        .set_text(&text)
        .map_err(|e| format!("Failed to copy to clipboard: {e}"))?;
    Ok(())
}

/// Reveal the `logs/` directory in the OS file manager.
#[tauri::command]
pub fn open_logs_folder() -> Result<(), String> {
    let dir = logging::logs_dir();
    let _ = std::fs::create_dir_all(&dir);

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {e}"))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {e}"))?;
    }

    Ok(())
}
