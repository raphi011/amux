//! Mouse event handling.

#![allow(dead_code)]

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::App;

use super::Action;

/// Handle mouse events and return the appropriate action.
pub fn handle_mouse_event(app: &App, mouse: MouseEvent) -> Action {
    match mouse.kind {
        MouseEventKind::ScrollUp => Action::ScrollUp(3),
        MouseEventKind::ScrollDown => Action::ScrollDown(3),
        MouseEventKind::Down(MouseButton::Left) => handle_left_click(app, mouse.column, mouse.row),
        _ => Action::None,
    }
}

fn handle_left_click(app: &App, x: u16, y: u16) -> Action {
    // Check if click is on input field - enter insert mode
    if app.click_areas.input_field.contains(x, y) && app.sessions.selected_session().is_some() {
        return Action::EnterInsertMode;
    }

    // Check if click is on permission mode toggle
    if app.click_areas.permission_mode.contains(x, y) {
        return Action::CyclePermissionMode;
    }

    // Check if click is on model selector
    if app.click_areas.model_selector.contains(x, y) {
        return Action::CycleModel;
    }

    // Check if click is on a session in the list
    for (session_idx, region) in &app.click_areas.session_items {
        if region.contains(x, y) {
            return Action::SelectSession(*session_idx);
        }
    }

    Action::None
}
