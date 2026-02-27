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
    BuildOutput(BuildTool),
    Markdown,
    Html,
    Sql,
    Csv,
    Dockerfile,
    EnvFile,
    Terraform,
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
            ContentType::BuildOutput(t) => format!("build/{t:?}").to_lowercase(),
            ContentType::Markdown => "markdown".to_string(),
            ContentType::Html => "html".to_string(),
            ContentType::Sql => "sql".to_string(),
            ContentType::Csv => "csv".to_string(),
            ContentType::Dockerfile => "dockerfile".to_string(),
            ContentType::EnvFile => "env".to_string(),
            ContentType::Terraform => "terraform".to_string(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BuildTool {
    Cargo,
    TypeScript,
    Eslint,
    Generic,
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
fn re_js_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?m)^\s+at\s+(?:async\s+)?[\w.<>\[\]$]+(?:\.[\w<>\[\]$]+)*\s+\(").unwrap()
    })
}

/// Rust: "   N: path::to::function" — numbered backtrace frames
fn re_rust_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^\s{1,6}\d+:\s+\S").unwrap())
}

fn re_rust_panic() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^thread '.+' panicked at").unwrap())
}

// ── Build output ──────────────────────────────────────────────────────────────

fn re_cargo_build() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?xm)
            # Cargo build lines (leading spaces are part of cargo's format)
            ^\ {0,3}(?:Compiling|Checking|Downloading|Downloaded|Fresh|Updating|Locking)\s
            |
            # Cargo error/warning markers
            ^error\[E\d+\]
            |
            ^warning:\
            |
            # Cargo finish / could not compile
            ^(?:Finished\ (?:dev|release|test)|error:\ could\ not\ compile)
        "
        ).unwrap()
    })
}

fn re_tsc_error() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?m)\w+\.tsx?\(\d+,\d+\): (?:error|warning) TS\d+:").unwrap()
    })
}

fn re_eslint_error() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // ESLint: "  12:5  error  message  rule/name" lines after a file path
        Regex::new(r"(?m)^\s+\d+:\d+\s+(?:error|warning)\s+\S").unwrap()
    })
}

// ── Log files ─────────────────────────────────────────────────────────────────

/// Broad log detection: ISO timestamps, bracketed levels, key=value levels.
/// NOTE: Cargo build patterns removed — those are now BuildOutput.
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

    // Scan first ~8 KB for performance on large inputs
    // Find a valid UTF-8 char boundary near 8192
    let sample = if text.len() > 8192 {
        let mut end = 8192;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    } else {
        text
    };

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

    // JS/TS: needs >= 2 "at" frames
    let js_matches = re_js_trace().find_iter(sample).count();
    if js_matches >= 2 {
        return ContentType::StackTrace(StackTraceLang::JavaScript);
    }

    // Rust backtrace: numbered frames, needs >= 2
    let rust_matches = re_rust_trace().find_iter(sample).count();
    if rust_matches >= 2 {
        return ContentType::StackTrace(StackTraceLang::Rust);
    }

    // ── 3. Build output (BEFORE log — cargo patterns were previously misdetected) ─
    let cargo_matches = re_cargo_build().find_iter(sample).count();
    if cargo_matches >= 2 {
        return ContentType::BuildOutput(BuildTool::Cargo);
    }
    let tsc_matches = re_tsc_error().find_iter(sample).count();
    if tsc_matches >= 1 {
        return ContentType::BuildOutput(BuildTool::TypeScript);
    }
    let eslint_matches = re_eslint_error().find_iter(sample).count();
    if eslint_matches >= 3 {
        return ContentType::BuildOutput(BuildTool::Eslint);
    }

    // ── 4. Log file ───────────────────────────────────────────────────────────
    let log_matches = re_log_line().find_iter(sample).count();
    if log_matches >= 3 {
        return ContentType::LogFile;
    }

    // ── 5. JSON ───────────────────────────────────────────────────────────────
    let trimmed = sample.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && lookslike_json(trimmed) {
        return ContentType::Json;
    }

    // ── 6. YAML ───────────────────────────────────────────────────────────────
    if lookslike_yaml(sample) {
        return ContentType::Yaml;
    }

    // ── 7. Markdown ───────────────────────────────────────────────────────────
    if lookslike_markdown(sample) {
        return ContentType::Markdown;
    }

    // ── 7.5. HTML/XML ────────────────────────────────────────────────────────
    if lookslike_html(sample) {
        return ContentType::Html;
    }

    // ── 7.6. SQL ─────────────────────────────────────────────────────────────
    if lookslike_sql(sample) {
        return ContentType::Sql;
    }

    // ── 7.7. Dockerfile ──────────────────────────────────────────────────────
    if lookslike_dockerfile(sample) {
        return ContentType::Dockerfile;
    }

    // ── 7.8. Terraform ───────────────────────────────────────────────────────
    if lookslike_terraform(sample) {
        return ContentType::Terraform;
    }

    // ── 7.9. .env file ───────────────────────────────────────────────────────
    if lookslike_env(sample) {
        return ContentType::EnvFile;
    }

    // ── 7.95. CSV ────────────────────────────────────────────────────────────
    if lookslike_csv(sample) {
        return ContentType::Csv;
    }

    // ── 8. Code blocks ────────────────────────────────────────────────────────
    if let Some(lang) = detect_code_language(sample) {
        return ContentType::Code(lang);
    }

    // ── 9. ANSI escape codes -> treat as log output ───────────────────────────
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
        "go" => ContentType::Code("go".to_string()),
        "java" => ContentType::Code("java".to_string()),
        "trace" | "stack" => ContentType::StackTrace(StackTraceLang::JavaScript),
        "build" | "cargo" => ContentType::BuildOutput(BuildTool::Cargo),
        "tsc" => ContentType::BuildOutput(BuildTool::TypeScript),
        "eslint" | "lint" => ContentType::BuildOutput(BuildTool::Eslint),
        "md" | "markdown" => ContentType::Markdown,
        "html" | "xml" => ContentType::Html,
        "sql" => ContentType::Sql,
        "csv" => ContentType::Csv,
        "dockerfile" | "docker" => ContentType::Dockerfile,
        "env" | "dotenv" => ContentType::EnvFile,
        "terraform" | "tf" | "hcl" => ContentType::Terraform,
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

fn lookslike_markdown(s: &str) -> bool {
    let lines: Vec<&str> = s.lines().take(40).collect();
    let h_count = lines.iter().filter(|l| l.starts_with('#')).count();
    let badge_count = lines
        .iter()
        .filter(|l| l.contains("![") && l.contains("]("))
        .count();
    let fence_count = lines.iter().filter(|l| l.starts_with("```")).count();
    let link_count = lines.iter().filter(|l| l.contains("](http")).count();
    // Require at least 2 structural markdown signals
    let score = (h_count >= 2) as u8
        + (badge_count >= 1) as u8
        + (fence_count >= 1) as u8
        + (link_count >= 2) as u8;
    score >= 2
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

// ── HTML/XML detection ──────────────────────────────────────────────────────

fn lookslike_html(s: &str) -> bool {
    let trimmed = s.trim_start();
    // Strong signals: doctype, html tag, common structural tags
    if trimmed.starts_with("<!DOCTYPE") || trimmed.starts_with("<!doctype") {
        return true;
    }
    if trimmed.starts_with("<html") || trimmed.starts_with("<HTML") {
        return true;
    }
    if trimmed.starts_with("<?xml") {
        return true;
    }
    // Weaker signals: need multiple HTML tags
    let tag_count = s.lines().filter(|l| {
        let t = l.trim();
        t.starts_with('<') && (
            t.starts_with("<div") || t.starts_with("<span") || t.starts_with("<p")
            || t.starts_with("<h1") || t.starts_with("<h2") || t.starts_with("<h3")
            || t.starts_with("<head") || t.starts_with("<body") || t.starts_with("<meta")
            || t.starts_with("<link") || t.starts_with("<script") || t.starts_with("<style")
            || t.starts_with("<form") || t.starts_with("<input") || t.starts_with("<table")
            || t.starts_with("<ul") || t.starts_with("<ol") || t.starts_with("<li")
            || t.starts_with("<a ") || t.starts_with("<img") || t.starts_with("<br")
            || t.starts_with("<nav") || t.starts_with("<header") || t.starts_with("<footer")
            || t.starts_with("<section") || t.starts_with("<article") || t.starts_with("<main")
            || t.starts_with("</")
        )
    }).count();
    tag_count >= 3
}

// ── SQL detection ───────────────────────────────────────────────────────────

fn lookslike_sql(s: &str) -> bool {
    let upper = s.to_uppercase();
    let sql_keywords = ["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "CREATE TABLE",
                        "ALTER TABLE", "DROP TABLE", "CREATE INDEX", "BEGIN", "COMMIT",
                        "WITH ", "JOIN ", "WHERE ", "GROUP BY", "ORDER BY", "HAVING"];
    let matches = sql_keywords.iter().filter(|kw| upper.contains(*kw)).count();
    // Need at least 2 different SQL keywords
    matches >= 2
}

// ── Dockerfile detection ────────────────────────────────────────────────────

fn lookslike_dockerfile(s: &str) -> bool {
    let keywords = ["FROM ", "RUN ", "COPY ", "ADD ", "CMD ", "ENTRYPOINT ",
                     "EXPOSE ", "ENV ", "WORKDIR ", "ARG ", "LABEL ", "USER ",
                     "VOLUME ", "HEALTHCHECK "];
    let has_from = s.lines().any(|l| l.trim().starts_with("FROM "));
    let keyword_count = keywords.iter().filter(|kw| {
        s.lines().any(|l| l.trim().starts_with(*kw))
    }).count();
    has_from && keyword_count >= 3
}

// ── Terraform/HCL detection ────────────────────────────────────────────────

fn lookslike_terraform(s: &str) -> bool {
    let tf_patterns = [
        "resource \"", "variable \"", "output \"", "data \"", "module \"",
        "provider \"", "terraform {", "locals {",
    ];
    let matches = tf_patterns.iter().filter(|p| s.contains(*p)).count();
    matches >= 2
}

// ── .env file detection ─────────────────────────────────────────────────────

fn re_env_line() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // KEY=value or export KEY=value (no spaces in key, not a comment)
        Regex::new(r"^(?:export\s+)?[A-Z][A-Z0-9_]+=").unwrap()
    })
}

fn lookslike_env(s: &str) -> bool {
    let total_lines = s.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#')).count();
    if total_lines < 2 {
        return false;
    }
    let env_lines = s.lines().filter(|l| re_env_line().is_match(l.trim())).count();
    // At least 60% of non-empty, non-comment lines should be KEY=value
    env_lines >= 2 && (env_lines as f64 / total_lines as f64) >= 0.6
}

// ── CSV detection ───────────────────────────────────────────────────────────

fn lookslike_csv(s: &str) -> bool {
    let lines: Vec<&str> = s.lines().take(10).collect();
    if lines.len() < 2 {
        return false;
    }
    // Check comma-delimited: header and data rows should have consistent field count
    let first_count = lines[0].split(',').count();
    if first_count < 3 {
        return false; // need at least 3 columns
    }
    // Check at least 3 rows have the same field count
    let consistent = lines.iter().filter(|l| l.split(',').count() == first_count).count();
    consistent >= 3
}
