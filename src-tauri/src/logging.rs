use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub source: String,
    pub level: String, // "info", "warn", "error"
    pub message: String,
}

#[derive(Clone)]
pub struct LogBuffer {
    entries: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES))),
        }
    }

    /// Clone the inner Arc so pipe threads can push entries
    pub fn clone_inner(&self) -> LogBuffer {
        LogBuffer {
            entries: self.entries.clone(),
        }
    }

    pub fn push(&self, entry: LogEntry) {
        if let Ok(mut buf) = self.entries.lock() {
            if buf.len() >= MAX_LOG_ENTRIES {
                buf.pop_front();
            }
            buf.push_back(entry);
        }
    }

    pub fn get_all(&self) -> Vec<LogEntry> {
        self.entries
            .lock()
            .map(|buf| buf.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// Emit a log entry to both the buffer and the frontend
pub fn emit_log(app: &AppHandle, buffer: &LogBuffer, source: &str, level: &str, message: &str) {
    let entry = LogEntry {
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        source: source.to_string(),
        level: level.to_string(),
        message: message.to_string(),
    };
    eprintln!("[{}] [{}] {}: {}", entry.timestamp, level, source, message);
    buffer.push(entry.clone());
    let _ = app.emit("log-entry", entry);
}
