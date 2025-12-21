//! UI components for the TUI.
//!
//! This module organizes UI rendering into logical components. Currently,
//! all render functions live in `ui.rs` and are re-exported here for a cleaner API.
//!
//! # Component Organization
//!
//! - `session_list` - Sidebar with sessions, hotkeys, and plan
//! - `output_area` - Main output/chat area with markdown rendering
//! - `input_bar` - Prompt input with attachments and mode indicators
//! - `permission_dialog` - Permission and question dialogs
//! - `folder_picker` - Various picker dialogs (folder, worktree, agent, etc.)
//! - `help_popup` - Help overlay with keybindings

// Re-export all render functions from ui.rs
// These can be incrementally moved to individual component files
// Note: These are currently unused but provide the public API for components

#[allow(unused_imports)]
pub use super::ui::{
    // Pickers
    render_agent_picker,
    render_branch_input,
    render_folder_picker,
    // Overlays
    render_help_popup,

    // Core components
    render_input_bar,
    render_output_area,
    // Dialogs
    render_permission_dialog,
    render_question_dialog,

    render_session_list,

    render_session_picker,
    render_worktree_cleanup,
    render_worktree_picker,

    // Utilities
    wrap_text,
};
