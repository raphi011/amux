# Gemini CLI Integration Plan

## Overview

Integrate [Gemini CLI](https://github.com/google-gemini/gemini-cli) as a second agent option in amux alongside Claude Code. Gemini CLI has native ACP support via the `--experimental-acp` flag, making integration straightforward.

## Current State

The codebase already has infrastructure for multiple agents:
- `AgentType` enum in `src/session/state.rs` defines `ClaudeCode` and `GeminiCli` variants
- `AgentConnection::spawn()` in `src/acp/client.rs` is generic (accepts any command)
- Sessions store `agent_type` metadata

**However**, agent spawning is hardcoded:
- `main.rs:577` - Always uses `AgentType::ClaudeCode`
- `main.rs:595, 653` - Hardcoded to `"claude-code-acp"` command

## Integration Steps

### Phase 1: Wire Up Existing Infrastructure

**1.1 Update command mapping** (`src/session/state.rs`)

```rust
impl AgentType {
    pub fn command(&self) -> &'static str {
        match self {
            AgentType::ClaudeCode => "claude-code-acp",
            AgentType::GeminiCli => "gemini",
        }
    }
    
    pub fn args(&self) -> Vec<&'static str> {
        match self {
            AgentType::ClaudeCode => vec![],
            AgentType::GeminiCli => vec!["--experimental-acp"],
        }
    }
}
```

**1.2 Update AgentConnection::spawn()** (`src/acp/client.rs`)

Pass `AgentType` to configure command and args:

```rust
pub async fn spawn(
    agent_type: AgentType,
    cwd: &Path,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<Self> {
    let command = agent_type.command();
    let args = agent_type.args();
    
    let mut child = Command::new(command)
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    // ...
}
```

**1.3 Update spawn calls** (`src/main.rs`)

Replace hardcoded command with agent type from session:

```rust
// Line ~595 and ~653
let agent_type = app.sessions.get(idx).map(|s| s.agent_type).unwrap_or(AgentType::ClaudeCode);
match AgentConnection::spawn(agent_type, &cwd_clone, event_tx.clone()).await {
```

### Phase 2: Agent Selection UI

**2.1 Add agent picker state** (`src/app.rs`)

```rust
pub enum InputMode {
    Normal,
    Insert,
    FolderPicker(FolderPickerState),
    AgentPicker { cwd: PathBuf },  // New state
}
```

**2.2 Modify folder picker flow**

After selecting a directory, transition to `AgentPicker` instead of spawning immediately:

```rust
// In handle_folder_picker_input()
if confirmed {
    let cwd = picker.selected_path();
    self.input_mode = InputMode::AgentPicker { cwd };
}
```

**2.3 Render agent picker** (`src/tui/ui.rs`)

Simple list showing:
- `[c] Claude Code`
- `[g] Gemini CLI`

**2.4 Handle agent selection** (`src/main.rs`)

```rust
InputMode::AgentPicker { cwd } => {
    match key.code {
        KeyCode::Char('c') => spawn_agent(AgentType::ClaudeCode, cwd),
        KeyCode::Char('g') => spawn_agent(AgentType::GeminiCli, cwd),
        KeyCode::Esc => app.input_mode = InputMode::Normal,
        _ => {}
    }
}
```

### Phase 3: Configuration & Credentials

**3.1 Create config module** (`src/config.rs`)

```rust
pub struct AgentConfig {
    pub available: bool,
    pub command: String,
    pub env: HashMap<String, String>,
}

pub fn check_agent_available(agent_type: AgentType) -> AgentConfig {
    match agent_type {
        AgentType::ClaudeCode => {
            // Check for ANTHROPIC_API_KEY or claude-code-acp in PATH
        }
        AgentType::GeminiCli => {
            // Check for GOOGLE_API_KEY or gemini in PATH
            // Note: Gemini CLI supports OAuth login, so API key is optional
        }
    }
}
```

**3.2 Environment variables**

| Agent | Required Env Vars | Notes |
|-------|-------------------|-------|
| Claude Code | `ANTHROPIC_API_KEY` | Via claude-code-acp adapter |
| Gemini CLI | `GOOGLE_API_KEY` (optional) | Supports OAuth login |

**3.3 Pass environment to spawn**

```rust
let mut cmd = Command::new(command);
for (key, value) in agent_config.env.iter() {
    cmd.env(key, value);
}
```

### Phase 4: UI Enhancements

**4.1 Display agent type in session list**

Add icon or label to session items:
- ` Claude: project-name`
- ` Gemini: project-name`

**4.2 Theme agent-specific colors** (`src/tui/theme.rs`)

```rust
pub fn agent_color(agent_type: AgentType) -> Color {
    match agent_type {
        AgentType::ClaudeCode => Color::Rgb(204, 119, 34),  // Anthropic orange
        AgentType::GeminiCli => Color::Rgb(66, 133, 244),   // Google blue
    }
}
```

## Prerequisites

1. **Gemini CLI installed**: `npm install -g @google/gemini-cli`
2. **Authentication**: Either:
   - Set `GOOGLE_API_KEY` environment variable, or
   - Run `gemini` once to complete OAuth login

## Testing Plan

1. Install Gemini CLI locally
2. Verify `gemini --experimental-acp` starts and accepts JSON-RPC
3. Test session creation with both agent types
4. Verify permission handling works for both
5. Test file operations (read/write)
6. Test terminal command execution

## File Changes Summary

| File | Changes |
|------|---------|
| `src/session/state.rs` | Add `command()` and `args()` methods |
| `src/acp/client.rs` | Update `spawn()` signature |
| `src/main.rs` | Use agent type from session, add agent picker handling |
| `src/app.rs` | Add `AgentPicker` input mode |
| `src/tui/ui.rs` | Render agent picker, show agent type in session list |
| `src/tui/theme.rs` | Add agent-specific colors |
| `src/config.rs` (new) | Agent availability checking |

## References

- [Gemini CLI GitHub](https://github.com/google-gemini/gemini-cli)
- [Agent Client Protocol](https://agentclientprotocol.com)
- [Zed ACP Integration](https://zed.dev/blog/bring-your-own-agent-to-zed)
- [Gemini CLI ACP Discussion](https://github.com/google-gemini/gemini-cli/discussions/7540)
