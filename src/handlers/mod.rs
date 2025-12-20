//! Event handlers module
//!
//! This module contains handlers for different event types:
//! - `agent` - Agent event processing
//! - `keyboard` - Keyboard input handling for different modes
//! - `permission` - Permission request/response handling
//! - `question` - User question request/response handling

mod agent;
mod keyboard;
mod permission;
mod question;

pub use agent::{handle_agent_event, EventResult};
pub use keyboard::KeyboardHandler;
pub use permission::PermissionService;
pub use question::QuestionService;
