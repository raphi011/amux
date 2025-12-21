# Rust TUI Architecture Comparison

A comparison of amux's architecture with popular Rust TUI projects, identifying patterns and potential improvements.

## Projects Analyzed

| Project | GitHub Stars | Framework | Key Pattern |
|---------|-------------|-----------|-------------|
| [GitUI](https://github.com/gitui-org/gitui) | 18k+ | ratatui | Multi-crate workspace with `asyncgit/` as separate crate |
| [Bottom](https://github.com/ClementTsang/bottom) | 10k+ | ratatui | Monolithic with strong config/schema separation |
| [Zellij](https://github.com/zellij-org/zellij) | 22k+ | custom | Client-server architecture with plugin system |
| [Ratatui](https://github.com/ratatui/ratatui) | 10k+ | - | Modular workspace with backend-specific crates |

## amux Current Structure

```
src/
├── main.rs          # Entry point, event loop, key handling (~1200 lines)
├── app.rs           # App state, input modes, pickers (~700 lines)
├── log.rs           # Debug logging
├── clipboard.rs     # Clipboard utilities
├── git.rs           # Git operations
├── acp/             # Agent Client Protocol
│   ├── mod.rs
│   ├── protocol.rs  # ACP types and message parsing
│   └── client.rs    # Agent connection handling (~700 lines)
├── session/
│   ├── mod.rs
│   ├── state.rs     # Session state, permissions (~500 lines)
│   ├── manager.rs   # Session list management
│   └── scanner.rs   # Session scanning
├── picker/
│   ├── mod.rs
│   └── traits.rs    # Reusable Picker trait
└── tui/
    ├── mod.rs
    ├── ui.rs        # Layout and rendering (~1200 lines)
    └── theme.rs     # Colors and styling
```

## Key Findings

### 1. Event Handling Consolidation

**Current State**: `main.rs` contains ~1200 lines mixing event loop, key handling, and business logic.

**Pattern from GitUI/Zellij**: Dedicated event handling modules.

**Suggested Structure**:
```
src/events/
├── mod.rs
├── handler.rs      # Event dispatch logic
├── keyboard.rs     # Key event handling by mode
└── mouse.rs        # Mouse event handling
```

**Benefits**:
- Easier to test key bindings in isolation
- Reduced cognitive load when modifying input handling
- Clearer separation of concerns

---

### 2. Component-Based UI Architecture

**Current State**: `ui.rs` has 1200+ lines with many top-level `render_*` functions.

**Pattern from tui-realm / r3bl_tui**: Trait-based component system.

```rust
pub trait Component {
    type State;
    fn handle_event(&mut self, event: &Event) -> Option<Action>;
    fn render(&self, frame: &mut Frame, area: Rect, state: &Self::State);
}
```

**Suggested Structure**:
```
src/tui/
├── components/
│   ├── mod.rs
│   ├── session_list.rs
│   ├── output_area.rs
│   ├── input_bar.rs
│   ├── permission_dialog.rs
│   ├── folder_picker.rs
│   └── help_popup.rs
├── layout.rs       # Layout calculations
├── theme.rs        # Colors and styling
└── mod.rs
```

**Benefits**:
- Each component encapsulates its own rendering logic
- Components can be tested independently
- Easier to add new UI elements

---

### 3. Action/Message Pattern

**Current State**: Direct state mutation scattered through event handling.

**Pattern from Ratatui templates / Elm architecture**:

```rust
pub enum Action {
    // Navigation
    Quit,
    NextSession,
    PrevSession,
    SelectSession(usize),
    
    // Input
    EnterInsertMode,
    ExitInsertMode,
    InputChar(char),
    SendPrompt,
    
    // Permissions
    AllowPermission { request_id: u64 },
    DenyPermission { request_id: u64 },
    
    // Agent
    SpawnAgent { agent_type: AgentType, cwd: PathBuf },
    KillSession,
}
```

**Benefits**:
- Decouples input handling from state changes
- Actions can be logged, replayed, or undone
- Easier to test state transitions
- Enables command palette / keybinding customization

---

### 4. Workspace Organization

**Current State**: Single crate with all code in `src/`.

**Pattern from GitUI**: Domain logic as separate crates.

**Potential Structure**:
```
Cargo.toml (workspace)
├── amux/              # TUI application
│   └── src/
├── amux-acp/          # Agent Client Protocol (reusable)
│   └── src/
│       ├── client.rs
│       ├── protocol.rs
│       └── types.rs
└── amux-session/      # Session management (optional)
    └── src/
```

**Benefits**:
- `amux-acp` could be used by other tools
- Cleaner dependency management
- Faster incremental compilation
- Easier unit testing of protocol logic

**Trade-offs**:
- More complexity for a smaller project
- Only worthwhile if ACP client has external use cases

---

### 5. Configuration System

**Current State**: `WorktreeConfig` with env var / CLI precedence.

**Pattern from Bottom**: Dedicated config with schema validation.

**Suggested Additions**:
```
~/.config/amux/
├── config.toml      # User configuration
└── themes/          # Custom color themes
```

```rust
#[derive(Deserialize)]
pub struct Config {
    pub worktree_dir: Option<PathBuf>,
    pub default_agent: AgentType,
    pub keybindings: KeyBindings,
    pub theme: String,
}
```

---

### 6. Scroll Event Debouncing

**Current State**: Direct scroll handling on each event.

**Pattern from flow-codex**: Accumulate scroll deltas over time window.

```rust
pub struct ScrollHelper {
    accumulated_delta: i32,
    last_event: Instant,
    debounce_ms: u64,
}

impl ScrollHelper {
    pub fn accumulate(&mut self, delta: i32) -> Option<i32> {
        let now = Instant::now();
        if now.duration_since(self.last_event).as_millis() > self.debounce_ms {
            self.accumulated_delta = delta;
        } else {
            self.accumulated_delta += delta;
        }
        self.last_event = now;
        
        // Return accumulated value after debounce period
        Some(self.accumulated_delta)
    }
}
```

---

## What amux Does Well

1. **Clean module boundaries** - `acp/`, `session/`, `tui/` are logically separated
2. **Picker trait abstraction** - Reusable pattern for all selection UIs
3. **Async architecture** - Proper tokio channels for non-blocking agent communication
4. **Terminal lifecycle** - Correct raw mode / alternate screen management with cleanup
5. **Diff visualization** - Clean integration with `similar` crate
6. **State machine hints** - `SessionState::can_transition_to()` shows good state management thinking

---

## Recommended Refactoring Priority

| Change | Effort | Impact | Priority |
|--------|--------|--------|----------|
| Extract key handling to `events/` | Medium | High | 1 |
| Add Action enum for state changes | Medium | High | 2 |
| Component-ize UI rendering | High | Medium | 3 |
| Extract `acp/` to workspace crate | Medium | Low | 4 |
| Add configuration file support | Low | Medium | 5 |
| Scroll debouncing | Low | Low | 6 |

---

## Architecture Decision: When to Refactor

Consider these patterns when:
- `main.rs` exceeds 500 lines of event handling
- Adding a new input mode requires touching 3+ files
- UI changes require modifying unrelated render functions
- You want to reuse the ACP client in another project
- Testing requires spinning up the full application

The current architecture is appropriate for the project's size. These patterns provide a roadmap for scaling.

---

## Sources

- [GitUI GitHub](https://github.com/gitui-org/gitui)
- [Bottom GitHub](https://github.com/ClementTsang/bottom)
- [Zellij GitHub](https://github.com/zellij-org/zellij)
- [Ratatui GitHub](https://github.com/ratatui/ratatui)
- [Ratatui Component Template](https://ratatui.rs/templates/component/tui-rs/)
- [tui-realm](https://github.com/veeso/tui-realm) - React/Elm inspired ratatui framework
- [r3bl_tui](https://docs.rs/r3bl_tui) - Unidirectional data flow TUI framework
