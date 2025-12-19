#![allow(dead_code)]

use std::path::PathBuf;

use crate::session::{Session, SessionManager, AgentType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,        // Navigation mode
    Insert,        // Typing mode
    FolderPicker,  // Selecting folder for new session
    AgentPicker,   // Selecting agent type for new session
    SessionPicker, // Selecting session to resume
}

/// Entry in the folder picker
#[derive(Debug, Clone)]
pub struct FolderEntry {
    pub name: String,
    pub path: PathBuf,
    pub git_branch: Option<String>,
    pub is_parent: bool, // ".." entry
}

/// State for the folder picker
#[derive(Debug, Clone)]
pub struct FolderPickerState {
    pub current_dir: PathBuf,
    pub entries: Vec<FolderEntry>,
    pub selected: usize,
}

impl FolderPickerState {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            current_dir: dir,
            entries: vec![],
            selected: 0,
        }
    }
}

/// A resumable session from Claude's storage
#[derive(Debug, Clone)]
pub struct ResumableSession {
    pub session_id: String,
    pub cwd: PathBuf,
    pub first_prompt: Option<String>,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub slug: Option<String>,
}

/// State for the session picker
#[derive(Debug, Clone)]
pub struct SessionPickerState {
    pub sessions: Vec<ResumableSession>,
    pub selected: usize,
}

impl SessionPickerState {
    pub fn new(sessions: Vec<ResumableSession>) -> Self {
        Self { sessions, selected: 0 }
    }

    pub fn select_next(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = (self.selected + 1) % self.sessions.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.sessions.len() - 1);
        }
    }

    pub fn selected_session(&self) -> Option<&ResumableSession> {
        self.sessions.get(self.selected)
    }
}

impl FolderPickerState {
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.entries.len() - 1);
        }
    }

    pub fn selected_entry(&self) -> Option<&FolderEntry> {
        self.entries.get(self.selected)
    }
}

/// State for the agent picker
#[derive(Debug, Clone)]
pub struct AgentPickerState {
    pub cwd: PathBuf,
    pub selected: usize,
}

impl AgentPickerState {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd, selected: 0 }
    }

    pub fn agents() -> &'static [AgentType] {
        &[AgentType::ClaudeCode, AgentType::GeminiCli]
    }

    pub fn select_next(&mut self) {
        let len = Self::agents().len();
        self.selected = (self.selected + 1) % len;
    }

    pub fn select_prev(&mut self) {
        let len = Self::agents().len();
        self.selected = self.selected.checked_sub(1).unwrap_or(len - 1);
    }

    pub fn selected_agent(&self) -> AgentType {
        Self::agents()[self.selected]
    }
}

/// Spinner frames for loading animation
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct App {
    pub sessions: SessionManager,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub viewport_height: usize,
    pub folder_picker: Option<FolderPickerState>,
    pub agent_picker: Option<AgentPickerState>,
    pub session_picker: Option<SessionPickerState>,
    pub spinner_frame: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            sessions: SessionManager::new(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            cursor_position: 0,
            viewport_height: 20, // Default, updated on render
            folder_picker: None,
            agent_picker: None,
            session_picker: None,
            spinner_frame: 0,
        }
    }

    /// Advance spinner animation
    pub fn tick_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
    }

    /// Get current spinner character
    pub fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Open the folder picker starting at the given directory
    pub fn open_folder_picker(&mut self, start_dir: PathBuf) {
        self.folder_picker = Some(FolderPickerState::new(start_dir));
        self.input_mode = InputMode::FolderPicker;
    }

    /// Close the folder picker without selecting
    pub fn close_folder_picker(&mut self) {
        self.folder_picker = None;
        self.input_mode = InputMode::Normal;
    }

    /// Update folder picker entries (called after scanning directory)
    pub fn set_folder_entries(&mut self, entries: Vec<FolderEntry>) {
        if let Some(picker) = &mut self.folder_picker {
            picker.entries = entries;
            picker.selected = 0;
        }
    }

    /// Navigate into selected folder or go to parent
    pub fn folder_picker_enter(&mut self) -> Option<PathBuf> {
        if let Some(picker) = &mut self.folder_picker {
            if let Some(entry) = picker.entries.get(picker.selected) {
                if entry.is_parent {
                    // Go to parent directory
                    if let Some(parent) = picker.current_dir.parent() {
                        picker.current_dir = parent.to_path_buf();
                        picker.selected = 0;
                        return None; // Signal to rescan
                    }
                } else {
                    // Return selected path for spawning session
                    return Some(entry.path.clone());
                }
            }
        }
        None
    }

    /// Enter into a subdirectory
    pub fn folder_picker_enter_dir(&mut self) -> bool {
        if let Some(picker) = &mut self.folder_picker {
            if let Some(entry) = picker.entries.get(picker.selected) {
                if !entry.is_parent {
                    picker.current_dir = entry.path.clone();
                    picker.selected = 0;
                    return true; // Signal to rescan
                }
            }
        }
        false
    }

    /// Go to parent directory
    pub fn folder_picker_go_up(&mut self) -> bool {
        if let Some(picker) = &mut self.folder_picker {
            if let Some(parent) = picker.current_dir.parent() {
                picker.current_dir = parent.to_path_buf();
                picker.selected = 0;
                return true; // Signal to rescan
            }
        }
        false
    }

    /// Open the agent picker for the given directory
    pub fn open_agent_picker(&mut self, cwd: PathBuf) {
        self.agent_picker = Some(AgentPickerState::new(cwd));
        self.input_mode = InputMode::AgentPicker;
    }

    /// Close the agent picker without selecting
    pub fn close_agent_picker(&mut self) {
        self.agent_picker = None;
        self.input_mode = InputMode::Normal;
    }

    /// Open the session picker with resumable sessions
    pub fn open_session_picker(&mut self, sessions: Vec<ResumableSession>) {
        self.session_picker = Some(SessionPickerState::new(sessions));
        self.input_mode = InputMode::SessionPicker;
    }

    /// Close the session picker without selecting
    pub fn close_session_picker(&mut self) {
        self.session_picker = None;
        self.input_mode = InputMode::Normal;
    }

    /// Update viewport height (called from render)
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
    }

    /// Scroll current session up
    pub fn scroll_up(&mut self, n: usize) {
        if let Some(session) = self.sessions.selected_session_mut() {
            session.scroll_up(n);
        }
    }

    /// Scroll current session down
    pub fn scroll_down(&mut self, n: usize) {
        if let Some(session) = self.sessions.selected_session_mut() {
            session.scroll_down(n);
        }
    }

    /// Scroll to top of output
    pub fn scroll_to_top(&mut self) {
        if let Some(session) = self.sessions.selected_session_mut() {
            session.scroll_offset = 0;
        }
    }

    /// Scroll to bottom of output
    pub fn scroll_to_bottom(&mut self) {
        if let Some(session) = self.sessions.selected_session_mut() {
            session.scroll_to_bottom(self.viewport_height);
        }
    }

    pub fn next_session(&mut self) {
        self.sessions.select_next();
    }

    pub fn prev_session(&mut self) {
        self.sessions.select_prev();
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.selected_session()
    }

    /// Spawn a new session and return its index
    pub fn spawn_session(&mut self, agent_type: AgentType, cwd: PathBuf) -> usize {
        let name = cwd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let id = format!("session_{}", self.sessions.len() + 1);
        let session = Session::new(id, name, agent_type, cwd);

        self.sessions.add_session(session);
        self.sessions.len() - 1
    }

    /// Kill the currently selected session
    pub fn kill_selected_session(&mut self) {
        self.sessions.remove_selected();
    }

    /// Enter insert mode
    pub fn enter_insert_mode(&mut self) {
        self.input_mode = InputMode::Insert;
    }

    /// Exit to normal mode
    pub fn exit_insert_mode(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    /// Add a character to input buffer
    pub fn input_char(&mut self, c: char) {
        self.input_buffer.insert(self.cursor_position, c);
        self.cursor_position += 1;
    }

    /// Delete character before cursor
    pub fn input_backspace(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.input_buffer.remove(self.cursor_position);
        }
    }

    /// Delete character at cursor
    pub fn input_delete(&mut self) {
        if self.cursor_position < self.input_buffer.len() {
            self.input_buffer.remove(self.cursor_position);
        }
    }

    /// Move cursor left
    pub fn input_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    /// Move cursor right
    pub fn input_right(&mut self) {
        if self.cursor_position < self.input_buffer.len() {
            self.cursor_position += 1;
        }
    }

    /// Take the input buffer (clears it)
    pub fn take_input(&mut self) -> String {
        self.cursor_position = 0;
        std::mem::take(&mut self.input_buffer)
    }
}
