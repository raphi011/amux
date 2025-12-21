//! Action enum for decoupling input handling from state changes.
//!
//! Actions represent user intents that can be logged, replayed, or customized.

#![allow(dead_code)]

use std::path::PathBuf;

use crate::acp::PermissionOptionId;
use crate::session::AgentType;

/// Actions that can be dispatched from event handlers.
///
/// These represent user intents and are processed by the App to update state.
#[derive(Debug, Clone)]
pub enum Action {
    // === Application ===
    /// Quit the application
    Quit,

    // === Mode switching ===
    /// Enter insert mode for typing
    EnterInsertMode,
    /// Exit to normal mode
    ExitInsertMode,
    /// Open help popup
    OpenHelp,
    /// Close help popup
    CloseHelp,

    // === Session navigation ===
    /// Select next session in list
    NextSession,
    /// Select previous session in list
    PrevSession,
    /// Select session by index (1-9)
    SelectSession(usize),

    // === Session management ===
    /// Open folder picker starting at path
    OpenFolderPicker(PathBuf),
    /// Close folder picker
    CloseFolderPicker,
    /// Open worktree picker
    OpenWorktreePicker,
    /// Close worktree picker
    CloseWorktreePicker,
    /// Open agent picker for directory
    OpenAgentPicker { cwd: PathBuf, is_worktree: bool },
    /// Close agent picker
    CloseAgentPicker,
    /// Spawn agent in directory
    SpawnAgent {
        agent_type: AgentType,
        cwd: PathBuf,
        is_worktree: bool,
    },
    /// Duplicate current session (same folder, same agent)
    DuplicateSession,
    /// Clear current session (replace with fresh session)
    ClearSession,
    /// Open clear session confirmation dialog
    OpenClearConfirm,
    /// Close clear session confirmation dialog
    CloseClearConfirm,
    /// Kill selected session
    KillSession,

    // === Input handling ===
    /// Add character to input buffer
    InputChar(char),
    /// Delete character before cursor
    InputBackspace,
    /// Delete character at cursor
    InputDelete,
    /// Move cursor left
    InputLeft,
    /// Move cursor right
    InputRight,
    /// Move cursor to start
    InputHome,
    /// Move cursor to end
    InputEnd,
    /// Move cursor word left
    InputWordLeft,
    /// Move cursor word right
    InputWordRight,
    /// Delete word before cursor
    InputDeleteWordBack,
    /// Delete word after cursor
    InputDeleteWordForward,
    /// Delete to end of line
    InputKillLine,
    /// Delete to start of line
    InputKillToStart,
    /// Clear input buffer (Ctrl+C)
    ClearInput,
    /// Submit prompt
    SubmitPrompt,

    // === Scrolling ===
    /// Scroll up by n lines
    ScrollUp(usize),
    /// Scroll down by n lines
    ScrollDown(usize),
    /// Scroll to top
    ScrollToTop,
    /// Scroll to bottom
    ScrollToBottom,

    // === Permissions ===
    /// Allow permission request
    AllowPermission,
    /// Deny permission request
    DenyPermission,
    /// Navigate permission options up
    PermissionUp,
    /// Navigate permission options down
    PermissionDown,
    /// Respond to permission with specific option
    RespondPermission {
        request_id: u64,
        option_id: Option<PermissionOptionId>,
    },

    // === Questions (ask_user) ===
    /// Submit answer to question
    SubmitAnswer,
    /// Cancel question (empty response)
    CancelQuestion,
    /// Input character into question answer
    QuestionInputChar(char),
    /// Delete character in question input
    QuestionInputBackspace,
    /// Delete at cursor in question input
    QuestionInputDelete,
    /// Move cursor left in question
    QuestionInputLeft,
    /// Move cursor right in question
    QuestionInputRight,
    /// Move cursor to start in question
    QuestionInputHome,
    /// Move cursor to end in question
    QuestionInputEnd,
    /// Navigate question options up
    QuestionUp,
    /// Navigate question options down
    QuestionDown,

    // === Folder picker ===
    /// Enter selected directory in folder picker
    FolderPickerEnterDir,
    /// Go up to parent directory
    FolderPickerGoUp,
    /// Select folder and proceed
    FolderPickerSelect,
    /// Navigate folder picker up
    FolderPickerUp,
    /// Navigate folder picker down
    FolderPickerDown,

    // === Worktree picker ===
    /// Navigate worktree picker up
    WorktreePickerUp,
    /// Navigate worktree picker down
    WorktreePickerDown,
    /// Select worktree entry
    WorktreePickerSelect,
    /// Open cleanup view
    WorktreePickerCleanup,

    // === Branch input ===
    /// Close branch input
    CloseBranchInput,
    /// Submit branch name
    SubmitBranchInput,
    /// Accept autocomplete selection
    BranchInputAcceptAutocomplete,
    /// Navigate autocomplete up
    BranchInputUp,
    /// Navigate autocomplete down
    BranchInputDown,
    /// Input character into branch name
    BranchInputChar(char),
    /// Delete character in branch input
    BranchInputBackspace,
    /// Move cursor left in branch input
    BranchInputLeft,
    /// Move cursor right in branch input
    BranchInputRight,

    // === Agent picker ===
    /// Navigate agent picker up
    AgentPickerUp,
    /// Navigate agent picker down
    AgentPickerDown,
    /// Select agent
    AgentPickerSelect,

    // === Session picker ===
    /// Close session picker
    CloseSessionPicker,
    /// Navigate session picker up
    SessionPickerUp,
    /// Navigate session picker down
    SessionPickerDown,
    /// Resume selected session
    SessionPickerSelect,

    // === Worktree cleanup ===
    /// Close worktree cleanup
    CloseWorktreeCleanup,
    /// Navigate cleanup picker up
    WorktreeCleanupUp,
    /// Navigate cleanup picker down
    WorktreeCleanupDown,
    /// Toggle selection of current entry
    WorktreeCleanupToggle,
    /// Select all cleanable entries
    WorktreeCleanupSelectAll,
    /// Deselect all entries
    WorktreeCleanupDeselectAll,
    /// Toggle delete branches option
    WorktreeCleanupToggleBranches,
    /// Execute cleanup
    WorktreeCleanupExecute,

    // === Permission mode ===
    /// Cycle permission mode (normal -> plan -> accept all)
    CyclePermissionMode,

    // === Sort mode ===
    /// Cycle sort mode (list -> grouped -> by name -> by time -> priority)
    CycleSortMode,

    // === Model selection ===
    /// Cycle to next model
    CycleModel,
    /// Set specific model
    SetModel {
        session_id: String,
        model_id: String,
    },

    // === Attachments ===
    /// Paste from clipboard
    PasteClipboard,
    /// Clear all attachments
    ClearAttachments,
    /// Select attachment row (move focus up from input)
    SelectAttachments,
    /// Deselect attachments (move focus back to input)
    DeselectAttachments,
    /// Move attachment selection left
    AttachmentLeft,
    /// Move attachment selection right
    AttachmentRight,
    /// Delete selected attachment
    DeleteSelectedAttachment,

    // === Bug Report ===
    /// Open bug report dialog
    OpenBugReport,
    /// Close bug report dialog
    CloseBugReport,
    /// Submit bug report
    SubmitBugReport,
    /// Input character into bug report
    BugReportInputChar(char),
    /// Delete character in bug report
    BugReportInputBackspace,
    /// Delete at cursor in bug report
    BugReportInputDelete,
    /// Move cursor left in bug report
    BugReportInputLeft,
    /// Move cursor right in bug report
    BugReportInputRight,
    /// Move cursor to start in bug report
    BugReportInputHome,
    /// Move cursor to end in bug report
    BugReportInputEnd,

    // === Debug ===
    /// Toggle debug mode for tool JSON display
    ToggleDebugToolJson,

    // === No-op ===
    /// No action to take
    None,
}
