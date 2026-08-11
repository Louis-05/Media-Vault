use crate::db::queries;
use crate::embedding;
use crate::tags_file;
use crate::media::{compute_checksum, detect_codec, detect_media_type, rename_to_uuid};
use crate::state::{AppState, WorkerMsg};
use crate::thumbnail;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tauri::{AppHandle, Emitter, Manager};

/// Yield the embedder to a pending search operation.
fn yield_for_search(
    search_request_rx: &mpsc::Receiver<()>,
    search_done_rx: &mpsc::Receiver<()>,
) {
    while search_request_rx.try_recv().is_ok() {
        let _ = search_done_rx.recv();
    }
}

/// Check for Stop/Pause messages without blocking.
/// Returns (should_stop, should_pause).
fn check_commands(rx: &mpsc::Receiver<WorkerMsg>) -> (bool, bool) {
    match rx.try_recv() {
        Ok(WorkerMsg::Stop) => (true, false),
        Ok(WorkerMsg::Pause) => (false, true),
        _ => (false, false),
    }
}

/// Process a single media item through the full pipeline:
/// thumbnail → animated preview → sync description → text embedding → mark processed → emit
#[allow(clippy::too_many_arguments)]
fn process_media(
    conn: &Connection,
    vault_path: &Path,
    app_handle: &AppHandle,
    id: &str,
    ext: &str,
    media_type: &str,
    search_request_rx: &mpsc::Receiver<()>,
    search_done_rx: &mpsc::Receiver<()>,
) {
    let media_dir = vault_path.join("media");
    let media_file = media_dir.join(format!("{id}.{ext}"));

    if !media_file.exists() {
        log::warn!("Media file missing during processing: {id}.{ext}");
        let _ = queries::mark_media_processed(conn, id);
        return;
    }

    // Step 1: Generate thumbnail
    if media_type != "audio"
        && thumbnail::get_thumbnail_path(vault_path, id).is_none()
        && !thumbnail::has_failed_thumbnail(vault_path, id)
    {
        if let Err(e) = thumbnail::generate_thumbnail(vault_path, &media_file, id) {
            log::error!("Failed to generate thumbnail for {id}: {e}");
        }
    }

    // Step 2: Generate animated preview (video/gif)
    if (media_type == "video" || media_type == "gif")
        && thumbnail::get_preview_path(vault_path, id).is_none()
        && !thumbnail::has_failed_preview(vault_path, id)
    {
        if let Err(e) = thumbnail::generate_animated_preview(vault_path, &media_file, id) {
            log::error!("Failed to generate animated preview for {id}: {e}");
        }
    }

    // Ensure media_type tag exists
    let _ = conn.execute(
        "INSERT OR IGNORE INTO tags (media_id, key, value) VALUES (?1, 'media_type', ?2)",
        rusqlite::params![id, media_type],
    );

    // Step 4: Generate text embedding from assembled tag text
    if let Ok(Some(tag_text)) = queries::assemble_tag_text(conn, id) {
        yield_for_search(search_request_rx, search_done_rx);

        let state = app_handle.state::<AppState>();
        let vector = {
            let mut embedder_guard = state.embedder.lock().unwrap();
            if let Some(embedder) = embedder_guard.as_mut() {
                embedding::embed_document(embedder, &tag_text).ok()
            } else {
                None
            }
        };

        if let Some(vec) = vector {
            let bytes = embedding::vector_to_bytes(&vec);
            if let Err(e) =
                queries::insert_embedding(conn, id, "text", &bytes, embedding::MODEL_NAME)
            {
                log::error!("Failed to store text embedding for {id}: {e}");
            }
        }
    }

    // Step 5: Mark as processed
    if let Err(e) = queries::mark_media_processed(conn, id) {
        log::error!("Failed to mark {id} as processed: {e}");
        return;
    }

    // Step 6: Emit event with full MediaInfo
    if let Ok(Some(media_info)) = queries::get_media_by_id(conn, id) {
        let _ = app_handle.emit("media-processed", &media_info);
    }
}

/// Scan the media/ folder for one new unregistered file and import it into the DB
/// as unprocessed. Returns Some((id, ext, media_type)) if a file was imported, None if
/// no new files found.
fn scan_one_new_file(
    conn: &Connection,
    vault_path: &Path,
) -> Option<(String, String, String)> {
    let media_dir = vault_path.join("media");
    let entries = match std::fs::read_dir(&media_dir) {
        Ok(e) => e,
        Err(e) => {
            log::error!("Failed to read media dir: {e}");
            return None;
        }
    };

    for entry in entries.flatten() {
        let filename = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        if filename.starts_with('.') {
            continue;
        }

        let ext = Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let media_type_str = match detect_media_type(&ext) {
            Some(mt) => mt,
            None => continue,
        };

        match rename_to_uuid(vault_path, &filename) {
            Ok(Some(info)) => {
                // Check for duplicate by checksum
                if let Ok(Some(_)) = queries::find_media_by_checksum(conn, &info.checksum) {
                    let dup_file = media_dir.join(format!("{}.{}", info.id, info.extension));
                    let _ = std::fs::remove_file(dup_file);
                    log::info!("Skipped duplicate: {filename}");
                    continue;
                }

                if let Err(e) = queries::insert_media(
                    conn,
                    &info.id,
                    &info.extension,
                    &info.media_type,
                    info.codec.as_deref(),
                    info.file_size,
                    Some(&info.checksum),
                    info.duration,
                ) {
                    log::error!("Failed to insert {}: {e}", info.id);
                    continue;
                }

                log::info!(
                    "Imported (renamed): {}.{} (type={}, original={})",
                    info.id, info.extension, info.media_type, filename
                );
                return Some((info.id, info.extension, info.media_type));
            }
            Ok(None) => {
                // Already UUID-named — check if registered in DB
                let stem = Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                if let Ok(None) = queries::get_media_by_id(conn, stem) {
                    let media_file = media_dir.join(&filename);
                    let checksum = compute_checksum(&media_file).ok();

                    if let Some(ref cs) = checksum {
                        if let Ok(Some(_)) = queries::find_media_by_checksum(conn, cs) {
                            let _ = std::fs::remove_file(&media_file);
                            log::info!("Skipped duplicate: {filename}");
                            continue;
                        }
                    }

                    let codec = detect_codec(&media_file);
                    let duration = if media_type_str == "video" || media_type_str == "audio" || media_type_str == "gif" {
                        crate::media::detect_duration(&media_file)
                    } else {
                        None
                    };
                    if let Err(e) = queries::insert_media(
                        conn,
                        stem,
                        &ext,
                        media_type_str,
                        codec.as_deref(),
                        entry.metadata().ok().map(|m| m.len()),
                        checksum.as_deref(),
                        duration,
                    ) {
                        log::error!("Failed to insert {filename}: {e}");
                        continue;
                    }

                    log::info!("Imported (existing UUID): {filename} (type={media_type_str})");
                    return Some((stem.to_string(), ext, media_type_str.to_string()));
                }
            }
            Err(e) => {
                log::error!("Failed to process {filename}: {e}");
            }
        }
    }

    None
}

/// Main worker loop. Runs in a dedicated background thread.
/// Handles both processing unprocessed DB items AND scanning for new files.
pub fn run_worker(
    vault_path: PathBuf,
    app_handle: AppHandle,
    rx: mpsc::Receiver<WorkerMsg>,
    search_request_rx: mpsc::Receiver<()>,
    search_done_rx: mpsc::Receiver<()>,
    bootstrap_json: bool,
) {
    let db_path = vault_path.join("vault.db");
    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Worker failed to open DB: {e}");
            return;
        }
    };
    let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");

    // Load tags.json once if bootstrapping from a fresh DB
    let json_tags = if bootstrap_json {
        let tags = tags_file::load(&vault_path);
        if !tags.is_empty() {
            log::info!("Loaded tags.json for bootstrap ({} media entries)", tags.len());
        }
        Some(tags)
    } else {
        None
    };

    let mut paused = false;

    loop {
        if paused {
            let remaining = queries::get_unprocessed_count(&conn).unwrap_or(0);
            crate::set_loading_status(
                &app_handle,
                &format!("Processing paused ({remaining} remaining)"),
            );

            match rx.recv() {
                Ok(WorkerMsg::Resume) => {
                    paused = false;
                    continue;
                }
                Ok(WorkerMsg::Stop) => return,
                Ok(WorkerMsg::Wake) | Ok(WorkerMsg::Pause) => continue,
                Err(_) => return,
            }
        }

        // Phase 1: Process any unprocessed items already in DB
        let items = match queries::get_unprocessed_media(&conn) {
            Ok(items) => items,
            Err(e) => {
                log::error!("Worker query failed: {e}");
                return;
            }
        };

        if !items.is_empty() {
            let total = items.len();
            for (i, (id, ext, media_type)) in items.iter().enumerate() {
                let (should_stop, should_pause) = check_commands(&rx);
                if should_stop {
                    return;
                }
                if should_pause {
                    paused = true;
                    break;
                }

                crate::set_loading_status(
                    &app_handle,
                    &format!("Processing media {}/{total}", i + 1),
                );

                process_media(
                    &conn, &vault_path, &app_handle, id, ext, media_type,
                    &search_request_rx, &search_done_rx,
                );
            }

            if paused {
                continue;
            }
            // Loop back to check for more (deep refresh may have added items)
            continue;
        }

        // Phase 2: Scan for new files on disk one at a time, import + process each
        loop {
            let (should_stop, should_pause) = check_commands(&rx);
            if should_stop {
                return;
            }
            if should_pause {
                paused = true;
                break;
            }

            if let Some((id, ext, media_type)) = scan_one_new_file(&conn, &vault_path) {
                crate::set_loading_status(&app_handle, "Processing new media...");

                // Bootstrap tags from JSON if this is a fresh DB
                if let Some(ref all_json_tags) = json_tags {
                    if let Some(media_json_tags) = all_json_tags.get(&id) {
                        let mut db_tags: Vec<crate::db::models::TagInfo> = Vec::new();
                        for (key, values) in media_json_tags {
                            for value in values {
                                db_tags.push(crate::db::models::TagInfo {
                                    key: key.clone(),
                                    value: value.clone(),
                                });
                            }
                        }
                        if let Err(e) = queries::set_tags(&conn, &id, &db_tags) {
                            log::error!("Failed to bootstrap tags for {id}: {e}");
                        }
                    }
                }

                process_media(
                    &conn, &vault_path, &app_handle, &id, &ext, &media_type,
                    &search_request_rx, &search_done_rx,
                );
            } else {
                break; // No more new files
            }
        }

        if paused {
            continue;
        }

        // Nothing to do — clear status and wait
        crate::set_loading_status(&app_handle, "");

        match rx.recv() {
            Ok(WorkerMsg::Wake) => continue,
            Ok(WorkerMsg::Stop) => return,
            Ok(WorkerMsg::Pause) => {
                paused = true;
                continue;
            }
            Ok(WorkerMsg::Resume) => continue,
            Err(_) => return,
        }
    }
}
