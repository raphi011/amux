//! Worktree picker popup component.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the worktree picker as a centered popup.
pub fn render_worktree_picker(frame: &mut Frame, area: Rect, app: &App) {
    // Calculate centered popup area
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 18u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.worktree_picker {
        // Header
        lines.push(Line::from(vec![Span::styled(
            "Select worktree or create new",
            Style::new().fg(TEXT_DIM),
        )]));
        lines.push(Line::raw("")); // spacing

        // Count cleanable worktrees
        let cleanable_count = picker
            .entries
            .iter()
            .filter(|e| !e.is_create_new && e.is_clean && e.is_merged)
            .count();

        // Calculate how many entries we can show
        let available_height = popup_height.saturating_sub(9) as usize; // title, header, spacing, help, legend

        // Calculate scroll offset to keep selected item visible
        let total_entries = picker.entries.len();
        let selected = picker.selected;
        let scroll_offset = if selected >= available_height {
            selected - available_height + 1
        } else {
            0
        };

        // List entries with scrolling
        for (i, entry) in picker
            .entries
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(available_height)
        {
            let is_selected = i == selected;
            let cursor = if is_selected { "> " } else { "  " };

            if entry.is_create_new {
                let name_style = if is_selected {
                    Style::new().fg(LOGO_MINT).bold()
                } else {
                    Style::new().fg(LOGO_MINT)
                };
                lines.push(Line::from(vec![
                    Span::raw(cursor),
                    Span::styled(&entry.name, name_style),
                ]));
            } else {
                let name_style = if is_selected {
                    Style::new().fg(TEXT_WHITE).bold()
                } else {
                    Style::new().fg(TEXT_WHITE)
                };

                // Status indicators
                let is_cleanable = entry.is_clean && entry.is_merged;
                let status_icon = if is_cleanable {
                    "󰄬 " // Checkmark - can be cleaned
                } else if !entry.is_clean {
                    "󰅖 " // X - has uncommitted changes
                } else {
                    "󰜛 " // Unmerged - clean but not merged
                };
                let status_color = if is_cleanable {
                    LOGO_MINT
                } else if !entry.is_clean {
                    LOGO_CORAL
                } else {
                    LOGO_GOLD
                };

                lines.push(Line::from(vec![
                    Span::raw(cursor),
                    Span::styled("󰙅 ", Style::new().fg(LOGO_GOLD)),
                    Span::styled(&entry.name, name_style),
                    Span::raw(" "),
                    Span::styled(status_icon, Style::new().fg(status_color)),
                ]));
            }
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

        if picker.entries.len() == 1 {
            lines.push(Line::styled(
                "  (no existing worktrees)",
                Style::new().fg(TEXT_DIM),
            ));
        }

        // Pad to fill available space
        while lines.len() < (popup_height - 6) as usize {
            lines.push(Line::raw(""));
        }

        // Help text
        lines.push(Line::raw(""));
        let mut help_spans = vec![
            Span::styled("[↑/↓]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" navigate · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" select · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ];

        // Show cleanup shortcut only if there are cleanable worktrees
        if cleanable_count > 0 {
            help_spans.push(Span::styled(" · ", Style::new().fg(TEXT_DIM)));
            help_spans.push(Span::styled("[c]", Style::new().fg(TEXT_WHITE)));
            help_spans.push(Span::styled(
                format!(" cleanup ({})", cleanable_count),
                Style::new().fg(TEXT_DIM),
            ));
        }

        lines.push(Line::from(help_spans));

        // Legend
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("󰄬 ", Style::new().fg(LOGO_MINT)),
            Span::styled("cleanable  ", Style::new().fg(TEXT_DIM)),
            Span::styled("󰜛 ", Style::new().fg(LOGO_GOLD)),
            Span::styled("unmerged  ", Style::new().fg(TEXT_DIM)),
            Span::styled("󰅖 ", Style::new().fg(LOGO_CORAL)),
            Span::styled("dirty", Style::new().fg(TEXT_DIM)),
        ]));
    }

    let block = Block::default()
        .title(" Select Worktree ")
        .title_style(Style::new().fg(LOGO_MINT).bold())
        .borders(Borders::ALL)
        .border_style(Style::new().fg(LOGO_MINT))
        .style(Style::new().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}
