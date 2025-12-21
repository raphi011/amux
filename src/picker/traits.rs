//! Picker trait definition
//!
//! A generic trait for list-based selection components.

/// A generic picker trait for list selection
///
/// This trait provides default implementations for common selection operations,
/// eliminating the need to duplicate select_next/select_prev logic across types.
///
/// # Example
///
/// ```ignore
/// struct MyPicker {
///     items: Vec<String>,
///     selected: usize,
/// }
///
/// impl Picker for MyPicker {
///     type Item = String;
///
///     fn items(&self) -> &[Self::Item] {
///         &self.items
///     }
///
///     fn selected_index(&self) -> usize {
///         self.selected
///     }
///
///     fn set_selected_index(&mut self, index: usize) {
///         self.selected = index;
///     }
/// }
/// ```
pub trait Picker {
    /// The type of items in the picker
    type Item;

    /// Get the list of items
    fn items(&self) -> &[Self::Item];

    /// Get the current selected index
    fn selected_index(&self) -> usize;

    /// Set the selected index
    fn set_selected_index(&mut self, index: usize);

    /// Get the number of items
    fn len(&self) -> usize {
        self.items().len()
    }

    /// Check if the picker is empty
    fn is_empty(&self) -> bool {
        self.items().is_empty()
    }

    /// Select the next item (wraps around)
    fn select_next(&mut self) {
        if !self.is_empty() {
            let next = (self.selected_index() + 1) % self.len();
            self.set_selected_index(next);
        }
    }

    /// Select the previous item (wraps around)
    fn select_prev(&mut self) {
        if !self.is_empty() {
            let prev = self.selected_index()
                .checked_sub(1)
                .unwrap_or(self.len() - 1);
            self.set_selected_index(prev);
        }
    }

    /// Get the currently selected item
    fn selected_item(&self) -> Option<&Self::Item> {
        self.items().get(self.selected_index())
    }

    /// Select a specific index (clamped to valid range)
    #[allow(dead_code)]
    fn select_index(&mut self, index: usize) {
        if !self.is_empty() {
            let clamped = index.min(self.len() - 1);
            self.set_selected_index(clamped);
        }
    }

    /// Reset selection to the first item
    #[allow(dead_code)]
    fn reset_selection(&mut self) {
        self.set_selected_index(0);
    }
}
