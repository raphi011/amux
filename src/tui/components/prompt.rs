//! Prompt component - prompt input with attachments and mode indicators.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, ClickRegion, InputMode};
use crate::events::Action;
use crate::session::PermissionMode;
use crate::tui::theme::*;

use super::wrap_text;

/// Render the prompt with attachments and mode indicators.
pub fn render_prompt(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_insert = app.input_mode == InputMode::Insert;
    let is_bash_mode = app.is_bash_mode();
    let width = area.width as usize;

    // Prompt style: gold for bash mode, green for insert, dim otherwise
    let prompt_style = if is_bash_mode {
        Style::new().fg(LOGO_GOLD)
    } else if is_insert {
        Style::new().fg(LOGO_MINT)
    } else {
        Style::new().fg(TEXT_DIM)
    };

    let input_style = if is_insert {
        Style::new().fg(TEXT_WHITE)
    } else {
        Style::new().fg(TEXT_DIM)
    };

    let mut lines: Vec<Line> = vec![];
    let mut attachment_line_count = 0;

    // Render attachments as a row above input (if any)
    if !app.attachments.is_empty() {
        let mut spans: Vec<Span> = vec![];
        for (i, attachment) in app.attachments.iter().enumerate() {
            let is_selected = app.selected_attachment == Some(i);

            // Format attachment label
            let label = if attachment.filename.is_empty() || attachment.filename == "clipboard" {
                format!("Image #{}", i + 1)
            } else {
                // Truncate long filenames
                let name = &attachment.filename;
                if name.len() > 20 {
                    // Find valid char boundary
                    let mut end = 17;
                    while end > 0 && !name.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &name[..end])
                } else {
                    name.clone()
                }
            };

            let style = if is_selected {
                Style::new().fg(Color::Black).bg(LOGO_GOLD)
            } else {
                Style::new().fg(LOGO_GOLD)
            };

            spans.push(Span::styled(format!("[{}]", label), style));

            if i < app.attachments.len() - 1 {
                spans.push(Span::raw(" "));
            }
        }

        // Add hint when attachment is selected
        if app.selected_attachment.is_some() {
            spans.push(Span::styled(
                " (backspace remove · ↓ cancel)",
                Style::new().fg(TEXT_DIM),
            ));
        }

        lines.push(Line::from(spans));
        attachment_line_count = 1;
    }

    // Show '!' for bash mode, '>' for normal prompt
    let prompt = if is_bash_mode { "! " } else { "> " };

    // Wrap the input text
    let content_width = width.saturating_sub(2); // Account for prompt "> "
    let wrapped = wrap_text(&app.input_buffer, content_width);

    // Calculate how many lines the input takes (for click region calculation)
    let input_line_count = wrapped.len();

    // Build lines with prompt on first line
    for (i, line_text) in wrapped.iter().enumerate() {
        if i == 0 {
            // First line: prompt + content
            lines.push(Line::from(vec![
                Span::styled(prompt, prompt_style),
                Span::styled(line_text.clone(), input_style),
            ]));
        } else {
            // Continuation lines: indent to align with first line content
            lines.push(Line::from(vec![
                Span::raw("  "), // Indent to match prompt width
                Span::styled(line_text.clone(), input_style),
            ]));
        }
    }

    // Add empty line between prompt and mode indicator
    lines.push(Line::raw(""));

    // Track where the mode line starts for click regions (add 1 for the empty line)
    let mode_line_y = area.y + attachment_line_count as u16 + input_line_count as u16 + 1;

    // Calculate permission mode text and model info for click region sizing
    // We need to extract these values before building the mode_line to avoid borrow conflicts
    let (permission_mode_start_x, permission_mode_width, model_start_x, model_name_len) =
        if let Some(session) = app.selected_session() {
            let mode = session.permission_mode;
            let mode_str = match mode {
                PermissionMode::Normal => "normal",
                PermissionMode::Plan => "plan",
                PermissionMode::AcceptAll => "accept all",
                PermissionMode::Yolo => "yolo",
            };
            // Agent name length + "  [tab] " (8 chars)
            let agent_name_len = session.agent_type.display_name().len();
            let perm_start = area.x + agent_name_len as u16 + 2; // Agent name + 2 spaces
            // "[tab] " is 6 chars, then the mode text
            let perm_width = 6 + mode_str.len();
            // Model starts after permission mode + 2 spaces
            let model_x = perm_start + perm_width as u16 + 2;
            let model_len = session.current_model_name().map(|n| n.len());
            (perm_start, perm_width, model_x, model_len)
        } else {
            (area.x, 0, area.x, None)
        };

    // Add permission mode indicator line
    // We need to clone/own the strings to avoid borrowing app during the Line construction
    // Also capture running bash command info before borrowing app
    let running_bash_info = app.running_bash_command.as_ref().map(|cmd| {
        let elapsed = cmd.started_at.elapsed();
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis() / 100; // tenths of a second
        (cmd.command.clone(), format!("{}.{}s", secs, millis))
    });

    let mode_line = if let Some(session) = app.selected_session() {
        let mode = session.permission_mode;
        let (mode_text, mode_color) = match mode {
            PermissionMode::Normal => ("normal", TEXT_DIM),
            PermissionMode::Plan => ("plan", LOGO_GOLD),
            PermissionMode::AcceptAll => ("accept all", LOGO_MINT),
            PermissionMode::Yolo => ("yolo", Color::Red),
        };

        // Agent name color based on type
        let agent_color = match session.agent_type {
            crate::session::AgentType::ClaudeCode => LOGO_CORAL,
            crate::session::AgentType::GeminiCli => LOGO_LIGHT_BLUE,
        };

        let mut spans = vec![
            Span::styled(
                session.agent_type.display_name(),
                Style::new().fg(agent_color),
            ),
            Span::styled("  [tab] ", Style::new().fg(TEXT_DIM)),
            Span::styled(mode_text, Style::new().fg(mode_color)),
        ];

        // Add model info if available - clone the string to own it
        if let Some(model_name) = session.current_model_name() {
            spans.push(Span::styled("  [m] ", Style::new().fg(TEXT_DIM)));
            spans.push(Span::styled(
                model_name.to_string(),
                Style::new().fg(LOGO_LIGHT_BLUE),
            ));
        }

        // Add running bash command timer if present
        if let Some((command, elapsed)) = &running_bash_info {
            // Truncate command if too long
            let max_cmd_len = 30;
            let display_cmd = if command.len() > max_cmd_len {
                format!("{}…", &command[..max_cmd_len - 1])
            } else {
                command.clone()
            };
            spans.push(Span::styled("  ", Style::new()));
            spans.push(Span::styled(
                format!("$ {} ", display_cmd),
                Style::new().fg(LOGO_GOLD),
            ));
            spans.push(Span::styled(elapsed.clone(), Style::new().fg(TEXT_DIM)));
        }

        Line::from(spans)
    } else {
        Line::from(vec![])
    };
    lines.push(mode_line);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);

    // Register interactive regions
    // Input field: covers attachment lines + input lines (not the mode line)
    let input_bounds = ClickRegion::new(
        area.x,
        area.y,
        area.width,
        (attachment_line_count + input_line_count) as u16,
    );
    if app.sessions.selected_session().is_some() {
        app.interactions
            .register_click("input_field", input_bounds, Action::EnterInsertMode);
    }

    // Permission mode toggle: "[tab] <mode>" (starts after agent name)
    let perm_bounds = ClickRegion::new(
        permission_mode_start_x,
        mode_line_y,
        permission_mode_width as u16,
        1,
    );
    app.interactions
        .register_click("permission_mode", perm_bounds, Action::CyclePermissionMode);

    // Model selector: "[m] <model_name>" - only if there's a model
    if let Some(model_len) = model_name_len {
        // "[m] " is 4 chars + model name length
        let model_width = 4 + model_len;
        let model_bounds = ClickRegion::new(model_start_x, mode_line_y, model_width as u16, 1);
        app.interactions
            .register_click("model_selector", model_bounds, Action::CycleModel);
    }

    // Set cursor position when in insert mode and not selecting attachments
    if is_insert && app.selected_attachment.is_none() {
        // Convert byte position to character position for display
        let char_position = app.input_buffer[..app.cursor_position].chars().count();

        // Calculate cursor position by iterating through wrapped lines
        // (simple division doesn't work because word wrap produces variable-length lines)
        let mut cursor_line = 0;
        let mut cursor_col = char_position;
        let mut found = false;
        let mut chars_so_far = 0;

        for (i, line_text) in wrapped.iter().enumerate() {
            let line_chars = line_text.chars().count();
            if chars_so_far + line_chars >= char_position {
                cursor_line = i;
                cursor_col = char_position - chars_so_far;
                found = true;
                break;
            }
            chars_so_far += line_chars;
            // Account for the space/newline that was consumed between lines
            if i < wrapped.len() - 1 {
                chars_so_far += 1; // space between words that caused the wrap
            }
        }

        // If cursor is past all content (at the very end), put it at end of last line
        if !found {
            cursor_line = wrapped.len().saturating_sub(1);
            cursor_col = wrapped.last().map(|l| l.chars().count()).unwrap_or(0);
        }

        // Add prompt offset (both "> " and "  " are 2 chars)
        let x_offset = 2;

        let cursor_x = area.x + x_offset as u16 + cursor_col as u16;
        let cursor_y = area.y + attachment_line_count as u16 + cursor_line as u16;
        crate::log::log(&format!(
            "Cursor render: byte_pos={}, char_pos={}, cursor_col={}, cursor_line={}, x={}, y={}, wrapped={:?}",
            app.cursor_position,
            char_position,
            cursor_col,
            cursor_line,
            cursor_x,
            cursor_y,
            wrapped
        ));
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}
