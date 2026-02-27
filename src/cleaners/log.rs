use regex::Regex;
use std::sync::OnceLock;

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

pub fn clean_log(s: &str, aggressive: bool) -> String {
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
