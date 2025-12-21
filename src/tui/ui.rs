use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use std::collections::BTreeMap;

use super::theme::*;
use crate::acp::{PermissionKind, PlanStatus};
use crate::app::{App, InputMode, SortMode};
use crate::picker::Picker;
use crate::session::{PermissionMode, Session, SessionState};

// Layout constants
const SIDEBAR_WIDTH: u16 = 40;
const SIDEBAR_LEFT_PADDING: u16 = 2;
const SEPARATOR_WIDTH: u16 = 1;
const CONTENT_LEFT_PADDING: u16 = 1;
const CONTENT_RIGHT_PADDING: u16 = 1;
const SIDEBAR_INNER_PADDING: u16 = 1;
const BORDER_WIDTH: u16 = 2;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Horizontal split: sidebar | left padding | separator | content left padding | main content | content right padding
    let content_layout = Layout::horizontal([
        Constraint::Length(SIDEBAR_WIDTH),
        Constraint::Length(SIDEBAR_LEFT_PADDING),
        Constraint::Length(SEPARATOR_WIDTH),
        Constraint::Length(CONTENT_LEFT_PADDING),
        Constraint::Min(0), // Main content
        Constraint::Length(CONTENT_RIGHT_PADDING),
    ])
    .split(area);

    // Sidebar with 1-char padding on left/right, no top padding
    let sidebar_outer = content_layout[0];
    let sidebar_inner = Rect {
        x: sidebar_outer.x + SIDEBAR_INNER_PADDING,
        y: sidebar_outer.y,
        width: sidebar_outer.width.saturating_sub(BORDER_WIDTH),
        height: sidebar_outer.height,
    };

    // Sidebar: logo + session list (includes hotkeys and plan at bottom)
    let sidebar_layout = Layout::vertical([
        Constraint::Length(1), // Logo (single line)
        Constraint::Min(0),    // Session list + hotkeys + plan
    ])
    .split(sidebar_inner);

    // Render logo at top of sidebar
    render_logo(frame, sidebar_layout[0]);

    // Render session list with hotkeys and plan at bottom
    render_session_list(frame, sidebar_layout[1], app);

    // Check if there's a pending permission or question
    let has_permission = app
        .selected_session()
        .map(|s| s.pending_permission.is_some())
        .unwrap_or(false);

    let has_question = app
        .selected_session()
        .map(|s| s.pending_question.is_some())
        .unwrap_or(false);

    // Render vertical separator
    render_separator(frame, content_layout[2]);

    // Calculate input bar height based on content wrapping
    let input_area_width = content_layout[4].width.saturating_sub(2) as usize; // Account for prompt "> "
    let input_height = if has_permission || has_question {
        0 // No input bar when permission/question dialog is shown
    } else {
        // Calculate wrapped lines for input buffer only (attachments are on separate line)
        let wrapped_lines = if input_area_width > 0 && !app.input_buffer.is_empty() {
            app.input_buffer.len().div_ceil(input_area_width).max(1)
        } else {
            1
        };
        // Add 1 for the mode indicator line, 1 for padding between prompt and mode, plus 1 if there are attachments
        let attachment_line = if app.has_attachments() { 1 } else { 0 };
        (wrapped_lines + 2 + attachment_line) as u16
    };

    // Calculate question dialog height
    let question_height = if has_question {
        if let Some(session) = app.selected_session() {
            if let Some(q) = &session.pending_question {
                // 2 for question + blank, options count, 2 for input, 1 for help
                let options_height = if q.options.is_empty() {
                    0
                } else {
                    q.options.len() as u16 + 1
                };
                5 + options_height
            } else {
                6
            }
        } else {
            6
        }
    } else {
        6
    };

    // Right side: output + separator + permission/question/input
    let right_layout = if has_permission {
        Layout::vertical([
            Constraint::Min(0),    // Output
            Constraint::Length(6), // Permission dialog
        ])
        .split(content_layout[4])
    } else if has_question {
        Layout::vertical([
            Constraint::Min(0),                  // Output
            Constraint::Length(question_height), // Question dialog
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
    if app.input_mode == InputMode::FolderPicker
        || app.input_mode == InputMode::WorktreeFolderPicker
        || app.input_mode == InputMode::WorktreeCleanupRepoPicker
    {
        render_folder_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::WorktreePicker {
        render_worktree_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::BranchInput {
        render_branch_input(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::AgentPicker {
        render_agent_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::SessionPicker {
        render_session_picker(frame, right_layout[0], app);
    } else if app.input_mode == InputMode::WorktreeCleanup {
        render_worktree_cleanup(frame, right_layout[0], app);
    } else {
        render_output_area(frame, right_layout[0], app);
    }

    // Render permission dialog, question dialog, or input bar
    if has_permission {
        render_permission_dialog(frame, right_layout[1], app);
    } else if has_question {
        render_question_dialog(frame, right_layout[1], app);
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

/// Render a single session entry and return the lines
fn render_session_entry<'a>(
    session: &'a Session,
    index: usize,
    is_selected: bool,
    spinner: &str,
    start_dir: &std::path::Path,
    show_number: bool,
) -> Vec<Line<'a>> {
    use crate::session::AgentType;

    let cursor = if is_selected { "> " } else { "  " };

    // Agent type color for second line
    let agent_color = match session.agent_type {
        AgentType::ClaudeCode => LOGO_CORAL,
        AgentType::GeminiCli => LOGO_LIGHT_BLUE,
    };

    // Activity indicator for working sessions
    let activity = if session.pending_permission.is_some() {
        " ⚠".to_string() // Permission required
    } else if session.pending_question.is_some() {
        " ?".to_string() // Question pending
    } else if session.state.is_active() {
        format!(" {}", spinner) // Animated spinner
    } else {
        String::new()
    };

    // Compute relative path from start_dir, or use session name as fallback
    let display_path = if let Ok(rel) = session.cwd.strip_prefix(start_dir) {
        if rel.as_os_str().is_empty() {
            ".".to_string()
        } else {
            format!("./{}", rel.display())
        }
    } else {
        // Fallback to just the session name if not under start_dir
        session.name.clone()
    };

    // First line: cursor + optional number + relative path + activity
    let first_line = if show_number {
        Line::from(vec![
            Span::raw(cursor),
            Span::styled(format!("{}. ", index + 1), Style::new().fg(TEXT_DIM)),
            Span::styled(
                display_path,
                if is_selected {
                    Style::new().fg(TEXT_WHITE).bold()
                } else {
                    Style::new().fg(TEXT_WHITE)
                },
            ),
            Span::styled(activity.clone(), Style::new().fg(LOGO_MINT)),
        ])
    } else {
        Line::from(vec![
            Span::raw(cursor),
            Span::styled(
                display_path,
                if is_selected {
                    Style::new().fg(TEXT_WHITE).bold()
                } else {
                    Style::new().fg(TEXT_WHITE)
                },
            ),
            Span::styled(activity.clone(), Style::new().fg(LOGO_MINT)),
        ])
    };

    // Second line: agent name + branch + worktree + mode
    let agent_name = session.agent_type.display_name();
    let mut second_spans = vec![
        Span::raw("   "),
        Span::styled(agent_name.to_string(), Style::new().fg(agent_color)),
        Span::raw("  "),
        Span::styled("🌿 ", Style::new().fg(BRANCH_GREEN)),
        Span::styled(session.git_branch.clone(), Style::new().fg(TEXT_DIM)),
    ];

    // Show worktree indicator (compact)
    if session.is_worktree {
        second_spans.push(Span::styled(" (wt)", Style::new().fg(TEXT_DIM)));
    }

    // Show mode if set (e.g., "plan")
    if let Some(mode) = &session.current_mode {
        second_spans.push(Span::raw("  "));
        second_spans.push(Span::styled(
            format!("[{}]", mode),
            Style::new().fg(LOGO_GOLD),
        ));
    }

    let second_line = Line::from(second_spans);

    vec![first_line, second_line, Line::raw("")] // Include spacing
}

/// Extract a display name from a git origin URL
fn origin_display_name(origin: &str) -> String {
    // origin is already normalized (e.g., "github.com/user/repo")
    // Extract just the repo name (last component)
    origin
        .rsplit('/')
        .next()
        .unwrap_or(origin)
        .to_string()
}

pub fn render_session_list(frame: &mut Frame, area: Rect, app: &mut App) {
    use crate::app::ClickRegion;

    // Start with empty line for padding after logo
    let mut session_lines: Vec<Line> = vec![Line::raw("")];
    let mut session_click_areas: Vec<(usize, ClickRegion)> = vec![];

    let spinner = app.spinner();
    let start_dir = app.start_dir.clone();
    let selected_index = app.sessions.selected_index();

    // Build a sorted list of (original_index, session) pairs based on sort mode
    let sessions = app.sessions.sessions();
    let mut sorted_indices: Vec<usize> = (0..sessions.len()).collect();

    match app.sort_mode {
        SortMode::List => {
            // Keep original order (no sorting needed)
        }
        SortMode::Grouped => {
            // Sort by git origin/folder name for grouping
            sorted_indices.sort_by(|&a, &b| {
                let key_a = sessions[a].git_origin.clone().unwrap_or_else(|| {
                    sessions[a]
                        .cwd
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });
                let key_b = sessions[b].git_origin.clone().unwrap_or_else(|| {
                    sessions[b]
                        .cwd
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });
                key_a.cmp(&key_b)
            });
        }
        SortMode::ByName => {
            // Sort alphabetically by session name
            sorted_indices.sort_by(|&a, &b| sessions[a].name.cmp(&sessions[b].name));
        }
        SortMode::ByCreatedTime => {
            // Sort by creation time (oldest first)
            sorted_indices.sort_by(|&a, &b| sessions[a].created_at.cmp(&sessions[b].created_at));
        }
        SortMode::Priority => {
            // Priority: permission prompts first, questions next, idle next, running last
            sorted_indices.sort_by(|&a, &b| {
                let priority = |s: &Session| -> u8 {
                    if s.pending_permission.is_some() {
                        0 // Highest priority
                    } else if s.pending_question.is_some() {
                        1
                    } else if s.state == SessionState::Idle {
                        2
                    } else {
                        3 // Running sessions last
                    }
                };
                priority(&sessions[a]).cmp(&priority(&sessions[b]))
            });
        }
    }

    // For grouped mode, render with group headers
    if app.sort_mode == SortMode::Grouped {
        // Group sessions by git origin, falling back to folder name
        let mut groups: BTreeMap<String, Vec<(usize, usize, &Session)>> = BTreeMap::new();

        for (display_idx, &original_idx) in sorted_indices.iter().enumerate() {
            let session = &sessions[original_idx];
            let key = session.git_origin.clone().unwrap_or_else(|| {
                session
                    .cwd
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });
            groups
                .entry(key)
                .or_default()
                .push((display_idx, original_idx, session));
        }

        for (origin, group_sessions) in &groups {
            // Group header - extract display name from origin or use folder name directly
            let display_name = origin_display_name(origin);

            session_lines.push(Line::from(vec![
                Span::styled("● ", Style::new().fg(LOGO_GOLD)),
                Span::styled(display_name, Style::new().fg(TEXT_WHITE).bold()),
                Span::styled(
                    format!(" ({})", group_sessions.len()),
                    Style::new().fg(TEXT_DIM),
                ),
            ]));

            // Sessions in this group
            for &(display_idx, original_idx, session) in group_sessions {
                let is_selected = original_idx == selected_index;
                let line_y = area.y + session_lines.len() as u16;

                // Use display_idx for the number shown to user
                let entry_lines =
                    render_session_entry(session, display_idx, is_selected, spinner, &start_dir, true);

                // Track click region with original_idx for selection
                session_click_areas.push((original_idx, ClickRegion::new(area.x, line_y, area.width, 3)));

                session_lines.extend(entry_lines);
            }
        }
    } else {
        // Non-grouped modes: render flat list with sorted order
        for (display_idx, &original_idx) in sorted_indices.iter().enumerate() {
            let session = &sessions[original_idx];
            let is_selected = original_idx == selected_index;
            let line_y = area.y + session_lines.len() as u16;

            // Use display_idx for the number shown to user
            let entry_lines =
                render_session_entry(session, display_idx, is_selected, spinner, &start_dir, true);

            // Track click region with original_idx for selection
            session_click_areas.push((original_idx, ClickRegion::new(area.x, line_y, area.width, 3)));

            session_lines.extend(entry_lines);
        }
    }

    // Update click areas for sessions
    app.click_areas.session_items = session_click_areas;

    if session_lines.is_empty() {
        session_lines.push(Line::styled("No sessions", Style::new().fg(TEXT_DIM)));
        session_lines.push(Line::styled(
            "Press [n] to create one",
            Style::new().fg(TEXT_DIM),
        ));
    }

    // Help hint line at bottom of sidebar with sort mode indicator
    let sort_mode_name = app.sort_mode.display_name();
    let hotkey_lines: Vec<Line> = vec![Line::from(vec![
        Span::styled("[?]", Style::new().fg(TEXT_WHITE)),
        Span::styled(" help  ", Style::new().fg(TEXT_DIM)),
        Span::styled("[v]", Style::new().fg(TEXT_WHITE)),
        Span::styled(" ", Style::new().fg(TEXT_DIM)),
        Span::styled(sort_mode_name, Style::new().fg(LOGO_LIGHT_BLUE)),
    ])];

    // Build plan lines for selected session
    let mut plan_lines: Vec<Line> = vec![];
    if let Some(session) = app.selected_session()
        && !session.plan_entries.is_empty()
    {
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
                PlanStatus::Completed => (
                    "●",
                    Style::new()
                        .fg(TEXT_DIM)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
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

pub fn render_output_area(frame: &mut Frame, area: Rect, app: &mut App) {
    use crate::session::OutputType;

    let inner_height = area.height as usize;
    let inner_width = area.width.saturating_sub(2) as usize; // Account for border

    // Track total rendered lines to update session afterwards
    let mut computed_total_lines: Option<usize> = None;

    let lines: Vec<Line> = if let Some(session) = app.selected_session() {
        if session.output.is_empty() {
            let status = match session.state {
                SessionState::Idle => {
                    format!("{} is idle.\n\nPress [i] to type a message.", session.name)
                }
                SessionState::Spawning => format!("Starting {}...", session.name),
                SessionState::Initializing => format!("Initializing {}...", session.name),
                SessionState::Prompting => format!("{} is working...", session.name),
                SessionState::AwaitingPermission => format!("{} needs permission.", session.name),
                SessionState::AwaitingUserInput => {
                    format!("{} is asking a question.", session.name)
                }
            };
            vec![Line::styled(status, Style::new().fg(TEXT_DIM))]
        } else {
            // Get active tool call ID and spinner for rendering
            let active_tool_id = session.active_tool_call_id.as_deref();
            let spinner = app.spinner();

            // First expand all output to visual lines
            let all_lines: Vec<Line> = session
                .output
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
                            skin.parse(
                                ratskin::RatSkin::parse_text(&output_line.content),
                                inner_width as u16,
                            )
                        }
                        OutputType::UserInput => {
                            // User prompt - cyan/blue
                            let wrapped = wrap_text(&output_line.content, inner_width);
                            wrapped
                                .into_iter()
                                .map(|text| {
                                    Line::from(vec![Span::styled(
                                        text,
                                        Style::new().fg(LOGO_LIGHT_BLUE).bold(),
                                    )])
                                })
                                .collect()
                        }
                        OutputType::ToolCall {
                            tool_call_id,
                            name,
                            description,
                            failed,
                        } => {
                            // Tool call - spinner if active, red dot if failed, green dot if complete
                            let is_active = active_tool_id == Some(tool_call_id.as_str());
                            let (indicator, indicator_color) = if is_active {
                                (format!("{} ", spinner), TOOL_DOT)
                            } else if *failed {
                                ("● ".to_string(), LOGO_CORAL)
                            } else {
                                ("● ".to_string(), TOOL_DOT)
                            };
                            // Filter out any "undefined" that might have slipped through
                            let clean_desc = description.as_ref().filter(|d| {
                                let trimmed = d.trim();
                                !trimmed.is_empty() && trimmed != "undefined" && trimmed != "null"
                            });
                            // Always use Name(...) format, even if description is empty
                            let display = match clean_desc {
                                Some(desc) => format!("{}({})", name, desc),
                                None => format!("{}()", name),
                            };
                            // Wrap tool call display for narrow windows (indicator is 2 chars)
                            let wrapped = wrap_text(&display, inner_width.saturating_sub(2));
                            wrapped
                                .into_iter()
                                .enumerate()
                                .map(|(i, text)| {
                                    let prefix = if i == 0 {
                                        Span::styled(
                                            indicator.clone(),
                                            Style::new().fg(indicator_color),
                                        )
                                    } else {
                                        Span::styled("  ", Style::new().fg(indicator_color))
                                    };
                                    Line::from(vec![
                                        prefix,
                                        Span::styled(text, Style::new().fg(TEXT_WHITE).bold()),
                                    ])
                                })
                                .collect()
                        }
                        OutputType::ToolOutput => {
                            // Tool output - └ connector, plain text (no markdown)
                            let wrapped =
                                wrap_text(&output_line.content, inner_width.saturating_sub(2));
                            wrapped
                                .into_iter()
                                .enumerate()
                                .map(|(i, text)| {
                                    let prefix = if i == 0 {
                                        Span::styled("└ ", Style::new().fg(TOOL_CONNECTOR))
                                    } else {
                                        Span::styled("  ", Style::new().fg(TOOL_CONNECTOR))
                                    };
                                    Line::from(vec![
                                        prefix,
                                        Span::styled(text, Style::new().fg(TEXT_DIM)),
                                    ])
                                })
                                .collect()
                        }
                        OutputType::DiffAdd => {
                            // Added line - green background, no padding
                            vec![Line::from(vec![
                                Span::styled("  ", Style::new()),
                                Span::styled(
                                    &output_line.content,
                                    Style::new().fg(DIFF_ADD_FG).bg(DIFF_ADD_BG),
                                ),
                            ])]
                        }
                        OutputType::DiffRemove => {
                            // Removed line - red background, no padding
                            vec![Line::from(vec![
                                Span::styled("  ", Style::new()),
                                Span::styled(
                                    &output_line.content,
                                    Style::new().fg(DIFF_REMOVE_FG).bg(DIFF_REMOVE_BG),
                                ),
                            ])]
                        }
                        OutputType::DiffContext => {
                            // Context line - dim
                            let content = &output_line.content;
                            vec![Line::from(vec![
                                Span::styled("  ", Style::new()),
                                Span::styled(
                                    format!(
                                        "{:width$}",
                                        content,
                                        width = inner_width.saturating_sub(2)
                                    ),
                                    Style::new().fg(TEXT_DIM),
                                ),
                            ])]
                        }
                        OutputType::DiffHeader => {
                            // Diff header - dim, indented to align with diff content
                            let content = &output_line.content;
                            vec![Line::from(vec![
                                Span::styled("  ", Style::new()),
                                Span::styled(
                                    format!(
                                        "{:width$}",
                                        content,
                                        width = inner_width.saturating_sub(2)
                                    ),
                                    Style::new().fg(TEXT_DIM),
                                ),
                            ])]
                        }
                        OutputType::Error => {
                            // Error - red
                            let wrapped =
                                wrap_text(&output_line.content, inner_width.saturating_sub(2));
                            wrapped
                                .into_iter()
                                .map(|text| {
                                    Line::from(vec![
                                        Span::styled("✗ ", Style::new().fg(LOGO_CORAL)),
                                        Span::styled(text, Style::new().fg(LOGO_CORAL)),
                                    ])
                                })
                                .collect()
                        }
                    }
                })
                .collect();

            // Apply scroll offset to visual lines
            // usize::MAX means "scroll to bottom"
            let total_lines = all_lines.len();
            computed_total_lines = Some(total_lines);
            let scroll_offset = session.scroll_offset;
            let start = if scroll_offset == usize::MAX {
                // Scroll to bottom: show last viewport worth of lines
                total_lines.saturating_sub(inner_height)
            } else {
                scroll_offset.min(total_lines.saturating_sub(1))
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

    // Update total_rendered_lines for accurate scroll calculations
    if let Some(total_lines) = computed_total_lines
        && let Some(session) = app.sessions.selected_session_mut()
    {
        session.total_rendered_lines = total_lines;
    }
}
/// Wrap text to fit within width, preserving words where possible.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
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

        // Helper to split at char boundary
        fn split_at_char_boundary(s: &str, max_bytes: usize) -> (&str, &str) {
            if s.len() <= max_bytes {
                return (s, "");
            }
            let mut end = max_bytes;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            (&s[..end], &s[end..])
        }

        for word in line.split(' ') {
            if current_line.is_empty() {
                if word.len() > width {
                    // Word is too long, split it at char boundaries
                    let mut remaining = word;
                    while remaining.len() > width {
                        let (chunk, rest) = split_at_char_boundary(remaining, width);
                        result.push(chunk.to_string());
                        remaining = rest;
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
                        let (chunk, rest) = split_at_char_boundary(remaining, width);
                        result.push(chunk.to_string());
                        remaining = rest;
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

pub fn render_input_bar(frame: &mut Frame, area: Rect, app: &mut App) {
    use crate::app::ClickRegion;

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

    // Always show the prompt
    let prompt = "> ";

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
    let (permission_mode_width, model_start_x, model_name_len) =
        if let Some(session) = app.selected_session() {
            let mode = session.permission_mode;
            let mode_str = match mode {
                PermissionMode::Normal => "normal",
                PermissionMode::Plan => "plan",
                PermissionMode::AcceptAll => "accept all",
            };
            // "[tab] " is 6 chars, then the mode text
            let perm_width = 6 + mode_str.len();
            // Model starts after permission mode + 2 spaces
            let model_x = area.x + perm_width as u16 + 2;
            let model_len = session.current_model_name().map(|n| n.len());
            (perm_width, model_x, model_len)
        } else {
            (0, area.x, None)
        };

    // Add permission mode indicator line
    // We need to clone/own the strings to avoid borrowing app during the Line construction
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

        // Add model info if available - clone the string to own it
        if let Some(model_name) = session.current_model_name() {
            spans.push(Span::styled("  [m] ", Style::new().fg(TEXT_DIM)));
            spans.push(Span::styled(
                model_name.to_string(),
                Style::new().fg(LOGO_LIGHT_BLUE),
            ));
        }

        Line::from(spans)
    } else {
        Line::from(vec![])
    };
    lines.push(mode_line);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);

    // Update click areas (after rendering, so no borrow conflicts)
    // Input field: covers attachment lines + input lines (not the mode line)
    app.click_areas.input_field = ClickRegion::new(
        area.x,
        area.y,
        area.width,
        (attachment_line_count + input_line_count) as u16,
    );

    // Permission mode toggle: "[tab] <mode>"
    app.click_areas.permission_mode =
        ClickRegion::new(area.x, mode_line_y, permission_mode_width as u16, 1);

    // Model selector: "[m] <model_name>" - only if there's a model
    if let Some(model_len) = model_name_len {
        // "[m] " is 4 chars + model name length
        let model_width = 4 + model_len;
        app.click_areas.model_selector =
            ClickRegion::new(model_start_x, mode_line_y, model_width as u16, 1);
    } else {
        app.click_areas.model_selector = ClickRegion::default();
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

pub fn render_folder_picker(frame: &mut Frame, area: Rect, app: &App) {
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
            lines.push(Line::styled(
                "  (no subdirectories)",
                Style::new().fg(TEXT_DIM),
            ));
        }
    }

    let paragraph = Paragraph::new(lines).style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

pub fn render_worktree_picker(frame: &mut Frame, area: Rect, app: &App) {
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

        // List entries
        for (i, entry) in picker.entries.iter().enumerate() {
            let is_selected = i == picker.selected;
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

        if picker.entries.len() == 1 {
            lines.push(Line::styled(
                "  (no existing worktrees)",
                Style::new().fg(TEXT_DIM),
            ));
        }

        // Help text
        lines.push(Line::raw(""));
        let mut help_spans = vec![
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

    let paragraph = Paragraph::new(lines).style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

pub fn render_branch_input(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::layout::Position;

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

pub fn render_agent_picker(frame: &mut Frame, area: Rect, app: &App) {
    use crate::app::AgentPickerState;
    use crate::session::AgentType;

    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.agent_picker {
        // Header with selected directory
        let folder_name = picker
            .cwd
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
                AgentType::ClaudeCode => ("", LOGO_CORAL), // Anthropic orange-ish
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

    let paragraph = Paragraph::new(lines).style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

pub fn render_session_picker(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(picker) = &app.session_picker {
        // Header
        lines.push(Line::from(vec![Span::styled(
            "Resume session",
            Style::new().fg(LOGO_LIGHT_BLUE).bold(),
        )]));
        lines.push(Line::raw("")); // spacing

        // List sessions
        for (i, session) in picker.sessions.iter().enumerate() {
            let is_selected = i == picker.selected;
            let cursor = if is_selected { "> " } else { "  " };

            // First line: cursor + folder name + timestamp
            let folder_name = session
                .cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let timestamp = session
                .timestamp
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
                    // Find valid char boundary
                    let mut end = max_len.saturating_sub(3);
                    while end > 0 && !prompt.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &prompt[..end])
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
            lines.push(Line::styled(
                "  (no resumable sessions found)",
                Style::new().fg(TEXT_DIM),
            ));
        }
    }

    let paragraph = Paragraph::new(lines).style(Style::new().fg(TEXT_WHITE));

    frame.render_widget(paragraph, area);
}

pub fn render_permission_dialog(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = vec![];

    if let Some(session) = app.selected_session()
        && let Some(perm) = &session.pending_permission
    {
        // Header - strip backticks from title
        let title = perm
            .title
            .clone()
            .unwrap_or_else(|| "Tool".to_string())
            .replace('`', "");
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

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(LOGO_GOLD));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

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

#[allow(clippy::vec_init_then_push)]
pub fn render_help_popup(frame: &mut Frame, area: Rect) {
    // Calculate centered popup area
    let popup_width = 50u16;
    let popup_height = 25u16;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(
        x,
        y,
        popup_width.min(area.width),
        popup_height.min(area.height),
    );

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = vec![];

    // Title
    lines.push(Line::from(vec![Span::styled(
        "Keyboard Shortcuts",
        Style::new().fg(TEXT_WHITE).bold(),
    )]));
    lines.push(Line::raw(""));

    // Normal mode
    lines.push(Line::styled(
        "Normal Mode",
        Style::new().fg(LOGO_LIGHT_BLUE).bold(),
    ));
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
        Span::styled("  d       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Duplicate session", Style::new().fg(TEXT_DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  v       ", Style::new().fg(TEXT_WHITE)),
        Span::styled("Cycle sort mode", Style::new().fg(TEXT_DIM)),
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
    lines.push(Line::styled(
        "Insert Mode",
        Style::new().fg(LOGO_MINT).bold(),
    ));
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
