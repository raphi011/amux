#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Instant;
use crate::acp::{PermissionOptionInfo, PermissionKind, PlanEntry};

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
}

impl SessionState {
    pub fn display(&self) -> &'static str {
        match self {
            SessionState::Spawning => "spawning...",
            SessionState::Initializing => "initializing...",
            SessionState::Idle => "idle",
            SessionState::Prompting => "working...",
            SessionState::AwaitingPermission => "⚠ permission required",
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

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub state: SessionState,
    pub cwd: PathBuf,
    pub git_branch: String,
    pub tokens_input: u32,
    pub tokens_output: u32,
    pub output: Vec<OutputLine>,
    pub last_activity: Option<Instant>,
    pub scroll_offset: usize,
    pub pending_permission: Option<PendingPermission>,
    pub plan_entries: Vec<PlanEntry>,
    pub current_mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OutputLine {
    pub content: String,
    pub line_type: OutputType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputType {
    Text,       // Agent response text
    UserInput,  // User's prompt
    ToolCall,
    ToolResult,
    Error,
}

impl Session {
    pub fn new(id: String, name: String, agent_type: AgentType, cwd: PathBuf) -> Self {
        Self {
            id,
            name,
            agent_type,
            state: SessionState::Spawning,
            cwd,
            git_branch: String::new(),
            tokens_input: 0,
            tokens_output: 0,
            output: vec![],
            last_activity: Some(Instant::now()),
            scroll_offset: 0,
            pending_permission: None,
            plan_entries: vec![],
            current_mode: None,
        }
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
    }

    /// Scroll to bottom of output (uses sentinel value, renderer handles actual positioning)
    pub fn scroll_to_bottom(&mut self, _viewport_height: usize) {
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
            if last.line_type == OutputType::Text && !last.content.is_empty() {
                last.content.push_str(&text);
                self.last_activity = Some(Instant::now());
                return;
            }
        }
        // No text line to append to, create new one
        self.add_output(text, OutputType::Text);
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
            tokens_input: 0,
            tokens_output: 0,
            output: vec![],
            last_activity: None,
            scroll_offset: 0,
            pending_permission: None,
            plan_entries: vec![],
            current_mode: None,
        }
    }
}
