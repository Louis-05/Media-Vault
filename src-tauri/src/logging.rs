use crate::log_buffer::BufferWriter;
use flexi_logger::{Duplicate, FileSpec, Logger};
use std::path::PathBuf;

/// Directory holding the log files: `logs/` next to the executable.
pub fn logs_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("logs")))
        .unwrap_or_else(|| PathBuf::from("logs"))
}

/// Initialize logging: write to a timestamped file in `logs/` next to the executable,
/// duplicate all messages to stderr, and keep them in an in-memory buffer that the
/// in-app log viewer reads.
pub fn init() {
    let logs_dir = logs_dir();

    let _ = std::fs::create_dir_all(&logs_dir);

    Logger::try_with_str("info")
        .unwrap()
        .log_to_file_and_writer(
            FileSpec::default()
                .directory(logs_dir)
                .basename("media-vault"),
            Box::new(BufferWriter),
        )
        .duplicate_to_stderr(Duplicate::All)
        .start()
        .expect("Failed to initialize logger");

    install_panic_hook();
}

/// Route panics through `log::error!` so they land in the log file and the in-app
/// viewer. Release builds use `panic = "abort"` and have no console, so without
/// this a panic leaves no trace at all.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        log::error!("Panic at {location}: {}", info.payload_as_str().unwrap_or("<non-string payload>"));
        previous(info);
    }));
}
