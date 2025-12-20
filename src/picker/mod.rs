//! Generic picker module
//!
//! Provides a trait and implementations for list-based selection UI components.
//! This eliminates duplicate select_next/select_prev logic across picker types.

mod traits;

pub use traits::Picker;
