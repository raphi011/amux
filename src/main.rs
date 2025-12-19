mod acp;
mod app;
mod log;
mod session;
mod tui;

use anyhow::Result;
use crossterm::{
    event::{Event, KeyCode, KeyEventKind, KeyModifiers, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::prelude::*;
use std::collections::HashMap;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;

use acp::{AgentConnection, AgentEvent, SessionUpdate, PermissionOptionId};
use app::{App, FolderEntry, InputMode};
use session::{AgentType, OutputType, SessionState, PendingPermission};

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

/// Command to send to an agent
enum AgentCommand {
    Prompt { session_id: String, text: String },
    PermissionResponse { request_id: u64, option_id: Option<PermissionOptionId> },
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

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Run the app
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
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
            // Keyboard events
            maybe_event = event_stream.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_event {
                    if key.kind == KeyEventKind::Press {
                        match app.input_mode {
                            InputMode::Normal => {
                                // Check if there's a pending permission request
                                let has_permission = app.sessions.selected_session()
                                    .map(|s| s.pending_permission.is_some())
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
                                } else {
                                    // Normal mode keys
                                    match key.code {
                                        KeyCode::Char('q') => return Ok(()),
                                        KeyCode::Esc => {
                                            // Escape does nothing in normal mode (focus stays on list)
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
                                            // Open folder picker
                                            let cwd = std::env::current_dir().unwrap_or_default();
                                            app.open_folder_picker(cwd.clone());
                                            let entries = scan_folder_entries(&cwd).await;
                                            app.set_folder_entries(entries);
                                        }
                                        KeyCode::Char('x') => {
                                            let idx = app.sessions.selected_index();
                                            agent_commands.remove(&idx);
                                            app.kill_selected_session();
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
                                                    app.open_agent_picker(path);
                                                }
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
                                            app.close_agent_picker();
                                            spawn_agent_in_dir(app, &agent_tx, &mut agent_commands, agent_type, cwd).await?;
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
                            InputMode::Insert => {
                                match key.code {
                                    KeyCode::Esc => {
                                        app.exit_insert_mode();
                                    }
                                    KeyCode::Enter => {
                                        let text = app.take_input();
                                        if !text.is_empty() {
                                            send_prompt(app, &agent_commands, &text).await;
                                        }
                                        app.exit_insert_mode();
                                    }
                                    KeyCode::Backspace => app.input_backspace(),
                                    KeyCode::Delete => app.input_delete(),
                                    KeyCode::Left => app.input_left(),
                                    KeyCode::Right => app.input_right(),
                                    KeyCode::Char(c) => app.input_char(c),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            // Agent events
            Some((session_idx, event)) = agent_rx.recv() => {
                handle_agent_event(app, session_idx, event);
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
) -> Result<()> {
    let session_idx = app.spawn_session(agent_type, cwd.clone());

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
                        AgentCommand::PermissionResponse { request_id, option_id } => {
                            if let Err(e) = conn.respond_permission(request_id, option_id).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Permission response failed: {}", e),
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
    let session_idx = app.spawn_session(agent_type, cwd.clone());

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
                        AgentCommand::PermissionResponse { request_id, option_id } => {
                            if let Err(e) = conn.respond_permission(request_id, option_id).await {
                                let _ = event_tx.send(AgentEvent::Error {
                                    message: format!("Permission response failed: {}", e),
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

    if let Some(session) = app.sessions.sessions_mut().get_mut(session_idx) {
        // Add user message to output
        session.add_output(format!("> {}", text), OutputType::UserInput);
        session.state = SessionState::Prompting;

        // Send command to agent
        if let Some(cmd_tx) = agent_commands.get(&session_idx) {
            let _ = cmd_tx.send(AgentCommand::Prompt {
                session_id: session.id.clone(),
                text: text.to_string(),
            }).await;
        }
    }
}

fn handle_agent_event(app: &mut App, session_idx: usize, event: AgentEvent) {
    let viewport_height = app.viewport_height;
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
            AgentEvent::SessionCreated { session_id } => {
                session.id = session_id;
                session.state = SessionState::Idle;
                session.add_output("Session ready. Press [i] to type.".to_string(), OutputType::Text);
            }
            AgentEvent::Update { update, .. } => {
                match update {
                    SessionUpdate::AgentMessageChunk { content } => {
                        if let acp::protocol::UpdateContent::Text { text } = content {
                            session.append_text(text);
                        }
                    }
                    SessionUpdate::ToolCall { title, .. } => {
                        let name = title
                            .filter(|t| t != "undefined" && !t.is_empty())
                            .unwrap_or_else(|| "tool".to_string());
                        // Add spacing before tool call
                        session.add_output(String::new(), OutputType::Text);
                        session.add_output(format!("[Tool: {}]", name), OutputType::ToolCall);
                    }
                    SessionUpdate::ToolCallUpdate { status, .. } => {
                        // Only show non-empty status updates
                        if !status.trim().is_empty() {
                            session.add_output(format!("  → {}", status), OutputType::ToolResult);
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
                session.state = SessionState::AwaitingPermission;
                session.pending_permission = Some(PendingPermission {
                    request_id,
                    tool_call_id,
                    title,
                    options,
                    selected: 0,
                });
            }
            AgentEvent::PromptComplete { .. } => {
                session.state = SessionState::Idle;
                session.pending_permission = None;
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
        session.scroll_to_bottom(viewport_height);
    }
}
