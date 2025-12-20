//! Agent event handler
//!
//! Processes events from agent connections and updates session state.

use crate::acp::{AgentEvent, PermissionOptionId, SessionUpdate, PermissionKind};
use crate::app::{App, InputMode};
use crate::session::{OutputType, PendingPermission, PendingQuestion, PermissionMode, SessionState};

/// Result of handling an agent event - may contain a command to send back
pub enum EventResult {
    None,
    AutoAcceptPermission { request_id: u64, option_id: PermissionOptionId },
}

/// Format agent capabilities into a human-readable string
fn format_agent_capabilities(caps: &serde_json::Value) -> String {
    let mut parts = vec![];

    // MCP capabilities
    if let Some(mcp) = caps.get("mcpCapabilities") {
        let mut mcp_features = vec![];
        if mcp.get("http").and_then(|v| v.as_bool()).unwrap_or(false) {
            mcp_features.push("HTTP");
        }
        if mcp.get("sse").and_then(|v| v.as_bool()).unwrap_or(false) {
            mcp_features.push("SSE");
        }
        if !mcp_features.is_empty() {
            parts.push(format!("MCP: {}", mcp_features.join(", ")));
        }
    }

    // Prompt capabilities
    if let Some(prompt) = caps.get("promptCapabilities") {
        let mut prompt_features = vec![];
        if prompt.get("embeddedContext").and_then(|v| v.as_bool()).unwrap_or(false) {
            prompt_features.push("embedded context");
        }
        if prompt.get("image").and_then(|v| v.as_bool()).unwrap_or(false) {
            prompt_features.push("images");
        }
        if !prompt_features.is_empty() {
            parts.push(format!("Supports: {}", prompt_features.join(", ")));
        }
    }

    // Session capabilities
    if let Some(session) = caps.get("sessionCapabilities") {
        let mut session_features = vec![];
        if session.get("resume").is_some() {
            session_features.push("resume");
        }
        if !session_features.is_empty() {
            parts.push(format!("Session: {}", session_features.join(", ")));
        }
    }

    if parts.is_empty() {
        "Agent capabilities: (none reported)".to_string()
    } else {
        format!("Agent capabilities: {}", parts.join(" | "))
    }
}

/// Handle an agent event and update session state
pub fn handle_agent_event(app: &mut App, session_id: &str, event: AgentEvent) -> EventResult {
    // Get these values before taking mutable borrow of sessions
    let is_insert_mode = app.input_mode == InputMode::Insert;
    let input_buffer = app.input_buffer.clone();
    let cursor_position = app.cursor_position;

    // Check if this session is the currently selected one
    let is_selected_session = app.sessions.selected_session()
        .map(|s| s.id == session_id)
        .unwrap_or(false);

    if let Some(session) = app.sessions.get_by_id_mut(session_id) {
        match event {
            AgentEvent::Initialized { agent_info, agent_capabilities } => {
                session.state = SessionState::Initializing;
                if let Some(info) = agent_info {
                    if let Some(name) = info.name {
                        session.add_output(
                            format!("Connected to {}", name),
                            OutputType::Text,
                        );
                    }
                }
                if let Some(caps) = agent_capabilities {
                    let formatted = format_agent_capabilities(&caps);
                    session.add_output(formatted, OutputType::Text);
                }
            }
            AgentEvent::SessionCreated { session_id: new_id, models } => {
                session.id = new_id;
                session.state = SessionState::Idle;
                if let Some(models_state) = models {
                    session.available_models = models_state.available_models;
                    session.current_model_id = Some(models_state.current_model_id);
                }
                session.add_output("Session ready. Press [i] to type.".to_string(), OutputType::Text);
            }
            AgentEvent::Update { update, .. } => {
                handle_session_update(session, update);
            }
            AgentEvent::PermissionRequest {
                request_id,
                tool_call_id,
                title,
                options,
                ..
            } => {
                // Check if we should auto-accept (AcceptAll mode)
                if session.permission_mode == PermissionMode::AcceptAll {
                    if let Some(option) = options.iter().find(|o| o.kind == PermissionKind::AllowOnce) {
                        session.state = SessionState::Prompting;
                        if session.scroll_offset == usize::MAX {
                            session.scroll_to_bottom();
                        }
                        return EventResult::AutoAcceptPermission {
                            request_id,
                            option_id: PermissionOptionId::from(option.option_id.clone()),
                        };
                    }
                }

                // Normal mode - show permission dialog
                session.state = SessionState::AwaitingPermission;
                session.pending_permission = Some(PendingPermission {
                    request_id,
                    tool_call_id,
                    title,
                    options,
                    selected: 0,
                });

                // Save input buffer if user was typing in this session
                if is_selected_session && is_insert_mode && !input_buffer.is_empty() {
                    session.save_input(input_buffer.clone(), cursor_position);
                }
            }
            AgentEvent::AskUserRequest {
                request_id,
                question,
                options,
                multi_select,
                ..
            } => {
                session.state = SessionState::AwaitingUserInput;
                session.pending_question = Some(PendingQuestion::new(
                    request_id,
                    question,
                    options,
                    multi_select,
                ));

                if is_selected_session && is_insert_mode && !input_buffer.is_empty() {
                    session.save_input(input_buffer.clone(), cursor_position);
                }
            }
            AgentEvent::PromptComplete { .. } => {
                session.state = SessionState::Idle;
                session.pending_permission = None;
                session.complete_active_tool();
                session.add_output(String::new(), OutputType::Text);
            }
            AgentEvent::FileWritten { diff, .. } => {
                session.add_tool_output(diff);
            }
            AgentEvent::Error { message } => {
                session.state = SessionState::Idle;
                session.add_output(format!("Error: {}", message), OutputType::Error);
            }
            AgentEvent::Disconnected => {
                session.state = SessionState::Idle;
                session.add_output("Disconnected".to_string(), OutputType::Text);
            }
        }
        // Auto-scroll to bottom only if already at bottom
        if session.scroll_offset == usize::MAX {
            session.scroll_to_bottom();
        }
    }
    EventResult::None
}

use crate::session::Session;

/// Handle a session update event
fn handle_session_update(session: &mut Session, update: SessionUpdate) {
    match update {
        SessionUpdate::AgentMessageChunk { content } => {
            if let crate::acp::protocol::UpdateContent::Text { text } = content {
                session.append_text(text);
            }
        }
        SessionUpdate::AgentThoughtChunk => {
            // Silently ignore
        }
        SessionUpdate::ToolCall { tool_call_id, title, raw_description, .. } => {
            let (name, description) = parse_tool_call_title(title, raw_description);
            
            // Only add spacing for new tool calls
            let is_new = !session.has_tool_call(&tool_call_id);
            if is_new {
                session.add_output(String::new(), OutputType::Text);
            }
            session.add_tool_call(tool_call_id, name, description);
        }
        SessionUpdate::ToolCallUpdate { tool_call_id, status } => {
            if status == "completed" {
                if session.active_tool_call_id.as_ref() == Some(&tool_call_id) {
                    session.complete_active_tool();
                }
            } else if status == "error" || status == "failed" {
                session.mark_tool_failed(&tool_call_id);
            } else if !status.trim().is_empty() && status != "in_progress" && status != "pending" {
                session.add_tool_output(status);
            }
        }
        SessionUpdate::Plan { entries } => {
            session.plan_entries = entries;
        }
        SessionUpdate::CurrentModeUpdate { current_mode_id } => {
            session.current_mode = Some(current_mode_id);
        }
        SessionUpdate::AvailableCommandsUpdate => {
            // Silently ignore
        }
        SessionUpdate::Other { raw_type } => {
            session.add_output(
                format!("[Unknown update: {}]", raw_type.as_deref().unwrap_or("?")),
                OutputType::Text,
            );
        }
    }
}

/// Parse a tool call title into name and description
fn parse_tool_call_title(title: Option<String>, raw_description: Option<String>) -> (String, Option<String>) {
    /// Strip all backticks from a string
    fn strip_backticks(s: &str) -> String {
        s.replace('`', "")
    }

    /// Clean up MCP tool names like "mcp__acp__Edit" -> "Edit"
    fn clean_tool_name(name: &str) -> &str {
        if let Some(pos) = name.rfind("__") {
            &name[pos + 2..]
        } else {
            name
        }
    }

    /// Check if a string is effectively "undefined" or empty
    fn is_undefined_or_empty(s: &str) -> bool {
        let trimmed = s.trim();
        trimmed.is_empty() || trimmed == "undefined" || trimmed == "null"
    }

    /// Clean description by removing "undefined" segments
    fn clean_description(desc: &str) -> Option<String> {
        let parts: Vec<&str> = desc.split(&[',', ':'][..])
            .map(|p| p.trim())
            .filter(|p| !is_undefined_or_empty(p) && !p.contains("undefined"))
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }

    /// Map common tool names to display names
    fn map_tool_name(name: &str) -> &str {
        match name {
            "Terminal" => "Bash",
            "Read File" => "Read",
            "Write File" => "Write",
            "Edit File" => "Edit",
            "grep" => "Grep",
            "glob" => "Glob",
            _ => name,
        }
    }

    let title_str = title
        .filter(|t| !is_undefined_or_empty(t))
        .unwrap_or_else(|| "Tool".to_string());

    let (name, description) = if let Some(paren_pos) = title_str.find('(') {
        let raw_name = strip_backticks(&title_str[..paren_pos]).trim().to_string();
        if is_undefined_or_empty(&raw_name) {
            ("Tool".to_string(), None)
        } else {
            let name = clean_tool_name(&raw_name);
            let desc = title_str[paren_pos + 1..].trim_end_matches(')').to_string();
            let desc = strip_backticks(&desc);
            let desc = clean_description(&desc);
            (map_tool_name(name).to_string(), desc)
        }
    } else if title_str.starts_with('`') && title_str.ends_with('`') {
        let cmd = strip_backticks(&title_str);
        ("Bash".to_string(), if is_undefined_or_empty(&cmd) { None } else { Some(cmd) })
    } else if let Some(backtick_pos) = title_str.find(" `") {
        let raw_name = &title_str[..backtick_pos];
        let name = clean_tool_name(raw_name);
        let desc = strip_backticks(&title_str[backtick_pos + 1..]);
        (map_tool_name(name).to_string(), if is_undefined_or_empty(&desc) { None } else { Some(desc) })
    } else {
        let clean_title = strip_backticks(&title_str);
        let cleaned_name = clean_tool_name(&clean_title);
        (map_tool_name(cleaned_name).to_string(), None)
    };

    // Filter raw_description
    let raw_description = raw_description.filter(|d| !is_undefined_or_empty(d));
    
    // For Read/Grep/Glob tools, prefer raw_description
    let description = match name.as_str() {
        "Read" | "Grep" | "Glob" => raw_description.or(description),
        _ => description.or(raw_description),
    };

    // Final safety filter
    let description = description.filter(|d| {
        !d.contains("undefined") && !d.trim().is_empty()
    });

    (name, description)
}
