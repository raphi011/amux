use ratatui::{
    layout::{Constraint, Layout, Rect, Position},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, InputMode};
use crate::session::SessionState;
use crate::acp::{PermissionKind, PlanStatus};
use super::theme::*;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main vertical layout: logo, content, hotkeys
    let main_layout = Layout::vertical([
        Constraint::Length(2),  // Logo + spacing
        Constraint::Min(0),     // Content
        Constraint::Length(1),  // Hotkeys
    ])
    .split(area);

    // Render centered colorful logo
    render_logo(frame, main_layout[0]);

    // Horizontal split: sidebar | gap | main content
    let content_layout = Layout::horizontal([
        Constraint::Length(38), // Sidebar
        Constraint::Length(1),  // Gap/padding
        Constraint::Min(0),     // Main content
    ])
    .split(main_layout[1]);

    // Render session list in sidebar
    render_session_list(frame, content_layout[0], app);

    // Check if there's a pending permission
    let has_permission = app.selected_session()
        .map(|s| s.pending_permission.is_some())
        .unwrap_or(false);

    // Right side: output + permission/input
    let right_layout = if has_permission {
        Layout::vertical([
            Constraint::Min(0),     // Output
            Constraint::Length(6),  // Permission dialog
        ])
        .split(content_layout[2])
    } else {
        Layout::vertical([
            Constraint::Min(0),     // Output
            Constraint::Length(1),  // Input bar
        ])
        .split(content_layout[2])
    };

    // Render folder picker, agent picker, session picker, or output area
    if app.input_mode == InputMode::FolderPicker {
        render_folder_picker(frame, right_layout[0], app);
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
        render_input_bar(frame, right_layout[1], app);
    }

    // Render hotkey bar
    render_hotkeys(frame, main_layout[2], app);
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

        // Agent type icon
        let (agent_icon, agent_color) = match session.agent_type {
            AgentType::ClaudeCode => ("", LOGO_CORAL),
            AgentType::GeminiCli => ("", LOGO_LIGHT_BLUE),
        };

        // Activity indicator for working sessions (animated spinner)
        let activity = if session.state.is_active() {
            format!(" {}", app.spinner())
        } else {
            String::new()
        };

        // First line: cursor + number + agent icon + name + activity
        let first_line = Line::from(vec![
            Span::raw(cursor),
            Span::styled(format!("{}. ", i + 1), Style::new().fg(TEXT_DIM)),
            Span::styled(format!("{} ", agent_icon), Style::new().fg(agent_color)),
            Span::styled(
                session.name.clone(),
                if is_selected {
                    Style::new().fg(TEXT_WHITE).bold()
                } else {
                    Style::new().fg(TEXT_WHITE)
                },
            ),
            Span::styled(activity.clone(), Style::new().fg(LOGO_MINT)),
        ]);

        // Second line: branch + mode
        let mut second_spans = vec![
            Span::raw("   "),
            Span::styled("🌿 ", Style::new().fg(BRANCH_GREEN)),
            Span::styled(session.git_branch.clone(), Style::new().fg(TEXT_DIM)),
        ];

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

    // Build plan lines for selected session (bottom-aligned)
    let mut plan_lines: Vec<Line> = vec![];
    if let Some(session) = app.selected_session() {
        if !session.plan_entries.is_empty() {
            // Separator
            let separator = "─".repeat(area.width.saturating_sub(2) as usize);
            plan_lines.push(Line::styled(separator, Style::new().fg(TEXT_DIM)));

            // Plan entries
            for entry in &session.plan_entries {
                let (icon, style) = match entry.status {
                    PlanStatus::Pending => ("○", Style::new().fg(TEXT_DIM)),
                    PlanStatus::InProgress => ("◐", Style::new().fg(LOGO_MINT)),
                    PlanStatus::Completed => ("●", Style::new().fg(LOGO_MINT)),
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

    // Calculate padding to bottom-align plan entries
    let total_height = area.height as usize;
    let session_height = session_lines.len();
    let plan_height = plan_lines.len();
    let padding = total_height.saturating_sub(session_height + plan_height);

    // Combine: sessions + padding + plan
    let mut lines = session_lines;
    for _ in 0..padding {
        lines.push(Line::raw(""));
    }
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
            // First expand all output to visual lines
            let all_lines: Vec<Line> = session.output
                .iter()
                .flat_map(|output_line| {
                    match output_line.line_type {
                        OutputType::Text => {
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
                        OutputType::ToolCall => {
                            // Tool call - gold with icon
                            let wrapped = wrap_text(&output_line.content, inner_width.saturating_sub(2));
                            wrapped.into_iter().enumerate().map(|(i, text)| {
                                if i == 0 {
                                    Line::from(vec![
                                        Span::styled("⚙ ", Style::new().fg(LOGO_GOLD)),
                                        Span::styled(text, Style::new().fg(LOGO_GOLD)),
                                    ])
                                } else {
                                    Line::from(vec![
                                        Span::styled("  ", Style::new().fg(TEXT_DIM)),
                                        Span::styled(text, Style::new().fg(LOGO_GOLD)),
                                    ])
                                }
                            }).collect()
                        }
                        OutputType::ToolResult => {
                            // Tool result - dim with indent
                            let wrapped = wrap_text(&output_line.content, inner_width.saturating_sub(2));
                            wrapped.into_iter().map(|text| {
                                Line::from(vec![
                                    Span::styled("  ", Style::new().fg(TEXT_DIM)),
                                    Span::styled(text, Style::new().fg(TEXT_DIM)),
                                ])
                            }).collect()
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

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::new().fg(TEXT_DIM));

    let paragraph = Paragraph::new(lines).block(block);

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

    let prompt = if is_insert { "> " } else { "  " };

    let input_line = Line::from(vec![
        Span::styled(prompt, prompt_style),
        Span::styled(&app.input_buffer, input_style),
    ]);

    let paragraph = Paragraph::new(input_line)
        .block(Block::default().borders(Borders::LEFT).border_style(Style::new().fg(TEXT_DIM)));

    frame.render_widget(paragraph, area);

    // Set cursor position when in insert mode
    if is_insert {
        // +2 for border and prompt "> "
        frame.set_cursor_position(Position::new(
            area.x + 1 + 2 + app.cursor_position as u16,
            area.y,
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

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::new().fg(TEXT_DIM));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
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

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::new().fg(TEXT_DIM));

    let paragraph = Paragraph::new(lines)
        .block(block)
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

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::new().fg(TEXT_DIM));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

fn render_hotkeys(frame: &mut Frame, area: Rect, app: &App) {
    let hotkeys = match app.input_mode {
        InputMode::Normal => Line::from(vec![
            Span::styled("[i]", Style::new().fg(TEXT_WHITE)),
            Span::styled("nput • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[n]", Style::new().fg(TEXT_WHITE)),
            Span::styled("ew • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[x]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" kill • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[C-u/d]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" scroll • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[q]", Style::new().fg(TEXT_WHITE)),
            Span::styled("uit", Style::new().fg(TEXT_DIM)),
        ]),
        InputMode::Insert => Line::from(vec![
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" send • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]),
        InputMode::FolderPicker => Line::from(vec![
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" select • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[l/→]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" enter • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[h/←]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" back • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]),
        InputMode::AgentPicker => Line::from(vec![
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" select • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[j/k]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" navigate • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]),
        InputMode::SessionPicker => Line::from(vec![
            Span::styled("[Enter]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" resume • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[j/k]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" navigate • ", Style::new().fg(TEXT_DIM)),
            Span::styled("[Esc]", Style::new().fg(TEXT_WHITE)),
            Span::styled(" cancel", Style::new().fg(TEXT_DIM)),
        ]),
    };

    let paragraph = Paragraph::new(hotkeys);
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
        .borders(Borders::LEFT | Borders::TOP)
        .border_style(Style::new().fg(LOGO_GOLD));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
