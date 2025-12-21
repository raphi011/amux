//! Mouse event handling.
//!
//! Mouse events are dispatched through the interaction registry, which is
//! populated by UI components during each render. This allows components
//! to define their own clickable/scrollable regions without modifying
//! the mouse handler.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::App;

use super::Action;

/// Handle mouse events and return the appropriate action.
///
/// This function delegates to the interaction registry for click and scroll
/// events. Components register their interactive regions during render,
/// and the registry handles hit testing and action dispatch.
pub fn handle_mouse_event(app: &App, mouse: MouseEvent) -> Action {
    let x = mouse.column;
    let y = mouse.row;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            // Check interaction registry for component-specific scroll handling
            let action = app.interactions.handle_scroll_up(x, y);
            if matches!(action, Action::None) {
                // Default scroll behavior if no component handles it
                Action::ScrollUp(3)
            } else {
                action
            }
        }
        MouseEventKind::ScrollDown => {
            let action = app.interactions.handle_scroll_down(x, y);
            if matches!(action, Action::None) {
                Action::ScrollDown(3)
            } else {
                action
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // Use interaction registry for click handling
            app.interactions.handle_click(x, y)
        }
        _ => Action::None,
    }
}
