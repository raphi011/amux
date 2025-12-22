mod acp;
mod app;
mod clipboard;
mod config;
mod events;
mod git;
mod log;
mod picker;
mod scroll;
mod session;
mod tui;

use anyhow::Result;
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::{FutureExt, StreamExt};
use ratatui::prelude::*;
use std::collections::HashMap;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;

use acp::{
    AgentConnection, AgentEvent, AskUserResponse, ContentBlock, PermissionOptionId, SessionUpdate,
};
use app::{
    App, CleanupEntry, FolderEntry, ImageAttachment, InputMode, WorktreeConfig, WorktreeEntry,
};
use clipboard::ClipboardContent;
use events::Action;
use picker::Picker;
use session::{
    AgentType, OutputType, PendingPermission, PendingQuestion, SessionState, check_all_agents,
};

/// Internal app events for async operations
#[derive(Debug)]
enum AppEvent {
    /// A worktree deletion completed (path of deleted worktree)
    WorktreeDeleted(std::path::PathBuf),
    /// A worktree deletion failed (path, error message)
    WorktreeDeletionFailed(std::path::PathBuf, String),
    /// A bash command completed (session_id, command, output, success)
    BashCommandCompleted {
        session_id: String,
        command: String,
        output: String,
        success: bool,
    },
}

/// Get the current git branch for a directory
async fn get_git_branch(cwd: &std::path::Path) -> String {
    match tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}

/// Check if a directory is a git repository and get its branch
async fn get_git_branch_if_repo(dir: &std::path::Path) -> Option<String> {
    let git_dir = dir.join(".git");
    if git_dir.exists() {
        let branch = get_git_branch(dir).await;
        if !branch.is_empty() {
            return Some(branch);
        }
    }
    None
}

/// Format agent capabilities into a human-readable string
fn format_agent_capabilities(caps: &serde_json::Value) -> String {
    let mut parts = vec![];

    // MCP capabilities
    if let Some(mcp) = caps.get("mcpCapabilities") {
        let mut mcp_features = vec![];
        if mcp.get("http").and_then(|v| v.as_bool()).unwrap_or(false) {
            mcp_features.push("HTTP");
        }
        if mcp.get("sse").and_then(|v| v.as_bool()).unwrap_or(false) {
            mcp_features.push("SSE");
        }
        if !mcp_features.is_empty() {
            parts.push(format!("MCP: {}", mcp_features.join(", ")));
        }
    }

    // Prompt capabilities
    if let Some(prompt) = caps.get("promptCapabilities") {
        let mut prompt_features = vec![];
        if prompt
            .get("embeddedContext")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            prompt_features.push("embedded context");
        }
        if prompt
            .get("image")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            prompt_features.push("images");
        }
        if !prompt_features.is_empty() {
            parts.push(format!("Supports: {}", prompt_features.join(", ")));
        }
    }

    // Session capabilities
    if let Some(session) = caps.get("sessionCapabilities") {
        let mut session_features = vec![];
        if session.get("resume").is_some() {
            session_features.push("resume");
        }
        if !session_features.is_empty() {
            parts.push(format!("Session: {}", session_features.join(", ")));
        }
    }

    if parts.is_empty() {
        "Agent capabilities: (none reported)".to_string()
    } else {
        format!("Agent capabilities: {}", parts.join(" | "))
    }
}

/// Scan a directory for subdirectories
async fn scan_folder_entries(dir: &std::path::Path) -> Vec<FolderEntry> {
    let mut entries = vec![];

    // Add current directory entry
    let current_git_branch = get_git_branch_if_repo(dir).await;
    entries.push(FolderEntry {
        name: ". (current folder)".to_string(),
        path: dir.to_path_buf(),
        git_branch: current_git_branch,
        is_parent: false,
        is_current: true,
    });

    // Add parent directory entry if not at root
    if dir.parent().is_some() {
        entries.push(FolderEntry {
            name: "..".to_string(),
            path: dir.parent().unwrap().to_path_buf(),
            git_branch: None,
            is_parent: true,
            is_current: false,
        });
    }

    // Read directory entries
    if let Ok(mut read_dir) = tokio::fs::read_dir(dir).await {
        let mut dirs = vec![];
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(file_type) = entry.file_type().await
                && file_type.is_dir()
            {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip hidden directories
                if !name.starts_with('.') {
                    dirs.push((name, entry.path()));
                }
            }
        }

        // Sort alphabetically
        dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        // Check for git repos
        for (name, path) in dirs {
            let git_branch = get_git_branch_if_repo(&path).await;
            entries.push(FolderEntry {
                name,
                path,
                git_branch,
                is_parent: false,
                is_current: false,
            });
        }
    }

    entries
}

/// Scan the worktree directory for existing worktrees
async fn scan_worktrees(worktree_dir: &std::path::Path, fetch_first: bool) -> Vec<WorktreeEntry> {
    let mut entries = vec![];

    // Always add "Create new worktree" option first
    entries.push(WorktreeEntry {
        name: "+ Create new worktree".to_string(),
        path: std::path::PathBuf::new(),
        is_create_new: true,
        is_clean: false,
        is_merged: false,
    });

    // Scan existing worktrees
    if let Ok(mut read_dir) = tokio::fs::read_dir(worktree_dir).await {
        let mut worktree_paths = vec![];
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(file_type) = entry.file_type().await
                && file_type.is_dir()
            {
                let path = entry.path();
                // Only include if it looks like a git worktree (has .git file or directory)
                let git_path = path.join(".git");
                if git_path.exists() {
                    worktree_paths.push(path);
                }
            }
        }

        // Fetch from all unique parent repos first (for accurate merge status)
        if fetch_first {
            let mut fetched_repos = std::collections::HashSet::new();
            for path in &worktree_paths {
                if let Some(parent_repo) = get_worktree_parent_repo(path).await
                    && fetched_repos.insert(parent_repo.clone())
                {
                    log::log(&format!(
                        "Fetching from origin in {}",
                        parent_repo.display()
                    ));
                    if let Err(e) = git::fetch_origin(&parent_repo).await {
                        log::log(&format!("Failed to fetch: {}", e));
                    }
                }
            }
        }

        // Now get status for each worktree
        let mut worktrees = vec![];
        for path in worktree_paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let is_clean = git::is_worktree_clean(&path).await.unwrap_or(false);
            let is_merged = get_worktree_merged_status(&path).await;
            worktrees.push((name, path, is_clean, is_merged));
        }

        // Sort alphabetically
        worktrees.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        for (name, path, is_clean, is_merged) in worktrees {
            entries.push(WorktreeEntry {
                name,
                path,
                is_create_new: false,
                is_clean,
                is_merged,
            });
        }
    }

    entries
}

/// Get the parent repo path for a worktree
async fn get_worktree_parent_repo(worktree_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let gitdir_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(worktree_path)
        .output()
        .await;

    match gitdir_output {
        Ok(output) if output.status.success() => {
            let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let common_dir = std::path::PathBuf::from(dir);
            common_dir.parent().map(|p| p.to_path_buf())
        }
        _ => None,
    }
}

/// Get the merged status for a worktree by finding its parent repo and branch
async fn get_worktree_merged_status(worktree_path: &std::path::Path) -> bool {
    // Get current branch
    let branch_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .await;

    let branch = match branch_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => return false,
    };

    // Get the common git dir (parent repo)
    let gitdir_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(worktree_path)
        .output()
        .await;

    let common_dir = match gitdir_output {
        Ok(output) if output.status.success() => {
            let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            std::path::PathBuf::from(dir)
        }
        _ => return false,
    };

    // The parent repo is one level up from the .git directory
    let parent_repo = common_dir.parent().unwrap_or(&common_dir);

    git::is_branch_merged(parent_repo, &branch)
        .await
        .unwrap_or(false)
}

/// Command to send to an agent
enum AgentCommand {
    Prompt {
        session_id: String,
        text: String,
    },
    PromptWithContent {
        session_id: String,
        content: Vec<ContentBlock>,
    },
    PermissionResponse {
        request_id: u64,
        option_id: Option<PermissionOptionId>,
    },
    AskUserResponse {
        request_id: u64,
        answer: String,
    },
    SetModel {
        session_id: String,
        model_id: String,
    },
}

/// Info for resuming a session
#[derive(Clone)]
struct ResumeInfo {
    session_id: String,
    cwd: std::path::PathBuf,
}

/// Submit a bug report to GitHub using the gh CLI
async fn submit_bug_report(
    description: &str,
    log_path: &std::path::Path,
    session_id: &str,
) -> Result<()> {
    use std::process::Stdio;

    let repo = "raphi011/amux";

    // Build the issue body with session info
    let body = format!(
        "## Bug Description\n\n{}\n\n## Session Info\n\n- **Session ID:** `{}`\n- **Log file:** `{}`\n\n## Log Contents\n\n<details>\n<summary>Click to expand log</summary>\n\n```\n{}\n```\n\n</details>",
        description,
        session_id,
        log_path.display(),
        tokio::fs::read_to_string(log_path)
            .await
            .unwrap_or_else(|_| "(could not read log file)".to_string())
    );

    let title = format!(
        "[Bug Report] {}",
        if description.len() > 50 {
            format!("{}...", &description[..47])
        } else {
            description.to_string()
        }
    );

    log::log(&format!("Submitting bug report: {}", title));

    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "create", "--repo", repo, "--title", &title, "--body", &body,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout);
        log::log(&format!("Bug report submitted: {}", url.trim()));
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::log(&format!("Failed to submit bug report: {}", stderr));
        anyhow::bail!("gh command failed: {}", stderr);
    }

    Ok(())
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        "amux {VERSION} - Terminal multiplexer for AI coding agents

USAGE:
    amux [OPTIONS] [DIRECTORY]

ARGS:
    [DIRECTORY]    Start directory for new sessions (default: current directory)

OPTIONS:
    -w, --worktree-dir <PATH>    Directory for git worktrees
    -V, --version                Print version information
    -h, --help                   Print this help message
"
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments first (before initializing terminal)
    let args: Vec<String> = std::env::args().collect();
    let mut start_dir = std::env::current_dir().unwrap_or_default();
    let mut worktree_dir_override: Option<std::path::PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--version" | "-V" => {
                println!("amux {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--worktree-dir" | "-w" => {
                if i + 1 < args.len() {
                    let path = std::path::PathBuf::from(&args[i + 1]);
                    if path.is_dir() {
                        worktree_dir_override = Some(path.canonicalize().unwrap_or(path));
                    } else {
                        // Directory doesn't exist yet, that's ok - it will be created
                        worktree_dir_override = Some(path);
                    }
                    i += 2;
                    continue;
                } else {
                    eprintln!("Warning: --worktree-dir requires a path argument");
                    i += 1;
                }
            }
            arg if !arg.starts_with('-') => {
                let path = std::path::PathBuf::from(arg);
                if path.is_dir() {
                    start_dir = path.canonicalize().unwrap_or(path);
                } else {
                    eprintln!(
                        "Warning: '{}' is not a valid directory, using current directory",
                        arg
                    );
                }
            }
            _ => {
                // Unknown flag, ignore
            }
        }
        i += 1;
    }

    // Initialize logging and panic hook
    let (log_path, session_id) = if let Ok((log_path, session_id)) = log::init() {
        log::log(&format!("Log file: {}", log_path.display()));
        log::install_panic_hook();
        (Some(log_path), Some(session_id))
    } else {
        (None, None)
    };

    // Load config
    let config = config::Config::load();

    // Load worktree config with precedence: CLI > env var > config file > default
    let worktree_config =
        WorktreeConfig::load(worktree_dir_override.or(config.worktree_dir.clone()));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(start_dir, worktree_config, config.mcp_servers);
    app.log_path = log_path;
    app.session_id = session_id;

    // Run the app
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    // Channel for agent events (keyed by session ID for stable routing)
    let (agent_tx, mut agent_rx) = mpsc::channel::<(String, AgentEvent)>(100);

    // Channel for internal app events (worktree deletion, etc.)
    let (app_event_tx, mut app_event_rx) = mpsc::channel::<AppEvent>(32);

    // Channels for sending commands to agents (keyed by session ID)
    let mut agent_commands: HashMap<String, mpsc::Sender<AgentCommand>> = HashMap::new();

    // Event stream for keyboard
    let mut event_stream = EventStream::new();

    // Open folder picker on startup
    let start = app.start_dir.clone();
    app.open_folder_picker(start.clone());
    let entries = scan_folder_entries(&start).await;
    app.set_folder_entries(entries);

    loop {
        // Render
        terminal.draw(|frame| tui::ui::render(frame, app))?;

        // Handle events with timeout for responsiveness
        // Use biased select to prioritize keyboard input over agent events
        tokio::select! {
            biased;
            // Terminal events (keyboard, paste, etc.)
            maybe_event = event_stream.next() => {
                if let Some(Ok(event)) = maybe_event {
                    // Handle paste events (from drag & drop or Cmd+V in some terminals)
                    if let Event::Paste(text) = &event {
                        // Auto-switch to insert mode if in normal mode with a session selected
                        if app.input_mode == InputMode::Normal && app.sessions.selected_session().is_some() {
                            app.enter_insert_mode();
                        }

                        if app.input_mode == InputMode::Insert {
                            // Check if it's a path to an image file
                            if let Some(path) = clipboard::try_parse_image_path(text) {
                                if let Some((filename, mime_type, data)) = clipboard::load_image_from_path(&path) {
                                    app.add_attachment(ImageAttachment {
                                        filename,
                                        mime_type,
                                        data,
                                    });
                                } else {
                                    // Not a valid image, paste as text
                                    for c in text.chars() {
                                        app.input_char(c);
                                    }
                                }
                            } else {
                                // Regular text, paste it
                                for c in text.chars() {
                                    app.input_char(c);
                                }
                            }
                        }
                        continue;
                    }

                    // Handle mouse events using the interaction registry
                    if let Event::Mouse(mouse) = &event {
                        let x = mouse.column;
                        let y = mouse.row;

                        let action = match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                let action = app.interactions.handle_scroll_up(x, y);
                                if matches!(action, Action::None) {
                                    Action::ScrollUp(3)
                                } else {
                                    action
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                let action = app.interactions.handle_scroll_down(x, y);
                                if matches!(action, Action::None) {
                                    Action::ScrollDown(3)
                                } else {
                                    action
                                }
                            }
                            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                                app.interactions.handle_click(x, y)
                            }
                            _ => Action::None,
                        };

                        // Handle the action
                        match action {
                            Action::ScrollUp(n) => {
                                app.scroll_up(n);
                                continue;
                            }
                            Action::ScrollDown(n) => {
                                app.scroll_down(n);
                                continue;
                            }
                            Action::EnterInsertMode => {
                                if app.sessions.selected_session().is_some() {
                                    app.enter_insert_mode();
                                }
                                continue;
                            }
                            Action::CyclePermissionMode => {
                                let session_idx = app.sessions.selected_index();
                                if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                    session.cycle_permission_mode();
                                }
                                continue;
                            }
                            Action::CycleModel => {
                                if let Some(session) = app.sessions.selected_session_mut()
                                    && let Some(model_id) = session.cycle_model() {
                                        let local_id = session.id.clone();
                                        let acp_session_id = session.acp_session_id.clone().unwrap_or_default();
                                        if let Some(cmd_tx) = agent_commands.get(&local_id) {
                                            let _ = cmd_tx.send(AgentCommand::SetModel {
                                                session_id: acp_session_id,
                                                model_id,
                                            }).await;
                                        }
                                    }
                                continue;
                            }
                            Action::SelectSession(idx) => {
                                app.select_session(idx);
                                continue;
                            }
                            Action::SelectPermissionOption(idx) => {
                                // Select and immediately allow the clicked permission option
                                if let Some(session) = app.sessions.selected_session_mut()
                                    && let Some(perm) = &mut session.pending_permission
                                    && idx < perm.options.len()
                                {
                                    perm.selected = idx;
                                    let option_id = perm.selected_option()
                                        .map(|o| PermissionOptionId::from(o.option_id.clone()));
                                    let request_id = perm.request_id;
                                    let session_id = session.id.clone();
                                    if let Some(cmd_tx) = agent_commands.get(&session_id) {
                                        let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                                            request_id,
                                            option_id,
                                        }).await;
                                    }
                                    session.pending_permission = None;
                                    session.state = SessionState::Prompting;
                                    // Restore saved input if any
                                    if let Some((buffer, cursor)) = session.take_saved_input() {
                                        app.input_buffer = buffer;
                                        app.cursor_position = cursor;
                                    }
                                }
                                continue;
                            }
                            Action::None => {}
                            _ => {
                                // Other actions not handled by mouse in main loop
                            }
                        }
                    }

                    // Handle key events
                    if let Event::Key(key) = event
                    && key.kind == KeyEventKind::Press {
                        match app.input_mode {
                            InputMode::Normal => {
                                // Check if there's a pending permission request
                                let has_permission = app.sessions.selected_session()
                                    .map(|s| s.pending_permission.is_some())
                                    .unwrap_or(false);

                                // Check if there's a pending question
                                let has_question = app.sessions.selected_session()
                                    .map(|s| s.pending_question.is_some())
                                    .unwrap_or(false);

                                if has_permission {
                                    // Permission mode keys
                                    match key.code {
                                        KeyCode::Char('y') | KeyCode::Enter => {
                                            // Allow - select the first allow_once option
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(perm) = &session.pending_permission {
                                                    let option_id = perm.selected_option()
                                                        .or_else(|| perm.allow_once_option())
                                                        .map(|o| PermissionOptionId::from(o.option_id.clone()));
                                                    let request_id = perm.request_id;
                                                    let session_id = session.id.clone();
                                                    if let Some(cmd_tx) = agent_commands.get(&session_id) {
                                                        let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                                                            request_id,
                                                            option_id,
                                                        }).await;
                                                    }
                                                    session.pending_permission = None;
                                                    session.state = SessionState::Prompting;
                                                    // Restore saved input if any
                                                    if let Some((buffer, cursor)) = session.take_saved_input() {
                                                        app.input_buffer = buffer;
                                                        app.cursor_position = cursor;
                                                    }
                                                }
                                        }
                                        KeyCode::Char('n') | KeyCode::Esc => {
                                            // Reject/Cancel
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(perm) = &session.pending_permission {
                                                    let request_id = perm.request_id;
                                                    let session_id = session.id.clone();
                                                    if let Some(cmd_tx) = agent_commands.get(&session_id) {
                                                        let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                                                            request_id,
                                                            option_id: None, // Cancelled
                                                        }).await;
                                                    }
                                                    session.pending_permission = None;
                                                    session.state = SessionState::Idle;
                                                    // Restore saved input if any
                                                    if let Some((buffer, cursor)) = session.take_saved_input() {
                                                        app.input_buffer = buffer;
                                                        app.cursor_position = cursor;
                                                    }
                                                }
                                        }
                                        KeyCode::Char('j') | KeyCode::Down => {
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx)
                                                && let Some(perm) = &mut session.pending_permission {
                                                    perm.select_next();
                                                }
                                        }
                                        KeyCode::Char('k') | KeyCode::Up => {
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx)
                                                && let Some(perm) = &mut session.pending_permission {
                                                    perm.select_prev();
                                                }
                                        }
                                        _ => {}
                                    }
                                } else if has_question {
                                    // Question mode keys - allows typing
                                    match key.code {
                                        KeyCode::Enter => {
                                            // Submit answer
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &session.pending_question {
                                                    let answer = question.get_answer();
                                                    let request_id = question.request_id;
                                                    let session_id = session.id.clone();
                                                    if let Some(cmd_tx) = agent_commands.get(&session_id) {
                                                        let _ = cmd_tx.send(AgentCommand::AskUserResponse {
                                                            request_id,
                                                            answer,
                                                        }).await;
                                                    }
                                                    session.pending_question = None;
                                                    session.state = SessionState::Prompting;
                                                    // Restore saved input if any
                                                    if let Some((buffer, cursor)) = session.take_saved_input() {
                                                        app.input_buffer = buffer;
                                                        app.cursor_position = cursor;
                                                    }
                                                }
                                        }
                                        KeyCode::Esc => {
                                            // Cancel - send empty response
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &session.pending_question {
                                                    let request_id = question.request_id;
                                                    let session_id = session.id.clone();
                                                    if let Some(cmd_tx) = agent_commands.get(&session_id) {
                                                        let _ = cmd_tx.send(AgentCommand::AskUserResponse {
                                                            request_id,
                                                            answer: String::new(),
                                                        }).await;
                                                    }
                                                    session.pending_question = None;
                                                    session.state = SessionState::Idle;
                                                    // Restore saved input if any
                                                    if let Some((buffer, cursor)) = session.take_saved_input() {
                                                        app.input_buffer = buffer;
                                                        app.cursor_position = cursor;
                                                    }
                                                }
                                        }
                                        KeyCode::Char(c) => {
                                            // Type into input
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question {
                                                    question.input_char(c);
                                                }
                                        }
                                        KeyCode::Backspace => {
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question {
                                                    question.input_backspace();
                                                }
                                        }
                                        KeyCode::Delete => {
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question {
                                                    question.input_delete();
                                                }
                                        }
                                        KeyCode::Left => {
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question {
                                                    question.input_left();
                                                }
                                        }
                                        KeyCode::Right => {
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question {
                                                    question.input_right();
                                                }
                                        }
                                        KeyCode::Home => {
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question {
                                                    question.input_home();
                                                }
                                        }
                                        KeyCode::End => {
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question {
                                                    question.input_end();
                                                }
                                        }
                                        KeyCode::Up => {
                                            // Navigate options if available
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question
                                                    && !question.is_free_text() {
                                                        question.select_prev();
                                                    }
                                        }
                                        KeyCode::Down => {
                                            // Navigate options if available
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(question) = &mut session.pending_question
                                                    && !question.is_free_text() {
                                                        question.select_next();
                                                    }
                                        }
                                        KeyCode::Tab => {
                                            // Cycle permission mode even when answering questions
                                            if let Some(session) = app.sessions.selected_session_mut() {
                                                session.cycle_permission_mode();
                                            }
                                        }
                                        _ => {}
                                    }
                                } else {
                                    // Normal mode keys
                                    match key.code {
                                        KeyCode::Char('q') => return Ok(()),
                                        KeyCode::Char('?') => {
                                            app.open_help();
                                        }
                                        KeyCode::Char('B') => {
                                            app.open_bug_report();
                                        }

                                        KeyCode::Tab => {
                                            // Cycle permission mode for selected session
                                            if let Some(session) = app.sessions.selected_session_mut() {
                                                session.cycle_permission_mode();
                                            }
                                        }
                                        KeyCode::Char('m') => {
                                            // Cycle model for selected session
                                            if let Some(session) = app.sessions.selected_session_mut()
                                                && let Some(model_id) = session.cycle_model() {
                                                    let local_id = session.id.clone();
                                                    let acp_session_id = session.acp_session_id.clone().unwrap_or_default();
                                                    if let Some(cmd_tx) = agent_commands.get(&local_id) {
                                                        let _ = cmd_tx.send(AgentCommand::SetModel {
                                                            session_id: acp_session_id,
                                                            model_id,
                                                        }).await;
                                                    }
                                                }
                                        }
                                        // Number keys to select session directly (using display order)
                                        KeyCode::Char(c @ '1'..='9') => {
                                            let display_idx = (c as usize) - ('1' as usize);
                                            // Convert display index to internal index
                                            if let Some(internal_idx) = app.internal_index_for_display(display_idx) {
                                                app.select_session(internal_idx);
                                            }
                                        }
                                        KeyCode::Char('j') | KeyCode::Down => app.next_session(),
                                        KeyCode::Char('k') | KeyCode::Up => app.prev_session(),
                                        KeyCode::Char('i') | KeyCode::Enter => {
                                            if app.sessions.selected_session().is_some() {
                                                app.enter_insert_mode();
                                            }
                                        }
                                        KeyCode::Char('n') => {
                                            // Open folder picker starting from configured directory
                                            let start = app.start_dir.clone();
                                            app.open_folder_picker(start.clone());
                                            let entries = scan_folder_entries(&start).await;
                                            app.set_folder_entries(entries);
                                        }
                                        KeyCode::Char('w') => {
                                            // Open worktree picker (existing worktrees or create new)
                                            // Don't fetch here - only fetch when opening cleanup view
                                            let worktree_dir = app.worktree_config.worktree_dir.clone();
                                            let entries = scan_worktrees(&worktree_dir, false).await;
                                            app.open_worktree_picker(entries);
                                        }
                                        KeyCode::Char('x') => {
                                            if let Some(session) = app.sessions.selected_session() {
                                                let session_id = session.id.clone();
                                                agent_commands.remove(&session_id);
                                            }
                                            app.kill_selected_session();
                                        }
                                        KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            // Duplicate current session (same folder, same agent)
                                            if let Some(session) = app.sessions.selected_session() {
                                                let agent_type = session.agent_type;
                                                let cwd = session.cwd.clone();
                                                let is_worktree = session.is_worktree;
                                                spawn_agent_in_dir(app, &agent_tx, &mut agent_commands, agent_type, cwd, is_worktree).await?;
                                            }
                                        }
                                        KeyCode::Char('c') => {
                                            // Clear session (with confirmation)
                                            if app.sessions.selected_session().is_some() {
                                                app.open_clear_confirm();
                                            }
                                        }
                                        KeyCode::Char('v') => {
                                            // Cycle through sort modes
                                            app.cycle_sort_mode();
                                        }
                                        KeyCode::Char('t') => {
                                            // Toggle debug tool JSON display
                                            app.toggle_debug_tool_json();
                                        }

                                        // Scroll output - vim style
                                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            // Ctrl+u: half page up
                                            let half_page = app.viewport_height / 2;
                                            app.scroll_up(half_page);
                                        }
                                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            // Ctrl+d: half page down
                                            let half_page = app.viewport_height / 2;
                                            app.scroll_down(half_page);
                                        }
                                        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            // Ctrl+b: full page up (back)
                                            app.scroll_up(app.viewport_height);
                                        }
                                        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            // Ctrl+f: full page down (forward)
                                            app.scroll_down(app.viewport_height);
                                        }
                                        KeyCode::PageUp => app.scroll_up(app.viewport_height),
                                        KeyCode::PageDown => app.scroll_down(app.viewport_height),
                                        KeyCode::Char('g') => app.scroll_to_top(),
                                        KeyCode::Char('G') => app.scroll_to_bottom(),
                                        _ => {}
                                    }
                                }
                            }
                            InputMode::FolderPicker => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => {
                                        app.close_folder_picker();
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if let Some(picker) = &mut app.folder_picker {
                                            picker.select_next();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if let Some(picker) = &mut app.folder_picker {
                                            picker.select_prev();
                                        }
                                    }
                                    KeyCode::Char('l') | KeyCode::Right => {
                                        // Enter directory
                                        if app.folder_picker_enter_dir()
                                            && let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                    }
                                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                                        // Go up
                                        if app.folder_picker_go_up()
                                            && let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                    }
                                    KeyCode::Enter => {
                                        // Select folder and open agent picker
                                        if let Some(picker) = &app.folder_picker
                                            && let Some(entry) = picker.selected_entry() {
                                                if entry.is_parent {
                                                    // Go up
                                                    if app.folder_picker_go_up()
                                                        && let Some(picker) = &app.folder_picker {
                                                            let entries = scan_folder_entries(&picker.current_dir).await;
                                                            app.set_folder_entries(entries);
                                                        }
                                                } else {
                                                    let path = entry.path.clone();
                                                    app.close_folder_picker();
                                                    let agents = check_all_agents();
                                                    app.open_agent_picker(path, false, agents);
                                                }
                                            }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::WorktreePicker => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => {
                                        app.close_worktree_picker();
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if let Some(picker) = &mut app.worktree_picker {
                                            picker.select_next();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if let Some(picker) = &mut app.worktree_picker {
                                            picker.select_prev();
                                        }
                                    }
                                    KeyCode::Char('c') => {
                                        // Open cleanup for worktrees in the picker
                                        // Re-scan with fetch to get accurate merge status
                                        let worktree_dir = app.worktree_config.worktree_dir.clone();
                                        log::log(&format!("Worktree cleanup: scanning dir {:?}", worktree_dir));
                                        app.close_worktree_picker();

                                        let worktree_entries = scan_worktrees(&worktree_dir, true).await;
                                        log::log(&format!("Worktree cleanup: found {} entries", worktree_entries.len()));
                                        let entries: Vec<CleanupEntry> = worktree_entries.iter()
                                            .filter(|e| !e.is_create_new)
                                            .map(|e| {
                                                // Extract branch name from worktree name (format: repo-branch)
                                                let branch = e.name.split_once('-')
                                                    .map(|(_, b)| b.to_string());
                                                CleanupEntry {
                                                    path: e.path.clone(),
                                                    branch,
                                                    is_clean: e.is_clean,
                                                    is_merged: e.is_merged,
                                                    selected: false,
                                                    is_deleting: false,
                                                }
                                            })
                                            .collect();

                                        log::log(&format!("Worktree cleanup: {} cleanup entries after filter", entries.len()));
                                        if !entries.is_empty() {
                                            app.open_worktree_cleanup(worktree_dir, entries);
                                        } else {
                                            log::log("Worktree cleanup: no entries to clean up, not opening dialog");
                                        }
                                    }
                                    KeyCode::Enter => {
                                        if let Some(picker) = &app.worktree_picker
                                            && let Some(entry) = picker.selected_entry() {
                                                if entry.is_create_new {
                                                    // Create new worktree - go to folder picker
                                                    app.close_worktree_picker();
                                                    let start = app.start_dir.clone();
                                                    app.open_worktree_folder_picker(start.clone());
                                                    let entries = scan_folder_entries(&start).await;
                                                    app.set_folder_entries(entries);
                                                } else {
                                                    // Open existing worktree
                                                    let path = entry.path.clone();
                                                    app.close_worktree_picker();
                                                    let agents = check_all_agents();
                                                    app.open_agent_picker(path, true, agents);
                                                }
                                            }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::WorktreeFolderPicker => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => {
                                        app.close_folder_picker();
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if let Some(picker) = &mut app.folder_picker {
                                            picker.select_next();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if let Some(picker) = &mut app.folder_picker {
                                            picker.select_prev();
                                        }
                                    }
                                    KeyCode::Char('l') | KeyCode::Right => {
                                        // Enter directory
                                        if app.folder_picker_enter_dir()
                                            && let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                    }
                                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                                        // Go up
                                        if app.folder_picker_go_up()
                                            && let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                    }
                                    KeyCode::Enter => {
                                        // Select git repo and proceed to branch input
                                        if let Some(picker) = &app.folder_picker
                                            && let Some(entry) = picker.selected_entry() {
                                                if entry.is_parent {
                                                    // Go up
                                                    if app.folder_picker_go_up()
                                                        && let Some(picker) = &app.folder_picker {
                                                            let entries = scan_folder_entries(&picker.current_dir).await;
                                                            app.set_folder_entries(entries);
                                                        }
                                                } else if entry.git_branch.is_some() {
                                                    // This is a git repo - proceed to branch input
                                                    let repo_path = entry.path.clone();
                                                    app.close_folder_picker();

                                                    // Fetch branches and open branch input
                                                    match git::list_branches(&repo_path).await {
                                                        Ok(branches) => {
                                                            app.open_branch_input(repo_path, branches);
                                                        }
                                                        Err(e) => {
                                                            log::log(&format!("Failed to list branches: {}", e));
                                                        }
                                                    }
                                                }
                                                // Non-git directories are ignored - only git repos can be selected
                                            }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::BranchInput => {
                                match key.code {
                                    KeyCode::Esc => {
                                        app.close_branch_input();
                                    }
                                    KeyCode::Enter => {
                                        // Create worktree and open agent picker
                                        if let Some(branch_state) = &app.branch_input {
                                            let branch_name = branch_state.branch_name().to_string();
                                            if !branch_name.is_empty() {
                                                let repo_path = branch_state.repo_path.clone();
                                                let repo_name = git::repo_name(&repo_path);
                                                let worktree_path = app.worktree_config.worktree_path(&repo_name, &branch_name);

                                                // Check if branch exists locally or as remote
                                                let local_exists = git::branch_exists(&repo_path, &branch_name).await.unwrap_or(false);
                                                let remote_exists = git::remote_branch_exists(&repo_path, &branch_name).await.unwrap_or(false);
                                                let create_branch = !local_exists && !remote_exists;

                                                // Create worktree
                                                match git::create_worktree(&repo_path, &worktree_path, &branch_name, create_branch).await {
                                                    Ok(()) => {
                                                        app.close_branch_input();
                                                        // Open agent picker for the new worktree
                                                        let agents = check_all_agents();
                                                        app.open_agent_picker(worktree_path, true, agents);
                                                    }
                                                    Err(e) => {
                                                        log::log(&format!("Failed to create worktree: {}", e));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Tab => {
                                        // Accept autocomplete selection
                                        if let Some(branch_state) = &mut app.branch_input {
                                            branch_state.accept_selection();
                                        }
                                    }
                                    KeyCode::Down => {
                                        if let Some(branch_state) = &mut app.branch_input {
                                            branch_state.select_next();
                                        }
                                    }
                                    KeyCode::Up => {
                                        if let Some(branch_state) = &mut app.branch_input {
                                            branch_state.select_prev();
                                        }
                                    }
                                    KeyCode::Char(c) => {
                                        if let Some(branch_state) = &mut app.branch_input {
                                            branch_state.input.insert(branch_state.cursor_position, c);
                                            branch_state.cursor_position += 1;
                                            branch_state.update_filter();
                                            branch_state.show_autocomplete = true;
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        if let Some(branch_state) = &mut app.branch_input
                                            && branch_state.cursor_position > 0 {
                                                branch_state.cursor_position -= 1;
                                                branch_state.input.remove(branch_state.cursor_position);
                                                branch_state.update_filter();
                                                branch_state.show_autocomplete = true;
                                            }
                                    }
                                    KeyCode::Left => {
                                        if let Some(branch_state) = &mut app.branch_input
                                            && branch_state.cursor_position > 0 {
                                                branch_state.cursor_position -= 1;
                                            }
                                    }
                                    KeyCode::Right => {
                                        if let Some(branch_state) = &mut app.branch_input
                                            && branch_state.cursor_position < branch_state.input.len() {
                                                branch_state.cursor_position += 1;
                                            }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::AgentPicker => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => {
                                        app.close_agent_picker();
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if let Some(picker) = &mut app.agent_picker {
                                            picker.select_next();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if let Some(picker) = &mut app.agent_picker {
                                            picker.select_prev();
                                        }
                                    }
                                    KeyCode::Enter => {
                                        // Spawn session with selected agent (only if available)
                                        if let Some(picker) = &app.agent_picker
                                            && picker.selected_is_available()
                                            && let Some(agent_type) = picker.selected_agent()
                                        {
                                            let cwd = picker.cwd.clone();
                                            let is_worktree = picker.is_worktree;
                                            app.close_agent_picker();
                                            spawn_agent_in_dir(app, &agent_tx, &mut agent_commands, agent_type, cwd, is_worktree).await?;
                                        }
                                        // If not available, do nothing (user can see the ✗ markers)
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::SessionPicker => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => {
                                        app.close_session_picker();
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if let Some(picker) = &mut app.session_picker {
                                            picker.select_next();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if let Some(picker) = &mut app.session_picker {
                                            picker.select_prev();
                                        }
                                    }
                                    KeyCode::Enter => {
                                        // Resume selected session (defaults to ClaudeCode for now)
                                        if let Some(picker) = &app.session_picker
                                            && let Some(session) = picker.selected_session() {
                                                let resume_info = ResumeInfo {
                                                    session_id: session.session_id.clone(),
                                                    cwd: session.cwd.clone(),
                                                };
                                                app.close_session_picker();
                                                spawn_agent_with_resume(app, &agent_tx, &mut agent_commands, AgentType::ClaudeCode, resume_info).await?;
                                            }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::Help => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                                        app.close_help();
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::BugReport => {
                                match key.code {
                                    KeyCode::Esc => {
                                        app.close_bug_report();
                                    }
                                    KeyCode::Enter => {
                                        // Submit bug report
                                        if let Some((description, log_path)) = app.take_bug_report()
                                            && !description.trim().is_empty()
                                        {
                                            let session_id = app.session_id.clone().unwrap_or_default();
                                            tokio::spawn(async move {
                                                if let Err(e) = submit_bug_report(&description, &log_path, &session_id).await {
                                                    log::log(&format!("Failed to submit bug report: {}", e));
                                                }
                                            });
                                        }
                                    }
                                    KeyCode::Char(c) => {
                                        if let Some(bug_report) = &mut app.bug_report {
                                            bug_report.input_char(c);
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        if let Some(bug_report) = &mut app.bug_report {
                                            bug_report.input_backspace();
                                        }
                                    }
                                    KeyCode::Delete => {
                                        if let Some(bug_report) = &mut app.bug_report {
                                            bug_report.input_delete();
                                        }
                                    }
                                    KeyCode::Left => {
                                        if let Some(bug_report) = &mut app.bug_report {
                                            bug_report.input_left();
                                        }
                                    }
                                    KeyCode::Right => {
                                        if let Some(bug_report) = &mut app.bug_report {
                                            bug_report.input_right();
                                        }
                                    }
                                    KeyCode::Home => {
                                        if let Some(bug_report) = &mut app.bug_report {
                                            bug_report.input_home();
                                        }
                                    }
                                    KeyCode::End => {
                                        if let Some(bug_report) = &mut app.bug_report {
                                            bug_report.input_end();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::WorktreeCleanupRepoPicker => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => {
                                        app.close_folder_picker();
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if let Some(picker) = &mut app.folder_picker {
                                            picker.select_next();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if let Some(picker) = &mut app.folder_picker {
                                            picker.select_prev();
                                        }
                                    }
                                    KeyCode::Char('l') | KeyCode::Right => {
                                        // Enter directory
                                        if app.folder_picker_enter_dir()
                                            && let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                    }
                                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                                        // Go up
                                        if app.folder_picker_go_up()
                                            && let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                    }
                                    KeyCode::Enter => {
                                        // Select git repo and scan for cleanable worktrees
                                        if let Some(picker) = &app.folder_picker
                                            && let Some(entry) = picker.selected_entry() {
                                                if entry.is_parent {
                                                    // Go up
                                                    if app.folder_picker_go_up()
                                                        && let Some(picker) = &app.folder_picker {
                                                            let entries = scan_folder_entries(&picker.current_dir).await;
                                                            app.set_folder_entries(entries);
                                                        }
                                                } else if entry.git_branch.is_some() {
                                                    // This is a git repo - scan for worktrees
                                                    let repo_path = entry.path.clone();
                                                    app.close_folder_picker();

                                                    // Fetch first for accurate merge status
                                                    log::log(&format!("Fetching from origin in {}", repo_path.display()));
                                                    if let Err(e) = git::fetch_origin(&repo_path).await {
                                                        log::log(&format!("Failed to fetch: {}", e));
                                                    }

                                                    match git::list_worktrees(&repo_path).await {
                                                        Ok(worktrees) => {
                                                            if worktrees.is_empty() {
                                                                log::log("No worktrees found for this repository");
                                                            } else {
                                                                let entries: Vec<CleanupEntry> = worktrees.into_iter().map(|w| {
                                                                    CleanupEntry {
                                                                        path: w.path,
                                                                        branch: w.branch,
                                                                        is_clean: w.is_clean,
                                                                        is_merged: w.is_merged,
                                                                        selected: false,
                                                                        is_deleting: false,
                                                                    }
                                                                }).collect();
                                                                app.open_worktree_cleanup(repo_path, entries);
                                                            }
                                                        }
                                                        Err(e) => {
                                                            log::log(&format!("Failed to list worktrees: {}", e));
                                                        }
                                                    }
                                                }
                                            }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::WorktreeCleanup => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => {
                                        app.close_worktree_cleanup();
                                    }
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        if let Some(cleanup) = &mut app.worktree_cleanup {
                                            cleanup.select_next();
                                        }
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        if let Some(cleanup) = &mut app.worktree_cleanup {
                                            cleanup.select_prev();
                                        }
                                    }
                                    KeyCode::Char(' ') => {
                                        // Toggle selection of current entry
                                        if let Some(cleanup) = &mut app.worktree_cleanup {
                                            cleanup.toggle_selected();
                                        }
                                    }
                                    KeyCode::Char('a') => {
                                        // Select all cleanable
                                        if let Some(cleanup) = &mut app.worktree_cleanup {
                                            cleanup.select_all_cleanable();
                                        }
                                    }
                                    KeyCode::Char('n') => {
                                        // Deselect all
                                        if let Some(cleanup) = &mut app.worktree_cleanup {
                                            cleanup.deselect_all();
                                        }
                                    }
                                    KeyCode::Char('b') => {
                                        // Toggle delete branches option
                                        if let Some(cleanup) = &mut app.worktree_cleanup {
                                            cleanup.toggle_delete_branches();
                                        }
                                    }
                                    KeyCode::Enter => {
                                        // Perform cleanup of selected worktrees asynchronously
                                        if let Some(cleanup) = &mut app.worktree_cleanup
                                            && cleanup.has_selection() {
                                                let delete_branches = cleanup.delete_branches;
                                                let selected: Vec<_> = cleanup.selected_entries()
                                                    .iter()
                                                    .map(|e| (e.path.clone(), e.branch.clone()))
                                                    .collect();

                                                // Mark selected entries as deleting
                                                for entry in &mut cleanup.entries {
                                                    if entry.selected {
                                                        entry.is_deleting = true;
                                                    }
                                                }

                                                // Spawn async deletion tasks for each selected worktree
                                                for (worktree_path, branch) in selected {
                                                    let tx = app_event_tx.clone();
                                                    tokio::spawn(async move {
                                                        // Get the actual git repo for this worktree
                                                        let Some(parent_repo) = get_worktree_parent_repo(&worktree_path).await else {
                                                            let _ = tx.send(AppEvent::WorktreeDeletionFailed(
                                                                worktree_path.clone(),
                                                                "Failed to find parent repo".to_string(),
                                                            )).await;
                                                            return;
                                                        };

                                                        // Remove worktree
                                                        if let Err(e) = git::remove_worktree(&parent_repo, &worktree_path, false).await {
                                                            let _ = tx.send(AppEvent::WorktreeDeletionFailed(
                                                                worktree_path.clone(),
                                                                e.to_string(),
                                                            )).await;
                                                            return;
                                                        }
                                                        log::log(&format!("Removed worktree: {}", worktree_path.display()));

                                                        // Delete branch if requested
                                                        if delete_branches
                                                            && let Some(branch_name) = branch
                                                        {
                                                            if let Err(e) = git::delete_branch(&parent_repo, &branch_name, false).await {
                                                                log::log(&format!("Failed to delete branch {}: {}", branch_name, e));
                                                            } else {
                                                                log::log(&format!("Deleted branch: {}", branch_name));
                                                            }
                                                        }

                                                        // Signal success
                                                        let _ = tx.send(AppEvent::WorktreeDeleted(worktree_path)).await;
                                                    });
                                                }
                                            }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::ClearConfirm => {
                                match key.code {
                                    KeyCode::Char('y') | KeyCode::Enter => {
                                        // Clear and respawn session
                                        if let Some(session) = app.sessions.selected_session() {
                                            let agent_type = session.agent_type;
                                            let cwd = session.cwd.clone();
                                            let is_worktree = session.is_worktree;
                                            let old_session_id = session.id.clone();

                                            // Remove the old session's command channel
                                            agent_commands.remove(&old_session_id);

                                            // Kill the old session
                                            app.kill_selected_session();

                                            // Close the confirmation dialog
                                            app.close_clear_confirm();

                                            // Spawn a new session with the same settings
                                            spawn_agent_in_dir(app, &agent_tx, &mut agent_commands, agent_type, cwd, is_worktree).await?;
                                        }
                                    }
                                    KeyCode::Char('n') | KeyCode::Esc => {
                                        app.close_clear_confirm();
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::Insert => {
                                // Log all key events in insert mode for debugging
                                log::log(&format!("Insert mode key: {:?}, modifiers: {:?}", key.code, key.modifiers));

                                // Check if there's a pending permission request
                                let has_permission = app.sessions.selected_session()
                                    .map(|s| s.pending_permission.is_some())
                                    .unwrap_or(false);

                                // Check if there's a pending question
                                let has_question = app.sessions.selected_session()
                                    .map(|s| s.pending_question.is_some())
                                    .unwrap_or(false);

                                match key.code {
                                    KeyCode::Esc => {
                                        app.exit_insert_mode();
                                        app.clear_attachments();
                                    }
                                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+C: clear input but stay in insert mode
                                        app.take_input();
                                        app.clear_attachments();
                                    }
                                    KeyCode::Enter if has_permission => {
                                        // Handle permission approval (same as normal mode)
                                        if let Some(session) = app.sessions.selected_session_mut()
                                            && let Some(perm) = &session.pending_permission {
                                                let option_id = perm.selected_option()
                                                    .or_else(|| perm.allow_once_option())
                                                    .map(|o| PermissionOptionId::from(o.option_id.clone()));
                                                let request_id = perm.request_id;
                                                let session_id = session.id.clone();
                                                if let Some(cmd_tx) = agent_commands.get(&session_id) {
                                                    let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                                                        request_id,
                                                        option_id,
                                                    }).await;
                                                }
                                                session.pending_permission = None;
                                                session.state = SessionState::Prompting;
                                            }
                                    }
                                    KeyCode::Enter if has_question => {
                                        // Handle question submission (same as normal mode)
                                        if let Some(session) = app.sessions.selected_session_mut()
                                            && let Some(question) = &session.pending_question {
                                                let answer = question.get_answer();
                                                let request_id = question.request_id;
                                                let session_id = session.id.clone();
                                                if let Some(cmd_tx) = agent_commands.get(&session_id) {
                                                    let _ = cmd_tx.send(AgentCommand::AskUserResponse {
                                                        request_id,
                                                        answer,
                                                    }).await;
                                                }
                                                session.pending_question = None;
                                                session.state = SessionState::Prompting;
                                            }
                                    }
                                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                        // Shift+Enter: insert newline
                                        app.input_char('\n');
                                    }
                                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+J: insert newline (traditional Unix)
                                        app.input_char('\n');
                                    }
                                    KeyCode::Enter => {
                                        let is_bash = app.is_bash_mode();
                                        let text = app.take_input();
                                        if is_bash && !text.is_empty() {
                                            // Execute bash command
                                            if let Some(session) = app.sessions.selected_session() {
                                                let session_id = session.id.clone();
                                                let cwd = session.cwd.clone();
                                                let command = text.clone();

                                                // Add command to output
                                                let session_idx = app.sessions.selected_index();
                                                if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                    session.add_output(format!("$ {}", command), OutputType::BashCommand);
                                                }

                                                // Start tracking the command
                                                app.start_bash_command(command.clone());

                                                // Execute asynchronously
                                                let tx = app_event_tx.clone();
                                                tokio::spawn(async move {
                                                    let output = tokio::process::Command::new("sh")
                                                        .arg("-c")
                                                        .arg(&command)
                                                        .current_dir(&cwd)
                                                        .output()
                                                        .await;

                                                    let (output_text, success) = match output {
                                                        Ok(out) => {
                                                            let stdout = String::from_utf8_lossy(&out.stdout);
                                                            let stderr = String::from_utf8_lossy(&out.stderr);
                                                            let combined = if stderr.is_empty() {
                                                                stdout.to_string()
                                                            } else if stdout.is_empty() {
                                                                stderr.to_string()
                                                            } else {
                                                                format!("{}\n{}", stdout, stderr)
                                                            };
                                                            (combined, out.status.success())
                                                        }
                                                        Err(e) => (format!("Error: {}", e), false),
                                                    };

                                                    let _ = tx.send(AppEvent::BashCommandCompleted {
                                                        session_id,
                                                        command,
                                                        output: output_text,
                                                        success,
                                                    }).await;
                                                });
                                            }
                                        } else if !text.is_empty() || app.has_attachments() {
                                            send_prompt(app, &agent_commands, &text).await;
                                        }
                                        app.exit_insert_mode();
                                    }
                                    KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+V: paste from clipboard
                                        match clipboard::read_clipboard() {
                                            Ok(ClipboardContent::Image { data, mime_type }) => {
                                                app.add_attachment(ImageAttachment {
                                                    filename: "clipboard".to_string(), // Shows as "Image #N" in UI
                                                    mime_type,
                                                    data,
                                                });
                                            }
                                            Ok(ClipboardContent::Text(text)) => {
                                                // Check if it's a path to an image file
                                                if let Some(path) = clipboard::try_parse_image_path(&text) {
                                                    if let Some((filename, mime_type, data)) = clipboard::load_image_from_path(&path) {
                                                        app.add_attachment(ImageAttachment {
                                                            filename,
                                                            mime_type,
                                                            data,
                                                        });
                                                    } else {
                                                        // Not a valid image, paste as text
                                                        for c in text.chars() {
                                                            app.input_char(c);
                                                        }
                                                    }
                                                } else {
                                                    // Regular text, paste it
                                                    for c in text.chars() {
                                                        app.input_char(c);
                                                    }
                                                }
                                            }
                                            Ok(ClipboardContent::None) | Err(_) => {}
                                        }
                                    }
                                    KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+X: clear all attachments
                                        app.clear_attachments();
                                    }
                                    KeyCode::Tab => {
                                        // Cycle permission mode for selected session
                                        let session_idx = app.sessions.selected_index();
                                        if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                            session.cycle_permission_mode();
                                        }
                                    }
                                    // Word/line navigation
                                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+A: jump to start of line
                                        app.input_home();
                                    }
                                    KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+E: jump to end of line
                                        app.input_end();
                                    }
                                    KeyCode::Home => app.input_home(),
                                    KeyCode::End => app.input_end(),
                                    KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+Left: move word left
                                        app.input_word_left();
                                    }
                                    KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+Right: move word right
                                        app.input_word_right();
                                    }
                                    KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+B: move word left (emacs style)
                                        app.input_word_left();
                                    }
                                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+F: move word right (emacs style)
                                        app.input_word_right();
                                    }
                                    // Word/line deletion
                                    KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+W: delete word before cursor
                                        app.input_delete_word_back();
                                    }
                                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+Backspace: delete word before cursor
                                        app.input_delete_word_back();
                                    }
                                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::ALT) => {
                                        // Alt+D: delete word after cursor
                                        app.input_delete_word_forward();
                                    }
                                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+K: delete to end of line
                                        app.input_kill_line();
                                    }
                                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+U: delete to start of line
                                        app.input_kill_to_start();
                                    }
                                    // Attachment navigation
                                    KeyCode::Up => {
                                        // Move to attachment selection if there are attachments
                                        if app.has_attachments() && app.selected_attachment.is_none() {
                                            app.select_attachments();
                                        }
                                    }
                                    KeyCode::Down => {
                                        // Move back to input from attachment selection
                                        if app.selected_attachment.is_some() {
                                            app.deselect_attachments();
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        if app.selected_attachment.is_some() {
                                            // Delete selected attachment
                                            app.delete_selected_attachment();
                                        } else {
                                            app.input_backspace();
                                        }
                                    }
                                    KeyCode::Delete => {
                                        if app.selected_attachment.is_some() {
                                            app.delete_selected_attachment();
                                        } else {
                                            app.input_delete();
                                        }
                                    }
                                    KeyCode::Left => {
                                        log::log(&format!("Left key pressed, cursor_position={}, input_len={}", app.cursor_position, app.input_buffer.len()));
                                        if app.selected_attachment.is_some() {
                                            app.attachment_left();
                                        } else {
                                            app.input_left();
                                        }
                                        log::log(&format!("After input_left, cursor_position={}", app.cursor_position));
                                    }
                                    KeyCode::Right => {
                                        log::log(&format!("Right key pressed, cursor_position={}, input_len={}", app.cursor_position, app.input_buffer.len()));
                                        if app.selected_attachment.is_some() {
                                            app.attachment_right();
                                        } else {
                                            app.input_right();
                                        }
                                        log::log(&format!("After input_right, cursor_position={}", app.cursor_position));
                                    }
                                    KeyCode::Char(c) => {
                                        // Typing deselects attachment and goes back to input
                                        if app.selected_attachment.is_some() {
                                            app.deselect_attachments();
                                        }
                                        app.input_char(c);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } // end Event::Key
                }

                // Drain any additional pending terminal events before re-rendering
                while let Some(Some(Ok(event))) = event_stream.next().now_or_never() {
                    // Handle paste events
                    if let Event::Paste(text) = &event {
                        if app.input_mode == InputMode::Normal && app.sessions.selected_session().is_some() {
                            app.enter_insert_mode();
                        }
                        if app.input_mode == InputMode::Insert {
                            if let Some(path) = clipboard::try_parse_image_path(text) {
                                if let Some((filename, mime_type, data)) = clipboard::load_image_from_path(&path) {
                                    app.add_attachment(ImageAttachment { filename, mime_type, data });
                                } else {
                                    for c in text.chars() { app.input_char(c); }
                                }
                            } else {
                                for c in text.chars() { app.input_char(c); }
                            }
                        }
                        continue;
                    }

                    // Handle key events in drain loop
                    if let Event::Key(key) = event
                    && key.kind == KeyEventKind::Press {
                        // Only handle simple insert-mode typing in the drain loop
                        // Complex actions (mode switches, commands) are handled in the main loop
                        if app.input_mode == InputMode::Insert {
                            match key.code {
                                KeyCode::Char(c) => {
                                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                                        app.input_char(c);
                                    }
                                }
                                KeyCode::Backspace => { app.input_backspace(); }
                                KeyCode::Left => { app.input_left(); }
                                KeyCode::Right => { app.input_right(); }
                                _ => break, // Complex key, exit drain loop and let main loop handle
                            }
                        } else {
                            // Non-insert mode, exit drain loop to handle properly
                            break;
                        }
                    }
                }
            }

            // Agent events
            Some((session_id, event)) = agent_rx.recv() => {
                let result = handle_agent_event(app, &session_id, event);
                // Handle auto-accept permission responses
                if let EventResult::AutoAcceptPermission { request_id, option_id } = result
                    && let Some(cmd_tx) = agent_commands.get(&session_id) {
                        let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                            request_id,
                            option_id: Some(option_id),
                        }).await;
                    }
            }

            // Internal app events (worktree deletion, etc.)
            Some(event) = app_event_rx.recv() => {
                match event {
                    AppEvent::WorktreeDeleted(path) => {
                        // Kill any sessions running in the deleted worktree
                        let sessions_to_kill: Vec<String> = app.sessions.sessions()
                            .iter()
                            .filter(|s| s.cwd == path)
                            .map(|s| s.id.clone())
                            .collect();

                        for session_id in sessions_to_kill {
                            log::log(&format!("Killing session {} in deleted worktree {}", session_id, path.display()));
                            agent_commands.remove(&session_id);
                            // Remove session from manager
                            app.sessions.sessions_mut().retain(|s| s.id != session_id);
                        }

                        // Remove the deleted entry from the cleanup list
                        if let Some(cleanup) = &mut app.worktree_cleanup {
                            cleanup.entries.retain(|e| e.path != path);
                            // If all entries are deleted, close the cleanup picker
                            if cleanup.entries.is_empty() {
                                app.close_worktree_cleanup();
                            }
                        }
                    }
                    AppEvent::WorktreeDeletionFailed(path, error) => {
                        log::log(&format!("Failed to delete worktree {}: {}", path.display(), error));
                        // Mark entry as no longer deleting (so user can retry)
                        if let Some(cleanup) = &mut app.worktree_cleanup
                            && let Some(entry) = cleanup.entries.iter_mut().find(|e| e.path == path)
                        {
                            entry.is_deleting = false;
                            entry.selected = false;
                        }
                    }
                    #[allow(unused_variables)]
                    AppEvent::BashCommandCompleted { session_id, command, output, success } => {
                        // Clear the running command tracker
                        app.complete_bash_command();

                        // Find the session and add the output
                        if let Some(session) = app.sessions.sessions_mut().iter_mut().find(|s| s.id == session_id) {
                            // Add output lines
                            if !output.is_empty() {
                                for line in output.lines() {
                                    let output_type = if success {
                                        OutputType::BashOutput
                                    } else {
                                        OutputType::Error
                                    };
                                    session.add_output(line.to_string(), output_type);
                                }
                            }
                            // Add empty line for spacing
                            session.add_output(String::new(), OutputType::Text);
                            // Scroll to bottom to show the output
                            session.scroll_to_bottom();
                        }
                    }
                }
            }

            // Timeout to keep UI responsive and tick spinner (16ms = ~60 FPS)
            _ = tokio::time::sleep(Duration::from_millis(16)) => {
                app.tick_spinner();
            }
        }
    }
}

async fn spawn_agent_in_dir(
    app: &mut App,
    agent_tx: &mpsc::Sender<(String, AgentEvent)>,
    agent_commands: &mut HashMap<String, mpsc::Sender<AgentCommand>>,
    agent_type: AgentType,
    cwd: std::path::PathBuf,
    is_worktree: bool,
) -> Result<()> {
    let session_id = app.spawn_session(agent_type, cwd.clone(), is_worktree);

    // Detect git branch and origin
    let branch = get_git_branch(&cwd).await;
    let origin = git::get_origin_url(&cwd).await;
    if let Some(session) = app.sessions.get_by_id_mut(&session_id) {
        session.git_branch = branch;
        session.git_origin = origin;
    }

    // Convert MCP servers from config format to protocol format
    let mcp_servers: Vec<acp::McpServer> =
        app.mcp_servers.iter().map(acp::McpServer::from).collect();

    // Channel for commands to this agent
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<AgentCommand>(32);
    agent_commands.insert(session_id.clone(), cmd_tx.clone());

    // Event channel for this agent
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);

    // Forward events to main channel (using session_id for stable routing)
    let main_tx = agent_tx.clone();
    let session_id_for_events = session_id.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if main_tx
                .send((session_id_for_events.clone(), event))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Spawn the agent task
    let cwd_clone = cwd.clone();
    tokio::spawn(async move {
        match AgentConnection::spawn(agent_type, &cwd_clone, event_tx.clone()).await {
            Ok(mut conn) => {
                // Initialize
                if let Err(e) = conn.initialize().await {
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            message: format!("Init failed: {}", e),
                        })
                        .await;
                    return;
                }

                // Create session with MCP servers
                if let Err(e) = conn
                    .new_session(cwd_clone.to_str().unwrap_or("."), mcp_servers)
                    .await
                {
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            message: format!("Session failed: {}", e),
                        })
                        .await;
                    return;
                }

                // Listen for commands
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        AgentCommand::Prompt { session_id, text } => {
                            if let Err(e) = conn.prompt(&session_id, &text).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Prompt failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::PromptWithContent {
                            session_id,
                            content,
                        } => {
                            if let Err(e) = conn.prompt_with_content(&session_id, content).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Prompt failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::PermissionResponse {
                            request_id,
                            option_id,
                        } => {
                            if let Err(e) = conn.respond_permission(request_id, option_id).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Permission response failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::AskUserResponse { request_id, answer } => {
                            let response = AskUserResponse::text(answer);
                            if let Err(e) = conn.respond_ask_user(request_id, response).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Ask user response failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::SetModel {
                            session_id,
                            model_id,
                        } => {
                            if let Err(e) = conn.set_model(&session_id, &model_id).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Set model failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = event_tx
                    .send(AgentEvent::Error {
                        message: format!("Spawn failed: {}", e),
                    })
                    .await;
            }
        }
    });

    Ok(())
}

async fn spawn_agent_with_resume(
    app: &mut App,
    agent_tx: &mpsc::Sender<(String, AgentEvent)>,
    agent_commands: &mut HashMap<String, mpsc::Sender<AgentCommand>>,
    agent_type: AgentType,
    resume_info: ResumeInfo,
) -> Result<()> {
    let cwd = resume_info.cwd.clone();
    let resume_session_id = resume_info.session_id.clone();
    // Check if cwd is inside the worktree directory
    let is_worktree = cwd.starts_with(&app.worktree_config.worktree_dir);
    let session_id = app.spawn_session(agent_type, cwd.clone(), is_worktree);

    // Detect git branch
    let branch = get_git_branch(&cwd).await;
    if let Some(session) = app.sessions.get_by_id_mut(&session_id) {
        session.git_branch = branch;
    }

    // Convert MCP servers from config format to protocol format
    let mcp_servers: Vec<acp::McpServer> =
        app.mcp_servers.iter().map(acp::McpServer::from).collect();

    // Channel for commands to this agent
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<AgentCommand>(32);
    agent_commands.insert(session_id.clone(), cmd_tx.clone());

    // Event channel for this agent
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);

    // Forward events to main channel (using session_id for stable routing)
    let main_tx = agent_tx.clone();
    let session_id_for_events = session_id.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if main_tx
                .send((session_id_for_events.clone(), event))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Spawn the agent task with session resume
    let cwd_clone = cwd.clone();
    tokio::spawn(async move {
        match AgentConnection::spawn(agent_type, &cwd_clone, event_tx.clone()).await {
            Ok(mut conn) => {
                // Initialize
                if let Err(e) = conn.initialize().await {
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            message: format!("Init failed: {}", e),
                        })
                        .await;
                    return;
                }

                // Load existing session with MCP servers
                if let Err(e) = conn
                    .load_session(
                        &resume_session_id,
                        cwd_clone.to_str().unwrap_or("."),
                        mcp_servers,
                    )
                    .await
                {
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            message: format!("Session load failed: {}", e),
                        })
                        .await;
                    return;
                }

                // Listen for commands
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        AgentCommand::Prompt { session_id, text } => {
                            if let Err(e) = conn.prompt(&session_id, &text).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Prompt failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::PromptWithContent {
                            session_id,
                            content,
                        } => {
                            if let Err(e) = conn.prompt_with_content(&session_id, content).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Prompt failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::PermissionResponse {
                            request_id,
                            option_id,
                        } => {
                            if let Err(e) = conn.respond_permission(request_id, option_id).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Permission response failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::AskUserResponse { request_id, answer } => {
                            let response = AskUserResponse::text(answer);
                            if let Err(e) = conn.respond_ask_user(request_id, response).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Ask user response failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                        AgentCommand::SetModel {
                            session_id,
                            model_id,
                        } => {
                            if let Err(e) = conn.set_model(&session_id, &model_id).await {
                                let _ = event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Set model failed: {}", e),
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = event_tx
                    .send(AgentEvent::Error {
                        message: format!("Spawn failed: {}", e),
                    })
                    .await;
            }
        }
    });

    Ok(())
}

async fn send_prompt(
    app: &mut App,
    agent_commands: &HashMap<String, mpsc::Sender<AgentCommand>>,
    text: &str,
) {
    // Take attachments before borrowing session
    let attachments = std::mem::take(&mut app.attachments);
    let has_attachments = !attachments.is_empty();

    if let Some(session) = app.sessions.selected_session_mut() {
        // Add spacing before user message
        session.add_output(String::new(), OutputType::Text);

        // Show user input with attachment indicator
        if has_attachments {
            let attachment_names: Vec<_> =
                attachments.iter().map(|a| a.filename.as_str()).collect();
            session.add_output(
                format!("> {} [+{}]", text, attachment_names.join(", ")),
                OutputType::UserInput,
            );
        } else {
            session.add_output(format!("> {}", text), OutputType::UserInput);
        }
        session.state = SessionState::Prompting;

        // Use local ID for HashMap lookup, ACP session ID for protocol
        let local_id = session.id.clone();
        let acp_session_id = session.acp_session_id.clone().unwrap_or_default();

        // Build content blocks
        if has_attachments {
            let mut content: Vec<ContentBlock> = vec![];

            // Add text if present
            if !text.is_empty() {
                content.push(ContentBlock::Text {
                    text: text.to_string(),
                });
            }

            // Add image attachments
            for attachment in attachments {
                content.push(ContentBlock::Image {
                    mime_type: attachment.mime_type,
                    data: attachment.data,
                });
            }

            // Send with content blocks
            if let Some(cmd_tx) = agent_commands.get(&local_id) {
                let _ = cmd_tx
                    .send(AgentCommand::PromptWithContent {
                        session_id: acp_session_id,
                        content,
                    })
                    .await;
            }
        } else {
            // Send simple text prompt
            if let Some(cmd_tx) = agent_commands.get(&local_id) {
                let _ = cmd_tx
                    .send(AgentCommand::Prompt {
                        session_id: acp_session_id,
                        text: text.to_string(),
                    })
                    .await;
            }
        }
    }
}

/// Result of handling an agent event - may contain a command to send back
enum EventResult {
    None,
    AutoAcceptPermission {
        request_id: u64,
        option_id: PermissionOptionId,
    },
}

fn handle_agent_event(app: &mut App, session_id: &str, event: AgentEvent) -> EventResult {
    // Get these values before taking mutable borrow of sessions
    let is_insert_mode = app.input_mode == InputMode::Insert;
    let input_buffer = app.input_buffer.clone();
    let cursor_position = app.cursor_position;

    // Check if this session is the currently selected one
    let is_selected_session = app
        .sessions
        .selected_session()
        .map(|s| s.id == session_id)
        .unwrap_or(false);

    if let Some(session) = app.sessions.get_by_id_mut(session_id) {
        match event {
            AgentEvent::Initialized {
                agent_info,
                agent_capabilities,
            } => {
                session.state = SessionState::Initializing;
                if let Some(info) = agent_info
                    && let Some(name) = info.name
                {
                    session.add_output(format!("Connected to {}", name), OutputType::Text);
                }
                if let Some(caps) = agent_capabilities {
                    // Format capabilities nicely
                    let formatted = format_agent_capabilities(&caps);
                    session.add_output(formatted, OutputType::Text);
                }
            }
            AgentEvent::SessionCreated { session_id, models } => {
                // Store the ACP session ID (used in protocol messages)
                // Keep session.id as the local stable ID (used for HashMap keys)
                session.acp_session_id = Some(session_id);
                session.state = SessionState::Idle;
                // Store model info if available
                if let Some(models_state) = models {
                    session.available_models = models_state.available_models;
                    session.current_model_id = Some(models_state.current_model_id);
                }
                session.add_output(
                    "Session ready. Press [i] to type.".to_string(),
                    OutputType::Text,
                );
            }
            AgentEvent::Update { update, .. } => {
                match update {
                    SessionUpdate::AgentMessageChunk { content } => {
                        if let acp::protocol::UpdateContent::Text { text } = content {
                            session.append_text(text);
                        }
                    }
                    SessionUpdate::AgentThoughtChunk { content } => {
                        if let acp::protocol::UpdateContent::Text { text } = content {
                            session.append_thought(text);
                        }
                    }
                    SessionUpdate::ToolCall {
                        tool_call_id,
                        title,
                        raw_json,
                        ..
                    } => {
                        // Use title directly, falling back to "Tool" if empty/undefined
                        let name = title
                            .filter(|t| {
                                let trimmed = t.trim();
                                !trimmed.is_empty() && trimmed != "undefined" && trimmed != "null"
                            })
                            .unwrap_or_else(|| "Tool".to_string());

                        // Only add spacing for new tool calls, not updates
                        let is_new = !session.has_tool_call(&tool_call_id);
                        if is_new {
                            session.add_output(String::new(), OutputType::Text);
                        }
                        session.add_tool_call(tool_call_id, name, None, raw_json);
                    }
                    SessionUpdate::ToolCallUpdate {
                        tool_call_id,
                        status,
                    } => {
                        // Check if this tool is completing
                        if status == "completed" {
                            // Mark the tool as complete if it's the active one
                            if session.active_tool_call_id.as_ref() == Some(&tool_call_id) {
                                session.complete_active_tool();
                            }
                        } else if status == "error" || status == "failed" {
                            // Mark the tool as failed
                            session.mark_tool_failed(&tool_call_id);
                        } else if !status.trim().is_empty()
                            && status != "in_progress"
                            && status != "pending"
                        {
                            // Only show meaningful status updates (not lifecycle states)
                            session.add_tool_output(status);
                        }
                    }
                    SessionUpdate::Plan { entries } => {
                        session.plan_entries = entries;
                    }
                    SessionUpdate::CurrentModeUpdate { current_mode_id } => {
                        session.current_mode = Some(current_mode_id);
                    }
                    SessionUpdate::AvailableCommandsUpdate => {
                        // Silently ignore - not needed for UI
                    }
                    SessionUpdate::Other { raw_type } => {
                        session.add_output(
                            format!("[Unknown update: {}]", raw_type.as_deref().unwrap_or("?")),
                            OutputType::Text,
                        );
                    }
                }
            }
            AgentEvent::PermissionRequest {
                request_id,
                tool_call_id,
                title,
                options,
                ..
            } => {
                // Check if we should auto-accept (AcceptAll or Yolo mode)
                if session.permission_mode.auto_accepts() {
                    // Find the first allow_once option
                    if let Some(option) = options
                        .iter()
                        .find(|o| o.kind == crate::acp::PermissionKind::AllowOnce)
                    {
                        session.state = SessionState::Prompting;
                        // Auto-scroll to bottom only if already at bottom
                        if session.scroll_offset == usize::MAX {
                            session.scroll_to_bottom();
                        }
                        return EventResult::AutoAcceptPermission {
                            request_id,
                            option_id: PermissionOptionId::from(option.option_id.clone()),
                        };
                    }
                }

                // Normal mode - show permission dialog
                session.state = SessionState::AwaitingPermission;
                session.pending_permission = Some(PendingPermission {
                    request_id,
                    tool_call_id,
                    title,
                    options,
                    selected: 0,
                });

                // Save input buffer if user was typing in this session
                if is_selected_session && is_insert_mode && !input_buffer.is_empty() {
                    session.save_input(input_buffer.clone(), cursor_position);
                }
            }
            AgentEvent::AskUserRequest {
                request_id,
                question,
                options,
                multi_select,
                ..
            } => {
                // Show clarifying question dialog
                session.state = SessionState::AwaitingUserInput;
                session.pending_question = Some(PendingQuestion::new(
                    request_id,
                    question,
                    options,
                    multi_select,
                ));

                // Save input buffer if user was typing in this session
                if is_selected_session && is_insert_mode && !input_buffer.is_empty() {
                    session.save_input(input_buffer.clone(), cursor_position);
                }
            }
            AgentEvent::PromptComplete { .. } => {
                session.state = SessionState::Idle;
                session.pending_permission = None;
                session.complete_active_tool();
                // Add blank line after response for spacing
                session.add_output(String::new(), OutputType::Text);
            }
            AgentEvent::FileWritten { diff, .. } => {
                // Show the diff (file path is already shown in the tool call)
                session.add_tool_output(diff);
            }
            AgentEvent::Error { message } => {
                session.state = SessionState::Idle;
                session.add_output(format!("Error: {}", message), OutputType::Error);
            }
            AgentEvent::Disconnected => {
                session.state = SessionState::Idle;
                session.add_output("Disconnected".to_string(), OutputType::Text);
            }
        }
        // Auto-scroll to bottom only if already at bottom (not scrolled up)
        if session.scroll_offset == usize::MAX {
            session.scroll_to_bottom();
        }
    }
    EventResult::None
}
