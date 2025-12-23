//! Agent picker popup component.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::session::AgentType;
use crate::tui::theme::*;

/// Render the agent picker as a centered popup.
pub fn render_agent_picker(frame: &mut Frame, area: Rect, app: &App) {
    // Calculate centered popup area
    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 16u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = vec![];

    // Track cursor position for filter input
    let mut cursor_position: Option<(u16, u16)> = None;

    if let Some(picker) = &app.agent_picker {
        // Header with selected directory
        let folder_name = picker
            .cwd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        lines.push(Line::from(vec![
            Span::styled("for ", Style::new().fg(TEXT_DIM)),
            Span::styled(folder_name, Style::new().fg(LOGO_LIGHT_BLUE).bold()),
        ]));
        lines.push(Line::raw("")); // spacing

        // Filter input line
        lines.push(Line::from(vec![
            Span::styled("Filter: ", Style::new().fg(LOGO_LIGHT_BLUE)),
            Span::styled(&picker.query, Style::new().fg(TEXT_WHITE)),
        ]));

        // Calculate cursor position (after "Filter: " which is 8 chars)
        let cursor_x = popup_area.x + 1 + 8 + picker.query_cursor as u16;
        let cursor_y = popup_area.y + 1 + 2; // +1 for border, +2 for header + empty line
        cursor_position = Some((cursor_x, cursor_y));

        lines.push(Line::raw("")); // spacing

        // List agent options with availability status (using filtered list)
        if picker.filtered.is_empty() {
            lines.push(Line::styled(
                "  (no matching agents)",
                Style::new().fg(TEXT_DIM),
            ));
        }

        for (i, availability) in picker.filtered.iter().enumerate() {
            let is_selected = i == picker.selected;
            let is_available = availability.is_available();
            let cursor = if is_selected { "> " } else { "  " };

            let (icon, color) = match availability.agent_type {
                AgentType::ClaudeCode => ("", LOGO_CORAL), // Anthropic orange-ish
                AgentType::GeminiCli => ("", LOGO_LIGHT_BLUE), // Google blue
            };

            let name = availability.agent_type.display_name();

            // Agent name line
            let name_style = if !is_available {
                Style::new().fg(TEXT_DIM)
            } else if is_selected {
                Style::new().fg(TEXT_WHITE).bold()
            } else {
                Style::new().fg(TEXT_WHITE)
            };

            lines.push(Line::from(vec![
                Span::styled(
                    cursor,
                    if is_selected {
                        Style::new().fg(LOGO_MINT)
                    } else {
                        Style::new().fg(TEXT_DIM)
                    },
                ),
                Span::styled(
                    format!("{} ", icon),
                    if is_available {
                        Style::new().fg(color)
                    } else {
                        Style::new().fg(TEXT_DIM)
                    },
                ),
                Span::styled(name, name_style),
            ]));

            // Show preconditions with check/cross marks
            for precondition in &availability.preconditions {
                let (mark, mark_color) = if precondition.satisfied {
                    ("✓", Color::Green)
                } else {
                    ("✗", Color::Red)
                };

                lines.push(Line::from(vec![
                    Span::raw("      "), // indent
                    Span::styled(mark, Style::new().fg(mark_color)),
                    Span::raw(" "),
                    Span::styled(precondition.description, Style::new().fg(TEXT_DIM)),
                ]));
            }

            // Add spacing between agents
            if i < picker.filtered.len() - 1 {
                lines.push(Line::raw(""));
            }
        }

        // Pad to fill available space
        while lines.len() < (popup_height - 4) as usize {
            lines.push(Line::raw(""));
        }

        // Help text at bottom
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("[↑/↓]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" navigate · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" select · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]));
    }

    let block = Block::default()
        .title(" Select Agent ")
        .title_style(Style::new().fg(LOGO_MINT).bold())
        .borders(Borders::ALL)
        .border_style(Style::new().fg(LOGO_MINT))
        .style(Style::new().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);

    // Set cursor position for filter input
    if let Some((x, y)) = cursor_position {
        frame.set_cursor_position(Position::new(x, y));
    }
}
