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

### Pre-built binaries

Download pre-built binaries from the [releases page](https://github.com/raphi011/amux/releases).

## Features

- **Multi-agent support** - Run Claude Code and Gemini CLI agents simultaneously
- **Session management** - Create, switch, and kill agent sessions on the fly
- **Real-time streaming** - See agent responses as they're generated
- **Permission handling** - Approve or reject file system and terminal operations
- **Markdown rendering** - Agent output is rendered with proper formatting
- **Git worktree integration** - Spawn agents in different worktrees, easily switch between them
- **Vim-style navigation** - Familiar keybindings for fast navigation
- **Scroll history** - Scroll through agent output with page up/down
- **Clipboard support** - Copy agent output to clipboard
- **Debug logging** - Detailed logs for troubleshooting in `~/.amux/logs/`

## Installation

### From source

```bash
git clone https://github.com/raphi011/amux.git
cd amux
cargo build --release
```

The binary will be at `target/release/amux`.

### Requirements

You'll need at least one supported agent installed:

| Agent | Installation |
|-------|--------------|
| Claude Code | `npx @anthropic-ai/claude-code-acp` |
| Gemini CLI | `npm install -g @google/gemini-cli` |

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

| Key | Action |
|-----|--------|
| `n` | New session |
| `x` | Kill current session |
| `j/k` | Navigate sessions |
| `1-9` | Jump to session by number |
| `i` | Insert mode (type message) |
| `Esc` | Normal mode |
| `Enter` | Send message (insert mode) |
| `Ctrl+u/d` | Scroll half page |
| `Ctrl+b/f` | Scroll full page |
| `g/G` | Scroll to top/bottom |
| `y` or `Enter` | Allow permission |
| `n` or `Esc` | Reject permission |
| `q` | Quit |

## License

MIT
