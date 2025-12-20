# amux

A TUI (Terminal User Interface) application for monitoring and managing multiple Claude Code agent instances.

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

- Spawns agents: Spawns new agents (claude code, gemini cli)
- Real-time Status: Shows current task, status, and last activity for each agent
- Keyboard Navigation: Vim-style shortcuts for efficient navigation
- Leverages [Zed Agent Client Protocol](https://zed.dev/acp)
- Git worktree support. Manage multiple worktrees, spawn agents in each worktree
- List / Detail view. List of all running agents, detailed view of each agent output

## Technical plan

ACP CLI Multiplexer - Implementation Summary
Goal
Build a tmux-like CLI tool in Rust that spawns multiple ACP (Agent Client Protocol) agents, manages their sessions, and renders their output in a TUI.
Architecture
┌────────────────────────────────────────────┐
│              CLI (TUI)                     │
│  ┌──────────────────────────────────────┐  │
│  │          Session Manager             │  │
│  │  - spawn/kill agents                 │  │
│  │  - route input to focused session    │  │
│  │  - track state per session           │  │
│  └──────────────┬───────────────────────┘  │
│                 │                          │
│  ┌──────────────┴───────────────────────┐  │
│  │         ACP Connection Pool          │  │
│  │  session_1: Claude Code (IDLE)       │  │
│  │  session_2: Gemini CLI (WORKING)     │  │
│  │  session_3: Custom Agent (TOOL)      │  │
│  └──────────────────────────────────────┘  │
└────────────────────────────────────────────┘
         │            │            │
       stdio        stdio        stdio
         ▼            ▼            ▼
      Agent 1      Agent 2      Agent 3
Per-Session State Machine
SPAWNING → INITIALIZING → IDLE ⇄ PROMPTING
                            ↓
                     AWAITING_PERMISSION
State transitions:

IDLE → PROMPTING: send session/prompt
PROMPTING → IDLE: receive PromptResponse(stop_reason: end_turn)
PROMPTING → AWAITING_PERMISSION: agent sends session/request_permission
AWAITING_PERMISSION → PROMPTING: respond to permission request

TUI Layout
┌─ Sessions ──────────────────────────────────┐
│ [1] claude-code    IDLE                     │
│ [2] gemini-cli     ● streaming...           │
│ [3] my-agent       ⚠ permission required    │
├─ Output (session 2) ────────────────────────┤
│ I'll analyze the codebase structure...      │
│ [tool: read_file] src/main.rs ✓             │
├─ Input ─────────────────────────────────────┤
│ > _                                         │
└─────────────────────────────────────────────┘
Layout structure:

Horizontal split: sidebar (fixed 30 chars) | main content
Vertical split on main: agent output (flex) | status bar (1 line)

Dependencies
toml[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
agent-client-protocol = "0.1"

# Optional helpers
tui-textarea = "0.7"    # input widget with cursor/history
tui-scrollview = "0.4"  # scrollable output viewport
Key Implementation Details

Async multiplexing: One tokio task per agent reading stdout, all feeding into a central mpsc channel
Event loop: Single loop handling agent updates + keyboard input, updating state and triggering re-renders
ACP protocol: Agents communicate via JSON-RPC over stdio; use stop_reason in PromptResponse to detect idle state
Rendering: Buffer session_update chunks per session, render focused session, show status badges on others

ACP Protocol Reference

Spec: https://agentclientprotocol.com
Key messages: initialize, session/new, session/prompt, session/update (notification), session/request_permission
Stop reasons: end_turn (idle), cancelled, max_tokens

Create a UI similar to the [screenshot](./ui.png)
