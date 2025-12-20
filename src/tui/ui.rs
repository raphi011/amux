use ratatui::{
    layout::{Constraint, Layout, Rect, Position},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, InputMode};
use crate::session::{SessionState, PermissionMode};
use crate::acp::{PermissionKind, PlanStatus};
use super::theme::*;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Horizontal split: sidebar | left padding | separator | right padding | main content
    let content_layout = Layout::horizontal([
        Constraint::Length(38), // Sidebar
        Constraint::Length(2),  // Left padding (2 spaces)
        Constraint::Length(1),  // Separator
        Constraint::Length(3),  // Right padding (3 spaces)
        Constraint::Min(0),     // Main content
    ])
    .split(area);

    // Sidebar: logo + session list (includes hotkeys and plan at bottom)
    let sidebar_layout = Layout::vertical([
        Constraint::Length(2),  // Logo
        Constraint::Min(0),     // Session list + hotkeys + plan
    ])
    .split(content_layout[0]);

    // Render logo at top of sidebar
    render_logo(frame, sidebar_layout[0]);

    // Render session list with hotkeys and plan at bottom
    render_session_list(frame, sidebar_layout[1], app);

    // Check if there's a pending permission
    let has_permission = app.selected_session()
        .map(|s| s.pending_permission.is_some())
        .unwrap_or(false);

    // Render vertical separator
    render_separator(frame, content_layout[2]);

    // Calculate input bar height based on content wrapping
    let input_area_width = content_layout[4].width.saturating_sub(2) as usize; // Account for prompt "> "
    let input_height = if has_permission {
        0 // No input bar when permission dialog is shown
    } else {
        // Calculate wrapped lines for input buffer only (attachments are on separate line)
        let wrapped_lines = if input_area_width > 0 && !app.input_buffer.is_empty() {
            ((app.input_buffer.len() + input_area_width - 1) / input_area_width).max(1)
        } else {
            1
        };
        // Add 1 for the mode indicator line, plus 1 if there are attachments
        let attachment_line = if app.has_attachments() { 1 } else { 0 };
        (wrapped_lines + 1 + attachment_line) as u16
    };

    // Right side: output + separator + permission/input
    let right_layout = if has_permission {
        Layout::vertical([
            Constraint::Min(0),     // Output
            Constraint::Length(6),  // Permission dialog
        ])
        .split(content_layout[4])
    } else {
        Layout::vertical([
            Constraint::Min(0),                      // Output
            Constraint::Length(1),                   // Empty line above separator
            Constraint::Length(1),                   // Horizontal separator
            Constraint::Length(1),                   // Empty line below separator
            Constraint::Length(input_height.max(2)), // Input bar (min 2 lines: input + mode)
        ])
        .split(content_layout[4])
    };

    // Render folder picker, agent picker, session picker, branch input, worktree picker, or output area
    if app.input_mode == InputMode::FolderPicker || app.input_mode == InputMode::WorktreeFolderPicker {
        render_folder_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::WorktreePicker {
        render_worktree_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::BranchInput {
        render_branch_input(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::AgentPicker {
        render_agent_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::SessionPicker {
        render_session_picker(frame, right_layout[0], app);
    } else {
        render_output_area(frame, right_layout[0], app);
    }

    // Render permission dialog or input bar
    if has_permission {
        render_permission_dialog(frame, right_layout[1], app);
    } else {
        // Render horizontal separator (index 1 is empty, 2 is separator, 3 is empty, 4 is input)
        render_horizontal_separator(frame, right_layout[2]);
        render_input_bar(frame, right_layout[4], app);
    }

    // Render help popup on top if in Help mode
    if app.input_mode == InputMode::Help {
        render_help_popup(frame, area);
    }
}

fn render_separator(frame: &mut Frame, area: Rect) {
    // Draw a vertical line of │ characters
    let separator: Vec<Line> = (0..area.height)
        .map(|_| Line::styled("│", Style::new().fg(TEXT_DIM)))
        .collect();
    let paragraph = Paragraph::new(separator);
    frame.render_widget(paragraph, area);
}

fn render_horizontal_separator(frame: &mut Frame, area: Rect) {
    // Draw a horizontal line of ─ characters
    let separator = "─".repeat(area.width as usize);
    let line = Line::styled(separator, Style::new().fg(TEXT_DIM));
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn render_logo(frame: &mut Frame, area: Rect) {
    // Center the colorful "amux" logo
    let padding = (area.width.saturating_sub(4)) / 2;
    let centered = Line::from(vec![
        Span::raw(" ".repeat(padding as usize)),
        Span::styled("a", Style::new().fg(LOGO_CORAL).bold()),
        Span::styled("m", Style::new().fg(LOGO_GOLD).bold()),
        Span::styled("u", Style::new().fg(LOGO_LIGHT_BLUE).bold()),
        Span::styled("x", Style::new().fg(LOGO_MINT).bold()),
    ]);

    let paragraph = Paragraph::new(centered);
    frame.render_widget(paragraph, area);
}

fn render_session_list(frame: &mut Frame, area: Rect, app: &App) {
    use crate::session::AgentType;

    let mut session_lines: Vec<Line> = vec![];

    for (i, session) in app.sessions.sessions().iter().enumerate() {
        let is_selected = i == app.sessions.selected_index();
        let cursor = if is_selected { "> " } else { "  " };

        // Agent type color for second line
        let agent_color = match session.agent_type {
            AgentType::ClaudeCode => LOGO_CORAL,
            AgentType::GeminiCli => LOGO_LIGHT_BLUE,
        };

        // Activity indicator for working sessions (animated spinner)
        let activity = if session.state.is_active() {
            format!(" {}", app.spinner())
        } else {
            String::new()
        };

        // Compute relative path from start_dir, or use session name as fallback
        let display_path = if let Ok(rel) = session.cwd.strip_prefix(&app.start_dir) {
            if rel.as_os_str().is_empty() {
                ".".to_string()
            } else {
                format!("./{}", rel.display())
            }
        } else {
            // Fallback to just the session name if not under start_dir
            session.name.clone()
        };

        // First line: cursor + number + relative path + activity
        let first_line = Line::from(vec![
            Span::raw(cursor),
            Span::styled(format!("{}. ", i + 1), Style::new().fg(TEXT_DIM)),
            Span::styled(
                display_path,
                if is_selected {
                    Style::new().fg(TEXT_WHITE).bold()
                } else {
                    Style::new().fg(TEXT_WHITE)
                },
            ),
            Span::styled(activity.clone(), Style::new().fg(LOGO_MINT)),
        ]);

        // Second line: agent name + branch + worktree + mode
        let agent_name = session.agent_type.display_name();
        let mut second_spans = vec![
            Span::raw("   "),
            Span::styled(agent_name, Style::new().fg(agent_color)),
            Span::raw("  "),
            Span::styled("🌿 ", Style::new().fg(BRANCH_GREEN)),
            Span::styled(session.git_branch.clone(), Style::new().fg(TEXT_DIM)),
        ];

        // Show worktree indicator
        if session.is_worktree {
            second_spans.push(Span::raw("  "));
            second_spans.push(Span::styled("worktree", Style::new().fg(TEXT_DIM)));
        }

        // Show mode if set (e.g., "plan")
        if let Some(mode) = &session.current_mode {
            second_spans.push(Span::raw("  "));
            second_spans.push(Span::styled(format!("[{}]", mode), Style::new().fg(LOGO_GOLD)));
        }

        let second_line = Line::from(second_spans);

        session_lines.push(first_line);
        session_lines.push(second_line);
        session_lines.push(Line::raw("")); // Spacing
    }

    if session_lines.is_empty() {
        session_lines.push(Line::styled("No sessions", Style::new().fg(TEXT_DIM)));
        session_lines.push(Line::styled("Press [n] to create one", Style::new().fg(TEXT_DIM)));
    }

    // Help hint line at bottom of sidebar
    let hotkey_lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("[?]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" help", Style::new().fg(TEXT_DIM)),
        ]),
    ];

    // Build plan lines for selected session
    let mut plan_lines: Vec<Line> = vec![];
    if let Some(session) = app.selected_session() {
        if !session.plan_entries.is_empty() {
            // Separator and header before plan
            let separator = "─".repeat(area.width.saturating_sub(2) as usize);
            plan_lines.push(Line::styled(separator, Style::new().fg(TEXT_DIM)));
            plan_lines.push(Line::styled("Tasks", Style::new().fg(TEXT_WHITE).bold()));
            plan_lines.push(Line::raw("")); // Empty line after header

            // Plan entries
            for entry in &session.plan_entries {
                let (icon, style) = match entry.status {
                    PlanStatus::Pending => ("○", Style::new().fg(TEXT_DIM)),
                    PlanStatus::InProgress => ("◐", Style::new().fg(LOGO_MINT)),
                    PlanStatus::Completed => ("●", Style::new().fg(TEXT_DIM).add_modifier(Modifier::CROSSED_OUT)),
                    PlanStatus::Unknown => ("?", Style::new().fg(TEXT_DIM)),
                };

                // Wrap content to fit sidebar (icon takes 2 chars)
                let max_width = area.width.saturating_sub(4) as usize;
                let wrapped = wrap_text(&entry.content, max_width);

                for (i, line_text) in wrapped.iter().enumerate() {
                    if i == 0 {
                        // First line: icon + text
                        plan_lines.push(Line::from(vec![
                            Span::styled(format!("{} ", icon), style),
                            Span::styled(line_text.clone(), style),
                        ]));
                    } else {
                        // Continuation lines: indent to align with text
                        plan_lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(line_text.clone(), style),
                        ]));
                    }
                }
            }
        }
    }

    // Calculate padding to bottom-align hotkeys + plan
    let total_height = area.height as usize;
    let session_height = session_lines.len();
    let hotkey_height = hotkey_lines.len();
    let plan_height = plan_lines.len();
    let bottom_height = hotkey_height + plan_height;
    let padding = total_height.saturating_sub(session_height + bottom_height);

    // Combine: sessions + padding + hotkeys + plan
    let mut lines = session_lines;
    for _ in 0..padding {
        lines.push(Line::raw(""));
    }
    lines.extend(hotkey_lines);
    lines.extend(plan_lines);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn render_output_area(frame: &mut Frame, area: Rect, app: &App) {
    use crate::session::OutputType;

    let inner_height = area.height.saturating_sub(0) as usize;
    let inner_width = area.width.saturating_sub(2) as usize; // Account for border

    let lines: Vec<Line> = if let Some(session) = app.selected_session() {
        if session.output.is_empty() {
            let status = match session.state {
                SessionState::Idle => format!("{} is idle.\n\nPress [i] to type a message.", session.name),
                SessionState::Spawning => format!("Starting {}...", session.name),
                SessionState::Initializing => format!("Initializing {}...", session.name),
                SessionState::Prompting => format!("{} is working...", session.name),
                SessionState::AwaitingPermission => format!("{} needs permission.", session.name),
            };
            vec![Line::styled(status, Style::new().fg(TEXT_DIM))]
        } else {
            // Get active tool call ID and spinner for rendering
            let active_tool_id = session.active_tool_call_id.as_deref();
            let spinner = app.spinner();

            // First expand all output to visual lines
            let all_lines: Vec<Line> = session.output
                .iter()
                .flat_map(|output_line| {
                    match &output_line.line_type {
                        OutputType::Text => {
                            // Empty lines for spacing
                            if output_line.content.is_empty() {
                                return vec![Line::raw("")];
                            }
                            // Agent response - render as markdown using ratskin/termimad
                            let skin = ratskin::RatSkin::default();
                            skin.parse(ratskin::RatSkin::parse_text(&output_line.content), inner_width as u16)
                        }
                        OutputType::UserInput => {
                            // User prompt - cyan/blue
                            let wrapped = wrap_text(&output_line.content, inner_width);
                            wrapped.into_iter().map(|text| {
                                Line::from(vec![
                                    Span::styled(text, Style::new().fg(LOGO_LIGHT_BLUE).bold()),
                                ])
                            }).collect()
                        }
                        OutputType::ToolCall { tool_call_id, name, description } => {
                            // Tool call - spinner if active, green dot if complete
                            let is_active = active_tool_id == Some(tool_call_id.as_str());
                            let indicator = if is_active {
                                format!("{} ", spinner)
                            } else {
                                "● ".to_string()
                            };
                            let display = if let Some(desc) = description {
                                format!("{}({})", name, desc)
                            } else {
                                name.clone()
                            };
                            vec![Line::from(vec![
                                Span::styled(indicator, Style::new().fg(TOOL_DOT)),
                                Span::styled(display, Style::new().fg(TEXT_WHITE).bold()),
                            ])]
                        }
                        OutputType::ToolOutput => {
                            // Tool output - └ connector, plain text (no markdown)
                            let wrapped = wrap_text(&output_line.content, inner_width.saturating_sub(2));
                            wrapped.into_iter().enumerate().map(|(i, text)| {
                                let prefix = if i == 0 {
                                    Span::styled("└ ", Style::new().fg(TOOL_CONNECTOR))
                                } else {
                                    Span::styled("  ", Style::new().fg(TOOL_CONNECTOR))
                                };
                                Line::from(vec![
                                    prefix,
                                    Span::styled(text, Style::new().fg(TEXT_DIM)),
                                ])
                            }).collect()
                        }
                        OutputType::DiffAdd => {
                            // Added line - green background
                            let content = &output_line.content;
                            vec![Line::from(vec![
                                Span::styled("  ", Style::new()),
                                Span::styled(
                                    format!("{:width$}", content, width = inner_width.saturating_sub(2)),
                                    Style::new().fg(DIFF_ADD_FG).bg(DIFF_ADD_BG),
                                ),
                            ])]
                        }
                        OutputType::DiffRemove => {
                            // Removed line - red background
                            let content = &output_line.content;
                            vec![Line::from(vec![
                                Span::styled("  ", Style::new()),
                                Span::styled(
                                    format!("{:width$}", content, width = inner_width.saturating_sub(2)),
                                    Style::new().fg(DIFF_REMOVE_FG).bg(DIFF_REMOVE_BG),
                                ),
                            ])]
                        }
                        OutputType::DiffContext => {
                            // Context line - dim
                            vec![Line::from(vec![
                                Span::styled("  ", Style::new()),
                                Span::styled(&output_line.content, Style::new().fg(TEXT_DIM)),
                            ])]
                        }
                        OutputType::DiffHeader => {
                            // Diff header - dim, with └ on first line
                            vec![Line::from(vec![
                                Span::styled("└ ", Style::new().fg(TOOL_CONNECTOR)),
                                Span::styled(&output_line.content, Style::new().fg(TEXT_DIM)),
                            ])]
                        }
                        OutputType::Error => {
                            // Error - red
                            let wrapped = wrap_text(&output_line.content, inner_width.saturating_sub(2));
                            wrapped.into_iter().map(|text| {
                                Line::from(vec![
                                    Span::styled("✗ ", Style::new().fg(LOGO_CORAL)),
                                    Span::styled(text, Style::new().fg(LOGO_CORAL)),
                                ])
                            }).collect()
                        }
                    }
                })
                .collect();

            // Apply scroll offset to visual lines
            // usize::MAX means "scroll to bottom"
            let total_lines = all_lines.len();
            let start = if session.scroll_offset == usize::MAX {
                // Scroll to bottom: show last viewport worth of lines
                total_lines.saturating_sub(inner_height)
            } else {
                session.scroll_offset.min(total_lines.saturating_sub(1))
            };
            let end = (start + inner_height).min(total_lines);
            all_lines[start..end].to_vec()
        }
    } else {
        vec![Line::styled(
            "No session selected.\n\nPress [n] to create a new session.",
            Style::new().fg(TEXT_DIM),
        )]
    };

    let paragraph = Paragraph::new(lines);

    frame.render_widget(paragraph, area);
}

/// Wrap a styled Line to fit within width, preserving span styles
fn wrap_styled_line(line: Line<'_>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![line.into_iter().map(|s| Span::styled(s.content.to_string(), s.style)).collect()];
    }

    let mut result: Vec<Line<'static>> = vec![];
    let mut current_spans: Vec<Span<'static>> = vec![];
    let mut current_width: usize = 0;

    for span in line.spans {
        let style = span.style;
        let content = span.content.to_string();

        for word in content.split_inclusive(' ') {
            let word_width = word.chars().count();

            if current_width + word_width > width && current_width > 0 {
                // Start a new line
                result.push(Line::from(current_spans));
                current_spans = vec![];
                current_width = 0;
            }

            // Handle very long words that need to be split
            if word_width > width {
                let mut remaining = word;
                while !remaining.is_empty() {
                    let take = remaining.chars().take(width - current_width).collect::<String>();
                    let take_len = take.chars().count();
                    current_spans.push(Span::styled(take, style));
                    current_width += take_len;
                    remaining = &remaining[remaining.char_indices().nth(take_len).map(|(i, _)| i).unwrap_or(remaining.len())..];

                    if !remaining.is_empty() {
                        result.push(Line::from(current_spans));
                        current_spans = vec![];
                        current_width = 0;
                    }
                }
            } else {
                current_spans.push(Span::styled(word.to_string(), style));
                current_width += word_width;
            }
        }
    }

    if !current_spans.is_empty() {
        result.push(Line::from(current_spans));
    }

    if result.is_empty() {
        result.push(Line::default());
    }

    result
}

/// Wrap text to fit within width, preserving words where possible
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut result = vec![];

    for line in text.split('\n') {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }

        let mut current_line = String::new();

        for word in line.split(' ') {
            if current_line.is_empty() {
                if word.len() > width {
                    // Word is too long, split it
                    let mut remaining = word;
                    while remaining.len() > width {
                        result.push(remaining[..width].to_string());
                        remaining = &remaining[width..];
                    }
                    current_line = remaining.to_string();
                } else {
                    current_line = word.to_string();
                }
            } else if current_line.len() + 1 + word.len() > width {
                // Line would be too long, start new line
                result.push(current_line);
                if word.len() > width {
                    let mut remaining = word;
                    while remaining.len() > width {
                        result.push(remaining[..width].to_string());
                        remaining = &remaining[width..];
                    }
                    current_line = remaining.to_string();
                } else {
                    current_line = word.to_string();
                }
            } else {
                current_line.push(' ');
                current_line.push_str(word);
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

fn render_input_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_insert = app.input_mode == InputMode::Insert;
    let width = area.width as usize;

    let prompt_style = if is_insert {
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
                    format!("{}...", &name[..17])
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
            spans.push(Span::styled(" (backspace remove · ↓ cancel)", Style::new().fg(TEXT_DIM)));
        }

        lines.push(Line::from(spans));
        attachment_line_count = 1;
    }

    // Always show the prompt
    let prompt = "> ";

    // Wrap the input text
    let content_width = width.saturating_sub(2); // Account for prompt "> "
    let wrapped = wrap_text(&app.input_buffer, content_width);

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

    // Add permission mode indicator line
    let mode_line = if let Some(session) = app.selected_session() {
        let mode = session.permission_mode;
        let (mode_text, mode_color) = match mode {
            PermissionMode::Normal => ("normal", TEXT_DIM),
            PermissionMode::Plan => ("plan", LOGO_GOLD),
            PermissionMode::AcceptAll => ("accept all", LOGO_MINT),
        };
        let mut spans = vec![
            Span::styled("[tab] ", Style::new().fg(TEXT_DIM)),
            Span::styled(mode_text, Style::new().fg(mode_color)),
        ];

        // Add model info if available
        if let Some(model_name) = session.current_model_name() {
            spans.push(Span::styled("  [m] ", Style::new().fg(TEXT_DIM)));
            spans.push(Span::styled(model_name, Style::new().fg(LOGO_LIGHT_BLUE)));
        }

        Line::from(spans)
    } else {
        Line::from(vec![])
    };
    lines.push(mode_line);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);

    // Set cursor position when in insert mode and not selecting attachments
    if is_insert && app.selected_attachment.is_none() {
        // Calculate cursor position with wrapping
        let cursor_line = if content_width > 0 { app.cursor_position / content_width } else { 0 };
        let cursor_col = if content_width > 0 { app.cursor_position % content_width } else { 0 };

        // Add prompt offset for first line
        let x_offset = 2; // prompt width

        frame.set_cursor_position(Position::new(
            area.x + x_offset as u16 + cursor_col as u16,
            area.y + attachment_line_count as u16 + cursor_line as u16,
        ));
    }
}

fn render_folder_picker(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.folder_picker {
        // Header with current directory
        lines.push(Line::from(vec![
            Span::styled("Select folder: ", Style::new().fg(TEXT_DIM)),
            Span::styled(
                picker.current_dir.to_string_lossy().to_string(),
                Style::new().fg(LOGO_LIGHT_BLUE),
            ),
        ]));
        lines.push(Line::raw("")); // spacing

        // List entries
        for (i, entry) in picker.entries.iter().enumerate() {
            let is_selected = i == picker.selected;
            let cursor = if is_selected { "> " } else { "  " };

            let mut spans = vec![
                Span::raw(cursor),
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

            // Show git branch if available
            if let Some(branch) = &entry.git_branch {
                spans.push(Span::raw("  "));
                spans.push(Span::styled("🌿 ", Style::new().fg(BRANCH_GREEN)));
                spans.push(Span::styled(branch, Style::new().fg(TEXT_DIM)));
            }

            lines.push(Line::from(spans));
        }

        if picker.entries.is_empty() || (picker.entries.len() == 1 && picker.entries[0].is_parent) {
            lines.push(Line::styled("  (no subdirectories)", Style::new().fg(TEXT_DIM)));
        }
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

fn render_worktree_picker(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.worktree_picker {
        // Header
        lines.push(Line::from(vec![
            Span::styled("Select worktree or create new", Style::new().fg(TEXT_DIM)),
        ]));
        lines.push(Line::raw("")); // spacing

        // List entries
        for (i, entry) in picker.entries.iter().enumerate() {
            let is_selected = i == picker.selected;
            let cursor = if is_selected { "> " } else { "  " };

            let (icon, name_style) = if entry.is_create_new {
                ("", Style::new().fg(LOGO_MINT))
            } else {
                ("󰙅 ", Style::new().fg(TEXT_WHITE))
            };

            let name_style = if is_selected {
                name_style.bold()
            } else {
                name_style
            };

            lines.push(Line::from(vec![
                Span::raw(cursor),
                Span::styled(icon, Style::new().fg(if entry.is_create_new { LOGO_MINT } else { LOGO_GOLD })),
                Span::styled(&entry.name, name_style),
            ]));
        }

        if picker.entries.len() == 1 {
            lines.push(Line::styled("  (no existing worktrees)", Style::new().fg(TEXT_DIM)));
        }
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

fn render_branch_input(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::layout::Position;

    let mut lines: Vec<Line> = vec![];

    if let Some(branch_state) = &app.branch_input {
        let repo_name = branch_state.repo_path
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

            for (i, branch) in branch_state.filtered.iter().enumerate().skip(start).take(max_display) {
                let is_selected = i == branch_state.selected;
                let cursor = if is_selected { "> " } else { "  " };

                let (icon, color) = if branch.is_remote {
                    ("󰅟 ", TEXT_DIM)  // Remote icon
                } else if branch.is_current {
                    ("󰘬 ", LOGO_MINT)  // Current branch indicator
                } else {
                    (" ", BRANCH_GREEN)  // Regular branch
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

    let paragraph = Paragraph::new(lines)
        .style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);

    // Position cursor in input field
    if let Some(branch_state) = &app.branch_input {
        frame.set_cursor_position(Position::new(
            area.x + 8 + branch_state.cursor_position as u16,  // 8 = "Branch: " length
            area.y + 2,  // Line 2 (after header and blank line)
        ));
    }
}

fn render_agent_picker(frame: &mut Frame, area: Rect, app: &App) {
    use crate::app::AgentPickerState;
    use crate::session::AgentType;

    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.agent_picker {
        // Header with selected directory
        let folder_name = picker.cwd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        lines.push(Line::from(vec![
            Span::styled("Select agent for ", Style::new().fg(TEXT_DIM)),
            Span::styled(folder_name, Style::new().fg(LOGO_LIGHT_BLUE).bold()),
        ]));
        lines.push(Line::raw("")); // spacing

        // List agent options
        for (i, agent_type) in AgentPickerState::agents().iter().enumerate() {
            let is_selected = i == picker.selected;
            let cursor = if is_selected { "> " } else { "  " };

            let (icon, color) = match agent_type {
                AgentType::ClaudeCode => ("", LOGO_CORAL),   // Anthropic orange-ish
                AgentType::GeminiCli => ("", LOGO_LIGHT_BLUE), // Google blue
            };

            let name = agent_type.display_name();

            lines.push(Line::from(vec![
                Span::raw(cursor),
                Span::styled(format!("{} ", icon), Style::new().fg(color)),
                Span::styled(
                    name,
                    if is_selected {
                        Style::new().fg(TEXT_WHITE).bold()
                    } else {
                        Style::new().fg(TEXT_WHITE)
                    },
                ),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

fn render_session_picker(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.session_picker {
        // Header
        lines.push(Line::from(vec![
            Span::styled("Resume session", Style::new().fg(LOGO_LIGHT_BLUE).bold()),
        ]));
        lines.push(Line::raw("")); // spacing

        // List sessions
        for (i, session) in picker.sessions.iter().enumerate() {
            let is_selected = i == picker.selected;
            let cursor = if is_selected { "> " } else { "  " };

            // First line: cursor + folder name + timestamp
            let folder_name = session.cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let timestamp = session.timestamp
                .map(|t| t.format("%b %d %H:%M").to_string())
                .unwrap_or_default();

            let first_spans = vec![
                Span::raw(cursor),
                Span::styled("📁 ", Style::new().fg(LOGO_GOLD)),
                Span::styled(
                    folder_name,
                    if is_selected {
                        Style::new().fg(TEXT_WHITE).bold()
                    } else {
                        Style::new().fg(TEXT_WHITE)
                    },
                ),
                Span::raw("  "),
                Span::styled(timestamp, Style::new().fg(TEXT_DIM)),
            ];
            lines.push(Line::from(first_spans));

            // Second line: first prompt (truncated)
            if let Some(prompt) = &session.first_prompt {
                // Truncate to fit
                let max_len = area.width.saturating_sub(6) as usize;
                let display = if prompt.len() > max_len {
                    format!("{}...", &prompt[..max_len.saturating_sub(3)])
                } else {
                    prompt.clone()
                };

                lines.push(Line::from(vec![
                    Span::raw("     "),
                    Span::styled(display, Style::new().fg(TEXT_DIM).italic()),
                ]));
            }

            lines.push(Line::raw("")); // spacing
        }

        if picker.sessions.is_empty() {
            lines.push(Line::styled("  (no resumable sessions found)", Style::new().fg(TEXT_DIM)));
        }
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

fn render_permission_dialog(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(session) = app.selected_session() {
        if let Some(perm) = &session.pending_permission {
            // Header
            let title = perm.title.clone().unwrap_or_else(|| "Tool".to_string());
            lines.push(Line::from(vec![
                Span::styled("⚠ ", Style::new().fg(LOGO_GOLD)),
                Span::styled("Permission required: ", Style::new().fg(LOGO_GOLD).bold()),
                Span::styled(title, Style::new().fg(TEXT_WHITE)),
            ]));
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
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(LOGO_GOLD));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_help_popup(frame: &mut Frame, area: Rect) {
    // Calculate centered popup area
    let popup_width = 50u16;
    let popup_height = 23u16;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width.min(area.width), popup_height.min(area.height));

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = vec![];

    // Title
    lines.push(Line::from(vec![
        Span::styled("Keyboard Shortcuts", Style::new().fg(TEXT_WHITE).bold()),
    ]));
    lines.push(Line::raw(""));

    // Normal mode
    lines.push(Line::styled("Normal Mode", Style::new().fg(LOGO_LIGHT_BLUE).bold()));
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

    // Insert mode
    lines.push(Line::styled("Insert Mode", Style::new().fg(LOGO_MINT).bold()));
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
