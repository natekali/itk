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

fn re_git_diff() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^(diff --git|--- a/|\+\+\+ b/)").unwrap())
}

fn re_rust_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^\s+\d+:\s+\S").unwrap())
}

fn re_python_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^Traceback \(most recent call last\):").unwrap())
}

fn re_js_trace() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^\s+at\s+\S+\s+\(").unwrap())
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

fn re_log_line() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?m)(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}|\[(?:INFO|WARN|ERROR|DEBUG|TRACE)\]|level=(info|warn|error|debug))"
        ).unwrap()
    })
}

fn re_ansi() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\x1b\[[\d;]*[mGKHF]").unwrap())
}

/// Detect the content type of `text`.
/// If `force_diff` is true, skip detection and return `GitDiff`.
pub fn detect(text: &str, force_diff: bool) -> ContentType {
    if force_diff {
        return ContentType::GitDiff;
    }

    let sample = if text.len() > 4096 { &text[..4096] } else { text };

    if re_git_diff().is_match(sample) {
        return ContentType::GitDiff;
    }

    if re_go_trace().is_match(sample) {
        return ContentType::StackTrace(StackTraceLang::Go);
    }
    if re_java_trace().is_match(sample) {
        return ContentType::StackTrace(StackTraceLang::Java);
    }
    if re_python_trace().is_match(sample) {
        return ContentType::StackTrace(StackTraceLang::Python);
    }
    let js_matches = re_js_trace().find_iter(sample).count();
    if js_matches >= 3 {
        return ContentType::StackTrace(StackTraceLang::JavaScript);
    }
    let rust_matches = re_rust_trace().find_iter(sample).count();
    if rust_matches >= 3 {
        return ContentType::StackTrace(StackTraceLang::Rust);
    }

    let log_matches = re_log_line().find_iter(sample).count();
    if log_matches >= 5 {
        return ContentType::LogFile;
    }

    let trimmed = sample.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && lookslike_json(trimmed) {
        return ContentType::Json;
    }

    if lookslike_yaml(sample) {
        return ContentType::Yaml;
    }

    if let Some(lang) = detect_code_language(sample) {
        return ContentType::Code(lang);
    }

    if re_ansi().is_match(sample) {
        return ContentType::LogFile;
    }

    ContentType::PlainText
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
