//! Separator components - vertical and horizontal line separators.

use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};

use crate::tui::theme::*;

/// Render a vertical separator (│ characters).
pub fn render_separator(frame: &mut Frame, area: Rect) {
    let separator: Vec<Line> = (0..area.height)
        .map(|_| Line::styled("│", Style::new().fg(TEXT_DIM)))
        .collect();
    let paragraph = Paragraph::new(separator);
    frame.render_widget(paragraph, area);
}

/// Render a horizontal separator (─ characters).
pub fn render_horizontal_separator(frame: &mut Frame, area: Rect) {
    let separator = "─".repeat(area.width as usize);
    let line = Line::styled(separator, Style::new().fg(TEXT_DIM));
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
