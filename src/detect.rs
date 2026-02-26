use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    StackTrace(StackTraceLang),
    GitDiff,
    LogFile,
    Json,
    Yaml,
    Code(String),
    PlainText,
}

impl ContentType {
    /// Human-readable label used in --stats header and gain dashboard.
    pub fn label(&self) -> String {
        match self {
            ContentType::StackTrace(l) => format!("trace/{l:?}").to_lowercase(),
            ContentType::GitDiff => "git-diff".to_string(),
            ContentType::LogFile => "log".to_string(),
            ContentType::Json => "json".to_string(),
            ContentType::Yaml => "yaml".to_string(),
            ContentType::Code(l) => format!("code/{l}"),
            ContentType::PlainText => "text".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StackTraceLang {
    Rust,
    Python,
    JavaScript,
    Go,
    Java,
    Unknown,
}

// ── Git diff ─────────────────────────────────────────────────────────────────

fn re_git_diff() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^(diff --git|--- a/|\+\+\+ b/)").unwrap())
}

// ── Stack traces ──────────────────────────────────────────────────────────────

fn re_python_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^Traceback \(most recent call last\):").unwrap())
}

fn re_go_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^goroutine \d+ \[").unwrap())
}

fn re_java_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?m)^\s+at\s+[\w\.$]+\([\w]+\.java:\d+\)").unwrap()
    })
}

/// JS/TS: matches "at X (..." OR "at async X (..." OR "at async X.Y (..."
/// Bug fix: old regex was `\S+\s+\(` which matched only ONE word before `(`.
/// "at async handleRequest (" has TWO words (async + handleRequest) before `(`.
fn re_js_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // Match: optional whitespace + "at" + one or more words/dots + "("
        // Covers: "at foo (", "at async foo (", "at Foo.bar (", "at Object.<anonymous> ("
        Regex::new(r"(?m)^\s+at\s+(?:async\s+)?[\w.<>\[\]$]+(?:\.[\w<>\[\]$]+)*\s+\(").unwrap()
    })
}

/// Rust: "   N: path::to::function" — numbered backtrace frames
/// Also catches "thread 'X' panicked" as strong Rust signal
fn re_rust_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Match numbered frames: "   3: some::path" (space, digits, colon, space, non-space)
    R.get_or_init(|| Regex::new(r"(?m)^\s{1,6}\d+:\s+\S").unwrap())
}

fn re_rust_panic() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^thread '.+' panicked at").unwrap())
}

// ── Log files ─────────────────────────────────────────────────────────────────

/// Broad log detection: ISO timestamps, bracketed levels, key=value levels, cargo/npm output
fn re_log_line() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?xm)
            # ISO-8601 date (full or partial)
            \d{4}-\d{2}-\d{2}[T\ ]\d{2}:\d{2}
            |
            # Bracketed log levels: [INFO], [ERROR], etc.
            \[(?:INFO|WARN(?:ING)?|ERROR|DEBUG|TRACE|FATAL)\]
            |
            # key=value log levels (logrus, zap, slog)
            (?:level|lvl|severity)=(?:info|warn|error|debug|trace|fatal)
            |
            # Prefix log levels (systemd, docker): INFO:, ERROR:
            ^(?:INFO|WARN|ERROR|DEBUG|TRACE|FATAL)[\s:]
            |
            # Cargo/npm build output lines
            ^(?:Compiling|Downloading|Downloaded|Checking|Finished|error\[E\d+\]|warning:)
        "
        ).unwrap()
    })
}

fn re_ansi() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\x1b\[[\d;]*[mGKHF]").unwrap())
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Detect the content type of `text`.
/// `force_diff`: bypass detection, return GitDiff (used with --diff flag).
/// `force_type`: override with explicit type name (used with --type flag).
pub fn detect(text: &str, force_diff: bool, force_type: Option<&str>) -> ContentType {
    if force_diff {
        return ContentType::GitDiff;
    }

    if let Some(t) = force_type {
        return parse_forced_type(t);
    }

    // Scan first 8 KB (increased from 4 KB for better log detection on pasted output)
    let sample = if text.len() > 8192 { &text[..8192] } else { text };

    // ── 1. Git diff — very distinctive markers ────────────────────────────────
    if re_git_diff().is_match(sample) {
        return ContentType::GitDiff;
    }

    // ── 2. Stack traces — priority order: most distinctive first ──────────────

    // Go: unique "goroutine N [" signature
    if re_go_trace().is_match(sample) {
        return ContentType::StackTrace(StackTraceLang::Go);
    }

    // Java: "at com.example.Foo(Foo.java:42)"
    if re_java_trace().is_match(sample) {
        return ContentType::StackTrace(StackTraceLang::Java);
    }

    // Python: starts with exact "Traceback (most recent call last):"
    if re_python_trace().is_match(sample) {
        return ContentType::StackTrace(StackTraceLang::Python);
    }

    // Rust: "thread '...' panicked" is a very strong single-line signal
    if re_rust_panic().is_match(sample) {
        return ContentType::StackTrace(StackTraceLang::Rust);
    }

    // JS/TS: needs >= 2 "at" frames (lowered from 3 — real traces often have 3-5 frames)
    let js_matches = re_js_trace().find_iter(sample).count();
    if js_matches >= 2 {
        return ContentType::StackTrace(StackTraceLang::JavaScript);
    }

    // Rust backtrace: numbered frames, needs >= 2
    let rust_matches = re_rust_trace().find_iter(sample).count();
    if rust_matches >= 2 {
        return ContentType::StackTrace(StackTraceLang::Rust);
    }

    // ── 3. Log file — lowered threshold to 3 (from 5) ────────────────────────
    let log_matches = re_log_line().find_iter(sample).count();
    if log_matches >= 3 {
        return ContentType::LogFile;
    }

    // ── 4. JSON ───────────────────────────────────────────────────────────────
    let trimmed = sample.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && lookslike_json(trimmed) {
        return ContentType::Json;
    }

    // ── 5. YAML ───────────────────────────────────────────────────────────────
    if lookslike_yaml(sample) {
        return ContentType::Yaml;
    }

    // ── 6. Code blocks ────────────────────────────────────────────────────────
    if let Some(lang) = detect_code_language(sample) {
        return ContentType::Code(lang);
    }

    // ── 7. ANSI escape codes → treat as log output ───────────────────────────
    if re_ansi().is_match(sample) {
        return ContentType::LogFile;
    }

    ContentType::PlainText
}

fn parse_forced_type(t: &str) -> ContentType {
    match t.to_lowercase().as_str() {
        "diff" | "git" | "patch" => ContentType::GitDiff,
        "log" | "logs" => ContentType::LogFile,
        "json" => ContentType::Json,
        "yaml" | "yml" => ContentType::Yaml,
        "rust" => ContentType::Code("rust".to_string()),
        "python" | "py" => ContentType::Code("python".to_string()),
        "js" | "javascript" => ContentType::Code("javascript".to_string()),
        "ts" | "typescript" => ContentType::Code("typescript".to_string()),
        "trace" | "stack" => ContentType::StackTrace(StackTraceLang::Unknown),
        _ => ContentType::PlainText,
    }
}

fn lookslike_json(s: &str) -> bool {
    s.contains('"') && (s.contains(':') || s.starts_with('['))
}

fn lookslike_yaml(s: &str) -> bool {
    if s.starts_with("---") {
        return true;
    }
    let kv_count = s
        .lines()
        .filter(|l| {
            let l = l.trim();
            if l.starts_with('#') || l.is_empty() {
                return false;
            }
            let mut parts = l.splitn(2, ':');
            let key = parts.next().unwrap_or("").trim();
            let val = parts.next();
            !key.contains(' ') && val.is_some()
        })
        .count();
    kv_count >= 4
}

fn detect_code_language(s: &str) -> Option<String> {
    // Existing markdown fence
    for line in s.lines().take(5) {
        let l = line.trim();
        if l.starts_with("```") {
            let lang = l.trim_start_matches('`').trim();
            if !lang.is_empty() {
                return Some(lang.to_string());
            }
        }
    }

    let head: Vec<&str> = s.lines().take(20).collect();
    let joined = head.join("\n");

    if joined.contains("fn ") && joined.contains("let ") {
        return Some("rust".to_string());
    }
    if joined.contains("def ") && joined.contains("import ") {
        return Some("python".to_string());
    }
    if joined.contains("function") || (joined.contains("const ") && joined.contains("=>")) {
        return Some("typescript".to_string());
    }
    if joined.contains("func ") && joined.contains("package ") {
        return Some("go".to_string());
    }
    if joined.contains("public class") || joined.contains("import java") {
        return Some("java".to_string());
    }
    if joined.contains("#include") || joined.contains("int main(") {
        return Some("c".to_string());
    }

    None
}
