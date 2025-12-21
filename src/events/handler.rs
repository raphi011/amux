//! Central event handler that coordinates keyboard, mouse, and agent events.

#![allow(dead_code)]

use crossterm::event::{Event, KeyEventKind};

use crate::acp::PermissionOptionId;
use crate::app::App;

use super::Action;
use super::keyboard::handle_key_event;
use super::mouse::handle_mouse_event;

/// Result of handling an agent event - may contain a command to send back
#[derive(Debug)]
pub enum EventResult {
    /// No special action needed
    None,
    /// Auto-accept permission (for AcceptAll mode)
    AutoAcceptPermission {
        request_id: u64,
        option_id: PermissionOptionId,
    },
}

/// Central event handler for the application.
pub struct EventHandler;

impl EventHandler {
    /// Handle a crossterm event (keyboard, mouse, paste) and return an action.
    pub fn handle_event(app: &App, event: &Event) -> Action {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => handle_key_event(app, *key),
            Event::Mouse(mouse) => handle_mouse_event(app, *mouse),
            Event::Paste(_) => Action::None, // Paste is handled specially in main.rs
            _ => Action::None,
        }
    }
}
