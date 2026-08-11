//! In-memory ring buffer of log records, so the app can show its own logs.
//!
//! Registered as an additional `flexi_logger` writer in [`crate::logging::init`],
//! it receives every record the file writer receives. The frontend polls
//! `get_logs_since` while the logs page is open; nothing is pushed, so the
//! buffer costs nothing when nobody is looking.

use flexi_logger::writers::LogWriter;
use flexi_logger::DeferredNow;
use log::Record;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Maximum number of records kept in memory. Older ones are dropped.
const CAPACITY: usize = 5000;

#[derive(Clone, Serialize)]
pub struct LogEntry {
    /// Monotonic, never reused — lets the frontend ask for "everything after N".
    pub seq: u64,
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

fn buffer() -> &'static Mutex<VecDeque<LogEntry>> {
    static BUFFER: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();
    BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(CAPACITY)))
}

static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);

/// Returns every entry recorded after `seq`, oldest first.
pub fn snapshot_since(seq: u64) -> Vec<LogEntry> {
    let buf = buffer().lock().unwrap();
    buf.iter().filter(|e| e.seq > seq).cloned().collect()
}

/// Empties the in-memory buffer. The log file on disk is untouched.
pub fn clear() {
    buffer().lock().unwrap().clear();
}

pub struct BufferWriter;

impl LogWriter for BufferWriter {
    fn write(&self, now: &mut DeferredNow, record: &Record) -> std::io::Result<()> {
        let entry = LogEntry {
            seq: NEXT_SEQ.fetch_add(1, Ordering::Relaxed),
            timestamp: now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level: record.level().to_string(),
            target: record.target().to_string(),
            message: record.args().to_string(),
        };

        // Keep this section free of anything that could log: `Mutex` is not
        // reentrant, so a nested `log::` call while holding the lock deadlocks.
        let mut buf = buffer().lock().unwrap();
        if buf.len() == CAPACITY {
            buf.pop_front();
        }
        buf.push_back(entry);

        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}
