use crate::detect::ContentType;
use regex::Regex;
use std::sync::OnceLock;

/// Lightweight context frame that gives LLMs instant orientation.
pub struct FrameContext {
    pub type_label: String,
    pub line_count: usize,
    pub annotations: Vec<String>,
}

/// Build a frame by extracting heuristic annotations from cleaned content.
pub fn build_frame(content: &str, ct: &ContentType) -> FrameContext {
    let line_count = content.lines().count();
    let type_label = ct.label();
    let annotations = match ct {
        ContentType::Json => annotate_json(content),
        ContentType::Yaml => annotate_yaml(content),
        ContentType::Code(lang) => annotate_code(content, lang),
        ContentType::StackTrace(_) => annotate_stack_trace(content),
        ContentType::GitDiff => annotate_git_diff(content),
        ContentType::LogFile => annotate_log(content),
        ContentType::BuildOutput(_) => annotate_build(content),
        ContentType::Markdown => annotate_markdown(content),
        ContentType::Html => annotate_html(content),
        ContentType::Sql => annotate_sql(content),
        ContentType::Csv => annotate_csv(content),
        ContentType::Dockerfile => annotate_dockerfile(content),
        ContentType::EnvFile => annotate_env(content),
        ContentType::Terraform => annotate_terraform(content),
        ContentType::PlainText => annotate_plain(content),
    };
    FrameContext { type_label, line_count, annotations }
}

/// Render the framed output: [context header] + content.
pub fn render_framed(content: &str, frame: &FrameContext, focus: Option<&str>) -> String {
    let mut parts: Vec<String> = vec![frame.type_label.clone()];
    parts.push(format!("{} lines", frame.line_count));
    parts.extend(frame.annotations.iter().cloned());

    let mut out = format!("[{}]", parts.join(" | "));
    if let Some(f) = focus {
        out.push_str(&format!("\n[Focus: {f}]"));
    }
    out.push('\n');
    out.push_str(content);
    out
}

// ── JSON annotations ─────────────────────────────────────────────────────────

fn annotate_json(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let trimmed = s.trim();

    if trimmed.starts_with('{') {
        // Count top-level keys (heuristic: lines matching `  "key":`)
        let top_keys = re_json_top_key().find_iter(s).count();
        if top_keys > 0 {
            notes.push(format!("{top_keys} top-level keys"));
        }

        // Detect error response
        let error_keys = ["\"error\"", "\"errors\"", "\"message\"", "\"status\"", "\"code\""];
        let has_error = error_keys.iter().any(|k| s.contains(k));
        if has_error {
            notes.push("error response".to_string());
        }
    } else if trimmed.starts_with('[') {
        // Array — count elements (heuristic: top-level objects)
        let obj_count = s.lines().filter(|l| l.trim() == "{" || l.trim() == "},").count();
        if obj_count > 1 {
            notes.push(format!("array of ~{} objects", obj_count / 2));
        }
    }

    // Detect nested depth (simple: max indentation)
    let max_indent = s.lines()
        .map(|l| l.len() - l.trim_start().len())
        .max()
        .unwrap_or(0);
    if max_indent > 12 {
        notes.push(format!("{}+ nesting depth", max_indent / 2));
    }

    notes
}

fn re_json_top_key() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?m)^  "[^"]+"\s*:"#).unwrap())
}

// ── YAML annotations ─────────────────────────────────────────────────────────

fn annotate_yaml(s: &str) -> Vec<String> {
    let mut notes = Vec::new();

    // Kubernetes detection
    if let Some(kind) = detect_k8s_kind(s) {
        notes.push(format!("Kubernetes {kind}"));
        // Count containers
        let containers = s.lines().filter(|l| {
            let t = l.trim();
            t.starts_with("- name:") || t.starts_with("- containerPort:")
        }).count();
        if containers > 0 {
            notes.push(format!("{containers} container(s)"));
        }
        // Detect resource limits
        if s.contains("resources:") {
            notes.push("resource limits set".to_string());
        }
        // Detect probes
        if s.contains("livenessProbe:") || s.contains("readinessProbe:") {
            notes.push("health probes configured".to_string());
        }
        return notes;
    }

    // Docker Compose detection
    if s.contains("services:") && (s.contains("image:") || s.contains("build:")) {
        notes.push("Docker Compose".to_string());
        let services = s.lines().filter(|l| {
            let indent = l.len() - l.trim_start().len();
            indent == 2 && l.trim().ends_with(':') && !l.trim().starts_with('#')
        }).count();
        if services > 0 {
            notes.push(format!("{services} service(s)"));
        }
        return notes;
    }

    // GitHub Actions detection
    if s.contains("on:") && (s.contains("jobs:") || s.contains("steps:")) {
        notes.push("GitHub Actions workflow".to_string());
        let jobs = s.lines().filter(|l| {
            let indent = l.len() - l.trim_start().len();
            indent == 2 && l.trim().ends_with(':') && !l.trim().starts_with('#')
                && !["on:", "jobs:", "name:", "env:"].contains(&l.trim())
        }).count();
        if jobs > 0 {
            notes.push(format!("{jobs} job(s)"));
        }
        return notes;
    }

    // OpenAPI detection
    if s.contains("openapi:") || s.contains("swagger:") {
        notes.push("OpenAPI spec".to_string());
        let paths = s.lines().filter(|l| l.trim().starts_with('/') && l.trim().ends_with(':')).count();
        if paths > 0 {
            notes.push(format!("{paths} endpoint(s)"));
        }
        return notes;
    }

    // Generic YAML — count top-level keys
    let top_keys = s.lines().filter(|l| {
        let trimmed = l.trim();
        !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with('-')
            && l.starts_with(|c: char| c.is_alphanumeric()) && trimmed.contains(':')
    }).count();
    if top_keys > 0 {
        notes.push(format!("{top_keys} top-level keys"));
    }

    notes
}

fn detect_k8s_kind(s: &str) -> Option<String> {
    for line in s.lines() {
        let trimmed = line.trim();
        if let Some(kind) = trimmed.strip_prefix("kind:") {
            let kind = kind.trim().trim_matches('"').trim_matches('\'');
            if !kind.is_empty() {
                return Some(kind.to_string());
            }
        }
    }
    None
}

// ── Code annotations ─────────────────────────────────────────────────────────

fn annotate_code(s: &str, lang: &str) -> Vec<String> {
    let mut notes = Vec::new();

    // Count exported/public functions
    let (export_count, fn_count) = match lang {
        "typescript" | "javascript" | "js" | "ts" => {
            let exports = re_ts_export().find_iter(s).count();
            let fns = re_ts_function().find_iter(s).count();
            (exports, fns)
        }
        "rust" => {
            let pubs = re_rust_pub_fn().find_iter(s).count();
            let fns = re_rust_fn().find_iter(s).count();
            (pubs, fns)
        }
        "python" => {
            let defs = re_python_def().find_iter(s).count();
            (0, defs)
        }
        "go" => {
            let fns = re_go_func().find_iter(s).count();
            let exported = s.lines().filter(|l| {
                let t = l.trim();
                t.starts_with("func ") && t.chars().nth(5).map(|c| c.is_uppercase()).unwrap_or(false)
            }).count();
            (exported, fns)
        }
        _ => (0, 0),
    };

    if export_count > 0 {
        notes.push(format!("{export_count} exported"));
    }
    if fn_count > 0 {
        notes.push(format!("{fn_count} functions"));
    }

    // Count classes/structs/interfaces
    let structs = s.lines().filter(|l| {
        let t = l.trim();
        t.starts_with("struct ") || t.starts_with("pub struct ")
            || t.starts_with("class ") || t.starts_with("export class ")
            || t.starts_with("interface ") || t.starts_with("export interface ")
            || t.starts_with("type ") && t.contains('=') && t.contains('{')
    }).count();
    if structs > 0 {
        notes.push(format!("{structs} types/classes"));
    }

    // Detect test file
    let is_test = s.contains("#[cfg(test)]") || s.contains("#[test]")
        || s.contains("describe(") || s.contains("it(")
        || s.contains("def test_") || s.contains("func Test");
    if is_test {
        notes.push("contains tests".to_string());
    }

    notes
}

fn re_ts_export() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^export\s+(?:function|const|class|interface|type|enum|default)").unwrap())
}

fn re_ts_function() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)(?:^|\s)(?:function|const|let|var)\s+\w+\s*[=(]").unwrap())
}

fn re_rust_pub_fn() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^pub\s+(?:async\s+)?fn\s+").unwrap())
}

fn re_rust_fn() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+").unwrap())
}

fn re_python_def() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^\s*(?:async\s+)?def\s+").unwrap())
}

fn re_go_func() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^func\s+").unwrap())
}

// ── Stack trace annotations ──────────────────────────────────────────────────

fn annotate_stack_trace(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let frame_count = s.lines().filter(|l| {
        let t = l.trim();
        t.starts_with("at ") || t.starts_with("File ") || t.contains(".go:")
            || (t.len() > 3 && t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && t.contains(": "))
    }).count();
    if frame_count > 0 {
        notes.push(format!("{frame_count} frames"));
    }

    // Extract root error (last non-empty, non-frame line)
    let error_line = s.lines().rev().find(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with("at ") && !t.starts_with("File ")
            && !t.contains("truncated") && !t.starts_with("//")
    });
    if let Some(err) = error_line {
        let err = err.trim();
        // Truncate long error messages
        if err.len() > 80 {
            notes.push(format!("{}...", &err[..77]));
        } else {
            notes.push(err.to_string());
        }
    }

    notes
}

// ── Git diff annotations ─────────────────────────────────────────────────────

fn annotate_git_diff(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let file_count = s.lines().filter(|l| l.starts_with("diff --git")).count();
    let added: usize = s.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).count();
    let removed: usize = s.lines().filter(|l| l.starts_with('-') && !l.starts_with("---")).count();

    if file_count > 0 {
        notes.push(format!("{file_count} file(s)"));
    }
    notes.push(format!("+{added}/-{removed}"));

    // Detect renames
    let renames = s.lines().filter(|l| l.starts_with("rename from ") || l.contains("similarity index")).count();
    if renames > 0 {
        notes.push("includes renames".to_string());
    }

    notes
}

// ── Log annotations ──────────────────────────────────────────────────────────

fn annotate_log(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let total = s.lines().count();
    let errors = s.lines().filter(|l| {
        let upper = l.to_uppercase();
        upper.contains("ERROR") || upper.contains("FATAL") || upper.contains("PANIC")
    }).count();
    let warnings = s.lines().filter(|l| l.to_uppercase().contains("WARN")).count();

    if errors > 0 {
        notes.push(format!("{errors} error(s)"));
    }
    if warnings > 0 {
        notes.push(format!("{warnings} warning(s)"));
    }
    if errors == 0 && warnings == 0 {
        notes.push(format!("{total} lines"));
    }

    notes
}

// ── Build output annotations ─────────────────────────────────────────────────

fn annotate_build(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let errors = s.lines().filter(|l| l.contains("error")).count();
    let warnings = s.lines().filter(|l| l.contains("warning")).count();
    if errors > 0 {
        notes.push(format!("{errors} error(s)"));
    }
    if warnings > 0 {
        notes.push(format!("{warnings} warning(s)"));
    }
    notes
}

// ── Markdown annotations ─────────────────────────────────────────────────────

fn annotate_markdown(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let headings = s.lines().filter(|l| l.starts_with('#')).count();
    let code_blocks = s.lines().filter(|l| l.trim().starts_with("```")).count() / 2;
    let words = s.split_whitespace().count();

    notes.push(format!("~{words} words"));
    if headings > 0 {
        notes.push(format!("{headings} sections"));
    }
    if code_blocks > 0 {
        notes.push(format!("{code_blocks} code blocks"));
    }

    notes
}

// ── Plain text annotations ───────────────────────────────────────────────────

fn annotate_plain(s: &str) -> Vec<String> {
    let words = s.split_whitespace().count();
    vec![format!("~{words} words")]
}

// ── HTML annotations ────────────────────────────────────────────────────────

fn annotate_html(s: &str) -> Vec<String> {
    let mut notes = Vec::new();

    let forms = s.lines().filter(|l| l.contains("<form")).count();
    let inputs = s.lines().filter(|l| l.contains("<input")).count();
    let scripts = s.lines().filter(|l| l.contains("<script")).count();

    if forms > 0 {
        notes.push(format!("{forms} form(s)"));
    }
    if inputs > 0 {
        notes.push(format!("{inputs} input(s)"));
    }
    if scripts > 0 {
        notes.push(format!("{scripts} script(s)"));
    }
    if s.contains("<table") {
        notes.push("contains table".to_string());
    }
    if s.contains("<nav") || s.contains("<header") {
        notes.push("has navigation".to_string());
    }

    notes
}

// ── SQL annotations ─────────────────────────────────────────────────────────

fn annotate_sql(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let upper = s.to_uppercase();

    let queries = ["SELECT", "INSERT", "UPDATE", "DELETE"];
    let query_counts: Vec<(&str, usize)> = queries.iter().map(|q| {
        let count = upper.matches(q).count();
        (*q, count)
    }).filter(|(_, c)| *c > 0).collect();

    for (q, c) in &query_counts {
        notes.push(format!("{c} {}", q.to_lowercase()));
    }

    if upper.contains("JOIN") {
        notes.push("JOIN detected".to_string());
    }
    if upper.contains("CREATE TABLE") {
        let tables = upper.matches("CREATE TABLE").count();
        notes.push(format!("{tables} table def(s)"));
    }

    notes
}

// ── CSV annotations ─────────────────────────────────────────────────────────

fn annotate_csv(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let total_rows = s.lines().count().saturating_sub(1);
    let col_count = s.lines().next().map(|l| l.split(',').count()).unwrap_or(0);

    notes.push(format!("{total_rows} rows"));
    notes.push(format!("{col_count} columns"));

    if let Some(header) = s.lines().next() {
        let cols: Vec<&str> = header.split(',').take(5).map(|c| c.trim()).collect();
        let display = if col_count > 5 {
            format!("{}, ...", cols.join(", "))
        } else {
            cols.join(", ")
        };
        notes.push(display);
    }

    notes
}

// ── Dockerfile annotations ──────────────────────────────────────────────────

fn annotate_dockerfile(s: &str) -> Vec<String> {
    let mut notes = Vec::new();

    let stages = s.lines().filter(|l| l.trim().starts_with("FROM ")).count();
    if stages > 1 {
        notes.push(format!("{stages} stages"));
    }

    let exposes: Vec<&str> = s.lines()
        .filter(|l| l.trim().starts_with("EXPOSE "))
        .collect();
    if !exposes.is_empty() {
        let ports: Vec<&str> = exposes.iter()
            .flat_map(|l| l.trim().strip_prefix("EXPOSE ").unwrap_or("").split_whitespace())
            .collect();
        notes.push(format!("exposes {}", ports.join(", ")));
    }

    let runs = s.lines().filter(|l| l.trim().starts_with("RUN ")).count();
    if runs > 0 {
        notes.push(format!("{runs} RUN commands"));
    }

    notes
}

// ── .env annotations ────────────────────────────────────────────────────────

fn annotate_env(s: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let total_vars = s.lines().filter(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#') && t.contains('=')
    }).count();
    let masked = s.lines().filter(|l| l.contains("=***")).count();

    notes.push(format!("{total_vars} vars"));
    if masked > 0 {
        notes.push(format!("{masked} secrets masked"));
    }

    notes
}

// ── Terraform annotations ───────────────────────────────────────────────────

fn annotate_terraform(s: &str) -> Vec<String> {
    let mut notes = Vec::new();

    let resources = s.lines().filter(|l| l.trim().starts_with("resource \"")).count();
    let variables = s.lines().filter(|l| l.trim().starts_with("variable \"")).count();
    let modules = s.lines().filter(|l| l.trim().starts_with("module \"")).count();
    let outputs = s.lines().filter(|l| l.trim().starts_with("output \"")).count();
    let data_sources = s.lines().filter(|l| l.trim().starts_with("data \"")).count();

    if resources > 0 {
        notes.push(format!("{resources} resources"));
    }
    if data_sources > 0 {
        notes.push(format!("{data_sources} data sources"));
    }
    if modules > 0 {
        notes.push(format!("{modules} modules"));
    }
    if variables > 0 {
        notes.push(format!("{variables} variables"));
    }
    if outputs > 0 {
        notes.push(format!("{outputs} outputs"));
    }

    notes
}
