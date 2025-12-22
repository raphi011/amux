#![allow(dead_code)]

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc};

use serde_json::Value;

use super::protocol::{AskUserOption, AskUserRequestParams, AskUserResponse, *};
use crate::log;
use crate::session::AgentType;

/// Tracked terminal state
struct Terminal {
    output: String,
    exit_code: Option<i32>,
    child: Option<Child>,
}

/// Shared state for terminals that can be accessed from multiple tasks
type Terminals = Arc<Mutex<HashMap<String, Terminal>>>;
type TerminalCounter = Arc<Mutex<u64>>;

/// Events from an agent connection
#[derive(Debug)]
pub enum AgentEvent {
    Initialized {
        agent_info: Option<AgentInfo>,
        agent_capabilities: Option<Value>,
    },
    SessionCreated {
        session_id: String,
        models: Option<ModelsState>,
    },
    Update {
        session_id: String,
        update: SessionUpdate,
    },
    PermissionRequest {
        request_id: u64,
        session_id: String,
        tool_call_id: String,
        title: Option<String>,
        options: Vec<PermissionOptionInfo>,
    },
    AskUserRequest {
        request_id: u64,
        session_id: String,
        question: String,
        options: Vec<AskUserOption>,
        multi_select: bool,
    },
    PromptComplete {
        stop_reason: StopReason,
    },
    FileWritten {
        session_id: String,
        path: String,
        diff: String,
    },
    Error {
        message: String,
    },
    Disconnected,
}

/// Connection to an ACP agent
pub struct AgentConnection {
    child: Child,
    request_id: u64,
    tx: mpsc::Sender<String>,
    /// Track the current prompt request ID for cancellation
    current_prompt_id: Option<u64>,
}

impl AgentConnection {
    /// Spawn a new agent process
    pub async fn spawn(
        agent_type: AgentType,
        cwd: &Path,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<Self> {
        let mut cmd = Command::new(agent_type.command());
        cmd.args(agent_type.args())
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        // For Claude Code ACP adapter, pass custom Claude executable if available
        if matches!(agent_type, AgentType::ClaudeCode)
            && let Ok(claude_path) = std::env::var("CLAUDE_CODE_EXECUTABLE")
        {
            cmd.env("CLAUDE_CODE_EXECUTABLE", claude_path);
        }

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("No stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("No stdout"))?;

        // Channel for sending messages to the write task
        let (tx, mut rx) = mpsc::channel::<String>(32);

        // Spawn write task
        let mut stdin = stdin;
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                log::log_outgoing(&msg);
                if stdin.write_all(msg.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        // Spawn read task
        let event_tx_clone = event_tx.clone();
        let response_tx = tx.clone(); // For sending responses to fs/terminal requests

        // Shared state for terminals - allows concurrent command execution
        let terminals: Terminals = Arc::new(Mutex::new(HashMap::new()));
        let terminal_counter: TerminalCounter = Arc::new(Mutex::new(0));

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }

                log::log_incoming(&line);

                match IncomingMessage::parse(&line) {
                    Ok(IncomingMessage::Response(resp)) => {
                        // Handle response based on result
                        if let Some(error) = resp.error {
                            let _ = event_tx_clone
                                .send(AgentEvent::Error {
                                    message: error.message,
                                })
                                .await;
                        } else if let Some(result) = resp.result {
                            // Try to parse as different result types
                            if let Ok(init) =
                                serde_json::from_value::<InitializeResult>(result.clone())
                            {
                                let _ = event_tx_clone
                                    .send(AgentEvent::Initialized {
                                        agent_info: init.agent_info,
                                        agent_capabilities: init.agent_capabilities,
                                    })
                                    .await;
                            } else if let Ok(session) =
                                serde_json::from_value::<NewSessionResult>(result.clone())
                            {
                                let _ = event_tx_clone
                                    .send(AgentEvent::SessionCreated {
                                        session_id: session.session_id,
                                        models: session.models,
                                    })
                                    .await;
                            } else if let Ok(prompt) =
                                serde_json::from_value::<PromptResult>(result.clone())
                            {
                                let _ = event_tx_clone
                                    .send(AgentEvent::PromptComplete {
                                        stop_reason: prompt.stop_reason,
                                    })
                                    .await;
                            } else if result.is_null() {
                                // session/load returns null on success
                                // The session ID should already be set, just emit SessionCreated
                                // with empty string (caller should handle this)
                                let _ = event_tx_clone
                                    .send(AgentEvent::SessionCreated {
                                        session_id: String::new(),
                                        models: None,
                                    })
                                    .await;
                            }
                        } else {
                            // Response with no result and no error (shouldn't happen, but handle gracefully)
                            log::log_event("Response with no result and no error");
                        }
                    }
                    Ok(IncomingMessage::Notification { method, params }) => {
                        if method == "session/update"
                            && let Some(params) = params
                        {
                            // Log tool calls containing "grep" to dedicated tools log
                            if let Some(update) = params.get("update")
                                && update.get("sessionUpdate").and_then(|s| s.as_str())
                                    == Some("tool_call")
                            {
                                let tool_name = update
                                    .get("title")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("unknown");
                                // Check if the entire update JSON contains "grep" (case-insensitive)
                                let update_str = update.to_string().to_lowercase();
                                if update_str.contains("grep") {
                                    log::log_tool_json(tool_name, update);
                                }
                            }

                            match serde_json::from_value::<SessionUpdateParams>(params.clone()) {
                                Ok(update_params) => {
                                    let _ = event_tx_clone
                                        .send(AgentEvent::Update {
                                            session_id: update_params.session_id,
                                            update: update_params.update,
                                        })
                                        .await;
                                }
                                Err(e) => {
                                    // Log parse error with raw JSON for debugging
                                    let update_type = params
                                        .get("update")
                                        .and_then(|u| u.get("sessionUpdate"))
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("unknown");
                                    let _ = event_tx_clone
                                        .send(AgentEvent::Error {
                                            message: format!(
                                                "Failed to parse session update '{}': {}",
                                                update_type, e
                                            ),
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                    Ok(IncomingMessage::Request { id, method, params }) => {
                        log::log_event(&format!("Request: {} (id={})", method, id));
                        if method == "session/request_permission" {
                            if let Some(params) = params {
                                match serde_json::from_value::<PermissionRequest>(params.clone()) {
                                    Ok(perm_req) => {
                                        log::log_event(&format!(
                                            "Permission request: {} options, title={:?}",
                                            perm_req.options.len(),
                                            perm_req.tool_call.title
                                        ));
                                        let _ = event_tx_clone
                                            .send(AgentEvent::PermissionRequest {
                                                request_id: id,
                                                session_id: perm_req.session_id,
                                                tool_call_id: perm_req.tool_call.tool_call_id,
                                                title: perm_req.tool_call.title,
                                                options: perm_req.options,
                                            })
                                            .await;
                                    }
                                    Err(e) => {
                                        let _ = event_tx_clone
                                            .send(AgentEvent::Error {
                                                message: format!(
                                                    "Permission parse error: {} - params: {:?}",
                                                    e, params
                                                ),
                                            })
                                            .await;
                                    }
                                }
                            }
                        } else if method == "session/ask_user" {
                            // Handle ask_user request (Claude Code extension)
                            if let Some(params) = params {
                                match serde_json::from_value::<AskUserRequestParams>(params.clone())
                                {
                                    Ok(ask_req) => {
                                        log::log_event(&format!(
                                            "Ask user request: question={:?}, options={}",
                                            ask_req.question,
                                            ask_req.options.len()
                                        ));
                                        let _ = event_tx_clone
                                            .send(AgentEvent::AskUserRequest {
                                                request_id: id,
                                                session_id: ask_req.session_id,
                                                question: ask_req.question,
                                                options: ask_req.options,
                                                multi_select: ask_req.multi_select,
                                            })
                                            .await;
                                    }
                                    Err(e) => {
                                        let _ = event_tx_clone
                                            .send(AgentEvent::Error {
                                                message: format!(
                                                    "Ask user parse error: {} - params: {:?}",
                                                    e, params
                                                ),
                                            })
                                            .await;
                                    }
                                }
                            }
                        } else if method == "fs/read_text_file" {
                            // Handle file read request
                            if let Some(params) = params {
                                match serde_json::from_value::<FsReadTextFileParams>(params.clone())
                                {
                                    Ok(fs_params) => {
                                        // Read the file
                                        let result = match tokio::fs::read_to_string(
                                            &fs_params.path,
                                        )
                                        .await
                                        {
                                            Ok(mut content) => {
                                                // Apply line/limit if specified
                                                if fs_params.line.is_some()
                                                    || fs_params.limit.is_some()
                                                {
                                                    let lines: Vec<&str> =
                                                        content.lines().collect();
                                                    let start = fs_params
                                                        .line
                                                        .unwrap_or(1)
                                                        .saturating_sub(1)
                                                        as usize;
                                                    let limit = fs_params.limit.unwrap_or(u32::MAX)
                                                        as usize;
                                                    let end = (start + limit).min(lines.len());
                                                    content = lines[start..end].join("\n");
                                                }
                                                serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "result": {
                                                        "_meta": null,
                                                        "content": content
                                                    }
                                                })
                                            }
                                            Err(e) => {
                                                serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "error": {
                                                        "code": -32000,
                                                        "message": format!("Failed to read file: {}", e)
                                                    }
                                                })
                                            }
                                        };
                                        let json =
                                            serde_json::to_string(&result).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                    Err(e) => {
                                        // Send error response
                                        let error_resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": {
                                                "code": -32602,
                                                "message": format!("Invalid params: {}", e)
                                            }
                                        });
                                        let json =
                                            serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "fs/write_text_file" {
                            // Handle file write request
                            if let Some(params) = params {
                                match serde_json::from_value::<FsWriteTextFileParams>(
                                    params.clone(),
                                ) {
                                    Ok(fs_params) => {
                                        // Read old content for diff (if file exists)
                                        let old_content =
                                            tokio::fs::read_to_string(&fs_params.path).await.ok();

                                        let result = match tokio::fs::write(
                                            &fs_params.path,
                                            &fs_params.content,
                                        )
                                        .await
                                        {
                                            Ok(()) => {
                                                // Generate and send diff
                                                let diff = generate_diff(
                                                    old_content.as_deref().unwrap_or(""),
                                                    &fs_params.content,
                                                    &fs_params.path,
                                                );
                                                let _ = event_tx_clone
                                                    .send(AgentEvent::FileWritten {
                                                        session_id: fs_params.session_id.clone(),
                                                        path: fs_params.path.clone(),
                                                        diff,
                                                    })
                                                    .await;

                                                serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "result": FsWriteTextFileResult { success: true }
                                                })
                                            }
                                            Err(e) => {
                                                serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "error": {
                                                        "code": -32000,
                                                        "message": format!("Failed to write file: {}", e)
                                                    }
                                                })
                                            }
                                        };
                                        let json =
                                            serde_json::to_string(&result).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                    Err(e) => {
                                        let error_resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": {
                                                "code": -32602,
                                                "message": format!("Invalid params: {}", e)
                                            }
                                        });
                                        let json =
                                            serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/create" {
                            // Handle terminal create request
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalCreateParams>(params.clone())
                                {
                                    Ok(term_params) => {
                                        // Assign terminal ID immediately
                                        let terminal_id = {
                                            let mut counter = terminal_counter.lock().await;
                                            *counter += 1;
                                            format!("term_{}", *counter)
                                        };

                                        // Insert placeholder terminal (command running)
                                        {
                                            let mut terms = terminals.lock().await;
                                            terms.insert(
                                                terminal_id.clone(),
                                                Terminal {
                                                    output: String::new(),
                                                    exit_code: None,
                                                    child: None,
                                                },
                                            );
                                        }

                                        // Respond immediately with terminal ID
                                        let result = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": {
                                                "_meta": null,
                                                "terminalId": terminal_id.clone()
                                            }
                                        });
                                        let json =
                                            serde_json::to_string(&result).unwrap_or_default();
                                        let _ = response_tx.send(json).await;

                                        // Build the full command with args
                                        let full_command = if term_params.args.is_empty() {
                                            term_params.command.clone()
                                        } else {
                                            format!(
                                                "{} {}",
                                                term_params.command,
                                                term_params.args.join(" ")
                                            )
                                        };

                                        let cwd = term_params.cwd.clone();
                                        let env_vars: Vec<_> = term_params
                                            .env
                                            .iter()
                                            .map(|e| (e.name.clone(), e.value.clone()))
                                            .collect();
                                        let output_limit = term_params.output_byte_limit;

                                        // Spawn command execution in background - doesn't block message loop
                                        let terminals_clone = Arc::clone(&terminals);
                                        let terminal_id_clone = terminal_id.clone();
                                        tokio::spawn(async move {
                                            // Use tokio::process::Command directly (async native)
                                            let mut cmd = Command::new("sh");
                                            cmd.arg("-c");
                                            cmd.arg(&full_command);
                                            if let Some(cwd) = &cwd {
                                                cmd.current_dir(cwd);
                                            }
                                            for (name, value) in &env_vars {
                                                cmd.env(name, value);
                                            }
                                            cmd.stdout(Stdio::piped());
                                            cmd.stderr(Stdio::piped());

                                            let result = cmd.output().await;

                                            // Update terminal with results
                                            let mut terms = terminals_clone.lock().await;
                                            if let Some(terminal) =
                                                terms.get_mut(&terminal_id_clone)
                                            {
                                                match result {
                                                    Ok(output) => {
                                                        let mut out =
                                                            String::from_utf8_lossy(&output.stdout)
                                                                .to_string();
                                                        out.push_str(&String::from_utf8_lossy(
                                                            &output.stderr,
                                                        ));

                                                        // Apply output byte limit if specified
                                                        if let Some(limit) = output_limit
                                                            && out.len() > limit
                                                        {
                                                            out = out[out.len() - limit..]
                                                                .to_string();
                                                        }

                                                        terminal.output = out;
                                                        terminal.exit_code = output.status.code();
                                                    }
                                                    Err(e) => {
                                                        terminal.output = format!("Error: {}", e);
                                                        terminal.exit_code = Some(-1);
                                                    }
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        let error_resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": {
                                                "code": -32602,
                                                "message": format!("Invalid params: {}", e)
                                            }
                                        });
                                        let json =
                                            serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/output" {
                            // Handle terminal output request
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalOutputParams>(params.clone())
                                {
                                    Ok(term_params) => {
                                        let terms = terminals.lock().await;
                                        if let Some(terminal) = terms.get(&term_params.terminal_id)
                                        {
                                            let result = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": {
                                                    "_meta": null,
                                                    "output": terminal.output.clone(),
                                                    "exitCode": terminal.exit_code,
                                                }
                                            });
                                            let json =
                                                serde_json::to_string(&result).unwrap_or_default();
                                            drop(terms); // Release lock before await
                                            let _ = response_tx.send(json).await;
                                        } else {
                                            drop(terms); // Release lock before await
                                            let error_resp = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "error": {
                                                    "code": -32000,
                                                    "message": "Terminal not found"
                                                }
                                            });
                                            let json = serde_json::to_string(&error_resp)
                                                .unwrap_or_default();
                                            let _ = response_tx.send(json).await;
                                        }
                                    }
                                    Err(e) => {
                                        let error_resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": {
                                                "code": -32602,
                                                "message": format!("Invalid params: {}", e)
                                            }
                                        });
                                        let json =
                                            serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/wait_for_exit" {
                            // Handle terminal wait request
                            // Poll until exit_code is set (command completed)
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalWaitParams>(params.clone()) {
                                    Ok(term_params) => {
                                        let timeout_ms = term_params.timeout_ms.unwrap_or(30000);
                                        let start = std::time::Instant::now();
                                        let terminal_id = term_params.terminal_id.clone();
                                        let terminals_clone = Arc::clone(&terminals);

                                        // Poll for completion
                                        loop {
                                            let terms = terminals_clone.lock().await;
                                            if let Some(terminal) = terms.get(&terminal_id) {
                                                if terminal.exit_code.is_some() {
                                                    // Command completed
                                                    let result = serde_json::json!({
                                                        "jsonrpc": "2.0",
                                                        "id": id,
                                                        "result": {
                                                            "_meta": null,
                                                            "exitCode": terminal.exit_code,
                                                            "timedOut": false,
                                                        }
                                                    });
                                                    let json = serde_json::to_string(&result)
                                                        .unwrap_or_default();
                                                    drop(terms);
                                                    let _ = response_tx.send(json).await;
                                                    break;
                                                }
                                            } else {
                                                drop(terms);
                                                let error_resp = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "error": {
                                                        "code": -32000,
                                                        "message": "Terminal not found"
                                                    }
                                                });
                                                let json = serde_json::to_string(&error_resp)
                                                    .unwrap_or_default();
                                                let _ = response_tx.send(json).await;
                                                break;
                                            }
                                            drop(terms);

                                            // Check timeout
                                            if start.elapsed().as_millis() as u64 > timeout_ms {
                                                let result = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "result": {
                                                        "_meta": null,
                                                        "exitCode": null,
                                                        "timedOut": true,
                                                    }
                                                });
                                                let json = serde_json::to_string(&result)
                                                    .unwrap_or_default();
                                                let _ = response_tx.send(json).await;
                                                break;
                                            }

                                            // Wait a bit before polling again
                                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                                50,
                                            ))
                                            .await;
                                        }
                                    }
                                    Err(e) => {
                                        let error_resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": {
                                                "code": -32602,
                                                "message": format!("Invalid params: {}", e)
                                            }
                                        });
                                        let json =
                                            serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/kill" || method == "terminal/release" {
                            // Handle terminal kill/release - remove from tracking
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalKillParams>(params.clone()) {
                                    Ok(term_params) => {
                                        let mut terms = terminals.lock().await;
                                        terms.remove(&term_params.terminal_id);
                                        drop(terms);
                                        let result = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": {}
                                        });
                                        let json =
                                            serde_json::to_string(&result).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                    Err(e) => {
                                        let error_resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": {
                                                "code": -32602,
                                                "message": format!("Invalid params: {}", e)
                                            }
                                        });
                                        let json =
                                            serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else {
                            // Unknown request - log it
                            let _ = event_tx_clone
                                .send(AgentEvent::Error {
                                    message: format!("Unknown request: {} (id={})", method, id),
                                })
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx_clone
                            .send(AgentEvent::Error {
                                message: format!("Parse error: {}", e),
                            })
                            .await;
                    }
                }
            }

            let _ = event_tx_clone.send(AgentEvent::Disconnected).await;
        });

        Ok(Self {
            child,
            request_id: 0,
            tx,
            current_prompt_id: None,
        })
    }

    fn next_id(&mut self) -> u64 {
        self.request_id += 1;
        self.request_id
    }

    async fn send(&mut self, request: JsonRpcRequest) -> Result<()> {
        let json = serde_json::to_string(&request)?;
        self.tx.send(json).await?;
        Ok(())
    }

    /// Send initialize request
    pub async fn initialize(&mut self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: 1,
            client_capabilities: ClientCapabilities {
                fs: Some(FsCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                }),
                terminal: Some(true),
            },
            client_info: ClientInfo {
                name: "amux".to_string(),
                title: "amux".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let request = JsonRpcRequest::new(
            self.next_id(),
            "initialize",
            Some(serde_json::to_value(params)?),
        );
        self.send(request).await
    }

    /// Create a new session
    pub async fn new_session(&mut self, cwd: &str, mcp_servers: Vec<McpServer>) -> Result<()> {
        let params = NewSessionParams {
            cwd: cwd.to_string(),
            mcp_servers,
        };

        let request = JsonRpcRequest::new(
            self.next_id(),
            "session/new",
            Some(serde_json::to_value(params)?),
        );
        self.send(request).await
    }

    /// Load an existing session
    pub async fn load_session(
        &mut self,
        session_id: &str,
        cwd: &str,
        mcp_servers: Vec<McpServer>,
    ) -> Result<()> {
        let params = LoadSessionParams {
            session_id: session_id.to_string(),
            cwd: cwd.to_string(),
            mcp_servers,
        };

        let request = JsonRpcRequest::new(
            self.next_id(),
            "session/load",
            Some(serde_json::to_value(params)?),
        );
        self.send(request).await
    }

    /// Send a prompt
    pub async fn prompt(&mut self, session_id: &str, text: &str) -> Result<()> {
        let params = PromptParams {
            session_id: session_id.to_string(),
            prompt: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        };

        let id = self.next_id();
        self.current_prompt_id = Some(id);
        let request =
            JsonRpcRequest::new(id, "session/prompt", Some(serde_json::to_value(params)?));
        self.send(request).await
    }

    /// Send a prompt with arbitrary content blocks (text, images, etc.)
    pub async fn prompt_with_content(
        &mut self,
        session_id: &str,
        content: Vec<ContentBlock>,
    ) -> Result<()> {
        let params = PromptParams {
            session_id: session_id.to_string(),
            prompt: content,
        };

        let id = self.next_id();
        self.current_prompt_id = Some(id);
        let request =
            JsonRpcRequest::new(id, "session/prompt", Some(serde_json::to_value(params)?));
        self.send(request).await
    }

    /// Cancel the current prompt if one is in progress
    pub async fn cancel_prompt(&mut self) -> Result<()> {
        if let Some(prompt_id) = self.current_prompt_id.take() {
            // Send $/cancel_request notification
            let notification = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "$/cancel_request",
                "params": {
                    "id": prompt_id
                }
            });
            let json = serde_json::to_string(&notification)?;
            self.tx.send(json).await?;
        }
        Ok(())
    }

    /// Respond to a permission request
    pub async fn respond_permission(
        &mut self,
        request_id: u64,
        option_id: Option<PermissionOptionId>,
    ) -> Result<()> {
        let result = match option_id {
            Some(id) => RequestPermissionResponse::selected(id),
            None => RequestPermissionResponse::cancelled(),
        };

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": result
        });

        let json = serde_json::to_string(&response)?;
        self.tx.send(json).await?;
        Ok(())
    }

    /// Respond to an ask_user request
    pub async fn respond_ask_user(
        &mut self,
        request_id: u64,
        response: AskUserResponse,
    ) -> Result<()> {
        let rpc_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": response
        });

        let json = serde_json::to_string(&rpc_response)?;
        self.tx.send(json).await?;
        Ok(())
    }

    /// Set the model for a session
    pub async fn set_model(&mut self, session_id: &str, model_id: &str) -> Result<()> {
        let params = SetModelParams {
            session_id: session_id.to_string(),
            model_id: model_id.to_string(),
        };

        let request = JsonRpcRequest::new(
            self.next_id(),
            "session/set_model",
            Some(serde_json::to_value(params)?),
        );
        self.send(request).await
    }

    /// Kill the agent process
    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}

/// Generate a unified diff between old and new content with line numbers
fn generate_diff(old: &str, new: &str, _path: &str) -> String {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(old, new);

    // Check if there are any changes
    if diff.iter_all_changes().all(|c| c.tag() == ChangeTag::Equal) {
        return "No changes".to_string();
    }

    let mut result = String::new();

    // Skip --- and +++ header lines since path is already shown in tool output

    // Generate unified diff with context (3 lines)
    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        // Hunk header
        result.push_str(&format!("{}\n", hunk.header()));

        // Hunk content with line numbers
        for change in hunk.iter_changes() {
            let (sign, old_line, new_line) = match change.tag() {
                ChangeTag::Delete => ('-', change.old_index().map(|i| i + 1), None),
                ChangeTag::Insert => ('+', None, change.new_index().map(|i| i + 1)),
                ChangeTag::Equal => (
                    ' ',
                    change.old_index().map(|i| i + 1),
                    change.new_index().map(|i| i + 1),
                ),
            };

            // Format line numbers: "old new" or spaces for missing
            let line_info = match (old_line, new_line) {
                (Some(o), Some(n)) => format!("{:>4} {:>4}", o, n),
                (Some(o), None) => format!("{:>4}     ", o),
                (None, Some(n)) => format!("     {:>4}", n),
                (None, None) => "         ".to_string(),
            };

            // Format: "sign old_line new_line | content"
            result.push_str(&format!("{}{} {}", sign, line_info, change));
        }
    }

    result
}
