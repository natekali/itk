//! Undo support: save/restore clipboard content from before ITK modified it.
//!
//! Stores the previous clipboard content in a temp file so `itk undo`
//! can restore it. Only the last modification is saved (not a full history).

use crate::clipboard;
use crate::style;
use std::path::PathBuf;

/// Path to the undo file.
fn undo_path() -> PathBuf {
    let tmp = std::env::temp_dir();
    tmp.join("itk-undo.txt")
}

/// Save current clipboard content before ITK modifies it.
/// Best-effort — never fails the main path.
pub fn save(content: &str) {
    let path = undo_path();
    let _ = std::fs::write(path, content);
}

/// Restore the previous clipboard content.
pub fn restore() {
    let path = undo_path();

    if !path.exists() {
        eprintln!("{} nothing to undo (no previous content saved)", style::dim("itk:"));
        return;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {} failed to read undo file: {e}", style::dim("itk:"), style::error("error:"));
            return;
        }
    };

    if content.is_empty() {
        eprintln!("{} nothing to undo (saved content was empty)", style::dim("itk:"));
        return;
    }

    match clipboard::write(&content) {
        Ok(()) => {
            let chars = content.len();
            let lines = content.lines().count();
            eprintln!("{} {} ({} chars, {} lines)",
                style::dim("itk:"),
                style::success("clipboard restored"),
                chars,
                lines
            );
            // Remove the undo file after successful restore
            let _ = std::fs::remove_file(&path);
        }
        Err(e) => {
            eprintln!("{} {} {e}", style::dim("itk:"), style::error("error:"));
        }
    }
}
