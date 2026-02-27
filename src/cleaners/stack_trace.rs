use crate::detect::StackTraceLang;
use regex::Regex;
use std::sync::OnceLock;

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

pub fn clean_stack_trace(s: &str, lang: &StackTraceLang, aggressive: bool) -> String {
    match lang {
        StackTraceLang::Python => clean_python_trace(s, aggressive),
        StackTraceLang::JavaScript => clean_js_trace(s, aggressive),
        StackTraceLang::Rust => clean_rust_trace(s, aggressive),
        StackTraceLang::Go => clean_go_trace(s, aggressive),
        StackTraceLang::Java => clean_java_trace(s, aggressive),
        StackTraceLang::Unknown => super::plain::clean_plain(s),
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
