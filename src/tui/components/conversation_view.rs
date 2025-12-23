//! Conversation view component - main chat/output display with markdown rendering.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, ClickRegion};
use crate::events::Action;
use crate::session::{OutputType, SessionState};
use crate::tui::theme::*;

use super::wrap_text;

/// Render the conversation view showing agent messages.
pub fn render_conversation_view(frame: &mut Frame, area: Rect, app: &mut App) {
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
            let debug_tool_json = app.debug_tool_json;

            // First expand all output to visual lines
            let mut all_lines: Vec<Line> = vec![];
            let mut last_line_type: Option<&OutputType> = None;

            for output_line in session.output.iter() {
                let mut lines_for_output: Vec<Line> = match &output_line.line_type {
                    OutputType::Text => {
                        // Empty lines for spacing
                        if output_line.content.is_empty() {
                            vec![Line::raw("")]
                        } else {
                            // Agent response - render as markdown using ratskin/termimad
                            let skin = ratskin::RatSkin::default();
                            skin.parse(
                                ratskin::RatSkin::parse_text(&output_line.content),
                                inner_width as u16,
                            )
                        }
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

                    OutputType::Thought => {
                        // Agent thinking - just show lightbulb and "Thinking..."
                        vec![Line::from(vec![
                            Span::styled("💡 ", Style::new().fg(LOGO_GOLD)),
                            Span::styled("Thinking...", Style::new().fg(LOGO_GOLD).italic()),
                        ])]
                    }
                    OutputType::ToolCall {
                        tool_call_id,
                        name,
                        description,
                        failed,
                        raw_json,
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
                        // Use the name (title) directly, rendered as markdown
                        let _ = description; // unused for now
                        let skin = ratskin::RatSkin::default();
                        let parsed_lines = skin.parse(
                            ratskin::RatSkin::parse_text(name),
                            inner_width.saturating_sub(2) as u16,
                        );
                        let mut lines: Vec<Line> = parsed_lines
                            .into_iter()
                            .enumerate()
                            .map(|(i, mut line)| {
                                let prefix = if i == 0 {
                                    Span::styled(
                                        indicator.clone(),
                                        Style::new().fg(indicator_color),
                                    )
                                } else {
                                    Span::styled("  ", Style::new().fg(indicator_color))
                                };
                                line.spans.insert(0, prefix);
                                line
                            })
                            .collect();

                        // If debug mode is on, render all raw JSON requests below the tool call
                        if debug_tool_json {
                            for json in raw_json {
                                for json_line in json.lines() {
                                    // Truncate long lines rather than wrap to preserve indentation
                                    let max_len = inner_width.saturating_sub(4);
                                    let display_line = if json_line.len() > max_len {
                                        format!("{}…", &json_line[..max_len.saturating_sub(1)])
                                    } else {
                                        json_line.to_string()
                                    };
                                    lines.push(Line::from(vec![
                                        Span::styled("  │ ", Style::new().fg(TEXT_DIM)),
                                        Span::styled(display_line, Style::new().fg(TEXT_DIM)),
                                    ]));
                                }
                            }
                        }

                        lines
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
                    OutputType::BashCommand => {
                        // Bash command - gold with $ prefix
                        let wrapped =
                            wrap_text(&output_line.content, inner_width.saturating_sub(2));
                        wrapped
                            .into_iter()
                            .enumerate()
                            .map(|(i, text)| {
                                if i == 0 {
                                    Line::from(vec![Span::styled(
                                        text,
                                        Style::new().fg(LOGO_GOLD).bold(),
                                    )])
                                } else {
                                    Line::from(vec![
                                        Span::styled("  ", Style::new()),
                                        Span::styled(text, Style::new().fg(LOGO_GOLD).bold()),
                                    ])
                                }
                            })
                            .collect()
                    }
                    OutputType::BashOutput => {
                        // Bash output - dim text with connector
                        let wrapped =
                            wrap_text(&output_line.content, inner_width.saturating_sub(2));
                        wrapped
                            .into_iter()
                            .map(|text| {
                                let prefix = Span::styled("│ ", Style::new().fg(LOGO_GOLD));
                                Line::from(vec![
                                    prefix,
                                    Span::styled(text, Style::new().fg(TEXT_DIM)),
                                ])
                            })
                            .collect()
                    }
                    OutputType::SystemMessage => {
                        // System message - light red/coral, italic
                        let wrapped =
                            wrap_text(&output_line.content, inner_width.saturating_sub(2));
                        wrapped
                            .into_iter()
                            .map(|text| {
                                Line::from(vec![Span::styled(
                                    text,
                                    Style::new().fg(LOGO_CORAL).italic(),
                                )])
                            })
                            .collect()
                    }
                };

                // Trim leading empty lines from this message
                while let Some(line) = lines_for_output.first() {
                    if line.spans.is_empty()
                        || line.spans.iter().all(|s| s.content.trim().is_empty())
                    {
                        lines_for_output.remove(0);
                    } else {
                        break;
                    }
                }

                // Trim trailing empty lines from this message
                while let Some(line) = lines_for_output.last() {
                    if line.spans.is_empty()
                        || line.spans.iter().all(|s| s.content.trim().is_empty())
                    {
                        lines_for_output.pop();
                    } else {
                        break;
                    }
                }

                // Add spacing when transitioning between different message types
                // This keeps diff lines together, tool output together, etc.
                let should_add_spacing = match (&last_line_type, &output_line.line_type) {
                    // Add spacing after user input
                    (Some(OutputType::UserInput), _) => true,
                    // Note: Thinking is now ephemeral and removed when new content arrives,
                    // so we don't need spacing rules for it anymore
                    // Add spacing after tool calls (before next content)
                    (
                        Some(OutputType::ToolCall { .. }),
                        OutputType::Text | OutputType::UserInput | OutputType::ToolCall { .. },
                    ) => true,
                    // Add spacing after text (agent response) before new user input or tool calls
                    (
                        Some(OutputType::Text),
                        OutputType::UserInput | OutputType::ToolCall { .. },
                    ) => true,
                    // Add spacing after tool output before new messages
                    (
                        Some(OutputType::ToolOutput),
                        OutputType::Text | OutputType::UserInput | OutputType::ToolCall { .. },
                    ) => true,
                    // Add spacing after bash output
                    (
                        Some(OutputType::BashOutput),
                        OutputType::Text | OutputType::UserInput | OutputType::ToolCall { .. },
                    ) => true,
                    // Don't add spacing between consecutive diff lines or within tool sequences
                    _ => false,
                };

                if should_add_spacing && !all_lines.is_empty() {
                    all_lines.push(Line::raw(""));
                }

                all_lines.extend(lines_for_output);
                last_line_type = Some(&output_line.line_type);
            }

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

    // Register output area as scrollable region
    let output_bounds = ClickRegion::new(area.x, area.y, area.width, area.height);
    app.interactions.register_scroll(
        "output_area",
        output_bounds,
        Action::ScrollUp(3),
        Action::ScrollDown(3),
    );

    // Update total_rendered_lines for accurate scroll calculations
    if let Some(total_lines) = computed_total_lines
        && let Some(session) = app.sessions.selected_session_mut()
    {
        session.total_rendered_lines = total_lines;
    }
}
