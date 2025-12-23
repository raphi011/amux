//! Session picker component.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the session picker for resuming sessions.
pub fn render_session_picker(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.session_picker {
        // Header
        lines.push(Line::from(vec![Span::styled(
            "Resume session",
            Style::new().fg(LOGO_LIGHT_BLUE).bold(),
        )]));
        lines.push(Line::raw("")); // spacing

        // List sessions
        for (i, session) in picker.sessions.iter().enumerate() {
            let is_selected = i == picker.selected;
            let cursor = if is_selected { "> " } else { "  " };

            // First line: cursor + folder name + timestamp
            let folder_name = session
                .cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let timestamp = session
                .timestamp
                .map(|t| t.format("%b %d %H:%M").to_string())
                .unwrap_or_default();

            let first_spans = vec![
                Span::raw(cursor),
                Span::styled("📁 ", Style::new().fg(LOGO_GOLD)),
                Span::styled(
                    folder_name,
                    if is_selected {
                        Style::new().fg(TEXT_WHITE).bold()
                    } else {
                        Style::new().fg(TEXT_WHITE)
                    },
                ),
                Span::raw("  "),
                Span::styled(timestamp, Style::new().fg(TEXT_DIM)),
            ];
            lines.push(Line::from(first_spans));

            // Second line: first prompt (truncated)
            if let Some(prompt) = &session.first_prompt {
                // Truncate to fit
                let max_len = area.width.saturating_sub(6) as usize;
                let display = if prompt.len() > max_len {
                    // Find valid char boundary
                    let mut end = max_len.saturating_sub(3);
                    while end > 0 && !prompt.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &prompt[..end])
                } else {
                    prompt.clone()
                };

                lines.push(Line::from(vec![
                    Span::raw("     "),
                    Span::styled(display, Style::new().fg(TEXT_DIM).italic()),
                ]));
            }

            lines.push(Line::raw("")); // spacing
        }

        if picker.sessions.is_empty() {
            lines.push(Line::styled(
                "  (no resumable sessions found)",
                Style::new().fg(TEXT_DIM),
            ));
        }
    }

    let paragraph = Paragraph::new(lines).style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}
