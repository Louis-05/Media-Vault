use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pre-generated 192x192 WebP placeholder with red "Failed to generate preview" text.
const ERROR_THUMBNAIL: &[u8] = include_bytes!("../../assets/error_thumbnail.webp");

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
        return Err(format!("ffmpeg failed (retry): {retry_stderr}"));
    }

    Err(format!("ffmpeg failed: {stderr}"))
}

/// Generate a static WebP thumbnail for any media type.
/// Saved to `previews/{media_id}.webp`.
pub fn generate_thumbnail(vault_path: &Path, media_path: &Path, media_id: &str) -> Result<(), String> {
    let previews = previews_dir(vault_path)?;
    let output = previews.join(format!("{media_id}.webp"));

    if output.exists() {
        return Ok(());
    }

    let input = media_path.to_str().ok_or("Invalid media path")?;
    let out = output.to_str().ok_or("Invalid output path")?;

    // Try with -ss 1 first (useful for videos)
    let result = run_ffmpeg(&[
        "-y", "-ss", "1", "-i", input, "-vframes", "1",
        "-vf", "scale=192:192:force_original_aspect_ratio=decrease",
        "-pix_fmt", "yuv420p", "-f", "webp", out,
    ]);

    if result.is_ok() && is_non_empty(&output) {
        return Ok(());
    }

    // Retry without -ss for images/gifs
    let result = run_ffmpeg(&[
        "-y", "-i", input, "-vframes", "1",
        "-vf", "scale=192:192:force_original_aspect_ratio=decrease",
        "-pix_fmt", "yuv420p", "-f", "webp", out,
    ]);

    if result.is_ok() && is_non_empty(&output) {
        return Ok(());
    }

    // Both attempts failed — write error placeholder
    log::warn!("Thumbnail generation failed for {media_id}, using error placeholder");
    write_error_placeholder(&output);
    Ok(())
}

/// Generate an animated preview for a video or GIF file (max 20 seconds).
/// For videos: MP4 preview with audio (if present). Saved as `{media_id}_preview.mp4`.
/// For GIFs: animated GIF preview. Saved as `{media_id}_preview.gif`.
pub fn generate_animated_preview(vault_path: &Path, media_path: &Path, media_id: &str) -> Result<(), String> {
    let previews = previews_dir(vault_path)?;
    let input = media_path.to_str().ok_or("Invalid media path")?;
    let duration = get_media_duration(input).unwrap_or(20.0);
    let preview_duration = format!("{}", duration.min(20.0));

    let ext = media_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let is_gif = ext == "gif";

    if is_gif {
        let output = previews.join(format!("{media_id}_preview.gif"));
        if output.exists() {
            return Ok(());
        }
        let out = output.to_str().ok_or("Invalid output path")?;

        let result = run_ffmpeg(&[
            "-y", "-i", input,
            "-t", &preview_duration,
            "-vf", "fps=24,scale=192:192:force_original_aspect_ratio=decrease:flags=lanczos",
            "-pix_fmt", "yuv420p",
            "-loop", "0",
            out,
        ]);

        if result.is_ok() && is_non_empty(&output) {
            return Ok(());
        }

        // Clean up empty/failed file and write placeholder as static webp
        let _ = fs::remove_file(&output);
        let fallback = previews.join(format!("{media_id}_preview.webp"));
        log::warn!("Animated preview generation failed for {media_id}, using error placeholder");
        write_error_placeholder(&fallback);
        Ok(())
    } else {
        let output = previews.join(format!("{media_id}_preview.mp4"));
        if output.exists() {
            return Ok(());
        }
        let out = output.to_str().ok_or("Invalid output path")?;

        let result = run_ffmpeg(&[
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
        ]);

        if result.is_ok() && is_non_empty(&output) {
            return Ok(());
        }

        // Clean up empty/failed file and write placeholder as static webp
        let _ = fs::remove_file(&output);
        let fallback = previews.join(format!("{media_id}_preview.webp"));
        log::warn!("Video preview generation failed for {media_id}, using error placeholder");
        write_error_placeholder(&fallback);
        Ok(())
    }
}

/// Get the thumbnail path for a media item. Returns None if it doesn't exist.
pub fn get_thumbnail_path(vault_path: &Path, media_id: &str) -> Option<PathBuf> {
    let path = vault_path.join("previews").join(format!("{media_id}.webp"));
    if path.exists() { Some(path) } else { None }
}

/// Get the animated preview path (MP4, GIF, or WebP fallback). Returns None if none exists.
pub fn get_preview_path(vault_path: &Path, media_id: &str) -> Option<PathBuf> {
    let previews = vault_path.join("previews");
    let mp4 = previews.join(format!("{media_id}_preview.mp4"));
    if mp4.exists() { return Some(mp4); }
    let gif = previews.join(format!("{media_id}_preview.gif"));
    if gif.exists() { return Some(gif); }
    let webp = previews.join(format!("{media_id}_preview.webp"));
    if webp.exists() { return Some(webp); }
    None
}

/// Remove all preview files for a media item.
pub fn remove_previews(vault_path: &Path, media_id: &str) {
    let previews = vault_path.join("previews");
    let _ = fs::remove_file(previews.join(format!("{media_id}.webp")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.gif")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.mp4")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.mp3")));
    let _ = fs::remove_file(previews.join(format!("{media_id}_preview.webp")));
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
