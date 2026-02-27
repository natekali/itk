//! Centralized terminal styling for ITK.
//!
//! All color/style logic lives here. Uses `owo-colors` with `supports-color`
//! to respect NO_COLOR, FORCE_COLOR, and TTY detection.
//!
//! Rule: stdout content is NEVER colored (goes to clipboard/pipe).
//! Only stderr status messages and interactive stdout (gain dashboard) get color.

use owo_colors::OwoColorize;
use std::io::IsTerminal;
use std::sync::OnceLock;

/// Whether to use color on stderr (status messages, errors).
pub fn use_color_stderr() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| {
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }
        if std::env::var("FORCE_COLOR").is_ok() {
            return true;
        }
        std::io::stderr().is_terminal()
    })
}

/// Whether to use color on stdout (gain dashboard, discover report).
pub fn use_color_stdout() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| {
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }
        if std::env::var("FORCE_COLOR").is_ok() {
            return true;
        }
        std::io::stdout().is_terminal()
    })
}

// ── Semantic formatters (stderr) ────────────────────────────────────────────

/// Error text: red bold. For error messages.
pub fn error(text: &str) -> String {
    if use_color_stderr() {
        format!("{}", text.red().bold())
    } else {
        text.to_string()
    }
}

/// Warning text: yellow. For caution/non-critical issues.
pub fn warning(text: &str) -> String {
    if use_color_stderr() {
        format!("{}", text.yellow())
    } else {
        text.to_string()
    }
}

/// Success text: green bold. For positive outcomes.
pub fn success(text: &str) -> String {
    if use_color_stderr() {
        format!("{}", text.green().bold())
    } else {
        text.to_string()
    }
}

/// Info text: cyan. For type labels, metadata.
pub fn info(text: &str) -> String {
    if use_color_stderr() {
        format!("{}", text.cyan())
    } else {
        text.to_string()
    }
}

/// Dim text: dimmed. For de-emphasized content (borders, timestamps).
pub fn dim(text: &str) -> String {
    if use_color_stderr() {
        format!("{}", text.dimmed())
    } else {
        text.to_string()
    }
}

/// Header text: bold. For section headers.
pub fn header(text: &str) -> String {
    if use_color_stderr() {
        format!("{}", text.bold())
    } else {
        text.to_string()
    }
}

/// Savings indicator: green if positive savings, yellow if negative.
pub fn savings_colored(text: &str, is_positive: bool) -> String {
    if use_color_stderr() {
        if is_positive {
            format!("{}", text.green().bold())
        } else {
            format!("{}", text.yellow())
        }
    } else {
        text.to_string()
    }
}

// ── Stdout-aware formatters (for gain/discover which print to stdout) ───────

pub fn out_dim(text: &str) -> String {
    if use_color_stdout() {
        format!("{}", text.dimmed())
    } else {
        text.to_string()
    }
}

pub fn out_header(text: &str) -> String {
    if use_color_stdout() {
        format!("{}", text.bold())
    } else {
        text.to_string()
    }
}

pub fn out_label(text: &str) -> String {
    if use_color_stdout() {
        format!("{}", text.cyan())
    } else {
        text.to_string()
    }
}

pub fn out_success(text: &str) -> String {
    if use_color_stdout() {
        format!("{}", text.green().bold())
    } else {
        text.to_string()
    }
}

pub fn out_savings(text: &str, is_positive: bool) -> String {
    if use_color_stdout() {
        if is_positive {
            format!("{}", text.green())
        } else {
            format!("{}", text.yellow())
        }
    } else {
        text.to_string()
    }
}

pub fn out_warning(text: &str) -> String {
    if use_color_stdout() {
        format!("{}", text.yellow())
    } else {
        text.to_string()
    }
}

pub fn out_error(text: &str) -> String {
    if use_color_stdout() {
        format!("{}", text.red().bold())
    } else {
        text.to_string()
    }
}
