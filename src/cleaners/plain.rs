pub fn clean_plain(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut blank_run = 0usize;
    // Track repeated lines for deduplication
    let mut last_line = String::new();
    let mut repeat_count = 0usize;
    let mut suppressed = 0usize;

    for line in s.lines() {
        let trimmed_end = line.trim_end();

        // Skip ASCII art borders: lines that are only -=+|_ characters
        if !trimmed_end.is_empty() && is_ascii_border(trimmed_end) {
            continue;
        }

        // Skip quoted reply chains (email/chat style)
        if trimmed_end.starts_with("> >") || trimmed_end.starts_with(">>>") {
            continue;
        }

        if trimmed_end.is_empty() {
            // Flush repeats before blank
            if suppressed > 0 {
                out.push(format!("  [... {suppressed} identical lines]"));
                suppressed = 0;
            }
            blank_run += 1;
            if blank_run <= 2 {
                out.push(String::new());
            }
            last_line.clear();
            repeat_count = 0;
        } else {
            blank_run = 0;
            // Deduplicate repeated lines
            if trimmed_end == last_line {
                repeat_count += 1;
                if repeat_count <= 2 {
                    out.push(trimmed_end.to_string());
                } else {
                    suppressed += 1;
                }
            } else {
                if suppressed > 0 {
                    out.push(format!("  [... {suppressed} identical lines]"));
                    suppressed = 0;
                }
                out.push(trimmed_end.to_string());
                last_line = trimmed_end.to_string();
                repeat_count = 1;
            }
        }
    }
    if suppressed > 0 {
        out.push(format!("  [... {suppressed} identical lines]"));
    }

    while out.first().map(|l: &String| l.is_empty()).unwrap_or(false) {
        out.remove(0);
    }
    while out.last().map(|l: &String| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

/// Detect lines that are purely ASCII borders/separators.
fn is_ascii_border(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() < 3 {
        return false;
    }
    trimmed.chars().all(|c| matches!(c, '-' | '=' | '+' | '|' | '_' | '*' | ' '))
}
