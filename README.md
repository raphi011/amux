# amux

A terminal multiplexer for AI coding agents. Run multiple AI agents side-by-side in a single terminal, manage their sessions, and interact with them through a unified TUI.

## What is amux?

amux (agent multiplexer) lets you spawn and manage multiple AI coding agents in one terminal window. Think of it as tmux, but purpose-built for AI agents. Instead of juggling multiple terminal tabs or windows for each agent, amux provides a unified interface where you can:

- Run multiple agents simultaneously (Claude Code, Gemini CLI)
- Switch between agent sessions instantly
- Monitor all agents at a glance
- Handle permission requests across sessions

## How it works

amux uses the [Agent Client Protocol (ACP)](https://agentclientprotocol.com) to communicate with AI agents. ACP is a standardized protocol that enables structured communication between clients and AI coding assistants over JSON-RPC.

When you start a new session, amux:

1. Spawns the agent process (e.g., `claude-code-acp` or `gemini --experimental-acp`)
2. Establishes a bidirectional JSON-RPC connection over stdio
3. Handles all ACP messages: prompts, streaming responses, tool calls, and permission requests
4. Renders the agent's output with markdown formatting in the terminal

The architecture follows a session manager pattern where each agent runs in its own async task, with all events funneled through a central event loop for UI updates.

## Installation

### Homebrew (macOS and Linux)

```bash
brew tap raphi011/amux https://github.com/raphi011/amux
brew install amux
```

### From source

```bash
cargo install --git https://github.com/raphi011/amux
```

Or clone and build:

```bash
git clone https://github.com/raphi011/amux.git
cd amux
cargo build --release
```

The binary will be at `target/release/amux`.

### Pre-built binaries

Download pre-built binaries from the [releases page](https://github.com/raphi011/amux/releases).

### Requirements

You'll need at least one supported agent installed:

| Agent | Installation |
|-------|--------------|
| Claude Code | `npx @anthropic-ai/claude-code-acp` |
| Gemini CLI | `npm install -g @google/gemini-cli` |

## Features

- **Multi-agent support** - Run Claude Code and Gemini CLI agents simultaneously
- **Session management** - Create, duplicate, switch, clear, and kill agent sessions
- **Real-time streaming** - See agent responses as they're generated
- **Permission handling** - Approve or reject file system and terminal operations with multiple permission modes
- **Markdown rendering** - Agent output is rendered with proper formatting using termimad
- **Git worktree integration** - Spawn agents in different worktrees, manage and clean up worktrees
- **Vim-style navigation** - Familiar keybindings for fast navigation
- **Scroll history** - Scroll through agent output with page up/down
- **Clipboard support** - Paste text and images from clipboard as attachments
- **Desktop notifications** - Get notified when agents need attention (permissions, questions, task complete)
- **Model cycling** - Switch between available models for agents
- **MCP server support** - Configure Model Context Protocol servers for agent sessions
- **Bug reporting** - Built-in bug report submission
- **Debug logging** - Detailed logs for troubleshooting in `~/.amux/logs/`

## Usage

Start amux in any directory:

```bash
amux
```

Or specify a directory:

```bash
amux /path/to/project
```

### Key bindings

#### Normal mode

| Key | Action |
|-----|--------|
| `i` / `Enter` | Enter insert mode |
| `n` | New session |
| `d` | Duplicate session |
| `c` | Clear session (with confirmation) |
| `x` | Kill current session |
| `j` / `k` | Navigate sessions |
| `1-9` | Jump to session by number |
| `w` | Open worktree picker |
| `m` | Cycle model |
| `v` | Cycle sort mode |
| `t` | Toggle debug tool JSON display |
| `Tab` | Cycle permission mode |
| `Ctrl+u` / `Ctrl+d` | Scroll half page |
| `Ctrl+b` / `Ctrl+f` | Scroll full page |
| `g` / `G` | Scroll to top/bottom |
| `?` | Open help |
| `B` | Open bug report |
| `q` | Quit |

#### Insert mode

| Key | Action |
|-----|--------|
| `Esc` | Exit insert mode |
| `Enter` | Send message |
| `Shift+Enter` / `Ctrl+j` | New line |
| `Ctrl+v` | Paste from clipboard |
| `Ctrl+x` | Clear attachments |
| `Ctrl+c` | Clear input |
| `Ctrl+a` / `Home` | Move to start of line |
| `Ctrl+e` / `End` | Move to end of line |
| `Alt+b` / `Alt+Left` | Move word left |
| `Alt+f` / `Alt+Right` | Move word right |
| `Ctrl+w` / `Alt+Backspace` | Delete word backward |
| `Alt+d` | Delete word forward |
| `Ctrl+k` | Kill to end of line |
| `Ctrl+u` | Kill to start of line |

#### Permission/question dialogs

| Key | Action |
|-----|--------|
| `y` / `Enter` | Allow/confirm |
| `n` / `Esc` | Deny/cancel |
| `j` / `k` | Navigate options |
| `Tab` | Cycle permission mode |

## Configuration

Configuration is stored in `~/.config/amux/config.toml`.

```toml
# Default agent for new sessions
default_agent = "ClaudeCode"  # or "GeminiCli"

# Directory for git worktrees
worktree_dir = "~/.amux/worktrees"

# Desktop notification settings
[notifications]
enabled = true
idle_delay_secs = 5
dedupe_interval_secs = 30

# MCP servers available to all sessions
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]

[[mcp_servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "your-token-here" }
```

**Note:** The ACP adapter (`claude-code-acp`) does NOT use Claude Code's standard MCP config (`~/.claude/mcp.json`). MCP servers must be configured in amux's config file to be available in sessions.

## Debug Logging

Logs are written to `~/.amux/logs/amux_<timestamp>.log` containing:
- All incoming/outgoing ACP messages
- Event processing
- Errors

## License

MIT
