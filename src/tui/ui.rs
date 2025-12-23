//! Main UI rendering - coordinates component layout and rendering.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::app::{App, InputMode};

// Re-export components for external use
pub use super::components::{
    render_agent_picker, render_branch_input, render_bug_report_popup, render_clear_confirm_popup,
    render_conversation_view, render_folder_picker, render_help_popup, render_horizontal_separator,
    render_logo, render_permission_dialog, render_prompt, render_question_dialog, render_separator,
    render_session_list, render_session_picker, render_worktree_cleanup, render_worktree_picker,
};

// Layout constants
const SIDEBAR_WIDTH: u16 = 40;
const SIDEBAR_LEFT_PADDING: u16 = 1;
const SEPARATOR_WIDTH: u16 = 1;
const CONTENT_LEFT_PADDING: u16 = 1;
const CONTENT_RIGHT_PADDING: u16 = 1;
const SIDEBAR_INNER_PADDING: u16 = 1;
const BORDER_WIDTH: u16 = 2;

/// Main render function - coordinates layout and delegates to components.
pub fn render(frame: &mut Frame, app: &mut App) {
    // Clear interaction registry at start of each frame
    app.interactions.clear();

    let area = frame.area();

    // Horizontal split: sidebar | left padding | separator | content left padding | main content | content right padding
    let content_layout = Layout::horizontal([
        Constraint::Length(SIDEBAR_WIDTH),
        Constraint::Length(SIDEBAR_LEFT_PADDING),
        Constraint::Length(SEPARATOR_WIDTH),
        Constraint::Length(CONTENT_LEFT_PADDING),
        Constraint::Min(0), // Main content
        Constraint::Length(CONTENT_RIGHT_PADDING),
    ])
    .split(area);

    // Sidebar with 1-char padding on left/right, no top padding
    let sidebar_outer = content_layout[0];
    let sidebar_inner = Rect {
        x: sidebar_outer.x + SIDEBAR_INNER_PADDING,
        y: sidebar_outer.y,
        width: sidebar_outer.width.saturating_sub(BORDER_WIDTH),
        height: sidebar_outer.height,
    };

    // Sidebar: logo + session list (includes hotkeys and plan at bottom)
    let sidebar_layout = Layout::vertical([
        Constraint::Length(1), // Logo (single line)
        Constraint::Min(0),    // Session list + hotkeys + plan
    ])
    .split(sidebar_inner);

    // Render logo at top of sidebar
    render_logo(frame, sidebar_layout[0]);

    // Render session list with hotkeys and plan at bottom
    render_session_list(frame, sidebar_layout[1], app);

    // Check if there's a pending permission or question
    let has_permission = app
        .selected_session()
        .map(|s| s.pending_permission.is_some())
        .unwrap_or(false);

    let has_question = app
        .selected_session()
        .map(|s| s.pending_question.is_some())
        .unwrap_or(false);

    // Render vertical separator
    render_separator(frame, content_layout[2]);

    // Calculate input bar height based on content wrapping
    let input_area_width = content_layout[4].width.saturating_sub(2) as usize; // Account for prompt "> "
    let input_height = if has_permission || has_question {
        0 // No input bar when permission/question dialog is shown
    } else {
        // Calculate wrapped lines for input buffer only (attachments are on separate line)
        let wrapped_lines = if input_area_width > 0 && !app.input_buffer.is_empty() {
            app.input_buffer.len().div_ceil(input_area_width).max(1)
        } else {
            1
        };
        // Add 1 for the mode indicator line, 1 for padding between prompt and mode, plus 1 if there are attachments
        let attachment_line = if app.has_attachments() { 1 } else { 0 };
        (wrapped_lines + 2 + attachment_line) as u16
    };

    // Calculate question dialog height
    let question_height = if has_question {
        if let Some(session) = app.selected_session() {
            if let Some(q) = &session.pending_question {
                // 2 for question + blank, options count, 2 for input, 1 for help
                let options_height = if q.options.is_empty() {
                    0
                } else {
                    q.options.len() as u16 + 1
                };
                5 + options_height
            } else {
                6
            }
        } else {
            6
        }
    } else {
        6
    };

    // Right side: output + separator + permission/question/input
    let right_layout = if has_permission {
        Layout::vertical([
            Constraint::Min(0),    // Output
            Constraint::Length(6), // Permission dialog
        ])
        .split(content_layout[4])
    } else if has_question {
        Layout::vertical([
            Constraint::Min(0),                  // Output
            Constraint::Length(question_height), // Question dialog
        ])
        .split(content_layout[4])
    } else {
        Layout::vertical([
            Constraint::Min(0),                      // Output
            Constraint::Length(1),                   // Empty line above separator
            Constraint::Length(1),                   // Horizontal separator
            Constraint::Length(1),                   // Empty line below separator
            Constraint::Length(input_height.max(2)), // Input bar (min 2 lines: input + mode)
        ])
        .split(content_layout[4])
    };

    // Always render the base content (output area, input bar, etc.)
    // Non-popup pickers still take over the content area
    if app.input_mode == InputMode::BranchInput {
        render_branch_input(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::SessionPicker {
        render_session_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::WorktreeCleanup {
        render_worktree_cleanup(frame, right_layout[0], app);
    } else {
        // Update viewport_height for scroll calculations
        app.viewport_height = right_layout[0].height as usize;
        render_conversation_view(frame, right_layout[0], app);
    }

    // Render permission dialog, question dialog, or input bar
    if has_permission {
        render_permission_dialog(frame, right_layout[1], app);
    } else if has_question {
        render_question_dialog(frame, right_layout[1], app);
    } else {
        // Render horizontal separator (index 1 is empty, 2 is separator, 3 is empty, 4 is input)
        render_horizontal_separator(frame, right_layout[2]);
        render_prompt(frame, right_layout[4], app);
    }

    // === Popup overlays (rendered on top of everything) ===

    // Render folder picker popup on top
    if app.input_mode == InputMode::FolderPicker
        || app.input_mode == InputMode::WorktreeFolderPicker
        || app.input_mode == InputMode::WorktreeCleanupRepoPicker
    {
        render_folder_picker(frame, area, app);
    }

    // Render agent picker popup on top
    if app.input_mode == InputMode::AgentPicker {
        render_agent_picker(frame, area, app);
    }

    // Render help popup on top if in Help mode
    if app.input_mode == InputMode::Help {
        render_help_popup(frame, area, app);
    }

    // Render bug report popup on top if in BugReport mode
    if app.input_mode == InputMode::BugReport {
        render_bug_report_popup(frame, area, app);
    }

    // Render clear session confirmation popup on top if in ClearConfirm mode
    if app.input_mode == InputMode::ClearConfirm {
        render_clear_confirm_popup(frame, area, app);
    }

    // Render worktree picker popup on top
    if app.input_mode == InputMode::WorktreePicker {
        render_worktree_picker(frame, area, app);
    }
}
