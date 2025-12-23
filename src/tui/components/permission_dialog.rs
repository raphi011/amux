//! Permission dialog component.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::acp::PermissionKind;
use crate::app::{App, ClickRegion};
use crate::events::Action;
use crate::tui::interaction::InteractiveRegion;
use crate::tui::theme::*;

/// Render the permission request dialog.
pub fn render_permission_dialog(frame: &mut Frame, area: Rect, app: &mut App) {
    let mut lines: Vec<Line> = vec![];
    let mut option_count = 0;

    if let Some(session) = app.selected_session()
        && let Some(perm) = &session.pending_permission
    {
        option_count = perm.options.len();

        // Header
        let title = perm.title.clone().unwrap_or_else(|| "Tool".to_string());

        // Parse title as markdown since it often contains backticks
        let skin = ratskin::RatSkin::default();
        let parsed_title_lines = skin.parse(
            ratskin::RatSkin::parse_text(&title),
            area.width.saturating_sub(25), // Account for prefix width
        );

        for (i, mut line) in parsed_title_lines.into_iter().enumerate() {
            if i == 0 {
                line.spans.insert(
                    0,
                    Span::styled("Permission required: ", Style::new().fg(LOGO_GOLD).bold()),
                );
                line.spans
                    .insert(0, Span::styled("⚠ ", Style::new().fg(LOGO_GOLD)));
            }
            lines.push(line);
        }
        lines.push(Line::raw(""));

        // Options
        for (i, option) in perm.options.iter().enumerate() {
            let is_selected = i == perm.selected;
            let cursor = if is_selected { "> " } else { "  " };

            let kind_icon = match option.kind {
                PermissionKind::AllowOnce => "✓",
                PermissionKind::AllowAlways => "✓✓",
                PermissionKind::RejectOnce => "✗",
                PermissionKind::RejectAlways => "✗✗",
                PermissionKind::Unknown => "?",
            };

            let style = if is_selected {
                Style::new().fg(TEXT_WHITE).bold()
            } else {
                Style::new().fg(TEXT_DIM)
            };

            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(kind_icon, style),
                Span::styled(" ", style),
                Span::styled(&option.name, style),
            ]));
        }

        // Help text
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("[y/Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" allow • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[n/Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" deny", Style::new().fg(TEXT_DIM)),
        ]));
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(LOGO_GOLD));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);

    // Register click regions for each option
    // Options start at line 2 (after header line and empty line), accounting for top border
    let content_y = area.y + 1; // +1 for top border
    let options_start_y = content_y + 2; // +2 for header and empty line

    for i in 0..option_count {
        let option_y = options_start_y + i as u16;
        if option_y < area.y + area.height {
            let bounds = ClickRegion::new(area.x, option_y, area.width, 1);
            app.interactions.register(
                InteractiveRegion::clickable(
                    "permission_option",
                    bounds,
                    Action::SelectPermissionOption(i),
                )
                .with_priority(100), // High priority so it captures clicks over other regions
            );
        }
    }
}
