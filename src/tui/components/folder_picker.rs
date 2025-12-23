//! Folder picker popup component.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the folder picker as a centered popup.
pub fn render_folder_picker(frame: &mut Frame, area: Rect, app: &App) {
    // Calculate centered popup area
    let popup_width = 70u16.min(area.width.saturating_sub(4));
    let popup_height = 20u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = vec![];

    // Track cursor position for filter input
    let mut cursor_position: Option<(u16, u16)> = None;

    if let Some(picker) = &app.folder_picker {
        // Current directory path (truncated if needed)
        let max_path_len = popup_width.saturating_sub(4) as usize;
        let path_str = picker.current_dir.to_string_lossy();
        let display_path = if path_str.len() > max_path_len {
            format!("...{}", &path_str[path_str.len() - max_path_len + 3..])
        } else {
            path_str.to_string()
        };

        lines.push(Line::from(vec![Span::styled(
            display_path,
            Style::new().fg(TEXT_DIM),
        )]));
        lines.push(Line::raw("")); // spacing

        // Filter input line
        lines.push(Line::from(vec![
            Span::styled("Filter: ", Style::new().fg(LOGO_LIGHT_BLUE)),
            Span::styled(&picker.query, Style::new().fg(TEXT_WHITE)),
        ]));

        // Calculate cursor position (after "Filter: " which is 8 chars)
        let cursor_x = popup_area.x + 1 + 8 + picker.query_cursor as u16;
        let cursor_y = popup_area.y + 1 + 2; // +1 for border, +2 for path + empty line
        cursor_position = Some((cursor_x, cursor_y));

        lines.push(Line::raw("")); // spacing

        // Calculate how many entries we can show
        let available_height = popup_height.saturating_sub(7) as usize; // title, path, filter, spacing, help

        // List entries with scrolling
        let total_entries = picker.entries.len();
        let selected = picker.selected;

        // Calculate scroll offset to keep selected item visible
        let scroll_offset = if selected >= available_height {
            selected - available_height + 1
        } else {
            0
        };

        for (i, entry) in picker
            .entries
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(available_height)
        {
            let is_selected = i == selected;
            let cursor = if is_selected { "> " } else { "  " };

            let mut spans = vec![
                Span::styled(
                    cursor,
                    if is_selected {
                        Style::new().fg(LOGO_MINT)
                    } else {
                        Style::new().fg(TEXT_DIM)
                    },
                ),
                Span::styled("📁 ", Style::new().fg(LOGO_GOLD)),
                Span::styled(
                    &entry.name,
                    if is_selected {
                        Style::new().fg(TEXT_WHITE).bold()
                    } else {
                        Style::new().fg(TEXT_WHITE)
                    },
                ),
            ];

            // Show "(current)" indicator for the current directory
            if entry.is_current {
                spans.push(Span::styled(" (current)", Style::new().fg(LOGO_MINT)));
            }

            // Show git branch if available
            if let Some(branch) = &entry.git_branch {
                spans.push(Span::raw("  "));
                spans.push(Span::styled("🌿 ", Style::new().fg(BRANCH_GREEN)));
                spans.push(Span::styled(branch, Style::new().fg(TEXT_DIM)));
            }

            lines.push(Line::from(spans));
        }

        // Show scroll indicator if needed
        if total_entries > available_height {
            let shown_end = (scroll_offset + available_height).min(total_entries);
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "  ({}-{} of {})",
                    scroll_offset + 1,
                    shown_end,
                    total_entries
                ),
                Style::new().fg(TEXT_DIM),
            )]));
        }

        if picker.entries.is_empty() {
            lines.push(Line::styled(
                "  (no matching directories)",
                Style::new().fg(TEXT_DIM),
            ));
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
            Span::styled("[→]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" enter · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[←]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" parent · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" select · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]));
    }

    let block = Block::default()
        .title(" New Session ")
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
