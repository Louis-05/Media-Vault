use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

pub struct ImportedMedia {
    pub id: String,
    pub extension: String,
    pub media_type: String,
    pub codec: Option<String>,
    pub file_size: Option<u64>,
    pub checksum: String,
}

pub fn compute_checksum(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| format!("Failed to read file: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Detect the video/image/audio codec using ffprobe.
pub fn detect_codec(file_path: &Path) -> Option<String> {
    let input = file_path.to_str()?;

    // Try video stream first
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=codec_name",
            "-of", "default=noprint_wrappers=1:nokey=1",
            input,
        ])
        .output()
        .ok()?;

    let codec = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !codec.is_empty() {
        return Some(codec);
    }

    // Fall back to audio stream
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "a:0",
            "-show_entries", "stream=codec_name",
            "-of", "default=noprint_wrappers=1:nokey=1",
            input,
        ])
        .output()
        .ok()?;

    let codec = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if codec.is_empty() { None } else { Some(codec) }
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "webp", "tiff", "tif", "avif"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv"];
const GIF_EXTENSIONS: &[&str] = &["gif"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "aac", "m4a", "wma", "opus"];

pub fn detect_media_type(extension: &str) -> Option<&'static str> {
    let ext = extension.to_lowercase();
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some("image")
    } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some("video")
    } else if GIF_EXTENSIONS.contains(&ext.as_str()) {
        Some("gif")
    } else if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some("audio")
    } else {
        None
    }
}

pub fn import_file(vault_path: &Path, source_path: &str) -> Result<ImportedMedia, String> {
    let source = Path::new(source_path);

    if !source.exists() {
        return Err(format!("File not found: {source_path}"));
    }

    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let media_type =
        detect_media_type(&extension).ok_or(format!("Unsupported file type: {extension}"))?;

    let checksum = compute_checksum(source)?;

    let id = Uuid::new_v4().to_string();
    let dest = vault_path
        .join("media")
        .join(format!("{id}.{extension}"));

    let metadata = fs::metadata(source).map_err(|e| format!("Failed to read metadata: {e}"))?;
    let file_size = Some(metadata.len());

    fs::copy(source, &dest).map_err(|e| format!("Failed to copy file: {e}"))?;

    let codec = detect_codec(&dest);

    Ok(ImportedMedia {
        id,
        extension,
        media_type: media_type.to_string(),
        codec,
        file_size,
        checksum,
    })
}

/// Rename a file in the media folder that doesn't have a UUID name yet.
/// Returns the new UUID and extension if renamed, or None if already a UUID.
pub fn rename_to_uuid(vault_path: &Path, filename: &str) -> Result<Option<ImportedMedia>, String> {
    let path = vault_path.join("media").join(filename);
    if !path.exists() {
        return Err(format!("File not found: {filename}"));
    }

    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // Check if already a UUID
    if Uuid::parse_str(stem).is_ok() {
        return Ok(None);
    }

    let extension = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let media_type =
        detect_media_type(&extension).ok_or(format!("Unsupported file type: {extension}"))?;

    let checksum = compute_checksum(&path)?;

    let id = Uuid::new_v4().to_string();
    let new_path = vault_path
        .join("media")
        .join(format!("{id}.{extension}"));

    let metadata = fs::metadata(&path).map_err(|e| format!("Failed to read metadata: {e}"))?;

    fs::rename(&path, &new_path).map_err(|e| format!("Failed to rename file: {e}"))?;

    let codec = detect_codec(&new_path);

    Ok(Some(ImportedMedia {
        id,
        extension,
        media_type: media_type.to_string(),
        codec,
        file_size: Some(metadata.len()),
        checksum,
    }))
}
