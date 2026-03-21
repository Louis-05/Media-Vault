use flexi_logger::{Duplicate, FileSpec, Logger};

/// Initialize logging: write to a timestamped file in `logs/` next to the executable,
/// and duplicate all messages to stderr.
pub fn init() {
    let logs_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("logs")))
        .unwrap_or_else(|| std::path::PathBuf::from("logs"));

    let _ = std::fs::create_dir_all(&logs_dir);

    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default()
                .directory(logs_dir)
                .basename("media-vault"),
        )
        .duplicate_to_stderr(Duplicate::All)
        .start()
        .expect("Failed to initialize logger");
}
