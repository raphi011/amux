use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::panic;
use once_cell::sync::Lazy;
use chrono::Local;

static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(None));

/// Install a panic hook that logs to the log file
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Log to our log file
        let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = if let Some(loc) = panic_info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "unknown location".to_string()
        };

        log(&format!("[PANIC] {} at {}", msg, location));

        // Also call the default hook to print to stderr
        default_hook(panic_info);
    }));
}

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

    log("=== amux started ===");

    Ok(log_path)
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

/// Truncate a string at a char boundary (up to max_bytes)
fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the last valid char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Log incoming ACP message (truncated for readability)
pub fn log_incoming(line: &str) {
    let display = if line.len() > 500 {
        let truncated = truncate_at_char_boundary(line, 500);
        format!("{}... ({} bytes total)", truncated, line.len())
    } else {
        line.to_string()
    };
    log(&format!("<-- {}", display));
}

/// Log outgoing ACP message
pub fn log_outgoing(line: &str) {
    let display = if line.len() > 500 {
        let truncated = truncate_at_char_boundary(line, 500);
        format!("{}... ({} bytes total)", truncated, line.len())
    } else {
        line.to_string()
    };
    log(&format!("--> {}", display));
}

/// Log an event
pub fn log_event(event: &str) {
    log(&format!("[EVENT] {}", event));
}
