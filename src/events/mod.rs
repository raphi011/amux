//! Event handling module for keyboard, mouse, and agent events.
//!
//! This module extracts event handling logic from main.rs into focused handlers
//! that dispatch Actions for state changes.
//!
//! # Migration Plan
//!
//! This module provides the foundation for migrating from inline event handling
//! in main.rs to an Action-based architecture. The integration will be done
//! incrementally to minimize risk.

mod action;
mod handler;
mod keyboard;
mod mouse;

// Re-export public types
// Note: These are not yet fully integrated into main.rs but provide the
// infrastructure for the Action-based event handling pattern.
#[allow(unused_imports)]
pub use action::Action;
#[allow(unused_imports)]
pub use handler::{EventHandler, EventResult};
