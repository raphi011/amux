mod acp;
mod app;
mod clipboard;
mod git;
mod log;
mod session;
mod tui;

use anyhow::Result;
use crossterm::{
    event::{Event, KeyCode, KeyEventKind, KeyModifiers, EventStream, EnableBracketedPaste, DisableBracketedPaste},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::prelude::*;
use std::collections::HashMap;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;

use acp::{AgentConnection, AgentEvent, SessionUpdate, PermissionOptionId, ContentBlock, AskUserResponse};
use app::{App, FolderEntry, InputMode, ImageAttachment, WorktreeConfig, WorktreeEntry};
use clipboard::ClipboardContent;
use session::{AgentType, OutputType, SessionState, PendingPermission, PendingQuestion, PermissionMode};

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

/// Scan a directory for subdirectories
async fn scan_folder_entries(dir: &std::path::Path) -> Vec<FolderEntry> {
    let mut entries = vec![];

    // Add parent directory entry if not at root
    if dir.parent().is_some() {
        entries.push(FolderEntry {
            name: "..".to_string(),
            path: dir.parent().unwrap().to_path_buf(),
            git_branch: None,
            is_parent: true,
        });
    }

    // Read directory entries
    if let Ok(mut read_dir) = tokio::fs::read_dir(dir).await {
        let mut dirs = vec![];
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Skip hidden directories
                    if !name.starts_with('.') {
                        dirs.push((name, entry.path()));
                    }
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
            });
        }
    }

    entries
}

/// Scan the worktree directory for existing worktrees
async fn scan_worktrees(worktree_dir: &std::path::Path) -> Vec<WorktreeEntry> {
    let mut entries = vec![];

    // Always add "Create new worktree" option first
    entries.push(WorktreeEntry {
        name: "+ Create new worktree".to_string(),
        path: std::path::PathBuf::new(),
        is_create_new: true,
    });

    // Scan existing worktrees
    if let Ok(mut read_dir) = tokio::fs::read_dir(worktree_dir).await {
        let mut worktrees = vec![];
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let path = entry.path();
                    // Only include if it looks like a git worktree (has .git file or directory)
                    let git_path = path.join(".git");
                    if git_path.exists() {
                        worktrees.push((name, path));
                    }
                }
            }
        }

        // Sort alphabetically
        worktrees.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        for (name, path) in worktrees {
            entries.push(WorktreeEntry {
                name,
                path,
                is_create_new: false,
            });
        }
    }

    entries
}

/// Command to send to an agent
enum AgentCommand {
    Prompt { session_id: String, text: String },
    PromptWithContent { session_id: String, content: Vec<ContentBlock> },
    PermissionResponse { request_id: u64, option_id: Option<PermissionOptionId> },
    AskUserResponse { request_id: u64, answer: String },
    CancelPrompt,
    SetModel { session_id: String, model_id: String },
}

/// Info for resuming a session
#[derive(Clone)]
struct ResumeInfo {
    session_id: String,
    cwd: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    if let Ok(log_path) = log::init() {
        log::log(&format!("Log file: {}", log_path.display()));
    }

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let mut start_dir = std::env::current_dir().unwrap_or_default();
    let mut worktree_dir_override: Option<std::path::PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
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
                    eprintln!("Warning: '{}' is not a valid directory, using current directory", arg);
                }
            }
            _ => {
                // Unknown flag, ignore
            }
        }
        i += 1;
    }

    // Load worktree config with precedence: CLI > env var > default
    let worktree_config = WorktreeConfig::load(worktree_dir_override);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(start_dir, worktree_config);

    // Run the app
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableBracketedPaste, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    // Channel for agent events
    let (agent_tx, mut agent_rx) = mpsc::channel::<(usize, AgentEvent)>(100);

    // Channels for sending commands to agents (one per session)
    let mut agent_commands: HashMap<usize, mpsc::Sender<AgentCommand>> = HashMap::new();

    // Event stream for keyboard
    let mut event_stream = EventStream::new();

    loop {
        // Render
        terminal.draw(|frame| tui::ui::render(frame, app))?;

        // Handle events with timeout for responsiveness
        tokio::select! {
            // Terminal events (keyboard, paste, etc.)
            maybe_event = event_stream.next() => {
                if let Some(Ok(event)) = maybe_event {
                    // Handle paste events (from drag & drop or Cmd+V in some terminals)
                    if let Event::Paste(text) = &event {
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

                    // Handle key events
                    if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
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
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(perm) = &session.pending_permission {
                                                    let option_id = perm.selected_option()
                                                        .or_else(|| perm.allow_once_option())
                                                        .map(|o| PermissionOptionId::from(o.option_id.clone()));
                                                    let request_id = perm.request_id;
                                                    if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                                                        let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                                                            request_id,
                                                            option_id,
                                                        }).await;
                                                    }
                                                    session.pending_permission = None;
                                                    session.state = SessionState::Prompting;
                                                }
                                            }
                                        }
                                        KeyCode::Char('n') | KeyCode::Esc => {
                                            // Reject/Cancel
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(perm) = &session.pending_permission {
                                                    let request_id = perm.request_id;
                                                    if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                                                        let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                                                            request_id,
                                                            option_id: None, // Cancelled
                                                        }).await;
                                                    }
                                                    session.pending_permission = None;
                                                    session.state = SessionState::Idle;
                                                }
                                            }
                                        }
                                        KeyCode::Char('j') | KeyCode::Down => {
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(perm) = &mut session.pending_permission {
                                                    perm.select_next();
                                                }
                                            }
                                        }
                                        KeyCode::Char('k') | KeyCode::Up => {
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(perm) = &mut session.pending_permission {
                                                    perm.select_prev();
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                } else if has_question {
                                    // Question mode keys - allows typing
                                    let session_idx = app.sessions.selected_index();
                                    match key.code {
                                        KeyCode::Enter => {
                                            // Submit answer
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &session.pending_question {
                                                    let answer = question.get_answer();
                                                    let request_id = question.request_id;
                                                    if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                                                        let _ = cmd_tx.send(AgentCommand::AskUserResponse {
                                                            request_id,
                                                            answer,
                                                        }).await;
                                                    }
                                                    session.pending_question = None;
                                                    session.state = SessionState::Prompting;
                                                }
                                            }
                                        }
                                        KeyCode::Esc => {
                                            // Cancel - send empty response
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &session.pending_question {
                                                    let request_id = question.request_id;
                                                    if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                                                        let _ = cmd_tx.send(AgentCommand::AskUserResponse {
                                                            request_id,
                                                            answer: String::new(),
                                                        }).await;
                                                    }
                                                    session.pending_question = None;
                                                    session.state = SessionState::Idle;
                                                }
                                            }
                                        }
                                        KeyCode::Char(c) => {
                                            // Type into input
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    question.input_char(c);
                                                }
                                            }
                                        }
                                        KeyCode::Backspace => {
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    question.input_backspace();
                                                }
                                            }
                                        }
                                        KeyCode::Delete => {
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    question.input_delete();
                                                }
                                            }
                                        }
                                        KeyCode::Left => {
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    question.input_left();
                                                }
                                            }
                                        }
                                        KeyCode::Right => {
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    question.input_right();
                                                }
                                            }
                                        }
                                        KeyCode::Home => {
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    question.input_home();
                                                }
                                            }
                                        }
                                        KeyCode::End => {
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    question.input_end();
                                                }
                                            }
                                        }
                                        KeyCode::Up => {
                                            // Navigate options if available
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    if !question.is_free_text() {
                                                        question.select_prev();
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Down => {
                                            // Navigate options if available
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(question) = &mut session.pending_question {
                                                    if !question.is_free_text() {
                                                        question.select_next();
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Tab => {
                                            // Cycle permission mode even when answering questions
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
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
                                        KeyCode::Esc => {
                                            // Cancel current prompt if session is working
                                            let session_idx = app.sessions.selected_index();
                                            let is_prompting = app.sessions.selected_session()
                                                .map(|s| s.state == SessionState::Prompting)
                                                .unwrap_or(false);
                                            if is_prompting {
                                                if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                                                    let _ = cmd_tx.send(AgentCommand::CancelPrompt).await;
                                                }
                                                if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                    session.add_output("⚠ Cancelled".to_string(), OutputType::Text);
                                                    session.state = SessionState::Idle;
                                                }
                                            }
                                        }
                                        KeyCode::Tab => {
                                            // Cycle permission mode for selected session
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                session.cycle_permission_mode();
                                            }
                                        }
                                        KeyCode::Char('m') => {
                                            // Cycle model for selected session
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                if let Some(model_id) = session.cycle_model() {
                                                    let session_id = session.id.clone();
                                                    if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                                                        let _ = cmd_tx.send(AgentCommand::SetModel {
                                                            session_id,
                                                            model_id,
                                                        }).await;
                                                    }
                                                }
                                            }
                                        }
                                        // Number keys to select session directly
                                        KeyCode::Char(c @ '1'..='9') => {
                                            let idx = (c as usize) - ('1' as usize);
                                            app.sessions.select_index(idx);
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
                                            let worktree_dir = app.worktree_config.worktree_dir.clone();
                                            let entries = scan_worktrees(&worktree_dir).await;
                                            app.open_worktree_picker(entries);
                                        }
                                        KeyCode::Char('x') => {
                                            let idx = app.sessions.selected_index();
                                            agent_commands.remove(&idx);
                                            app.kill_selected_session();
                                        }
                                        KeyCode::Char('c') => {
                                            // Clear current session output
                                            let session_idx = app.sessions.selected_index();
                                            if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
                                                session.clear();
                                            }
                                        }
                                        // TODO: 'r' for resume - waiting for session/load ACP support
                                        // KeyCode::Char('r') => {
                                        //     let sessions = scan_resumable_sessions().await;
                                        //     if !sessions.is_empty() {
                                        //         app.open_session_picker(sessions);
                                        //     }
                                        // }
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
                                        if app.folder_picker_enter_dir() {
                                            if let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                        }
                                    }
                                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                                        // Go up
                                        if app.folder_picker_go_up() {
                                            if let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                        }
                                    }
                                    KeyCode::Enter => {
                                        // Select folder and open agent picker
                                        if let Some(picker) = &app.folder_picker {
                                            if let Some(entry) = picker.selected_entry() {
                                                if entry.is_parent {
                                                    // Go up
                                                    if app.folder_picker_go_up() {
                                                        if let Some(picker) = &app.folder_picker {
                                                            let entries = scan_folder_entries(&picker.current_dir).await;
                                                            app.set_folder_entries(entries);
                                                        }
                                                    }
                                                } else {
                                                    let path = entry.path.clone();
                                                    app.close_folder_picker();
                                                    app.open_agent_picker(path, false);
                                                }
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
                                    KeyCode::Enter => {
                                        if let Some(picker) = &app.worktree_picker {
                                            if let Some(entry) = picker.selected_entry() {
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
                                                    app.open_agent_picker(path, true);
                                                }
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
                                        if app.folder_picker_enter_dir() {
                                            if let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                        }
                                    }
                                    KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                                        // Go up
                                        if app.folder_picker_go_up() {
                                            if let Some(picker) = &app.folder_picker {
                                                let entries = scan_folder_entries(&picker.current_dir).await;
                                                app.set_folder_entries(entries);
                                            }
                                        }
                                    }
                                    KeyCode::Enter => {
                                        // Select git repo and proceed to branch input
                                        if let Some(picker) = &app.folder_picker {
                                            if let Some(entry) = picker.selected_entry() {
                                                if entry.is_parent {
                                                    // Go up
                                                    if app.folder_picker_go_up() {
                                                        if let Some(picker) = &app.folder_picker {
                                                            let entries = scan_folder_entries(&picker.current_dir).await;
                                                            app.set_folder_entries(entries);
                                                        }
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
                                                        app.open_agent_picker(worktree_path, true);
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
                                        if let Some(branch_state) = &mut app.branch_input {
                                            if branch_state.cursor_position > 0 {
                                                branch_state.cursor_position -= 1;
                                                branch_state.input.remove(branch_state.cursor_position);
                                                branch_state.update_filter();
                                                branch_state.show_autocomplete = true;
                                            }
                                        }
                                    }
                                    KeyCode::Left => {
                                        if let Some(branch_state) = &mut app.branch_input {
                                            if branch_state.cursor_position > 0 {
                                                branch_state.cursor_position -= 1;
                                            }
                                        }
                                    }
                                    KeyCode::Right => {
                                        if let Some(branch_state) = &mut app.branch_input {
                                            if branch_state.cursor_position < branch_state.input.len() {
                                                branch_state.cursor_position += 1;
                                            }
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
                                        // Spawn session with selected agent
                                        if let Some(picker) = &app.agent_picker {
                                            let agent_type = picker.selected_agent();
                                            let cwd = picker.cwd.clone();
                                            let is_worktree = picker.is_worktree;
                                            app.close_agent_picker();
                                            spawn_agent_in_dir(app, &agent_tx, &mut agent_commands, agent_type, cwd, is_worktree).await?;
                                        }
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
                                        if let Some(picker) = &app.session_picker {
                                            if let Some(session) = picker.selected_session() {
                                                let resume_info = ResumeInfo {
                                                    session_id: session.session_id.clone(),
                                                    cwd: session.cwd.clone(),
                                                };
                                                app.close_session_picker();
                                                spawn_agent_with_resume(app, &agent_tx, &mut agent_commands, AgentType::ClaudeCode, resume_info).await?;
                                            }
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
                            InputMode::Insert => {
                                match key.code {
                                    KeyCode::Esc => {
                                        app.exit_insert_mode();
                                        app.clear_attachments();
                                    }
                                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        // Ctrl+C: clear input and exit insert mode
                                        app.take_input();
                                        app.clear_attachments();
                                        app.exit_insert_mode();
                                    }
                                    KeyCode::Enter => {
                                        let text = app.take_input();
                                        if !text.is_empty() || app.has_attachments() {
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
                                        if app.selected_attachment.is_some() {
                                            app.attachment_left();
                                        } else {
                                            app.input_left();
                                        }
                                    }
                                    KeyCode::Right => {
                                        if app.selected_attachment.is_some() {
                                            app.attachment_right();
                                        } else {
                                            app.input_right();
                                        }
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
                    }
                    } // end Event::Key
                }
            }

            // Agent events
            Some((session_idx, event)) = agent_rx.recv() => {
                let result = handle_agent_event(app, session_idx, event);
                // Handle auto-accept permission responses
                if let EventResult::AutoAcceptPermission { request_id, option_id } = result {
                    if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                        let _ = cmd_tx.send(AgentCommand::PermissionResponse {
                            request_id,
                            option_id: Some(option_id),
                        }).await;
                    }
                }
            }

            // Timeout to keep UI responsive and tick spinner
            _ = tokio::time::sleep(Duration::from_millis(80)) => {
                app.tick_spinner();
            }
        }
    }
}

async fn spawn_agent_in_dir(
    app: &mut App,
    agent_tx: &mpsc::Sender<(usize, AgentEvent)>,
    agent_commands: &mut HashMap<usize, mpsc::Sender<AgentCommand>>,
    agent_type: AgentType,
    cwd: std::path::PathBuf,
    is_worktree: bool,
) -> Result<()> {
    let session_idx = app.spawn_session(agent_type, cwd.clone(), is_worktree);

    // Detect git branch
    let branch = get_git_branch(&cwd).await;
    if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
        session.git_branch = branch;
    }

    // Channel for commands to this agent
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<AgentCommand>(32);
    agent_commands.insert(session_idx, cmd_tx.clone());

    // Event channel for this agent
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);

    // Forward events to main channel
    let main_tx = agent_tx.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if main_tx.send((session_idx, event)).await.is_err() {
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
                    let _ = event_tx.send(AgentEvent::Error {
                        message: format!("Init failed: {}", e),
                    }).await;
                    return;
                }

                // Create session
                if let Err(e) = conn.new_session(cwd_clone.to_str().unwrap_or(".")).await {
                    let _ = event_tx.send(AgentEvent::Error {
                        message: format!("Session failed: {}", e),
                    }).await;
                    return;
                }

                // Listen for commands
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        AgentCommand::Prompt { session_id, text } => {
                            if let Err(e) = conn.prompt(&session_id, &text).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Prompt failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::PromptWithContent { session_id, content } => {
                            if let Err(e) = conn.prompt_with_content(&session_id, content).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Prompt failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::PermissionResponse { request_id, option_id } => {
                            if let Err(e) = conn.respond_permission(request_id, option_id).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Permission response failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::AskUserResponse { request_id, answer } => {
                            let response = AskUserResponse::text(answer);
                            if let Err(e) = conn.respond_ask_user(request_id, response).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Ask user response failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::CancelPrompt => {
                            if let Err(e) = conn.cancel_prompt().await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Cancel failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::SetModel { session_id, model_id } => {
                            if let Err(e) = conn.set_model(&session_id, &model_id).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Set model failed: {}", e),
                                }).await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = event_tx.send(AgentEvent::Error {
                    message: format!("Spawn failed: {}", e),
                }).await;
            }
        }
    });

    Ok(())
}

async fn spawn_agent_with_resume(
    app: &mut App,
    agent_tx: &mpsc::Sender<(usize, AgentEvent)>,
    agent_commands: &mut HashMap<usize, mpsc::Sender<AgentCommand>>,
    agent_type: AgentType,
    resume_info: ResumeInfo,
) -> Result<()> {
    let cwd = resume_info.cwd.clone();
    let session_id = resume_info.session_id.clone();
    // Check if cwd is inside the worktree directory
    let is_worktree = cwd.starts_with(&app.worktree_config.worktree_dir);
    let session_idx = app.spawn_session(agent_type, cwd.clone(), is_worktree);

    // Detect git branch
    let branch = get_git_branch(&cwd).await;
    if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
        session.git_branch = branch;
        session.id = session_id.clone(); // Pre-set the session ID
    }

    // Channel for commands to this agent
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<AgentCommand>(32);
    agent_commands.insert(session_idx, cmd_tx.clone());

    // Event channel for this agent
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);

    // Forward events to main channel
    let main_tx = agent_tx.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if main_tx.send((session_idx, event)).await.is_err() {
                break;
            }
        }
    });

    // Spawn the agent task with session resume
    let cwd_clone = cwd.clone();
    let session_id_clone = session_id.clone();
    tokio::spawn(async move {
        match AgentConnection::spawn(agent_type, &cwd_clone, event_tx.clone()).await {
            Ok(mut conn) => {
                // Initialize
                if let Err(e) = conn.initialize().await {
                    let _ = event_tx.send(AgentEvent::Error {
                        message: format!("Init failed: {}", e),
                    }).await;
                    return;
                }

                // Load existing session
                if let Err(e) = conn.load_session(&session_id_clone, cwd_clone.to_str().unwrap_or(".")).await {
                    let _ = event_tx.send(AgentEvent::Error {
                        message: format!("Session load failed: {}", e),
                    }).await;
                    return;
                }

                // Listen for commands
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        AgentCommand::Prompt { session_id, text } => {
                            if let Err(e) = conn.prompt(&session_id, &text).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Prompt failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::PromptWithContent { session_id, content } => {
                            if let Err(e) = conn.prompt_with_content(&session_id, content).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Prompt failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::PermissionResponse { request_id, option_id } => {
                            if let Err(e) = conn.respond_permission(request_id, option_id).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Permission response failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::AskUserResponse { request_id, answer } => {
                            let response = AskUserResponse::text(answer);
                            if let Err(e) = conn.respond_ask_user(request_id, response).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Ask user response failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::CancelPrompt => {
                            if let Err(e) = conn.cancel_prompt().await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Cancel failed: {}", e),
                                }).await;
                            }
                        }
                        AgentCommand::SetModel { session_id, model_id } => {
                            if let Err(e) = conn.set_model(&session_id, &model_id).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Set model failed: {}", e),
                                }).await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = event_tx.send(AgentEvent::Error {
                    message: format!("Spawn failed: {}", e),
                }).await;
            }
        }
    });

    Ok(())
}

async fn send_prompt(
    app: &mut App,
    agent_commands: &HashMap<usize, mpsc::Sender<AgentCommand>>,
    text: &str,
) {
    let session_idx = app.sessions.selected_index();

    // Take attachments before borrowing session
    let attachments = std::mem::take(&mut app.attachments);
    let has_attachments = !attachments.is_empty();

    if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
        // Add spacing before user message
        session.add_output(String::new(), OutputType::Text);

        // Show user input with attachment indicator
        if has_attachments {
            let attachment_names: Vec<_> = attachments.iter().map(|a| a.filename.as_str()).collect();
            session.add_output(
                format!("> {} [+{}]", text, attachment_names.join(", ")),
                OutputType::UserInput,
            );
        } else {
            session.add_output(format!("> {}", text), OutputType::UserInput);
        }
        session.state = SessionState::Prompting;

        // Build content blocks
        if has_attachments {
            let mut content: Vec<ContentBlock> = vec![];

            // Add text if present
            if !text.is_empty() {
                content.push(ContentBlock::Text { text: text.to_string() });
            }

            // Add image attachments
            for attachment in attachments {
                content.push(ContentBlock::Image {
                    mime_type: attachment.mime_type,
                    data: attachment.data,
                });
            }

            // Send with content blocks
            if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                let _ = cmd_tx.send(AgentCommand::PromptWithContent {
                    session_id: session.id.clone(),
                    content,
                }).await;
            }
        } else {
            // Send simple text prompt
            if let Some(cmd_tx) = agent_commands.get(&session_idx) {
                let _ = cmd_tx.send(AgentCommand::Prompt {
                    session_id: session.id.clone(),
                    text: text.to_string(),
                }).await;
            }
        }
    }
}

/// Result of handling an agent event - may contain a command to send back
enum EventResult {
    None,
    AutoAcceptPermission { request_id: u64, option_id: PermissionOptionId },
}

fn handle_agent_event(app: &mut App, session_idx: usize, event: AgentEvent) -> EventResult {
    if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
        match event {
            AgentEvent::Initialized { agent_info } => {
                session.state = SessionState::Initializing;
                if let Some(info) = agent_info {
                    if let Some(name) = info.name {
                        session.add_output(
                            format!("Connected to {}", name),
                            OutputType::Text,
                        );
                    }
                }
            }
            AgentEvent::SessionCreated { session_id, models } => {
                session.id = session_id;
                session.state = SessionState::Idle;
                // Store model info if available
                if let Some(models_state) = models {
                    session.available_models = models_state.available_models;
                    session.current_model_id = Some(models_state.current_model_id);
                }
                session.add_output("Session ready. Press [i] to type.".to_string(), OutputType::Text);
            }
            AgentEvent::Update { update, .. } => {
                match update {
                    SessionUpdate::AgentMessageChunk { content } => {
                        if let acp::protocol::UpdateContent::Text { text } = content {
                            session.append_text(text);
                        }
                    }
                    SessionUpdate::AgentThoughtChunk => {
                        // Silently ignore
                    }
                    SessionUpdate::ToolCall { tool_call_id, title, raw_description, .. } => {
                        let title_str = title
                            .filter(|t| t != "undefined" && !t.is_empty())
                            .unwrap_or_else(|| "Tool".to_string());

                        // Helper to strip all backticks from a string
                        fn strip_backticks(s: &str) -> String {
                            s.replace('`', "")
                        }

                        // Parse title like "Bash(git push)" or "Read(`src/main.rs`)" into name and description
                        let (name, description) = if let Some(paren_pos) = title_str.find('(') {
                            let name = strip_backticks(&title_str[..paren_pos]);
                            let desc = title_str[paren_pos + 1..].trim_end_matches(')').to_string();
                            // Strip backticks from description
                            let desc = strip_backticks(&desc);
                            (name, if desc.is_empty() { None } else { Some(desc) })
                        } else if title_str.starts_with('`') && title_str.ends_with('`') {
                            // Command in backticks like `cd /path && cargo build`
                            let cmd = strip_backticks(&title_str);
                            ("Bash".to_string(), Some(cmd))
                        } else {
                            // Map common tool names and strip any stray backticks
                            let clean_title = strip_backticks(&title_str);
                            let mapped_name = match clean_title.as_str() {
                                "Terminal" => "Bash",
                                "Read File" => "Read",
                                "Write File" => "Write",
                                "Edit File" => "Edit",
                                other => other,
                            };
                            (mapped_name.to_string(), None)
                        };

                        // Use raw_description if no description was parsed from title
                        // This helps with tools like Task that send description in rawInput
                        let description = description.or(raw_description);

                        // Only add spacing for new tool calls, not updates
                        let is_new = !session.has_tool_call(&tool_call_id);
                        if is_new {
                            session.add_output(String::new(), OutputType::Text);
                        }
                        session.add_tool_call(tool_call_id, name, description);
                    }
                    SessionUpdate::ToolCallUpdate { tool_call_id, status } => {
                        // Check if this tool is completing
                        if status == "completed" {
                            // Mark the tool as complete if it's the active one
                            if session.active_tool_call_id.as_ref() == Some(&tool_call_id) {
                                session.complete_active_tool();
                            }
                        } else if !status.trim().is_empty() && status != "in_progress" && status != "pending" {
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
                        session.add_output(format!("[Unknown update: {}]", raw_type.as_deref().unwrap_or("?")), OutputType::Text);
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
                // Check if we should auto-accept (AcceptAll mode)
                if session.permission_mode == PermissionMode::AcceptAll {
                    // Find the first allow_once option
                    if let Some(option) = options.iter().find(|o| o.kind == crate::acp::PermissionKind::AllowOnce) {
                        session.state = SessionState::Prompting;
                        // Auto-scroll to bottom
                        session.scroll_to_bottom();
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
            }
            AgentEvent::PromptComplete { .. } => {
                session.state = SessionState::Idle;
                session.pending_permission = None;
                session.complete_active_tool();
                // Add blank line after response for spacing
                session.add_output(String::new(), OutputType::Text);
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
        // Auto-scroll to bottom on new output
        session.scroll_to_bottom();
    }
    EventResult::None
}
