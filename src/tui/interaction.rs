//! Component-level mouse interactivity system.

#![allow(dead_code)]
//!
//! This module provides a trait-based system for handling mouse interactions
//! at the component level. Components register their interactive regions during
//! render, and mouse events are automatically routed to the appropriate component.
//!
//! # Example
//!
//! ```ignore
//! // During render, register an interactive region:
//! registry.register(InteractiveRegion {
//!     id: "session_0",
//!     bounds: ClickRegion::new(x, y, width, height),
//!     on_click: Some(Action::SelectSession(0)),
//!     on_scroll_up: None,
//!     on_scroll_down: None,
//!     priority: 0,
//! });
//!
//! // Mouse events are automatically dispatched to matching regions
//! ```

use crate::app::ClickRegion;
use crate::events::Action;

/// An interactive region that can respond to mouse events.
///
/// Components create these during render to define clickable/scrollable areas.
#[derive(Debug, Clone)]
pub struct InteractiveRegion {
    /// Unique identifier for this region (for debugging/logging)
    pub id: &'static str,

    /// The bounds of this interactive region
    pub bounds: ClickRegion,

    /// Action to dispatch on left click (None = not clickable)
    pub on_click: Option<Action>,

    /// Action to dispatch on scroll up (None = not scrollable)
    pub on_scroll_up: Option<Action>,

    /// Action to dispatch on scroll down (None = not scrollable)
    pub on_scroll_down: Option<Action>,

    /// Priority for overlapping regions (higher = checked first)
    /// Use this for popups/dialogs that should capture clicks over underlying content
    pub priority: i32,
}

impl InteractiveRegion {
    /// Create a new clickable region
    pub fn clickable(id: &'static str, bounds: ClickRegion, action: Action) -> Self {
        Self {
            id,
            bounds,
            on_click: Some(action),
            on_scroll_up: None,
            on_scroll_down: None,
            priority: 0,
        }
    }

    /// Create a new scrollable region
    pub fn scrollable(
        id: &'static str,
        bounds: ClickRegion,
        scroll_up: Action,
        scroll_down: Action,
    ) -> Self {
        Self {
            id,
            bounds,
            on_click: None,
            on_scroll_up: Some(scroll_up),
            on_scroll_down: Some(scroll_down),
            priority: 0,
        }
    }

    /// Create a region that's both clickable and scrollable
    pub fn interactive(
        id: &'static str,
        bounds: ClickRegion,
        on_click: Action,
        scroll_up: Action,
        scroll_down: Action,
    ) -> Self {
        Self {
            id,
            bounds,
            on_click: Some(on_click),
            on_scroll_up: Some(scroll_up),
            on_scroll_down: Some(scroll_down),
            priority: 0,
        }
    }

    /// Set the priority (for builder pattern)
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Check if a point is within this region's bounds
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.bounds.contains(x, y)
    }
}

/// Registry of interactive regions, rebuilt each frame during render.
///
/// Components register their interactive regions here during render,
/// and the mouse handler queries this registry to dispatch events.
#[derive(Debug, Default)]
pub struct InteractionRegistry {
    regions: Vec<InteractiveRegion>,
}

impl InteractionRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Clear all registered regions (call at start of each render)
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Register an interactive region
    pub fn register(&mut self, region: InteractiveRegion) {
        self.regions.push(region);
    }

    /// Register a simple clickable region
    pub fn register_click(&mut self, id: &'static str, bounds: ClickRegion, action: Action) {
        self.register(InteractiveRegion::clickable(id, bounds, action));
    }

    /// Register a scrollable region
    pub fn register_scroll(
        &mut self,
        id: &'static str,
        bounds: ClickRegion,
        scroll_up: Action,
        scroll_down: Action,
    ) {
        self.register(InteractiveRegion::scrollable(
            id,
            bounds,
            scroll_up,
            scroll_down,
        ));
    }

    /// Register a region for a session list item
    pub fn register_session_item(&mut self, session_idx: usize, bounds: ClickRegion) {
        self.register(InteractiveRegion::clickable(
            "session_item",
            bounds,
            Action::SelectSession(session_idx),
        ));
    }

    /// Find the action to dispatch for a click at (x, y)
    ///
    /// Returns the action from the highest-priority region that contains the point
    /// and has a click handler.
    pub fn handle_click(&self, x: u16, y: u16) -> Action {
        // Sort by priority (highest first) and find first match
        let mut candidates: Vec<_> = self
            .regions
            .iter()
            .filter(|r| r.contains(x, y) && r.on_click.is_some())
            .collect();

        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        candidates
            .first()
            .and_then(|r| r.on_click.clone())
            .unwrap_or(Action::None)
    }

    /// Find the action to dispatch for a scroll up at (x, y)
    pub fn handle_scroll_up(&self, x: u16, y: u16) -> Action {
        let mut candidates: Vec<_> = self
            .regions
            .iter()
            .filter(|r| r.contains(x, y) && r.on_scroll_up.is_some())
            .collect();

        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        candidates
            .first()
            .and_then(|r| r.on_scroll_up.clone())
            .unwrap_or(Action::None)
    }

    /// Find the action to dispatch for a scroll down at (x, y)
    pub fn handle_scroll_down(&self, x: u16, y: u16) -> Action {
        let mut candidates: Vec<_> = self
            .regions
            .iter()
            .filter(|r| r.contains(x, y) && r.on_scroll_down.is_some())
            .collect();

        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        candidates
            .first()
            .and_then(|r| r.on_scroll_down.clone())
            .unwrap_or(Action::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_contains() {
        let region =
            InteractiveRegion::clickable("test", ClickRegion::new(10, 10, 20, 10), Action::None);

        assert!(region.contains(10, 10)); // top-left corner
        assert!(region.contains(15, 15)); // center
        assert!(region.contains(29, 19)); // just inside bottom-right
        assert!(!region.contains(30, 20)); // just outside
        assert!(!region.contains(9, 10)); // just left
    }

    #[test]
    fn test_priority_ordering() {
        let mut registry = InteractionRegistry::new();

        // Register overlapping regions with different priorities
        registry.register(
            InteractiveRegion::clickable(
                "background",
                ClickRegion::new(0, 0, 100, 100),
                Action::ScrollToTop,
            )
            .with_priority(0),
        );

        registry.register(
            InteractiveRegion::clickable(
                "popup",
                ClickRegion::new(20, 20, 60, 60),
                Action::ScrollToBottom,
            )
            .with_priority(10),
        );

        // Click in popup area should return popup's action
        assert!(matches!(
            registry.handle_click(50, 50),
            Action::ScrollToBottom
        ));

        // Click outside popup should return background's action
        assert!(matches!(registry.handle_click(5, 5), Action::ScrollToTop));
    }
}
