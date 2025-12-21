# amux

A terminal multiplexer for AI coding agents. Manages multiple agent sessions in a single TUI interface.

## Architecture

```
src/
├── main.rs          # Entry point, event loop, key handling
├── app.rs           # App state, input modes, folder picker
├── log.rs           # Debug logging to ~/.amux/logs/
├── acp/             # Agent Client Protocol implementation
│   ├── mod.rs       # Module exports
│   ├── protocol.rs  # ACP types and message parsing
│   └── client.rs    # Agent connection, event handling
├── session/         # Session management
│   ├── mod.rs
│   ├── state.rs     # Session state, permission handling
│   └── manager.rs   # Session list management
└── tui/             # Terminal UI
    ├── mod.rs
    ├── ui.rs        # Layout and rendering
    └── theme.rs     # Colors and styling
```

## Agent Client Protocol (ACP)

This project uses the [Agent Client Protocol](https://agentclientprotocol.com) for communication with coding agents.

### Dependencies

- `agent-client-protocol = "0.9"` - Official Rust ACP crate from crates.io

### Protocol Implementation

The ACP implementation is in `src/acp/`:

- **protocol.rs**: Re-exports types from `agent-client-protocol` crate and defines custom types for:
  - JSON-RPC request/response handling
  - Session updates (message chunks, tool calls, plan updates)
  - Permission request/response (custom impl due to non-exhaustive ACP types)
  - File system and terminal request parsing

- **client.rs**: Agent connection management:
  - Spawns agent process based on `AgentType` (Claude Code or Gemini CLI)
  - Handles bidirectional JSON-RPC communication
  - Processes incoming requests (fs/read, fs/write, terminal/*)
  - Emits events for UI updates

### Key Types

From `agent-client-protocol`:
- `PermissionOptionId` - Identifier for permission options

Custom types (in protocol.rs):
- `SessionUpdate` - Enum for session update types
- `PermissionOptionInfo` - Permission option details
- `PermissionKind` - Allow/reject permission kinds
- `RequestPermissionResponse` - Permission response (custom, ACP's is non-exhaustive)
- `PlanEntry`, `PlanStatus` - TODO list items from agent

### Supported Agents

| Agent | Command | ACP Flag | Requirements |
|-------|---------|----------|--------------|
| Claude Code | `claude-code-acp` | (native) | `npx @anthropic-ai/claude-code-acp` |
| Gemini CLI | `gemini` | `--experimental-acp` | `npm install -g @google/gemini-cli` |

Both agents implement ACP natively. The `AgentType` enum in `src/session/state.rs` defines:
- `command()` - Returns the executable name
- `args()` - Returns any required flags (e.g., `--experimental-acp` for Gemini)

## Debug Logging

Logs are written to `~/.amux/logs/amux_<timestamp>.log` containing:
- All incoming/outgoing ACP messages
- Event processing
- Errors

## Key Bindings

- `i` - Insert mode (type message)
- `Esc` - Normal mode
- `j/k` - Navigate sessions
- `1-9` - Select session by number
- `n` - New session
- `x` - Kill session
- `Ctrl+u/d` - Scroll half page up/down
- `Ctrl+b/f` - Scroll full page up/down
- `g/G` - Scroll to top/bottom
- `y/Enter` - Allow permission
- `n/Esc` - Reject permission
- `q` - Quit

## TODO

- [x] **Markdown rendering** - Using ratskin 0.3 for termimad-based markdown rendering

## Blocked on ACP

These features require ACP spec/agent support that doesn't exist yet:

- **Token usage** - Display input/output tokens per session. ACP doesn't currently expose token counts in session updates.
- **Session resume** - Resume previous sessions. Requires `session/load` ACP support which is not yet implemented.
- **Prompt cancellation** - Esc sends `$/cancel_request` but agents don't honor it. Cancellation is still a draft feature in the ACP spec (see protocol/draft/cancellation).

## Building

```bash
cargo build --release
```

Requires Rust 2024 edition.
