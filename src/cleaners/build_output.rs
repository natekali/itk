use crate::detect::BuildTool;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn re_cargo_noise() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Lines that carry zero signal -- drop entirely
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

fn re_eslint_violation() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // "  12:5  error  message text  rule/name"
        Regex::new(r"^\s+(\d+):(\d+)\s+(error|warning)\s+(.+?)\s{2,}([\w/@-]+(?:/[\w-]+)*)\s*$")
            .unwrap()
    })
}

pub fn clean_build_output(s: &str, tool: &BuildTool, aggressive: bool) -> String {
    match tool {
        BuildTool::Cargo => clean_cargo_output(s, aggressive),
        BuildTool::TypeScript => clean_tsc_output(s, aggressive),
        BuildTool::Eslint => clean_eslint_output(s, aggressive),
        BuildTool::Generic => super::log::clean_log(s, aggressive),
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
            // Pipe/gutter lines for code context -- skip for brevity
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
        out.push(err.to_string());
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
                .push(format!("{}:{} -- {}", file, ln, msg));
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
                .split(" -- ")
                .nth(1)
                .unwrap_or(&occurrences[0]);
            out.push(format!("\n{code} ({count}x): {first_msg}"));
            for loc in occurrences {
                let location = loc.split(" -- ").next().unwrap_or(loc);
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
        // Summary line (last line): "X 42 problems (30 errors, 12 warnings)"
        if trimmed.starts_with('\u{2716}') || trimmed.starts_with('\u{2713}') || trimmed.contains("problem") {
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
            out.push(format!("  {rule} ({count}x):"));
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
