# Undoing the Debug Tool JSON Feature

This document describes how to remove the temporary debug toggle ('t' key) that renders raw ACP JSON under tool calls.

## Files Modified

### 1. `src/session/state.rs`

**Remove `raw_json` from `OutputType::ToolCall`:**

```rust
// Change this:
ToolCall {
    tool_call_id: String,
    name: String,
    description: Option<String>,
    failed: bool,
    raw_json: Option<String>,  // REMOVE THIS LINE
},

// Back to:
ToolCall {
    tool_call_id: String,
    name: String,
    description: Option<String>,
    failed: bool,
},
```

**Update `add_tool_call` method signature and body:**

Remove the `raw_json` parameter and all related logic in the `add_tool_call` method (around line 562).

### 2. `src/app.rs`

**Remove `debug_tool_json` field from `App` struct:**

```rust
// Remove this line from the App struct:
pub debug_tool_json: bool,
```

**Remove from `App::new()`:**

```rust
// Remove this line from the constructor:
debug_tool_json: false,
```

**Remove `toggle_debug_tool_json` method:**

```rust
// Remove this entire method:
pub fn toggle_debug_tool_json(&mut self) {
    self.debug_tool_json = !self.debug_tool_json;
}
```

### 3. `src/events/action.rs`

**Remove the `ToggleDebugToolJson` action:**

```rust
// Remove these lines:
// === Debug ===
/// Toggle debug mode for tool JSON display
ToggleDebugToolJson,
```

### 4. `src/events/keyboard.rs`

**Remove the 't' key handler:**

```rust
// Remove these lines from handle_normal_mode:
// Toggle debug tool JSON display
KeyCode::Char('t') => Action::ToggleDebugToolJson,
```

### 5. `src/main.rs`

**Remove 't' key handling:**

```rust
// Remove these lines (around line 910):
KeyCode::Char('t') => {
    // Toggle debug tool JSON display
    app.toggle_debug_tool_json();
}
```

**Update SessionUpdate::ToolCall pattern match:**

```rust
// Change this:
SessionUpdate::ToolCall {
    tool_call_id,
    title,
    raw_description,
    raw_json,
    ..
} => {

// Back to:
SessionUpdate::ToolCall {
    tool_call_id,
    title,
    raw_description,
    ..
} => {
```

**Update `add_tool_call` call:**

```rust
// Change this:
session.add_tool_call(tool_call_id, name, description, raw_json);

// Back to:
session.add_tool_call(tool_call_id, name, description);
```

### 6. `src/acp/protocol.rs`

**Remove `raw_json` from `SessionUpdate::ToolCall`:**

```rust
// Remove raw_json field from the enum variant and the deserializer
```

### 7. `src/tui/ui.rs`

**Remove debug JSON rendering in `render_output_area`:**

Remove the `debug_tool_json` variable and the entire block that renders JSON under tool calls.

## Quick Revert

If using git, you can revert all changes with:

```bash
git checkout -- src/session/state.rs src/app.rs src/events/action.rs src/events/keyboard.rs src/main.rs src/acp/protocol.rs src/tui/ui.rs
```

Then delete this file:

```bash
rm docs/UNDO_DEBUG_TOOL_JSON.md
```
