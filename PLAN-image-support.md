# Image Paste/Drag Support Implementation Plan

## Overview

Add support for pasting or dragging images into amux, similar to Claude Code. Images will be attached to the current prompt and sent via ACP's `image` content block type.

## ACP Protocol Support

The ACP protocol already supports image content blocks:

```json
{
  "type": "image",
  "mimeType": "image/png",
  "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB..."  // base64 encoded
}
```

## Implementation Steps

### Phase 1: Extend ContentBlock Type

**File: `src/acp/protocol.rs`**

Update the `ContentBlock` enum to support images:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Image {
        #[serde(rename = "mimeType")]
        mime_type: String,
        data: String,  // base64 encoded
    },
}
```

### Phase 2: Add Image Attachment State

**File: `src/app.rs`**

Add attachment tracking to App state:

```rust
pub struct ImageAttachment {
    pub filename: String,      // Display name
    pub mime_type: String,     // image/png, image/jpeg, etc.
    pub data: String,          // base64 encoded data
}

pub struct App {
    // ... existing fields
    pub attachments: Vec<ImageAttachment>,
}
```

Add methods:
- `add_attachment(&mut self, attachment: ImageAttachment)`
- `clear_attachments(&mut self)`
- `has_attachments(&self) -> bool`

### Phase 3: Implement Clipboard Reading

**New dependency in `Cargo.toml`:**

```toml
arboard = "3"   # Cross-platform clipboard access
base64 = "0.22" # Base64 encoding
```

**File: `src/clipboard.rs` (new)**

```rust
use arboard::Clipboard;
use base64::Engine;

pub enum ClipboardContent {
    Text(String),
    Image { data: Vec<u8>, mime_type: String },
    None,
}

pub fn read_clipboard() -> Result<ClipboardContent> {
    let mut clipboard = Clipboard::new()?;
    
    // Try image first
    if let Ok(image) = clipboard.get_image() {
        let png_data = encode_as_png(&image)?;
        let base64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
        return Ok(ClipboardContent::Image {
            data: png_data,
            mime_type: "image/png".to_string(),
        });
    }
    
    // Fall back to text
    if let Ok(text) = clipboard.get_text() {
        return Ok(ClipboardContent::Text(text));
    }
    
    Ok(ClipboardContent::None)
}
```

### Phase 4: Handle Paste Events

**File: `src/main.rs`**

Add paste handling in the event loop:

```rust
// Enable bracketed paste mode in terminal setup
crossterm::terminal::enable_raw_mode()?;
execute!(stdout, EnableBracketedPaste)?;  // Add this

// In the event loop:
Event::Paste(text) => {
    // Terminal paste event - just text
    if app.mode == InputMode::Insert {
        for c in text.chars() {
            app.input_char(c);
        }
    }
}

// For Ctrl+V explicit paste:
KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    if app.mode == InputMode::Insert {
        match clipboard::read_clipboard() {
            Ok(ClipboardContent::Image { data, mime_type }) => {
                app.add_attachment(ImageAttachment {
                    filename: format!("image_{}.png", app.attachments.len() + 1),
                    mime_type,
                    data: base64::encode(&data),
                });
            }
            Ok(ClipboardContent::Text(text)) => {
                for c in text.chars() {
                    app.input_char(c);
                }
            }
            _ => {}
        }
    }
}
```

### Phase 5: Handle File Path Detection

When pasting text that looks like a file path to an image, offer to attach it:

```rust
fn try_attach_file_path(path: &str) -> Option<ImageAttachment> {
    let path = Path::new(path.trim());
    if !path.exists() { return None; }
    
    let extension = path.extension()?.to_str()?;
    let mime_type = match extension.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    
    let data = std::fs::read(path).ok()?;
    let base64_data = base64::encode(&data);
    
    Some(ImageAttachment {
        filename: path.file_name()?.to_string_lossy().to_string(),
        mime_type: mime_type.to_string(),
        data: base64_data,
    })
}
```

### Phase 6: Update Prompt Sending

**File: `src/main.rs` - `send_prompt()`**

Modify to include attachments:

```rust
async fn send_prompt(app: &mut App, agent_commands: &HashMap<...>, text: &str) {
    // Build content blocks
    let mut content_blocks: Vec<ContentBlock> = vec![];
    
    // Add text if present
    if !text.is_empty() {
        content_blocks.push(ContentBlock::Text { text: text.to_string() });
    }
    
    // Add image attachments
    for attachment in &app.attachments {
        content_blocks.push(ContentBlock::Image {
            mime_type: attachment.mime_type.clone(),
            data: attachment.data.clone(),
        });
    }
    
    // Send to agent
    if let Some(cmd_tx) = agent_commands.get(&session_idx) {
        let _ = cmd_tx.send(AgentCommand::PromptWithContent {
            session_id: session.id.clone(),
            content: content_blocks,
        }).await;
    }
    
    // Clear attachments after sending
    app.clear_attachments();
}
```

### Phase 7: Update ACP Client

**File: `src/acp/client.rs`**

Add method to send prompts with arbitrary content blocks:

```rust
pub async fn prompt_with_content(
    &mut self,
    session_id: &str,
    content: Vec<ContentBlock>,
) -> Result<()> {
    let params = PromptParams {
        session_id: session_id.to_string(),
        prompt: content,
    };

    let request = JsonRpcRequest::new(
        self.next_id(),
        "session/prompt",
        Some(serde_json::to_value(params)?),
    );
    self.send(request).await
}
```

### Phase 8: UI Updates

**File: `src/tui/ui.rs`**

Show attachment indicator in the input area:

```rust
fn render_input_area(frame: &mut Frame, app: &App, area: Rect) {
    // Show attachment count if any
    let input_title = if app.attachments.is_empty() {
        "Input".to_string()
    } else {
        format!("Input [{} image(s) attached]", app.attachments.len())
    };
    
    // Render input box with updated title
    let input_block = Block::default()
        .title(input_title)
        .borders(Borders::ALL);
    
    // ... rest of input rendering
}
```

Show attachment preview/list:

```rust
fn render_attachments(frame: &mut Frame, app: &App, area: Rect) {
    if app.attachments.is_empty() {
        return;
    }
    
    let items: Vec<ListItem> = app.attachments
        .iter()
        .enumerate()
        .map(|(i, att)| {
            ListItem::new(format!("[{}] {} ({})", i + 1, att.filename, att.mime_type))
        })
        .collect();
    
    let list = List::new(items)
        .block(Block::default().title("Attachments").borders(Borders::ALL));
    
    frame.render_widget(list, area);
}
```

### Phase 9: Attachment Management Keybindings

**File: `src/main.rs`**

Add keybindings for managing attachments:

| Key | Action |
|-----|--------|
| `Ctrl+V` | Paste from clipboard (text or image) |
| `Ctrl+A` | Open file picker to attach image |
| `Ctrl+X` | Clear all attachments |

### Phase 10: iTerm2/Kitty Image Protocol (Optional Enhancement)

For terminals that support inline images (iTerm2, Kitty, WezTerm), show image thumbnails:

```rust
fn render_image_preview(image_data: &[u8]) -> String {
    // iTerm2 inline image protocol
    let base64 = base64::encode(image_data);
    format!("\x1b]1337;File=inline=1:{}\x07", base64)
}
```

## File Changes Summary

| File | Changes |
|------|---------|
| `Cargo.toml` | Add `arboard`, `base64`, `image` dependencies |
| `src/acp/protocol.rs` | Add `Image` variant to `ContentBlock` |
| `src/app.rs` | Add `attachments` field and management methods |
| `src/clipboard.rs` | New file for clipboard reading |
| `src/main.rs` | Handle paste events, update `send_prompt()` |
| `src/acp/client.rs` | Add `prompt_with_content()` method |
| `src/tui/ui.rs` | Show attachment indicator and list |

## Dependencies to Add

```toml
arboard = "3"           # Clipboard access
base64 = "0.22"         # Base64 encoding
image = "0.25"          # Image processing (for clipboard images)
```

## Testing Considerations

1. Test clipboard paste with:
   - Plain text
   - PNG images
   - JPEG images
   - Mixed content

2. Test file path detection:
   - Absolute paths
   - Relative paths
   - Non-existent paths
   - Non-image files

3. Test with both agents:
   - Claude Code (full image support)
   - Gemini CLI (verify image capability)

## Edge Cases

- Large images: Consider size limits and compression
- Unsupported formats: Convert to PNG before sending
- Terminal without clipboard: Graceful fallback to text-only
- Agent without image capability: Show error message
