//! Sidebar component - logo, session list, plan entries, and hotkeys.

use std::collections::BTreeMap;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::acp::PlanStatus;
use crate::app::{App, ClickRegion, SortMode};
use crate::events::Action;
use crate::picker::Picker;
use crate::session::{Session, SessionState};
use crate::tui::interaction::InteractiveRegion;
use crate::tui::theme::*;

use super::wrap_text;

/// Render the colorful "amux" logo centered in the area.
pub fn render_logo(frame: &mut Frame, area: Rect) {
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

/// Render a single session entry and return the lines.
pub fn render_session_entry<'a>(
    session: &'a Session,
    index: usize,
    is_selected: bool,
    spinner: &str,
    start_dir: &std::path::Path,
    show_number: bool,
) -> Vec<Line<'a>> {
    let cursor = if is_selected { "> " } else { "  " };

    // Activity indicator for working sessions
    let (activity, activity_color) = if session.pending_permission.is_some() {
        (" ⚠".to_string(), LOGO_GOLD) // Permission required - orange/gold
    } else if session.pending_question.is_some() {
        (" ?".to_string(), LOGO_GOLD) // Question pending - orange/gold
    } else if session.state.is_active() {
        (format!(" {}", spinner), LOGO_MINT) // Animated spinner - green
    } else {
        (String::new(), LOGO_MINT)
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
            Span::styled(activity.clone(), Style::new().fg(activity_color)),
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
            Span::styled(activity.clone(), Style::new().fg(activity_color)),
        ])
    };

    // Second line: branch + worktree + diff stats + mode
    let mut second_spans = vec![
        Span::raw("   "),
        Span::styled("🌿 ", Style::new().fg(BRANCH_GREEN)),
        Span::styled(session.git_branch.clone(), Style::new().fg(TEXT_DIM)),
    ];

    // Show worktree indicator (compact)
    if session.is_worktree {
        second_spans.push(Span::styled(" (wt)", Style::new().fg(TEXT_DIM)));
    }

    // Show diff stats if available (e.g., "+45 -12")
    if let Some(ref diff_stats) = session.diff_stats
        && (diff_stats.insertions > 0 || diff_stats.deletions > 0)
    {
        second_spans.push(Span::raw("  "));
        if diff_stats.insertions > 0 {
            second_spans.push(Span::styled(
                format!("+{}", diff_stats.insertions),
                Style::new().fg(DIFF_ADD_FG),
            ));
        }
        if diff_stats.deletions > 0 {
            if diff_stats.insertions > 0 {
                second_spans.push(Span::raw(" "));
            }
            second_spans.push(Span::styled(
                format!("-{}", diff_stats.deletions),
                Style::new().fg(DIFF_REMOVE_FG),
            ));
        }
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

/// Extract a display name from a git origin URL.
fn origin_display_name(origin: &str) -> String {
    // origin is already normalized (e.g., "github.com/user/repo")
    // Extract just the repo name (last component)
    origin.rsplit('/').next().unwrap_or(origin).to_string()
}

/// Render the session list with hotkeys and plan at bottom.
pub fn render_session_list(frame: &mut Frame, area: Rect, app: &mut App) {
    // Start with empty line for padding after logo
    let mut session_lines: Vec<Line> = vec![Line::raw("")];

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
        SortMode::ByAgent => {
            // Sort by agent type for grouping
            sorted_indices.sort_by(|&a, &b| {
                sessions[a]
                    .agent_type
                    .display_name()
                    .cmp(sessions[b].agent_type.display_name())
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

    // For grouped modes, render with group headers
    if app.sort_mode == SortMode::Grouped || app.sort_mode == SortMode::ByAgent {
        // Group sessions by git origin or agent type
        let mut groups: BTreeMap<String, Vec<(usize, usize, &Session)>> = BTreeMap::new();

        for (display_idx, &original_idx) in sorted_indices.iter().enumerate() {
            let session = &sessions[original_idx];
            let key = if app.sort_mode == SortMode::ByAgent {
                session.agent_type.display_name().to_string()
            } else {
                session.git_origin.clone().unwrap_or_else(|| {
                    session
                        .cwd
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                })
            };
            groups
                .entry(key)
                .or_default()
                .push((display_idx, original_idx, session));
        }

        for (group_key, group_sessions) in &groups {
            // Group header - for ByAgent use key directly, otherwise extract display name
            let display_name = if app.sort_mode == SortMode::ByAgent {
                group_key.clone()
            } else {
                origin_display_name(group_key)
            };

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
                let entry_lines = render_session_entry(
                    session,
                    display_idx,
                    is_selected,
                    spinner,
                    &start_dir,
                    true,
                );

                // Register interactive region for session item
                let bounds = ClickRegion::new(area.x, line_y, area.width, 3);
                app.interactions.register_session_item(original_idx, bounds);

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

            // Register interactive region for session item
            let bounds = ClickRegion::new(area.x, line_y, area.width, 3);
            app.interactions.register_session_item(original_idx, bounds);

            session_lines.extend(entry_lines);
        }
    }

    // Update display order mapping for hotkey selection (1-9)
    app.session_display_order.display_to_internal = sorted_indices;

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
        let separator = "─".repeat(area.width.saturating_sub(1) as usize);
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

    // Track hotkey line position for click regions
    let hotkey_line_y = area.y + lines.len() as u16;

    lines.extend(hotkey_lines);
    lines.extend(plan_lines);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);

    // Register click regions for sidebar hotkeys (with priority to override session items)
    // "[?] help  " is 10 chars
    let help_bounds = ClickRegion::new(area.x, hotkey_line_y, 10, 1);
    app.interactions.register(
        InteractiveRegion::clickable("sidebar_help", help_bounds, Action::OpenHelp)
            .with_priority(1),
    );

    // "[v] <sort_mode>" starts at position 10
    let sort_bounds =
        ClickRegion::new(area.x + 10, hotkey_line_y, area.width.saturating_sub(10), 1);
    app.interactions.register(
        InteractiveRegion::clickable("sidebar_sort", sort_bounds, Action::CycleSortMode)
            .with_priority(1),
    );
}
