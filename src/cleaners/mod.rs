pub mod stack_trace;
pub mod git_diff;
pub mod log;
pub mod json;
pub mod yaml;
pub mod code;
pub mod build_output;
pub mod markdown;
pub mod plain;
pub mod html;
pub mod sql;
pub mod csv;
pub mod dockerfile;
pub mod env;
pub mod terraform;

use crate::detect::ContentType;

pub struct CleanOptions {
    pub aggressive: bool,
    pub _diff_mode: bool,
    pub content_type: ContentType,
}

/// Entry point: clean `input` according to `opts`.
/// Uses catch_unwind as a last resort -- on any internal panic, returns input unchanged.
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

    match &opts.content_type {
        ContentType::StackTrace(lang) => stack_trace::clean_stack_trace(&stripped, lang, opts.aggressive),
        ContentType::GitDiff => git_diff::clean_git_diff(&stripped, opts.aggressive),
        ContentType::LogFile => log::clean_log(&stripped, opts.aggressive),
        ContentType::Json => json::clean_json(&stripped, opts.aggressive),
        ContentType::Yaml => yaml::clean_yaml(&stripped, opts.aggressive),
        ContentType::Code(lang) => code::clean_code(&stripped, lang, opts.aggressive),
        ContentType::BuildOutput(tool) => build_output::clean_build_output(&stripped, tool, opts.aggressive),
        ContentType::Markdown => markdown::clean_markdown(&stripped, opts.aggressive),
        ContentType::Html => html::clean_html(&stripped, opts.aggressive),
        ContentType::Sql => sql::clean_sql(&stripped, opts.aggressive),
        ContentType::Csv => csv::clean_csv(&stripped, opts.aggressive),
        ContentType::Dockerfile => dockerfile::clean_dockerfile(&stripped, opts.aggressive),
        ContentType::EnvFile => env::clean_env(&stripped, opts.aggressive),
        ContentType::Terraform => terraform::clean_terraform(&stripped, opts.aggressive),
        ContentType::PlainText => plain::clean_plain(&stripped),
    }
}

// ── Shared utilities ────────────────────────────────────────────────────────

pub(crate) fn strip_ansi(s: &str) -> String {
    strip_ansi_escapes::strip_str(s)
}

pub(crate) fn collapse_blank_lines(s: &str, max_blanks: usize) -> String {
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
