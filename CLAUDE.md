# amux

A terminal multiplexer for AI coding agents. Manages multiple agent sessions in a single TUI interface.

## Architecture

```
src/
├── main.rs          # Entry point, event loop, key handling
├── app.rs           # App state, input modes, picker state
├── clipboard.rs     # System clipboard integration (text & images)
├── config.rs        # Configuration file support (~/.config/amux/config.toml)
├── git.rs           # Git operations (worktrees, branches)
├── log.rs           # Debug logging to ~/.amux/logs/
├── scroll.rs        # Scroll event debouncing
├── acp/             # Agent Client Protocol implementation
│   ├── mod.rs       # Module exports
│   ├── protocol.rs  # ACP types and message parsing
│   └── client.rs    # Agent connection, event handling
├── events/          # Event handling (Action-based architecture)
│   ├── mod.rs       # Module exports
│   ├── action.rs    # Action enum for state changes
│   ├── handler.rs   # Central event handler
│   ├── keyboard.rs  # Keyboard event handling by mode
│   └── mouse.rs     # Mouse event handling
├── picker/          # Generic picker UI components
│   ├── mod.rs       # Module exports
│   └── traits.rs    # PickerItem trait for picker entries
├── session/         # Session management
│   ├── mod.rs       # Module exports
│   ├── state.rs     # Session state, permission handling
│   ├── manager.rs   # Session list management
│   └── scanner.rs   # Session discovery (existing agent sessions)
└── tui/             # Terminal UI
    ├── mod.rs       # Module exports
    ├── ui.rs        # Layout and rendering
    ├── theme.rs     # Colors and styling
    └── components/  # UI component organization
        └── mod.rs   # Re-exports render functions
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

### MCP Server Configuration

MCP (Model Context Protocol) servers can be configured in `~/.config/amux/config.toml`. These servers are passed to agent sessions via ACP's `session/new` params.

```toml
# Example MCP server configurations

[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed/dir"]

[[mcp_servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "your-token-here" }

[[mcp_servers]]
name = "postgres"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
env = { DATABASE_URL = "postgresql://localhost/mydb" }
```

**Note:** The ACP adapter (`claude-code-acp`) does NOT use Claude Code's standard MCP config (`~/.claude/mcp.json`). MCP servers must be configured in amux's config file to be available in sessions.

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
- `d` - Duplicate session
- `c` - Clear session (restart with confirmation)
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
- **Prompt cancellation** - Esc to cancel running prompts. Cancellation is still a draft feature in the ACP spec (see protocol/draft/cancellation) and agents don't honor `$/cancel_request`. Workaround: use `c` to clear/restart the session.
- **Clarifying questions** - Agent asks follow-up questions during planning or complex tasks. UI support exists (`session/ask_user`) but this is a Claude Code extension not yet implemented by agents.
- **Extended thinking** - Display agent reasoning/thinking. The ACP spec only defines `agent_message_chunk` for streamed text; there is no `agent_thought_chunk` update type. Code has support ready if ACP adds this.

## Building

```bash
cargo build --release
```

Requires Rust 2024 edition.

## Releasing

When creating a new release:
1. Update `version` in `Cargo.toml` to match the new tag
2. Update `version` in `flake.nix` to match the new tag
3. Commit the version bump
4. Create and push the git tag (e.g., `git tag v0.4.0 && git push --tags`)
