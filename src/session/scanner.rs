//! Session scanner for finding resumable Claude sessions

use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use crate::app::ResumableSession;

/// JSONL entry structure for parsing session files
#[derive(Debug, Deserialize)]
struct SessionEntry {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    cwd: Option<String>,
    timestamp: Option<String>,
    slug: Option<String>,
    message: Option<MessageContent>,
    #[serde(rename = "type")]
    entry_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

/// Scan Claude's session storage for resumable sessions
pub async fn scan_resumable_sessions() -> Vec<ResumableSession> {
    let mut sessions = vec![];

    // Claude stores sessions in ~/.claude/projects/<project-path>/<session-id>.jsonl
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return sessions,
    };

    let projects_dir = home.join(".claude").join("projects");
    if !projects_dir.exists() {
        return sessions;
    }

    // Read all project directories
    let mut project_entries = match tokio::fs::read_dir(&projects_dir).await {
        Ok(entries) => entries,
        Err(_) => return sessions,
    };

    while let Ok(Some(project_entry)) = project_entries.next_entry().await {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        // Read session files in this project directory
        let mut session_files = match tokio::fs::read_dir(&project_path).await {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        while let Ok(Some(session_file)) = session_files.next_entry().await {
            let file_path = session_file.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            // Try to parse session info from the JSONL file
            if let Some(session) = parse_session_file(&file_path).await {
                sessions.push(session);
            }
        }
    }

    // Sort by timestamp, most recent first
    sessions.sort_by(|a, b| {
        match (&b.timestamp, &a.timestamp) {
            (Some(tb), Some(ta)) => tb.cmp(ta),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    // Return only the most recent sessions (limit to 20)
    sessions.truncate(20);
    sessions
}

/// Parse a session JSONL file to extract session info
async fn parse_session_file(path: &PathBuf) -> Option<ResumableSession> {
    let content = tokio::fs::read_to_string(path).await.ok()?;

    let mut session_id: Option<String> = None;
    let mut cwd: Option<PathBuf> = None;
    let mut first_prompt: Option<String> = None;
    let mut timestamp: Option<DateTime<Utc>> = None;
    let mut slug: Option<String> = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: SessionEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Extract session ID from first entry that has it
        if session_id.is_none() {
            if let Some(ref id) = entry.session_id {
                session_id = Some(id.clone());
            }
        }

        // Extract cwd from first entry that has it
        if cwd.is_none() {
            if let Some(ref c) = entry.cwd {
                cwd = Some(PathBuf::from(c));
            }
        }

        // Extract slug from first entry that has it
        if slug.is_none() {
            if let Some(ref s) = entry.slug {
                slug = Some(s.clone());
            }
        }

        // Extract first user prompt
        if first_prompt.is_none() {
            if entry.entry_type.as_deref() == Some("user") {
                if let Some(ref msg) = entry.message {
                    if msg.role.as_deref() == Some("user") {
                        first_prompt = extract_text_content(&msg.content);
                    }
                }
            }
        }

        // Extract timestamp
        if let Some(ref ts) = entry.timestamp {
            if let Ok(parsed) = DateTime::parse_from_rfc3339(ts) {
                let parsed_utc = parsed.with_timezone(&Utc);
                if timestamp.is_none() || timestamp.as_ref().is_some_and(|t| parsed_utc > *t) {
                    timestamp = Some(parsed_utc);
                }
            }
        }

        // Once we have all needed info, we can stop early
        if session_id.is_some() && cwd.is_some() && first_prompt.is_some() && timestamp.is_some() {
            break;
        }
    }

    // Only return if we have at least session_id and cwd
    let session_id = session_id?;
    let cwd = cwd?;

    // Skip empty session files
    if first_prompt.is_none() && timestamp.is_none() {
        return None;
    }

    // Skip warmup/cache sessions (not real conversations)
    if let Some(ref prompt) = first_prompt {
        let prompt_lower = prompt.to_lowercase();
        if prompt_lower == "warmup" || prompt_lower.starts_with("warmup") {
            return None;
        }
    }

    Some(ResumableSession {
        session_id,
        cwd,
        first_prompt,
        timestamp,
        slug,
    })
}

/// Extract text content from a message content value
fn extract_text_content(content: &Option<serde_json::Value>) -> Option<String> {
    match content {
        Some(serde_json::Value::String(s)) => {
            // Skip meta/command messages
            if s.starts_with("<command-") || s.starts_with("<local-command") || s.contains("Caveat:") {
                return None;
            }
            Some(truncate_text(s, 100))
        }
        Some(serde_json::Value::Array(arr)) => {
            // Look for text blocks in the array
            for item in arr {
                if let Some(obj) = item.as_object() {
                    if obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                            if !text.starts_with("<command-") && !text.starts_with("<local-command") && !text.contains("Caveat:") {
                                return Some(truncate_text(text, 100));
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Truncate text to a maximum length with ellipsis
fn truncate_text(text: &str, max_len: usize) -> String {
    // Get first line only
    let first_line = text.lines().next().unwrap_or(text);
    if first_line.len() <= max_len {
        first_line.to_string()
    } else {
        // Find a valid char boundary for truncation
        let mut end = max_len.saturating_sub(3);
        while end > 0 && !first_line.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &first_line[..end])
    }
}
