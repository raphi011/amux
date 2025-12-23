use std::path::PathBuf;

use crate::config::McpServerConfig;
use crate::notification::{NotificationConfig, NotificationManager};
use crate::picker::Picker;
use crate::session::{AgentAvailability, AgentType, Session, SessionManager};
use crate::tui::interaction::InteractionRegistry;

/// Sort/view mode for the session list
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SortMode {
    /// Flat list in creation order
    #[default]
    List,
    /// Sessions grouped by git origin
    Grouped,
    /// Sessions grouped by agent type
    ByAgent,
    /// Sorted alphabetically by name
    ByName,
    /// Sorted by creation time (oldest first)
    ByCreatedTime,
    /// Priority mode: permission prompts first, idle next, running last
    Priority,
}

impl SortMode {
    /// Cycle to the next sort mode
    pub fn next(self) -> Self {
        match self {
            SortMode::List => SortMode::Grouped,
            SortMode::Grouped => SortMode::ByAgent,
            SortMode::ByAgent => SortMode::ByName,
            SortMode::ByName => SortMode::ByCreatedTime,
            SortMode::ByCreatedTime => SortMode::Priority,
            SortMode::Priority => SortMode::List,
        }
    }

    /// Short display name for the mode
    pub fn display_name(self) -> &'static str {
        match self {
            SortMode::List => "list",
            SortMode::Grouped => "grouped",
            SortMode::ByAgent => "by agent",
            SortMode::ByName => "by name",
            SortMode::ByCreatedTime => "by time",
            SortMode::Priority => "priority",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,                    // Navigation mode
    Insert,                    // Typing mode
    FolderPicker,              // Selecting folder for new session
    AgentPicker,               // Selecting agent type for new session
    SessionPicker,             // Selecting session to resume
    Help,                      // Help popup showing all hotkeys
    WorktreePicker,            // Selecting existing worktree or create new
    WorktreeFolderPicker,      // Selecting git repo for worktree
    BranchInput,               // Entering branch name with autocomplete
    WorktreeCleanup,           // Cleaning up merged worktrees
    WorktreeCleanupRepoPicker, // Selecting git repo for worktree cleanup
    BugReport,                 // Entering bug report description
    ClearConfirm,              // Confirming session clear
}

/// Entry in the folder picker
#[derive(Debug, Clone)]
pub struct FolderEntry {
    pub name: String,
    pub path: PathBuf,
    pub git_branch: Option<String>,
    pub is_parent: bool,  // ".." entry
    pub is_current: bool, // "." entry (current directory)
}

/// State for the folder picker
#[derive(Debug, Clone)]
pub struct FolderPickerState {
    pub current_dir: PathBuf,
    pub entries: Vec<FolderEntry>,
    pub selected: usize,
    /// All entries (unfiltered)
    pub all_entries: Vec<FolderEntry>,
    /// Filter query string
    pub query: String,
    /// Cursor position in the query input
    pub query_cursor: usize,
}

impl FolderPickerState {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            current_dir: dir,
            entries: vec![],
            selected: 0,
            all_entries: vec![],
            query: String::new(),
            query_cursor: 0,
        }
    }

    /// Update the filtered list based on the current query
    pub fn update_filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.entries = self
            .all_entries
            .iter()
            .filter(|e| {
                // When filtering, skip parent entry so it doesn't always match first
                if e.is_parent {
                    return query_lower.is_empty();
                }
                e.name.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect();

        // Adjust selected index to stay within bounds
        if self.entries.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.entries.len() - 1);
        }
    }

    /// Add a character to the query
    pub fn query_input_char(&mut self, c: char) {
        self.query.insert(self.query_cursor, c);
        self.query_cursor += c.len_utf8();
        self.update_filter();
    }

    /// Delete character before cursor in query
    pub fn query_backspace(&mut self) {
        if self.query_cursor > 0 {
            let mut new_pos = self.query_cursor - 1;
            while new_pos > 0 && !self.query.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.query.remove(new_pos);
            self.query_cursor = new_pos;
            self.update_filter();
        }
    }

    /// Delete character at cursor in query
    pub fn query_delete(&mut self) {
        if self.query_cursor < self.query.len() {
            self.query.remove(self.query_cursor);
            self.update_filter();
        }
    }

    /// Move query cursor left
    pub fn query_left(&mut self) {
        if self.query_cursor > 0 {
            let mut new_pos = self.query_cursor - 1;
            while new_pos > 0 && !self.query.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.query_cursor = new_pos;
        }
    }

    /// Move query cursor right
    pub fn query_right(&mut self) {
        if self.query_cursor < self.query.len() {
            let mut new_pos = self.query_cursor + 1;
            while new_pos < self.query.len() && !self.query.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            self.query_cursor = new_pos;
        }
    }

    /// Move cursor to start of query
    pub fn query_home(&mut self) {
        self.query_cursor = 0;
    }

    /// Move cursor to end of query
    pub fn query_end(&mut self) {
        self.query_cursor = self.query.len();
    }
}

/// A resumable session from Claude's storage
#[derive(Debug, Clone)]
pub struct ResumableSession {
    #[allow(dead_code)] // TODO: Session resume feature
    pub session_id: String,
    pub cwd: PathBuf,
    pub first_prompt: Option<String>,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// State for the session picker
#[derive(Debug, Clone)]
pub struct SessionPickerState {
    pub sessions: Vec<ResumableSession>,
    pub selected: usize,
}

impl SessionPickerState {
    #[allow(dead_code)] // TODO: Session resume feature
    pub fn new(sessions: Vec<ResumableSession>) -> Self {
        Self {
            sessions,
            selected: 0,
        }
    }
}

impl Picker for SessionPickerState {
    type Item = ResumableSession;

    fn items(&self) -> &[Self::Item] {
        &self.sessions
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected = index;
    }
}

impl FolderPickerState {
    pub fn selected_entry(&self) -> Option<&FolderEntry> {
        self.selected_item()
    }
}

impl Picker for FolderPickerState {
    type Item = FolderEntry;

    fn items(&self) -> &[Self::Item] {
        &self.entries
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected = index;
    }
}

/// State for the agent picker
#[derive(Debug, Clone)]
pub struct AgentPickerState {
    pub cwd: PathBuf,
    pub selected: usize,
    pub is_worktree: bool,
    /// Availability info for each agent
    pub agents: Vec<AgentAvailability>,
    /// Filtered agents based on query
    pub filtered: Vec<AgentAvailability>,
    /// Filter query string
    pub query: String,
    /// Cursor position in the query input
    pub query_cursor: usize,
}

impl AgentPickerState {
    pub fn new(cwd: PathBuf, is_worktree: bool, agents: Vec<AgentAvailability>) -> Self {
        // Start with first available agent selected, or 0 if none available
        let selected = agents.iter().position(|a| a.is_available()).unwrap_or(0);
        let filtered = agents.clone();
        Self {
            cwd,
            selected,
            is_worktree,
            filtered,
            agents,
            query: String::new(),
            query_cursor: 0,
        }
    }

    /// Update the filtered list based on the current query
    pub fn update_filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.filtered = self
            .agents
            .iter()
            .filter(|a| {
                a.agent_type
                    .display_name()
                    .to_lowercase()
                    .contains(&query_lower)
            })
            .cloned()
            .collect();

        // Adjust selected index to stay within bounds
        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.filtered.len() - 1);
        }
    }

    /// Add a character to the query
    pub fn query_input_char(&mut self, c: char) {
        self.query.insert(self.query_cursor, c);
        self.query_cursor += c.len_utf8();
        self.update_filter();
    }

    /// Delete character before cursor in query
    pub fn query_backspace(&mut self) {
        if self.query_cursor > 0 {
            let mut new_pos = self.query_cursor - 1;
            while new_pos > 0 && !self.query.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.query.remove(new_pos);
            self.query_cursor = new_pos;
            self.update_filter();
        }
    }

    /// Delete character at cursor in query
    pub fn query_delete(&mut self) {
        if self.query_cursor < self.query.len() {
            self.query.remove(self.query_cursor);
            self.update_filter();
        }
    }

    /// Move query cursor left
    pub fn query_left(&mut self) {
        if self.query_cursor > 0 {
            let mut new_pos = self.query_cursor - 1;
            while new_pos > 0 && !self.query.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.query_cursor = new_pos;
        }
    }

    /// Move query cursor right
    pub fn query_right(&mut self) {
        if self.query_cursor < self.query.len() {
            let mut new_pos = self.query_cursor + 1;
            while new_pos < self.query.len() && !self.query.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            self.query_cursor = new_pos;
        }
    }

    /// Move cursor to start of query
    pub fn query_home(&mut self) {
        self.query_cursor = 0;
    }

    /// Move cursor to end of query
    pub fn query_end(&mut self) {
        self.query_cursor = self.query.len();
    }

    pub fn selected_agent(&self) -> Option<AgentType> {
        self.selected_item().map(|a| a.agent_type)
    }

    /// Check if any agent is available
    #[allow(dead_code)]
    pub fn any_available(&self) -> bool {
        self.agents.iter().any(|a| a.is_available())
    }
}

impl Picker for AgentPickerState {
    type Item = AgentAvailability;

    fn items(&self) -> &[Self::Item] {
        &self.filtered
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected = index;
    }
}

/// Entry in the worktree picker
#[derive(Debug, Clone)]
pub struct WorktreeEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_create_new: bool,
    /// Whether the worktree has no uncommitted changes
    pub is_clean: bool,
    /// Whether the branch is merged into the default branch
    pub is_merged: bool,
}

/// State for the worktree picker
#[derive(Debug, Clone)]
pub struct WorktreePickerState {
    pub entries: Vec<WorktreeEntry>,
    pub selected: usize,
}

impl WorktreePickerState {
    pub fn new(entries: Vec<WorktreeEntry>) -> Self {
        Self {
            entries,
            selected: 0,
        }
    }

    pub fn selected_entry(&self) -> Option<&WorktreeEntry> {
        self.selected_item()
    }
}

impl Picker for WorktreePickerState {
    type Item = WorktreeEntry;

    fn items(&self) -> &[Self::Item] {
        &self.entries
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected = index;
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
    pub selected: bool,    // Whether this entry is selected for cleanup
    pub is_deleting: bool, // Whether this entry is currently being deleted
}

/// State for the worktree cleanup picker
#[derive(Debug, Clone)]
pub struct WorktreeCleanupState {
    pub repo_path: std::path::PathBuf,
    pub entries: Vec<CleanupEntry>,
    pub cursor: usize,
    pub delete_branches: bool, // Whether to also delete branches
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

    pub fn cleanable_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.is_clean && e.is_merged)
            .count()
    }
}

impl Picker for WorktreeCleanupState {
    type Item = CleanupEntry;

    fn items(&self) -> &[Self::Item] {
        &self.entries
    }

    fn selected_index(&self) -> usize {
        self.cursor
    }

    fn set_selected_index(&mut self, index: usize) {
        self.cursor = index;
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
    /// Filter branches based on current input
    pub fn update_filter(&mut self) {
        let query = self.input.to_lowercase();
        self.filtered = self
            .branches
            .iter()
            .filter(|b| b.name.to_lowercase().contains(&query))
            .cloned()
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    /// Accept the currently selected branch
    pub fn accept_selection(&mut self) {
        if let Some(branch) = self.selected_item() {
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

impl Picker for BranchInputState {
    type Item = BranchEntry;

    fn items(&self) -> &[Self::Item] {
        &self.filtered
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected = index;
    }
}

/// State for bug report input
#[derive(Debug, Clone)]
pub struct BugReportState {
    pub description: String,
    pub cursor_position: usize,
    pub log_path: PathBuf,
}

impl BugReportState {
    pub fn new(log_path: PathBuf) -> Self {
        Self {
            description: String::new(),
            cursor_position: 0,
            log_path,
        }
    }

    pub fn input_char(&mut self, c: char) {
        self.description.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    pub fn input_backspace(&mut self) {
        if self.cursor_position > 0 {
            let mut new_pos = self.cursor_position - 1;
            while new_pos > 0 && !self.description.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.description.remove(new_pos);
            self.cursor_position = new_pos;
        }
    }

    pub fn input_delete(&mut self) {
        if self.cursor_position < self.description.len() {
            self.description.remove(self.cursor_position);
        }
    }

    pub fn input_left(&mut self) {
        if self.cursor_position > 0 {
            let mut new_pos = self.cursor_position - 1;
            while new_pos > 0 && !self.description.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.cursor_position = new_pos;
        }
    }

    pub fn input_right(&mut self) {
        if self.cursor_position < self.description.len() {
            let mut new_pos = self.cursor_position + 1;
            while new_pos < self.description.len() && !self.description.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            self.cursor_position = new_pos;
        }
    }

    pub fn input_home(&mut self) {
        self.cursor_position = 0;
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
        self.worktree_dir
            .join(format!("{}-{}", repo_name, safe_branch))
    }
}

/// Spinner frames for loading animation
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// State for a running bash command
#[derive(Debug, Clone)]
pub struct RunningBashCommand {
    pub command: String,
    pub started_at: std::time::Instant,
}

/// A clickable region in the UI
#[derive(Debug, Clone, Copy, Default)]
pub struct ClickRegion {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl ClickRegion {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// Mapping from display order to internal session indices
/// Updated during each render based on the current sort mode
#[derive(Debug, Clone, Default)]
pub struct SessionDisplayOrder {
    /// Maps display index (0, 1, 2...) to internal session index
    /// e.g., if sorted_indices[0] = 2, then the first displayed session
    /// is actually sessions[2]
    pub display_to_internal: Vec<usize>,
}

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
    pub bug_report: Option<BugReportState>,
    pub spinner_frame: usize,
    pub spinner_tick: usize,
    pub attachments: Vec<ImageAttachment>,
    pub selected_attachment: Option<usize>,
    pub start_dir: PathBuf,
    pub worktree_config: WorktreeConfig,
    /// Interactive regions registry, rebuilt each frame during render
    pub interactions: InteractionRegistry,
    /// Mapping from display index to internal session index, updated during render
    pub session_display_order: SessionDisplayOrder,
    /// Counter for generating unique session IDs
    next_session_id: u64,
    /// Session list sort/view mode
    pub sort_mode: SortMode,
    /// Path to the current log file for bug reports
    pub log_path: Option<PathBuf>,
    /// Unique session ID for this amux instance (for matching logs)
    pub session_id: Option<String>,
    /// Debug mode: show raw ACP JSON under tool calls (toggle with 't')
    pub debug_tool_json: bool,
    /// MCP servers to pass to agent sessions
    pub mcp_servers: Vec<McpServerConfig>,
    /// Whether the input is in bash mode (first char is '!')
    pub bash_mode: bool,
    /// Currently running bash command (for timer display)
    pub running_bash_command: Option<RunningBashCommand>,
    /// Desktop notification manager
    pub notifications: NotificationManager,
    /// Last time git diff stats were refreshed
    pub last_git_refresh: std::time::Instant,
}

impl App {
    pub fn new(
        start_dir: PathBuf,
        worktree_config: WorktreeConfig,
        mcp_servers: Vec<McpServerConfig>,
        notification_config: NotificationConfig,
    ) -> Self {
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
            bug_report: None,
            spinner_frame: 0,
            spinner_tick: 0,
            attachments: Vec::new(),
            selected_attachment: None,
            start_dir,
            worktree_config,
            interactions: InteractionRegistry::new(),
            session_display_order: SessionDisplayOrder::default(),
            next_session_id: 1,
            sort_mode: SortMode::default(),
            log_path: None,
            session_id: None,
            debug_tool_json: false,
            mcp_servers,
            bash_mode: false,
            running_bash_command: None,
            notifications: NotificationManager::new(notification_config),
            last_git_refresh: std::time::Instant::now(),
        }
    }

    /// Toggle debug mode for tool JSON display
    pub fn toggle_debug_tool_json(&mut self) {
        self.debug_tool_json = !self.debug_tool_json;
    }

    /// Get the internal session index for a display index (1-9 hotkeys)
    /// Returns None if the display index is out of bounds
    pub fn internal_index_for_display(&self, display_idx: usize) -> Option<usize> {
        self.session_display_order
            .display_to_internal
            .get(display_idx)
            .copied()
    }

    /// Cycle through sort modes
    pub fn cycle_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next();
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
        if let Some(idx) = self.selected_attachment
            && idx > 0
        {
            self.selected_attachment = Some(idx - 1);
        }
    }

    /// Move attachment selection right
    pub fn attachment_right(&mut self) {
        if let Some(idx) = self.selected_attachment
            && idx + 1 < self.attachments.len()
        {
            self.selected_attachment = Some(idx + 1);
        }
    }

    /// Delete the currently selected attachment
    pub fn delete_selected_attachment(&mut self) {
        if let Some(idx) = self.selected_attachment
            && idx < self.attachments.len()
        {
            self.attachments.remove(idx);
            // Adjust selection
            if self.attachments.is_empty() {
                self.selected_attachment = None;
            } else if idx >= self.attachments.len() {
                self.selected_attachment = Some(self.attachments.len() - 1);
            }
        }
    }

    /// Advance spinner animation (every other tick to slow it down)
    pub fn tick_spinner(&mut self) {
        self.spinner_tick += 1;
        if self.spinner_tick.is_multiple_of(2) {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }

    /// Get current spinner character
    pub fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Check if git diff stats should be refreshed (every 5 seconds)
    pub fn should_refresh_git_stats(&self) -> bool {
        self.last_git_refresh.elapsed() >= std::time::Duration::from_secs(5)
    }

    /// Mark that git stats were just refreshed
    pub fn mark_git_refreshed(&mut self) {
        self.last_git_refresh = std::time::Instant::now();
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
            picker.all_entries = entries;
            picker.query.clear();
            picker.query_cursor = 0;
            picker.update_filter();
            picker.selected = 0;
        }
    }

    /// Enter into a subdirectory
    pub fn folder_picker_enter_dir(&mut self) -> bool {
        if let Some(picker) = &mut self.folder_picker
            && let Some(entry) = picker.entries.get(picker.selected)
            && !entry.is_parent
        {
            picker.current_dir = entry.path.clone();
            picker.selected = 0;
            return true; // Signal to rescan
        }
        false
    }

    /// Go to parent directory
    pub fn folder_picker_go_up(&mut self) -> bool {
        if let Some(picker) = &mut self.folder_picker
            && let Some(parent) = picker.current_dir.parent()
        {
            picker.current_dir = parent.to_path_buf();
            picker.selected = 0;
            return true; // Signal to rescan
        }
        false
    }

    /// Open the agent picker for the given directory
    pub fn open_agent_picker(
        &mut self,
        cwd: PathBuf,
        is_worktree: bool,
        agents: Vec<AgentAvailability>,
    ) {
        self.agent_picker = Some(AgentPickerState::new(cwd, is_worktree, agents));
        self.input_mode = InputMode::AgentPicker;
    }

    /// Close the agent picker without selecting
    pub fn close_agent_picker(&mut self) {
        self.agent_picker = None;
        self.input_mode = InputMode::Normal;
    }

    /// Open the session picker with resumable sessions
    #[allow(dead_code)] // TODO: Session resume feature
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

    /// Open the bug report dialog
    pub fn open_bug_report(&mut self) {
        let log_path = self.log_path.clone().unwrap_or_default();
        self.bug_report = Some(BugReportState::new(log_path));
        self.input_mode = InputMode::BugReport;
    }

    /// Close the bug report dialog
    pub fn close_bug_report(&mut self) {
        self.bug_report = None;
        self.input_mode = InputMode::Normal;
    }

    /// Open the clear session confirmation dialog
    pub fn open_clear_confirm(&mut self) {
        self.input_mode = InputMode::ClearConfirm;
    }

    /// Close the clear session confirmation dialog
    pub fn close_clear_confirm(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    /// Scroll current session up
    pub fn scroll_up(&mut self, n: usize) {
        let viewport = self.viewport_height;
        if let Some(session) = self.sessions.selected_session_mut() {
            // Use total_rendered_lines which accounts for text wrapping
            let total_lines = session.total_rendered_lines.max(viewport);
            session.scroll_up(n, total_lines, viewport);
        }
    }

    /// Scroll current session down
    pub fn scroll_down(&mut self, n: usize) {
        let viewport = self.viewport_height;
        if let Some(session) = self.sessions.selected_session_mut() {
            // Use total_rendered_lines which accounts for text wrapping
            let total_lines = session.total_rendered_lines.max(viewport);
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

    /// Save current input buffer to the selected session
    fn save_input_to_session(&mut self) {
        if let Some(session) = self.sessions.selected_session_mut() {
            session.input_buffer = std::mem::take(&mut self.input_buffer);
            session.input_cursor = self.cursor_position;
            self.cursor_position = 0;
        }
    }

    /// Restore input buffer from the selected session
    fn restore_input_from_session(&mut self) {
        if let Some(session) = self.sessions.selected_session_mut() {
            self.input_buffer = std::mem::take(&mut session.input_buffer);
            self.cursor_position = session.input_cursor;
            session.input_cursor = 0;
        }
    }

    pub fn next_session(&mut self) {
        self.save_input_to_session();
        self.sessions.select_next();
        self.restore_input_from_session();
    }

    pub fn prev_session(&mut self) {
        self.save_input_to_session();
        self.sessions.select_prev();
        self.restore_input_from_session();
    }

    /// Select session by index, saving/restoring input buffers
    pub fn select_session(&mut self, index: usize) {
        self.save_input_to_session();
        self.sessions.select_index(index);
        self.restore_input_from_session();
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.selected_session()
    }

    /// Spawn a new session and return its unique ID
    pub fn spawn_session(
        &mut self,
        agent_type: AgentType,
        cwd: PathBuf,
        is_worktree: bool,
    ) -> String {
        let name = cwd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let id = format!("session_{}", self.next_session_id);
        self.next_session_id += 1;
        let session = Session::new(id.clone(), name, agent_type, cwd, is_worktree);

        // Save current session's input before switching to the new session
        self.save_input_to_session();
        self.sessions.add_session(session);
        // New session has empty input, so no need to restore
        id
    }

    /// Kill the currently selected session
    pub fn kill_selected_session(&mut self) {
        // Clear current input (it belongs to the session being killed)
        self.input_buffer.clear();
        self.cursor_position = 0;
        self.sessions.remove_selected();
        // Restore input from the newly selected session
        self.restore_input_from_session();
    }

    /// Enter insert mode
    pub fn enter_insert_mode(&mut self) {
        self.input_mode = InputMode::Insert;
    }

    /// Exit to normal mode
    pub fn exit_insert_mode(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    /// Exit bash mode (stays in insert mode)
    pub fn exit_bash_mode(&mut self) {
        self.bash_mode = false;
    }

    /// Add a character to input buffer
    pub fn input_char(&mut self, c: char) {
        // Check if typing '!' as first character enters bash mode
        if c == '!' && self.input_buffer.is_empty() && self.cursor_position == 0 {
            self.bash_mode = true;
            // Don't add the '!' to the buffer - it's just the mode indicator
            return;
        }
        self.input_buffer.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// Delete character before cursor
    pub fn input_backspace(&mut self) {
        if self.cursor_position > 0 {
            // Find the previous char boundary
            let mut new_pos = self.cursor_position - 1;
            while new_pos > 0 && !self.input_buffer.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.input_buffer.remove(new_pos);
            self.cursor_position = new_pos;
        } else if self.bash_mode && self.input_buffer.is_empty() {
            // Backspace on empty buffer in bash mode exits bash mode
            self.bash_mode = false;
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
            // Find the previous char boundary
            let mut new_pos = self.cursor_position - 1;
            while new_pos > 0 && !self.input_buffer.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.cursor_position = new_pos;
        }
    }

    /// Move cursor right
    pub fn input_right(&mut self) {
        if self.cursor_position < self.input_buffer.len() {
            // Find the next char boundary
            let mut new_pos = self.cursor_position + 1;
            while new_pos < self.input_buffer.len() && !self.input_buffer.is_char_boundary(new_pos)
            {
                new_pos += 1;
            }
            self.cursor_position = new_pos;
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

    /// Take the input buffer (clears it) and reset bash mode
    pub fn take_input(&mut self) -> String {
        self.cursor_position = 0;
        self.bash_mode = false;
        std::mem::take(&mut self.input_buffer)
    }

    /// Check if currently in bash mode
    pub fn is_bash_mode(&self) -> bool {
        self.bash_mode
    }

    /// Start tracking a running bash command
    pub fn start_bash_command(&mut self, command: String) {
        self.running_bash_command = Some(RunningBashCommand {
            command,
            started_at: std::time::Instant::now(),
        });
    }

    /// Complete the running bash command
    pub fn complete_bash_command(&mut self) {
        self.running_bash_command = None;
    }
}
