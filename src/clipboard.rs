//! Clipboard handling for image paste support

use anyhow::Result;
use arboard::Clipboard;
use base64::Engine;
use image::ImageEncoder;
use std::path::Path;

/// Content read from the clipboard
pub enum ClipboardContent {
    /// Plain text content
    Text(String),
    /// Image content with base64-encoded PNG data
    Image {
        data: String, // base64 encoded
        mime_type: String,
    },
    /// No content available
    None,
}

/// Read content from the system clipboard
/// Prioritizes images over text
pub fn read_clipboard() -> Result<ClipboardContent> {
    let mut clipboard = Clipboard::new()?;

    // Try image first
    if let Ok(img) = clipboard.get_image() {
        let png_data = encode_as_png(&img)?;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&png_data);
        return Ok(ClipboardContent::Image {
            data: base64_data,
            mime_type: "image/png".to_string(),
        });
    }

    // Fall back to text
    if let Ok(text) = clipboard.get_text() {
        if !text.is_empty() {
            return Ok(ClipboardContent::Text(text));
        }
    }

    Ok(ClipboardContent::None)
}

/// Encode an arboard ImageData as PNG
fn encode_as_png(img: &arboard::ImageData) -> Result<Vec<u8>> {
    use image::{ImageBuffer, Rgba};

    // arboard gives us RGBA data
    let width = img.width as u32;
    let height = img.height as u32;

    // Create image buffer from raw bytes
    let img_buffer: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(width, height, img.bytes.to_vec())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    // Encode as PNG
    let mut png_data = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
    encoder.write_image(
        &img_buffer,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;

    Ok(png_data)
}

/// Try to load an image from a file path
/// Returns base64-encoded image data if successful
pub fn load_image_from_path(path: &Path) -> Option<(String, String, String)> {
    if !path.exists() || !path.is_file() {
        return None;
    }

    let extension = path.extension()?.to_str()?;
    let mime_type = match extension.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };

    let data = std::fs::read(path).ok()?;
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&data);

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string();

    Some((filename, mime_type.to_string(), base64_data))
}

/// Check if a string looks like a file path to an image
pub fn try_parse_image_path(text: &str) -> Option<std::path::PathBuf> {
    let trimmed = text.trim();

    // Skip if it looks like multiple lines
    if trimmed.contains('\n') {
        return None;
    }

    // Try different path formats:
    // 1. Direct path (may contain spaces)
    // 2. Path with escaped spaces (backslash before space)
    // 3. Quoted path
    let candidates = [
        trimmed.to_string(),
        // Remove surrounding quotes if present
        trimmed.trim_matches('"').trim_matches('\'').to_string(),
        // Unescape backslash-escaped spaces
        trimmed.replace("\\ ", " "),
    ];

    for candidate in &candidates {
        let path = Path::new(candidate);

        // Check if extension is an image type
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            let is_image = matches!(
                extension.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp"
            );

            if is_image && path.exists() {
                return Some(path.to_path_buf());
            }
        }
    }

    None
}
