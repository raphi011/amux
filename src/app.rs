#![allow(dead_code)]

use std::path::PathBuf;

use crate::session::{Session, SessionManager, AgentType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,              // Navigation mode
    Insert,              // Typing mode
    FolderPicker,        // Selecting folder for new session
    AgentPicker,         // Selecting agent type for new session
    SessionPicker,       // Selecting session to resume
    Help,                // Help popup showing all hotkeys
    WorktreePicker,      // Selecting existing worktree or create new
    WorktreeFolderPicker, // Selecting git repo for worktree
    BranchInput,         // Entering branch name with autocomplete
    WorktreeCleanup,     // Cleaning up merged worktrees
    WorktreeCleanupRepoPicker, // Selecting git repo for worktree cleanup
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
    pub is_worktree: bool,
}

impl AgentPickerState {
    pub fn new(cwd: PathBuf, is_worktree: bool) -> Self {
        Self { cwd, selected: 0, is_worktree }
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

/// Entry in the worktree picker
#[derive(Debug, Clone)]
pub struct WorktreeEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_create_new: bool,
}

/// State for the worktree picker
#[derive(Debug, Clone)]
pub struct WorktreePickerState {
    pub entries: Vec<WorktreeEntry>,
    pub selected: usize,
}

impl WorktreePickerState {
    pub fn new(entries: Vec<WorktreeEntry>) -> Self {
        Self { entries, selected: 0 }
    }

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

    pub fn selected_entry(&self) -> Option<&WorktreeEntry> {
        self.entries.get(self.selected)
    }
}

/// A git branch entry for autocomplete
#[derive(Debug, Clone)]
pub struct BranchEntry {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

/// Entry in the worktree cleanup picker
#[derive(Debug, Clone)]
pub struct CleanupEntry {
    pub path: std::path::PathBuf,
    pub branch: Option<String>,
    pub is_clean: bool,
    pub is_merged: bool,
    pub selected: bool,  // Whether this entry is selected for cleanup
}

/// State for the worktree cleanup picker
#[derive(Debug, Clone)]
pub struct WorktreeCleanupState {
    pub repo_path: std::path::PathBuf,
    pub entries: Vec<CleanupEntry>,
    pub cursor: usize,
    pub delete_branches: bool,  // Whether to also delete branches
}

impl WorktreeCleanupState {
    pub fn new(repo_path: std::path::PathBuf, entries: Vec<CleanupEntry>) -> Self {
        Self {
            repo_path,
            entries,
            cursor: 0,
            delete_branches: true,
        }
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = (self.cursor + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = self.cursor.checked_sub(1).unwrap_or(self.entries.len() - 1);
        }
    }

    pub fn toggle_selected(&mut self) {
        if let Some(entry) = self.entries.get_mut(self.cursor) {
            entry.selected = !entry.selected;
        }
    }

    pub fn toggle_delete_branches(&mut self) {
        self.delete_branches = !self.delete_branches;
    }

    pub fn select_all_cleanable(&mut self) {
        for entry in &mut self.entries {
            if entry.is_clean && entry.is_merged {
                entry.selected = true;
            }
        }
    }

    pub fn deselect_all(&mut self) {
        for entry in &mut self.entries {
            entry.selected = false;
        }
    }

    pub fn selected_entries(&self) -> Vec<&CleanupEntry> {
        self.entries.iter().filter(|e| e.selected).collect()
    }

    pub fn has_selection(&self) -> bool {
        self.entries.iter().any(|e| e.selected)
    }

    pub fn cleanable_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_clean && e.is_merged).count()
    }
}

/// State for branch input with autocomplete
#[derive(Debug, Clone)]
pub struct BranchInputState {
    pub repo_path: PathBuf,
    pub input: String,
    pub cursor_position: usize,
    pub branches: Vec<BranchEntry>,
    pub filtered: Vec<BranchEntry>,
    pub selected: usize,
    pub show_autocomplete: bool,
}

impl BranchInputState {
    pub fn new(repo_path: PathBuf, branches: Vec<BranchEntry>) -> Self {
        Self {
            repo_path,
            input: String::new(),
            cursor_position: 0,
            filtered: branches.clone(),
            branches,
            selected: 0,
            show_autocomplete: true,
        }
    }

    /// Filter branches based on current input
    pub fn update_filter(&mut self) {
        let query = self.input.to_lowercase();
        self.filtered = self.branches
            .iter()
            .filter(|b| b.name.to_lowercase().contains(&query))
            .cloned()
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.filtered.len() - 1);
        }
    }

    /// Accept the currently selected branch
    pub fn accept_selection(&mut self) {
        if let Some(branch) = self.filtered.get(self.selected) {
            self.input = branch.name.clone();
            self.cursor_position = self.input.len();
        }
        self.show_autocomplete = false;
    }

    /// Get the branch name to use
    pub fn branch_name(&self) -> &str {
        &self.input
    }
}

/// Configuration for git worktrees
#[derive(Debug, Clone)]
pub struct WorktreeConfig {
    pub worktree_dir: PathBuf,
}

impl WorktreeConfig {
    /// Load worktree config with precedence: cli_override > env var > default
    pub fn load(cli_override: Option<PathBuf>) -> Self {
        let worktree_dir = cli_override
            .or_else(|| std::env::var("AMUX_WORKTREE_DIR").ok().map(PathBuf::from))
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".amux/worktrees")
            });

        Self { worktree_dir }
    }

    /// Generate worktree path for a repo and branch
    pub fn worktree_path(&self, repo_name: &str, branch_name: &str) -> PathBuf {
        // Sanitize branch name for filesystem (replace / with -)
        let safe_branch = branch_name.replace('/', "-");
        self.worktree_dir.join(format!("{}-{}", repo_name, safe_branch))
    }
}

/// Spinner frames for loading animation
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// An image attachment ready to be sent with a prompt
#[derive(Debug, Clone)]
pub struct ImageAttachment {
    pub filename: String,
    pub mime_type: String,
    pub data: String, // base64 encoded
}

pub struct App {
    pub sessions: SessionManager,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub viewport_height: usize,
    pub folder_picker: Option<FolderPickerState>,
    pub agent_picker: Option<AgentPickerState>,
    pub session_picker: Option<SessionPickerState>,
    pub worktree_picker: Option<WorktreePickerState>,
    pub branch_input: Option<BranchInputState>,
    pub worktree_cleanup: Option<WorktreeCleanupState>,
    pub spinner_frame: usize,
    pub attachments: Vec<ImageAttachment>,
    pub selected_attachment: Option<usize>,
    pub start_dir: PathBuf,
    pub worktree_config: WorktreeConfig,
}

impl App {
    pub fn new(start_dir: PathBuf, worktree_config: WorktreeConfig) -> Self {
        Self {
            sessions: SessionManager::new(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            cursor_position: 0,
            viewport_height: 20, // Default, updated on render
            folder_picker: None,
            agent_picker: None,
            session_picker: None,
            worktree_picker: None,
            branch_input: None,
            worktree_cleanup: None,
            spinner_frame: 0,
            attachments: Vec::new(),
            selected_attachment: None,
            start_dir,
            worktree_config,
        }
    }

    /// Add an image attachment
    pub fn add_attachment(&mut self, attachment: ImageAttachment) {
        self.attachments.push(attachment);
        // Don't auto-select, user stays in input mode
    }

    /// Clear all attachments
    pub fn clear_attachments(&mut self) {
        self.attachments.clear();
        self.selected_attachment = None;
    }

    /// Check if there are any attachments
    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }

    /// Select the attachment list (move focus up from input)
    pub fn select_attachments(&mut self) {
        if !self.attachments.is_empty() {
            self.selected_attachment = Some(self.attachments.len() - 1);
        }
    }

    /// Deselect attachments (move focus back to input)
    pub fn deselect_attachments(&mut self) {
        self.selected_attachment = None;
    }

    /// Move attachment selection left
    pub fn attachment_left(&mut self) {
        if let Some(idx) = self.selected_attachment {
            if idx > 0 {
                self.selected_attachment = Some(idx - 1);
            }
        }
    }

    /// Move attachment selection right
    pub fn attachment_right(&mut self) {
        if let Some(idx) = self.selected_attachment {
            if idx + 1 < self.attachments.len() {
                self.selected_attachment = Some(idx + 1);
            }
        }
    }

    /// Delete the currently selected attachment
    pub fn delete_selected_attachment(&mut self) {
        if let Some(idx) = self.selected_attachment {
            if idx < self.attachments.len() {
                self.attachments.remove(idx);
                // Adjust selection
                if self.attachments.is_empty() {
                    self.selected_attachment = None;
                } else if idx >= self.attachments.len() {
                    self.selected_attachment = Some(self.attachments.len() - 1);
                }
            }
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
    pub fn open_agent_picker(&mut self, cwd: PathBuf, is_worktree: bool) {
        self.agent_picker = Some(AgentPickerState::new(cwd, is_worktree));
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

    /// Open the worktree picker with existing worktrees
    pub fn open_worktree_picker(&mut self, entries: Vec<WorktreeEntry>) {
        self.worktree_picker = Some(WorktreePickerState::new(entries));
        self.input_mode = InputMode::WorktreePicker;
    }

    /// Close the worktree picker
    pub fn close_worktree_picker(&mut self) {
        self.worktree_picker = None;
        self.input_mode = InputMode::Normal;
    }

    /// Open folder picker for worktree repo selection
    pub fn open_worktree_folder_picker(&mut self, start_dir: PathBuf) {
        self.folder_picker = Some(FolderPickerState::new(start_dir));
        self.input_mode = InputMode::WorktreeFolderPicker;
    }

    /// Open branch input with autocomplete
    pub fn open_branch_input(&mut self, repo_path: PathBuf, branches: Vec<BranchEntry>) {
        self.branch_input = Some(BranchInputState::new(repo_path, branches));
        self.input_mode = InputMode::BranchInput;
    }

    /// Close branch input
    pub fn close_branch_input(&mut self) {
        self.branch_input = None;
        self.input_mode = InputMode::Normal;
    }

    /// Open worktree cleanup picker
    pub fn open_worktree_cleanup(&mut self, repo_path: PathBuf, entries: Vec<CleanupEntry>) {
        let mut state = WorktreeCleanupState::new(repo_path, entries);
        // Pre-select all cleanable entries
        state.select_all_cleanable();
        self.worktree_cleanup = Some(state);
        self.input_mode = InputMode::WorktreeCleanup;
    }

    /// Close worktree cleanup picker
    pub fn close_worktree_cleanup(&mut self) {
        self.worktree_cleanup = None;
        self.input_mode = InputMode::Normal;
    }

    /// Open the help popup
    pub fn open_help(&mut self) {
        self.input_mode = InputMode::Help;
    }

    /// Close the help popup
    pub fn close_help(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    /// Update viewport height (called from render)
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
    }

    /// Scroll current session up
    pub fn scroll_up(&mut self, n: usize) {
        let viewport = self.viewport_height;
        if let Some(session) = self.sessions.selected_session_mut() {
            // Use output length as approximation for total lines
            // (actual rendered lines may be more due to wrapping, but this is good enough)
            let total_lines = session.output.len().max(viewport);
            session.scroll_up(n, total_lines, viewport);
        }
    }

    /// Scroll current session down
    pub fn scroll_down(&mut self, n: usize) {
        let viewport = self.viewport_height;
        if let Some(session) = self.sessions.selected_session_mut() {
            let total_lines = session.output.len().max(viewport);
            session.scroll_down(n, total_lines, viewport);
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
            session.scroll_to_bottom();
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
    pub fn spawn_session(&mut self, agent_type: AgentType, cwd: PathBuf, is_worktree: bool) -> usize {
        let name = cwd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let id = format!("session_{}", self.sessions.len() + 1);
        let session = Session::new(id, name, agent_type, cwd, is_worktree);

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

    /// Move cursor to start of input
    pub fn input_home(&mut self) {
        self.cursor_position = 0;
    }

    /// Move cursor to end of input
    pub fn input_end(&mut self) {
        self.cursor_position = self.input_buffer.len();
    }

    /// Move cursor to the start of the previous word
    pub fn input_word_left(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        let bytes = self.input_buffer.as_bytes();
        let mut pos = self.cursor_position;

        // Skip any whitespace immediately before cursor
        while pos > 0 && bytes[pos - 1].is_ascii_whitespace() {
            pos -= 1;
        }

        // Skip the word (non-whitespace characters)
        while pos > 0 && !bytes[pos - 1].is_ascii_whitespace() {
            pos -= 1;
        }

        self.cursor_position = pos;
    }

    /// Move cursor to the end of the next word
    pub fn input_word_right(&mut self) {
        let len = self.input_buffer.len();
        if self.cursor_position >= len {
            return;
        }

        let bytes = self.input_buffer.as_bytes();
        let mut pos = self.cursor_position;

        // Skip any whitespace at cursor
        while pos < len && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }

        // Skip the word (non-whitespace characters)
        while pos < len && !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }

        self.cursor_position = pos;
    }

    /// Delete the word before cursor
    pub fn input_delete_word_back(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        let start = self.cursor_position;
        self.input_word_left();
        let end = self.cursor_position;

        // Remove the characters between new position and old position
        self.input_buffer.drain(end..start);
    }

    /// Delete the word after cursor
    pub fn input_delete_word_forward(&mut self) {
        let len = self.input_buffer.len();
        if self.cursor_position >= len {
            return;
        }

        let start = self.cursor_position;

        // Find end of word
        let bytes = self.input_buffer.as_bytes();
        let mut end = start;

        // Skip any whitespace at cursor
        while end < len && bytes[end].is_ascii_whitespace() {
            end += 1;
        }

        // Skip the word (non-whitespace characters)
        while end < len && !bytes[end].is_ascii_whitespace() {
            end += 1;
        }

        // Remove the characters
        self.input_buffer.drain(start..end);
    }

    /// Delete from cursor to end of line
    pub fn input_kill_line(&mut self) {
        self.input_buffer.truncate(self.cursor_position);
    }

    /// Delete from cursor to start of line
    pub fn input_kill_to_start(&mut self) {
        self.input_buffer.drain(..self.cursor_position);
        self.cursor_position = 0;
    }

    /// Take the input buffer (clears it)
    pub fn take_input(&mut self) -> String {
        self.cursor_position = 0;
        std::mem::take(&mut self.input_buffer)
    }
}
