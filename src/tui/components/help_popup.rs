//! Help popup component.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the help popup with keyboard shortcuts.
#[allow(clippy::vec_init_then_push)]
pub fn render_help_popup(frame: &mut Frame, area: Rect, app: &App) {
    // Calculate centered popup area
    let popup_width = 50u16;
    let popup_height = 28u16; // Increased to fit bug report line
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
        "Keyboard Shortcuts",
        Style::new().fg(TEXT_WHITE).bold(),
    )]));
    lines.push(Line::raw(""));

    // Normal mode
    lines.push(Line::styled(
        "Normal Mode",
        Style::new().fg(LOGO_LIGHT_BLUE).bold(),
    ));
    lines.push(Line::from(vec![
        Span::styled("  i       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Enter insert mode", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  n       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("New session", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  w       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("New worktree session", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  x       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Kill session", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  d       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Duplicate session", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  c       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Clear session (restart)", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  v       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Cycle sort mode", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  j/k     ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Navigate sessions", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  1-9     ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Select session by number", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  C-u/C-d ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Scroll half page", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  g/G     ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Scroll to top/bottom", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Tab     ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Cycle permission mode", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  m       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Cycle model", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  q       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Quit", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::raw(""));

    // Bug report section with session ID
    lines.push(Line::styled(
        "Bug Reports",
        Style::new().fg(LOGO_CORAL).bold(),
    ));
    lines.push(Line::from(vec![
        Span::styled("  B       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Report bug", Style::new().fg(TEXT_DIM)),
    ]));
    if let Some(sid) = &app.session_id {
        lines.push(Line::from(vec![
            Span::styled("  Session ", Style::new().fg(TEXT_DIM)),
            Span::styled(sid.clone(), Style::new().fg(LOGO_GOLD)),
        ]));
    }
    lines.push(Line::raw(""));

    // Insert mode
    lines.push(Line::styled(
        "Insert Mode",
        Style::new().fg(LOGO_MINT).bold(),
    ));
    lines.push(Line::from(vec![
        Span::styled("  Enter   ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Send message", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Esc     ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Cancel / Normal mode", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  C-v     ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Paste (text or image)", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::raw(""));

    // Footer
    lines.push(Line::from(vec![
        Span::styled("Press ", Style::new().fg(TEXT_DIM)),
        Span::styled("?", Style::new().fg(TEXT_WHITE)),
        Span::styled(" or ", Style::new().fg(TEXT_DIM)),
        Span::styled("Esc", Style::new().fg(TEXT_WHITE)),
        Span::styled(" to close", Style::new().fg(TEXT_DIM)),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(LOGO_LIGHT_BLUE))
        .style(Style::new().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}
