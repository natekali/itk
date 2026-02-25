use crate::detect::{ContentType, StackTraceLang};
use crate::prompt;
use regex::Regex;
use std::sync::OnceLock;

pub struct CleanOptions<'a> {
    pub aggressive: bool,
    pub _diff_mode: bool,
    pub add_summary: bool,
    pub prompt_type: Option<&'a str>,
    pub content_type: ContentType,
}

/// Entry point: clean `input` according to `opts`.
/// Uses catch_unwind as a last resort — on any internal panic, returns input unchanged.
pub fn clean(input: &str, opts: &CleanOptions) -> String {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        clean_inner(input, opts)
    }));
    match result {
        Ok(s) => s,
        Err(_) => input.to_string(),
    }
}

fn clean_inner(input: &str, opts: &CleanOptions) -> String {
    let stripped = strip_ansi(input);

    let mut cleaned = match &opts.content_type {
        ContentType::StackTrace(lang) => clean_stack_trace(&stripped, lang, opts.aggressive),
        ContentType::GitDiff => clean_git_diff(&stripped, opts.aggressive),
        ContentType::LogFile => clean_log(&stripped, opts.aggressive),
        ContentType::Json => clean_json(&stripped),
        ContentType::Yaml => clean_yaml(&stripped),
        ContentType::Code(lang) => clean_code(&stripped, lang),
        ContentType::PlainText => clean_plain(&stripped),
    };

    if opts.add_summary {
        let summary = summarize(&cleaned, &opts.content_type);
        cleaned = format!("{summary}\n\n{cleaned}");
    }

    if let Some(pt) = opts.prompt_type {
        cleaned = prompt::wrap(&cleaned, pt, &opts.content_type);
    }

    cleaned
}

// ── ANSI ──────────────────────────────────────────────────────────────────────

fn strip_ansi(s: &str) -> String {
    strip_ansi_escapes::strip_str(s)
}

// ── STACK TRACES ─────────────────────────────────────────────────────────────

fn re_python_frame() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"^\s+File ".+", line \d+, in .+$"#).unwrap())
}

fn re_js_frame() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\s+at\s+.+$").unwrap())
}

fn re_rust_frame_numbered() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\s+\d+:\s+.+$").unwrap())
}

fn re_go_goroutine() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^goroutine \d+ \[").unwrap())
}

fn re_java_frame() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^\s+at\s+[\w\.$]+\([\w]+\.java:\d+\)$").unwrap()
    })
}

fn re_java_caused_by() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?:Caused by:|Exception in thread)").unwrap())
}

fn clean_stack_trace(s: &str, lang: &StackTraceLang, aggressive: bool) -> String {
    match lang {
        StackTraceLang::Python => clean_python_trace(s, aggressive),
        StackTraceLang::JavaScript => clean_js_trace(s, aggressive),
        StackTraceLang::Rust => clean_rust_trace(s, aggressive),
        StackTraceLang::Go => clean_go_trace(s, aggressive),
        StackTraceLang::Java => clean_java_trace(s, aggressive),
        StackTraceLang::Unknown => clean_plain(s),
    }
}

fn clean_python_trace(s: &str, aggressive: bool) -> String {
    let frame_limit: usize = if aggressive { 5 } else { 20 };
    let mut out = Vec::new();
    let mut in_traceback = false;
    let mut frame_count = 0usize;
    let mut last_was_frame_header = false;
    let mut truncated = false;

    for line in s.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("Traceback (most recent call last):") {
            out.push(line.to_string());
            in_traceback = true;
            frame_count = 0;
            truncated = false;
            last_was_frame_header = false;
            continue;
        }

        if in_traceback {
            if re_python_frame().is_match(line) {
                frame_count += 1;
                if frame_count <= frame_limit {
                    out.push(line.to_string());
                } else if !truncated {
                    out.push("  ... [frames truncated by itk]".to_string());
                    truncated = true;
                }
                last_was_frame_header = true;
                continue;
            }

            // The code line that follows a File/line header
            if last_was_frame_header {
                if frame_count <= frame_limit {
                    out.push(line.to_string());
                }
                last_was_frame_header = false;
                continue;
            }

            // Error message / end of traceback block
            if !trimmed.is_empty() {
                in_traceback = false;
                out.push(line.to_string());
            }
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

fn clean_js_trace(s: &str, aggressive: bool) -> String {
    let frame_limit: usize = if aggressive { 8 } else { 25 };
    let mut out = Vec::new();
    let mut frame_count = 0usize;
    let mut truncated = false;

    for line in s.lines() {
        if re_js_frame().is_match(line) {
            frame_count += 1;
            if frame_count <= frame_limit {
                out.push(shorten_path(line));
            } else if !truncated {
                out.push("    ... [frames truncated by itk]".to_string());
                truncated = true;
            }
        } else {
            out.push(line.to_string());
            if line.trim().is_empty() {
                frame_count = 0;
                truncated = false;
            }
        }
    }
    out.join("\n")
}

fn shorten_path(line: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let re = R.get_or_init(|| {
        Regex::new(r"(?:[A-Za-z]:[/\\]|/)(?:[^/\\():\s]+[/\\]){3,}").unwrap()
    });
    re.replace_all(line, ".../").into_owned()
}

fn clean_rust_trace(s: &str, aggressive: bool) -> String {
    let frame_limit: usize = if aggressive { 10 } else { 30 };
    let mut out = Vec::new();
    let mut in_backtrace = false;
    let mut frame_count = 0usize;
    let mut truncated = false;

    for line in s.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("stack backtrace:") || trimmed.starts_with("thread '") {
            out.push(line.to_string());
            in_backtrace = true;
            frame_count = 0;
            truncated = false;
            continue;
        }

        if in_backtrace {
            if re_rust_frame_numbered().is_match(line) {
                if aggressive && is_rust_internal_frame(line) {
                    continue;
                }
                frame_count += 1;
                if frame_count <= frame_limit {
                    out.push(line.to_string());
                } else if !truncated {
                    out.push("   ... [backtrace truncated by itk]".to_string());
                    truncated = true;
                }
            } else {
                out.push(line.to_string());
                if trimmed.is_empty() {
                    in_backtrace = false;
                }
            }
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

fn is_rust_internal_frame(line: &str) -> bool {
    let internal = [
        "std::", "core::", "alloc::", "backtrace::", "__rust_",
        "rust_begin_unwind", "rust_panic", "lang_start",
        "tokio::runtime::task", "tokio::runtime::blocking",
    ];
    internal.iter().any(|pat| line.contains(pat))
}

fn clean_go_trace(s: &str, aggressive: bool) -> String {
    let frame_limit: usize = if aggressive { 8 } else { 20 };
    let mut out = Vec::new();
    let mut in_goroutine = false;
    let mut frame_count = 0usize;

    for line in s.lines() {
        let trimmed = line.trim();

        if re_go_goroutine().is_match(line) {
            out.push(line.to_string());
            in_goroutine = true;
            frame_count = 0;
            continue;
        }

        if in_goroutine {
            // Go frames: function call line + file:line pair
            if !trimmed.is_empty() && (trimmed.ends_with(')') || trimmed.contains(".go:")) {
                frame_count += 1;
                if frame_count <= frame_limit {
                    out.push(line.to_string());
                }
            } else {
                out.push(line.to_string());
                if trimmed.is_empty() {
                    in_goroutine = false;
                }
            }
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

fn clean_java_trace(s: &str, aggressive: bool) -> String {
    let frame_limit: usize = if aggressive { 10 } else { 30 };
    let mut out = Vec::new();
    let mut frame_count = 0usize;
    let mut truncated = false;

    for line in s.lines() {
        let trimmed = line.trim();

        if re_java_caused_by().is_match(trimmed) {
            // New exception block — reset counters
            out.push(line.to_string());
            frame_count = 0;
            truncated = false;
            continue;
        }

        if re_java_frame().is_match(line) {
            if aggressive && is_java_internal(trimmed) {
                continue;
            }
            frame_count += 1;
            if frame_count <= frame_limit {
                out.push(line.to_string());
            } else if !truncated {
                out.push("\t... [frames truncated by itk]".to_string());
                truncated = true;
            }
        } else {
            out.push(line.to_string());
            if trimmed.is_empty() {
                frame_count = 0;
                truncated = false;
            }
        }
    }
    out.join("\n")
}

fn is_java_internal(line: &str) -> bool {
    let internal = ["java.lang.", "java.util.", "sun.", "com.sun.", "jdk."];
    internal.iter().any(|p| line.contains(p))
}

// ── GIT DIFF ──────────────────────────────────────────────────────────────────

fn re_diff_hunk() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^@@ .+ @@").unwrap())
}

fn re_diff_file_header() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?:diff --git|--- |\+\+\+ |index |Binary files)").unwrap())
}

fn clean_git_diff(s: &str, aggressive: bool) -> String {
    let keep: usize = if aggressive { 1 } else { 2 };
    let mut out: Vec<String> = Vec::new();
    let mut in_hunk = false;
    // Sliding window of context lines seen since the last +/- line.
    // When a new +/- line arrives: keep only the last `keep` lines as pre-context.
    // When the hunk ends: keep only the first `keep` lines as post-context.
    let mut ctx_window: Vec<String> = Vec::new();
    let mut last_was_change = false;

    let flush_pre_context = |out: &mut Vec<String>, ctx: &mut Vec<String>, keep: usize| {
        // Emit up to `keep` trailing lines from ctx as pre-change context
        if ctx.len() > keep {
            out.push(format!(" ... [{} context lines omitted]", ctx.len() - keep));
            let start = ctx.len() - keep;
            for l in ctx.drain(start..) {
                out.push(l);
            }
        } else {
            out.extend(ctx.drain(..));
        }
        ctx.clear();
    };

    let flush_post_context = |out: &mut Vec<String>, ctx: &mut Vec<String>, keep: usize| {
        // Emit up to `keep` leading lines from ctx as post-change context
        if ctx.len() > keep {
            for l in ctx.drain(..keep) {
                out.push(l);
            }
            out.push(format!(" ... [{} context lines omitted]", ctx.len()));
        } else {
            out.extend(ctx.drain(..));
        }
        ctx.clear();
    };

    for line in s.lines() {
        if re_diff_file_header().is_match(line) {
            if last_was_change {
                flush_post_context(&mut out, &mut ctx_window, keep);
            } else {
                ctx_window.clear();
            }
            out.push(line.to_string());
            in_hunk = false;
            last_was_change = false;
            continue;
        }

        if re_diff_hunk().is_match(line) {
            if last_was_change {
                flush_post_context(&mut out, &mut ctx_window, keep);
            } else {
                ctx_window.clear();
            }
            out.push(line.to_string());
            in_hunk = true;
            last_was_change = false;
            continue;
        }

        if !in_hunk {
            out.push(line.to_string());
            continue;
        }

        match line.chars().next() {
            Some('+') | Some('-') => {
                if last_was_change {
                    // Context lines between two change lines — emit as pre-context
                    flush_pre_context(&mut out, &mut ctx_window, keep);
                } else {
                    // First change after context — emit trailing context_window as pre-context
                    flush_pre_context(&mut out, &mut ctx_window, keep);
                }
                out.push(line.to_string());
                last_was_change = true;
            }
            Some(' ') => {
                if last_was_change {
                    // Post-change context — accumulate
                    ctx_window.push(line.to_string());
                } else {
                    // Pre-change context — accumulate (will be trimmed when change arrives)
                    ctx_window.push(line.to_string());
                }
            }
            _ => {
                // e.g. "\ No newline at end of file"
                if last_was_change {
                    flush_post_context(&mut out, &mut ctx_window, keep);
                } else {
                    ctx_window.clear();
                }
                out.push(line.to_string());
                last_was_change = false;
            }
        }
    }

    if last_was_change {
        flush_post_context(&mut out, &mut ctx_window, keep);
    } else {
        ctx_window.clear();
    }

    out.join("\n")
}

// ── LOG FILES ─────────────────────────────────────────────────────────────────

fn re_timestamp() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?"
        ).unwrap()
    })
}

fn re_hex_addr() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b0x[0-9a-fA-F]{4,}\b").unwrap())
}

fn re_long_num() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b\d{6,}\b").unwrap())
}

fn re_progress_bar() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?:\r[^\n]|\[={0,}[=> ]{0,}\]\s*\d+%|\d+%\s*\[)").unwrap()
    })
}

fn clean_log(s: &str, aggressive: bool) -> String {
    let max_repeated: usize = if aggressive { 1 } else { 3 };
    let mut out: Vec<String> = Vec::new();
    let mut last_normalized = String::new();
    let mut repeat_count = 0usize;
    let mut suppressed = 0usize;

    for line in s.lines() {
        // Skip progress-bar lines
        if re_progress_bar().is_match(line) {
            continue;
        }
        // Skip carriage-return-overwrite lines (terminal progress)
        if line.starts_with('\r') {
            continue;
        }

        let normalized = normalize_log_line(line);

        if normalized == last_normalized && !normalized.trim().is_empty() {
            repeat_count += 1;
            if repeat_count <= max_repeated {
                out.push(line.to_string());
            } else {
                suppressed += 1;
            }
        } else {
            if suppressed > 0 {
                out.push(format!("  [... {suppressed} identical lines suppressed]"));
                suppressed = 0;
            }
            out.push(line.to_string());
            last_normalized = normalized;
            repeat_count = 1;
        }
    }
    if suppressed > 0 {
        out.push(format!("  [... {suppressed} identical lines suppressed]"));
    }
    out.join("\n")
}

fn normalize_log_line(line: &str) -> String {
    let mut s = re_timestamp().replace_all(line, "TS").into_owned();
    s = re_hex_addr().replace_all(&s, "ADDR").into_owned();
    s = re_long_num().replace_all(&s, "NUM").into_owned();
    s
}

// ── JSON ──────────────────────────────────────────────────────────────────────

fn clean_json(s: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(val) => {
            let compacted = compact_json_arrays(val);
            serde_json::to_string_pretty(&compacted).unwrap_or_else(|_| s.to_string())
        }
        Err(_) => clean_plain(s),
    }
}

fn compact_json_arrays(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match val {
        Value::Array(arr) => {
            let all_primitive = arr.iter().all(|v| !v.is_array() && !v.is_object());
            if all_primitive && arr.len() > 20 {
                let len = arr.len();
                let preview: Vec<Value> = arr.into_iter().take(3).collect();
                let mut result = preview;
                result.push(Value::String(format!("... [{} more items]", len - 3)));
                Value::Array(result)
            } else {
                Value::Array(arr.into_iter().map(compact_json_arrays).collect())
            }
        }
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, compact_json_arrays(v))).collect())
        }
        other => other,
    }
}

// ── YAML ──────────────────────────────────────────────────────────────────────

fn clean_yaml(s: &str) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0u32;

    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank_run = 0;
        let leading = line.len() - line.trim_start().len();
        let normalized_indent = ((leading + 1) / 2) * 2;
        out.push(format!("{:indent$}{trimmed}", "", indent = normalized_indent));
    }
    out.join("\n")
}

// ── CODE ──────────────────────────────────────────────────────────────────────

fn clean_code(s: &str, lang: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with("```") {
        normalize_fenced_code(trimmed)
    } else {
        format!("```{lang}\n{}\n```", collapse_blank_lines(trimmed, 2))
    }
}

fn normalize_fenced_code(s: &str) -> String {
    collapse_blank_lines(s, 2)
}

fn collapse_blank_lines(s: &str, max_blanks: usize) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0usize;

    for line in s.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= max_blanks {
                out.push(String::new());
            }
        } else {
            blank_run = 0;
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

// ── PLAIN TEXT ────────────────────────────────────────────────────────────────

pub fn clean_plain(s: &str) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0usize;

    for line in s.lines() {
        let trimmed_end = line.trim_end();
        if trimmed_end.is_empty() {
            blank_run += 1;
            if blank_run <= 2 {
                out.push(String::new());
            }
        } else {
            blank_run = 0;
            out.push(trimmed_end.to_string());
        }
    }

    while out.first().map(|l: &String| l.is_empty()).unwrap_or(false) {
        out.remove(0);
    }
    while out.last().map(|l: &String| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

// ── SUMMARY ──────────────────────────────────────────────────────────────────

fn summarize(content: &str, ct: &ContentType) -> String {
    match ct {
        ContentType::StackTrace(lang) => {
            let error_line = content
                .lines()
                .rev()
                .find(|l| {
                    let t = l.trim();
                    !t.is_empty()
                        && !t.starts_with("at ")
                        && !t.starts_with("File ")
                        && !t.contains("truncated")
                })
                .unwrap_or("unknown error");
            format!("// Stack trace ({lang:?}) — root cause: {}", error_line.trim())
        }
        ContentType::GitDiff => {
            let files = content.lines().filter(|l| l.starts_with("diff --git")).count();
            let added: usize = content.lines().filter(|l| l.starts_with('+')).count();
            let removed: usize = content.lines().filter(|l| l.starts_with('-')).count();
            format!("// Git diff — {files} file(s), +{added}/-{removed} lines")
        }
        ContentType::LogFile => {
            let lines = content.lines().count();
            format!("// Log output — {lines} lines (cleaned by itk)")
        }
        ContentType::Json => "// JSON (compacted by itk)".to_string(),
        ContentType::Yaml => "// YAML config (compacted by itk)".to_string(),
        ContentType::Code(lang) => {
            let lines = content.lines().count();
            format!("// {lang} — {lines} lines")
        }
        ContentType::PlainText => {
            let words = content.split_whitespace().count();
            format!("// Plain text — ~{words} words")
        }
    }
}
