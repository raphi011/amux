//! Bug report popup component.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::tui::theme::*;

use super::wrap_text;

/// Render the bug report popup.
pub fn render_bug_report_popup(frame: &mut Frame, area: Rect, app: &App) {
    // Calculate centered popup area
    let popup_width = 60u16;
    let popup_height = 12u16;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(
        x,
        y,
        popup_width.min(area.width),
        popup_height.min(area.height),
    );

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = vec![
        // Title
        Line::from(vec![Span::styled(
            "Report a Bug",
            Style::new().fg(LOGO_CORAL).bold(),
        )]),
        Line::raw(""),
        // Instructions
        Line::from(vec![Span::styled(
            "Describe the bug you encountered:",
            Style::new().fg(TEXT_DIM),
        )]),
        Line::raw(""),
    ];

    // Input field
    let description = if let Some(bug_report) = &app.bug_report {
        &bug_report.description
    } else {
        ""
    };

    // Wrap input to fit popup width (minus borders and padding)
    let input_width = (popup_width - 4) as usize;
    let wrapped = wrap_text(description, input_width);
    for line_text in &wrapped {
        lines.push(Line::from(vec![
            Span::styled("> ", Style::new().fg(LOGO_MINT)),
            Span::styled(line_text.clone(), Style::new().fg(TEXT_WHITE)),
        ]));
    }

    lines.push(Line::raw(""));

    // Session ID and log path info
    if let Some(sid) = &app.session_id {
        lines.push(Line::from(vec![
            Span::styled("Session ID: ", Style::new().fg(TEXT_DIM)),
            Span::styled(sid.clone(), Style::new().fg(LOGO_GOLD).bold()),
        ]));
    }
    if let Some(bug_report) = &app.bug_report {
        let log_display = bug_report
            .log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(no log)");
        lines.push(Line::from(vec![
            Span::styled("Log file: ", Style::new().fg(TEXT_DIM)),
            Span::styled(log_display, Style::new().fg(LOGO_LIGHT_BLUE)),
        ]));
    }

    lines.push(Line::raw(""));

    // Footer
    lines.push(Line::from(vec![
        Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
        Span::styled(" submit  ", Style::new().fg(TEXT_DIM)),
        Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
        Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(LOGO_CORAL))
        .style(Style::new().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);

    // Set cursor position
    if let Some(bug_report) = &app.bug_report {
        let char_pos = bug_report.description[..bug_report.cursor_position]
            .chars()
            .count();
        let cursor_line = char_pos / input_width;
        let cursor_col = char_pos % input_width;

        // Account for border (1), prompt "> " (2)
        let cursor_x = popup_area.x + 1 + 2 + cursor_col as u16;
        // Account for border (1), title (1), empty (1), instructions (1), empty (1), then input lines
        let cursor_y = popup_area.y + 5 + cursor_line as u16;

        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}
