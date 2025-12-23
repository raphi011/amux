//! Clear session confirmation popup component.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the clear session confirmation popup.
pub fn render_clear_confirm_popup(frame: &mut Frame, area: Rect, app: &App) {
    // Get session name for display
    let session_name = app
        .selected_session()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "session".to_string());

    // Calculate centered popup area
    let popup_width = 50u16;
    let popup_height = 8u16;
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

    let mut lines: Vec<Line> = vec![];

    // Title
    lines.push(Line::from(vec![Span::styled(
        "Clear Session",
        Style::new().fg(LOGO_CORAL).bold(),
    )]));
    lines.push(Line::raw(""));

    // Warning message
    lines.push(Line::from(vec![Span::styled(
        format!("Clear \"{}\" and start fresh?", session_name),
        Style::new().fg(TEXT_WHITE),
    )]));
    lines.push(Line::from(vec![Span::styled(
        "All conversation history will be lost.",
        Style::new().fg(TEXT_DIM),
    )]));
    lines.push(Line::raw(""));

    // Footer with options
    lines.push(Line::from(vec![
        Span::styled("[y]", Style::new().fg(LOGO_CORAL)),
        Span::styled(" yes  ", Style::new().fg(TEXT_DIM)),
        Span::styled("[n]", Style::new().fg(TEXT_WHITE)),
        Span::styled(" no", Style::new().fg(TEXT_DIM)),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(LOGO_CORAL))
        .style(Style::new().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}
