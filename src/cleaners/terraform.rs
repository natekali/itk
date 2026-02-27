/// Clean Terraform/HCL: strip comments, collapse defaults, summarize.
pub fn clean_terraform(s: &str, aggressive: bool) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0usize;
    // State for collapsing default blocks
    let mut skip_default_block = false;
    let mut default_depth = 0i32;

    for line in s.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // Handle blank lines
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank_run = 0;

        // Skip default value blocks in aggressive mode
        if skip_default_block {
            for ch in trimmed.chars() {
                match ch {
                    '{' | '[' => default_depth += 1,
                    '}' | ']' => default_depth -= 1,
                    _ => {}
                }
            }
            if default_depth <= 0 {
                skip_default_block = false;
            }
            continue;
        }

        // Strip comments
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            if !aggressive {
                out.push(line.to_string());
            }
            continue;
        }

        // Strip inline comments
        let line_content = if aggressive {
            strip_inline_comment(trimmed)
        } else {
            trimmed.to_string()
        };

        // Aggressive: skip default blocks in variables
        if aggressive && line_content.trim().starts_with("default") && line_content.contains('{') {
            skip_default_block = true;
            default_depth = 0;
            for ch in line_content.chars() {
                match ch {
                    '{' | '[' => default_depth += 1,
                    '}' | ']' => default_depth -= 1,
                    _ => {}
                }
            }
            if default_depth > 0 {
                out.push(format!("{:indent$}default = {{ ... }}", "", indent = indent));
                continue;
            }
            skip_default_block = false;
            // Single-line default -- keep as-is
        }

        // Aggressive: skip description lines
        if aggressive && line_content.trim().starts_with("description") {
            continue;
        }

        // Normalize indentation
        let normalized_indent = ((indent + 1) / 2) * 2;
        out.push(format!("{:indent$}{}", "", line_content.trim(), indent = normalized_indent));
    }

    out.join("\n")
}

/// Strip inline # or // comments from a line (outside strings)
fn strip_inline_comment(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut string_char = b'"';
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == string_char {
                in_string = false;
            }
        } else {
            if b == b'"' {
                in_string = true;
                string_char = b'"';
            } else if b == b'#' {
                return line[..i].trim_end().to_string();
            } else if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                return line[..i].trim_end().to_string();
            }
        }
        i += 1;
    }
    line.to_string()
}
