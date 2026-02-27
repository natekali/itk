/// Default key=value pairs commonly found in Kubernetes/Docker/CI YAML.
/// Only suppressed in aggressive mode to avoid hiding intentional values.
const YAML_DEFAULTS: &[(&str, &str)] = &[
    ("enabled", "true"),
    ("disabled", "false"),
    ("replicas", "1"),
    ("debug", "false"),
    ("verbose", "false"),
    ("restart", "always"),
    ("protocol", "http"),
    ("ssl", "false"),
    ("tls", "false"),
];

/// Documentation-only field keys in OpenAPI / JSON Schema YAML.
const YAML_DOC_KEYS: &[&str] = &[
    "description",
    "title",
    "summary",
    "example",
    "examples",
    "$comment",
    "x-description",
];

pub fn clean_yaml(s: &str, aggressive: bool) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0u32;
    // State for skipping multi-line block scalar values under doc keys
    let mut skip_block = false;
    let mut block_indent = 0usize;
    // State for truncating long block scalars (|, >)
    let mut in_block_scalar = false;
    let mut block_scalar_indent = 0usize;
    let mut block_scalar_lines = 0usize;
    let mut block_scalar_truncated = false;
    let mut block_scalar_total = 0usize;
    // State for skipping status sections (aggressive)
    let mut skip_status = false;
    let mut status_indent = 0usize;

    for line in s.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // Exit block-scalar skip when we outdent
        if skip_block {
            if !trimmed.is_empty() && indent <= block_indent {
                skip_block = false;
            } else {
                continue;
            }
        }

        // Exit status section skip when we outdent
        if skip_status {
            if !trimmed.is_empty() && indent <= status_indent {
                skip_status = false;
            } else {
                continue;
            }
        }

        // Handle long block scalar truncation
        if in_block_scalar {
            if trimmed.is_empty() || indent > block_scalar_indent {
                block_scalar_lines += 1;
                block_scalar_total += 1;
                if block_scalar_lines <= 3 {
                    let normalized_indent = ((indent + 1) / 2) * 2;
                    out.push(format!("{:indent$}{trimmed}", "", indent = normalized_indent));
                } else if !block_scalar_truncated {
                    block_scalar_truncated = true;
                    // We'll insert the count after the block ends
                }
                continue;
            } else {
                // Block scalar ended -- insert truncation note if needed
                if block_scalar_truncated {
                    let hidden = block_scalar_total - 3;
                    let note_indent = ((block_scalar_indent + 2 + 1) / 2) * 2;
                    out.push(format!("{:indent$}# ... [{hidden} more lines]", "", indent = note_indent));
                }
                in_block_scalar = false;
            }
        }

        // Strip comment-only lines
        if trimmed.starts_with('#') {
            continue;
        }

        // Blank line handling
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank_run = 0;

        // Aggressive-mode filters
        if aggressive {
            // Skip status sections in Kubernetes resource dumps
            if indent == 0 && trimmed == "status:" {
                skip_status = true;
                status_indent = indent;
                out.push("status: # [omitted by itk]".to_string());
                continue;
            }

            // Skip documentation keys (description, title, example, etc.)
            let is_doc_key = YAML_DOC_KEYS.iter().any(|dk| {
                trimmed == *dk
                    || trimmed.starts_with(&format!("{dk}:"))
                    || trimmed.starts_with(&format!("{dk} :"))
            });
            if is_doc_key {
                let after_colon = trimmed
                    .splitn(2, ':')
                    .nth(1)
                    .map(|s| s.trim())
                    .unwrap_or("");
                if after_colon == "|" || after_colon == ">" || after_colon.is_empty() {
                    skip_block = true;
                    block_indent = indent;
                }
                continue;
            }

            // Skip known default key=value pairs (guard: never skip lines with & anchor)
            if !trimmed.contains('&') {
                let is_default = YAML_DEFAULTS.iter().any(|(key, val)| {
                    trimmed == format!("{key}: {val}")
                        || trimmed == format!("{key}: \"{val}\"")
                        || trimmed == format!("{key}: '{val}'")
                });
                if is_default {
                    continue;
                }
            }
        }

        // Detect block scalar start and set up truncation
        let after_colon = trimmed.splitn(2, ':').nth(1).map(|s| s.trim()).unwrap_or("");
        if after_colon == "|" || after_colon == ">" || after_colon == "|+" || after_colon == ">-" {
            in_block_scalar = true;
            block_scalar_indent = indent;
            block_scalar_lines = 0;
            block_scalar_truncated = false;
            block_scalar_total = 0;
        }

        // Normalize indentation to even multiples of 2
        let normalized_indent = ((indent + 1) / 2) * 2;
        out.push(format!("{:indent$}{trimmed}", "", indent = normalized_indent));
    }

    // Flush final block scalar truncation
    if in_block_scalar && block_scalar_truncated {
        let hidden = block_scalar_total - 3;
        let note_indent = ((block_scalar_indent + 2 + 1) / 2) * 2;
        out.push(format!("{:indent$}# ... [{hidden} more lines]", "", indent = note_indent));
    }

    out.join("\n")
}
