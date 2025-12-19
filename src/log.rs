use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use chrono::Local;

static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(None));
static LOG_PATH: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

/// Initialize logging to a file
pub fn init() -> std::io::Result<PathBuf> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".amux")
        .join("logs");

    std::fs::create_dir_all(&log_dir)?;

    let log_path = log_dir.join(format!("amux_{}.log", timestamp));

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;

    *LOG_FILE.lock().unwrap() = Some(file);
    *LOG_PATH.lock().unwrap() = Some(log_path.clone());

    log("=== amux started ===");

    Ok(log_path)
}

/// Get the current log file path
pub fn path() -> Option<PathBuf> {
    LOG_PATH.lock().unwrap().clone()
}

/// Log a message with timestamp
pub fn log(msg: &str) {
    let timestamp = Local::now().format("%H:%M:%S%.3f");
    let line = format!("[{}] {}\n", timestamp, msg);

    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut file) = *guard {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

/// Log incoming ACP message (truncated for readability)
pub fn log_incoming(line: &str) {
    let display = if line.len() > 500 {
        format!("{}... ({} bytes total)", &line[..500], line.len())
    } else {
        line.to_string()
    };
    log(&format!("<-- {}", display));
}

/// Log outgoing ACP message
pub fn log_outgoing(line: &str) {
    let display = if line.len() > 500 {
        format!("{}... ({} bytes total)", &line[..500], line.len())
    } else {
        line.to_string()
    };
    log(&format!("--> {}", display));
}

/// Log an event
pub fn log_event(event: &str) {
    log(&format!("[EVENT] {}", event));
}

/// Log an error
pub fn log_error(error: &str) {
    log(&format!("[ERROR] {}", error));
}
