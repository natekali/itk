use regex::Regex;
use std::sync::OnceLock;
use super::collapse_blank_lines;

pub fn clean_code(s: &str, lang: &str, aggressive: bool) -> String {
    let trimmed = s.trim();

    // Detect if content is already fenced
    let (content, lang_tag) = if trimmed.starts_with("```") {
        let first_line = trimmed.lines().next().unwrap_or("```");
        let detected_lang = first_line.trim_start_matches('`').trim();
        let inner = trimmed
            .lines()
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n");
        let inner = inner.trim_end_matches('`').trim_end();
        (inner.to_string(), if detected_lang.is_empty() { lang.to_string() } else { detected_lang.to_string() })
    } else {
        (trimmed.to_string(), lang.to_string())
    };

    // Apply cleaning passes — always strip doc comments and trailing whitespace
    let content = code_strip_doc_block_comments(&content, &lang_tag);
    let content = code_strip_line_doc_comments(&content, &lang_tag);
    let content = code_strip_trailing_comments(&content, &lang_tag);
    let content = code_strip_trailing_whitespace(&content);
    let content = code_collapse_imports(&content, &lang_tag);
    // In aggressive mode: also strip test modules, decorators
    let content = if aggressive {
        let content = code_strip_test_modules(&content, &lang_tag);
        
        code_strip_decorators(&content, &lang_tag)
    } else {
        content
    };
    let content = code_collapse_getters_setters(&content, &lang_tag);
    let content = collapse_blank_lines(&content, if aggressive { 1 } else { 2 });

    format!("```{lang_tag}\n{content}\n```")
}

/// Remove block doc comments: /** ... */ and /*! ... */
fn code_strip_doc_block_comments(s: &str, _lang: &str) -> String {
    let mut out = Vec::new();
    let mut in_doc_block = false;

    for line in s.lines() {
        let trimmed = line.trim();
        if in_doc_block {
            if trimmed.contains("*/") {
                in_doc_block = false;
            }
            // Drop the line
            continue;
        }
        // Detect start of block doc comment
        if trimmed.starts_with("/**") || trimmed.starts_with("/*!") {
            // Single-line: /** comment */
            if trimmed.ends_with("*/") && trimmed.len() > 4 {
                continue; // single-line doc block -- drop
            }
            in_doc_block = true;
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

/// Remove single-line doc comments (///, //!, #-style for Python)
fn code_strip_line_doc_comments(s: &str, lang: &str) -> String {
    match lang {
        "rust" => {
            s.lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.starts_with("///") && !t.starts_with("//!")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "python" => {
            // Strip standalone comment lines (not inline comments)
            s.lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.starts_with('#')
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "typescript" | "javascript" | "js" | "ts" | "go" | "java" | "kotlin"
        | "c" | "cpp" | "csharp" | "swift" => {
            // Strip standalone // comment lines
            s.lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.starts_with("//")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => s.to_string(),
    }
}

/// Strip trailing whitespace from every line.
fn code_strip_trailing_whitespace(s: &str) -> String {
    s.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove trailing inline comments from code lines.
/// Guards against stripping `//` inside string literals (simple heuristic).
fn code_strip_trailing_comments(s: &str, lang: &str) -> String {
    let comment_prefix = match lang {
        "python" | "ruby" | "bash" | "sh" | "yaml" => "#",
        "rust" | "javascript" | "typescript" | "js" | "ts"
        | "go" | "java" | "c" | "cpp" | "csharp" | "swift" | "kotlin" => "//",
        _ => return s.to_string(), // unknown lang -- don't touch
    };

    s.lines()
        .map(|line| strip_trailing_comment_from_line(line, comment_prefix))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_trailing_comment_from_line<'a>(line: &'a str, prefix: &str) -> &'a str {
    // Don't strip from blank or comment-only lines
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(prefix) {
        return line;
    }

    let prefix_bytes = prefix.as_bytes();
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut string_char = b'"';
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2; // skip escaped char
                continue;
            }
            if b == string_char {
                in_string = false;
            }
        } else if b == b'"' || b == b'\'' || b == b'`' {
            in_string = true;
            string_char = b;
        } else if bytes[i..].starts_with(prefix_bytes) {
            // Found comment start outside string -- strip and trim trailing whitespace
            return line[..i].trim_end();
        }
        i += 1;
    }
    line
}

/// Collapse import/use/from blocks of >= 3 consecutive lines into a summary.
fn code_collapse_imports(s: &str, lang: &str) -> String {
    let import_prefix: &[&str] = match lang {
        "rust" => &["use "],
        "python" => &["import ", "from "],
        "typescript" | "javascript" | "js" | "ts" => &["import "],
        "go" | "java" | "kotlin" => &["import "],
        _ => return s.to_string(),
    };

    let lines: Vec<&str> = s.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Check if this starts an import block
        let line = lines[i];
        let is_import = import_prefix.iter().any(|p| line.trim_start().starts_with(p));

        if is_import {
            // Collect the full run
            let start = i;
            while i < lines.len()
                && import_prefix
                    .iter()
                    .any(|p| lines[i].trim_start().starts_with(p))
            {
                i += 1;
            }
            let count = i - start;
            if count >= 3 {
                // Extract module names for display (up to 4)
                let names: Vec<&str> = lines[start..i]
                    .iter()
                    .take(4)
                    .map(|l| {
                        // Extract first identifier after the keyword
                        let after_kw = import_prefix
                            .iter()
                            .find_map(|p| l.trim_start().strip_prefix(p))
                            .unwrap_or(l.trim_start());
                        after_kw
                            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != ':')
                            .next()
                            .unwrap_or("")
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                let names_str = names.join(", ");
                let ellipsis = if count > 4 { ", ..." } else { "" };
                out.push(format!(
                    "// [{count} imports: {names_str}{ellipsis}]"
                ));
            } else {
                // Too few -- emit as-is
                for l in &lines[start..i] {
                    out.push(l.to_string());
                }
            }
        } else {
            out.push(line.to_string());
            i += 1;
        }
    }
    out.join("\n")
}

/// Remove #[cfg(test)] modules in Rust (aggressive only).
fn code_strip_test_modules(s: &str, lang: &str) -> String {
    if lang != "rust" {
        return s.to_string();
    }
    let lines: Vec<&str> = s.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        // Detect #[cfg(test)] followed by mod tests { ... }
        if trimmed == "#[cfg(test)]" {
            // Skip everything until the matching closing brace at the same indent level
            let base_indent = lines[i].len() - trimmed.len();
            let mut depth = 0i32;
            let mut found_mod = false;
            let mut j = i + 1;
            while j < lines.len() {
                let lt = lines[j].trim();
                if !found_mod && lt.starts_with("mod ") {
                    found_mod = true;
                }
                depth += lt.chars().filter(|c| *c == '{').count() as i32;
                depth -= lt.chars().filter(|c| *c == '}').count() as i32;
                j += 1;
                if found_mod && depth <= 0 {
                    break;
                }
            }
            if found_mod {
                out.push(format!("{}// [test module omitted by itk]", " ".repeat(base_indent)));
                i = j;
                continue;
            }
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    out.join("\n")
}

/// Strip #[derive(...)], @decorators, and similar attribute lines (aggressive only).
fn code_strip_decorators(s: &str, lang: &str) -> String {
    let prefix: &[&str] = match lang {
        "rust" => &["#[derive(", "#[allow(", "#[warn(", "#[serde("],
        "python" => &["@"],
        "typescript" | "javascript" | "ts" | "js" => &["@"],
        "java" | "kotlin" => &["@"],
        _ => return s.to_string(),
    };
    s.lines()
        .filter(|l| {
            let t = l.trim();
            !prefix.iter().any(|p| t.starts_with(p))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collapse getter/setter boilerplate in Java/TypeScript.
fn code_collapse_getters_setters(s: &str, lang: &str) -> String {
    match lang {
        "java" | "typescript" | "ts" | "javascript" | "js" => {}
        _ => return s.to_string(),
    }
    let re_getter = re_getter_setter();
    let lines: Vec<&str> = s.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut getter_names: Vec<String> = Vec::new();
    let mut getter_start: Option<usize> = None;

    while i < lines.len() {
        if re_getter.is_match(lines[i]) {
            if getter_start.is_none() {
                getter_start = Some(out.len());
            }
            // Extract property name from get/set method
            if let Some(name) = extract_accessor_name(lines[i]) {
                if !getter_names.contains(&name) {
                    getter_names.push(name);
                }
            }
            // Skip the method body (until closing brace at same indent)
            let mut depth = 0i32;
            loop {
                if i >= lines.len() { break; }
                depth += lines[i].chars().filter(|c| *c == '{').count() as i32;
                depth -= lines[i].chars().filter(|c| *c == '}').count() as i32;
                i += 1;
                if depth <= 0 { break; }
            }
        } else {
            // Flush accumulated getters/setters
            if getter_names.len() >= 3 {
                if let Some(start) = getter_start {
                    // Remove any already-added getter lines
                    out.truncate(start);
                }
                let names = if getter_names.len() > 4 {
                    format!("{}, ...", getter_names[..4].join(", "))
                } else {
                    getter_names.join(", ")
                };
                out.push(format!("  // [{} getters/setters: {}]", getter_names.len(), names));
            }
            getter_names.clear();
            getter_start = None;
            out.push(lines[i].to_string());
            i += 1;
        }
    }
    // Flush final batch
    if getter_names.len() >= 3 {
        if let Some(start) = getter_start {
            out.truncate(start);
        }
        let names = if getter_names.len() > 4 {
            format!("{}, ...", getter_names[..4].join(", "))
        } else {
            getter_names.join(", ")
        };
        out.push(format!("  // [{} getters/setters: {}]", getter_names.len(), names));
    }
    out.join("\n")
}

fn re_getter_setter() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)^\s+(?:public\s+|private\s+|protected\s+)?(?:get|set)\s+\w+\s*\(").unwrap()
    })
}

fn extract_accessor_name(line: &str) -> Option<String> {
    let re = re_accessor_name();
    re.captures(line).map(|c| c.get(1).unwrap().as_str().to_lowercase())
}

fn re_accessor_name() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)(?:get|set)\s+(\w+)\s*\(").unwrap()
    })
}
