//! Branch input component.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the branch input dialog for creating worktrees.
pub fn render_branch_input(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(branch_state) = &app.branch_input {
        let repo_name = branch_state
            .repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Header
        lines.push(Line::from(vec![
            Span::styled("Create worktree in ", Style::new().fg(TEXT_DIM)),
            Span::styled(repo_name, Style::new().fg(LOGO_LIGHT_BLUE).bold()),
        ]));
        lines.push(Line::raw(""));

        // Branch input line
        lines.push(Line::from(vec![
            Span::styled("Branch: ", Style::new().fg(TEXT_DIM)),
            Span::styled(&branch_state.input, Style::new().fg(TEXT_WHITE)),
        ]));

        // Autocomplete dropdown
        if branch_state.show_autocomplete && !branch_state.filtered.is_empty() {
            lines.push(Line::raw(""));

            let max_display = 8;
            let start = if branch_state.selected >= max_display {
                branch_state.selected - max_display + 1
            } else {
                0
            };

            for (i, branch) in branch_state
                .filtered
                .iter()
                .enumerate()
                .skip(start)
                .take(max_display)
            {
                let is_selected = i == branch_state.selected;
                let cursor = if is_selected { "> " } else { "  " };

                let (icon, color) = if branch.is_remote {
                    ("󰅟 ", TEXT_DIM) // Remote icon
                } else if branch.is_current {
                    ("󰘬 ", LOGO_MINT) // Current branch indicator
                } else {
                    (" ", BRANCH_GREEN) // Regular branch
                };

                let style = if is_selected {
                    Style::new().fg(TEXT_WHITE).bold()
                } else {
                    Style::new().fg(color)
                };

                let mut spans = vec![
                    Span::styled(cursor, style),
                    Span::styled(icon, Style::new().fg(color)),
                    Span::styled(&branch.name, style),
                ];

                if branch.is_current {
                    spans.push(Span::styled(" (current)", Style::new().fg(TEXT_DIM)));
                } else if branch.is_remote {
                    spans.push(Span::styled(" (remote)", Style::new().fg(TEXT_DIM)));
                }

                lines.push(Line::from(spans));
            }
        }

        // Help text
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("[Tab]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" complete · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" create · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]));
    }

    let paragraph = Paragraph::new(lines).style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);

    // Position cursor in input field
    if let Some(branch_state) = &app.branch_input {
        frame.set_cursor_position(Position::new(
            area.x + 8 + branch_state.cursor_position as u16, // 8 = "Branch: " length
            area.y + 2, // Line 2 (after header and blank line)
        ));
    }
}
