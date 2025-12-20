//! ACP Protocol types
//!
//! This module re-exports types from the `agent-client-protocol` crate and defines
//! additional types needed for our implementation.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

// Re-export commonly used types from agent-client-protocol
pub use agent_client_protocol::{
    // Permission types - for reference/compatibility
    PermissionOptionId,
};

// ============================================================================
// JSON-RPC base types (not provided by ACP crate)
// ============================================================================

/// JSON-RPC request
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        }
    }
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

// ============================================================================
// Initialize types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub fs: Option<FsCapabilities>,
    pub terminal: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCapabilities {
    pub read_text_file: bool,
    pub write_text_file: bool,
}

#[derive(Debug, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub agent_capabilities: Option<Value>,
    pub agent_info: Option<AgentInfo>,
}

#[derive(Debug, Deserialize)]
pub struct AgentInfo {
    pub name: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
}

// ============================================================================
// Session types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionParams {
    pub cwd: String,
    pub mcp_servers: Vec<McpServer>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadSessionParams {
    pub session_id: String,
    pub cwd: String,
    pub mcp_servers: Vec<McpServer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionResult {
    pub session_id: String,
}

// ============================================================================
// Prompt types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptParams {
    pub session_id: String,
    pub prompt: Vec<ContentBlock>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Image {
        #[serde(rename = "mimeType")]
        mime_type: String,
        data: String, // base64 encoded
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptResult {
    pub stop_reason: StopReason,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    Cancelled,
    Refusal,
    #[serde(other)]
    Unknown,
}

// ============================================================================
// Session update types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdateParams {
    pub session_id: String,
    pub update: SessionUpdate,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateContent {
    Text { text: String },
    #[serde(other)]
    Other,
}

/// Plan entry from agent (TODO list item)
#[derive(Debug, Deserialize, Clone)]
pub struct PlanEntry {
    pub content: String,
    pub priority: PlanPriority,
    pub status: PlanStatus,
    #[serde(rename = "_meta", default)]
    pub meta: Option<Value>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanPriority {
    High,
    Medium,
    Low,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
    #[serde(other)]
    Unknown,
}

/// Session update variants - manually deserialize to handle unknown types gracefully
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    AgentMessageChunk { content: UpdateContent },
    ToolCall {
        tool_call_id: String,
        title: Option<String>,
        status: Option<String>,
    },
    ToolCallUpdate {
        tool_call_id: String,
        status: String,
    },
    Plan { entries: Vec<PlanEntry> },
    CurrentModeUpdate {
        current_mode_id: String,
    },
    AvailableCommandsUpdate,
    Other { raw_type: Option<String> },
}

impl<'de> serde::Deserialize<'de> for SessionUpdate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let update_type = value.get("sessionUpdate").and_then(|v| v.as_str());

        match update_type {
            Some("agent_message_chunk") => {
                let content = serde_json::from_value(
                    value.get("content").cloned().unwrap_or(Value::Null)
                ).unwrap_or(UpdateContent::Other);
                Ok(SessionUpdate::AgentMessageChunk { content })
            }
            Some("tool_call") => {
                Ok(SessionUpdate::ToolCall {
                    tool_call_id: value.get("toolCallId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: value.get("title").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    status: value.get("status").and_then(|v| v.as_str()).map(|s| s.to_string()),
                })
            }
            Some("tool_call_update") => {
                Ok(SessionUpdate::ToolCallUpdate {
                    tool_call_id: value.get("toolCallId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    status: value.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            }
            Some("plan") => {
                let entries = value.get("entries")
                    .and_then(|v| serde_json::from_value::<Vec<PlanEntry>>(v.clone()).ok())
                    .unwrap_or_default();
                Ok(SessionUpdate::Plan { entries })
            }
            Some("current_mode_update") => {
                Ok(SessionUpdate::CurrentModeUpdate {
                    current_mode_id: value.get("currentModeId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            }
            Some("available_commands_update") => {
                Ok(SessionUpdate::AvailableCommandsUpdate)
            }
            other => {
                Ok(SessionUpdate::Other { raw_type: other.map(|s| s.to_string()) })
            }
        }
    }
}

// ============================================================================
// Permission request parsing (incoming from agent)
// ============================================================================

/// Permission request params (for parsing incoming requests)
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestParams {
    pub session_id: String,
    pub tool_call: ToolCallInfo,
    pub options: Vec<PermissionOptionInfo>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallInfo {
    pub tool_call_id: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// Permission option info (for parsing, maps to PermissionOption)
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOptionInfo {
    pub option_id: String,
    pub name: String,
    pub kind: PermissionKindInfo,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKindInfo {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
    #[serde(other)]
    Unknown,
}

// ============================================================================
// File system request params (for parsing incoming requests)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadTextFileParams {
    pub session_id: String,
    pub path: String,
    pub line: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteTextFileParams {
    pub session_id: String,
    pub path: String,
    pub content: String,
}

// ============================================================================
// Terminal request params (for parsing incoming requests)
// ============================================================================

/// Environment variable entry
#[derive(Debug, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCreateParams {
    pub session_id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    pub output_byte_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputParams {
    pub session_id: String,
    pub terminal_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalWaitParams {
    pub session_id: String,
    pub terminal_id: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalKillParams {
    pub session_id: String,
    pub terminal_id: String,
}

// ============================================================================
// Response types for file system operations
// ============================================================================

#[derive(Debug, Serialize)]
pub struct FsWriteTextFileResult {
    pub success: bool,
}

// ============================================================================
// Type aliases for backward compatibility
// ============================================================================

/// Alias for PermissionRequestParams (used in client.rs)
pub type PermissionRequest = PermissionRequestParams;

/// Alias for PermissionKindInfo
pub type PermissionKind = PermissionKindInfo;

// ============================================================================
// Permission response types (using ACP-compatible format)
// ============================================================================

/// Permission response outcome - internally tagged with "outcome" field
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RequestPermissionOutcome {
    Cancelled,
    Selected {
        #[serde(rename = "optionId")]
        option_id: PermissionOptionId,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<Value>,
    },
}

/// Permission response (matches ACP RequestPermissionResponse structure)
#[derive(Debug, Serialize)]
pub struct RequestPermissionResponse {
    pub outcome: RequestPermissionOutcome,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl RequestPermissionResponse {
    pub fn selected(option_id: PermissionOptionId) -> Self {
        Self {
            outcome: RequestPermissionOutcome::Selected {
                option_id,
                meta: None,
            },
            meta: None,
        }
    }

    pub fn cancelled() -> Self {
        Self {
            outcome: RequestPermissionOutcome::Cancelled,
            meta: None,
        }
    }
}

// ============================================================================
// Message parsing
// ============================================================================

#[derive(Debug)]
pub enum IncomingMessage {
    Response(JsonRpcResponse),
    Notification { method: String, params: Option<Value> },
    Request { id: u64, method: String, params: Option<Value> },
}

impl IncomingMessage {
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        let value: Value = serde_json::from_str(line)?;

        let has_id = value.get("id").and_then(|v| v.as_u64()).is_some();
        let method = value.get("method").and_then(|m| m.as_str());

        match (has_id, method) {
            (true, Some(method)) => {
                let id = value.get("id").and_then(|v| v.as_u64()).unwrap();
                let params = value.get("params").cloned();
                Ok(IncomingMessage::Request {
                    id,
                    method: method.to_string(),
                    params,
                })
            }
            (true, None) => {
                let response: JsonRpcResponse = serde_json::from_value(value)?;
                Ok(IncomingMessage::Response(response))
            }
            (false, Some(method)) => {
                let params = value.get("params").cloned();
                Ok(IncomingMessage::Notification {
                    method: method.to_string(),
                    params,
                })
            }
            (false, None) => {
                let response: JsonRpcResponse = serde_json::from_value(value)?;
                Ok(IncomingMessage::Response(response))
            }
        }
    }
}
