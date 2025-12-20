#![allow(dead_code)]

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::protocol::*;
use crate::log;
use crate::session::AgentType;

/// Tracked terminal state
struct Terminal {
    output: String,
    exit_code: Option<i32>,
    child: Option<Child>,
}

/// Events from an agent connection
#[derive(Debug)]
pub enum AgentEvent {
    Initialized {
        agent_info: Option<AgentInfo>,
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
    PromptComplete {
        stop_reason: StopReason,
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
        let mut child = Command::new(agent_type.command())
            .args(agent_type.args())
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

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
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut terminals: HashMap<String, Terminal> = HashMap::new();
            let mut terminal_counter: u64 = 0;

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
                            if let Ok(init) = serde_json::from_value::<InitializeResult>(result.clone()) {
                                let _ = event_tx_clone
                                    .send(AgentEvent::Initialized {
                                        agent_info: init.agent_info,
                                    })
                                    .await;
                            } else if let Ok(session) = serde_json::from_value::<NewSessionResult>(result.clone()) {
                                let _ = event_tx_clone
                                    .send(AgentEvent::SessionCreated {
                                        session_id: session.session_id,
                                        models: session.models,
                                    })
                                    .await;
                            } else if let Ok(prompt) = serde_json::from_value::<PromptResult>(result.clone()) {
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
                        if method == "session/update" {
                            if let Some(params) = params {
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
                                        let update_type = params.get("update")
                                            .and_then(|u| u.get("sessionUpdate"))
                                            .and_then(|s| s.as_str())
                                            .unwrap_or("unknown");
                                        let _ = event_tx_clone
                                            .send(AgentEvent::Error {
                                                message: format!("Failed to parse session update '{}': {}", update_type, e),
                                            })
                                            .await;
                                    }
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
                                                message: format!("Permission parse error: {} - params: {:?}", e, params),
                                            })
                                            .await;
                                    }
                                }
                            }
                        } else if method == "fs/read_text_file" {
                            // Handle file read request
                            if let Some(params) = params {
                                match serde_json::from_value::<FsReadTextFileParams>(params.clone()) {
                                    Ok(fs_params) => {
                                        // Read the file
                                        let result = match tokio::fs::read_to_string(&fs_params.path).await {
                                            Ok(mut content) => {
                                                // Apply line/limit if specified
                                                if fs_params.line.is_some() || fs_params.limit.is_some() {
                                                    let lines: Vec<&str> = content.lines().collect();
                                                    let start = fs_params.line.unwrap_or(1).saturating_sub(1) as usize;
                                                    let limit = fs_params.limit.unwrap_or(u32::MAX) as usize;
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
                                        let json = serde_json::to_string(&result).unwrap_or_default();
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
                                        let json = serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "fs/write_text_file" {
                            // Handle file write request
                            if let Some(params) = params {
                                match serde_json::from_value::<FsWriteTextFileParams>(params.clone()) {
                                    Ok(fs_params) => {
                                        let result = match tokio::fs::write(&fs_params.path, &fs_params.content).await {
                                            Ok(()) => {
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
                                        let json = serde_json::to_string(&result).unwrap_or_default();
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
                                        let json = serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/create" {
                            // Handle terminal create request
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalCreateParams>(params.clone()) {
                                    Ok(term_params) => {
                                        // Build the full command with args
                                        let full_command = if term_params.args.is_empty() {
                                            term_params.command.clone()
                                        } else {
                                            format!("{} {}", term_params.command, term_params.args.join(" "))
                                        };

                                        // Execute through shell to support pipes, redirects, etc.
                                        let mut cmd = std::process::Command::new("sh");
                                        cmd.arg("-c");
                                        cmd.arg(&full_command);
                                        if let Some(cwd) = &term_params.cwd {
                                            cmd.current_dir(cwd);
                                        }
                                        for env_var in &term_params.env {
                                            cmd.env(&env_var.name, &env_var.value);
                                        }
                                        cmd.stdout(Stdio::piped());
                                        cmd.stderr(Stdio::piped());

                                        match cmd.output() {
                                            Ok(output) => {
                                                terminal_counter += 1;
                                                let terminal_id = format!("term_{}", terminal_counter);

                                                let mut out = String::from_utf8_lossy(&output.stdout).to_string();
                                                out.push_str(&String::from_utf8_lossy(&output.stderr));

                                                // Apply output byte limit if specified
                                                if let Some(limit) = term_params.output_byte_limit {
                                                    if out.len() > limit {
                                                        out = out[out.len() - limit..].to_string();
                                                    }
                                                }

                                                let exit_code = output.status.code();
                                                terminals.insert(terminal_id.clone(), Terminal {
                                                    output: out,
                                                    exit_code,
                                                    child: None,
                                                });

                                                let result = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "result": {
                                                        "_meta": null,
                                                        "terminalId": terminal_id
                                                    }
                                                });
                                                let json = serde_json::to_string(&result).unwrap_or_default();
                                                let _ = response_tx.send(json).await;
                                            }
                                            Err(e) => {
                                                let error_resp = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": id,
                                                    "error": {
                                                        "code": -32000,
                                                        "message": format!("Failed to execute command: {}", e)
                                                    }
                                                });
                                                let json = serde_json::to_string(&error_resp).unwrap_or_default();
                                                let _ = response_tx.send(json).await;
                                            }
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
                                        let json = serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/output" {
                            // Handle terminal output request
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalOutputParams>(params.clone()) {
                                    Ok(term_params) => {
                                        if let Some(terminal) = terminals.get(&term_params.terminal_id) {
                                            let result = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": {
                                                    "_meta": null,
                                                    "output": terminal.output.clone(),
                                                    "exitCode": terminal.exit_code,
                                                }
                                            });
                                            let json = serde_json::to_string(&result).unwrap_or_default();
                                            let _ = response_tx.send(json).await;
                                        } else {
                                            let error_resp = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "error": {
                                                    "code": -32000,
                                                    "message": "Terminal not found"
                                                }
                                            });
                                            let json = serde_json::to_string(&error_resp).unwrap_or_default();
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
                                        let json = serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/wait_for_exit" {
                            // Handle terminal wait request - since we run sync, it's already done
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalWaitParams>(params.clone()) {
                                    Ok(term_params) => {
                                        if let Some(terminal) = terminals.get(&term_params.terminal_id) {
                                            let result = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": {
                                                    "_meta": null,
                                                    "exitCode": terminal.exit_code,
                                                    "timedOut": false,
                                                }
                                            });
                                            let json = serde_json::to_string(&result).unwrap_or_default();
                                            let _ = response_tx.send(json).await;
                                        } else {
                                            let error_resp = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "error": {
                                                    "code": -32000,
                                                    "message": "Terminal not found"
                                                }
                                            });
                                            let json = serde_json::to_string(&error_resp).unwrap_or_default();
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
                                        let json = serde_json::to_string(&error_resp).unwrap_or_default();
                                        let _ = response_tx.send(json).await;
                                    }
                                }
                            }
                        } else if method == "terminal/kill" || method == "terminal/release" {
                            // Handle terminal kill/release - remove from tracking
                            if let Some(params) = params {
                                match serde_json::from_value::<TerminalKillParams>(params.clone()) {
                                    Ok(term_params) => {
                                        terminals.remove(&term_params.terminal_id);
                                        let result = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": {}
                                        });
                                        let json = serde_json::to_string(&result).unwrap_or_default();
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
                                        let json = serde_json::to_string(&error_resp).unwrap_or_default();
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
    pub async fn new_session(&mut self, cwd: &str) -> Result<()> {
        let params = NewSessionParams {
            cwd: cwd.to_string(),
            mcp_servers: vec![],
        };

        let request = JsonRpcRequest::new(
            self.next_id(),
            "session/new",
            Some(serde_json::to_value(params)?),
        );
        self.send(request).await
    }

    /// Load an existing session
    pub async fn load_session(&mut self, session_id: &str, cwd: &str) -> Result<()> {
        let params = LoadSessionParams {
            session_id: session_id.to_string(),
            cwd: cwd.to_string(),
            mcp_servers: vec![],
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
        let request = JsonRpcRequest::new(
            id,
            "session/prompt",
            Some(serde_json::to_value(params)?),
        );
        self.send(request).await
    }

    /// Send a prompt with arbitrary content blocks (text, images, etc.)
    pub async fn prompt_with_content(&mut self, session_id: &str, content: Vec<ContentBlock>) -> Result<()> {
        let params = PromptParams {
            session_id: session_id.to_string(),
            prompt: content,
        };

        let id = self.next_id();
        self.current_prompt_id = Some(id);
        let request = JsonRpcRequest::new(
            id,
            "session/prompt",
            Some(serde_json::to_value(params)?),
        );
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
    pub async fn respond_permission(&mut self, request_id: u64, option_id: Option<PermissionOptionId>) -> Result<()> {
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
