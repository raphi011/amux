//! Worktree cleanup component.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the worktree cleanup dialog.
pub fn render_worktree_cleanup(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(cleanup) = &app.worktree_cleanup {
        let repo_name = cleanup
            .repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Header
        lines.push(Line::from(vec![
            Span::styled("Cleanup worktrees in ", Style::new().fg(TEXT_DIM)),
            Span::styled(repo_name, Style::new().fg(LOGO_LIGHT_BLUE).bold()),
        ]));
        lines.push(Line::raw(""));

        // Status line
        let cleanable = cleanup.cleanable_count();
        let selected_count = cleanup.selected_entries().len();
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} worktrees", cleanup.entries.len()),
                Style::new().fg(TEXT_WHITE),
            ),
            Span::styled(" · ", Style::new().fg(TEXT_DIM)),
            Span::styled(
                format!("{} cleanable", cleanable),
                Style::new().fg(LOGO_MINT),
            ),
            Span::styled(" · ", Style::new().fg(TEXT_DIM)),
            Span::styled(
                format!("{} selected", selected_count),
                Style::new().fg(LOGO_CORAL),
            ),
        ]));
        lines.push(Line::raw(""));

        // List entries
        for (i, entry) in cleanup.entries.iter().enumerate() {
            let is_cursor = i == cleanup.cursor;
            let cursor = if is_cursor { "> " } else { "  " };

            // Show deleting state or normal state
            if entry.is_deleting {
                // Show deleting indicator with spinner
                let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let spinner = spinner_frames[app.spinner_frame % spinner_frames.len()];

                // Branch name or path
                let display_name = entry.branch.as_deref().unwrap_or_else(|| {
                    entry
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                });

                lines.push(Line::from(vec![
                    Span::styled(cursor, Style::new().fg(TEXT_DIM)),
                    Span::styled(spinner, Style::new().fg(LOGO_GOLD)),
                    Span::styled(" deleting ", Style::new().fg(LOGO_GOLD)),
                    Span::styled(display_name, Style::new().fg(TEXT_DIM)),
                ]));
            } else {
                // Checkbox
                let checkbox = if entry.selected { "[x] " } else { "[ ] " };

                // Status icons
                let clean_icon = if entry.is_clean { "󰄬 " } else { "󰅖 " }; // Checkmark or X
                let clean_color = if entry.is_clean {
                    LOGO_MINT
                } else {
                    LOGO_CORAL
                };

                let merged_icon = if entry.is_merged { "󰘬 " } else { "󰜛 " }; // Merged or unmerged
                let merged_color = if entry.is_merged {
                    LOGO_MINT
                } else {
                    LOGO_GOLD
                };

                // Branch name or path
                let display_name = entry.branch.as_deref().unwrap_or_else(|| {
                    entry
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                });

                let name_style = if is_cursor {
                    Style::new().fg(TEXT_WHITE).bold()
                } else if entry.is_clean && entry.is_merged {
                    Style::new().fg(LOGO_MINT)
                } else {
                    Style::new().fg(TEXT_DIM)
                };

                lines.push(Line::from(vec![
                    Span::styled(cursor, Style::new().fg(TEXT_WHITE)),
                    Span::styled(
                        checkbox,
                        if entry.selected {
                            Style::new().fg(LOGO_CORAL)
                        } else {
                            Style::new().fg(TEXT_DIM)
                        },
                    ),
                    Span::styled(clean_icon, Style::new().fg(clean_color)),
                    Span::styled(merged_icon, Style::new().fg(merged_color)),
                    Span::styled(display_name, name_style),
                ]));
            }
        }

        if cleanup.entries.is_empty() {
            lines.push(Line::styled(
                "  (no worktrees found)",
                Style::new().fg(TEXT_DIM),
            ));
        }

        // Options
        lines.push(Line::raw(""));
        let branch_checkbox = if cleanup.delete_branches {
            "[x]"
        } else {
            "[ ]"
        };
        lines.push(Line::from(vec![
            Span::styled("  ", Style::new()),
            Span::styled(
                branch_checkbox,
                if cleanup.delete_branches {
                    Style::new().fg(LOGO_CORAL)
                } else {
                    Style::new().fg(TEXT_DIM)
                },
            ),
            Span::styled(" Delete branches too ", Style::new().fg(TEXT_DIM)),
            Span::styled("[b]", Style::new().fg(TEXT_WHITE)),
        ]));

        // Legend
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::new()),
            Span::styled("󰄬 ", Style::new().fg(LOGO_MINT)),
            Span::styled("clean  ", Style::new().fg(TEXT_DIM)),
            Span::styled("󰅖 ", Style::new().fg(LOGO_CORAL)),
            Span::styled("dirty  ", Style::new().fg(TEXT_DIM)),
            Span::styled("󰘬 ", Style::new().fg(LOGO_MINT)),
            Span::styled("merged  ", Style::new().fg(TEXT_DIM)),
            Span::styled("󰜛 ", Style::new().fg(LOGO_GOLD)),
            Span::styled("unmerged", Style::new().fg(TEXT_DIM)),
        ]));

        // Help text
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("[Space]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" toggle · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[a]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" all · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[n]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" none · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cleanup · ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]));
    }

    let paragraph = Paragraph::new(lines).style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}
