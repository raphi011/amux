//! Keyboard input handling
//!
//! Provides a structured approach to handling keyboard events
//! across different input modes.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, InputMode};
use crate::session::Session;
use super::{PermissionService, QuestionService};

/// Actions that can result from keyboard handling
#[derive(Debug)]
pub enum KeyAction {
    /// No action needed
    None,
    /// Quit the application
    Quit,
    /// Send a permission response
    SendPermission {
        session_id: String,
        request_id: u64,
        accepted: bool,
    },
    /// Send a question response
    SendQuestion {
        session_id: String,
        request_id: u64,
        answer: String,
    },
    /// Send a prompt to the agent
    SendPrompt {
        session_id: String,
        text: String,
        has_attachments: bool,
    },
    /// Cancel the current prompt
    CancelPrompt {
        session_id: String,
    },
    /// Set the model for a session
    SetModel {
        session_id: String,
        model_id: String,
    },
    /// Scan folder entries (for picker refresh)
    ScanFolder {
        path: std::path::PathBuf,
    },
    /// Spawn a new agent session
    SpawnAgent {
        agent_type: crate::session::AgentType,
        cwd: std::path::PathBuf,
        is_worktree: bool,
    },
    /// Open worktree picker
    OpenWorktreePicker,
    /// Open folder picker
    OpenFolderPicker,
    /// Scan worktrees for cleanup
    ScanWorktreesForCleanup {
        worktree_dir: std::path::PathBuf,
    },
}

/// Handles keyboard input for the application
pub struct KeyboardHandler;

impl KeyboardHandler {
    /// Handle a key event in normal mode when permission is pending
    pub fn handle_permission_mode(
        session: &mut Session,
        code: KeyCode,
    ) -> Option<(u64, bool)> {
        match code {
            KeyCode::Char('y') | KeyCode::Enter => {
                if let Some((request_id, _option_id)) = PermissionService::accept(session) {
                    return Some((request_id, true));
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                if let Some(request_id) = PermissionService::reject(session) {
                    return Some((request_id, false));
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                PermissionService::select_next(session);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                PermissionService::select_prev(session);
            }
            _ => {}
        }
        None
    }

    /// Handle a key event in normal mode when question is pending
    pub fn handle_question_mode(
        session: &mut Session,
        code: KeyCode,
    ) -> Option<(u64, String)> {
        match code {
            KeyCode::Enter => {
                return QuestionService::submit(session);
            }
            KeyCode::Esc => {
                if let Some(request_id) = QuestionService::cancel(session) {
                    return Some((request_id, String::new()));
                }
            }
            KeyCode::Char(c) => {
                QuestionService::input_char(session, c);
            }
            KeyCode::Backspace => {
                QuestionService::input_backspace(session);
            }
            KeyCode::Delete => {
                QuestionService::input_delete(session);
            }
            KeyCode::Left => {
                QuestionService::input_left(session);
            }
            KeyCode::Right => {
                QuestionService::input_right(session);
            }
            KeyCode::Home => {
                QuestionService::input_home(session);
            }
            KeyCode::End => {
                QuestionService::input_end(session);
            }
            KeyCode::Up => {
                QuestionService::select_prev(session);
            }
            KeyCode::Down => {
                QuestionService::select_next(session);
            }
            KeyCode::Tab => {
                session.cycle_permission_mode();
            }
            _ => {}
        }
        None
    }

    /// Handle scroll navigation in normal mode
    pub fn handle_scroll(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                let half_page = app.viewport_height / 2;
                app.scroll_up(half_page);
            }
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                let half_page = app.viewport_height / 2;
                app.scroll_down(half_page);
            }
            KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
                app.scroll_up(app.viewport_height);
            }
            KeyCode::Char('f') if modifiers.contains(KeyModifiers::CONTROL) => {
                app.scroll_down(app.viewport_height);
            }
            KeyCode::PageUp => app.scroll_up(app.viewport_height),
            KeyCode::PageDown => app.scroll_down(app.viewport_height),
            KeyCode::Char('g') => app.scroll_to_top(),
            KeyCode::Char('G') => app.scroll_to_bottom(),
            _ => {}
        }
    }

    /// Check if a key event is a scroll command
    pub fn is_scroll_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
        matches!(
            (code, modifiers.contains(KeyModifiers::CONTROL)),
            (KeyCode::Char('u'), true)
                | (KeyCode::Char('d'), true)
                | (KeyCode::Char('b'), true)
                | (KeyCode::Char('f'), true)
                | (KeyCode::PageUp, _)
                | (KeyCode::PageDown, _)
                | (KeyCode::Char('g'), false)
                | (KeyCode::Char('G'), false)
        )
    }

    /// Handle insert mode input editing with modifiers
    pub fn handle_insert_editing(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            // Word/line navigation
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                app.input_home();
                true
            }
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                app.input_end();
                true
            }
            KeyCode::Home => {
                app.input_home();
                true
            }
            KeyCode::End => {
                app.input_end();
                true
            }
            KeyCode::Left if modifiers.contains(KeyModifiers::ALT) => {
                app.input_word_left();
                true
            }
            KeyCode::Right if modifiers.contains(KeyModifiers::ALT) => {
                app.input_word_right();
                true
            }
            KeyCode::Char('b') if modifiers.contains(KeyModifiers::ALT) => {
                app.input_word_left();
                true
            }
            KeyCode::Char('f') if modifiers.contains(KeyModifiers::ALT) => {
                app.input_word_right();
                true
            }
            // Word/line deletion
            KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                app.input_delete_word_back();
                true
            }
            KeyCode::Backspace if modifiers.contains(KeyModifiers::ALT) => {
                app.input_delete_word_back();
                true
            }
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::ALT) => {
                app.input_delete_word_forward();
                true
            }
            KeyCode::Char('k') if modifiers.contains(KeyModifiers::CONTROL) => {
                app.input_kill_line();
                true
            }
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                app.input_kill_to_start();
                true
            }
            _ => false,
        }
    }
}
