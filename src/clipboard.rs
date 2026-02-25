/// Cross-platform clipboard read/write via arboard.
/// On headless Linux (no display server), these return descriptive errors.

pub fn read() -> Result<String, String> {
    arboard::Clipboard::new()
        .map_err(|e| format!("clipboard init failed: {e}"))?
        .get_text()
        .map_err(|e| format!("clipboard read failed: {e}"))
}

pub fn write(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .map_err(|e| format!("clipboard init failed: {e}"))?
        .set_text(text.to_owned())
        .map_err(|e| format!("clipboard write failed: {e}"))
}
