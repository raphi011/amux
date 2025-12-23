//! UI components for the TUI.
//!
//! This module organizes UI rendering into logical components.
//!
//! # Component Organization
//!
//! - `sidebar` - Logo, session list, hotkeys, and plan entries
//! - `conversation_view` - Main conversation/chat area with markdown rendering
//! - `prompt` - Prompt input with attachments and mode indicators
//! - `permission_dialog` - Permission request dialog
//! - `question_dialog` - Agent question dialog
//! - `folder_picker` - Folder selection picker
//! - `worktree_picker` - Worktree selection picker
//! - `branch_input` - Branch name input for worktree creation
//! - `worktree_cleanup` - Worktree cleanup dialog
//! - `agent_picker` - Agent type selection picker
//! - `session_picker` - Session resume picker
//! - `help_popup` - Help overlay with keybindings
//! - `bug_report_popup` - Bug report dialog
//! - `clear_confirm_popup` - Clear session confirmation
//! - `separators` - Vertical and horizontal line separators

mod agent_picker;
mod branch_input;
mod bug_report_popup;
mod clear_confirm_popup;
mod folder_picker;
mod help_popup;
mod prompt;
mod conversation_view;
mod permission_dialog;
mod question_dialog;
mod separators;
mod session_picker;
mod sidebar;
mod worktree_cleanup;
mod worktree_picker;

// Re-export all render functions for use in ui.rs
pub use agent_picker::render_agent_picker;
pub use branch_input::render_branch_input;
pub use bug_report_popup::render_bug_report_popup;
pub use clear_confirm_popup::render_clear_confirm_popup;
pub use folder_picker::render_folder_picker;
pub use help_popup::render_help_popup;
pub use prompt::render_prompt;
pub use conversation_view::render_conversation_view;
pub use permission_dialog::render_permission_dialog;
pub use question_dialog::render_question_dialog;
pub use separators::{render_horizontal_separator, render_separator};
pub use session_picker::render_session_picker;
pub use sidebar::{render_logo, render_session_list};
pub use worktree_cleanup::render_worktree_cleanup;
pub use worktree_picker::render_worktree_picker;

/// Wrap text to fit within width, preserving words where possible.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut result = vec![];

    for line in text.split('\n') {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_char_count = 0;

        // Helper to split a word at character count boundary
        fn split_word_at_chars(s: &str, max_chars: usize) -> (&str, &str) {
            let char_count = s.chars().count();
            if char_count <= max_chars {
                return (s, "");
            }
            // Find byte position after max_chars characters
            let byte_pos = s
                .char_indices()
                .nth(max_chars)
                .map(|(i, _)| i)
                .unwrap_or(s.len());
            (&s[..byte_pos], &s[byte_pos..])
        }

        for word in line.split(' ') {
            let word_char_count = word.chars().count();

            if current_line.is_empty() {
                if word_char_count > width {
                    // Word is too long, split it
                    let mut remaining = word;
                    while remaining.chars().count() > width {
                        let (chunk, rest) = split_word_at_chars(remaining, width);
                        result.push(chunk.to_string());
                        remaining = rest;
                    }
                    current_line = remaining.to_string();
                    current_char_count = remaining.chars().count();
                } else {
                    current_line = word.to_string();
                    current_char_count = word_char_count;
                }
            } else if current_char_count + 1 + word_char_count > width {
                // Line would be too long, start new line
                result.push(current_line);
                if word_char_count > width {
                    let mut remaining = word;
                    while remaining.chars().count() > width {
                        let (chunk, rest) = split_word_at_chars(remaining, width);
                        result.push(chunk.to_string());
                        remaining = rest;
                    }
                    current_line = remaining.to_string();
                    current_char_count = remaining.chars().count();
                } else {
                    current_line = word.to_string();
                    current_char_count = word_char_count;
                }
            } else {
                current_line.push(' ');
                current_line.push_str(word);
                current_char_count += 1 + word_char_count;
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}
