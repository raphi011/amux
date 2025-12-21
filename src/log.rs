use chrono::Local;
use once_cell::sync::Lazy;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(None));
static TOOL_LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(None));
static SESSION_ID: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Generate a short unique session ID (6 hex chars)
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Use nanoseconds for uniqueness, take last 24 bits (6 hex chars)
    let nanos = now.as_nanos() as u32;
    format!("{:06x}", nanos & 0xFFFFFF)
}

/// Get the current session ID
#[allow(dead_code)]
pub fn session_id() -> Option<String> {
    SESSION_ID.lock().ok().and_then(|guard| guard.clone())
}

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
/// Returns a tuple of (log_path, session_id)
pub fn init() -> std::io::Result<(PathBuf, String)> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let sid = generate_session_id();

    // Store the session ID globally
    if let Ok(mut guard) = SESSION_ID.lock() {
        *guard = Some(sid.clone());
    }

    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".amux")
        .join("logs");

    std::fs::create_dir_all(&log_dir)?;

    // Include session ID in log filename for easy matching
    let log_path = log_dir.join(format!("amux_{}_{}.log", timestamp, sid));
    let tool_log_path = log_dir.join(format!("amux_{}_{}_tools.log", timestamp, sid));

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;

    let tool_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tool_log_path)?;

    *LOG_FILE.lock().unwrap() = Some(file);
    *TOOL_LOG_FILE.lock().unwrap() = Some(tool_file);

    log(&format!("=== amux started (session: {}) ===", sid));
    log_tool(&format!("=== amux tool log started (session: {}) ===", sid));

    Ok((log_path, sid))
}

/// Log a message with timestamp
pub fn log(msg: &str) {
    let timestamp = Local::now().format("%H:%M:%S%.3f");
    let line = format!("[{}] {}\n", timestamp, msg);

    if let Ok(mut guard) = LOG_FILE.lock()
        && let Some(ref mut file) = *guard
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
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

/// Log a tool call JSON to the dedicated tools log file
pub fn log_tool(msg: &str) {
    let timestamp = Local::now().format("%H:%M:%S%.3f");
    let line = format!("[{}] {}\n", timestamp, msg);

    if let Ok(mut guard) = TOOL_LOG_FILE.lock()
        && let Some(ref mut file) = *guard
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

/// Log a tool call with pretty-printed JSON
pub fn log_tool_json(tool_name: &str, json: &serde_json::Value) {
    let pretty = serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string());
    log_tool(&format!("=== {} ===\n{}", tool_name, pretty));
}
