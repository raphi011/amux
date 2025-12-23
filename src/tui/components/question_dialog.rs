//! Question dialog component.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::tui::theme::*;

/// Render the question dialog for agent questions.
pub fn render_question_dialog(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(session) = app.selected_session()
        && let Some(question) = &session.pending_question
    {
        // Header with question
        lines.push(Line::from(vec![
            Span::styled("? ", Style::new().fg(Color::Cyan)),
            Span::styled(&question.question, Style::new().fg(TEXT_WHITE).bold()),
        ]));
        lines.push(Line::raw(""));

        // Options if present
        if !question.options.is_empty() {
            for (i, option) in question.options.iter().enumerate() {
                let is_selected = i == question.selected;
                let cursor = if is_selected { "> " } else { "  " };

                let style = if is_selected {
                    Style::new().fg(TEXT_WHITE).bold()
                } else {
                    Style::new().fg(TEXT_DIM)
                };

                lines.push(Line::from(vec![
                    Span::styled(cursor, style),
                    Span::styled(&option.label, style),
                ]));
            }
            lines.push(Line::raw(""));
        }

        // Input field
        let input_prefix = "> ";
        let cursor_pos = question.cursor_position;
        let input = &question.input;

        // Show cursor in input
        let before_cursor = &input[..cursor_pos];
        let at_cursor = if cursor_pos < input.len() {
            &input[cursor_pos..cursor_pos + 1]
        } else {
            " "
        };
        let after_cursor = if cursor_pos < input.len() {
            &input[cursor_pos + 1..]
        } else {
            ""
        };

        lines.push(Line::from(vec![
            Span::styled(input_prefix, Style::new().fg(Color::Cyan)),
            Span::styled(before_cursor, Style::new().fg(TEXT_WHITE)),
            Span::styled(at_cursor, Style::new().fg(Color::Black).bg(TEXT_WHITE)),
            Span::styled(after_cursor, Style::new().fg(TEXT_WHITE)),
        ]));

        // Help text
        lines.push(Line::raw(""));
        if question.options.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
                Span::styled(" submit • ", Style::new().fg(TEXT_DIM)),
                Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
                Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("[↑/↓]", Style::new().fg(TEXT_WHITE)),
                Span::styled(" select • ", Style::new().fg(TEXT_DIM)),
                Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
                Span::styled(" submit • ", Style::new().fg(TEXT_DIM)),
                Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
                Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
            ]));
        }
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
