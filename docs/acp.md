# Agent Client Protocol (ACP) for amux

## Overview

ACP is a JSON-RPC 2.0 based protocol for communication between code editors and AI coding agents. It runs over stdio (stdin/stdout pipes) with agents in separate processes.

## Adapter Required

Claude Code doesn't have a native `--acp` flag. Use the `@zed-industries/claude-code-acp` adapter:

```bash
npm install -g @zed-industries/claude-code-acp
```

Run with:
```bash
ANTHROPIC_API_KEY=sk-... claude-code-acp
```

### Using a Custom Claude Build

If you have a custom Claude Code build, you can tell the ACP adapter to use it by setting the `CLAUDE_CODE_EXECUTABLE` environment variable:

```bash
export CLAUDE_CODE_EXECUTABLE=/path/to/your/custom/claude
```

Then run amux as normal. The adapter will use your custom Claude executable instead of the default SDK CLI.

## Protocol Flow

```
Client                          Agent
   |                              |
   |-- initialize --------------->|
   |<------------ initialized ----|
   |                              |
   |-- session/new -------------->|
   |<-------- session created ----|
   |                              |
   |-- session/prompt ----------->|
   |<------ session/update -------|  (streaming, multiple)
   |<------ session/update -------|
   |<-------- prompt complete ----|
```

## Key Messages

### initialize

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": 1,
    "clientCapabilities": {
      "fs": { "readTextFile": true, "writeTextFile": true },
      "terminal": true
    },
    "clientInfo": { "name": "amux", "title": "amux", "version": "0.1.0" }
  }
}
```

### session/new

Request (mcpServers is required, can be empty):
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session/new",
  "params": {
    "cwd": "/path/to/project",
    "mcpServers": []
  }
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": { "sessionId": "session_123" }
}
```

### session/prompt

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session/prompt",
  "params": {
    "sessionId": "session_123",
    "prompt": [{ "type": "text", "text": "Hello" }]
  }
}
```

Response (after all updates complete):
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": { "stopReason": "end_turn" }
}
```

## Session Updates (Notifications)

Updates are sent as notifications during prompt processing.

### agent_message_chunk

Streaming text from the agent:
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "session_123",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": { "type": "text", "text": "Hello! How can I help?" }
    }
  }
}
```

### tool_call

Agent is calling a tool:
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "session_123",
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "tool_1",
      "title": "Read file",
      "status": "running"
    }
  }
}
```

### tool_call_update

Tool execution progress:
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "session_123",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "tool_1",
      "status": "Reading src/main.rs..."
    }
  }
}
```

## Stop Reasons

- `end_turn` - Agent finished responding
- `max_tokens` - Token limit reached
- `cancelled` - Client cancelled the request
- `refusal` - Agent refused to continue

## Known Limitations

### Token Usage

ACP does not currently report token usage. The protocol has no standard `usage_update` session update type. Token counting would need to be implemented at the SDK level.

### MCP Servers

MCP (Model Context Protocol) servers can be passed to `session/new`:
```json
{
  "mcpServers": [{
    "name": "my-mcp",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"],
    "env": []
  }]
}
```

If Claude Code CLI has MCP servers configured, they should be accessible through the ACP session without additional setup.

## Permission Requests

The agent can request permission before executing certain tools via `session/request_permission`. This is a JSON-RPC **request** (not notification) that requires a response.

Request from agent:
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "session/request_permission",
  "params": {
    "sessionId": "session_123",
    "toolCall": { "toolCallId": "call_001", "title": "Write file" },
    "options": [
      { "optionId": "allow_once", "name": "Allow once", "kind": "allow_once" },
      { "optionId": "allow_always", "name": "Always allow", "kind": "allow_always" },
      { "optionId": "reject", "name": "Deny", "kind": "reject_once" }
    ]
  }
}
```

Response from client (selected):
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "outcome": "selected",
    "optionId": "allow_once"
  }
}
```

Response from client (cancelled):
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "outcome": "cancelled"
  }
}
```

## Session Updates

All session updates use the `sessionUpdate` discriminator field:

| Update Type | Description |
|-------------|-------------|
| `agent_message_chunk` | Streaming text from agent |
| `tool_call` | Tool execution started |
| `tool_call_update` | Tool execution progress |
| `plan` | Agent's task list/todos |
| `current_mode_update` | Mode changed (e.g., "plan") |

## File System Requests

When the client advertises `fs` capabilities, the agent can request file operations. These are JSON-RPC **requests** (not notifications) that require a response.

### fs/read_text_file

Request from agent:
```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "fs/read_text_file",
  "params": {
    "sessionId": "session_123",
    "path": "/path/to/file.txt",
    "line": 1,
    "limit": 100
  }
}
```

Response from client:
```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "result": { "content": "file contents here..." }
}
```

### fs/write_text_file

Request from agent:
```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "fs/write_text_file",
  "params": {
    "sessionId": "session_123",
    "path": "/path/to/file.txt",
    "content": "new file contents"
  }
}
```

Response from client:
```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "result": { "success": true }
}
```

## Features Available for amux

| Feature | Status | Notes |
|---------|--------|-------|
| Spawn agent | ✅ | Via `claude-code-acp` adapter |
| Send prompts | ✅ | `session/prompt` with text content |
| Stream responses | ✅ | `agent_message_chunk` updates |
| Tool calls | ✅ | `tool_call` and `tool_call_update` |
| Permission requests | ✅ | `session/request_permission` with UI |
| Plan/todo display | ✅ | `plan` updates shown in sidebar |
| Mode display | ✅ | `current_mode_update` shows [plan] etc |
| Multiple sessions | ✅ | Each gets unique `sessionId` |
| File read | ✅ | `fs/read_text_file` with line/limit |
| File write | ✅ | `fs/write_text_file` |
| Token counting | ❌ | Not supported by protocol |
| Cancel prompt | ⚠️ | `$/cancel_request` - not tested |

## Resources

- [Agent Client Protocol](https://agentclientprotocol.com)
- [claude-code-acp](https://github.com/zed-industries/claude-code-acp)
- [ACP Python SDK](https://github.com/agentclientprotocol/python-sdk)
