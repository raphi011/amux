use crate::acp::{AskUserOption, PermissionKind, PermissionOptionInfo, PlanEntry, PlanStatus};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum AgentType {
    ClaudeCode,
    GeminiCli,
}

impl AgentType {
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentType::ClaudeCode => "Claude",
            AgentType::GeminiCli => "Gemini",
        }
    }

    pub fn command(&self) -> &'static str {
        match self {
            AgentType::ClaudeCode => "claude-code-acp",
            AgentType::GeminiCli => "gemini",
        }
    }

    pub fn args(&self) -> &'static [&'static str] {
        match self {
            AgentType::ClaudeCode => &[],
            AgentType::GeminiCli => &["--experimental-acp"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionState {
    Spawning,
    Initializing,
    Idle,
    Prompting,
    AwaitingPermission,
    AwaitingUserInput,
}

impl SessionState {
    /// Check if a transition to the target state is valid
    #[allow(dead_code)]
    pub fn can_transition_to(&self, target: SessionState) -> bool {
        use SessionState::*;
        match (self, target) {
            // From Spawning
            (Spawning, Initializing) => true,
            (Spawning, Idle) => true, // Error/disconnect case

            // From Initializing
            (Initializing, Idle) => true,
            (Initializing, Prompting) => true, // Auto-prompt on init

            // From Idle
            (Idle, Prompting) => true,
            (Idle, Spawning) => false, // Can't go back to spawning

            // From Prompting
            (Prompting, Idle) => true, // Prompt complete or cancelled
            (Prompting, AwaitingPermission) => true,
            (Prompting, AwaitingUserInput) => true,

            // From AwaitingPermission
            (AwaitingPermission, Prompting) => true, // Permission granted
            (AwaitingPermission, Idle) => true,      // Permission denied

            // From AwaitingUserInput
            (AwaitingUserInput, Prompting) => true, // Answer provided
            (AwaitingUserInput, Idle) => true,      // Cancelled

            // Self-transitions are always valid (no-op)
            (a, b) if *a == b => true,

            _ => false,
        }
    }

    /// Returns true if the session is waiting for user interaction
    #[allow(dead_code)]
    pub fn awaiting_user(&self) -> bool {
        matches!(
            self,
            SessionState::AwaitingPermission | SessionState::AwaitingUserInput
        )
    }

    /// Returns true if the session can receive a new prompt
    #[allow(dead_code)]
    pub fn can_prompt(&self) -> bool {
        matches!(self, SessionState::Idle)
    }
}

/// Permission handling mode for a session
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PermissionMode {
    #[default]
    Normal, // Ask for each permission
    Plan,      // Plan mode - more cautious
    AcceptAll, // Auto-accept all permissions
}

impl PermissionMode {
    /// Cycle to the next mode
    pub fn next(self) -> Self {
        match self {
            PermissionMode::Normal => PermissionMode::Plan,
            PermissionMode::Plan => PermissionMode::AcceptAll,
            PermissionMode::AcceptAll => PermissionMode::Normal,
        }
    }

    /// Display name for the mode
    #[allow(dead_code)]
    pub fn display(&self) -> &'static str {
        match self {
            PermissionMode::Normal => "normal",
            PermissionMode::Plan => "plan",
            PermissionMode::AcceptAll => "accept all",
        }
    }
}

impl SessionState {
    #[allow(dead_code)]
    pub fn display(&self) -> &'static str {
        match self {
            SessionState::Spawning => "spawning...",
            SessionState::Initializing => "initializing...",
            SessionState::Idle => "idle",
            SessionState::Prompting => "working...",
            SessionState::AwaitingPermission => "⚠ permission required",
            SessionState::AwaitingUserInput => "? question",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            SessionState::Spawning | SessionState::Initializing | SessionState::Prompting
        )
    }
}

/// Pending permission request
#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub request_id: u64,
    #[allow(dead_code)]
    pub tool_call_id: String,
    pub title: Option<String>,
    pub options: Vec<PermissionOptionInfo>,
    pub selected: usize,
}

impl PendingPermission {
    pub fn select_next(&mut self) {
        if !self.options.is_empty() {
            self.selected = (self.selected + 1) % self.options.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.options.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.options.len() - 1);
        }
    }

    pub fn selected_option(&self) -> Option<&PermissionOptionInfo> {
        self.options.get(self.selected)
    }

    /// Find the first "allow once" option
    pub fn allow_once_option(&self) -> Option<&PermissionOptionInfo> {
        self.options
            .iter()
            .find(|o| o.kind == PermissionKind::AllowOnce)
    }
}

/// Pending clarifying question from agent
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub request_id: u64,
    pub question: String,
    pub options: Vec<AskUserOption>,
    #[allow(dead_code)]
    pub multi_select: bool,
    pub selected: usize,
    pub input: String,
    pub cursor_position: usize,
}

impl PendingQuestion {
    pub fn new(
        request_id: u64,
        question: String,
        options: Vec<AskUserOption>,
        multi_select: bool,
    ) -> Self {
        Self {
            request_id,
            question,
            options,
            multi_select,
            selected: 0,
            input: String::new(),
            cursor_position: 0,
        }
    }

    /// Check if this is a free-text question (no options)
    pub fn is_free_text(&self) -> bool {
        self.options.is_empty()
    }

    pub fn select_next(&mut self) {
        if !self.options.is_empty() {
            self.selected = (self.selected + 1) % self.options.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.options.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.options.len() - 1);
        }
    }

    pub fn selected_option(&self) -> Option<&AskUserOption> {
        self.options.get(self.selected)
    }

    /// Get the answer based on current state
    pub fn get_answer(&self) -> String {
        if self.is_free_text() {
            self.input.clone()
        } else if let Some(opt) = self.selected_option() {
            opt.value.clone().unwrap_or_else(|| opt.label.clone())
        } else {
            self.input.clone()
        }
    }

    // Input handling methods
    pub fn input_char(&mut self, c: char) {
        self.input.insert(self.cursor_position, c);
        self.cursor_position += 1;
    }

    pub fn input_backspace(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.input.remove(self.cursor_position);
        }
    }

    pub fn input_delete(&mut self) {
        if self.cursor_position < self.input.len() {
            self.input.remove(self.cursor_position);
        }
    }

    pub fn input_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn input_right(&mut self) {
        if self.cursor_position < self.input.len() {
            self.cursor_position += 1;
        }
    }

    pub fn input_home(&mut self) {
        self.cursor_position = 0;
    }

    pub fn input_end(&mut self) {
        self.cursor_position = self.input.len();
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    /// Local session ID (stable, used as HashMap key for agent_commands)
    pub id: String,
    /// ACP session ID from the agent (used in protocol messages)
    pub acp_session_id: Option<String>,
    pub name: String,
    pub agent_type: AgentType,
    pub state: SessionState,
    pub cwd: PathBuf,
    pub git_branch: String,
    pub git_origin: Option<String>,
    pub is_worktree: bool,
    #[allow(dead_code)] // TODO: Display token usage in UI
    pub tokens_input: u32,
    #[allow(dead_code)] // TODO: Display token usage in UI
    pub tokens_output: u32,
    pub output: Vec<OutputLine>,
    pub last_activity: Option<Instant>,
    /// When this session was created
    pub created_at: SystemTime,
    pub scroll_offset: usize,
    /// Total rendered lines after text wrapping (updated during render)
    pub total_rendered_lines: usize,
    pub pending_permission: Option<PendingPermission>,
    pub pending_question: Option<PendingQuestion>,
    pub plan_entries: Vec<PlanEntry>,
    pub current_mode: Option<String>,
    pub active_tool_call_id: Option<String>,
    pub permission_mode: PermissionMode,
    pub available_models: Vec<ModelInfo>,
    pub current_model_id: Option<String>,
    /// Saved input buffer when permission/question dialog interrupts typing
    pub saved_input: Option<(String, usize)>, // (buffer, cursor_position)
    /// Per-session prompt input buffer
    pub input_buffer: String,
    /// Per-session cursor position in input buffer
    pub input_cursor: usize,
}

/// Re-export ModelInfo for use in session
pub use crate::acp::ModelInfo;

#[derive(Debug, Clone)]
pub struct OutputLine {
    pub content: String,
    pub line_type: OutputType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputType {
    Text,      // Agent response text
    UserInput, // User's prompt
    ToolCall {
        tool_call_id: String,
        name: String,
        description: Option<String>,
        failed: bool,                  // Whether the tool call failed
        raw_json: Option<String>,      // Raw ACP JSON for debug rendering
    },
    ToolOutput,  // Output from a tool (shown with └ connector)
    DiffAdd,     // Added line in diff (green)
    DiffRemove,  // Removed line in diff (red)
    DiffContext, // Context line in diff (dim)
    DiffHeader,  // Diff header line (e.g. "Added 18 lines, removed 11")
    Error,
}

impl Session {
    pub fn new(
        id: String,
        name: String,
        agent_type: AgentType,
        cwd: PathBuf,
        is_worktree: bool,
    ) -> Self {
        Self {
            id,
            acp_session_id: None,
            name,
            agent_type,
            state: SessionState::Spawning,
            cwd,
            git_branch: String::new(),
            git_origin: None,
            is_worktree,
            tokens_input: 0,
            tokens_output: 0,
            output: vec![],
            last_activity: Some(Instant::now()),
            created_at: SystemTime::now(),
            scroll_offset: usize::MAX,
            total_rendered_lines: 0,
            pending_permission: None,
            pending_question: None,
            plan_entries: vec![],
            current_mode: None,
            active_tool_call_id: None,
            permission_mode: PermissionMode::default(),
            available_models: vec![],
            current_model_id: None,
            saved_input: None,
            input_buffer: String::new(),
            input_cursor: 0,
        }
    }

    /// Transition to a new state, logging invalid transitions
    ///
    /// This method validates the transition and logs a warning if the
    /// transition is invalid (but still allows it for backward compatibility).
    #[allow(dead_code)]
    pub fn transition_to(&mut self, new_state: SessionState) {
        if !self.state.can_transition_to(new_state) {
            crate::log::log(&format!(
                "Warning: Invalid state transition {:?} -> {:?} for session {}",
                self.state, new_state, self.id
            ));
        }
        self.state = new_state;
    }

    /// Check if the session has a pending permission request
    #[allow(dead_code)]
    pub fn has_pending_permission(&self) -> bool {
        self.pending_permission.is_some()
    }

    /// Check if the session has a pending question
    #[allow(dead_code)]
    pub fn has_pending_question(&self) -> bool {
        self.pending_question.is_some()
    }

    /// Check if the session is awaiting any user input
    #[allow(dead_code)]
    pub fn is_awaiting_user(&self) -> bool {
        self.state.awaiting_user()
    }

    /// Save the current input buffer (called when permission/question interrupts)
    pub fn save_input(&mut self, buffer: String, cursor: usize) {
        if !buffer.is_empty() {
            self.saved_input = Some((buffer, cursor));
        }
    }

    /// Take the saved input buffer (returns and clears it)
    pub fn take_saved_input(&mut self) -> Option<(String, usize)> {
        self.saved_input.take()
    }

    /// Cycle to the next permission mode
    pub fn cycle_permission_mode(&mut self) {
        self.permission_mode = self.permission_mode.next();
    }

    /// Cycle to the next available model, returns the new model_id if changed
    pub fn cycle_model(&mut self) -> Option<String> {
        if self.available_models.is_empty() {
            return None;
        }

        let current_idx = self
            .current_model_id
            .as_ref()
            .and_then(|id| self.available_models.iter().position(|m| &m.model_id == id))
            .unwrap_or(0);

        let next_idx = (current_idx + 1) % self.available_models.len();
        let next_model_id = self.available_models[next_idx].model_id.clone();
        self.current_model_id = Some(next_model_id.clone());
        Some(next_model_id)
    }

    /// Get display name for current model
    pub fn current_model_name(&self) -> Option<&str> {
        self.current_model_id.as_ref().and_then(|id| {
            self.available_models
                .iter()
                .find(|m| &m.model_id == id)
                .map(|m| m.name.as_str())
        })
    }

    /// Get a description of what the agent is currently working on.
    /// Returns the in-progress plan entry if available, otherwise falls back
    /// to the last user prompt.
    pub fn current_activity(&self) -> Option<String> {
        // First priority: in-progress plan entry
        if let Some(entry) = self
            .plan_entries
            .iter()
            .find(|e| e.status == PlanStatus::InProgress)
        {
            return Some(entry.content.clone());
        }

        // Second priority: last user prompt
        for line in self.output.iter().rev() {
            if matches!(line.line_type, OutputType::UserInput) {
                return Some(line.content.clone());
            }
        }

        None
    }

    /// Scroll up by n lines. If at bottom (usize::MAX), first normalize to actual position.
    pub fn scroll_up(&mut self, n: usize, total_lines: usize, viewport_height: usize) {
        // Normalize usize::MAX to actual bottom position
        if self.scroll_offset == usize::MAX {
            self.scroll_offset = total_lines.saturating_sub(viewport_height);
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize, total_lines: usize, viewport_height: usize) {
        // Normalize usize::MAX to actual bottom position
        if self.scroll_offset == usize::MAX {
            self.scroll_offset = total_lines.saturating_sub(viewport_height);
        }
        // Cap at the maximum scrollable position
        let max_scroll = total_lines.saturating_sub(viewport_height);
        self.scroll_offset = self.scroll_offset.saturating_add(n).min(max_scroll);
    }

    /// Scroll to bottom of output (uses sentinel value, renderer handles actual positioning)
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = usize::MAX;
    }

    #[allow(dead_code)] // TODO: Display token usage in UI
    pub fn total_tokens(&self) -> u32 {
        self.tokens_input + self.tokens_output
    }

    pub fn add_output(&mut self, content: String, line_type: OutputType) {
        self.output.push(OutputLine { content, line_type });
        self.last_activity = Some(Instant::now());
    }

    /// Append text to the last output line (for streaming), or create new line
    pub fn append_text(&mut self, text: String) {
        if let Some(last) = self.output.last_mut() {
            // Only append to non-empty text lines (empty lines are for spacing)
            if matches!(last.line_type, OutputType::Text) && !last.content.is_empty() {
                last.content.push_str(&text);
                self.last_activity = Some(Instant::now());
                return;
            }
        }
        // No text line to append to, create new one
        self.add_output(text, OutputType::Text);
    }

    /// Check if a tool call with this ID already exists
    pub fn has_tool_call(&self, tool_call_id: &str) -> bool {
        self.output.iter().rev().any(|line| {
            matches!(&line.line_type, OutputType::ToolCall { tool_call_id: id, .. } if id == tool_call_id)
        })
    }

    /// Add or update a tool call in output
    /// If a tool call with the same ID already exists, update it (for progressive disclosure)
    pub fn add_tool_call(
        &mut self,
        tool_call_id: String,
        name: String,
        description: Option<String>,
        raw_json: Option<String>,
    ) {
        // Check if we already have this tool call - if so, update it
        for line in self.output.iter_mut().rev() {
            if let OutputType::ToolCall {
                tool_call_id: existing_id,
                name: existing_name,
                description: existing_desc,
                raw_json: existing_raw_json,
                ..
            } = &mut line.line_type
                && existing_id == &tool_call_id
            {
                // Update with better info if available
                if description.is_some() && existing_desc.is_none() {
                    *existing_desc = description;
                }
                // Update name if we got a more specific one
                if name != "Tool"
                    && (existing_name == "Tool"
                        || existing_name == "Read File"
                        || existing_name == "Edit"
                        || existing_name == "Terminal")
                {
                    *existing_name = name;
                }
                // Store raw JSON if we got it and don't have one yet
                if raw_json.is_some() && existing_raw_json.is_none() {
                    *existing_raw_json = raw_json;
                }
                self.last_activity = Some(Instant::now());
                return;
            }
        }

        // New tool call - add it
        self.active_tool_call_id = Some(tool_call_id.clone());
        self.output.push(OutputLine {
            content: String::new(),
            line_type: OutputType::ToolCall {
                tool_call_id,
                name,
                description,
                failed: false,
                raw_json,
            },
        });
        self.last_activity = Some(Instant::now());
    }

    /// Mark the current tool as complete
    pub fn complete_active_tool(&mut self) {
        self.active_tool_call_id = None;
    }

    /// Mark a tool call as failed
    pub fn mark_tool_failed(&mut self, tool_call_id: &str) {
        for line in self.output.iter_mut().rev() {
            if let OutputType::ToolCall {
                tool_call_id: existing_id,
                failed,
                ..
            } = &mut line.line_type
                && existing_id == tool_call_id
            {
                *failed = true;
                break;
            }
        }
        // Also complete the tool so it stops spinning
        if self.active_tool_call_id.as_ref() == Some(&tool_call_id.to_string()) {
            self.active_tool_call_id = None;
        }
    }

    /// Add tool output, parsing for diff content
    pub fn add_tool_output(&mut self, content: String) {
        // Skip status-only lines like "completed", "running", etc.
        let dominated = content.trim().to_lowercase();
        if dominated == "completed" || dominated == "running" || dominated == "pending" {
            return;
        }

        // Check if this looks like diff content
        // Diff lines from generate_diff have format: "<sign><line_info> <content>"
        // where sign is '+', '-', or ' ' and line_info is like "  42   43"
        for line in content.lines() {
            let (line_type, stored_content) = if line.starts_with('+') && !line.starts_with("+++") {
                // Added line - strip the '+' prefix since we use color coding
                (OutputType::DiffAdd, line[1..].to_string())
            } else if line.starts_with('-') && !line.starts_with("---") {
                // Removed line - strip the '-' prefix since we use color coding
                (OutputType::DiffRemove, line[1..].to_string())
            } else if line.starts_with(' ')
                && line.len() > 10
                && line
                    .chars()
                    .skip(1)
                    .take(9)
                    .all(|c| c.is_ascii_digit() || c == ' ')
            {
                // Context line - starts with space followed by line numbers (e.g., " 123  456 ")
                // Strip the leading space to align with add/remove lines
                (OutputType::DiffContext, line[1..].to_string())
            } else if line.starts_with("@@") {
                // Skip @@ hunk headers entirely
                continue;
            } else if line.starts_with("diff ")
                || line.starts_with("index ")
                || line.starts_with("---")
                || line.starts_with("+++")
                || line.starts_with("Added ")
                || line.starts_with("Removed ")
                || line.contains(" lines,")
            {
                (OutputType::DiffHeader, line.to_string())
            } else {
                (OutputType::ToolOutput, line.to_string())
            };
            self.output.push(OutputLine {
                content: stored_content,
                line_type,
            });
        }
        self.last_activity = Some(Instant::now());
    }

    /// Create a mock session for UI development
    pub fn mock(id: &str, name: &str, agent_type: AgentType, branch: &str) -> Self {
        Self {
            id: id.to_string(),
            acp_session_id: None,
            name: name.to_string(),
            agent_type,
            state: SessionState::Idle,
            cwd: PathBuf::from(format!("~/Code/{}", name)),
            git_branch: branch.to_string(),
            git_origin: None,
            is_worktree: false,
            tokens_input: 0,
            tokens_output: 0,
            output: vec![],
            last_activity: None,
            created_at: SystemTime::now(),
            scroll_offset: usize::MAX,
            total_rendered_lines: 0,
            pending_permission: None,
            pending_question: None,
            plan_entries: vec![],
            current_mode: None,
            active_tool_call_id: None,
            permission_mode: PermissionMode::default(),
            available_models: vec![],
            current_model_id: None,
            saved_input: None,
            input_buffer: String::new(),
            input_cursor: 0,
        }
    }
}
