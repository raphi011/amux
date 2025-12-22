//! Keyboard event handling by input mode.

#![allow(dead_code)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, InputMode};
use crate::session::SessionState;

use super::Action;

/// Handle keyboard events and return the appropriate action.
pub fn handle_key_event(app: &App, key: KeyEvent) -> Action {
    match app.input_mode {
        InputMode::Normal => handle_normal_mode(app, key),
        InputMode::Insert => handle_insert_mode(app, key),
        InputMode::FolderPicker => handle_folder_picker_mode(key),
        InputMode::WorktreeFolderPicker => handle_worktree_folder_picker_mode(key),
        InputMode::WorktreePicker => handle_worktree_picker_mode(key),
        InputMode::BranchInput => handle_branch_input_mode(key),
        InputMode::AgentPicker => handle_agent_picker_mode(key),
        InputMode::SessionPicker => handle_session_picker_mode(key),
        InputMode::WorktreeCleanup => handle_worktree_cleanup_mode(key),
        InputMode::WorktreeCleanupRepoPicker => handle_worktree_cleanup_repo_picker_mode(key),
        InputMode::Help => handle_help_mode(key),
        InputMode::BugReport => handle_bug_report_mode(key),
        InputMode::ClearConfirm => handle_clear_confirm_mode(key),
    }
}

fn handle_normal_mode(app: &App, key: KeyEvent) -> Action {
    // Check for pending permission or question
    let has_permission = app
        .sessions
        .selected_session()
        .map(|s| s.pending_permission.is_some())
        .unwrap_or(false);

    let has_question = app
        .sessions
        .selected_session()
        .map(|s| s.pending_question.is_some())
        .unwrap_or(false);

    if has_permission {
        return handle_permission_mode(key);
    }

    if has_question {
        return handle_question_mode(app, key);
    }

    // Check if agent is currently prompting (for cancel support)
    let is_prompting = app
        .sessions
        .selected_session()
        .map(|s| s.state == SessionState::Prompting)
        .unwrap_or(false);

    // Normal navigation mode
    match key.code {
        // Cancel running prompt with Esc
        KeyCode::Esc if is_prompting => Action::CancelPrompt,

        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('?') => Action::OpenHelp,
        KeyCode::Char('B') => Action::OpenBugReport,

        // Permission mode cycling
        KeyCode::Tab => Action::CyclePermissionMode,

        // Model cycling
        KeyCode::Char('m') => Action::CycleModel,

        // Session selection by number (using display order)
        KeyCode::Char(c @ '1'..='9') => {
            let display_idx = (c as usize) - ('1' as usize);
            // Convert display index to internal index using the mapping
            if let Some(internal_idx) = app.internal_index_for_display(display_idx) {
                Action::SelectSession(internal_idx)
            } else {
                Action::None
            }
        }

        // Session navigation
        KeyCode::Char('j') | KeyCode::Down => Action::NextSession,
        KeyCode::Char('k') | KeyCode::Up => Action::PrevSession,

        // Enter insert mode
        KeyCode::Char('i') | KeyCode::Enter => {
            if app.sessions.selected_session().is_some() {
                Action::EnterInsertMode
            } else {
                Action::None
            }
        }

        // New session
        KeyCode::Char('n') => Action::OpenFolderPicker(app.start_dir.clone()),

        // Worktree picker
        KeyCode::Char('w') => Action::OpenWorktreePicker,

        // Kill session
        KeyCode::Char('x') => Action::KillSession,

        // Duplicate session
        KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::DuplicateSession
        }

        // Clear session (with confirmation)
        KeyCode::Char('c') => Action::OpenClearConfirm,

        // Cycle sort mode
        KeyCode::Char('v') => Action::CycleSortMode,

        // Toggle debug tool JSON display
        KeyCode::Char('t') => Action::ToggleDebugToolJson,

        // Scroll - vim style
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let half_page = app.viewport_height / 2;
            Action::ScrollUp(half_page)
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let half_page = app.viewport_height / 2;
            Action::ScrollDown(half_page)
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::ScrollUp(app.viewport_height)
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::ScrollDown(app.viewport_height)
        }
        KeyCode::PageUp => Action::ScrollUp(app.viewport_height),
        KeyCode::PageDown => Action::ScrollDown(app.viewport_height),
        KeyCode::Char('g') => Action::ScrollToTop,
        KeyCode::Char('G') => Action::ScrollToBottom,

        _ => Action::None,
    }
}

fn handle_permission_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => Action::AllowPermission,
        KeyCode::Char('n') | KeyCode::Esc => Action::DenyPermission,
        KeyCode::Char('j') | KeyCode::Down => Action::PermissionDown,
        KeyCode::Char('k') | KeyCode::Up => Action::PermissionUp,
        _ => Action::None,
    }
}

fn handle_question_mode(app: &App, key: KeyEvent) -> Action {
    // Check if this is a free text question or has options
    let is_free_text = app
        .sessions
        .selected_session()
        .and_then(|s| s.pending_question.as_ref())
        .map(|q| q.is_free_text())
        .unwrap_or(true);

    match key.code {
        KeyCode::Enter => Action::SubmitAnswer,
        KeyCode::Esc => Action::CancelQuestion,
        KeyCode::Char(c) => Action::QuestionInputChar(c),
        KeyCode::Backspace => Action::QuestionInputBackspace,
        KeyCode::Delete => Action::QuestionInputDelete,
        KeyCode::Left => Action::QuestionInputLeft,
        KeyCode::Right => Action::QuestionInputRight,
        KeyCode::Home => Action::QuestionInputHome,
        KeyCode::End => Action::QuestionInputEnd,
        KeyCode::Up => {
            if is_free_text {
                Action::None
            } else {
                Action::QuestionUp
            }
        }
        KeyCode::Down => {
            if is_free_text {
                Action::None
            } else {
                Action::QuestionDown
            }
        }
        KeyCode::Tab => Action::CyclePermissionMode,
        _ => Action::None,
    }
}

pub fn handle_insert_mode(app: &App, key: KeyEvent) -> Action {
    // Check for pending permission or question (can still interact in insert mode)
    let has_permission = app
        .sessions
        .selected_session()
        .map(|s| s.pending_permission.is_some())
        .unwrap_or(false);

    let has_question = app
        .sessions
        .selected_session()
        .map(|s| s.pending_question.is_some())
        .unwrap_or(false);

    match key.code {
        KeyCode::Esc if app.bash_mode => Action::ExitBashMode,
        KeyCode::Esc if has_permission => Action::DenyPermission,
        KeyCode::Esc => Action::ExitInsertMode,

        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::ClearInput,

        KeyCode::Enter if has_permission => Action::AllowPermission,
        KeyCode::Enter if has_question => Action::SubmitAnswer,
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => Action::InputNewline,
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::InputNewline,
        KeyCode::Enter => Action::SubmitPrompt,

        // Clipboard
        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::PasteClipboard
        }
        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::ClearAttachments
        }

        // Permission mode cycling
        KeyCode::Tab => Action::CyclePermissionMode,

        // Navigation - emacs style
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::InputHome,
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::InputEnd,
        KeyCode::Home => Action::InputHome,
        KeyCode::End => Action::InputEnd,

        // Word navigation
        KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => Action::InputWordLeft,
        KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => Action::InputWordRight,
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => Action::InputWordLeft,
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => Action::InputWordRight,

        // Word/line deletion
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::InputDeleteWordBack
        }
        KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
            Action::InputDeleteWordBack
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::ALT) => {
            Action::InputDeleteWordForward
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::InputKillLine
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::InputKillToStart
        }

        // Attachment navigation
        KeyCode::Up => {
            if app.has_attachments() && app.selected_attachment.is_none() {
                Action::SelectAttachments
            } else {
                Action::None
            }
        }
        KeyCode::Down => {
            if app.selected_attachment.is_some() {
                Action::DeselectAttachments
            } else {
                Action::None
            }
        }

        // Basic editing
        KeyCode::Backspace => {
            if app.selected_attachment.is_some() {
                Action::DeleteSelectedAttachment
            } else {
                Action::InputBackspace
            }
        }
        KeyCode::Delete => {
            if app.selected_attachment.is_some() {
                Action::DeleteSelectedAttachment
            } else {
                Action::InputDelete
            }
        }
        KeyCode::Left => {
            if app.selected_attachment.is_some() {
                Action::AttachmentLeft
            } else {
                Action::InputLeft
            }
        }
        KeyCode::Right => {
            if app.selected_attachment.is_some() {
                Action::AttachmentRight
            } else {
                Action::InputRight
            }
        }

        // Character input
        KeyCode::Char(c) => {
            // Typing deselects attachment
            if app.selected_attachment.is_some() {
                // Will need to handle deselect + input as two actions
                // For now, just input the char (deselect handled in action processing)
            }
            Action::InputChar(c)
        }

        _ => Action::None,
    }
}

fn handle_folder_picker_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseFolderPicker,
        KeyCode::Char('j') | KeyCode::Down => Action::FolderPickerDown,
        KeyCode::Char('k') | KeyCode::Up => Action::FolderPickerUp,
        KeyCode::Char('l') | KeyCode::Right => Action::FolderPickerEnterDir,
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => Action::FolderPickerGoUp,
        KeyCode::Enter => Action::FolderPickerSelect,
        _ => Action::None,
    }
}

fn handle_worktree_folder_picker_mode(key: KeyEvent) -> Action {
    // Same as folder picker but with different selection behavior
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseFolderPicker,
        KeyCode::Char('j') | KeyCode::Down => Action::FolderPickerDown,
        KeyCode::Char('k') | KeyCode::Up => Action::FolderPickerUp,
        KeyCode::Char('l') | KeyCode::Right => Action::FolderPickerEnterDir,
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => Action::FolderPickerGoUp,
        KeyCode::Enter => Action::FolderPickerSelect, // Will be handled specially for worktrees
        _ => Action::None,
    }
}

fn handle_worktree_picker_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseWorktreePicker,
        KeyCode::Char('j') | KeyCode::Down => Action::WorktreePickerDown,
        KeyCode::Char('k') | KeyCode::Up => Action::WorktreePickerUp,
        KeyCode::Char('c') => Action::WorktreePickerCleanup,
        KeyCode::Enter => Action::WorktreePickerSelect,
        _ => Action::None,
    }
}

fn handle_branch_input_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::CloseBranchInput,
        KeyCode::Enter => Action::SubmitBranchInput,
        KeyCode::Tab => Action::BranchInputAcceptAutocomplete,
        KeyCode::Down => Action::BranchInputDown,
        KeyCode::Up => Action::BranchInputUp,
        KeyCode::Char(c) => Action::BranchInputChar(c),
        KeyCode::Backspace => Action::BranchInputBackspace,
        KeyCode::Left => Action::BranchInputLeft,
        KeyCode::Right => Action::BranchInputRight,
        _ => Action::None,
    }
}

fn handle_agent_picker_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseAgentPicker,
        KeyCode::Char('j') | KeyCode::Down => Action::AgentPickerDown,
        KeyCode::Char('k') | KeyCode::Up => Action::AgentPickerUp,
        KeyCode::Enter => Action::AgentPickerSelect,

        // Filter input
        KeyCode::Char(c) => Action::AgentPickerInputChar(c),
        KeyCode::Backspace => Action::AgentPickerInputBackspace,
        KeyCode::Delete => Action::AgentPickerInputDelete,
        KeyCode::Left => Action::AgentPickerInputLeft,
        KeyCode::Right => Action::AgentPickerInputRight,
        KeyCode::Home => Action::AgentPickerInputHome,
        KeyCode::End => Action::AgentPickerInputEnd,

        _ => Action::None,
    }
}

fn handle_session_picker_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseSessionPicker,
        KeyCode::Char('j') | KeyCode::Down => Action::SessionPickerDown,
        KeyCode::Char('k') | KeyCode::Up => Action::SessionPickerUp,
        KeyCode::Enter => Action::SessionPickerSelect,
        _ => Action::None,
    }
}

fn handle_worktree_cleanup_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseWorktreeCleanup,
        KeyCode::Char('j') | KeyCode::Down => Action::WorktreeCleanupDown,
        KeyCode::Char('k') | KeyCode::Up => Action::WorktreeCleanupUp,
        KeyCode::Char(' ') => Action::WorktreeCleanupToggle,
        KeyCode::Char('a') => Action::WorktreeCleanupSelectAll,
        KeyCode::Char('n') => Action::WorktreeCleanupDeselectAll,
        KeyCode::Char('b') => Action::WorktreeCleanupToggleBranches,
        KeyCode::Enter => Action::WorktreeCleanupExecute,
        _ => Action::None,
    }
}

fn handle_worktree_cleanup_repo_picker_mode(key: KeyEvent) -> Action {
    // Same as folder picker
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseFolderPicker,
        KeyCode::Char('j') | KeyCode::Down => Action::FolderPickerDown,
        KeyCode::Char('k') | KeyCode::Up => Action::FolderPickerUp,
        KeyCode::Char('l') | KeyCode::Right => Action::FolderPickerEnterDir,
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => Action::FolderPickerGoUp,
        KeyCode::Enter => Action::FolderPickerSelect,
        _ => Action::None,
    }
}

fn handle_help_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Action::CloseHelp,
        _ => Action::None,
    }
}

fn handle_clear_confirm_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => Action::ClearSession,
        KeyCode::Char('n') | KeyCode::Esc => Action::CloseClearConfirm,
        _ => Action::None,
    }
}

fn handle_bug_report_mode(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::CloseBugReport,
        KeyCode::Enter => Action::SubmitBugReport,
        KeyCode::Char(c) => Action::BugReportInputChar(c),
        KeyCode::Backspace => Action::BugReportInputBackspace,
        KeyCode::Delete => Action::BugReportInputDelete,
        KeyCode::Left => Action::BugReportInputLeft,
        KeyCode::Right => Action::BugReportInputRight,
        KeyCode::Home => Action::BugReportInputHome,
        KeyCode::End => Action::BugReportInputEnd,
        _ => Action::None,
    }
}
