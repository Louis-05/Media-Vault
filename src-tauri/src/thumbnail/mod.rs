use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Pre-generated 192x192 WebP placeholder with red "Failed to generate preview" text.
const ERROR_THUMBNAIL: &[u8] = include_bytes!("../../assets/error_thumbnail.webp");

/// Message returned when ffmpeg cannot be located at all.
pub const FFMPEG_MISSING: &str =
    "ffmpeg was not found. Place ffmpeg and ffprobe next to the application executable, \
     or install them and make sure they are on your PATH.";

/// Failure markers are written next to the real previews but with a distinct
/// `.failed.webp` suffix so they can never be mistaken for a successful result.
fn failed_thumbnail_path(vault_path: &Path, media_id: &str) -> PathBuf {
    vault_path.join("previews").join(format!("{media_id}.failed.webp"))
}

fn failed_preview_path(vault_path: &Path, media_id: &str) -> PathBuf {
    vault_path.join("previews").join(format!("{media_id}_preview.failed.webp"))
}

/// Write the error placeholder to the given path.
fn write_error_placeholder(path: &Path) {
    if let Err(e) = fs::write(path, ERROR_THUMBNAIL) {
        log::error!("Failed to write error placeholder to {}: {e}", path.display());
    }
}

/// Resolve an executable name: look for it next to the running binary first,
/// fall back to the bare name (system PATH lookup).
fn resolve_exe(name: &str) -> std::ffi::OsString {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return candidate.into_os_string();
            }
            #[cfg(windows)]
            {
                let candidate = dir.join(format!("{name}.exe"));
                if candidate.exists() {
                    return candidate.into_os_string();
                }
            }
        }
    }
    OsStr::new(name).to_owned()
}

fn ffmpeg() -> Command {
    let cmd = Command::new(resolve_exe("ffmpeg"));
    #[cfg(windows)]
    {
        let mut cmd = cmd;
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        return cmd;
    }
    #[cfg(not(windows))]
    cmd
}

fn ffprobe() -> Command {
    let cmd = Command::new(resolve_exe("ffprobe"));
    #[cfg(windows)]
    {
        let mut cmd = cmd;
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        return cmd;
    }
    #[cfg(not(windows))]
    cmd
}

/// Probe whether ffmpeg can actually be spawned. Result is cached for the
/// lifetime of the process — an ffmpeg installed while the app is running is
/// picked up on the next start.
pub fn ffmpeg_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let ok = ffmpeg()
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            log::info!("ffmpeg found at {:?}", resolve_exe("ffmpeg"));
        } else {
            log::error!("{FFMPEG_MISSING}");
        }
        ok
    })
}

/// Keep only the last `n` lines of ffmpeg output — the interesting part of a
/// failure — so log lines stay readable.
fn tail_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.trim_end().lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join(" | ")
}

/// Check that a file exists and is non-empty.
fn is_non_empty(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false)
}

/// Ensure the previews directory exists and return its path.
fn previews_dir(vault_path: &Path) -> Result<PathBuf, String> {
    let dir = vault_path.join("previews");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create previews dir: {e}"))?;
    Ok(dir)
}

/// Run an ffmpeg command. If it fails with "Invalid color space", retry using
/// the libvpx-vp9 decoder which handles broken VP9 color metadata gracefully.
fn run_ffmpeg(args: &[&str]) -> Result<(), String> {
    let result = ffmpeg()
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

    if result.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&result.stderr);

    // If the error is about invalid color space, retry with libvpx-vp9 decoder
    if stderr.contains("Invalid color space") || stderr.contains("Invalid color range") {
        // Build new args: insert "-c:v libvpx-vp9" before the first "-i"
        let mut new_args: Vec<&str> = Vec::new();
        let mut inserted = false;
        for &arg in args {
            if arg == "-i" && !inserted {
                new_args.push("-c:v");
                new_args.push("libvpx-vp9");
                inserted = true;
            }
            new_args.push(arg);
        }

        let retry_result = ffmpeg()
            .args(&new_args)
            .output()
            .map_err(|e| format!("Failed to run ffmpeg (retry): {e}"))?;

        if retry_result.status.success() {
            return Ok(());
        }

        let retry_stderr = String::from_utf8_lossy(&retry_result.stderr);
        return Err(format!("ffmpeg failed (retry): {}", tail_lines(&retry_stderr, 4)));
    }

    Err(format!("ffmpeg failed: {}", tail_lines(&stderr, 4)))
}

/// Generate a static WebP thumbnail for any media type.
/// Saved to `previews/{media_id}.webp`.
///
/// If ffmpeg itself is unavailable this returns an error *without* leaving any
/// file behind, so the item is retried once ffmpeg is installed. Only a real
/// per-file ffmpeg failure writes the `.failed.webp` marker.
pub fn generate_thumbnail(vault_path: &Path, media_path: &Path, media_id: &str) -> Result<(), String> {
    let previews = previews_dir(vault_path)?;
    let output = previews.join(format!("{media_id}.webp"));

    if output.exists() || failed_thumbnail_path(vault_path, media_id).exists() {
        return Ok(());
    }

    if !ffmpeg_available() {
        return Err(FFMPEG_MISSING.to_string());
    }

    let input = media_path.to_str().ok_or("Invalid media path")?;
    let out = output.to_str().ok_or("Invalid output path")?;

    // Try with -ss 1 first (useful for videos)
    let first = run_ffmpeg(&[
        "-y", "-ss", "1", "-i", input, "-vframes", "1",
        "-vf", "scale=192:192:force_original_aspect_ratio=decrease",
        "-pix_fmt", "yuv420p", "-f", "webp", out,
    ]);

    if first.is_ok() && is_non_empty(&output) {
        return Ok(());
    }

    // Retry without -ss for images/gifs and for clips shorter than a second
    let second = run_ffmpeg(&[
        "-y", "-i", input, "-vframes", "1",
        "-vf", "scale=192:192:force_original_aspect_ratio=decrease",
        "-pix_fmt", "yuv420p", "-f", "webp", out,
    ]);

    if second.is_ok() && is_non_empty(&output) {
        return Ok(());
    }

    // Both attempts failed — record a marker so this file isn't retried forever
    let _ = fs::remove_file(&output);
    let reason = second.err().or(first.err()).unwrap_or_else(|| "produced an empty file".into());
    write_error_placeholder(&failed_thumbnail_path(vault_path, media_id));
    Err(format!("thumbnail generation failed: {reason}"))
}

/// Generate an animated preview for a video or GIF file (max 20 seconds).
/// For videos: MP4 preview with audio (if present). Saved as `{media_id}_preview.mp4`.
/// For GIFs: animated GIF preview. Saved as `{media_id}_preview.gif`.
pub fn generate_animated_preview(vault_path: &Path, media_path: &Path, media_id: &str) -> Result<(), String> {
    let previews = previews_dir(vault_path)?;

    let ext = media_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let is_gif = ext == "gif";
    let output = if is_gif {
        previews.join(format!("{media_id}_preview.gif"))
    } else {
        previews.join(format!("{media_id}_preview.mp4"))
    };

    if output.exists() || failed_preview_path(vault_path, media_id).exists() {
        return Ok(());
    }

    if !ffmpeg_available() {
        return Err(FFMPEG_MISSING.to_string());
    }

    let input = media_path.to_str().ok_or("Invalid media path")?;
    let duration = get_media_duration(input).unwrap_or(20.0);
    let preview_duration = format!("{}", duration.min(20.0));
    let out = output.to_str().ok_or("Invalid output path")?;

    let result = if is_gif {
        // GIF is palettized: generate a per-clip palette instead of letting
        // ffmpeg fall back to the fixed 8-bit rgb8 palette.
        run_ffmpeg(&[
            "-y", "-i", input,
            "-t", &preview_duration,
            "-filter_complex",
            "fps=24,scale=192:192:force_original_aspect_ratio=decrease:flags=lanczos,split[a][b];\
             [a]palettegen=stats_mode=diff[p];\
             [b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
            "-loop", "0",
            out,
        ])
    } else {
        run_ffmpeg(&[
            "-y", "-i", input,
            "-t", &preview_duration,
            "-vf", "scale=192:192:force_original_aspect_ratio=decrease:flags=lanczos,pad=ceil(iw/2)*2:ceil(ih/2)*2",
            "-pix_fmt", "yuv420p",
            "-c:v", "libx264",
            "-preset", "fast",
            "-crf", "30",
            "-c:a", "aac",
            "-b:a", "64k",
            "-ac", "1",
            "-movflags", "+faststart",
            out,
        ])
    };

    if result.is_ok() && is_non_empty(&output) {
        return Ok(());
    }

    // Clean up the empty/partial file and record a marker
    let _ = fs::remove_file(&output);
    let reason = result.err().unwrap_or_else(|| "produced an empty file".into());
    write_error_placeholder(&failed_preview_path(vault_path, media_id));
    Err(format!("animated preview generation failed: {reason}"))
}

/// Get the *real* thumbnail path for a media item. Returns None if generation
/// hasn't succeeded — callers use this to decide what still needs generating.
pub fn get_thumbnail_path(vault_path: &Path, media_id: &str) -> Option<PathBuf> {
    let path = vault_path.join("previews").join(format!("{media_id}.webp"));
    if path.exists() { Some(path) } else { None }
}

/// Get the *real* animated preview path (MP4 or GIF). Returns None if generation
/// hasn't succeeded.
pub fn get_preview_path(vault_path: &Path, media_id: &str) -> Option<PathBuf> {
    let previews = vault_path.join("previews");
    let mp4 = previews.join(format!("{media_id}_preview.mp4"));
    if mp4.exists() { return Some(mp4); }
    let gif = previews.join(format!("{media_id}_preview.gif"));
    if gif.exists() { return Some(gif); }
    None
}

/// Thumbnail to show in the UI: the real one, or the failure placeholder.
pub fn get_display_thumbnail_path(vault_path: &Path, media_id: &str) -> Option<PathBuf> {
    get_thumbnail_path(vault_path, media_id).or_else(|| {
        let failed = failed_thumbnail_path(vault_path, media_id);
        if failed.exists() { Some(failed) } else { None }
    })
}

/// Preview to show in the UI: the real one, or the failure placeholder.
pub fn get_display_preview_path(vault_path: &Path, media_id: &str) -> Option<PathBuf> {
    get_preview_path(vault_path, media_id).or_else(|| {
        let failed = failed_preview_path(vault_path, media_id);
        if failed.exists() { Some(failed) } else { None }
    })
}

/// Whether a previous run already tried and failed to build this thumbnail.
pub fn has_failed_thumbnail(vault_path: &Path, media_id: &str) -> bool {
    failed_thumbnail_path(vault_path, media_id).exists()
}

/// Whether a previous run already tried and failed to build this preview.
pub fn has_failed_preview(vault_path: &Path, media_id: &str) -> bool {
    failed_preview_path(vault_path, media_id).exists()
}

/// Remove all preview files for a media item, failure markers included.
pub fn remove_previews(vault_path: &Path, media_id: &str) {
    let previews = vault_path.join("previews");
    let _ = fs::remove_file(previews.join(format!("{media_id}.webp")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.gif")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.mp4")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.mp3")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.webp")));
    let _ = fs::remove_file(failed_thumbnail_path(vault_path, media_id));
    let _ = fs::remove_file(failed_preview_path(vault_path, media_id));
}

/// Older builds wrote the error placeholder straight to the real preview path,
/// which made a failed generation indistinguishable from a good one — the item
/// then counted as done forever. Delete those in-band placeholders so the normal
/// stale-media detection picks the items back up.
///
/// Returns the number of files removed.
pub fn repair_placeholder_previews(vault_path: &Path) -> u32 {
    let previews = vault_path.join("previews");
    let entries = match fs::read_dir(&previews) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut removed = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        // Failure markers are supposed to hold these bytes — leave them alone.
        if path.to_string_lossy().contains(".failed.") {
            continue;
        }
        let is_placeholder = entry
            .metadata()
            .map(|m| m.len() == ERROR_THUMBNAIL.len() as u64)
            .unwrap_or(false)
            && fs::read(&path).map(|d| d == ERROR_THUMBNAIL).unwrap_or(false);

        if is_placeholder && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }

    if removed > 0 {
        log::info!("Removed {removed} stale placeholder previews — they will be regenerated");
    }
    removed
}

fn get_media_duration(input: &str) -> Result<f64, String> {
    let output = ffprobe()
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            input,
        ])
        .output()
        .map_err(|e| format!("ffprobe failed: {e}"))?;

    let s = String::from_utf8_lossy(&output.stdout);
    s.trim().parse::<f64>().map_err(|e| format!("Failed to parse duration: {e}"))
}
