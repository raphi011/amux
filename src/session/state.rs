#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Instant;
use crate::acp::{PermissionOptionInfo, PermissionKind, PlanEntry, AskUserOption};

#[derive(Debug, Clone, Copy, PartialEq)]
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

/// Permission handling mode for a session
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PermissionMode {
    #[default]
    Normal,    // Ask for each permission
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
    pub fn display(&self) -> &'static str {
        match self {
            PermissionMode::Normal => "normal",
            PermissionMode::Plan => "plan",
            PermissionMode::AcceptAll => "accept all",
        }
    }
}

impl SessionState {
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
        matches!(self, SessionState::Spawning | SessionState::Initializing | SessionState::Prompting)
    }
}

/// Pending permission request
#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub request_id: u64,
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
            self.selected = self.selected.checked_sub(1).unwrap_or(self.options.len() - 1);
        }
    }

    pub fn selected_option(&self) -> Option<&PermissionOptionInfo> {
        self.options.get(self.selected)
    }

    /// Find the first "allow once" option
    pub fn allow_once_option(&self) -> Option<&PermissionOptionInfo> {
        self.options.iter().find(|o| o.kind == PermissionKind::AllowOnce)
    }
}

/// Pending clarifying question from agent
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub request_id: u64,
    pub question: String,
    pub options: Vec<AskUserOption>,
    pub multi_select: bool,
    pub selected: usize,
    pub input: String,
    pub cursor_position: usize,
}

impl PendingQuestion {
    pub fn new(request_id: u64, question: String, options: Vec<AskUserOption>, multi_select: bool) -> Self {
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
            self.selected = self.selected.checked_sub(1).unwrap_or(self.options.len() - 1);
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
    pub id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub state: SessionState,
    pub cwd: PathBuf,
    pub git_branch: String,
    pub is_worktree: bool,
    pub tokens_input: u32,
    pub tokens_output: u32,
    pub output: Vec<OutputLine>,
    pub last_activity: Option<Instant>,
    pub scroll_offset: usize,
    pub pending_permission: Option<PendingPermission>,
    pub pending_question: Option<PendingQuestion>,
    pub plan_entries: Vec<PlanEntry>,
    pub current_mode: Option<String>,
    pub active_tool_call_id: Option<String>,
    pub permission_mode: PermissionMode,
    pub available_models: Vec<ModelInfo>,
    pub current_model_id: Option<String>,
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
    Text,       // Agent response text
    UserInput,  // User's prompt
    ToolCall {
        tool_call_id: String,
        name: String,
        description: Option<String>,
    },
    ToolOutput,  // Output from a tool (shown with └ connector)
    DiffAdd,     // Added line in diff (green)
    DiffRemove,  // Removed line in diff (red)
    DiffContext, // Context line in diff (dim)
    DiffHeader,  // Diff header line (e.g. "Added 18 lines, removed 11")
    Error,
}

impl Session {
    pub fn new(id: String, name: String, agent_type: AgentType, cwd: PathBuf, is_worktree: bool) -> Self {
        Self {
            id,
            name,
            agent_type,
            state: SessionState::Spawning,
            cwd,
            git_branch: String::new(),
            is_worktree,
            tokens_input: 0,
            tokens_output: 0,
            output: vec![],
            last_activity: Some(Instant::now()),
            scroll_offset: 0,
            pending_permission: None,
            pending_question: None,
            plan_entries: vec![],
            current_mode: None,
            active_tool_call_id: None,
            permission_mode: PermissionMode::default(),
            available_models: vec![],
            current_model_id: None,
        }
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

        let current_idx = self.current_model_id.as_ref()
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
            self.available_models.iter()
                .find(|m| &m.model_id == id)
                .map(|m| m.name.as_str())
        })
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
    pub fn add_tool_call(&mut self, tool_call_id: String, name: String, description: Option<String>) {
        // Check if we already have this tool call - if so, update it
        for line in self.output.iter_mut().rev() {
            if let OutputType::ToolCall { tool_call_id: existing_id, name: existing_name, description: existing_desc } = &mut line.line_type {
                if existing_id == &tool_call_id {
                    // Update with better info if available
                    if description.is_some() && existing_desc.is_none() {
                        *existing_desc = description;
                    }
                    // Update name if we got a more specific one
                    if name != "Tool" && (existing_name == "Tool" || existing_name == "Read File" || existing_name == "Edit" || existing_name == "Terminal") {
                        *existing_name = name;
                    }
                    self.last_activity = Some(Instant::now());
                    return;
                }
            }
        }

        // New tool call - add it
        self.active_tool_call_id = Some(tool_call_id.clone());
        self.output.push(OutputLine {
            content: String::new(),
            line_type: OutputType::ToolCall { tool_call_id, name, description },
        });
        self.last_activity = Some(Instant::now());
    }

    /// Mark the current tool as complete
    pub fn complete_active_tool(&mut self) {
        self.active_tool_call_id = None;
    }

    /// Add tool output, parsing for diff content
    pub fn add_tool_output(&mut self, content: String) {
        // Skip status-only lines like "completed", "running", etc.
        let dominated = content.trim().to_lowercase();
        if dominated == "completed" || dominated == "running" || dominated == "pending" {
            return;
        }

        // Check if this looks like diff content
        for line in content.lines() {
            let trimmed = line.trim_start();
            let line_type = if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
                OutputType::DiffAdd
            } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
                OutputType::DiffRemove
            } else if trimmed.starts_with("@@") || trimmed.starts_with("diff ")
                    || trimmed.starts_with("index ") || trimmed.starts_with("---")
                    || trimmed.starts_with("+++") {
                OutputType::DiffHeader
            } else if line.starts_with("Added ") || line.starts_with("Removed ")
                    || line.contains(" lines,") {
                OutputType::DiffHeader
            } else {
                OutputType::ToolOutput
            };
            self.output.push(OutputLine {
                content: line.to_string(),
                line_type,
            });
        }
        self.last_activity = Some(Instant::now());
    }

    /// Create a mock session for UI development
    pub fn mock(id: &str, name: &str, agent_type: AgentType, branch: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            agent_type,
            state: SessionState::Idle,
            cwd: PathBuf::from(format!("~/Code/{}", name)),
            git_branch: branch.to_string(),
            is_worktree: false,
            tokens_input: 0,
            tokens_output: 0,
            output: vec![],
            last_activity: None,
            scroll_offset: 0,
            pending_permission: None,
            pending_question: None,
            plan_entries: vec![],
            current_mode: None,
            active_tool_call_id: None,
            permission_mode: PermissionMode::default(),
            available_models: vec![],
            current_model_id: None,
        }
    }
}
