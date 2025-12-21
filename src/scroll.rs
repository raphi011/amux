//! Scroll event debouncing for smooth scrolling.
//!
//! Accumulates scroll deltas over a time window to prevent jittery scrolling
//! from high-resolution scroll events (e.g., trackpads, precision mice).
//!
//! # Example
//!
//! ```rust,ignore
//! let mut scroll_helper = ScrollHelper::default();
//!
//! // In event loop:
//! if let Some(delta) = scroll_helper.accumulate(scroll_delta) {
//!     app.scroll(delta);
//! }
//! ```

#![allow(dead_code)]

use std::time::{Duration, Instant};

/// Helper for debouncing scroll events.
///
/// Accumulates scroll deltas and only returns a value after the debounce
/// period has elapsed, preventing rapid-fire scroll events from causing
/// jumpy behavior.
#[derive(Debug, Clone)]
pub struct ScrollHelper {
    /// Accumulated scroll delta since last emission
    accumulated_delta: i32,
    /// Time of last scroll event
    last_event: Option<Instant>,
    /// Debounce duration in milliseconds
    debounce_ms: u64,
    /// Minimum delta threshold before emitting
    threshold: i32,
}

impl Default for ScrollHelper {
    fn default() -> Self {
        Self::new(50, 1)
    }
}

impl ScrollHelper {
    /// Create a new scroll helper with specified debounce time and threshold.
    ///
    /// # Arguments
    /// * `debounce_ms` - Time window for accumulating scroll events (milliseconds)
    /// * `threshold` - Minimum accumulated delta before emitting a scroll action
    pub fn new(debounce_ms: u64, threshold: i32) -> Self {
        Self {
            accumulated_delta: 0,
            last_event: None,
            debounce_ms,
            threshold,
        }
    }

    /// Accumulate a scroll delta and return the accumulated value if ready.
    ///
    /// Returns `Some(delta)` if the debounce period has elapsed since the last
    /// scroll event, or `None` if still accumulating.
    pub fn accumulate(&mut self, delta: i32) -> Option<i32> {
        let now = Instant::now();

        match self.last_event {
            Some(last) => {
                let elapsed = now.duration_since(last);
                if elapsed > Duration::from_millis(self.debounce_ms) {
                    // Debounce period elapsed, start fresh accumulation
                    self.accumulated_delta = delta;
                } else {
                    // Still within debounce window, accumulate
                    self.accumulated_delta += delta;
                }
            }
            None => {
                // First scroll event
                self.accumulated_delta = delta;
            }
        }

        self.last_event = Some(now);

        // Return accumulated value if it exceeds threshold
        if self.accumulated_delta.abs() >= self.threshold {
            let result = self.accumulated_delta;
            self.accumulated_delta = 0;
            Some(result)
        } else {
            None
        }
    }

    /// Reset the scroll helper state.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.accumulated_delta = 0;
        self.last_event = None;
    }

    /// Check if there's a pending accumulated delta that should be flushed.
    ///
    /// Call this periodically (e.g., on timeout) to flush any remaining delta.
    #[allow(dead_code)]
    pub fn flush(&mut self) -> Option<i32> {
        if self.accumulated_delta.abs() >= self.threshold {
            let result = self.accumulated_delta;
            self.accumulated_delta = 0;
            Some(result)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_accumulate_within_debounce() {
        let mut helper = ScrollHelper::new(100, 3);

        // First event should not emit (below threshold)
        assert!(helper.accumulate(1).is_none());
        assert!(helper.accumulate(1).is_none());

        // Third event should emit (reaches threshold)
        assert_eq!(helper.accumulate(1), Some(3));
    }

    #[test]
    fn test_accumulate_after_debounce() {
        let mut helper = ScrollHelper::new(10, 1);

        // First event
        assert_eq!(helper.accumulate(5), Some(5));

        // Wait for debounce to expire
        thread::sleep(Duration::from_millis(20));

        // Second event should start fresh accumulation
        assert_eq!(helper.accumulate(3), Some(3));
    }

    #[test]
    fn test_negative_delta() {
        let mut helper = ScrollHelper::new(100, 2);

        assert!(helper.accumulate(-1).is_none());
        assert_eq!(helper.accumulate(-1), Some(-2));
    }
}
