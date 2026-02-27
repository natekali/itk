use crate::detect::{BuildTool, ContentType, StackTraceLang};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub struct CleanOptions {
    pub aggressive: bool,
    pub _diff_mode: bool,
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

    let cleaned = match &opts.content_type {
        ContentType::StackTrace(lang) => clean_stack_trace(&stripped, lang, opts.aggressive),
        ContentType::GitDiff => clean_git_diff(&stripped, opts.aggressive),
        ContentType::LogFile => clean_log(&stripped, opts.aggressive),
        ContentType::Json => clean_json(&stripped, opts.aggressive),
        ContentType::Yaml => clean_yaml(&stripped, opts.aggressive),
        ContentType::Code(lang) => clean_code(&stripped, lang, opts.aggressive),
        ContentType::BuildOutput(tool) => clean_build_output(&stripped, tool, opts.aggressive),
        ContentType::Markdown => clean_markdown(&stripped, opts.aggressive),
        ContentType::PlainText => clean_plain(&stripped),
    };

    // Note: summary/framing and prompt wrapping are now handled in main.rs
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

            if last_was_frame_header {
                if frame_count <= frame_limit {
                    out.push(line.to_string());
                }
                last_was_frame_header = false;
                continue;
            }

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
    let mut ctx_window: Vec<String> = Vec::new();
    let mut last_was_change = false;

    let flush_pre_context = |out: &mut Vec<String>, ctx: &mut Vec<String>, keep: usize| {
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
                flush_pre_context(&mut out, &mut ctx_window, keep);
                out.push(line.to_string());
                last_was_change = true;
            }
            Some(' ') => {
                ctx_window.push(line.to_string());
            }
            _ => {
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
        if re_progress_bar().is_match(line) {
            continue;
        }
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

// ── BUILD OUTPUT ──────────────────────────────────────────────────────────────

fn re_cargo_noise() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Lines that carry zero signal — drop entirely
    R.get_or_init(|| {
        Regex::new(
            r"(?x)^\ {0,3}(?:Compiling|Checking|Downloading|Downloaded|Fresh|Updating|Locking|Blocking|Fetching)\s"
        ).unwrap()
    })
}

fn re_cargo_error_loc() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // "  --> src/main.rs:42:5" location lines
    R.get_or_init(|| Regex::new(r"^\s+-->\s+").unwrap())
}

fn re_cargo_pipe() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // "   |" gutter lines used for code context in error messages
    R.get_or_init(|| Regex::new(r"^\s+\|\s*").unwrap())
}

fn re_tsc_error_line() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^(.+\.tsx?)\((\d+),(\d+)\): (error|warning) (TS\d+): (.+)$").unwrap()
    })
}

fn clean_build_output(s: &str, tool: &BuildTool, aggressive: bool) -> String {
    match tool {
        BuildTool::Cargo => clean_cargo_output(s, aggressive),
        BuildTool::TypeScript => clean_tsc_output(s, aggressive),
        BuildTool::Eslint => clean_eslint_output(s, aggressive),
        BuildTool::Generic => clean_log(s, aggressive),
    }
}

fn clean_cargo_output(s: &str, _aggressive: bool) -> String {
    // Pass 1: collect errors/warnings, drop noise
    // Group by file: BTreeMap<file_path, Vec<message>>
    let mut errors_by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut warnings_by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut finish_line: Option<String> = None;
    let mut error_summary: Option<String> = None;

    // State for grouping: current error/warning being accumulated
    let mut current_file = String::new();
    let mut current_msg: Vec<String> = Vec::new();
    let mut current_is_error = false;
    let mut in_diagnostic = false;

    let flush = |file: &str, msg: &[String], is_error: bool,
                  errors: &mut BTreeMap<String, Vec<String>>,
                  warnings: &mut BTreeMap<String, Vec<String>>| {
        if msg.is_empty() || file.is_empty() {
            return;
        }
        let joined = msg.join(" ").trim().to_string();
        if is_error {
            errors.entry(file.to_string()).or_default().push(joined);
        } else {
            warnings.entry(file.to_string()).or_default().push(joined);
        }
    };

    for line in s.lines() {
        let trimmed = line.trim();

        // Drop pure noise lines
        if re_cargo_noise().is_match(line) {
            continue;
        }

        // Finish / could not compile
        if trimmed.starts_with("Finished ") || trimmed.starts_with("error: could not compile") {
            flush(&current_file, &current_msg, current_is_error,
                  &mut errors_by_file, &mut warnings_by_file);
            current_msg.clear();
            in_diagnostic = false;
            if trimmed.starts_with("Finished ") {
                finish_line = Some(line.trim().to_string());
            } else {
                error_summary = Some(line.trim().to_string());
            }
            continue;
        }

        // New error[Exxxx] line
        if trimmed.starts_with("error[") || trimmed.starts_with("error: ") {
            flush(&current_file, &current_msg, current_is_error,
                  &mut errors_by_file, &mut warnings_by_file);
            current_msg.clear();
            current_is_error = true;
            in_diagnostic = true;
            current_file = String::new();
            current_msg.push(trimmed.to_string());
            continue;
        }

        // New warning line
        if trimmed.starts_with("warning:") && in_diagnostic {
            flush(&current_file, &current_msg, current_is_error,
                  &mut errors_by_file, &mut warnings_by_file);
            current_msg.clear();
            current_is_error = false;
            current_file = String::new();
            current_msg.push(trimmed.to_string());
            continue;
        }

        if in_diagnostic {
            // Location line: "  --> src/main.rs:42:5"
            if re_cargo_error_loc().is_match(line) {
                // Extract file path from "--> path:line:col"
                if let Some(path_part) = trimmed.strip_prefix("-->").map(|s| s.trim()) {
                    // Take just the file (before the first colon after the path)
                    let file_path = path_part
                        .split(':')
                        .next()
                        .unwrap_or(path_part)
                        .trim()
                        .to_string();
                    current_file = file_path;
                }
                continue;
            }
            // Pipe/gutter lines for code context — skip for brevity
            if re_cargo_pipe().is_match(line) {
                continue;
            }
        }
    }
    // Flush last diagnostic
    flush(&current_file, &current_msg, current_is_error,
          &mut errors_by_file, &mut warnings_by_file);

    // Render
    let mut out: Vec<String> = Vec::new();

    let total_errors: usize = errors_by_file.values().map(|v| v.len()).sum();
    let total_warnings: usize = warnings_by_file.values().map(|v| v.len()).sum();

    // Summary header
    out.push(format!(
        "// Build: {} error(s), {} warning(s)",
        total_errors, total_warnings
    ));

    // Errors grouped by file
    for (file, msgs) in &errors_by_file {
        let file_display = if file.is_empty() { "(unknown)" } else { file.as_str() };
        out.push(format!("\n{} ({} error(s)):", file_display, msgs.len()));
        for m in msgs {
            out.push(format!("  {m}"));
        }
    }

    // Warnings grouped by file (non-aggressive: show warnings too)
    if !_aggressive || total_errors == 0 {
        for (file, msgs) in &warnings_by_file {
            let file_display = if file.is_empty() { "(unknown)" } else { file.as_str() };
            out.push(format!("\n{} ({} warning(s)):", file_display, msgs.len()));
            for m in msgs {
                out.push(format!("  {m}"));
            }
        }
    }

    // Status lines
    if let Some(fin) = &finish_line {
        out.push(format!("\n{fin}"));
    }
    if let Some(err) = &error_summary {
        out.push(format!("{err}"));
    }

    out.join("\n")
}

fn clean_tsc_output(s: &str, _aggressive: bool) -> String {
    // Group errors by TS error code: BTreeMap<code, Vec<(file, line, message)>>
    let re = re_tsc_error_line();
    let mut by_code: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut other_lines: Vec<String> = Vec::new();

    for line in s.lines() {
        if let Some(caps) = re.captures(line) {
            let file = caps.get(1).map_or("", |m| m.as_str());
            let ln = caps.get(2).map_or("", |m| m.as_str());
            let code = caps.get(5).map_or("TS?", |m| m.as_str()).to_string();
            let msg = caps.get(6).map_or("", |m| m.as_str());
            by_code
                .entry(code)
                .or_default()
                .push(format!("{}:{} — {}", file, ln, msg));
        } else if !line.trim().is_empty() {
            other_lines.push(line.to_string());
        }
    }

    let total: usize = by_code.values().map(|v| v.len()).sum();
    let mut out = vec![format!("// TypeScript: {} error(s)", total)];

    for (code, occurrences) in &by_code {
        let count = occurrences.len();
        if count == 1 {
            out.push(format!("\n{code}: {}", occurrences[0]));
        } else {
            // Show first message as representative, list all locations
            let first_msg = occurrences[0]
                .split(" — ")
                .nth(1)
                .unwrap_or(&occurrences[0]);
            out.push(format!("\n{code} ({count}×): {first_msg}"));
            for loc in occurrences {
                let location = loc.split(" — ").next().unwrap_or(loc);
                out.push(format!("  {location}"));
            }
        }
    }

    for l in &other_lines {
        out.push(l.clone());
    }

    out.join("\n")
}

fn clean_eslint_output(s: &str, aggressive: bool) -> String {
    // ESLint output: file path lines followed by "  line:col  error/warning  msg  rule"
    // Group violations by rule
    let re_violation = re_eslint_violation();
    let mut by_rule: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut current_file = String::new();
    let mut summary_line: Option<String> = None;

    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Summary line (last line): "✖ 42 problems (30 errors, 12 warnings)"
        if trimmed.starts_with('✖') || trimmed.starts_with('✓') || trimmed.contains("problem") {
            summary_line = Some(trimmed.to_string());
            continue;
        }
        // File path line (no leading whitespace, ends with .js/.ts/.tsx etc.)
        if !line.starts_with(' ') && !line.starts_with('\t') {
            current_file = trimmed.to_string();
            continue;
        }
        // Violation line
        if let Some(caps) = re_violation.captures(line) {
            let severity = caps.get(3).map_or("", |m| m.as_str());
            let rule = caps.get(5).map_or("unknown", |m| m.as_str()).to_string();
            let lnum = caps.get(1).map_or("?", |m| m.as_str());
            let msg = caps.get(4).map_or("", |m| m.as_str());
            if aggressive && severity == "warning" {
                continue; // drop warnings in aggressive mode
            }
            by_rule
                .entry(rule)
                .or_default()
                .push(format!("{}:{} {}", current_file, lnum, msg));
        }
    }

    let total: usize = by_rule.values().map(|v| v.len()).sum();
    let mut out = vec![format!("// ESLint: {} violation(s)", total)];
    for (rule, locs) in &by_rule {
        let count = locs.len();
        if count == 1 {
            out.push(format!("  {rule}: {}", locs[0]));
        } else {
            out.push(format!("  {rule} ({count}×):"));
            for loc in locs.iter().take(3) {
                out.push(format!("    {loc}"));
            }
            if count > 3 {
                out.push(format!("    ... {} more", count - 3));
            }
        }
    }
    if let Some(s) = summary_line {
        out.push(format!("\n{s}"));
    }
    out.join("\n")
}

fn re_eslint_violation() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // "  12:5  error  message text  rule/name"
        Regex::new(r"^\s+(\d+):(\d+)\s+(error|warning)\s+(.+?)\s{2,}([\w/@-]+(?:/[\w-]+)*)\s*$")
            .unwrap()
    })
}

// ── JSON ──────────────────────────────────────────────────────────────────────

fn clean_json(s: &str, aggressive: bool) -> String {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(val) => {
            let val = json_extract_error_context(val);
            let val = json_prune_empty(val);
            if aggressive {
                let val = json_strip_metadata_keys(val);
                let val = json_truncate_long_strings(val);
                let val = json_round_floats(val);
                let val = json_dedup_array_objects(val, 1);
                let val = json_collapse_single_child_paths(val);
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| s.to_string())
            } else {
                let val = json_truncate_long_strings(val);
                let val = json_dedup_array_objects(val, 2);
                let val = json_collapse_single_child_paths(val);
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| s.to_string())
            }
        }
        Err(_) => clean_plain(s),
    }
}

/// If root object has error-signal keys, keep only those + id fields.
fn json_extract_error_context(val: serde_json::Value) -> serde_json::Value {
    use serde_json::{Map, Value};
    const ERROR_KEYS: &[&str] = &[
        "error", "errors", "message", "code", "status",
        "detail", "details", "description", "trace", "stack",
    ];
    const ID_KEYS: &[&str] = &["id", "request_id", "trace_id", "correlation_id"];

    if let Value::Object(ref map) = val {
        let has_error = map.keys().any(|k| ERROR_KEYS.contains(&k.as_str()));
        if has_error {
            let mut extracted: Map<String, Value> = map
                .iter()
                .filter(|(k, _)| {
                    ERROR_KEYS.contains(&k.as_str()) || ID_KEYS.contains(&k.as_str())
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let omitted = map.len().saturating_sub(extracted.len());
            if omitted > 0 {
                extracted.insert(
                    "_itk_omitted".to_string(),
                    Value::String(format!("{omitted} non-error fields omitted")),
                );
            }
            return Value::Object(extracted);
        }
    }
    val
}

/// Remove null values and empty arrays/objects recursively.
fn json_prune_empty(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match val {
        Value::Object(map) => {
            let pruned: serde_json::Map<String, Value> = map
                .into_iter()
                .filter(|(_, v)| match v {
                    Value::Null => false,
                    Value::Array(a) if a.is_empty() => false,
                    Value::Object(o) if o.is_empty() => false,
                    _ => true,
                })
                .map(|(k, v)| (k, json_prune_empty(v)))
                .collect();
            Value::Object(pruned)
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(json_prune_empty).collect())
        }
        other => other,
    }
}

/// If an array contains N objects all sharing the same key schema, show only
/// `show_count` examples plus a count marker.
fn json_dedup_array_objects(val: serde_json::Value, show_count: usize) -> serde_json::Value {
    use serde_json::Value;
    use std::collections::BTreeSet;

    match val {
        Value::Array(arr) if arr.len() > show_count + 1 => {
            // Check if all elements are objects with identical key sets
            let schemas: Vec<BTreeSet<String>> = arr
                .iter()
                .filter_map(|v| {
                    v.as_object()
                        .map(|o| o.keys().cloned().collect::<BTreeSet<_>>())
                })
                .collect();
            let all_objects = schemas.len() == arr.len();
            let all_same_schema = all_objects
                && !schemas.is_empty()
                && schemas.windows(2).all(|w| w[0] == w[1]);

            if all_same_schema {
                let total = arr.len();
                let shown = show_count.min(total);
                let mut result: Vec<Value> = arr
                    .into_iter()
                    .take(shown)
                    .map(|v| json_dedup_array_objects(v, show_count))
                    .collect();
                let hidden = total - shown;
                if hidden > 0 {
                    result.push(Value::String(format!(
                        "... {hidden} more objects with same structure"
                    )));
                }
                Value::Array(result)
            } else {
                // Recurse into elements but keep all
                Value::Array(
                    arr.into_iter()
                        .map(|v| json_dedup_array_objects(v, show_count))
                        .collect(),
                )
            }
        }
        Value::Array(arr) => {
            // Also compact primitive arrays > 20 (original behaviour)
            let all_primitive = arr.iter().all(|v| !v.is_array() && !v.is_object());
            if all_primitive && arr.len() > 20 {
                let len = arr.len();
                let mut result: Vec<Value> = arr.into_iter().take(3).collect();
                result.push(Value::String(format!("... [{} more items]", len - 3)));
                Value::Array(result)
            } else {
                Value::Array(
                    arr.into_iter()
                        .map(|v| json_dedup_array_objects(v, show_count))
                        .collect(),
                )
            }
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_dedup_array_objects(v, show_count)))
                .collect(),
        ),
        other => other,
    }
}

/// Collapse single-child object chains: {"a": {"b": {"c": 42}}} → {"a.b.c": 42}
fn json_collapse_single_child_paths(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match val {
        Value::Object(map) => {
            // Recurse first
            let map: serde_json::Map<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, json_collapse_single_child_paths(v)))
                .collect();

            // If this object has exactly one key and its value is also a single-key object,
            // merge the key paths
            if map.len() == 1 {
                let (k, v) = map.into_iter().next().unwrap();
                if let Value::Object(ref inner) = v {
                    if inner.len() == 1 {
                        let (ik, iv) = inner.clone().into_iter().next().unwrap();
                        let merged = format!("{k}.{ik}");
                        let mut result = serde_json::Map::new();
                        result.insert(merged, iv);
                        return Value::Object(result);
                    }
                }
                let mut result = serde_json::Map::new();
                result.insert(k, v);
                return Value::Object(result);
            }
            Value::Object(map)
        }
        Value::Array(arr) => {
            Value::Array(
                arr.into_iter()
                    .map(json_collapse_single_child_paths)
                    .collect(),
            )
        }
        other => other,
    }
}

/// Truncate string values > 200 chars (removes base64 blobs, long descriptions).
fn json_truncate_long_strings(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match val {
        Value::String(s) if s.len() > 200 => {
            Value::String(format!("{}...[{} chars]", &s[..100], s.len()))
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_truncate_long_strings(v)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.into_iter().map(json_truncate_long_strings).collect(),
        ),
        other => other,
    }
}

/// Remove common metadata/noise keys (HAL, OData, GraphQL __typename, timestamps).
fn json_strip_metadata_keys(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    const NOISE_KEYS: &[&str] = &[
        "_links", "_embedded", "@odata.context", "@odata.type",
        "$schema", "__typename", "createdAt", "updatedAt",
        "created_at", "updated_at", "modified_at", "modifiedAt",
    ];
    match val {
        Value::Object(map) => {
            let filtered: serde_json::Map<String, Value> = map
                .into_iter()
                .filter(|(k, _)| !NOISE_KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k, json_strip_metadata_keys(v)))
                .collect();
            Value::Object(filtered)
        }
        Value::Array(arr) => Value::Array(
            arr.into_iter().map(json_strip_metadata_keys).collect(),
        ),
        other => other,
    }
}

/// Round float values to 2 decimal places.
fn json_round_floats(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match val {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f.fract() != 0.0 {
                    let rounded = (f * 100.0).round() / 100.0;
                    return Value::Number(
                        serde_json::Number::from_f64(rounded).unwrap_or(n)
                    );
                }
            }
            Value::Number(n)
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_round_floats(v)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.into_iter().map(json_round_floats).collect(),
        ),
        other => other,
    }
}

// ── YAML ──────────────────────────────────────────────────────────────────────

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

fn clean_yaml(s: &str, aggressive: bool) -> String {
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
                // Block scalar ended — insert truncation note if needed
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
                out.push(format!("status: # [omitted by itk]"));
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

// ── CODE ──────────────────────────────────────────────────────────────────────

fn clean_code(s: &str, lang: &str, aggressive: bool) -> String {
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

    // Apply cleaning passes
    let content = code_strip_doc_block_comments(&content, &lang_tag);
    let content = code_strip_trailing_comments(&content, &lang_tag);
    let content = code_collapse_imports(&content, &lang_tag);
    // In aggressive mode: strip doc comments, test modules, decorators
    let content = if aggressive {
        let content = code_strip_line_doc_comments(&content, &lang_tag);
        let content = code_strip_test_modules(&content, &lang_tag);
        let content = code_strip_decorators(&content, &lang_tag);
        content
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
                continue; // single-line doc block — drop
            }
            in_doc_block = true;
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

/// Remove single-line doc comments (///, //!) — only in aggressive mode for Rust
fn code_strip_line_doc_comments(s: &str, lang: &str) -> String {
    if lang != "rust" {
        return s.to_string();
    }
    s.lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("///") && !t.starts_with("//!")
        })
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
        _ => return s.to_string(), // unknown lang — don't touch
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
        } else {
            if b == b'"' || b == b'\'' || b == b'`' {
                in_string = true;
                string_char = b;
            } else if bytes[i..].starts_with(prefix_bytes) {
                // Found comment start outside string — strip and trim trailing whitespace
                return line[..i].trim_end();
            }
        }
        i += 1;
    }
    line
}

/// Collapse import/use/from blocks of ≥ 4 consecutive lines into a summary.
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
            if count >= 4 {
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
                // Too few — emit as-is
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
            let _base_indent = lines[i].len() - lines[i].trim_start().len();
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
            if getter_names.len() >= 4 {
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
    if getter_names.len() >= 4 {
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

// ── MARKDOWN ──────────────────────────────────────────────────────────────────

const NOISE_SECTIONS: &[&str] = &[
    "installation",
    "getting started",
    "prerequisites",
    "requirements",
    "contributing",
    "contributors",
    "code of conduct",
    "license",
    "changelog",
    "releases",
    "roadmap",
    "acknowledgements",
    "acknowledgments",
    "credits",
    "sponsor",
];

fn clean_markdown(s: &str, aggressive: bool) -> String {
    // Pass 1: remove HTML comments <!-- ... -->
    let s = re_html_comment()
        .replace_all(s, "")
        .into_owned();

    let lines: Vec<&str> = s.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut skip_section = false;
    let mut skip_section_level = 0usize;

    for line in &lines {
        let trimmed = line.trim();

        // Count leading #s for heading level
        let heading_level = if trimmed.starts_with('#') {
            trimmed.chars().take_while(|c| *c == '#').count()
        } else {
            0
        };

        // End of a skipped section: new heading at same or higher level
        if skip_section && heading_level > 0 && heading_level <= skip_section_level {
            skip_section = false;
        }

        if skip_section {
            continue;
        }

        // Aggressive: skip known noise sections
        if aggressive && heading_level >= 2 {
            let heading_text = trimmed
                .trim_start_matches('#')
                .trim()
                .to_lowercase();
            if NOISE_SECTIONS.iter().any(|s| heading_text.starts_with(s)) {
                skip_section = true;
                skip_section_level = heading_level;
                continue;
            }
        }

        // Skip badge lines: standalone lines that are only image/badge markdown
        // Covers: ![alt](url), [![alt](img)](link), [![alt](img)](link)
        if is_badge_line(trimmed) {
            continue;
        }

        // Convert H3+ headings to bold bullet points
        if heading_level >= 3 {
            let heading_text = trimmed.trim_start_matches('#').trim();
            out.push(format!("- **{heading_text}**"));
            continue;
        }

        out.push(line.to_string());
    }

    // Collapse excessive blank lines
    collapse_blank_lines(&out.join("\n"), 1)
}

fn is_badge_line(line: &str) -> bool {
    // A badge line is one where the entire trimmed content consists of
    // markdown image/link patterns. Heuristic: line contains "![" or starts with
    // "[![" and has no plain text content outside of markdown link syntax.
    // Patterns:
    //   ![alt](url)
    //   [![alt](img_url)](link_url)
    //   Multiple badges on one line
    // Note: strip_ansi_escapes may convert [![ to \![, so check both forms.
    if !line.contains("![") && !line.contains("\\![") {
        return false;
    }
    // Normalise escaped bangs from ANSI stripping before matching
    let normalised = line.replace("\\!", "!");
    // Remove all markdown image-link patterns and check nothing substantive remains
    let cleaned = re_badge_pattern()
        .replace_all(&normalised, "")
        .trim()
        .to_string();
    cleaned.is_empty()
}

fn re_badge_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Matches: [![alt](img)](link) or ![alt](url)
    R.get_or_init(|| {
        Regex::new(r#"\[?!\[[^\]]*\]\([^)]*\)\]?(?:\([^)]*\))?"#).unwrap()
    })
}

fn re_html_comment() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").unwrap())
}

// ── PLAIN TEXT ────────────────────────────────────────────────────────────────

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

// Keep this to avoid breaking normalize_fenced_code call if needed elsewhere
#[allow(dead_code)]
fn _normalize_fenced_code_unused(s: &str) -> String {
    normalize_fenced_code(s)
}
